"""Provisioning REST API for HA-VoIP.

Provides ``aiohttp``-based HTTP endpoints for:

* Extension management (create, delete, list, QR provisioning)
* Certificate lifecycle (ACME, manual upload, self-signed generation)
* Network connectivity diagnostics
* Backup and restore

Authentication is enforced via Home Assistant long-lived access tokens
passed in the ``Authorization: Bearer <token>`` header.  A simple
token-bucket rate limiter is applied per-IP.
"""

from __future__ import annotations

import asyncio
import hashlib
import io
import json
import logging
import secrets
import time
import uuid
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Coroutine, Dict, List, Optional, Set

from aiohttp import web

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Rate-limiter
# ---------------------------------------------------------------------------

@dataclass
class _TokenBucket:
    tokens: float
    last_refill: float
    capacity: float = 30.0
    refill_rate: float = 1.0  # tokens per second


class RateLimiter:
    """Simple per-IP token-bucket rate limiter."""

    def __init__(self, capacity: float = 30.0, refill_rate: float = 1.0) -> None:
        self._buckets: Dict[str, _TokenBucket] = {}
        self._capacity = capacity
        self._refill_rate = refill_rate

    def allow(self, ip: str) -> bool:
        now = time.monotonic()
        bucket = self._buckets.get(ip)
        if bucket is None:
            bucket = _TokenBucket(
                tokens=self._capacity,
                last_refill=now,
                capacity=self._capacity,
                refill_rate=self._refill_rate,
            )
            self._buckets[ip] = bucket

        elapsed = now - bucket.last_refill
        bucket.tokens = min(bucket.capacity, bucket.tokens + elapsed * bucket.refill_rate)
        bucket.last_refill = now

        if bucket.tokens >= 1.0:
            bucket.tokens -= 1.0
            return True
        return False


# ---------------------------------------------------------------------------
# Extension data model
# ---------------------------------------------------------------------------

@dataclass
class Extension:
    """SIP extension record."""

    ext_id: str
    username: str
    password: str
    display_name: str = ""
    domain: str = ""
    transport: str = "wss"
    created_at: str = ""
    enabled: bool = True
    callerid: str = ""

    def __post_init__(self) -> None:
        if not self.created_at:
            self.created_at = datetime.now(timezone.utc).isoformat()


class ExtensionStore:
    """Persistent JSON-backed extension storage."""

    def __init__(self, path: str | Path) -> None:
        self._path = Path(path)
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._extensions: Dict[str, Extension] = {}
        self._load()

    def _load(self) -> None:
        if self._path.exists():
            try:
                data = json.loads(self._path.read_text())
                for ext_id, ext_data in data.items():
                    self._extensions[ext_id] = Extension(**ext_data)
            except Exception:
                logger.warning("Failed to load extensions from %s", self._path)

    def _save(self) -> None:
        data = {eid: asdict(ext) for eid, ext in self._extensions.items()}
        self._path.write_text(json.dumps(data, indent=2))

    def create(
        self,
        display_name: str = "",
        domain: str = "",
        transport: str = "wss",
        callerid: str = "",
    ) -> Extension:
        ext_id = str(len(self._extensions) + 1001)
        while ext_id in self._extensions:
            ext_id = str(int(ext_id) + 1)
        username = f"ext{ext_id}"
        password = secrets.token_urlsafe(16)
        ext = Extension(
            ext_id=ext_id,
            username=username,
            password=password,
            display_name=display_name or f"Extension {ext_id}",
            domain=domain,
            transport=transport,
            callerid=callerid or ext_id,
        )
        self._extensions[ext_id] = ext
        self._save()
        logger.info("Created extension %s (%s)", ext_id, username)
        return ext

    def delete(self, ext_id: str) -> bool:
        if ext_id in self._extensions:
            del self._extensions[ext_id]
            self._save()
            logger.info("Deleted extension %s", ext_id)
            return True
        return False

    def get(self, ext_id: str) -> Optional[Extension]:
        return self._extensions.get(ext_id)

    def list_all(self) -> List[Extension]:
        return list(self._extensions.values())

    def to_dict(self) -> Dict:
        return {eid: asdict(ext) for eid, ext in self._extensions.items()}


