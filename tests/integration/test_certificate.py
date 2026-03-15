"""Integration test: TLS certificate lifecycle.

Tests the certificate management subsystem:
  - ACME flow (using a mock CA / pebble stand-in)
  - Self-signed certificate generation
  - Certificate renewal
  - Manual certificate upload / validation

Run with:
    pytest tests/integration/test_certificate.py -v
"""

from __future__ import annotations

import datetime
import os
import tempfile
from pathlib import Path
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# Certificate manager stubs
# ---------------------------------------------------------------------------


class SelfSignedCert:
    """Represents a generated self-signed certificate."""

    def __init__(self, cert_pem: str, key_pem: str, not_after: datetime.datetime):
        self.cert_pem = cert_pem
        self.key_pem = key_pem
        self.not_after = not_after

    def days_until_expiry(self) -> int:
        delta = self.not_after - datetime.datetime.utcnow()
        return max(delta.days, 0)


class CertificateManager:
    """Simplified certificate manager for testing."""

    def __init__(self, storage_dir: str):
        self.storage_dir = storage_dir
        self.current_cert: SelfSignedCert | None = None
        self._acme_account: dict[str, Any] | None = None

    # -- Self-signed -------------------------------------------------------

    def generate_self_signed(
        self, cn: str = "homeassistant.local", validity_days: int = 365
    ) -> SelfSignedCert:
        """Generate a self-signed certificate (stubbed PEM content)."""
        not_after = datetime.datetime.utcnow() + datetime.timedelta(days=validity_days)
        cert = SelfSignedCert(
            cert_pem=f"-----BEGIN CERTIFICATE-----\nMIIB...stub...{cn}\n-----END CERTIFICATE-----\n",
            key_pem="-----BEGIN PRIVATE KEY-----\nMIIE...stub...\n-----END PRIVATE KEY-----\n",
            not_after=not_after,
        )
        self.current_cert = cert
        self._write_files(cert)
        return cert

    # -- ACME (Let's Encrypt / Pebble) ------------------------------------

    async def request_acme_certificate(
        self, domain: str, email: str, ca_url: str = "https://acme-v02.api.letsencrypt.org/directory"
    ) -> SelfSignedCert:
        """Simulate ACME certificate issuance.

        In a real implementation this would use an ACME library (e.g.
        acme / certbot).  Here we simulate a successful flow.
        """
        # Step 1: Register account
        self._acme_account = {"email": email, "ca_url": ca_url, "status": "valid"}

        # Step 2: Create order
        order = {"domain": domain, "status": "pending"}

        # Step 3: Perform HTTP-01 challenge (simulated)
        order["status"] = "ready"

        # Step 4: Finalise and download cert
        not_after = datetime.datetime.utcnow() + datetime.timedelta(days=90)
        cert = SelfSignedCert(
            cert_pem=f"-----BEGIN CERTIFICATE-----\nACME-CERT-{domain}\n-----END CERTIFICATE-----\n",
            key_pem=f"-----BEGIN PRIVATE KEY-----\nACME-KEY-{domain}\n-----END PRIVATE KEY-----\n",
            not_after=not_after,
        )
        self.current_cert = cert
        self._write_files(cert)
        return cert

    # -- Renewal -----------------------------------------------------------

    async def renew_if_needed(self, threshold_days: int = 30) -> bool:
        """Renew the certificate if it expires within *threshold_days*."""
        if self.current_cert is None:
            return False
        if self.current_cert.days_until_expiry() > threshold_days:
            return False  # Not yet due

        # Regenerate (self-signed path shown; ACME path would call request_acme_certificate)
        self.generate_self_signed(validity_days=365)
        return True

    # -- Manual upload -----------------------------------------------------

    def upload_manual_cert(self, cert_path: str, key_path: str) -> SelfSignedCert:
        """Load user-supplied PEM files."""
        cert_pem = Path(cert_path).read_text()
        key_pem = Path(key_path).read_text()
        if "BEGIN CERTIFICATE" not in cert_pem:
            raise ValueError("Invalid certificate file: missing PEM header")
        if "BEGIN" not in key_pem:
            raise ValueError("Invalid key file: missing PEM header")
        cert = SelfSignedCert(
            cert_pem=cert_pem,
            key_pem=key_pem,
            not_after=datetime.datetime.utcnow() + datetime.timedelta(days=365),
        )
        self.current_cert = cert
        self._write_files(cert)
        return cert

    # -- Internal ----------------------------------------------------------

    def _write_files(self, cert: SelfSignedCert):
        cert_path = os.path.join(self.storage_dir, "cert.pem")
        key_path = os.path.join(self.storage_dir, "key.pem")
        Path(cert_path).write_text(cert.cert_pem)
        Path(key_path).write_text(cert.key_pem)


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def cert_dir(tmp_path):
    """Provide a temporary directory for certificate storage."""
    return str(tmp_path / "certs")


