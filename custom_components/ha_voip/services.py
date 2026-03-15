"""Service handlers for the HA VoIP integration.

Exposes HA services that proxy commands to the voip-engine via the
coordinator's gRPC client.
"""

from __future__ import annotations

import logging
from typing import Any

import voluptuous as vol

from homeassistant.core import HomeAssistant, ServiceCall, ServiceResponse, SupportsResponse
from homeassistant.exceptions import HomeAssistantError
from homeassistant.helpers import config_validation as cv

from .const import (
    DATA_COORDINATOR,
    DOMAIN,
    SERVICE_HANGUP,
    SERVICE_MAKE_CALL,
    SERVICE_MUTE_TOGGLE,
    SERVICE_RECORD_TOGGLE,
    SERVICE_SEND_DTMF,
    SERVICE_TRANSFER,
    SUPPORTED_CODECS,
)
from .coordinator import VoipDataUpdateCoordinator

_LOGGER = logging.getLogger(__name__)

# ---------------------------------------------------------------------------
# Voluptuous schemas for service parameters
# ---------------------------------------------------------------------------

SCHEMA_MAKE_CALL = vol.Schema(
    {
        vol.Required("target"): cv.string,
        vol.Required("from_extension"): cv.string,
        vol.Optional("caller_id", default=""): cv.string,
        vol.Optional("codec"): vol.In(SUPPORTED_CODECS),
        vol.Optional("auto_answer", default=False): cv.boolean,
        vol.Optional("record", default=False): cv.boolean,
    }
)

SCHEMA_HANGUP = vol.Schema(
    {
        vol.Required("call_id"): cv.string,
    }
)

SCHEMA_TRANSFER = vol.Schema(
    {
        vol.Required("call_id"): cv.string,
        vol.Required("target"): cv.string,
        vol.Optional("blind", default=True): cv.boolean,
    }
)

SCHEMA_RECORD_TOGGLE = vol.Schema(
    {
        vol.Required("call_id"): cv.string,
    }
)

SCHEMA_MUTE_TOGGLE = vol.Schema(
    {
        vol.Required("call_id"): cv.string,
    }
)

SCHEMA_SEND_DTMF = vol.Schema(
    {
        vol.Required("call_id"): cv.string,
        vol.Required("digits"): vol.All(
            cv.string, vol.Match(r"^[0-9A-D*#]+$")
        ),
    }
)


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _get_coordinator(hass: HomeAssistant) -> VoipDataUpdateCoordinator:
    """Retrieve the coordinator; raise if not available."""
    domain_data: dict[str, Any] | None = hass.data.get(DOMAIN)
    if domain_data is None:
        raise HomeAssistantError("HA VoIP integration is not loaded")
    coordinator: VoipDataUpdateCoordinator | None = domain_data.get(DATA_COORDINATOR)
    if coordinator is None:
        raise HomeAssistantError("HA VoIP coordinator is not available")
    return coordinator


# ---------------------------------------------------------------------------
# Service handler implementations
# ---------------------------------------------------------------------------


async def _handle_make_call(call: ServiceCall) -> ServiceResponse:
    """Handle the voip.make_call service."""
    coordinator = _get_coordinator(call.hass)
    client = coordinator.grpc_client

    if not client.connected:
        raise HomeAssistantError("VoIP engine is not connected")

    target: str = call.data["target"]
    from_ext: str = call.data["from_extension"]
    caller_id: str = call.data.get("caller_id", "")

    _LOGGER.info(
        "Service make_call: target=%s, from=%s, caller_id=%s",
        target,
        from_ext,
        caller_id,
    )

    try:
        result = await client.make_call(
            target=target,
            from_extension=from_ext,
            caller_id=caller_id,
        )
    except ConnectionError as exc:
        raise HomeAssistantError(f"Failed to make call: {exc}") from exc

    # Trigger a data refresh so entities update immediately
    await coordinator.async_request_refresh()

    return {
        "call_id": result.get("call_id", ""),
        "status": result.get("status", "unknown"),
    }


async def _handle_hangup(call: ServiceCall) -> None:
    """Handle the voip.hangup service."""
    coordinator = _get_coordinator(call.hass)
    client = coordinator.grpc_client

    if not client.connected:
        raise HomeAssistantError("VoIP engine is not connected")

    call_id: str = call.data["call_id"]
    _LOGGER.info("Service hangup: call_id=%s", call_id)

    success = await client.hangup_call(call_id)
    if not success:
        raise HomeAssistantError(f"Failed to hang up call {call_id}")

    await coordinator.async_request_refresh()


