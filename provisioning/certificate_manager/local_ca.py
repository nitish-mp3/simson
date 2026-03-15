"""Local Certificate Authority for LAN / self-signed certificate generation.

Provides ``LocalCA`` which can:

* Generate a root CA certificate and key pair.
* Issue end-entity (server) certificates signed by the CA with SAN support
  for both DNS names and IP addresses.
* Track issued certificate serial numbers.
* Generate per-OS instructions for trusting the CA certificate.
"""

from __future__ import annotations

import ipaddress
import json
import logging
import os
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Union

from cryptography import x509
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, rsa
from cryptography.hazmat.primitives.serialization import (
    Encoding,
    NoEncryption,
    PrivateFormat,
)
from cryptography.x509.oid import ExtendedKeyUsageOID, NameOID

logger = logging.getLogger(__name__)

# Default validity periods
DEFAULT_CA_VALIDITY_DAYS = 3650  # ~10 years
DEFAULT_CERT_VALIDITY_DAYS = 825  # ~2.25 years (Apple limit)
CA_KEY_SIZE = 4096
SERVER_KEY_SIZE = 2048


class LocalCA:
    """Self-contained local Certificate Authority.

    Parameters
    ----------
    data_dir:
        Directory where CA material (cert, key, serial tracking) is stored.
    ca_cn:
        Common Name for the CA certificate if one needs to be generated.
    """

    CA_CERT_FILE = "ca.pem"
    CA_KEY_FILE = "ca.key"
    SERIAL_FILE = "serial.json"

    def __init__(
        self,
        data_dir: str | Path,
        ca_cn: str = "HA-VoIP Local CA",
    ) -> None:
        self.data_dir = Path(data_dir)
        self.data_dir.mkdir(parents=True, exist_ok=True)
        (self.data_dir / "issued").mkdir(exist_ok=True)
        self.ca_cn = ca_cn
        self._serial_path = self.data_dir / self.SERIAL_FILE

    # ------------------------------------------------------------------
    # Serial number tracking
    # ------------------------------------------------------------------
    def _load_serials(self) -> Dict:
        if self._serial_path.exists():
            return json.loads(self._serial_path.read_text())
        return {"next_serial": 2, "issued": []}

    def _save_serials(self, data: Dict) -> None:
        self._serial_path.write_text(json.dumps(data, indent=2))

    def _next_serial(self) -> int:
        data = self._load_serials()
        serial = data["next_serial"]
        data["next_serial"] = serial + 1
        self._save_serials(data)
        return serial

    def _record_issued(
        self,
        serial: int,
        domain: str,
        san_list: List[str],
        not_after: datetime,
    ) -> None:
        data = self._load_serials()
        data["issued"].append(
            {
                "serial": serial,
                "domain": domain,
                "san_list": san_list,
                "not_after": not_after.isoformat(),
                "issued_at": datetime.now(timezone.utc).isoformat(),
            }
        )
        self._save_serials(data)

    # ------------------------------------------------------------------
    # CA generation
    # ------------------------------------------------------------------
    def generate_ca(
        self,
        cn: Optional[str] = None,
        validity_days: int = DEFAULT_CA_VALIDITY_DAYS,
        organization: str = "HA-VoIP",
    ) -> Tuple[bytes, bytes]:
        """Generate a new root CA certificate and key pair.

        Parameters
        ----------
        cn:
            Common Name for the CA.  Defaults to ``self.ca_cn``.
        validity_days:
            How many days the CA cert is valid.
        organization:
            Organization name for the CA subject.

        Returns
        -------
        (ca_cert_pem, ca_key_pem)
        """
        cn = cn or self.ca_cn

        # Generate RSA key for CA
        ca_key = rsa.generate_private_key(
            public_exponent=65537,
            key_size=CA_KEY_SIZE,
        )

        now = datetime.now(timezone.utc)
        subject = issuer = x509.Name(
            [
                x509.NameAttribute(NameOID.COMMON_NAME, cn),
                x509.NameAttribute(NameOID.ORGANIZATION_NAME, organization),
                x509.NameAttribute(NameOID.ORGANIZATIONAL_UNIT_NAME, "Certificate Authority"),
            ]
        )

        ca_cert = (
            x509.CertificateBuilder()
            .subject_name(subject)
            .issuer_name(issuer)
            .public_key(ca_key.public_key())
            .serial_number(1)
            .not_valid_before(now)
            .not_valid_after(now + timedelta(days=validity_days))
            .add_extension(
                x509.BasicConstraints(ca=True, path_length=0),
                critical=True,
            )
            .add_extension(
                x509.KeyUsage(
                    digital_signature=True,
                    content_commitment=False,
                    key_encipherment=False,
                    data_encipherment=False,
                    key_agreement=False,
                    key_cert_sign=True,
                    crl_sign=True,
                    encipher_only=False,
                    decipher_only=False,
                ),
                critical=True,
            )
            .add_extension(
                x509.SubjectKeyIdentifier.from_public_key(ca_key.public_key()),
                critical=False,
            )
            .sign(ca_key, hashes.SHA256())
        )

        ca_cert_pem = ca_cert.public_bytes(Encoding.PEM)
        ca_key_pem = ca_key.private_bytes(Encoding.PEM, PrivateFormat.PKCS8, NoEncryption())

        # Persist
        (self.data_dir / self.CA_CERT_FILE).write_bytes(ca_cert_pem)
        key_path = self.data_dir / self.CA_KEY_FILE
        key_path.write_bytes(ca_key_pem)
        try:
            os.chmod(key_path, 0o600)
        except OSError:
            pass

        # Reset serial tracking
        self._save_serials({"next_serial": 2, "issued": []})

        logger.info(
            "Generated local CA: CN=%s, valid until %s",
            cn,
            (now + timedelta(days=validity_days)).isoformat(),
        )
        return ca_cert_pem, ca_key_pem

    # ------------------------------------------------------------------
    # Helpers to load existing CA
    # ------------------------------------------------------------------
    def _load_ca(self) -> Tuple[x509.Certificate, rsa.RSAPrivateKey]:
        """Load the CA cert and key from disk."""
        cert_path = self.data_dir / self.CA_CERT_FILE
        key_path = self.data_dir / self.CA_KEY_FILE

        if not cert_path.exists() or not key_path.exists():
            raise FileNotFoundError(
                "CA certificate or key not found. Call generate_ca() first."
            )

        ca_cert = x509.load_pem_x509_certificate(cert_path.read_bytes())
        ca_key = serialization.load_pem_private_key(key_path.read_bytes(), password=None)

        if not isinstance(ca_key, rsa.RSAPrivateKey):
            raise TypeError("CA key must be RSA")

        return ca_cert, ca_key

    def has_ca(self) -> bool:
        """Return True if a CA cert and key exist on disk."""
        return (
            (self.data_dir / self.CA_CERT_FILE).exists()
            and (self.data_dir / self.CA_KEY_FILE).exists()
        )

    # ------------------------------------------------------------------
    # Certificate issuance
    # ------------------------------------------------------------------
    @staticmethod
    def _parse_san_entry(entry: str) -> Union[x509.DNSName, x509.IPAddress]:
        """Parse a SAN string into either a DNSName or IPAddress."""
        entry = entry.strip()
        try:
            addr = ipaddress.ip_address(entry)
            return x509.IPAddress(addr)
        except ValueError:
            pass
        try:
            net = ipaddress.ip_network(entry, strict=False)
            return x509.IPAddress(net)
        except ValueError:
            pass
        return x509.DNSName(entry)

    def issue_certificate(
        self,
        domain: str,
        san_list: Optional[List[str]] = None,
        ca_cert: Optional[bytes] = None,
        ca_key: Optional[bytes] = None,
        validity_days: int = DEFAULT_CERT_VALIDITY_DAYS,
        key_size: int = SERVER_KEY_SIZE,
    ) -> Tuple[bytes, bytes]:
        """Issue a server certificate signed by the local CA.

        Parameters
        ----------
        domain:
            Primary domain / CN for the certificate.
        san_list:
            Additional Subject Alternative Names (hostnames or IPs).
            ``domain`` is always included automatically.
        ca_cert:
            PEM-encoded CA certificate.  If ``None``, loads from disk.
        ca_key:
            PEM-encoded CA private key.  If ``None``, loads from disk.
        validity_days:
            Certificate validity period.
        key_size:
            RSA key size for the server certificate.

        Returns
        -------
        (cert_pem, key_pem)
        """
        # Resolve CA material
        if ca_cert and ca_key:
            ca_certificate = x509.load_pem_x509_certificate(ca_cert)
            ca_private_key = serialization.load_pem_private_key(ca_key, password=None)
        else:
            ca_certificate, ca_private_key = self._load_ca()

        # Build SAN list (always include the primary domain)
        all_sans_str = [domain]
        if san_list:
            for s in san_list:
                if s not in all_sans_str:
                    all_sans_str.append(s)

        san_objects = [self._parse_san_entry(s) for s in all_sans_str]

        # Generate server key
        server_key = rsa.generate_private_key(
            public_exponent=65537,
            key_size=key_size,
        )

        serial = self._next_serial()
        now = datetime.now(timezone.utc)
        not_after = now + timedelta(days=validity_days)

        subject = x509.Name(
            [
                x509.NameAttribute(NameOID.COMMON_NAME, domain),
                x509.NameAttribute(NameOID.ORGANIZATION_NAME, "HA-VoIP"),
            ]
        )

        builder = (
            x509.CertificateBuilder()
            .subject_name(subject)
            .issuer_name(ca_certificate.subject)
            .public_key(server_key.public_key())
            .serial_number(serial)
            .not_valid_before(now)
            .not_valid_after(not_after)
            .add_extension(
                x509.BasicConstraints(ca=False, path_length=None),
                critical=True,
            )
            .add_extension(
                x509.KeyUsage(
                    digital_signature=True,
                    content_commitment=False,
                    key_encipherment=True,
                    data_encipherment=False,
                    key_agreement=False,
                    key_cert_sign=False,
                    crl_sign=False,
                    encipher_only=False,
                    decipher_only=False,
                ),
                critical=True,
            )
            .add_extension(
                x509.ExtendedKeyUsage(
                    [
                        ExtendedKeyUsageOID.SERVER_AUTH,
                        ExtendedKeyUsageOID.CLIENT_AUTH,
                    ]
                ),
                critical=False,
            )
            .add_extension(
                x509.SubjectAlternativeName(san_objects),
                critical=False,
            )
            .add_extension(
                x509.SubjectKeyIdentifier.from_public_key(server_key.public_key()),
                critical=False,
            )
            .add_extension(
                x509.AuthorityKeyIdentifier.from_issuer_public_key(
                    ca_private_key.public_key()  # type: ignore[arg-type]
                ),
                critical=False,
            )
        )

        cert = builder.sign(ca_private_key, hashes.SHA256())  # type: ignore[arg-type]

        cert_pem = cert.public_bytes(Encoding.PEM)
        key_pem = server_key.private_bytes(Encoding.PEM, PrivateFormat.PKCS8, NoEncryption())

        # Store issued certificate
        safe_domain = domain.replace("*", "_wildcard_").replace(":", "_")
        issued_dir = self.data_dir / "issued" / safe_domain
        issued_dir.mkdir(parents=True, exist_ok=True)
        (issued_dir / "cert.pem").write_bytes(cert_pem)
        key_file = issued_dir / "key.pem"
        key_file.write_bytes(key_pem)
        try:
            os.chmod(key_file, 0o600)
        except OSError:
            pass

        # Record serial
        self._record_issued(serial, domain, all_sans_str, not_after)

        logger.info(
            "Issued certificate: CN=%s, serial=%d, SANs=%s, expires %s",
            domain,
            serial,
            all_sans_str,
            not_after.isoformat(),
        )
        return cert_pem, key_pem

    # ------------------------------------------------------------------
    # CA certificate retrieval (for user import)
    # ------------------------------------------------------------------
    def get_ca_certificate(self) -> bytes:
        """Return the CA certificate PEM for users to import into trust stores.

        Raises ``FileNotFoundError`` if no CA has been generated.
        """
        cert_path = self.data_dir / self.CA_CERT_FILE
        if not cert_path.exists():
            raise FileNotFoundError("No CA certificate found. Call generate_ca() first.")
        return cert_path.read_bytes()

    def get_ca_certificate_der(self) -> bytes:
        """Return the CA certificate in DER format (useful for Windows / Android)."""
        pem = self.get_ca_certificate()
        cert = x509.load_pem_x509_certificate(pem)
        return cert.public_bytes(Encoding.DER)

    # ------------------------------------------------------------------
    # Install instructions
    # ------------------------------------------------------------------
    def generate_install_instructions(self) -> str:
        """Generate human-readable instructions for installing the CA cert
        into various operating system trust stores.

        Returns a multi-line string with instructions for macOS, Windows,
        Linux, iOS, and Android.
        """
        ca_path = str(self.data_dir / self.CA_CERT_FILE)

        instructions = f"""\
========================================================================
  Local CA Certificate Installation Instructions
========================================================================

Your CA certificate is located at:
  {ca_path}

You must install this certificate in your device's trust store so that
browsers and SIP clients will accept the self-signed certificates
issued by this CA.

--- macOS ---
  1. Double-click the CA certificate file (ca.pem) to open Keychain Access.
  2. Add it to the "System" keychain.
  3. Find the certificate, double-click it, expand "Trust", and set
     "When using this certificate" to "Always Trust".
  4. Close the dialog and enter your password to confirm.

  Or via terminal:
    sudo security add-trusted-cert -d -r trustRoot \\
      -k /Library/Keychains/System.keychain "{ca_path}"

--- Windows ---
  1. Rename ca.pem to ca.crt (or use the DER format export).
  2. Double-click the file and click "Install Certificate...".
  3. Select "Local Machine" -> "Place all certificates in the following
     store" -> Browse -> "Trusted Root Certification Authorities".
  4. Click Finish and confirm the security warning.

  Or via PowerShell (Admin):
    Import-Certificate -FilePath "{ca_path}" \\
      -CertStoreLocation Cert:\\LocalMachine\\Root

--- Linux (Debian/Ubuntu) ---
    sudo cp "{ca_path}" /usr/local/share/ca-certificates/ha-voip-ca.crt
    sudo update-ca-certificates

--- Linux (RHEL/Fedora/CentOS) ---
    sudo cp "{ca_path}" /etc/pki/ca-trust/source/anchors/ha-voip-ca.pem
    sudo update-ca-trust extract

--- iOS ---
  1. Email or AirDrop the CA certificate to the device.
  2. Open Settings -> General -> VPN & Device Management.
  3. Install the profile.
  4. Go to Settings -> General -> About -> Certificate Trust Settings.
  5. Enable full trust for the HA-VoIP CA.

--- Android ---
  1. Copy the CA certificate to the device.
  2. Go to Settings -> Security -> Encryption & credentials.
  3. Tap "Install a certificate" -> "CA certificate".
  4. Select the file and confirm.

--- Firefox (all platforms) ---
  Firefox uses its own certificate store:
  1. Open Preferences/Settings -> Privacy & Security -> Certificates.
  2. Click "View Certificates" -> "Authorities" -> "Import".
  3. Select the CA certificate and check "Trust this CA to identify websites".

========================================================================
"""
        return instructions

    # ------------------------------------------------------------------
    # Listing issued certificates
    # ------------------------------------------------------------------
    def list_issued(self) -> List[Dict]:
        """Return metadata for all certificates issued by this CA."""
        data = self._load_serials()
        return data.get("issued", [])

    def get_issued_certificate(self, domain: str) -> Optional[Tuple[bytes, bytes]]:
        """Retrieve ``(cert_pem, key_pem)`` for a previously issued cert."""
        safe = domain.replace("*", "_wildcard_").replace(":", "_")
        issued_dir = self.data_dir / "issued" / safe
        cert_path = issued_dir / "cert.pem"
        key_path = issued_dir / "key.pem"
        if not cert_path.exists() or not key_path.exists():
            return None
        return cert_path.read_bytes(), key_path.read_bytes()