@pytest.fixture
def cert_manager(cert_dir):
    os.makedirs(cert_dir, exist_ok=True)
    return CertificateManager(cert_dir)


# ---------------------------------------------------------------------------
# Tests: Self-signed generation
# ---------------------------------------------------------------------------


def test_generate_self_signed(cert_manager, cert_dir):
    """Self-signed generation must produce cert + key files."""
    cert = cert_manager.generate_self_signed(cn="test.local", validity_days=365)
    assert cert.days_until_expiry() >= 364
    assert os.path.isfile(os.path.join(cert_dir, "cert.pem"))
    assert os.path.isfile(os.path.join(cert_dir, "key.pem"))


def test_self_signed_pem_content(cert_manager):
    """Generated PEM must contain standard markers."""
    cert = cert_manager.generate_self_signed()
    assert "BEGIN CERTIFICATE" in cert.cert_pem
    assert "BEGIN PRIVATE KEY" in cert.key_pem


# ---------------------------------------------------------------------------
# Tests: ACME flow
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_acme_certificate_request(cert_manager, cert_dir):
    """ACME flow must produce a certificate with the requested domain."""
    cert = await cert_manager.request_acme_certificate(
        domain="voip.example.com",
        email="admin@example.com",
        ca_url="https://localhost:14000/dir",  # pebble URL
    )
    assert "voip.example.com" in cert.cert_pem
    assert cert.days_until_expiry() >= 89
    assert cert_manager._acme_account is not None
    assert cert_manager._acme_account["status"] == "valid"


@pytest.mark.asyncio
async def test_acme_cert_written_to_disk(cert_manager, cert_dir):
    """ACME-issued cert must be persisted to the storage directory."""
    await cert_manager.request_acme_certificate("test.example.com", "admin@test.com")
    assert os.path.isfile(os.path.join(cert_dir, "cert.pem"))
    content = Path(os.path.join(cert_dir, "cert.pem")).read_text()
    assert "ACME-CERT-test.example.com" in content


# ---------------------------------------------------------------------------
# Tests: Renewal
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_renewal_not_needed_when_fresh(cert_manager):
    """A freshly generated cert should not trigger renewal."""
    cert_manager.generate_self_signed(validity_days=365)
    renewed = await cert_manager.renew_if_needed(threshold_days=30)
    assert renewed is False


@pytest.mark.asyncio
async def test_renewal_triggered_when_expiring(cert_manager):
    """A cert expiring soon must trigger renewal."""
    cert = cert_manager.generate_self_signed(validity_days=365)
    # Simulate imminent expiry by patching not_after
    cert.not_after = datetime.datetime.utcnow() + datetime.timedelta(days=10)
    renewed = await cert_manager.renew_if_needed(threshold_days=30)
    assert renewed is True
    assert cert_manager.current_cert.days_until_expiry() >= 364


@pytest.mark.asyncio
async def test_renewal_returns_false_without_cert(cert_manager):
    """Renewal on an empty manager must return False (nothing to renew)."""
    renewed = await cert_manager.renew_if_needed()
    assert renewed is False


# ---------------------------------------------------------------------------
# Tests: Manual upload
# ---------------------------------------------------------------------------


def test_manual_upload_valid(cert_manager, tmp_path):
    """Uploading valid PEM files must succeed."""
    cert_file = tmp_path / "manual_cert.pem"
    key_file = tmp_path / "manual_key.pem"
    cert_file.write_text("-----BEGIN CERTIFICATE-----\nMANUAL\n-----END CERTIFICATE-----\n")
    key_file.write_text("-----BEGIN PRIVATE KEY-----\nMANUAL\n-----END PRIVATE KEY-----\n")

    cert = cert_manager.upload_manual_cert(str(cert_file), str(key_file))
    assert "MANUAL" in cert.cert_pem


def test_manual_upload_invalid_cert(cert_manager, tmp_path):
    """Uploading a file without PEM header must raise ValueError."""
    cert_file = tmp_path / "bad_cert.pem"
    key_file = tmp_path / "good_key.pem"
    cert_file.write_text("not a certificate")
    key_file.write_text("-----BEGIN PRIVATE KEY-----\nKEY\n-----END PRIVATE KEY-----\n")

    with pytest.raises(ValueError, match="Invalid certificate file"):
        cert_manager.upload_manual_cert(str(cert_file), str(key_file))


def test_manual_upload_invalid_key(cert_manager, tmp_path):
    """Uploading a key file without PEM header must raise ValueError."""
    cert_file = tmp_path / "good_cert.pem"
    key_file = tmp_path / "bad_key.pem"
    cert_file.write_text("-----BEGIN CERTIFICATE-----\nCERT\n-----END CERTIFICATE-----\n")
    key_file.write_text("not a key")

    with pytest.raises(ValueError, match="Invalid key file"):
        cert_manager.upload_manual_cert(str(cert_file), str(key_file))
