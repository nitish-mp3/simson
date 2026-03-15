"""Secure certificate storage with format conversion and lifecycle tracking.

Provides the ``CertStore`` class for persisting TLS certificates and
private keys, querying expiration dates, detecting certificates that need
renewal, and exporting/importing in PEM or DER format.  Optionally
encrypts private keys at rest using Fernet (AES-128-CBC via the
``cryptography`` library).
"""

from __future__ import annotations

import hashlib
import json
import logging
import os
import shutil
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Tuple

from cryptography import x509
from cryptography.fernet import Fernet
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.serialization import (
    BestAvailableEncryption,
    Encoding,
    NoEncryption,
    PrivateFormat,
    PublicFormat,
)

logger = logging.getLogger(__name__)

# How many days before expiry a certificate is considered renewal-worthy.
DEFAULT_RENEWAL_THRESHOLD_DAYS = 30


@dataclass
class CertBundle:
    """An in-memory bundle of certificate material."""

    domain: str
    certificate_pem: bytes
    private_key_pem: bytes
    chain_pem: bytes
    fingerprint_sha256: str = ""
    not_before: Optional[datetime] = None
    not_after: Optional[datetime] = None

    def __post_init__(self) -> None:
        if not self.fingerprint_sha256 and self.certificate_pem:
            try:
                cert = x509.load_pem_x509_certificate(self.certificate_pem)
                self.fingerprint_sha256 = cert.fingerprint(hashes.SHA256()).hex()
                self.not_before = cert.not_valid_before_utc
                self.not_after = cert.not_valid_after_utc
            except Exception:
                pass


@dataclass
class CertInfo:
    """Lightweight metadata about a stored certificate (no key material)."""

    domain: str
    fingerprint_sha256: str
    issuer: str
    subject: str
    not_before: str
    not_after: str
    serial_number: str
    san_list: List[str] = field(default_factory=list)
    is_expired: bool = False
    days_until_expiry: int = 0


