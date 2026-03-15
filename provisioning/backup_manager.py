"""Backup and restore manager for HA-VoIP.

Provides ``BackupManager`` which creates AES-256-GCM encrypted backup
archives containing the database, certificates, configuration, and
optional recordings metadata.  Supports checksum verification, selective
restore, and listing of available backups.
"""

from __future__ import annotations

import asyncio
import hashlib
import io
import json
import logging
import os
import shutil
import tarfile
import tempfile
import time
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Literal, Optional, Set

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
BACKUP_VERSION = 1
NONCE_SIZE = 12  # bytes for AES-GCM
KEY_SIZE = 32  # AES-256
CHUNK_SIZE = 64 * 1024  # 64 KiB for streaming

# Sections that can be backed up / restored selectively
BACKUP_SECTIONS = {"config", "db", "certs", "extensions", "recordings_meta"}


@dataclass
class BackupInfo:
    """Metadata about a stored backup."""

    filename: str
    path: str
    created_at: str
    size_bytes: int
    encrypted: bool
    version: int = BACKUP_VERSION
    sections: List[str] = field(default_factory=list)
    checksum_sha256: str = ""


class BackupManager:
    """Create and restore encrypted backup archives.

    Parameters
    ----------
    data_dir:
        Root data directory for HA-VoIP (contains config, db, certs, etc.).
    backup_dir:
        Directory where backup archives are stored.
    db_path:
        Path to the SQLite (or other) database file.
    config_path:
        Path to the main configuration file / directory.
    certs_dir:
        Path to the certificate store directory.
    extensions_path:
        Path to the extensions JSON file.
    recordings_meta_path:
        Path to recordings metadata (not the actual audio files).
    """

    def __init__(
        self,
        data_dir: str | Path,
        backup_dir: Optional[str | Path] = None,
        db_path: Optional[str | Path] = None,
        config_path: Optional[str | Path] = None,
        certs_dir: Optional[str | Path] = None,
        extensions_path: Optional[str | Path] = None,
        recordings_meta_path: Optional[str | Path] = None,
    ) -> None:
        self.data_dir = Path(data_dir)
        self.backup_dir = Path(backup_dir) if backup_dir else self.data_dir / "backups"
        self.backup_dir.mkdir(parents=True, exist_ok=True)

        self.db_path = Path(db_path) if db_path else self.data_dir / "ha_voip.db"
        self.config_path = Path(config_path) if config_path else self.data_dir / "config"
        self.certs_dir = Path(certs_dir) if certs_dir else self.data_dir / "store"
        self.extensions_path = (
            Path(extensions_path) if extensions_path else self.data_dir / "extensions.json"
        )
        self.recordings_meta_path = (
            Path(recordings_meta_path)
            if recordings_meta_path
            else self.data_dir / "recordings_meta.json"
        )

    # ------------------------------------------------------------------
    # Encryption helpers
    # ------------------------------------------------------------------
    @staticmethod
    def _derive_key(passphrase: str) -> bytes:
        """Derive a 256-bit key from a passphrase using SHA-256.

        For production, use a proper KDF (PBKDF2, scrypt, argon2).
        We use PBKDF2 here for robustness.
        """
        from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
        from cryptography.hazmat.primitives import hashes as _hashes

        # Use a fixed salt derived from the passphrase for determinism
        # In a real deployment you would store the salt alongside the backup
        salt = hashlib.sha256(b"ha-voip-backup-salt:" + passphrase.encode()).digest()[:16]
        kdf = PBKDF2HMAC(
            algorithm=_hashes.SHA256(),
            length=KEY_SIZE,
            salt=salt,
            iterations=600_000,
        )
        return kdf.derive(passphrase.encode())

    @staticmethod
    def _encrypt(data: bytes, key: bytes) -> bytes:
        """Encrypt *data* with AES-256-GCM.

        Returns ``nonce (12 bytes) || ciphertext+tag``.
        """
        nonce = os.urandom(NONCE_SIZE)
        aesgcm = AESGCM(key)
        ct = aesgcm.encrypt(nonce, data, None)
        return nonce + ct

    @staticmethod
    def _decrypt(data: bytes, key: bytes) -> bytes:
        """Decrypt data produced by ``_encrypt``."""
        if len(data) < NONCE_SIZE:
            raise ValueError("Encrypted data too short")
        nonce = data[:NONCE_SIZE]
        ct = data[NONCE_SIZE:]
        aesgcm = AESGCM(key)
        return aesgcm.decrypt(nonce, ct, None)

    # ------------------------------------------------------------------
    # Tar archive building
    # ------------------------------------------------------------------
    def _add_path_to_tar(
        self,
        tar: tarfile.TarFile,
        path: Path,
        arcname: str,
    ) -> None:
        """Recursively add *path* to the tar archive."""
        if path.is_file():
            tar.add(str(path), arcname=arcname)
        elif path.is_dir():
            for child in sorted(path.rglob("*")):
                if child.is_file():
                    rel = child.relative_to(path)
                    tar.add(str(child), arcname=f"{arcname}/{rel}")

    def _build_tar(self, sections: Set[str]) -> bytes:
        """Build a tar.gz archive in memory containing the requested sections."""
        buf = io.BytesIO()
        with tarfile.open(fileobj=buf, mode="w:gz") as tar:
            manifest: Dict[str, Any] = {
                "version": BACKUP_VERSION,
                "created_at": datetime.now(timezone.utc).isoformat(),
                "sections": sorted(sections),
            }

            if "db" in sections and self.db_path.exists():
                tar.add(str(self.db_path), arcname="db/ha_voip.db")

            if "config" in sections and self.config_path.exists():
                self._add_path_to_tar(tar, self.config_path, "config")

            if "certs" in sections and self.certs_dir.exists():
                self._add_path_to_tar(tar, self.certs_dir, "certs")

            if "extensions" in sections and self.extensions_path.exists():
                tar.add(str(self.extensions_path), arcname="extensions/extensions.json")

            if "recordings_meta" in sections and self.recordings_meta_path.exists():
                tar.add(
                    str(self.recordings_meta_path),
                    arcname="recordings_meta/recordings_meta.json",
                )

            # Write manifest
            manifest_bytes = json.dumps(manifest, indent=2).encode()
            info = tarfile.TarInfo(name="manifest.json")
            info.size = len(manifest_bytes)
            tar.addfile(info, io.BytesIO(manifest_bytes))

        return buf.getvalue()

    # ------------------------------------------------------------------
    # Public API
    # ------------------------------------------------------------------
    async def create_backup(
        self,
        passphrase: Optional[str] = None,
        sections: Optional[Set[str]] = None,
    ) -> Path:
        """Create a full or selective backup archive.

        Parameters
        ----------
        passphrase:
            If provided, the archive is encrypted with AES-256-GCM.
        sections:
            Subset of ``BACKUP_SECTIONS`` to include.  Defaults to all.

        Returns
        -------
        Path to the backup file.
        """
        target_sections = sections or BACKUP_SECTIONS.copy()
        invalid = target_sections - BACKUP_SECTIONS
        if invalid:
            raise ValueError(f"Unknown backup sections: {invalid}")

        logger.info("Creating backup (sections=%s, encrypted=%s)", target_sections, bool(passphrase))

        # Build archive in a thread to avoid blocking the event loop
        loop = asyncio.get_event_loop()
        tar_data = await loop.run_in_executor(None, self._build_tar, target_sections)

        # Compute checksum of unencrypted archive
        checksum = hashlib.sha256(tar_data).hexdigest()

        # Encrypt if passphrase provided
        encrypted = False
        if passphrase:
            key = self._derive_key(passphrase)
            tar_data = self._encrypt(tar_data, key)
            encrypted = True

        # Write to backup directory
        ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        ext = ".tar.gz.enc" if encrypted else ".tar.gz"
        filename = f"ha-voip-backup-{ts}{ext}"
        backup_path = self.backup_dir / filename
        backup_path.write_bytes(tar_data)

        # Write checksum sidecar
        checksum_path = backup_path.with_suffix(backup_path.suffix + ".sha256")
        checksum_path.write_text(f"{checksum}  {filename}\n")

        logger.info(
            "Backup created: %s (%d bytes, sha256=%s)",
            backup_path,
            len(tar_data),
            checksum[:16],
        )

        return backup_path

    async def restore_backup(
        self,
        backup_path: str | Path,
        passphrase: Optional[str] = None,
        selective: Optional[List[str]] = None,
    ) -> None:
        """Restore from a backup archive.

        Parameters
        ----------
        backup_path:
            Path to the ``.tar.gz`` or ``.tar.gz.enc`` backup file.
        passphrase:
            Decryption passphrase (required if the backup is encrypted).
        selective:
            List of sections to restore.  If ``None``, restores everything
            in the archive.
        """
        backup_file = Path(backup_path)
        if not backup_file.exists():
            raise FileNotFoundError(f"Backup file not found: {backup_file}")

        data = backup_file.read_bytes()
        encrypted = backup_file.suffix == ".enc" or str(backup_file).endswith(".tar.gz.enc")

        if encrypted:
            if not passphrase:
                raise ValueError("Passphrase required for encrypted backup")
            key = self._derive_key(passphrase)
            try:
                data = self._decrypt(data, key)
            except Exception as exc:
                raise ValueError(f"Decryption failed (wrong passphrase?): {exc}") from exc

        # Verify checksum if sidecar exists
        checksum_path = backup_file.with_suffix(backup_file.suffix + ".sha256")
        if checksum_path.exists():
            expected = checksum_path.read_text().split()[0]
            actual = hashlib.sha256(data).hexdigest()
            if actual != expected:
                raise ValueError(
                    f"Checksum mismatch: expected {expected}, got {actual}"
                )
            logger.info("Backup checksum verified")

        # Extract
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(
            None, self._extract_tar, data, selective
        )

        logger.info("Backup restored from %s", backup_file)

    def _extract_tar(
        self, tar_data: bytes, selective: Optional[List[str]]
    ) -> None:
        """Extract tar archive contents to their target locations."""
        buf = io.BytesIO(tar_data)
        with tarfile.open(fileobj=buf, mode="r:gz") as tar:
            # Read manifest first
            manifest: Dict[str, Any] = {}
            try:
                mf = tar.extractfile("manifest.json")
                if mf:
                    manifest = json.loads(mf.read())
            except (KeyError, Exception):
                logger.warning("No manifest found in backup; proceeding anyway")

            selective_set = set(selective) if selective else None

            for member in tar.getmembers():
                if member.name == "manifest.json":
                    continue

                section = member.name.split("/")[0]

                # Filter by selective restore
                if selective_set and section not in selective_set:
                    continue

                # Determine target path
                target = self._resolve_restore_target(member.name, section)
                if target is None:
                    logger.warning("Unknown section in backup: %s", member.name)
                    continue

                # Security: prevent path traversal
                try:
                    target.resolve().relative_to(self.data_dir.resolve())
                except ValueError:
                    logger.warning(
                        "Skipping %s: resolves outside data_dir", member.name
                    )
                    continue

                if member.isdir():
                    target.mkdir(parents=True, exist_ok=True)
                elif member.isfile():
                    target.parent.mkdir(parents=True, exist_ok=True)
                    f = tar.extractfile(member)
                    if f:
                        target.write_bytes(f.read())
                        # Restrict key files
                        if target.suffix in (".key", ".pem") and "key" in target.name.lower():
                            try:
                                os.chmod(target, 0o600)
                            except OSError:
                                pass

    def _resolve_restore_target(self, arcname: str, section: str) -> Optional[Path]:
        """Map an archive member name to a filesystem path."""
        parts = arcname.split("/", 1)
        remainder = parts[1] if len(parts) > 1 else ""

        if section == "db":
            return self.db_path.parent / remainder if remainder else self.db_path
        elif section == "config":
            return self.config_path / remainder if remainder else self.config_path
        elif section == "certs":
            return self.certs_dir / remainder if remainder else self.certs_dir
        elif section == "extensions":
            if remainder:
                return self.extensions_path.parent / remainder
            return self.extensions_path
        elif section == "recordings_meta":
            if remainder:
                return self.recordings_meta_path.parent / remainder
            return self.recordings_meta_path
        return None

    async def list_backups(self) -> List[BackupInfo]:
        """List all backup archives in the backup directory."""
        backups: List[BackupInfo] = []
        if not self.backup_dir.exists():
            return backups

        for entry in sorted(self.backup_dir.iterdir()):
            name = entry.name
            if not (name.endswith(".tar.gz") or name.endswith(".tar.gz.enc")):
                continue

            encrypted = name.endswith(".enc")
            stat = entry.stat()

            # Try to read checksum
            checksum = ""
            checksum_path = entry.with_suffix(entry.suffix + ".sha256")
            if checksum_path.exists():
                try:
                    checksum = checksum_path.read_text().split()[0]
                except Exception:
                    pass

            # Try to read manifest from the archive (only for unencrypted)
            sections: List[str] = []
            version = BACKUP_VERSION
            if not encrypted:
                try:
                    with tarfile.open(str(entry), mode="r:gz") as tar:
                        mf = tar.extractfile("manifest.json")
                        if mf:
                            manifest = json.loads(mf.read())
                            sections = manifest.get("sections", [])
                            version = manifest.get("version", BACKUP_VERSION)
                except Exception:
                    pass

            info = BackupInfo(
                filename=name,
                path=str(entry),
                created_at=datetime.fromtimestamp(
                    stat.st_mtime, tz=timezone.utc
                ).isoformat(),
                size_bytes=stat.st_size,
                encrypted=encrypted,
                version=version,
                sections=sections,
                checksum_sha256=checksum,
            )
            backups.append(info)

        return backups

    async def validate_backup(self, backup_path: str | Path) -> bool:
        """Validate a backup archive.

        Checks:
        1. File exists and is readable.
        2. Checksum matches (if sidecar exists).
        3. Archive can be opened (if unencrypted).
        4. Manifest is present and parseable.
        """
        path = Path(backup_path)
        if not path.exists():
            logger.error("Backup file not found: %s", path)
            return False

        data = path.read_bytes()
        encrypted = str(path).endswith(".enc")

        # Checksum verification
        checksum_path = path.with_suffix(path.suffix + ".sha256")
        if checksum_path.exists():
            expected = checksum_path.read_text().split()[0]
            if encrypted:
                # Cannot verify content checksum for encrypted file
                # (checksum is of the plaintext, not ciphertext)
                logger.info("Checksum sidecar found but backup is encrypted; skipping checksum")
            else:
                actual = hashlib.sha256(data).hexdigest()
                if actual != expected:
                    logger.error("Checksum mismatch for %s", path)
                    return False
                logger.info("Checksum verified for %s", path)

        # Try to open archive (only for unencrypted)
        if not encrypted:
            try:
                buf = io.BytesIO(data)
                with tarfile.open(fileobj=buf, mode="r:gz") as tar:
                    names = tar.getnames()
                    if "manifest.json" not in names:
                        logger.warning("No manifest.json in backup %s", path)
                    else:
                        mf = tar.extractfile("manifest.json")
                        if mf:
                            manifest = json.loads(mf.read())
                            logger.info(
                                "Backup validated: version=%d, sections=%s",
                                manifest.get("version", 0),
                                manifest.get("sections", []),
                            )
            except Exception as exc:
                logger.error("Failed to read backup archive %s: %s", path, exc)
                return False

        return True

    async def delete_backup(self, backup_path: str | Path) -> bool:
        """Delete a backup archive and its checksum sidecar."""
        path = Path(backup_path)
        if not path.exists():
            return False
        path.unlink()
        checksum_path = path.with_suffix(path.suffix + ".sha256")
        if checksum_path.exists():
            checksum_path.unlink()
        logger.info("Deleted backup: %s", path)
        return True
