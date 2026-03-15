"""Network connectivity tester for HA-VoIP.

Implements ``NetworkTester`` which runs diagnostics against WSS, TURN,
STUN, and RTP port ranges to verify that the network environment is
suitable for VoIP operation.  Includes the exact port-fallback algorithm
used by the HA-VoIP engine and automatic NAT type detection via STUN.
"""

from __future__ import annotations

import asyncio
import enum
import json
import logging
import socket
import struct
import time
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

logger = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------
WSS_PORTS = [443, 8443, 7443, 5061]
TURN_UDP_PORTS = [3478]
TURN_TCP_PORTS = [3478]
TURN_TLS_PORTS = [5349, 443]
DEFAULT_STUN_SERVER = "stun.l.google.com"
DEFAULT_STUN_PORT = 19302
CONNECT_TIMEOUT = 5.0  # seconds
STUN_TIMEOUT = 5.0

# STUN message constants (RFC 5389)
STUN_MAGIC_COOKIE = 0x2112A442
STUN_BINDING_REQUEST = 0x0001
STUN_BINDING_RESPONSE = 0x0101
STUN_ATTR_MAPPED_ADDRESS = 0x0001
STUN_ATTR_XOR_MAPPED_ADDRESS = 0x0020
STUN_ATTR_CHANGE_REQUEST = 0x0003
STUN_ATTR_OTHER_ADDRESS = 0x802C


class NATType(str, enum.Enum):
    """Detected NAT type."""

    OPEN = "open"
    FULL_CONE = "full_cone"
    RESTRICTED_CONE = "restricted_cone"
    PORT_RESTRICTED = "port_restricted"
    SYMMETRIC = "symmetric"
    BLOCKED = "blocked"
    UNKNOWN = "unknown"


@dataclass
class PortTestResult:
    """Result of testing a single port."""

    port: int
    protocol: str  # "tcp", "udp", "tls"
    reachable: bool
    latency_ms: float = 0.0
    error: str = ""


@dataclass
class STUNResult:
    """Result of a STUN binding test."""

    server: str
    port: int
    success: bool
    mapped_address: str = ""
    mapped_port: int = 0
    latency_ms: float = 0.0
    error: str = ""


@dataclass
class NATDetectionResult:
    """Result of NAT type detection."""

    nat_type: NATType = NATType.UNKNOWN
    external_ip: str = ""
    external_port: int = 0
    details: str = ""


@dataclass
class DiagnosticResult:
    """Full diagnostic result."""

    timestamp: str = ""
    host: str = ""
    wss_results: List[PortTestResult] = field(default_factory=list)
    turn_results: List[PortTestResult] = field(default_factory=list)
    rtp_results: List[PortTestResult] = field(default_factory=list)
    stun_result: Optional[STUNResult] = None
    nat_detection: Optional[NATDetectionResult] = None
    recommended_wss_port: Optional[int] = None
    recommended_turn_config: Optional[str] = None
    recommendations: List[str] = field(default_factory=list)
    overall_status: str = "unknown"  # "good", "degraded", "failed"

    def __post_init__(self) -> None:
        if not self.timestamp:
            self.timestamp = datetime.now(timezone.utc).isoformat()

    def to_dict(self) -> Dict[str, Any]:
        """Convert to a JSON-serializable dictionary."""
        d: Dict[str, Any] = {
            "timestamp": self.timestamp,
            "host": self.host,
            "overall_status": self.overall_status,
            "recommendations": self.recommendations,
            "recommended_wss_port": self.recommended_wss_port,
            "recommended_turn_config": self.recommended_turn_config,
            "wss_results": [asdict(r) for r in self.wss_results],
            "turn_results": [asdict(r) for r in self.turn_results],
            "rtp_results": [asdict(r) for r in self.rtp_results],
        }
        if self.stun_result:
            d["stun_result"] = asdict(self.stun_result)
        if self.nat_detection:
            d["nat_detection"] = asdict(self.nat_detection)
        return d

    def to_json(self, indent: int = 2) -> str:
        """Serialize to JSON string."""
        return json.dumps(self.to_dict(), indent=indent)

    def export_support_bundle(self, path: str | Path) -> Path:
        """Write diagnostic results to a JSON file for support."""
        out = Path(path)
        out.write_text(self.to_json())
        logger.info("Support bundle exported to %s", out)
        return out


