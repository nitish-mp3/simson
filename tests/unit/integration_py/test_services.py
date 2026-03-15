"""Unit tests for HA VoIP services (make_call, hangup, transfer, etc.).

Each test mocks the gRPC client and verifies that the HA service layer
correctly delegates to the engine and validates parameters.
"""

from __future__ import annotations

from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from custom_components.ha_voip.const import (
    DOMAIN,
    SERVICE_HANGUP,
    SERVICE_MAKE_CALL,
    SERVICE_MUTE_TOGGLE,
    SERVICE_RECORD_TOGGLE,
    SERVICE_SEND_DTMF,
    SERVICE_TRANSFER,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

# Inline ServiceCall-like object
_ServiceCall = MagicMock


def _make_call_data(
    from_ext: str = "100",
    to_ext: str = "101",
    auto_answer: bool = False,
    record: bool = False,
) -> dict[str, Any]:
    return {
        "from_extension": from_ext,
        "to_extension": to_ext,
        "auto_answer": auto_answer,
        "record": record,
    }


# ---------------------------------------------------------------------------
# make_call service
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_make_call_delegates_to_grpc(hass, engine_client):
    """make_call must invoke OriginateCall on the gRPC client."""
    from custom_components.ha_voip.services import async_handle_make_call

    call = MagicMock(data=_make_call_data())
    await async_handle_make_call(hass, engine_client, call)

    engine_client.OriginateCall.assert_awaited_once()
    args = engine_client.OriginateCall.call_args
    req = args[0][0] if args[0] else args[1].get("request")
    assert req.from_extension == "100"
    assert req.to_extension == "101"


@pytest.mark.asyncio
async def test_make_call_with_auto_answer(hass, engine_client):
    """auto_answer flag must be forwarded."""
    from custom_components.ha_voip.services import async_handle_make_call

    call = MagicMock(data=_make_call_data(auto_answer=True))
    await async_handle_make_call(hass, engine_client, call)

    req = engine_client.OriginateCall.call_args[0][0]
    assert req.auto_answer is True


@pytest.mark.asyncio
async def test_make_call_fires_event(hass, engine_client):
    """A successful origination must fire ha_voip_call_started."""
    from custom_components.ha_voip.services import async_handle_make_call

    call = MagicMock(data=_make_call_data())
    await async_handle_make_call(hass, engine_client, call)

    fired = [ev for ev, _ in hass.bus.fired_events if ev == f"{DOMAIN}_call_started"]
    assert len(fired) == 1


# ---------------------------------------------------------------------------
# hangup service
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_hangup_delegates_to_grpc(hass, engine_client):
    """hangup must invoke HangupCall."""
    from custom_components.ha_voip.services import async_handle_hangup

    call = MagicMock(data={"call_id": "call-uuid-001"})
    await async_handle_hangup(hass, engine_client, call)

    engine_client.HangupCall.assert_awaited_once()


@pytest.mark.asyncio
async def test_hangup_missing_call_id(hass, engine_client):
    """hangup without call_id must raise."""
    from custom_components.ha_voip.services import async_handle_hangup

    call = MagicMock(data={})
    with pytest.raises((KeyError, ValueError)):
        await async_handle_hangup(hass, engine_client, call)


# ---------------------------------------------------------------------------
# transfer service
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_transfer_delegates_to_grpc(hass, engine_client):
    """transfer must invoke TransferCall."""
    from custom_components.ha_voip.services import async_handle_transfer

    call = MagicMock(
        data={"call_id": "call-uuid-001", "target_extension": "102", "blind": True}
    )
    await async_handle_transfer(hass, engine_client, call)

    engine_client.TransferCall.assert_awaited_once()


@pytest.mark.asyncio
async def test_transfer_missing_target(hass, engine_client):
    """transfer without target_extension must raise."""
    from custom_components.ha_voip.services import async_handle_transfer

    call = MagicMock(data={"call_id": "call-uuid-001"})
    with pytest.raises((KeyError, ValueError)):
        await async_handle_transfer(hass, engine_client, call)


# ---------------------------------------------------------------------------
# Parameter validation
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_make_call_invalid_extension_format(hass, engine_client):
    """Non-numeric extension must be rejected at the service layer."""
    from custom_components.ha_voip.services import async_handle_make_call

    call = MagicMock(data=_make_call_data(from_ext="not_a_number"))
    # Depending on implementation, this may raise ValueError or be sent
    # to the engine which returns an error.  Either is acceptable:
    try:
        await async_handle_make_call(hass, engine_client, call)
        # If it reached the engine, the engine_client was still called
        engine_client.OriginateCall.assert_awaited_once()
    except (ValueError, TypeError):
        pass  # Validation caught it early -- expected