async def _handle_transfer(call: ServiceCall) -> None:
    """Handle the voip.transfer service."""
    coordinator = _get_coordinator(call.hass)
    client = coordinator.grpc_client

    if not client.connected:
        raise HomeAssistantError("VoIP engine is not connected")

    call_id: str = call.data["call_id"]
    target: str = call.data["target"]
    _LOGGER.info("Service transfer: call_id=%s, target=%s", call_id, target)

    success = await client.transfer_call(call_id, target)
    if not success:
        raise HomeAssistantError(
            f"Failed to transfer call {call_id} to {target}"
        )

    await coordinator.async_request_refresh()


async def _handle_record_toggle(call: ServiceCall) -> None:
    """Handle the voip.record_toggle service."""
    coordinator = _get_coordinator(call.hass)
    client = coordinator.grpc_client

    if not client.connected:
        raise HomeAssistantError("VoIP engine is not connected")

    call_id: str = call.data["call_id"]
    _LOGGER.info("Service record_toggle: call_id=%s", call_id)

    success = await client.toggle_recording(call_id)
    if not success:
        raise HomeAssistantError(
            f"Failed to toggle recording for call {call_id}"
        )

    await coordinator.async_request_refresh()


async def _handle_mute_toggle(call: ServiceCall) -> None:
    """Handle the voip.mute_toggle service."""
    coordinator = _get_coordinator(call.hass)
    client = coordinator.grpc_client

    if not client.connected:
        raise HomeAssistantError("VoIP engine is not connected")

    call_id: str = call.data["call_id"]
    _LOGGER.info("Service mute_toggle: call_id=%s", call_id)

    success = await client.toggle_mute(call_id)
    if not success:
        raise HomeAssistantError(
            f"Failed to toggle mute for call {call_id}"
        )

    await coordinator.async_request_refresh()


async def _handle_send_dtmf(call: ServiceCall) -> None:
    """Handle the voip.send_dtmf service."""
    coordinator = _get_coordinator(call.hass)
    client = coordinator.grpc_client

    if not client.connected:
        raise HomeAssistantError("VoIP engine is not connected")

    call_id: str = call.data["call_id"]
    digits: str = call.data["digits"]
    _LOGGER.info("Service send_dtmf: call_id=%s, digits=%s", call_id, digits)

    success = await client.send_dtmf(call_id, digits)
    if not success:
        raise HomeAssistantError(
            f"Failed to send DTMF '{digits}' on call {call_id}"
        )


# ---------------------------------------------------------------------------
# Registration
# ---------------------------------------------------------------------------


async def async_register_services(hass: HomeAssistant) -> None:
    """Register all HA VoIP services."""

    hass.services.async_register(
        DOMAIN,
        SERVICE_MAKE_CALL,
        _handle_make_call,
        schema=SCHEMA_MAKE_CALL,
        supports_response=SupportsResponse.OPTIONAL,
    )

    hass.services.async_register(
        DOMAIN,
        SERVICE_HANGUP,
        _handle_hangup,
        schema=SCHEMA_HANGUP,
    )

    hass.services.async_register(
        DOMAIN,
        SERVICE_TRANSFER,
        _handle_transfer,
        schema=SCHEMA_TRANSFER,
    )

    hass.services.async_register(
        DOMAIN,
        SERVICE_RECORD_TOGGLE,
        _handle_record_toggle,
        schema=SCHEMA_RECORD_TOGGLE,
    )

    hass.services.async_register(
        DOMAIN,
        SERVICE_MUTE_TOGGLE,
        _handle_mute_toggle,
        schema=SCHEMA_MUTE_TOGGLE,
    )

    hass.services.async_register(
        DOMAIN,
        SERVICE_SEND_DTMF,
        _handle_send_dtmf,
        schema=SCHEMA_SEND_DTMF,
    )

    _LOGGER.debug("Registered %d HA VoIP services", 6)


async def async_unregister_services(hass: HomeAssistant) -> None:
    """Remove all HA VoIP services."""
    for svc in (
        SERVICE_MAKE_CALL,
        SERVICE_HANGUP,
        SERVICE_TRANSFER,
        SERVICE_RECORD_TOGGLE,
        SERVICE_MUTE_TOGGLE,
        SERVICE_SEND_DTMF,
    ):
        hass.services.async_remove(DOMAIN, svc)
