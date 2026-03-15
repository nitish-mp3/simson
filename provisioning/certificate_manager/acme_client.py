"""ACME client for Let's Encrypt certificate provisioning.

Implements the ACME v2 protocol (RFC 8555) using aiohttp for HTTP
communication and the ``cryptography`` library for all crypto operations.
Supports both HTTP-01 and DNS-01 challenges, staging and production
Let's Encrypt endpoints, automatic renewal scheduling, and certificate
chain validation.
"""

from __future__ import annotations

import asyncio
import base64
import hashlib
import json
import logging
import os
import time
from dataclasses import dataclass, field
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any, Dict, List, Literal, Optional, Tuple

import aiohttp
from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, rsa, padding
from cryptography.hazmat.primitives.serialization import (
    Encoding,
    NoEncryption,
    PrivateFormat,
    PublicFormat,
)
from cryptography.x509.oid import NameOID, ExtensionOID

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# ACME directory URLs
# ---------------------------------------------------------------------------
LE_PRODUCTION_DIRECTORY = "https://acme-v02.api.letsencrypt.org/directory"
LE_STAGING_DIRECTORY = "https://acme-staging-v02.api.letsencrypt.org/directory"

# ---------------------------------------------------------------------------
# Retry / timing constants
# ---------------------------------------------------------------------------
MAX_RETRIES = 5
RETRY_BACKOFF_BASE = 2.0  # seconds
CHALLENGE_POLL_INTERVAL = 2.0  # seconds
CHALLENGE_POLL_TIMEOUT = 300  # seconds
ORDER_POLL_TIMEOUT = 300  # seconds
RENEWAL_THRESHOLD_DAYS = 30


@dataclass
class ACMEChallenge:
    """Represents a pending ACME challenge."""

    type: str  # "http-01" or "dns-01"
    url: str
    token: str
    key_authorization: str
    domain: str
    status: str = "pending"


@dataclass
class ACMEOrder:
    """Tracks the state of an ACME certificate order."""

    order_url: str
    domains: List[str]
    authorizations: List[str]
    finalize_url: str
    certificate_url: Optional[str] = None
    status: str = "pending"
    challenges: List[ACMEChallenge] = field(default_factory=list)


def _b64url(data: bytes) -> str:
    """Base64url-encode *data* without padding (RFC 7515)."""
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode("ascii")


def _b64url_decode(s: str) -> bytes:
    """Decode a base64url string."""
    s += "=" * (-len(s) % 4)
    return base64.urlsafe_b64decode(s)


