"""WebSocket API for the HA VoIP integration.

Provides real-time communication between the HA frontend card and the
voip-engine.  All command types match exactly what voip-card.ts sends.
"""

from __future__ import annotations

import logging
from typing import Any

import voluptuous as vol

from homeassistant.components import websocket_api
from homeassistant.core import HomeAssistant, callback
from homeassistant.helpers import config_validation as cv

from .const import (
    CALL_STATE_RINGING,
    CALL_STATE_ON_HOLD,
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
from .coordinator import CallInfo, ExtensionInfo, VoipDataUpdateCoordinator

_LOGGER = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

_CALL_STATE_MAP: dict[str, str] = {
    "idle": "idle",
    "ringing": "ringing",
    "dialing": "dialing",
    "in_call": "connected",
    "on_hold": "on_hold",
    "transferring": "transferring",
    "ended": "ended",
}


def _call_to_ws(call: CallInfo) -> dict[str, Any]:
    """Convert a CallInfo dataclass to the card's CallState object shape."""
    direction = "outbound" if call.from_extension else "inbound"
    remote = call.caller_id if direction == "inbound" else call.callee_id
    return {
        "id": call.call_id,
        "state": _CALL_STATE_MAP.get(call.state, call.state),
        "direction": direction,
        "remoteNumber": remote or "",
        "remoteName": None,
        "startTime": int(call.start_time * 1000) if call.start_time else None,
        "connectTime": int(call.answer_time * 1000) if call.answer_time else None,
        "isMuted": call.is_muted,
        "isOnHold": call.state == CALL_STATE_ON_HOLD,
        "isRecording": call.is_recording,
        "isSpeaker": False,
        "duration": int(call.duration),
    }


def _ext_to_ws(ext: ExtensionInfo) -> dict[str, Any]:
    """Convert an ExtensionInfo to the card's Extension object shape."""
    return {
        "id": ext.number,
        "number": ext.number,
        "name": ext.display_name or ext.number,
        "status": "available" if ext.registered else "offline",
        "registeredAt": str(ext.last_seen) if ext.last_seen else None,
    }


def _get_coordinator(hass: HomeAssistant) -> VoipDataUpdateCoordinator:
    """Return the coordinator or raise a descriptive error."""
    data: dict[str, Any] | None = hass.data.get(DOMAIN)
    if data is None:
        raise vol.Invalid("HA VoIP integration is not loaded")
    coord: VoipDataUpdateCoordinator | None = data.get(DATA_COORDINATOR)
    if coord is None:
        raise vol.Invalid("HA VoIP coordinator is not available")
    return coord


def _first_extension(coordinator: VoipDataUpdateCoordinator) -> str | None:
    """Return the number of the first registered extension, or any extension."""
    if not coordinator.data:
        return None
    for ext in coordinator.data.extensions.values():
        if ext.registered:
            return ext.number
    # Fall back to the first configured extension even if unregistered
    for ext in coordinator.data.extensions.values():
        return ext.number
    return None


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


def async_register_websocket_api(hass: HomeAssistant) -> None:
    """Register all WebSocket commands for HA VoIP."""
    cmds = [
        # Card-facing commands (types that voip-card.ts sends)
        ws_subscribe,
        ws_extensions,
        ws_history,
        ws_call,
        ws_answer,
        ws_hangup,
        ws_mute,
        ws_hold,
        ws_record,
        ws_transfer,
        ws_dtmf,
        ws_webrtc_offer,
        ws_webrtc_answer,
        ws_webrtc_candidate,
        ws_diagnostics,
        ws_onboarding,
        # Legacy / admin commands kept for backward compatibility
        ws_subscribe_events,
        ws_send_sdp,
        ws_send_ice_candidate,
        ws_get_extensions,
        ws_get_call_history,
        ws_network_diagnostics,
        ws_get_config,
    ]
    for cmd in cmds:
        websocket_api.async_register_command(hass, cmd)
    _LOGGER.debug("Registered %d VoIP WebSocket API commands", len(cmds))


# ===========================================================================
# CARD-FACING COMMANDS  (match exactly what voip-card.ts sends)
# ===========================================================================

# ---------------------------------------------------------------------------
# voip/subscribe  — real-time event subscription
# ---------------------------------------------------------------------------


@websocket_api.websocket_command({vol.Required("type"): "voip/subscribe"})
@websocket_api.async_response
async def ws_subscribe(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Subscribe to all VoIP events. Sends events in the card's VoipEvent format."""

    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    def _build_call_state_event(event_data: dict[str, Any], state_override: str | None = None) -> dict[str, Any] | None:
        """Build a call_state event dict from HA event data."""
        call_id: str = event_data.get("call_id", "")
        if not call_id:
            return None
        # Prefer the coordinator's cached CallInfo (most up-to-date)
        if coordinator.data and call_id in coordinator.data.calls:
            call_ws = _call_to_ws(coordinator.data.calls[call_id])
        else:
            # Build a minimal object from raw event data
            direction = event_data.get("direction", "inbound")
            call_ws = {
                "id": call_id,
                "state": state_override or event_data.get("state", "idle"),
                "direction": direction,
                "remoteNumber": event_data.get("caller_id") or event_data.get("callee_id") or "",
                "remoteName": event_data.get("caller_name"),
                "startTime": None,
                "connectTime": None,
                "isMuted": False,
                "isOnHold": False,
                "isRecording": False,
                "isSpeaker": False,
                "duration": 0,
            }
        if state_override:
            call_ws["state"] = state_override
        return call_ws

    @callback
    def _on_call_started(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "dialing")
        if call_ws:
            connection.send_message(
                websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
            )

    @callback
    def _on_call_ringing(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "ringing")
        if not call_ws:
            return
        connection.send_message(
            websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
        )
        # Also send incoming_call event so the popup appears
        if call_ws.get("direction") == "inbound":
            connection.send_message(
                websocket_api.event_message(
                    msg["id"],
                    {
                        "event": "incoming_call",
                        "data": {
                            "call_id": call_ws["id"],
                            "caller_number": call_ws["remoteNumber"],
                            "caller_name": call_ws.get("remoteName"),
                        },
                    },
                )
            )

    @callback
    def _on_call_answered(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "connected")
        if call_ws:
            connection.send_message(
                websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
            )

    @callback
    def _on_call_ended(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "ended")
        if call_ws:
            connection.send_message(
                websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
            )

    @callback
    def _on_call_held(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "on_hold")
        if call_ws:
            connection.send_message(
                websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
            )

    @callback
    def _on_call_resumed(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "connected")
        if call_ws:
            connection.send_message(
                websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
            )

    @callback
    def _on_call_transferred(event: Any) -> None:
        call_ws = _build_call_state_event(event.data, "transferring")
        if call_ws:
            connection.send_message(
                websocket_api.event_message(msg["id"], {"event": "call_state", "data": call_ws})
            )

    @callback
    def _on_registration_changed(event: Any) -> None:
        if not coordinator.data:
            return
        exts = [_ext_to_ws(e) for e in coordinator.data.extensions.values()]
        connection.send_message(
            websocket_api.event_message(msg["id"], {"event": "extensions", "data": exts})
        )

    unsub_list = [
        hass.bus.async_listen(EVENT_CALL_STARTED, _on_call_started),
        hass.bus.async_listen(EVENT_CALL_RINGING, _on_call_ringing),
        hass.bus.async_listen(EVENT_CALL_ANSWERED, _on_call_answered),
        hass.bus.async_listen(EVENT_CALL_ENDED, _on_call_ended),
        hass.bus.async_listen(EVENT_CALL_HELD, _on_call_held),
        hass.bus.async_listen(EVENT_CALL_RESUMED, _on_call_resumed),
        hass.bus.async_listen(EVENT_CALL_TRANSFERRED, _on_call_transferred),
        hass.bus.async_listen(EVENT_REGISTRATION_CHANGED, _on_registration_changed),
    ]

    @callback
    def _cancel() -> None:
        for unsub in unsub_list:
            unsub()

    connection.subscriptions[msg["id"]] = _cancel
    connection.send_result(msg["id"])


# ---------------------------------------------------------------------------
# voip/extensions  — list extensions with card-compatible shape
# ---------------------------------------------------------------------------


@websocket_api.websocket_command({vol.Required("type"): "voip/extensions"})
@websocket_api.async_response
async def ws_extensions(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return extensions in the card's Extension[] format."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    exts: list[dict[str, Any]] = []
    if coordinator.data:
        exts = [_ext_to_ws(e) for e in coordinator.data.extensions.values()]

    connection.send_result(msg["id"], exts)


# ---------------------------------------------------------------------------
# voip/history  — call history with card-compatible shape
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/history",
        vol.Optional("limit", default=50): vol.All(vol.Coerce(int), vol.Range(min=1, max=500)),
    }
)
@websocket_api.async_response
async def ws_history(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return call history in the card's CallHistoryEntry[] format."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    try:
        raw = await coordinator.engine_client.get_call_history(limit=msg.get("limit", 50))
        connection.send_result(msg["id"], raw if isinstance(raw, list) else [])
    except ConnectionError as exc:
        connection.send_error(msg["id"], "engine_error", str(exc))


# ---------------------------------------------------------------------------
# voip/call  — initiate an outbound call
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/call",
        vol.Required("number"): cv.string,
        vol.Optional("from_extension"): cv.string,
    }
)
@websocket_api.async_response
async def ws_call(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Initiate an outbound call. Returns {call_id}."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    if not coordinator.engine_client.connected:
        connection.send_error(msg["id"], "engine_offline", "VoIP engine is not connected")
        return

    from_ext = msg.get("from_extension") or _first_extension(coordinator)
    if not from_ext:
        connection.send_error(msg["id"], "no_extension", "No registered extension available")
        return

    try:
        result = await coordinator.engine_client.make_call(
            target=msg["number"],
            from_extension=from_ext,
        )
        connection.send_result(msg["id"], {"call_id": result.get("call_id", "")})
    except ConnectionError as exc:
        connection.send_error(msg["id"], "engine_error", str(exc))


# ---------------------------------------------------------------------------
# voip/answer  — answer an incoming call
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/answer",
        vol.Required("call_id"): cv.string,
    }
)
@websocket_api.async_response
async def ws_answer(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Answer an incoming call."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.answer_call(msg["call_id"])
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to answer call")


# ---------------------------------------------------------------------------
# voip/hangup  — hang up a call
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/hangup",
        vol.Required("call_id"): cv.string,
    }
)
@websocket_api.async_response
async def ws_hangup(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Hang up a call."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.hangup_call(msg["call_id"])
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to hang up call")


# ---------------------------------------------------------------------------
# voip/mute  — mute / unmute
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/mute",
        vol.Required("call_id"): cv.string,
        vol.Optional("mute", default=True): cv.boolean,
    }
)
@websocket_api.async_response
async def ws_mute(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Mute or unmute a call."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.toggle_mute(msg["call_id"])
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to toggle mute")


# ---------------------------------------------------------------------------
# voip/hold  — hold / resume
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/hold",
        vol.Required("call_id"): cv.string,
        vol.Optional("hold", default=True): cv.boolean,
    }
)
@websocket_api.async_response
async def ws_hold(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Hold or resume a call."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.hold_call(msg["call_id"], msg.get("hold", True))
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to toggle hold")


# ---------------------------------------------------------------------------
# voip/record  — toggle recording
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/record",
        vol.Required("call_id"): cv.string,
        vol.Optional("record", default=True): cv.boolean,
    }
)
@websocket_api.async_response
async def ws_record(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Toggle call recording."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.toggle_recording(msg["call_id"])
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to toggle recording")


# ---------------------------------------------------------------------------
# voip/transfer  — transfer a call
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/transfer",
        vol.Required("call_id"): cv.string,
        vol.Required("target"): cv.string,
        vol.Optional("blind", default=True): cv.boolean,
    }
)
@websocket_api.async_response
async def ws_transfer(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Transfer a call."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.transfer_call(
        msg["call_id"], msg["target"]
    )
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to transfer call")


