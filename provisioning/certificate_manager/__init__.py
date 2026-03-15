"""Certificate management facade for the HA-VoIP provisioning system.

Exposes the ``CertificateManager`` class that unifies three modes of
operation:

* **acme** -- Fully automatic certificates via Let's Encrypt (or any
  ACME-compatible CA).
* **manual** -- User-uploaded certificate and key files.
* **self_signed** -- Locally generated CA and server certificates for
  LAN-only deployments.

Consumers only interact with ``CertificateManager``; the underlying
``ACMEClient``, ``CertStore``, and ``LocalCA`` classes are implementation
details.
"""

from __future__ import annotations

import asyncio
import enum
import logging
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Coroutine, Dict, List, Optional, Tuple

from .acme_client import ACMEClient, ACMEError
from .cert_store import CertBundle, CertInfo, CertStore
from .local_ca import LocalCA

__all__ = [
    "CertificateManager",
    "CertMode",
    "CertStatus",
    "CertBundle",
    "CertInfo",
    "ACMEClient",
    "ACMEError",
    "CertStore",
    "LocalCA",
]

logger = logging.getLogger(__name__)


class CertMode(str, enum.Enum):
    """Supported certificate provisioning modes."""

    ACME = "acme"
    MANUAL = "manual"
    SELF_SIGNED = "self_signed"


@dataclass
class CertStatus:
    """Snapshot of the current certificate state."""

    mode: CertMode
    domain: Optional[str] = None
    has_certificate: bool = False
    fingerprint_sha256: Optional[str] = None
    issuer: Optional[str] = None
    not_before: Optional[str] = None
    not_after: Optional[str] = None
    days_until_expiry: Optional[int] = None
    is_expired: bool = False
    renewal_pending: bool = False
    last_error: Optional[str] = None


