"""Diagnostics support for the HA VoIP integration.

Provides:
- ``async_get_config_entry_diagnostics`` -- HA diagnostics panel data
- ``async_run_network_diagnostics``      -- on-demand connectivity tests
"""

from __future__ import annotations

import asyncio
import logging
import socket
import ssl
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant

from .const import (
    CONF_CERT_MODE,
    CONF_CERT_PATH,
    CONF_ENGINE_HOST,
    CONF_EXTERNAL_HOST,
    CONF_GRPC_PORT,
    CONF_KEY_PATH,
    CONF_SIP_PORT,
    CONF_STUN_SERVER,
    CONF_TURN_SERVER,
    CONF_WS_PORT,
    DATA_COORDINATOR,
    DATA_ENGINE_MANAGER,
    DEFAULT_ENGINE_HOST,
    DEFAULT_GRPC_PORT,
    DEFAULT_SIP_PORT,
    DEFAULT_STUN_SERVER,
    DEFAULT_WS_PORT,
    DOMAIN,
)

_LOGGER = logging.getLogger(__name__)

# Keys whose values must be redacted in diagnostics output
_SENSITIVE_KEYS = frozenset(
    {
        "turn_password",
        "key_path",
        "cert_path",
        "acme_email",
        "password",
    }
)


def _redact(data: Any) -> Any:
    """Recursively redact sensitive values."""
    if isinstance(data, dict):
        return {
            k: ("**REDACTED**" if k in _SENSITIVE_KEYS else _redact(v))
            for k, v in data.items()
        }
    if isinstance(data, list):
        return [_redact(item) for item in data]
    return data


# ---------------------------------------------------------------------------
# HA diagnostics panel hook
# ---------------------------------------------------------------------------


async def async_get_config_entry_diagnostics(
    hass: HomeAssistant, entry: ConfigEntry
) -> dict[str, Any]:
    """Return diagnostics data for the config entry.

    Called by HA when the user clicks "Download Diagnostics" in the UI.
    """
    domain_data: dict[str, Any] = hass.data.get(DOMAIN, {})

    # Engine manager state
    engine_mgr = domain_data.get(DATA_ENGINE_MANAGER)
    engine_info: dict[str, Any] = {}
    if engine_mgr is not None:
        engine_info = {
            "state": engine_mgr.state,
            "is_running": engine_mgr.is_running,
        }

    # Coordinator data snapshot
    coordinator = domain_data.get(DATA_COORDINATOR)
    coord_data: dict[str, Any] = {}
    if coordinator is not None and coordinator.data is not None:
        d = coordinator.data
        coord_data = {
            "engine": {
                "state": d.engine.state,
                "uptime_seconds": d.engine.uptime_seconds,
                "version": d.engine.version,
                "active_call_count": d.engine.active_call_count,
                "total_calls_handled": d.engine.total_calls_handled,
                "registered_extension_count": d.engine.registered_extension_count,
                "cpu_usage_percent": d.engine.cpu_usage_percent,
                "memory_usage_mb": d.engine.memory_usage_mb,
            },
            "active_calls": len(d.calls),
            "registered_extensions": [
                {"number": e.number, "registered": e.registered}
                for e in d.extensions.values()
            ],
            "call_history_count": len(d.call_history),
        }

    # Network diagnostics
    network = await async_run_network_diagnostics(hass)

    return {
        "timestamp": datetime.now(tz=timezone.utc).isoformat(),
        "config_entry": _redact(dict(entry.data)),
        "options": _redact(dict(entry.options)),
        "engine_manager": engine_info,
        "coordinator_data": coord_data,
        "network_diagnostics": network,
    }


# ---------------------------------------------------------------------------
# On-demand network diagnostics
# ---------------------------------------------------------------------------


async def async_run_network_diagnostics(
    hass: HomeAssistant,
) -> dict[str, Any]:
    """Run connectivity tests and return a results dict.

    Tests:
    1. Engine health endpoint reachability (HTTP 8080)
    2. SIP port availability
    3. WebSocket port availability
    4. STUN server connectivity
    5. TURN server connectivity (if configured)
    6. Certificate file validation (if manual)
    """
    results: dict[str, Any] = {}
    entries = hass.config_entries.async_entries(DOMAIN)
    if not entries:
        return {"error": "No config entry found"}

    data = entries[0].data
    opts = entries[0].options

    def _get(key: str, default: Any = None) -> Any:
        return opts.get(key, data.get(key, default))

    # 1. Engine health endpoint (port 8080)
    engine_host = _get(CONF_ENGINE_HOST, DEFAULT_ENGINE_HOST)
    results["health_endpoint"] = await _check_tcp_port(engine_host, 8080)

    # 2. SIP port
    sip_port = _get(CONF_SIP_PORT, DEFAULT_SIP_PORT)
    results["sip_port_available"] = await hass.async_add_executor_job(
        _is_port_available, sip_port
    )

    # 3. WS port
    ws_port = _get(CONF_WS_PORT, DEFAULT_WS_PORT)
    results["ws_port_available"] = await hass.async_add_executor_job(
        _is_port_available, ws_port
    )

    # 4. STUN
    stun_server = _get(CONF_STUN_SERVER, DEFAULT_STUN_SERVER)
    if stun_server:
        results["stun"] = await _check_stun(stun_server)

    # 5. TURN
    turn_server = _get(CONF_TURN_SERVER, "")
    if turn_server:
        results["turn"] = await _check_turn(turn_server)
    else:
        results["turn"] = {"configured": False}

    # 6. Certificate
    cert_mode = _get(CONF_CERT_MODE, "self_signed")
    results["certificate"] = await hass.async_add_executor_job(
        _check_certificate, cert_mode, _get(CONF_CERT_PATH, ""), _get(CONF_KEY_PATH, "")
    )

    return results