# ---------------------------------------------------------------------------
# API application builder
# ---------------------------------------------------------------------------

class ProvisioningAPI:
    """Aiohttp-based REST API for HA-VoIP provisioning.

    Parameters
    ----------
    data_dir:
        Root data directory for extensions, state, etc.
    valid_tokens:
        Set of Home Assistant long-lived tokens that are authorized
        to access the API.  If empty, authentication is disabled
        (for development only).
    cert_manager:
        An initialized ``CertificateManager`` instance.
    network_tester:
        An initialized ``NetworkTester`` instance.
    backup_manager:
        An initialized ``BackupManager`` instance.
    sip_domain:
        Default SIP domain for extensions.
    wss_url:
        WebSocket Secure URL for SIP clients.
    """

    def __init__(
        self,
        data_dir: str | Path,
        valid_tokens: Optional[Set[str]] = None,
        cert_manager: Any = None,
        network_tester: Any = None,
        backup_manager: Any = None,
        sip_domain: str = "",
        wss_url: str = "",
    ) -> None:
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.valid_tokens = valid_tokens or set()
        self.cert_manager = cert_manager
        self.network_tester = network_tester
        self.backup_manager = backup_manager
        self.sip_domain = sip_domain
        self.wss_url = wss_url

        self.extensions = ExtensionStore(self.data_dir / "extensions.json")
        self._rate_limiter = RateLimiter()

        # In-memory storage for async test results
        self._network_test_results: Dict[str, Any] = {}

    # ------------------------------------------------------------------
    # Authentication middleware
    # ------------------------------------------------------------------
    @web.middleware
    async def auth_middleware(
        self,
        request: web.Request,
        handler: Callable,
    ) -> web.StreamResponse:
        """Verify Bearer token and enforce rate limiting."""
        # Rate-limit
        peer = request.remote or "unknown"
        if not self._rate_limiter.allow(peer):
            return web.json_response(
                {"error": "Rate limit exceeded"}, status=429
            )

        # Skip auth if no tokens configured (dev mode)
        if not self.valid_tokens:
            return await handler(request)

        auth_header = request.headers.get("Authorization", "")
        if not auth_header.startswith("Bearer "):
            return web.json_response(
                {"error": "Missing or invalid Authorization header"}, status=401
            )
        token = auth_header[7:]
        if token not in self.valid_tokens:
            return web.json_response({"error": "Invalid token"}, status=403)

        return await handler(request)

    # ------------------------------------------------------------------
    # Application factory
    # ------------------------------------------------------------------
    def create_app(self) -> web.Application:
        """Build and return the ``aiohttp.web.Application``."""
        app = web.Application(middlewares=[self.auth_middleware])
        app.router.add_routes(
            [
                # Extensions
                web.post("/api/provision/extension", self.create_extension),
                web.delete(
                    "/api/provision/extension/{ext_id}",
                    self.delete_extension,
                ),
                web.get("/api/provision/extensions", self.list_extensions),
                web.get(
                    "/api/provision/qr/{extension_id}",
                    self.get_extension_qr,
                ),
                # Certificates
                web.post(
                    "/api/provision/certificate/request",
                    self.request_certificate,
                ),
                web.post(
                    "/api/provision/certificate/upload",
                    self.upload_certificate,
                ),
                web.post(
                    "/api/provision/certificate/generate-local",
                    self.generate_local_certificate,
                ),
                web.get(
                    "/api/provision/certificate/status",
                    self.certificate_status,
                ),
                # Network tests
                web.post(
                    "/api/provision/network-test",
                    self.run_network_test,
                ),
                web.get(
                    "/api/provision/network-test/results",
                    self.get_network_test_results,
                ),
                # Backup / restore
                web.post("/api/provision/backup", self.create_backup),
                web.post("/api/provision/restore", self.restore_backup),
            ]
        )
        return app

    # ------------------------------------------------------------------
    # Extension endpoints
    # ------------------------------------------------------------------
    async def create_extension(self, request: web.Request) -> web.Response:
        """POST /api/provision/extension

        Body (JSON): ``{"display_name": "...", "transport": "wss"}``
        """
        try:
            body = await request.json()
        except Exception:
            body = {}

        display_name = body.get("display_name", "")
        transport = body.get("transport", "wss")
        callerid = body.get("callerid", "")

        ext = self.extensions.create(
            display_name=display_name,
            domain=self.sip_domain,
            transport=transport,
            callerid=callerid,
        )

        return web.json_response(
            {
                "status": "created",
                "extension": asdict(ext),
            },
            status=201,
        )

    async def delete_extension(self, request: web.Request) -> web.Response:
        """DELETE /api/provision/extension/{ext_id}"""
        ext_id = request.match_info["ext_id"]
        if self.extensions.delete(ext_id):
            return web.json_response({"status": "deleted", "ext_id": ext_id})
        return web.json_response(
            {"error": f"Extension {ext_id} not found"}, status=404
        )

    async def list_extensions(self, request: web.Request) -> web.Response:
        """GET /api/provision/extensions"""
        exts = self.extensions.list_all()
        return web.json_response(
            {"extensions": [asdict(e) for e in exts]}
        )

    async def get_extension_qr(self, request: web.Request) -> web.Response:
        """GET /api/provision/qr/{extension_id}

        Returns a PNG QR code encoding SIP client configuration.
        """
        ext_id = request.match_info["extension_id"]
        ext = self.extensions.get(ext_id)
        if ext is None:
            return web.json_response(
                {"error": f"Extension {ext_id} not found"}, status=404
            )

        # Build SIP provisioning URI
        config = {
            "sip_user": ext.username,
            "sip_password": ext.password,
            "sip_domain": ext.domain or self.sip_domain,
            "transport": ext.transport,
            "display_name": ext.display_name,
            "wss_url": self.wss_url,
        }

        try:
            import qrcode  # type: ignore[import-untyped]

            qr = qrcode.QRCode(
                version=None,
                error_correction=qrcode.constants.ERROR_CORRECT_M,
                box_size=8,
                border=4,
            )
            qr.add_data(json.dumps(config))
            qr.make(fit=True)
            img = qr.make_image(fill_color="black", back_color="white")

            buf = io.BytesIO()
            img.save(buf, format="PNG")
            buf.seek(0)

            return web.Response(
                body=buf.read(),
                content_type="image/png",
                headers={
                    "Content-Disposition": f'inline; filename="ext_{ext_id}_qr.png"'
                },
            )
        except ImportError:
            # Fallback: return JSON config if qrcode library not available
            logger.warning(
                "qrcode library not installed; returning JSON config instead of QR image"
            )
            return web.json_response(
                {
                    "config": config,
                    "note": "Install 'qrcode[pil]' for QR code image generation",
                }
            )

    # ------------------------------------------------------------------
    # Certificate endpoints
    # ------------------------------------------------------------------
    async def request_certificate(self, request: web.Request) -> web.Response:
        """POST /api/provision/certificate/request

        Body: ``{"domain": "example.com", "challenge_type": "http-01"}``
        """
        if self.cert_manager is None:
            return web.json_response(
                {"error": "Certificate manager not configured"}, status=503
            )

        try:
            body = await request.json()
        except Exception:
            return web.json_response(
                {"error": "Invalid JSON body"}, status=400
            )

        domain = body.get("domain")
        if not domain:
            return web.json_response(
                {"error": "domain is required"}, status=400
            )

        challenge_type = body.get("challenge_type", "http-01")
        if challenge_type not in ("http-01", "dns-01"):
            return web.json_response(
                {"error": "challenge_type must be 'http-01' or 'dns-01'"},
                status=400,
            )

        try:
            self.cert_manager.domain = domain
            self.cert_manager.acme_challenge_type = challenge_type
            bundle = await self.cert_manager.auto_provision()
            return web.json_response(
                {
                    "status": "issued",
                    "domain": domain,
                    "fingerprint": bundle.fingerprint_sha256,
                    "not_after": bundle.not_after.isoformat() if bundle.not_after else None,
                }
            )
        except Exception as exc:
            logger.exception("Certificate request failed")
            return web.json_response(
                {"error": str(exc)}, status=500
            )

    async def upload_certificate(self, request: web.Request) -> web.Response:
        """POST /api/provision/certificate/upload

        Expects a multipart form with ``cert`` and ``key`` file fields,
        and an optional ``chain`` field.
        """
        if self.cert_manager is None:
            return web.json_response(
                {"error": "Certificate manager not configured"}, status=503
            )

        try:
            reader = await request.multipart()
        except Exception:
            return web.json_response(
                {"error": "Expected multipart/form-data"}, status=400
            )

        cert_data: Optional[bytes] = None
        key_data: Optional[bytes] = None
        chain_data: Optional[bytes] = None
        domain: Optional[str] = None

        async for part in reader:
            name = part.name
            data = await part.read()
            if name == "cert":
                cert_data = data
            elif name == "key":
                key_data = data
            elif name == "chain":
                chain_data = data
            elif name == "domain":
                domain = data.decode() if isinstance(data, bytes) else str(data)

        if cert_data is None or key_data is None:
            return web.json_response(
                {"error": "'cert' and 'key' fields are required"}, status=400
            )

        try:
            # Write temp files for the upload_manual interface
            tmp_dir = self.data_dir / "tmp_upload"
            tmp_dir.mkdir(exist_ok=True)
            cert_path = tmp_dir / "cert.pem"
            key_path = tmp_dir / "key.pem"
            cert_path.write_bytes(cert_data)
            key_path.write_bytes(key_data)

            chain_path = None
            if chain_data:
                chain_path = tmp_dir / "chain.pem"
                chain_path.write_bytes(chain_data)

            bundle = self.cert_manager.upload_manual(
                cert_path=cert_path,
                key_path=key_path,
                chain_path=chain_path,
                domain=domain,
            )

            # Cleanup temp files
            cert_path.unlink(missing_ok=True)
            key_path.unlink(missing_ok=True)
            if chain_path:
                chain_path.unlink(missing_ok=True)

            return web.json_response(
                {
                    "status": "uploaded",
                    "domain": bundle.domain,
                    "fingerprint": bundle.fingerprint_sha256,
                    "not_after": bundle.not_after.isoformat() if bundle.not_after else None,
                }
            )
        except Exception as exc:
            logger.exception("Certificate upload failed")
            return web.json_response({"error": str(exc)}, status=500)

    async def generate_local_certificate(self, request: web.Request) -> web.Response:
        """POST /api/provision/certificate/generate-local

        Body: ``{"domain": "ha-voip.local", "san_list": ["192.168.1.100"]}``
        """
        if self.cert_manager is None:
            return web.json_response(
                {"error": "Certificate manager not configured"}, status=503
            )

        try:
            body = await request.json()
        except Exception:
            body = {}

        domain = body.get("domain")
        san_list = body.get("san_list", [])

        try:
            bundle = self.cert_manager.generate_local(
                domain=domain, san_list=san_list
            )
            # Also return CA install instructions
            instructions = ""
            try:
                instructions = self.cert_manager.local_ca.generate_install_instructions()
            except Exception:
                pass

            return web.json_response(
                {
                    "status": "generated",
                    "domain": bundle.domain,
                    "fingerprint": bundle.fingerprint_sha256,
                    "not_after": bundle.not_after.isoformat() if bundle.not_after else None,
                    "install_instructions": instructions,
                }
            )
        except Exception as exc:
            logger.exception("Local certificate generation failed")
            return web.json_response({"error": str(exc)}, status=500)

    async def certificate_status(self, request: web.Request) -> web.Response:
        """GET /api/provision/certificate/status"""
        if self.cert_manager is None:
            return web.json_response(
                {"error": "Certificate manager not configured"}, status=503
            )

        status = self.cert_manager.get_cert_status()
        return web.json_response(
            {
                "mode": status.mode.value,
                "domain": status.domain,
                "has_certificate": status.has_certificate,
                "fingerprint_sha256": status.fingerprint_sha256,
                "issuer": status.issuer,
                "not_before": status.not_before,
                "not_after": status.not_after,
                "days_until_expiry": status.days_until_expiry,
                "is_expired": status.is_expired,
                "renewal_pending": status.renewal_pending,
                "last_error": status.last_error,
            }
        )

    # ------------------------------------------------------------------
    # Network test endpoints
    # ------------------------------------------------------------------
    async def run_network_test(self, request: web.Request) -> web.Response:
        """POST /api/provision/network-test

        Body: ``{"host": "example.com", "tests": ["wss", "turn", "stun"]}``
        Starts diagnostics asynchronously and returns a test ID.
        """
        if self.network_tester is None:
            return web.json_response(
                {"error": "Network tester not configured"}, status=503
            )

        try:
            body = await request.json()
        except Exception:
            body = {}

        test_id = str(uuid.uuid4())
        self._network_test_results[test_id] = {
            "status": "running",
            "started_at": datetime.now(timezone.utc).isoformat(),
        }

        # Run in background
        asyncio.create_task(
            self._run_network_test_async(test_id, body)
        )

        return web.json_response(
            {"test_id": test_id, "status": "running"}, status=202
        )

    async def _run_network_test_async(
        self, test_id: str, params: Dict
    ) -> None:
        try:
            result = await self.network_tester.run_full_diagnostic(
                host=params.get("host"),
                stun_server=params.get("stun_server"),
            )
            self._network_test_results[test_id] = {
                "status": "completed",
                "completed_at": datetime.now(timezone.utc).isoformat(),
                "result": result.to_dict() if hasattr(result, "to_dict") else asdict(result),
            }
        except Exception as exc:
            self._network_test_results[test_id] = {
                "status": "error",
                "error": str(exc),
            }

    async def get_network_test_results(self, request: web.Request) -> web.Response:
        """GET /api/provision/network-test/results?test_id=...

        Returns results for a specific test or all recent tests.
        """
        test_id = request.query.get("test_id")
        if test_id:
            result = self._network_test_results.get(test_id)
            if result is None:
                return web.json_response(
                    {"error": "Test not found"}, status=404
                )
            return web.json_response({"test_id": test_id, **result})

        return web.json_response({"tests": self._network_test_results})

    # ------------------------------------------------------------------
    # Backup endpoints
    # ------------------------------------------------------------------
    async def create_backup(self, request: web.Request) -> web.Response:
        """POST /api/provision/backup"""
        if self.backup_manager is None:
            return web.json_response(
                {"error": "Backup manager not configured"}, status=503
            )

        try:
            body = await request.json()
        except Exception:
            body = {}

        passphrase = body.get("passphrase")

        try:
            backup_path = await self.backup_manager.create_backup(
                passphrase=passphrase,
            )
            return web.json_response(
                {
                    "status": "created",
                    "path": str(backup_path),
                }
            )
        except Exception as exc:
            logger.exception("Backup creation failed")
            return web.json_response({"error": str(exc)}, status=500)

    async def restore_backup(self, request: web.Request) -> web.Response:
        """POST /api/provision/restore

        Body: ``{"path": "/path/to/backup.tar.gz.enc", "passphrase": "..."}``
        """
        if self.backup_manager is None:
            return web.json_response(
                {"error": "Backup manager not configured"}, status=503
            )

        try:
            body = await request.json()
        except Exception:
            return web.json_response(
                {"error": "Invalid JSON body"}, status=400
            )

        backup_path = body.get("path")
        if not backup_path:
            return web.json_response(
                {"error": "'path' is required"}, status=400
            )

        passphrase = body.get("passphrase")
        selective = body.get("selective")  # e.g., ["config", "db", "certs"]

        try:
            await self.backup_manager.restore_backup(
                backup_path=backup_path,
                passphrase=passphrase,
                selective=selective,
            )
            return web.json_response({"status": "restored"})
        except Exception as exc:
            logger.exception("Restore failed")
            return web.json_response({"error": str(exc)}, status=500)


def create_provisioning_app(
    data_dir: str | Path,
    valid_tokens: Optional[Set[str]] = None,
    cert_manager: Any = None,
    network_tester: Any = None,
    backup_manager: Any = None,
    sip_domain: str = "",
    wss_url: str = "",
) -> web.Application:
    """Convenience factory for the provisioning aiohttp application."""
    api = ProvisioningAPI(
        data_dir=data_dir,
        valid_tokens=valid_tokens,
        cert_manager=cert_manager,
        network_tester=network_tester,
        backup_manager=backup_manager,
        sip_domain=sip_domain,
        wss_url=wss_url,
    )
    return api.create_app()