class ACMEClient:
    """Async ACME v2 client for certificate lifecycle management.

    Parameters
    ----------
    data_dir:
        Directory where account keys, certificates, and state are stored.
    email:
        Contact email registered with the ACME CA.
    staging:
        If ``True``, use the Let's Encrypt staging environment.
    challenge_dir:
        Directory to write HTTP-01 challenge files to.  Must be served
        at ``http://<domain>/.well-known/acme-challenge/``.
    dns_hook:
        Optional async callable ``(domain, txt_value) -> None`` used to
        provision DNS-01 TXT records.
    dns_cleanup_hook:
        Optional async callable ``(domain, txt_value) -> None`` to remove
        DNS-01 TXT records after validation.
    """

    def __init__(
        self,
        data_dir: str | Path,
        email: str,
        *,
        staging: bool = False,
        challenge_dir: str | Path | None = None,
        dns_hook: Any = None,
        dns_cleanup_hook: Any = None,
    ) -> None:
        self.data_dir = Path(data_dir)
        self.email = email
        self.staging = staging
        self.directory_url = LE_STAGING_DIRECTORY if staging else LE_PRODUCTION_DIRECTORY

        self.challenge_dir = Path(challenge_dir) if challenge_dir else self.data_dir / "challenges"
        self.dns_hook = dns_hook
        self.dns_cleanup_hook = dns_cleanup_hook

        # Internal state -------------------------------------------------
        self._directory: Dict[str, Any] = {}
        self._account_key: Optional[ec.EllipticCurvePrivateKey] = None
        self._account_url: Optional[str] = None
        self._nonce: Optional[str] = None
        self._session: Optional[aiohttp.ClientSession] = None
        self._renewal_task: Optional[asyncio.Task[None]] = None

        # Ensure directory structure
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.challenge_dir.mkdir(parents=True, exist_ok=True)
        (self.data_dir / "certs").mkdir(exist_ok=True)
        (self.data_dir / "keys").mkdir(exist_ok=True)

    # ------------------------------------------------------------------
    # Context manager helpers
    # ------------------------------------------------------------------
    async def _get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession()
        return self._session

    async def close(self) -> None:
        """Shut down the HTTP session and cancel any running renewal task."""
        if self._renewal_task and not self._renewal_task.done():
            self._renewal_task.cancel()
            try:
                await self._renewal_task
            except asyncio.CancelledError:
                pass
        if self._session and not self._session.closed:
            await self._session.close()

    # ------------------------------------------------------------------
    # Account key management
    # ------------------------------------------------------------------
    def _account_key_path(self) -> Path:
        return self.data_dir / "account.key"

    async def _load_or_create_account_key(self) -> ec.EllipticCurvePrivateKey:
        """Load existing account key or generate a new ECDSA P-256 key."""
        key_path = self._account_key_path()
        if key_path.exists():
            pem = key_path.read_bytes()
            key = serialization.load_pem_private_key(pem, password=None)
            if not isinstance(key, ec.EllipticCurvePrivateKey):
                raise TypeError("Account key must be ECDSA")
            logger.info("Loaded existing ACME account key from %s", key_path)
            return key

        key = ec.generate_private_key(ec.SECP256R1())
        pem = key.private_bytes(Encoding.PEM, PrivateFormat.PKCS8, NoEncryption())
        key_path.write_bytes(pem)
        # Restrict file permissions where supported
        try:
            os.chmod(key_path, 0o600)
        except OSError:
            pass
        logger.info("Generated new ACME account key at %s", key_path)
        return key

    def _jwk(self) -> Dict[str, str]:
        """Return the JSON Web Key (public) for the account key."""
        assert self._account_key is not None
        pub = self._account_key.public_key()
        nums = pub.public_numbers()
        x_bytes = nums.x.to_bytes(32, "big")
        y_bytes = nums.y.to_bytes(32, "big")
        return {
            "kty": "EC",
            "crv": "P-256",
            "x": _b64url(x_bytes),
            "y": _b64url(y_bytes),
        }

    def _thumbprint(self) -> str:
        """Compute the JWK thumbprint (RFC 7638) of the account key."""
        jwk = self._jwk()
        # Canonical JSON with sorted keys
        canonical = json.dumps(
            {"crv": jwk["crv"], "kty": jwk["kty"], "x": jwk["x"], "y": jwk["y"]},
            separators=(",", ":"),
            sort_keys=True,
        ).encode()
        digest = hashlib.sha256(canonical).digest()
        return _b64url(digest)

    # ------------------------------------------------------------------
    # ACME directory & nonce management
    # ------------------------------------------------------------------
    async def _fetch_directory(self) -> Dict[str, Any]:
        session = await self._get_session()
        async with session.get(self.directory_url) as resp:
            resp.raise_for_status()
            self._directory = await resp.json()
            return self._directory

    async def _get_nonce(self) -> str:
        if self._nonce:
            nonce = self._nonce
            self._nonce = None
            return nonce
        session = await self._get_session()
        url = self._directory.get("newNonce", self.directory_url)
        async with session.head(url) as resp:
            nonce = resp.headers["Replay-Nonce"]
            return nonce

    # ------------------------------------------------------------------
    # JWS signing
    # ------------------------------------------------------------------
    def _sign_jws(
        self,
        payload: Any,
        url: str,
        nonce: str,
        use_jwk: bool = False,
    ) -> Dict[str, str]:
        """Create a Flattened JWS (RFC 7515) with ES256."""
        assert self._account_key is not None

        protected: Dict[str, Any] = {
            "alg": "ES256",
            "nonce": nonce,
            "url": url,
        }
        if use_jwk:
            protected["jwk"] = self._jwk()
        else:
            assert self._account_url is not None
            protected["kid"] = self._account_url

        protected_b64 = _b64url(json.dumps(protected).encode())

        if payload is None:
            # POST-as-GET
            payload_b64 = ""
        elif payload == "":
            payload_b64 = ""
        else:
            payload_b64 = _b64url(json.dumps(payload).encode())

        signing_input = f"{protected_b64}.{payload_b64}".encode("ascii")
        der_sig = self._account_key.sign(signing_input, ec.ECDSA(hashes.SHA256()))

        # Convert DER signature to raw r||s (64 bytes for P-256)
        from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature

        r, s = decode_dss_signature(der_sig)
        sig_bytes = r.to_bytes(32, "big") + s.to_bytes(32, "big")

        return {
            "protected": protected_b64,
            "payload": payload_b64,
            "signature": _b64url(sig_bytes),
        }

    async def _acme_request(
        self,
        url: str,
        payload: Any = None,
        use_jwk: bool = False,
        max_retries: int = MAX_RETRIES,
    ) -> Tuple[Dict[str, Any] | bytes, Dict[str, str]]:
        """Send a signed ACME request with retry and back-off."""
        session = await self._get_session()

        for attempt in range(max_retries):
            nonce = await self._get_nonce()
            body = self._sign_jws(payload, url, nonce, use_jwk=use_jwk)
            headers = {"Content-Type": "application/jose+json"}

            async with session.post(url, json=body, headers=headers) as resp:
                # Capture replay nonce from response
                if "Replay-Nonce" in resp.headers:
                    self._nonce = resp.headers["Replay-Nonce"]

                resp_headers = dict(resp.headers)

                if resp.status == 400:
                    error_body = await resp.json()
                    if error_body.get("type") == "urn:ietf:params:acme:error:badNonce":
                        logger.warning("Bad nonce (attempt %d/%d), retrying", attempt + 1, max_retries)
                        self._nonce = None
                        await asyncio.sleep(RETRY_BACKOFF_BASE ** attempt)
                        continue
                    raise ACMEError(
                        f"ACME error: {error_body.get('detail', error_body)}",
                        status=resp.status,
                        acme_type=error_body.get("type", ""),
                    )

                if resp.status >= 400:
                    text = await resp.text()
                    raise ACMEError(
                        f"ACME request failed ({resp.status}): {text}",
                        status=resp.status,
                    )

                content_type = resp.headers.get("Content-Type", "")
                if "json" in content_type:
                    data = await resp.json()
                else:
                    data = await resp.read()

                return data, resp_headers

        raise ACMEError("Max retries exceeded for ACME request")

    # ------------------------------------------------------------------
    # Account registration
    # ------------------------------------------------------------------
    async def _register_account(self) -> str:
        """Register or fetch existing ACME account. Returns account URL."""
        url = self._directory["newAccount"]
        payload = {
            "termsOfServiceAgreed": True,
            "contact": [f"mailto:{self.email}"],
        }
        data, headers = await self._acme_request(url, payload, use_jwk=True)
        account_url = headers.get("Location", "")
        logger.info("ACME account registered/fetched: %s", account_url)
        return account_url

    async def _ensure_initialized(self) -> None:
        """Ensure directory fetched, account key loaded, and account registered."""
        if not self._directory:
            await self._fetch_directory()
        if self._account_key is None:
            self._account_key = await self._load_or_create_account_key()
        if self._account_url is None:
            self._account_url = await self._register_account()

    # ------------------------------------------------------------------
    # CSR generation
    # ------------------------------------------------------------------
    @staticmethod
    def _generate_csr(
        domain: str,
        san_list: Optional[List[str]] = None,
    ) -> Tuple[bytes, rsa.RSAPrivateKey]:
        """Generate a CSR and new RSA-2048 private key for *domain*.

        Returns ``(csr_der, private_key)``.
        """
        key = rsa.generate_private_key(public_exponent=65537, key_size=2048)

        names = [x509.DNSName(domain)]
        if san_list:
            for name in san_list:
                if name != domain:
                    names.append(x509.DNSName(name))

        csr_builder = (
            x509.CertificateSigningRequestBuilder()
            .subject_name(x509.Name([x509.NameAttribute(NameOID.COMMON_NAME, domain)]))
            .add_extension(x509.SubjectAlternativeName(names), critical=False)
        )
        csr = csr_builder.sign(key, hashes.SHA256())
        return csr.public_bytes(Encoding.DER), key

    # ------------------------------------------------------------------
    # Challenge handling
    # ------------------------------------------------------------------
    async def _handle_http01(self, challenge: ACMEChallenge) -> None:
        """Write the HTTP-01 challenge response file."""
        token_path = self.challenge_dir / challenge.token
        token_path.write_text(challenge.key_authorization)
        logger.info("HTTP-01 challenge file written: %s", token_path)

    async def _handle_dns01(self, challenge: ACMEChallenge) -> None:
        """Provision a DNS TXT record for the DNS-01 challenge."""
        if self.dns_hook is None:
            raise ACMEError(
                "DNS-01 challenge requested but no dns_hook was provided"
            )
        # The TXT value is the base64url-encoded SHA-256 of the key authorization
        txt_value = _b64url(
            hashlib.sha256(challenge.key_authorization.encode()).digest()
        )
        record_name = f"_acme-challenge.{challenge.domain}"
        await self.dns_hook(record_name, txt_value)
        logger.info("DNS-01 TXT record provisioned for %s", record_name)

    async def _cleanup_http01(self, challenge: ACMEChallenge) -> None:
        token_path = self.challenge_dir / challenge.token
        if token_path.exists():
            token_path.unlink()

    async def _cleanup_dns01(self, challenge: ACMEChallenge) -> None:
        if self.dns_cleanup_hook:
            txt_value = _b64url(
                hashlib.sha256(challenge.key_authorization.encode()).digest()
            )
            record_name = f"_acme-challenge.{challenge.domain}"
            await self.dns_cleanup_hook(record_name, txt_value)

    async def _respond_to_challenge(self, challenge_url: str) -> None:
        """Tell the CA we are ready for validation."""
        await self._acme_request(challenge_url, {})

    async def _poll_challenge(self, challenge_url: str) -> str:
        """Poll until the challenge is valid or invalid."""
        deadline = time.monotonic() + CHALLENGE_POLL_TIMEOUT
        while time.monotonic() < deadline:
            data, _ = await self._acme_request(challenge_url, payload=None)
            status = data.get("status", "pending") if isinstance(data, dict) else "pending"
            if status == "valid":
                return "valid"
            if status == "invalid":
                detail = data.get("error", {}).get("detail", "unknown") if isinstance(data, dict) else "unknown"
                raise ACMEError(f"Challenge validation failed: {detail}")
            await asyncio.sleep(CHALLENGE_POLL_INTERVAL)
        raise ACMEError("Challenge validation timed out")

    async def _poll_order(self, order_url: str) -> Dict[str, Any]:
        """Poll order status until ready/valid or error."""
        deadline = time.monotonic() + ORDER_POLL_TIMEOUT
        while time.monotonic() < deadline:
            data, _ = await self._acme_request(order_url, payload=None)
            if isinstance(data, dict):
                status = data.get("status", "pending")
                if status in ("ready", "valid"):
                    return data
                if status == "invalid":
                    raise ACMEError(f"Order became invalid: {data}")
            await asyncio.sleep(CHALLENGE_POLL_INTERVAL)
        raise ACMEError("Order polling timed out")

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------
    async def request_certificate(
        self,
        domain: str,
        challenge_type: Literal["http-01", "dns-01"] = "http-01",
        san_list: Optional[List[str]] = None,
    ) -> Tuple[bytes, bytes, bytes]:
        """Request a new certificate from the ACME CA.

        Parameters
        ----------
        domain:
            The primary domain name for the certificate.
        challenge_type:
            ``"http-01"`` or ``"dns-01"``.
        san_list:
            Additional Subject Alternative Names.

        Returns
        -------
        tuple of (certificate_pem, private_key_pem, chain_pem)
        """
        logger.info(
            "Requesting certificate for %s via %s (staging=%s)",
            domain,
            challenge_type,
            self.staging,
        )
        await self._ensure_initialized()

        # 1. Create order
        identifiers = [{"type": "dns", "value": domain}]
        if san_list:
            for name in san_list:
                if name != domain:
                    identifiers.append({"type": "dns", "value": name})

        order_payload = {"identifiers": identifiers}
        order_data, order_headers = await self._acme_request(
            self._directory["newOrder"], order_payload
        )
        assert isinstance(order_data, dict)
        order_url = order_headers.get("Location", "")
        order = ACMEOrder(
            order_url=order_url,
            domains=[i["value"] for i in identifiers],
            authorizations=order_data["authorizations"],
            finalize_url=order_data["finalize"],
            status=order_data.get("status", "pending"),
        )
        logger.info("ACME order created: %s (status=%s)", order_url, order.status)

        # 2. Process authorizations
        if order.status == "pending":
            for auth_url in order.authorizations:
                auth_data, _ = await self._acme_request(auth_url, payload=None)
                assert isinstance(auth_data, dict)
                auth_domain = auth_data["identifier"]["value"]

                # Find the requested challenge type
                target_challenge = None
                for ch in auth_data.get("challenges", []):
                    if ch["type"] == challenge_type:
                        target_challenge = ch
                        break

                if target_challenge is None:
                    raise ACMEError(
                        f"Challenge type {challenge_type} not available for {auth_domain}"
                    )

                key_auth = f"{target_challenge['token']}.{self._thumbprint()}"
                acme_challenge = ACMEChallenge(
                    type=challenge_type,
                    url=target_challenge["url"],
                    token=target_challenge["token"],
                    key_authorization=key_auth,
                    domain=auth_domain,
                )
                order.challenges.append(acme_challenge)

                # Provision the challenge response
                if challenge_type == "http-01":
                    await self._handle_http01(acme_challenge)
                else:
                    await self._handle_dns01(acme_challenge)

                # Notify CA we are ready
                await self._respond_to_challenge(acme_challenge.url)

                # Poll until valid
                status = await self._poll_challenge(acme_challenge.url)
                acme_challenge.status = status
                logger.info(
                    "Challenge %s for %s: %s", challenge_type, auth_domain, status
                )

            # Cleanup challenges
            for ch in order.challenges:
                if ch.type == "http-01":
                    await self._cleanup_http01(ch)
                else:
                    await self._cleanup_dns01(ch)

        # 3. Poll order until ready
        order_data = await self._poll_order(order.order_url)

        # 4. Finalize with CSR
        csr_der, private_key = self._generate_csr(domain, san_list)
        finalize_payload = {"csr": _b64url(csr_der)}
        finalize_data, _ = await self._acme_request(
            order.finalize_url, finalize_payload
        )
        assert isinstance(finalize_data, dict)

        # 5. Poll until certificate is available
        if finalize_data.get("status") != "valid":
            finalize_data = await self._poll_order(order.order_url)

        cert_url = finalize_data.get("certificate")
        if not cert_url:
            raise ACMEError("No certificate URL in finalized order")

        # 6. Download certificate chain
        cert_data, _ = await self._acme_request(cert_url, payload=None)
        if isinstance(cert_data, dict):
            raise ACMEError("Expected PEM certificate, got JSON")
        cert_pem = cert_data if isinstance(cert_data, bytes) else cert_data.encode()

        # Split full chain into leaf + intermediates
        pem_blocks = cert_pem.split(b"-----END CERTIFICATE-----")
        leaf_pem = pem_blocks[0] + b"-----END CERTIFICATE-----\n" if pem_blocks else cert_pem
        chain_pem = b""
        for block in pem_blocks[1:]:
            stripped = block.strip()
            if stripped:
                chain_pem += stripped + b"\n-----END CERTIFICATE-----\n"

        key_pem = private_key.private_bytes(Encoding.PEM, PrivateFormat.PKCS8, NoEncryption())

        # Persist
        domain_safe = domain.replace("*", "_wildcard_")
        (self.data_dir / "certs" / f"{domain_safe}.pem").write_bytes(cert_pem)
        (self.data_dir / "keys" / f"{domain_safe}.key").write_bytes(key_pem)
        try:
            os.chmod(self.data_dir / "keys" / f"{domain_safe}.key", 0o600)
        except OSError:
            pass

        # Validate chain
        self._validate_chain(cert_pem)

        logger.info("Certificate for %s obtained and stored", domain)
        return cert_pem, key_pem, chain_pem

    async def renew_certificate(
        self,
        domain: str,
        challenge_type: Literal["http-01", "dns-01"] = "http-01",
    ) -> Tuple[bytes, bytes, bytes]:
        """Renew a certificate for *domain*.

        This simply requests a new certificate (ACME does not have a
        dedicated renewal endpoint).
        """
        logger.info("Renewing certificate for %s", domain)
        return await self.request_certificate(domain, challenge_type)

    async def revoke_certificate(self, domain: str) -> None:
        """Revoke the certificate stored for *domain*."""
        await self._ensure_initialized()

        domain_safe = domain.replace("*", "_wildcard_")
        cert_path = self.data_dir / "certs" / f"{domain_safe}.pem"
        if not cert_path.exists():
            raise ACMEError(f"No certificate found for {domain}")

        cert_pem = cert_path.read_bytes()
        cert = x509.load_pem_x509_certificate(cert_pem)
        cert_der = cert.public_bytes(Encoding.DER)

        revoke_url = self._directory.get("revokeCert")
        if not revoke_url:
            raise ACMEError("ACME directory does not have revokeCert endpoint")

        payload = {"certificate": _b64url(cert_der)}
        await self._acme_request(revoke_url, payload)
        logger.info("Certificate for %s revoked", domain)

        # Remove local files
        cert_path.unlink(missing_ok=True)
        key_path = self.data_dir / "keys" / f"{domain_safe}.key"
        key_path.unlink(missing_ok=True)

    # ------------------------------------------------------------------
    # Chain validation
    # ------------------------------------------------------------------
    @staticmethod
    def _validate_chain(cert_pem: bytes) -> bool:
        """Basic validation: parse all certs in the PEM bundle and check
        that the leaf is not yet expired."""
        certs: List[x509.Certificate] = []
        remaining = cert_pem
        while b"-----BEGIN CERTIFICATE-----" in remaining:
            cert = x509.load_pem_x509_certificate(remaining)
            certs.append(cert)
            end_marker = b"-----END CERTIFICATE-----"
            idx = remaining.find(end_marker)
            if idx == -1:
                break
            remaining = remaining[idx + len(end_marker):]

        if not certs:
            raise ACMEError("No certificates found in PEM bundle")

        leaf = certs[0]
        now = datetime.now(timezone.utc)
        if leaf.not_valid_after_utc < now:
            raise ACMEError(
                f"Leaf certificate expired at {leaf.not_valid_after_utc}"
            )
        if leaf.not_valid_before_utc > now:
            raise ACMEError(
                f"Leaf certificate not yet valid (starts {leaf.not_valid_before_utc})"
            )

        logger.debug(
            "Certificate chain validated: %d cert(s), leaf expires %s",
            len(certs),
            leaf.not_valid_after_utc.isoformat(),
        )
        return True

    # ------------------------------------------------------------------
    # Renewal scheduling
    # ------------------------------------------------------------------
    async def start_renewal_loop(
        self,
        domains: List[str],
        challenge_type: Literal["http-01", "dns-01"] = "http-01",
        check_interval_hours: int = 12,
    ) -> None:
        """Start a background task that checks for renewals periodically."""
        if self._renewal_task and not self._renewal_task.done():
            logger.warning("Renewal loop already running")
            return

        self._renewal_task = asyncio.create_task(
            self._renewal_loop(domains, challenge_type, check_interval_hours)
        )
        logger.info(
            "Renewal loop started for %s (check every %dh)",
            domains,
            check_interval_hours,
        )

    async def _renewal_loop(
        self,
        domains: List[str],
        challenge_type: str,
        check_interval_hours: int,
    ) -> None:
        while True:
            try:
                for domain in domains:
                    if self._needs_renewal(domain):
                        logger.info("Certificate for %s needs renewal", domain)
                        try:
                            await self.renew_certificate(domain, challenge_type)  # type: ignore[arg-type]
                        except ACMEError as exc:
                            logger.error(
                                "Failed to renew %s: %s", domain, exc
                            )
                    else:
                        logger.debug("Certificate for %s is still valid", domain)
            except Exception:
                logger.exception("Unexpected error in renewal loop")

            await asyncio.sleep(check_interval_hours * 3600)

    def _needs_renewal(self, domain: str) -> bool:
        """Return True if the cert for *domain* expires within the threshold."""
        domain_safe = domain.replace("*", "_wildcard_")
        cert_path = self.data_dir / "certs" / f"{domain_safe}.pem"
        if not cert_path.exists():
            return True

        try:
            cert_pem = cert_path.read_bytes()
            cert = x509.load_pem_x509_certificate(cert_pem)
            expiry = cert.not_valid_after_utc
            remaining = expiry - datetime.now(timezone.utc)
            return remaining < timedelta(days=RENEWAL_THRESHOLD_DAYS)
        except Exception:
            logger.exception("Error checking renewal for %s", domain)
            return True

    def get_certificate_expiry(self, domain: str) -> Optional[datetime]:
        """Return the expiry datetime of the stored certificate, or None."""
        domain_safe = domain.replace("*", "_wildcard_")
        cert_path = self.data_dir / "certs" / f"{domain_safe}.pem"
        if not cert_path.exists():
            return None
        try:
            cert_pem = cert_path.read_bytes()
            cert = x509.load_pem_x509_certificate(cert_pem)
            return cert.not_valid_after_utc
        except Exception:
            return None


class ACMEError(Exception):
    """Raised when an ACME operation fails."""

    def __init__(
        self,
        message: str,
        status: int = 0,
        acme_type: str = "",
    ) -> None:
        super().__init__(message)
        self.status = status
        self.acme_type = acme_type