# ---------------------------------------------------------------------------
# Individual test helpers
# ---------------------------------------------------------------------------


async def _check_tcp_port(host: str, port: int) -> dict[str, Any]:
    """Try to open a TCP connection."""
    try:
        _, writer = await asyncio.wait_for(
            asyncio.open_connection(host, port), timeout=5.0
        )
        writer.close()
        await writer.wait_closed()
        return {"reachable": True, "host": host, "port": port}
    except (OSError, asyncio.TimeoutError) as exc:
        return {"reachable": False, "host": host, "port": port, "error": str(exc)}


def _is_port_available(port: int) -> dict[str, Any]:
    """Check if a port is free to bind (UDP + TCP)."""
    for proto, kind in ((socket.SOCK_STREAM, "tcp"), (socket.SOCK_DGRAM, "udp")):
        try:
            s = socket.socket(socket.AF_INET, proto)
            s.settimeout(0.5)
            s.bind(("0.0.0.0", port))
            s.close()
        except OSError as exc:
            return {"available": False, "port": port, "protocol": kind, "error": str(exc)}
    return {"available": True, "port": port}


async def _check_stun(server: str) -> dict[str, Any]:
    """Basic STUN connectivity check (just resolve + UDP reachability)."""
    # Parse stun:host:port
    parts = server.replace("stun:", "").split(":")
    host = parts[0]
    port = int(parts[1]) if len(parts) > 1 else 3478

    try:
        # Resolve hostname
        infos = await asyncio.get_event_loop().getaddrinfo(
            host, port, family=socket.AF_INET, type=socket.SOCK_DGRAM
        )
        if not infos:
            return {"reachable": False, "host": host, "port": port, "error": "DNS resolution failed"}

        resolved_ip = infos[0][4][0]

        # Try a quick UDP round-trip (STUN Binding Request)
        transport, _ = await asyncio.wait_for(
            asyncio.get_event_loop().create_datagram_endpoint(
                asyncio.DatagramProtocol, remote_addr=(resolved_ip, port)
            ),
            timeout=5.0,
        )
        transport.close()
        return {"reachable": True, "host": host, "port": port, "resolved_ip": resolved_ip}
    except Exception as exc:  # noqa: BLE001
        return {"reachable": False, "host": host, "port": port, "error": str(exc)}


async def _check_turn(server: str) -> dict[str, Any]:
    """Basic TURN server reachability check."""
    parts = server.replace("turn:", "").replace("turns:", "").split(":")
    host = parts[0]
    port = int(parts[1]) if len(parts) > 1 else 3478

    try:
        _, writer = await asyncio.wait_for(
            asyncio.open_connection(host, port), timeout=5.0
        )
        writer.close()
        await writer.wait_closed()
        return {"configured": True, "reachable": True, "host": host, "port": port}
    except (OSError, asyncio.TimeoutError) as exc:
        return {"configured": True, "reachable": False, "host": host, "port": port, "error": str(exc)}


def _check_certificate(
    mode: str, cert_path: str, key_path: str
) -> dict[str, Any]:
    """Validate certificate files (manual mode) or report mode."""
    result: dict[str, Any] = {"mode": mode}

    if mode != "manual":
        result["valid"] = True
        result["note"] = f"Certificate mode is '{mode}'; no file validation needed."
        return result

    # Check cert file
    if not cert_path:
        result["valid"] = False
        result["error"] = "Certificate path is empty"
        return result

    cert_file = Path(cert_path)
    if not cert_file.is_file():
        result["valid"] = False
        result["error"] = f"Certificate file not found: {cert_path}"
        return result

    # Check key file
    if not key_path:
        result["valid"] = False
        result["error"] = "Key path is empty"
        return result

    key_file = Path(key_path)
    if not key_file.is_file():
        result["valid"] = False
        result["error"] = f"Key file not found: {key_path}"
        return result

    # Try loading the cert + key
    try:
        ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        ctx.load_cert_chain(cert_path, key_path)
        result["valid"] = True
    except ssl.SSLError as exc:
        result["valid"] = False
        result["error"] = f"SSL error loading cert/key: {exc}"
    except Exception as exc:  # noqa: BLE001
        result["valid"] = False
        result["error"] = str(exc)

    return result
