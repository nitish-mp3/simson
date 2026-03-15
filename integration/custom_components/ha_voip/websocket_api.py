"""WebSocket API for the HA VoIP integration.

Provides real-time communication between the HA frontend (or any WS client)
and the voip-engine.  Handles call event subscriptions, SDP/ICE relay for
WebRTC, and administrative queries.
"""

from __future__ import annotations

import logging
from typing import Any

import voluptuous as vol

from homeassistant.components import websocket_api
from homeassistant.core import HomeAssistant, callback
from homeassistant.helpers import config_validation as cv

from .const import (
    DATA_COORDINATOR,
    DOMAIN,
    EVENT_CALL_ANSWERED,
    EVENT_CALL_ENDED,
    EVENT_CALL_HELD,
    EVENT_CALL_RESUMED,
    EVENT_CALL_RINGING,
    EVENT_CALL_STARTED,
    EVENT_CALL_TRANSFERRED,
    EVENT_DTMF_RECEIVED,
    EVENT_ENGINE_STATE_CHANGED,
    EVENT_REGISTRATION_CHANGED,
)
from .coordinator import VoipDataUpdateCoordinator

_LOGGER = logging.getLogger(__name__)

# Events that are relayed to WS subscribers
_SUBSCRIBABLE_EVENTS: list[str] = [
    EVENT_CALL_STARTED,
    EVENT_CALL_RINGING,
    EVENT_CALL_ANSWERED,
    EVENT_CALL_ENDED,
    EVENT_CALL_HELD,
    EVENT_CALL_RESUMED,
    EVENT_CALL_TRANSFERRED,
    EVENT_REGISTRATION_CHANGED,
    EVENT_ENGINE_STATE_CHANGED,
    EVENT_DTMF_RECEIVED,
]


def _get_coordinator(hass: HomeAssistant) -> VoipDataUpdateCoordinator:
    """Return the coordinator or raise."""
    data: dict[str, Any] | None = hass.data.get(DOMAIN)
    if data is None:
        raise vol.Invalid("HA VoIP integration is not loaded")
    coord: VoipDataUpdateCoordinator | None = data.get(DATA_COORDINATOR)
    if coord is None:
        raise vol.Invalid("HA VoIP coordinator is not available")
    return coord


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


def async_register_websocket_api(hass: HomeAssistant) -> None:
    """Register all WebSocket commands for HA VoIP."""
    websocket_api.async_register_command(hass, ws_subscribe_events)
    websocket_api.async_register_command(hass, ws_send_sdp)
    websocket_api.async_register_command(hass, ws_send_ice_candidate)
    websocket_api.async_register_command(hass, ws_get_extensions)
    websocket_api.async_register_command(hass, ws_get_call_history)
    websocket_api.async_register_command(hass, ws_network_diagnostics)
    websocket_api.async_register_command(hass, ws_get_config)
    _LOGGER.debug("Registered VoIP WebSocket API commands")