class NetworkTester:
    """Run VoIP network connectivity diagnostics.

    Parameters
    ----------
    default_host:
        Default target host if not specified per-test.
    rtp_port_start / rtp_port_end:
        RTP port range to test.
    stun_server:
        Default STUN server address.
    stun_port:
        Default STUN server port.
    """

    def __init__(
        self,
        default_host: str = "",
        rtp_port_start: int = 10000,
        rtp_port_end: int = 10010,
        stun_server: str = DEFAULT_STUN_SERVER,
        stun_port: int = DEFAULT_STUN_PORT,
    ) -> None:
        self.default_host = default_host
        self.rtp_port_start = rtp_port_start
        self.rtp_port_end = rtp_port_end
        self.stun_server = stun_server
        self.stun_port = stun_port

    # ------------------------------------------------------------------
    # TCP connectivity check
    # ------------------------------------------------------------------
    async def _test_tcp_port(
        self, host: str, port: int, use_tls: bool = False
    ) -> PortTestResult:
        """Test TCP (optionally TLS) connectivity to host:port."""
        proto = "tls" if use_tls else "tcp"
        start = time.monotonic()
        try:
            if use_tls:
                import ssl

                ctx = ssl.create_default_context()
                ctx.check_hostname = False
                ctx.verify_mode = ssl.CERT_NONE
                _, writer = await asyncio.wait_for(
                    asyncio.open_connection(host, port, ssl=ctx),
                    timeout=CONNECT_TIMEOUT,
                )
            else:
                _, writer = await asyncio.wait_for(
                    asyncio.open_connection(host, port),
                    timeout=CONNECT_TIMEOUT,
                )

            elapsed = (time.monotonic() - start) * 1000
            writer.close()
            await writer.wait_closed()

            return PortTestResult(
                port=port,
                protocol=proto,
                reachable=True,
                latency_ms=round(elapsed, 2),
            )
        except Exception as exc:
            elapsed = (time.monotonic() - start) * 1000
            return PortTestResult(
                port=port,
                protocol=proto,
                reachable=False,
                latency_ms=round(elapsed, 2),
                error=str(exc),
            )

    # ------------------------------------------------------------------
    # UDP connectivity check
    # ------------------------------------------------------------------
    async def _test_udp_port(self, host: str, port: int) -> PortTestResult:
        """Test UDP connectivity by sending a small probe packet.

        Note: UDP being connectionless means we can only detect if the
        port is *not* blocked (ICMP unreachable) or if we get a response.
        For STUN/TURN servers this works well; for arbitrary hosts it is
        less reliable.
        """
        start = time.monotonic()
        loop = asyncio.get_event_loop()
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.setblocking(False)
            sock.settimeout(CONNECT_TIMEOUT)

            # Send a STUN binding request as the probe (works for TURN servers)
            probe = self._build_stun_request()
            await loop.sock_sendto(sock, probe, (host, port))

            try:
                data = await asyncio.wait_for(
                    loop.sock_recv(sock, 1024),
                    timeout=CONNECT_TIMEOUT,
                )
                elapsed = (time.monotonic() - start) * 1000
                sock.close()
                return PortTestResult(
                    port=port,
                    protocol="udp",
                    reachable=True,
                    latency_ms=round(elapsed, 2),
                )
            except asyncio.TimeoutError:
                elapsed = (time.monotonic() - start) * 1000
                sock.close()
                # Timeout does not necessarily mean blocked for UDP
                return PortTestResult(
                    port=port,
                    protocol="udp",
                    reachable=False,
                    latency_ms=round(elapsed, 2),
                    error="Timeout (port may still be open for UDP)",
                )
        except Exception as exc:
            elapsed = (time.monotonic() - start) * 1000
            return PortTestResult(
                port=port,
                protocol="udp",
                reachable=False,
                latency_ms=round(elapsed, 2),
                error=str(exc),
            )

    # ------------------------------------------------------------------
    # WSS port fallback test
    # ------------------------------------------------------------------
    async def test_wss_connectivity(
        self, host: Optional[str] = None
    ) -> List[PortTestResult]:
        """Test WSS connectivity with the port fallback sequence.

        Tries ports 443, 8443, 7443, 5061 (all TLS).
        """
        host = host or self.default_host
        if not host:
            raise ValueError("No host specified")

        results = await asyncio.gather(
            *[self._test_tcp_port(host, port, use_tls=True) for port in WSS_PORTS]
        )
        return list(results)

    # ------------------------------------------------------------------
    # TURN connectivity test
    # ------------------------------------------------------------------
    async def test_turn_connectivity(
        self, host: Optional[str] = None
    ) -> List[PortTestResult]:
        """Test TURN server connectivity.

        Tries UDP 3478, TCP 3478, TLS 5349, TLS 443.
        """
        host = host or self.default_host
        if not host:
            raise ValueError("No host specified")

        tasks = []
        for port in TURN_UDP_PORTS:
            tasks.append(self._test_udp_port(host, port))
        for port in TURN_TCP_PORTS:
            tasks.append(self._test_tcp_port(host, port))
        for port in TURN_TLS_PORTS:
            tasks.append(self._test_tcp_port(host, port, use_tls=True))

        results = await asyncio.gather(*tasks)
        return list(results)

    # ------------------------------------------------------------------
    # RTP port range test
    # ------------------------------------------------------------------
    async def test_rtp_ports(
        self,
        host: Optional[str] = None,
        port_start: Optional[int] = None,
        port_end: Optional[int] = None,
    ) -> List[PortTestResult]:
        """Check RTP port availability by testing a sample of the range.

        For local-side testing, this checks if ports can be bound.
        For remote testing, it attempts UDP connectivity.
        """
        host = host or self.default_host
        start = port_start or self.rtp_port_start
        end = port_end or self.rtp_port_end

        results: List[PortTestResult] = []

        if not host or host in ("127.0.0.1", "localhost", "0.0.0.0"):
            # Local test: check if ports can be bound
            sample_ports = list(range(start, min(end, start + 10)))
            for port in sample_ports:
                try:
                    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
                    sock.bind(("0.0.0.0", port))
                    sock.close()
                    results.append(
                        PortTestResult(port=port, protocol="udp", reachable=True)
                    )
                except OSError as exc:
                    results.append(
                        PortTestResult(
                            port=port,
                            protocol="udp",
                            reachable=False,
                            error=str(exc),
                        )
                    )
        else:
            # Remote test: probe a sample
            sample_ports = list(range(start, min(end, start + 5)))
            tasks = [self._test_udp_port(host, p) for p in sample_ports]
            results = list(await asyncio.gather(*tasks))

        return results

    # ------------------------------------------------------------------
    # STUN binding test
    # ------------------------------------------------------------------
    @staticmethod
    def _build_stun_request(
        change_ip: bool = False, change_port: bool = False
    ) -> bytes:
        """Build a STUN Binding Request (RFC 5389)."""
        import os as _os

        tx_id = _os.urandom(12)
        msg_type = STUN_BINDING_REQUEST
        attrs = b""

        if change_ip or change_port:
            flags = 0
            if change_ip:
                flags |= 0x04
            if change_port:
                flags |= 0x02
            attr_value = struct.pack("!I", flags)
            attrs += struct.pack("!HH", STUN_ATTR_CHANGE_REQUEST, len(attr_value))
            attrs += attr_value

        msg_len = len(attrs)
        header = struct.pack(
            "!HHI", msg_type, msg_len, STUN_MAGIC_COOKIE
        ) + tx_id

        return header + attrs

    @staticmethod
    def _parse_stun_response(data: bytes) -> Tuple[str, int]:
        """Parse a STUN response and extract the mapped address.

        Returns ``(ip, port)``.
        """
        if len(data) < 20:
            raise ValueError("Response too short for STUN")

        msg_type = struct.unpack("!H", data[0:2])[0]
        if msg_type != STUN_BINDING_RESPONSE:
            raise ValueError(f"Not a Binding Response: 0x{msg_type:04x}")

        msg_len = struct.unpack("!H", data[2:4])[0]
        tx_id = data[8:20]

        offset = 20
        mapped_ip = ""
        mapped_port = 0

        while offset < 20 + msg_len:
            if offset + 4 > len(data):
                break
            attr_type, attr_len = struct.unpack("!HH", data[offset : offset + 4])
            attr_data = data[offset + 4 : offset + 4 + attr_len]

            if attr_type == STUN_ATTR_XOR_MAPPED_ADDRESS and attr_len >= 8:
                family = attr_data[1]
                xor_port = struct.unpack("!H", attr_data[2:4])[0]
                mapped_port = xor_port ^ (STUN_MAGIC_COOKIE >> 16)
                if family == 0x01:  # IPv4
                    xor_ip = struct.unpack("!I", attr_data[4:8])[0]
                    ip_int = xor_ip ^ STUN_MAGIC_COOKIE
                    mapped_ip = socket.inet_ntoa(struct.pack("!I", ip_int))

            elif attr_type == STUN_ATTR_MAPPED_ADDRESS and attr_len >= 8 and not mapped_ip:
                family = attr_data[1]
                mapped_port = struct.unpack("!H", attr_data[2:4])[0]
                if family == 0x01:
                    mapped_ip = socket.inet_ntoa(attr_data[4:8])

            # Pad to 4-byte boundary
            offset += 4 + attr_len
            if attr_len % 4:
                offset += 4 - (attr_len % 4)

        if not mapped_ip:
            raise ValueError("No mapped address in STUN response")

        return mapped_ip, mapped_port

    async def test_stun(
        self,
        server: Optional[str] = None,
        port: Optional[int] = None,
    ) -> STUNResult:
        """Perform a STUN binding request and return the mapped address."""
        server = server or self.stun_server
        port = port or self.stun_port

        loop = asyncio.get_event_loop()
        start = time.monotonic()

        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.setblocking(False)

            request = self._build_stun_request()
            await loop.sock_sendto(sock, request, (server, port))

            data = await asyncio.wait_for(
                loop.sock_recv(sock, 2048),
                timeout=STUN_TIMEOUT,
            )
            elapsed = (time.monotonic() - start) * 1000
            sock.close()

            mapped_ip, mapped_port = self._parse_stun_response(data)

            return STUNResult(
                server=server,
                port=port,
                success=True,
                mapped_address=mapped_ip,
                mapped_port=mapped_port,
                latency_ms=round(elapsed, 2),
            )
        except Exception as exc:
            elapsed = (time.monotonic() - start) * 1000
            return STUNResult(
                server=server,
                port=port,
                success=False,
                latency_ms=round(elapsed, 2),
                error=str(exc),
            )

    # ------------------------------------------------------------------
    # NAT type detection
    # ------------------------------------------------------------------
    async def detect_nat_type(
        self,
        stun_server: Optional[str] = None,
        stun_port: Optional[int] = None,
    ) -> NATDetectionResult:
        """Detect the NAT type using the classic STUN algorithm.

        Uses multiple STUN tests:
        1. Basic binding request
        2. Binding request from a different source port
        3. Compare mapped addresses to classify NAT type.

        This is a simplified version of RFC 3489 NAT detection.
        """
        server = stun_server or self.stun_server
        port = stun_port or self.stun_port

        # Test 1: basic binding request
        result1 = await self.test_stun(server, port)
        if not result1.success:
            return NATDetectionResult(
                nat_type=NATType.BLOCKED,
                details=f"STUN binding failed: {result1.error}",
            )

        ext_ip1 = result1.mapped_address
        ext_port1 = result1.mapped_port

        # Detect our local address
        try:
            local_sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            local_sock.connect((server, port))
            local_ip = local_sock.getsockname()[0]
            local_sock.close()
        except Exception:
            local_ip = ""

        # If our mapped IP equals our local IP, we might be open
        if local_ip and ext_ip1 == local_ip:
            return NATDetectionResult(
                nat_type=NATType.OPEN,
                external_ip=ext_ip1,
                external_port=ext_port1,
                details="External IP matches local IP; no NAT detected.",
            )

        # Test 2: send from a different local port to the same server
        loop = asyncio.get_event_loop()
        try:
            sock2 = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock2.setblocking(False)
            sock2.bind(("0.0.0.0", 0))  # random port

            request = self._build_stun_request()
            await loop.sock_sendto(sock2, request, (server, port))
            data2 = await asyncio.wait_for(
                loop.sock_recv(sock2, 2048),
                timeout=STUN_TIMEOUT,
            )
            local_port2 = sock2.getsockname()[1]
            sock2.close()

            ext_ip2, ext_port2 = self._parse_stun_response(data2)
        except Exception:
            # Cannot run Test 2; fall back to basic classification
            return NATDetectionResult(
                nat_type=NATType.UNKNOWN,
                external_ip=ext_ip1,
                external_port=ext_port1,
                details="Could not complete NAT type detection (test 2 failed).",
            )

        # Compare results
        if ext_ip1 != ext_ip2:
            # Different external IPs = symmetric NAT (very restrictive)
            return NATDetectionResult(
                nat_type=NATType.SYMMETRIC,
                external_ip=ext_ip1,
                external_port=ext_port1,
                details=(
                    "Different external IPs observed from different local ports. "
                    "Symmetric NAT detected. WebRTC may have connectivity issues."
                ),
            )

        # Same external IP but different external ports = port-restricted or symmetric
        if ext_ip1 == ext_ip2 and ext_port1 != ext_port2:
            return NATDetectionResult(
                nat_type=NATType.SYMMETRIC,
                external_ip=ext_ip1,
                external_port=ext_port1,
                details=(
                    "Same external IP but different mapped ports for different "
                    "source ports. Symmetric NAT detected."
                ),
            )

        # Same IP and port mapping = some form of cone NAT
        # To distinguish full cone vs restricted, we would need a STUN server
        # that supports the CHANGE-REQUEST attribute (increasingly rare).
        # We default to restricted cone as the most common case.
        return NATDetectionResult(
            nat_type=NATType.FULL_CONE,
            external_ip=ext_ip1,
            external_port=ext_port1,
            details=(
                "Consistent external IP/port mapping observed. "
                "Cone NAT detected (likely full cone or restricted cone). "
                "VoIP should work well with STUN."
            ),
        )

    # ------------------------------------------------------------------
    # Full diagnostic
    # ------------------------------------------------------------------
    async def run_full_diagnostic(
        self,
        host: Optional[str] = None,
        stun_server: Optional[str] = None,
    ) -> DiagnosticResult:
        """Run a comprehensive network diagnostic.

        Tests WSS ports, TURN ports, RTP port range, STUN binding,
        and NAT type detection.
        """
        target = host or self.default_host
        result = DiagnosticResult(host=target)

        # Run tests concurrently where possible
        tasks = {}

        if target:
            tasks["wss"] = asyncio.create_task(
                self.test_wss_connectivity(target)
            )
            tasks["turn"] = asyncio.create_task(
                self.test_turn_connectivity(target)
            )
            tasks["rtp"] = asyncio.create_task(
                self.test_rtp_ports(target)
            )

        tasks["stun"] = asyncio.create_task(
            self.test_stun(stun_server or self.stun_server)
        )
        tasks["nat"] = asyncio.create_task(
            self.detect_nat_type(stun_server or self.stun_server)
        )

        # Await all
        done = {}
        for name, task in tasks.items():
            try:
                done[name] = await task
            except Exception as exc:
                logger.error("Diagnostic test '%s' failed: %s", name, exc)
                done[name] = None

        # Populate result
        if "wss" in done and done["wss"]:
            result.wss_results = done["wss"]
            # Find first reachable WSS port
            for pr in result.wss_results:
                if pr.reachable:
                    result.recommended_wss_port = pr.port
                    break

        if "turn" in done and done["turn"]:
            result.turn_results = done["turn"]
            # Recommend a TURN config
            for pr in result.turn_results:
                if pr.reachable:
                    result.recommended_turn_config = (
                        f"turn:{target}:{pr.port}?transport={pr.protocol}"
                    )
                    break

        if "rtp" in done and done["rtp"]:
            result.rtp_results = done["rtp"]

        if "stun" in done and done["stun"]:
            result.stun_result = done["stun"]

        if "nat" in done and done["nat"]:
            result.nat_detection = done["nat"]

        # Build recommendations
        result.recommendations = self._build_recommendations(result)
        result.overall_status = self._assess_overall(result)

        return result

    # ------------------------------------------------------------------
    # Recommendation engine
    # ------------------------------------------------------------------
    @staticmethod
    def _build_recommendations(result: DiagnosticResult) -> List[str]:
        recs: List[str] = []

        # WSS
        wss_ok = any(r.reachable for r in result.wss_results)
        if not wss_ok and result.wss_results:
            recs.append(
                "No WSS port is reachable. Ensure at least one of ports "
                f"{WSS_PORTS} is open and forwarded to this server."
            )
        elif result.wss_results:
            reachable = [r.port for r in result.wss_results if r.reachable]
            if 443 not in reachable:
                recs.append(
                    "Port 443 is not reachable for WSS. Using fallback port "
                    f"{reachable[0]}. Some corporate firewalls may block non-443 "
                    "WebSocket connections."
                )

        # TURN
        turn_ok = any(r.reachable for r in result.turn_results)
        if not turn_ok and result.turn_results:
            recs.append(
                "No TURN ports are reachable. TURN relay is required for "
                "clients behind symmetric NAT. Ensure TURN server is running "
                "and ports are accessible."
            )

        # STUN
        if result.stun_result and not result.stun_result.success:
            recs.append(
                "STUN binding failed. Check that UDP is not blocked by the "
                "firewall and that the STUN server is reachable."
            )

        # NAT
        if result.nat_detection:
            if result.nat_detection.nat_type == NATType.SYMMETRIC:
                recs.append(
                    "Symmetric NAT detected. Direct peer-to-peer media will "
                    "likely fail. A TURN relay server is required for reliable "
                    "VoIP operation."
                )
            elif result.nat_detection.nat_type == NATType.BLOCKED:
                recs.append(
                    "UDP appears to be blocked. VoIP media transport requires "
                    "UDP. Check firewall rules."
                )

        # RTP
        rtp_blocked = [r for r in result.rtp_results if not r.reachable]
        if rtp_blocked and result.rtp_results:
            pct = len(rtp_blocked) / len(result.rtp_results) * 100
            if pct > 50:
                recs.append(
                    f"{len(rtp_blocked)}/{len(result.rtp_results)} sampled RTP "
                    "ports are unavailable. Ensure the RTP port range is not "
                    "in use by other services."
                )

        if not recs:
            recs.append("All connectivity tests passed. Network looks good for VoIP.")

        return recs

    @staticmethod
    def _assess_overall(result: DiagnosticResult) -> str:
        """Compute an overall health assessment."""
        issues = 0

        wss_ok = any(r.reachable for r in result.wss_results)
        if not wss_ok and result.wss_results:
            issues += 2  # Critical

        if result.stun_result and not result.stun_result.success:
            issues += 1

        if result.nat_detection:
            if result.nat_detection.nat_type in (NATType.SYMMETRIC, NATType.BLOCKED):
                issues += 1

        turn_ok = any(r.reachable for r in result.turn_results)
        if not turn_ok and result.turn_results:
            issues += 1

        if issues == 0:
            return "good"
        elif issues <= 2:
            return "degraded"
        else:
            return "failed"