class CertStore:
    """Persistent certificate and key store.

    Parameters
    ----------
    base_dir:
        Root directory for stored certificates.
    encryption_key:
        Optional Fernet key (base64-encoded 32-byte key) used to encrypt
        private keys at rest.  If ``None``, keys are stored as plain PEM
        (file permissions are still restricted to owner-only).
    renewal_threshold_days:
        Number of days before expiry at which a certificate is flagged
        for renewal.
    """

    CERT_FILENAME = "cert.pem"
    KEY_FILENAME = "privkey.pem"
    CHAIN_FILENAME = "chain.pem"
    FULLCHAIN_FILENAME = "fullchain.pem"
    META_FILENAME = "meta.json"

    def __init__(
        self,
        base_dir: str | Path,
        encryption_key: Optional[str] = None,
        renewal_threshold_days: int = DEFAULT_RENEWAL_THRESHOLD_DAYS,
    ) -> None:
        self.base_dir = Path(base_dir)
        self.base_dir.mkdir(parents=True, exist_ok=True)
        self.renewal_threshold_days = renewal_threshold_days

        self._fernet: Optional[Fernet] = None
        if encryption_key:
            self._fernet = Fernet(encryption_key.encode() if isinstance(encryption_key, str) else encryption_key)

    # ------------------------------------------------------------------
    # Internal helpers
    # ------------------------------------------------------------------
    def _domain_dir(self, domain: str) -> Path:
        safe = domain.replace("*", "_wildcard_").replace(":", "_")
        return self.base_dir / safe

    def _write_file(self, path: Path, data: bytes, restrict: bool = False) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        if restrict:
            try:
                os.chmod(path, 0o600)
            except OSError:
                pass

    @staticmethod
    def _parse_cert(pem: bytes) -> x509.Certificate:
        return x509.load_pem_x509_certificate(pem)

    def _encrypt_key(self, key_pem: bytes) -> bytes:
        if self._fernet:
            return self._fernet.encrypt(key_pem)
        return key_pem

    def _decrypt_key(self, data: bytes) -> bytes:
        if self._fernet:
            return self._fernet.decrypt(data)
        return data

    def _build_meta(self, cert: x509.Certificate, domain: str) -> Dict:
        now = datetime.now(timezone.utc)
        expiry = cert.not_valid_after_utc
        days_left = (expiry - now).days

        san_list: List[str] = []
        try:
            san_ext = cert.extensions.get_extension_for_oid(
                x509.oid.ExtensionOID.SUBJECT_ALTERNATIVE_NAME
            )
            san_list = san_ext.value.get_values_for_type(x509.DNSName)
        except x509.ExtensionNotFound:
            pass

        return {
            "domain": domain,
            "fingerprint_sha256": cert.fingerprint(hashes.SHA256()).hex(),
            "issuer": cert.issuer.rfc4514_string(),
            "subject": cert.subject.rfc4514_string(),
            "serial_number": format(cert.serial_number, "x"),
            "not_before": cert.not_valid_before_utc.isoformat(),
            "not_after": expiry.isoformat(),
            "san_list": san_list,
            "is_expired": expiry < now,
            "days_until_expiry": max(days_left, 0),
            "stored_at": now.isoformat(),
        }

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------
    def store_certificate(
        self,
        domain: str,
        cert: bytes,
        key: bytes,
        chain: bytes = b"",
    ) -> CertBundle:
        """Persist a certificate, key, and optional chain for *domain*.

        Parameters
        ----------
        domain:
            Domain name used as the storage key.
        cert:
            PEM-encoded leaf certificate.
        key:
            PEM-encoded private key.
        chain:
            PEM-encoded intermediate/root chain.

        Returns
        -------
        CertBundle with populated metadata fields.
        """
        ddir = self._domain_dir(domain)
        ddir.mkdir(parents=True, exist_ok=True)

        # Parse certificate for metadata
        parsed = self._parse_cert(cert)
        meta = self._build_meta(parsed, domain)

        # Store cert
        self._write_file(ddir / self.CERT_FILENAME, cert)

        # Store private key (optionally encrypted)
        encrypted_key = self._encrypt_key(key)
        self._write_file(ddir / self.KEY_FILENAME, encrypted_key, restrict=True)

        # Store chain
        self._write_file(ddir / self.CHAIN_FILENAME, chain if chain else b"")

        # Store full chain (cert + intermediates)
        fullchain = cert
        if chain:
            fullchain = cert.rstrip() + b"\n" + chain
        self._write_file(ddir / self.FULLCHAIN_FILENAME, fullchain)

        # Store metadata
        self._write_file(ddir / self.META_FILENAME, json.dumps(meta, indent=2).encode())

        logger.info(
            "Stored certificate for %s (expires %s, fingerprint %s)",
            domain,
            meta["not_after"],
            meta["fingerprint_sha256"][:16],
        )

        return CertBundle(
            domain=domain,
            certificate_pem=cert,
            private_key_pem=key,
            chain_pem=chain,
            fingerprint_sha256=meta["fingerprint_sha256"],
            not_before=parsed.not_valid_before_utc,
            not_after=parsed.not_valid_after_utc,
        )

    def get_certificate(self, domain: str) -> Optional[CertBundle]:
        """Retrieve the stored certificate bundle for *domain*.

        Returns ``None`` if no certificate is stored.
        """
        ddir = self._domain_dir(domain)
        cert_path = ddir / self.CERT_FILENAME
        key_path = ddir / self.KEY_FILENAME
        chain_path = ddir / self.CHAIN_FILENAME

        if not cert_path.exists() or not key_path.exists():
            return None

        cert_pem = cert_path.read_bytes()
        key_encrypted = key_path.read_bytes()
        key_pem = self._decrypt_key(key_encrypted)
        chain_pem = chain_path.read_bytes() if chain_path.exists() else b""

        return CertBundle(
            domain=domain,
            certificate_pem=cert_pem,
            private_key_pem=key_pem,
            chain_pem=chain_pem,
        )

    def list_certificates(self) -> List[CertInfo]:
        """List metadata for all stored certificates."""
        results: List[CertInfo] = []
        if not self.base_dir.exists():
            return results

        for entry in sorted(self.base_dir.iterdir()):
            meta_path = entry / self.META_FILENAME
            if not meta_path.exists():
                continue
            try:
                meta = json.loads(meta_path.read_text())
                # Recompute dynamic fields
                not_after = datetime.fromisoformat(meta["not_after"])
                if not_after.tzinfo is None:
                    not_after = not_after.replace(tzinfo=timezone.utc)
                now = datetime.now(timezone.utc)
                days_left = (not_after - now).days

                info = CertInfo(
                    domain=meta["domain"],
                    fingerprint_sha256=meta["fingerprint_sha256"],
                    issuer=meta.get("issuer", ""),
                    subject=meta.get("subject", ""),
                    not_before=meta["not_before"],
                    not_after=meta["not_after"],
                    serial_number=meta.get("serial_number", ""),
                    san_list=meta.get("san_list", []),
                    is_expired=not_after < now,
                    days_until_expiry=max(days_left, 0),
                )
                results.append(info)
            except Exception:
                logger.warning("Failed to read metadata from %s", meta_path)
        return results

    def delete_certificate(self, domain: str) -> bool:
        """Delete all stored material for *domain*.

        Returns ``True`` if something was deleted.
        """
        ddir = self._domain_dir(domain)
        if not ddir.exists():
            return False
        shutil.rmtree(ddir)
        logger.info("Deleted certificate for %s", domain)
        return True

    def get_expiry(self, domain: str) -> Optional[datetime]:
        """Return the expiry time for the stored certificate, or ``None``."""
        ddir = self._domain_dir(domain)
        cert_path = ddir / self.CERT_FILENAME
        if not cert_path.exists():
            return None
        try:
            cert = self._parse_cert(cert_path.read_bytes())
            return cert.not_valid_after_utc
        except Exception:
            return None

    def check_renewal_needed(self) -> List[str]:
        """Return a list of domains whose certificates need renewal."""
        needs_renewal: List[str] = []
        now = datetime.now(timezone.utc)
        for info in self.list_certificates():
            not_after = datetime.fromisoformat(info.not_after)
            if not_after.tzinfo is None:
                not_after = not_after.replace(tzinfo=timezone.utc)
            remaining = not_after - now
            if remaining.days < self.renewal_threshold_days:
                needs_renewal.append(info.domain)
        return needs_renewal

    # ------------------------------------------------------------------
    # Format conversion
    # ------------------------------------------------------------------
    def get_certificate_der(self, domain: str) -> Optional[bytes]:
        """Return the leaf certificate in DER format."""
        bundle = self.get_certificate(domain)
        if bundle is None:
            return None
        cert = x509.load_pem_x509_certificate(bundle.certificate_pem)
        return cert.public_bytes(Encoding.DER)

    def get_private_key_der(self, domain: str) -> Optional[bytes]:
        """Return the private key in DER format."""
        bundle = self.get_certificate(domain)
        if bundle is None:
            return None
        key = serialization.load_pem_private_key(bundle.private_key_pem, password=None)
        return key.private_bytes(Encoding.DER, PrivateFormat.PKCS8, NoEncryption())

    def export_pkcs12(
        self,
        domain: str,
        passphrase: Optional[bytes] = None,
    ) -> Optional[bytes]:
        """Export certificate + key as PKCS#12 (.pfx) archive."""
        bundle = self.get_certificate(domain)
        if bundle is None:
            return None

        from cryptography.hazmat.primitives.serialization.pkcs12 import (
            serialize_key_and_certificates,
        )

        cert = x509.load_pem_x509_certificate(bundle.certificate_pem)
        key = serialization.load_pem_private_key(bundle.private_key_pem, password=None)

        # Parse chain certs
        ca_certs: List[x509.Certificate] = []
        if bundle.chain_pem:
            remaining = bundle.chain_pem
            while b"-----BEGIN CERTIFICATE-----" in remaining:
                try:
                    ca_cert = x509.load_pem_x509_certificate(remaining)
                    ca_certs.append(ca_cert)
                except Exception:
                    break
                idx = remaining.find(b"-----END CERTIFICATE-----")
                if idx == -1:
                    break
                remaining = remaining[idx + len(b"-----END CERTIFICATE-----"):]

        enc = BestAvailableEncryption(passphrase) if passphrase else NoEncryption()
        return serialize_key_and_certificates(
            name=domain.encode(),
            key=key,
            cert=cert,
            cas=ca_certs or None,
            encryption_algorithm=enc,
        )

    # ------------------------------------------------------------------
    # CA certificate management (for self-signed mode)
    # ------------------------------------------------------------------
    def store_ca_certificate(self, ca_cert_pem: bytes, ca_key_pem: bytes) -> None:
        """Store a local CA certificate and key."""
        ca_dir = self.base_dir / "_ca"
        ca_dir.mkdir(parents=True, exist_ok=True)
        self._write_file(ca_dir / "ca.pem", ca_cert_pem)
        encrypted_key = self._encrypt_key(ca_key_pem)
        self._write_file(ca_dir / "ca.key", encrypted_key, restrict=True)
        logger.info("Stored local CA certificate")

    def get_ca_certificate(self) -> Optional[Tuple[bytes, bytes]]:
        """Return ``(ca_cert_pem, ca_key_pem)`` or ``None``."""
        ca_dir = self.base_dir / "_ca"
        cert_path = ca_dir / "ca.pem"
        key_path = ca_dir / "ca.key"
        if not cert_path.exists() or not key_path.exists():
            return None
        ca_cert = cert_path.read_bytes()
        ca_key = self._decrypt_key(key_path.read_bytes())
        return ca_cert, ca_key

    # ------------------------------------------------------------------
    # Backup / export
    # ------------------------------------------------------------------
    def export_all(self, output_dir: str | Path) -> Path:
        """Export all certificates and keys to *output_dir*.

        Returns the output directory path.
        """
        output = Path(output_dir)
        output.mkdir(parents=True, exist_ok=True)
        for info in self.list_certificates():
            bundle = self.get_certificate(info.domain)
            if bundle is None:
                continue
            domain_dir = output / info.domain.replace("*", "_wildcard_")
            domain_dir.mkdir(parents=True, exist_ok=True)
            (domain_dir / "cert.pem").write_bytes(bundle.certificate_pem)
            (domain_dir / "key.pem").write_bytes(bundle.private_key_pem)
            if bundle.chain_pem:
                (domain_dir / "chain.pem").write_bytes(bundle.chain_pem)
        logger.info("Exported %d certificate(s) to %s", len(self.list_certificates()), output)
        return output

    def import_certificate(
        self,
        domain: str,
        cert_path: str | Path,
        key_path: str | Path,
        chain_path: Optional[str | Path] = None,
    ) -> CertBundle:
        """Import a certificate from file paths."""
        cert_pem = Path(cert_path).read_bytes()
        key_pem = Path(key_path).read_bytes()
        chain_pem = Path(chain_path).read_bytes() if chain_path else b""
        return self.store_certificate(domain, cert_pem, key_pem, chain_pem)