# ---------------------------------------------------------------------------
# voip/subscribe_events
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/subscribe_events",
        vol.Optional("event_types", default=[]): vol.All(
            cv.ensure_list, [vol.In(_SUBSCRIBABLE_EVENTS)]
        ),
    }
)
@websocket_api.async_response
async def ws_subscribe_events(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Subscribe to VoIP events (call state changes, registrations, etc.)."""
    requested = msg.get("event_types") or _SUBSCRIBABLE_EVENTS

    @callback
    def _forward_event(event: Any) -> None:
        """Forward an HA event to the WS subscriber."""
        connection.send_message(
            websocket_api.event_message(
                msg["id"],
                {
                    "event_type": event.event_type,
                    "data": event.data,
                },
            )
        )

    unsub_list: list[Any] = []
    for event_type in requested:
        unsub = hass.bus.async_listen(event_type, _forward_event)
        unsub_list.append(unsub)

    @callback
    def _cancel_subscription() -> None:
        for unsub in unsub_list:
            unsub()

    connection.subscriptions[msg["id"]] = _cancel_subscription
    connection.send_result(msg["id"])


# ---------------------------------------------------------------------------
# voip/send_sdp
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/send_sdp",
        vol.Required("call_id"): cv.string,
        vol.Required("sdp"): cv.string,
        vol.Required("sdp_type"): vol.In(["offer", "answer"]),
    }
)
@websocket_api.async_response
async def ws_send_sdp(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Relay an SDP offer/answer from the browser to the engine."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    call_id: str = msg["call_id"]
    sdp: str = msg["sdp"]
    sdp_type: str = msg["sdp_type"]

    _LOGGER.debug(
        "WS send_sdp: call_id=%s, type=%s, len=%d",
        call_id,
        sdp_type,
        len(sdp),
    )

    try:
        result = await coordinator.grpc_client.relay_sdp(
            call_id, sdp, sdp_type
        )
        connection.send_result(msg["id"], result)
    except ConnectionError as exc:
        connection.send_error(msg["id"], "engine_error", str(exc))


# ---------------------------------------------------------------------------
# voip/send_ice_candidate
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/send_ice_candidate",
        vol.Required("call_id"): cv.string,
        vol.Required("candidate"): cv.string,
        vol.Optional("sdp_mid", default=""): cv.string,
        vol.Optional("sdp_mline_index", default=0): vol.Coerce(int),
    }
)
@websocket_api.async_response
async def ws_send_ice_candidate(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Relay an ICE candidate from the browser to the engine."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    success = await coordinator.grpc_client.relay_ice_candidate(
        call_id=msg["call_id"],
        candidate=msg["candidate"],
        sdp_mid=msg.get("sdp_mid", ""),
        sdp_mline_index=msg.get("sdp_mline_index", 0),
    )

    if success:
        connection.send_result(msg["id"])
    else:
        connection.send_error(
            msg["id"], "engine_error", "Failed to relay ICE candidate"
        )


# ---------------------------------------------------------------------------
# voip/get_extensions
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/get_extensions",
    }
)
@websocket_api.async_response
async def ws_get_extensions(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return the list of configured extensions and their status."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    data = coordinator.data
    extensions = []
    if data:
        for ext in data.extensions.values():
            extensions.append(
                {
                    "number": ext.number,
                    "display_name": ext.display_name,
                    "registered": ext.registered,
                    "user_agent": ext.user_agent,
                    "contact_uri": ext.contact_uri,
                    "last_seen": ext.last_seen,
                }
            )

    connection.send_result(msg["id"], {"extensions": extensions})


# ---------------------------------------------------------------------------
# voip/get_call_history
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/get_call_history",
        vol.Optional("limit", default=50): vol.All(
            vol.Coerce(int), vol.Range(min=1, max=500)
        ),
    }
)
@websocket_api.async_response
async def ws_get_call_history(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return recent call history."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    limit: int = msg.get("limit", 50)

    try:
        history = await coordinator.grpc_client.get_call_history(limit=limit)
        connection.send_result(msg["id"], {"history": history})
    except ConnectionError as exc:
        connection.send_error(msg["id"], "engine_error", str(exc))


# ---------------------------------------------------------------------------
# voip/network_diagnostics
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/network_diagnostics",
    }
)
@websocket_api.async_response
async def ws_network_diagnostics(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Run connectivity tests and return the results."""
    # Defer to diagnostics module
    from .diagnostics import async_run_network_diagnostics  # noqa: PLC0415

    try:
        results = await async_run_network_diagnostics(hass)
        connection.send_result(msg["id"], results)
    except Exception as exc:  # noqa: BLE001
        _LOGGER.exception("Network diagnostics failed")
        connection.send_error(msg["id"], "diagnostics_error", str(exc))


# ---------------------------------------------------------------------------
# voip/get_config
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/get_config",
    }
)
@websocket_api.async_response
async def ws_get_config(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return the current VoIP configuration (sanitized) for the frontend."""
    domain_data: dict[str, Any] | None = hass.data.get(DOMAIN)
    if domain_data is None:
        connection.send_error(msg["id"], "not_ready", "Integration not loaded")
        return

    # Get config entry
    entries = hass.config_entries.async_entries(DOMAIN)
    if not entries:
        connection.send_error(msg["id"], "not_configured", "No config entry")
        return

    entry = entries[0]
    # Build a sanitized copy (strip passwords/secrets)
    sanitized: dict[str, Any] = {}
    sensitive_keys = {"turn_password", "key_path"}

    for key, value in entry.data.items():
        if key in sensitive_keys:
            sanitized[key] = "***" if value else ""
        elif key == "extensions":
            # Strip passwords from extension list
            sanitized[key] = [
                {k: ("***" if k == "password" else v) for k, v in ext.items()}
                for ext in (value if isinstance(value, list) else [])
            ]
        else:
            sanitized[key] = value

    # Merge options on top
    for key, value in entry.options.items():
        if key in sensitive_keys:
            sanitized[key] = "***" if value else ""
        else:
            sanitized[key] = value

    connection.send_result(msg["id"], {"config": sanitized})
