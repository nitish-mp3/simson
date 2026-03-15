"""HA-VoIP provisioning package.

Re-exports the primary classes for convenient access::

    from provisioning import (
        CertificateManager,
        NetworkTester,
        BackupManager,
        ProvisioningAPI,
        create_provisioning_app,
    )
"""

from __future__ import annotations

from .backup_manager import BackupManager, BackupInfo
from .certificate_manager import (
    ACMEClient,
    ACMEError,
    CertBundle,
    CertificateManager,
    CertInfo,
    CertMode,
    CertStatus,
    CertStore,
    LocalCA,
)
from .network_tester import (
    DiagnosticResult,
    NATDetectionResult,
    NATType,
    NetworkTester,
    PortTestResult,
    STUNResult,
)
from .provisioning_api import (
    Extension,
    ExtensionStore,
    ProvisioningAPI,
    create_provisioning_app,
)

__all__ = [
    # Certificate management
    "CertificateManager",
    "CertMode",
    "CertStatus",
    "CertBundle",
    "CertInfo",
    "CertStore",
    "ACMEClient",
    "ACMEError",
    "LocalCA",
    # Network testing
    "NetworkTester",
    "DiagnosticResult",
    "NATDetectionResult",
    "NATType",
    "PortTestResult",
    "STUNResult",
    # Backup
    "BackupManager",
    "BackupInfo",
    # Provisioning API
    "ProvisioningAPI",
    "Extension",
    "ExtensionStore",
    "create_provisioning_app",
]