class CertificateManager:
    """Unified certificate lifecycle manager.

    Parameters
    ----------
    data_dir:
        Root directory for all certificate material.
    mode:
        One of ``CertMode.ACME``, ``CertMode.MANUAL``, or
        ``CertMode.SELF_SIGNED``.
    domain:
        Primary domain for ACME / self-signed certificates.
    email:
        Contact email for ACME account registration.
    staging:
        Use Let's Encrypt staging environment.
    acme_challenge_type:
        ``"http-01"`` or ``"dns-01"``.
    challenge_dir:
        Directory served at ``/.well-known/acme-challenge/``.
    dns_hook / dns_cleanup_hook:
        Async callables for DNS-01 provisioning.
    san_list:
        Additional Subject Alternative Names.
    encryption_key:
        Fernet key for encrypting stored private keys.
    on_cert_changed:
        Optional async callback invoked after a certificate is changed,
        useful for reloading TLS listeners.
    """

    def __init__(
        self,
        data_dir: str | Path,
        mode: CertMode | str = CertMode.SELF_SIGNED,
        domain: Optional[str] = None,
        email: Optional[str] = None,
        staging: bool = False,
        acme_challenge_type: str = "http-01",
        challenge_dir: Optional[str | Path] = None,
        dns_hook: Any = None,
        dns_cleanup_hook: Any = None,
        san_list: Optional[List[str]] = None,
        encryption_key: Optional[str] = None,
        on_cert_changed: Optional[Callable[[], Coroutine[Any, Any, None]]] = None,
    ) -> None:
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        self.mode = CertMode(mode) if isinstance(mode, str) else mode
        self.domain = domain
        self.email = email
        self.staging = staging
        self.acme_challenge_type = acme_challenge_type
        self.san_list = san_list or []
        self._on_cert_changed = on_cert_changed
        self._last_error: Optional[str] = None

        # Sub-components
        self.store = CertStore(
            self.data_dir / "store",
            encryption_key=encryption_key,
        )
        self.local_ca = LocalCA(self.data_dir / "local_ca")
        self._acme_client: Optional[ACMEClient] = None

        if self.mode == CertMode.ACME:
            if not email:
                raise ValueError("ACME mode requires an email address")
            if not domain:
                raise ValueError("ACME mode requires a domain")
            self._acme_client = ACMEClient(
                data_dir=self.data_dir / "acme",
                email=email,
                staging=staging,
                challenge_dir=challenge_dir,
                dns_hook=dns_hook,
                dns_cleanup_hook=dns_cleanup_hook,
            )

        self._renewal_task: Optional[asyncio.Task[None]] = None

    # ------------------------------------------------------------------
    # Full automatic provisioning
    # ------------------------------------------------------------------
    async def auto_provision(self) -> CertBundle:
        """Automatically provision a certificate based on the configured mode.

        * **ACME** -- Requests a certificate from Let's Encrypt.
        * **MANUAL** -- Raises if no certificate has been uploaded.
        * **SELF_SIGNED** -- Generates a local CA (if needed) and issues
          a server certificate.

        Returns the resulting ``CertBundle``.
        """
        logger.info("Auto-provisioning certificate (mode=%s)", self.mode.value)

        if self.mode == CertMode.ACME:
            return await self._provision_acme()
        elif self.mode == CertMode.MANUAL:
            return self._provision_manual()
        else:
            return self._provision_self_signed()

    async def _provision_acme(self) -> CertBundle:
        assert self._acme_client is not None
        assert self.domain is not None

        try:
            cert_pem, key_pem, chain_pem = await self._acme_client.request_certificate(
                self.domain,
                challenge_type=self.acme_challenge_type,  # type: ignore[arg-type]
                san_list=self.san_list or None,
            )
            bundle = self.store.store_certificate(
                self.domain, cert_pem, key_pem, chain_pem
            )
            self._last_error = None
            await self._notify_changed()
            return bundle
        except ACMEError as exc:
            self._last_error = str(exc)
            logger.error("ACME provisioning failed: %s", exc)
            raise

    def _provision_manual(self) -> CertBundle:
        if not self.domain:
            raise ValueError("Domain must be set for manual mode")
        bundle = self.store.get_certificate(self.domain)
        if bundle is None:
            raise FileNotFoundError(
                f"No manually uploaded certificate found for {self.domain}. "
                "Use upload_manual() first."
            )
        return bundle

    def _provision_self_signed(self) -> CertBundle:
        domain = self.domain or "ha-voip.local"

        # Ensure CA exists
        if not self.local_ca.has_ca():
            logger.info("Generating local CA for self-signed mode")
            ca_cert_pem, ca_key_pem = self.local_ca.generate_ca()
            self.store.store_ca_certificate(ca_cert_pem, ca_key_pem)

        # Build SAN list
        sans = [domain]
        for s in self.san_list:
            if s not in sans:
                sans.append(s)
        # Always include localhost and common LAN addresses
        for extra in ["localhost", "127.0.0.1", "::1"]:
            if extra not in sans:
                sans.append(extra)

        cert_pem, key_pem = self.local_ca.issue_certificate(
            domain=domain,
            san_list=sans,
        )

        ca_cert_pem = self.local_ca.get_ca_certificate()
        bundle = self.store.store_certificate(
            domain, cert_pem, key_pem, ca_cert_pem
        )
        self._last_error = None
        logger.info("Self-signed certificate provisioned for %s", domain)
        return bundle

    # ------------------------------------------------------------------
    # Manual upload
    # ------------------------------------------------------------------
    def upload_manual(
        self,
        cert_path: str | Path,
        key_path: str | Path,
        chain_path: Optional[str | Path] = None,
        domain: Optional[str] = None,
    ) -> CertBundle:
        """Upload manually obtained certificate and key files.

        Parameters
        ----------
        cert_path:
            Path to PEM certificate file.
        key_path:
            Path to PEM private key file.
        chain_path:
            Optional path to PEM certificate chain.
        domain:
            Override domain name (otherwise extracted from cert CN).
        """
        from cryptography import x509 as _x509

        cert_pem = Path(cert_path).read_bytes()
        key_pem = Path(key_path).read_bytes()
        chain_pem = Path(chain_path).read_bytes() if chain_path else b""

        if not domain:
            cert = _x509.load_pem_x509_certificate(cert_pem)
            cn_attrs = cert.subject.get_attributes_for_oid(_x509.oid.NameOID.COMMON_NAME)
            domain = cn_attrs[0].value if cn_attrs else "unknown"

        bundle = self.store.store_certificate(domain, cert_pem, key_pem, chain_pem)
        self.domain = domain
        self._last_error = None
        logger.info("Manual certificate uploaded for %s", domain)
        return bundle

    # ------------------------------------------------------------------
    # Generate local (self-signed)
    # ------------------------------------------------------------------
    def generate_local(
        self,
        domain: Optional[str] = None,
        san_list: Optional[List[str]] = None,
    ) -> CertBundle:
        """Generate a self-signed certificate for LAN use.

        This is a convenience wrapper around ``_provision_self_signed()``.
        """
        old_domain = self.domain
        old_sans = self.san_list
        try:
            if domain:
                self.domain = domain
            if san_list:
                self.san_list = san_list
            return self._provision_self_signed()
        finally:
            self.domain = old_domain
            self.san_list = old_sans

    # ------------------------------------------------------------------
    # Active certificate retrieval
    # ------------------------------------------------------------------
    def get_active_cert(self) -> Optional[CertBundle]:
        """Return the currently active certificate bundle, or ``None``."""
        domain = self.domain or "ha-voip.local"
        return self.store.get_certificate(domain)

    # ------------------------------------------------------------------
    # Certificate status
    # ------------------------------------------------------------------
    def get_cert_status(self) -> CertStatus:
        """Return a snapshot of the current certificate state."""
        domain = self.domain or "ha-voip.local"
        bundle = self.store.get_certificate(domain)

        status = CertStatus(mode=self.mode, domain=domain, last_error=self._last_error)

        if bundle and bundle.not_after:
            now = datetime.now(timezone.utc)
            days_left = (bundle.not_after - now).days
            status.has_certificate = True
            status.fingerprint_sha256 = bundle.fingerprint_sha256
            status.not_before = bundle.not_before.isoformat() if bundle.not_before else None
            status.not_after = bundle.not_after.isoformat()
            status.days_until_expiry = max(days_left, 0)
            status.is_expired = bundle.not_after < now
            status.renewal_pending = days_left < 30

            # Extract issuer
            try:
                from cryptography import x509 as _x509

                cert = _x509.load_pem_x509_certificate(bundle.certificate_pem)
                status.issuer = cert.issuer.rfc4514_string()
            except Exception:
                pass

        return status

    # ------------------------------------------------------------------
    # Renewal loop
    # ------------------------------------------------------------------
    async def start_renewal_loop(
        self,
        check_interval_hours: int = 12,
    ) -> None:
        """Start a background renewal loop.

        For ACME mode, delegates to the ACME client's renewal logic.
        For self-signed mode, reissues certificates when they expire.
        Manual mode only monitors and logs warnings.
        """
        if self._renewal_task and not self._renewal_task.done():
            logger.warning("Renewal loop already running")
            return

        self._renewal_task = asyncio.create_task(
            self._renewal_loop(check_interval_hours)
        )
        logger.info(
            "Certificate renewal loop started (mode=%s, interval=%dh)",
            self.mode.value,
            check_interval_hours,
        )

    async def stop_renewal_loop(self) -> None:
        """Stop the background renewal loop."""
        if self._renewal_task and not self._renewal_task.done():
            self._renewal_task.cancel()
            try:
                await self._renewal_task
            except asyncio.CancelledError:
                pass
            self._renewal_task = None
            logger.info("Certificate renewal loop stopped")

    async def _renewal_loop(self, check_interval_hours: int) -> None:
        while True:
            try:
                domains_needing_renewal = self.store.check_renewal_needed()
                if domains_needing_renewal:
                    logger.info(
                        "Certificates needing renewal: %s", domains_needing_renewal
                    )
                    for domain in domains_needing_renewal:
                        await self._perform_renewal(domain)
                else:
                    logger.debug("All certificates are current")
            except asyncio.CancelledError:
                raise
            except Exception:
                logger.exception("Error in renewal loop")

            await asyncio.sleep(check_interval_hours * 3600)

    async def _perform_renewal(self, domain: str) -> None:
        """Renew a single domain certificate."""
        try:
            if self.mode == CertMode.ACME and self._acme_client:
                cert_pem, key_pem, chain_pem = await self._acme_client.renew_certificate(
                    domain, self.acme_challenge_type  # type: ignore[arg-type]
                )
                self.store.store_certificate(domain, cert_pem, key_pem, chain_pem)
                logger.info("ACME renewal completed for %s", domain)
            elif self.mode == CertMode.SELF_SIGNED:
                sans = [domain] + self.san_list
                cert_pem, key_pem = self.local_ca.issue_certificate(
                    domain=domain,
                    san_list=sans,
                )
                ca_cert_pem = self.local_ca.get_ca_certificate()
                self.store.store_certificate(domain, cert_pem, key_pem, ca_cert_pem)
                logger.info("Self-signed renewal completed for %s", domain)
            elif self.mode == CertMode.MANUAL:
                logger.warning(
                    "Certificate for %s needs renewal but mode is manual. "
                    "Please upload a new certificate.",
                    domain,
                )
                return

            self._last_error = None
            await self._notify_changed()
        except Exception as exc:
            self._last_error = str(exc)
            logger.error("Renewal failed for %s: %s", domain, exc)

    # ------------------------------------------------------------------
    # Service reload
    # ------------------------------------------------------------------
    async def reload_services(self) -> None:
        """Invoke the on_cert_changed callback to reload TLS services
        (e.g., WSS, SIP-TLS) without downtime.
        """
        await self._notify_changed()

    async def _notify_changed(self) -> None:
        if self._on_cert_changed:
            try:
                await self._on_cert_changed()
                logger.info("TLS services notified of certificate change")
            except Exception:
                logger.exception("Error notifying services of certificate change")

    # ------------------------------------------------------------------
    # Cleanup
    # ------------------------------------------------------------------
    async def close(self) -> None:
        """Shut down the renewal loop and release resources."""
        await self.stop_renewal_loop()
        if self._acme_client:
            await self._acme_client.close()