# ---------------------------------------------------------------------------
# voip/dtmf  — send DTMF digit(s)
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/dtmf",
        vol.Required("call_id"): cv.string,
        vol.Required("digit"): vol.All(cv.string, vol.Match(r"^[0-9A-D*#]+$")),
    }
)
@websocket_api.async_response
async def ws_dtmf(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Send DTMF digit(s) on a call."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.send_dtmf(msg["call_id"], msg["digit"])
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to send DTMF")


# ---------------------------------------------------------------------------
# voip/webrtc_offer  — relay SDP offer from browser to engine
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/webrtc_offer",
        vol.Required("call_id"): cv.string,
        vol.Required("sdp"): cv.string,
    }
)
@websocket_api.async_response
async def ws_webrtc_offer(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Relay a WebRTC SDP offer from the browser to the engine."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    result = await coordinator.engine_client.relay_sdp(msg["call_id"], msg["sdp"], "offer")
    connection.send_result(msg["id"], result)


# ---------------------------------------------------------------------------
# voip/webrtc_answer  — relay SDP answer from browser to engine
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/webrtc_answer",
        vol.Required("call_id"): cv.string,
        vol.Required("sdp"): cv.string,
    }
)
@websocket_api.async_response
async def ws_webrtc_answer(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Relay a WebRTC SDP answer from the browser to the engine."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    result = await coordinator.engine_client.relay_sdp(msg["call_id"], msg["sdp"], "answer")
    connection.send_result(msg["id"], result)


# ---------------------------------------------------------------------------
# voip/webrtc_candidate  — relay ICE candidate
# ---------------------------------------------------------------------------


@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/webrtc_candidate",
        vol.Required("call_id"): cv.string,
        vol.Required("candidate"): dict,
    }
)
@websocket_api.async_response
async def ws_webrtc_candidate(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Relay a WebRTC ICE candidate from the browser to the engine."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    cand: dict[str, Any] = msg["candidate"]
    ok = await coordinator.engine_client.relay_ice_candidate(
        call_id=msg["call_id"],
        candidate=cand.get("candidate", ""),
        sdp_mid=cand.get("sdpMid", ""),
        sdp_mline_index=cand.get("sdpMLineIndex", 0),
    )
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to relay ICE candidate")


# ---------------------------------------------------------------------------
# voip/diagnostics  — run network diagnostics
# ---------------------------------------------------------------------------


@websocket_api.websocket_command({vol.Required("type"): "voip/diagnostics"})
@websocket_api.async_response
async def ws_diagnostics(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Run connectivity diagnostics and return results."""
    from .diagnostics import async_run_network_diagnostics  # noqa: PLC0415

    try:
        results = await async_run_network_diagnostics(hass)
        connection.send_result(msg["id"], results)
    except Exception as exc:  # noqa: BLE001
        _LOGGER.exception("Network diagnostics failed")
        connection.send_error(msg["id"], "diagnostics_error", str(exc))


# ---------------------------------------------------------------------------
# voip/onboarding  — return setup status for onboarding wizard
# ---------------------------------------------------------------------------


@websocket_api.websocket_command({vol.Required("type"): "voip/onboarding"})
@websocket_api.async_response
async def ws_onboarding(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return onboarding / setup status."""
    entries = hass.config_entries.async_entries(DOMAIN)
    if not entries:
        connection.send_result(msg["id"], {"configured": False, "extensions": []})
        return

    entry = entries[0]
    raw_extensions = entry.data.get("extensions", [])
    extensions = [
        {"number": e.get("number", ""), "name": e.get("name", "")}
        for e in (raw_extensions if isinstance(raw_extensions, list) else [])
    ]
    connection.send_result(
        msg["id"],
        {
            "configured": True,
            "engine_mode": entry.data.get("engine_mode", "local"),
            "extensions": extensions,
            "options": dict(entry.options),
        },
    )


# ===========================================================================
# LEGACY / ADMIN COMMANDS  (kept for backward compatibility)
# ===========================================================================

# ---------------------------------------------------------------------------
# voip/subscribe_events  (legacy — use voip/subscribe instead)
# ---------------------------------------------------------------------------

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
    """Subscribe to raw HA VoIP events (admin, legacy)."""
    requested = msg.get("event_types") or _SUBSCRIBABLE_EVENTS

    @callback
    def _forward(event: Any) -> None:
        connection.send_message(
            websocket_api.event_message(
                msg["id"],
                {"event_type": event.event_type, "data": event.data},
            )
        )

    unsub_list = [hass.bus.async_listen(et, _forward) for et in requested]

    @callback
    def _cancel() -> None:
        for u in unsub_list:
            u()

    connection.subscriptions[msg["id"]] = _cancel
    connection.send_result(msg["id"])


# ---------------------------------------------------------------------------
# voip/send_sdp  (legacy)
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
    """Relay SDP to engine (legacy)."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    result = await coordinator.engine_client.relay_sdp(
        msg["call_id"], msg["sdp"], msg["sdp_type"]
    )
    connection.send_result(msg["id"], result)


# ---------------------------------------------------------------------------
# voip/send_ice_candidate  (legacy)
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
    """Relay ICE candidate to engine (legacy)."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    ok = await coordinator.engine_client.relay_ice_candidate(
        call_id=msg["call_id"],
        candidate=msg["candidate"],
        sdp_mid=msg.get("sdp_mid", ""),
        sdp_mline_index=msg.get("sdp_mline_index", 0),
    )
    if ok:
        connection.send_result(msg["id"])
    else:
        connection.send_error(msg["id"], "engine_error", "Failed to relay ICE candidate")


# ---------------------------------------------------------------------------
# voip/get_extensions  (legacy)
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command({vol.Required("type"): "voip/get_extensions"})
@websocket_api.async_response
async def ws_get_extensions(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return extensions list (legacy, wrapped format)."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    extensions = []
    if coordinator.data:
        for ext in coordinator.data.extensions.values():
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
# voip/get_call_history  (legacy)
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command(
    {
        vol.Required("type"): "voip/get_call_history",
        vol.Optional("limit", default=50): vol.All(vol.Coerce(int), vol.Range(min=1, max=500)),
    }
)
@websocket_api.async_response
async def ws_get_call_history(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return call history (legacy, wrapped format)."""
    try:
        coordinator = _get_coordinator(hass)
    except vol.Invalid as exc:
        connection.send_error(msg["id"], "not_ready", str(exc))
        return

    try:
        history = await coordinator.engine_client.get_call_history(limit=msg.get("limit", 50))
        connection.send_result(msg["id"], {"history": history})
    except ConnectionError as exc:
        connection.send_error(msg["id"], "engine_error", str(exc))


# ---------------------------------------------------------------------------
# voip/network_diagnostics  (legacy)
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command({vol.Required("type"): "voip/network_diagnostics"})
@websocket_api.async_response
async def ws_network_diagnostics(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Run network diagnostics (legacy)."""
    from .diagnostics import async_run_network_diagnostics  # noqa: PLC0415

    try:
        results = await async_run_network_diagnostics(hass)
        connection.send_result(msg["id"], results)
    except Exception as exc:  # noqa: BLE001
        _LOGGER.exception("Network diagnostics failed")
        connection.send_error(msg["id"], "diagnostics_error", str(exc))


# ---------------------------------------------------------------------------
# voip/get_config  (legacy)
# ---------------------------------------------------------------------------


@websocket_api.require_admin
@websocket_api.websocket_command({vol.Required("type"): "voip/get_config"})
@websocket_api.async_response
async def ws_get_config(
    hass: HomeAssistant,
    connection: websocket_api.ActiveConnection,
    msg: dict[str, Any],
) -> None:
    """Return sanitized integration config (legacy)."""
    domain_data: dict[str, Any] | None = hass.data.get(DOMAIN)
    if domain_data is None:
        connection.send_error(msg["id"], "not_ready", "Integration not loaded")
        return

    entries = hass.config_entries.async_entries(DOMAIN)
    if not entries:
        connection.send_error(msg["id"], "not_configured", "No config entry")
        return

    entry = entries[0]
    sensitive_keys = {"turn_password", "key_path"}
    sanitized: dict[str, Any] = {}

    for key, value in entry.data.items():
        if key in sensitive_keys:
            sanitized[key] = "***" if value else ""
        elif key == "extensions":
            sanitized[key] = [
                {k: ("***" if k == "password" else v) for k, v in ext.items()}
                for ext in (value if isinstance(value, list) else [])
            ]
        else:
            sanitized[key] = value

    for key, value in entry.options.items():
        if key in sensitive_keys:
            sanitized[key] = "***" if value else ""
        else:
            sanitized[key] = value

    connection.send_result(msg["id"], {"config": sanitized})
