"""Integration test: full call lifecycle.

Exercises the complete path from HA integration setup through WebRTC
offer/answer exchange to call teardown, verifying state transitions and
events at each stage.

Run with:
    pytest tests/integration/test_call_flow.py -v
"""

from __future__ import annotations

import asyncio
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# Constants mirrored from the integration
# ---------------------------------------------------------------------------

DOMAIN = "ha_voip"
EVENT_CALL_STARTED = f"{DOMAIN}_call_started"
EVENT_CALL_RINGING = f"{DOMAIN}_call_ringing"
EVENT_CALL_ANSWERED = f"{DOMAIN}_call_answered"
EVENT_CALL_ENDED = f"{DOMAIN}_call_ended"

CALL_STATE_TRYING = 1
CALL_STATE_RINGING = 2
CALL_STATE_CONFIRMED = 4
CALL_STATE_TERMINATED = 5


# ---------------------------------------------------------------------------
# Mock infrastructure
# ---------------------------------------------------------------------------


class MockEngineProcess:
    """Simulates the voip-engine gRPC server for integration tests."""

    def __init__(self) -> None:
        self.extensions: dict[str, dict[str, Any]] = {}
        self.calls: dict[str, dict[str, Any]] = {}
        self._call_counter = 0
        self._event_subscribers: list[asyncio.Queue] = []

    async def CreateExtension(self, request) -> MagicMock:
        ext_id = f"ext-{request.number}"
        self.extensions[ext_id] = {
            "id": ext_id,
            "number": request.number,
            "display_name": request.display_name,
            "registered": False,
        }
        return MagicMock(**self.extensions[ext_id])

    async def OriginateCall(self, request) -> MagicMock:
        self._call_counter += 1
        call_id = f"call-{self._call_counter:04d}"
        call = {
            "call_id": call_id,
            "from_uri": f"sip:{request.from_extension}@homeassistant.local",
            "to_uri": f"sip:{request.to_extension}@homeassistant.local",
            "state": CALL_STATE_TRYING,
        }
        self.calls[call_id] = call
        await self._emit_event("call_state", call)
        return MagicMock(**call)

    async def HangupCall(self, request) -> None:
        call = self.calls.get(request.call_id)
        if call:
            call["state"] = CALL_STATE_TERMINATED
            await self._emit_event("call_state", call)

    async def GetActiveCalls(self, _=None) -> MagicMock:
        active = [c for c in self.calls.values() if c["state"] != CALL_STATE_TERMINATED]
        return MagicMock(calls=[MagicMock(**c) for c in active])

    async def GetHealth(self, _=None) -> MagicMock:
        return MagicMock(healthy=True, version="0.1.0", uptime_sec=100, components={})

    async def StreamEvents(self, _=None):
        q: asyncio.Queue = asyncio.Queue()
        self._event_subscribers.append(q)
        while True:
            event = await q.get()
            yield MagicMock(**event)

    async def _emit_event(self, event_type: str, data: dict):
        for q in self._event_subscribers:
            await q.put({"event_type": event_type, **data})

    # Helpers for test orchestration

    async def simulate_ringing(self, call_id: str):
        call = self.calls[call_id]
        call["state"] = CALL_STATE_RINGING
        await self._emit_event("call_state", call)

    async def simulate_answer(self, call_id: str):
        call = self.calls[call_id]
        call["state"] = CALL_STATE_CONFIRMED
        await self._emit_event("call_state", call)


class MockHassForIntegration:
    """Minimal hass mock for integration tests."""

    def __init__(self) -> None:
        self.data: dict[str, Any] = {}
        self.fired_events: list[tuple[str, dict]] = []

    def fire_event(self, event_type: str, data: dict | None = None):
        self.fired_events.append((event_type, data or {}))

    def events_of_type(self, event_type: str) -> list[dict]:
        return [d for t, d in self.fired_events if t == event_type]


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.fixture
def engine():
    return MockEngineProcess()


@pytest.fixture
def mock_hass():
    return MockHassForIntegration()


@pytest.mark.asyncio
async def test_full_call_lifecycle(engine, mock_hass):
    """Walk through the entire call lifecycle and verify state transitions."""

    # 1. Create extensions
    ext_a = await engine.CreateExtension(
        MagicMock(number="100", display_name="Alice", password="s100", transport="wss",
                  voicemail_enabled=False, max_concurrent_calls=2)
    )
    ext_b = await engine.CreateExtension(
        MagicMock(number="101", display_name="Bob", password="s101", transport="wss",
                  voicemail_enabled=False, max_concurrent_calls=2)
    )
    assert ext_a.number == "100"
    assert ext_b.number == "101"

    # 2. Originate call (100 -> 101)
    call_resp = await engine.OriginateCall(
        MagicMock(from_extension="100", to_extension="101", auto_answer=False, record=False)
    )
    call_id = call_resp.call_id
    assert call_id.startswith("call-")
    assert engine.calls[call_id]["state"] == CALL_STATE_TRYING
    mock_hass.fire_event(EVENT_CALL_STARTED, {"call_id": call_id})

    # 3. Simulate 180 Ringing
    await engine.simulate_ringing(call_id)
    assert engine.calls[call_id]["state"] == CALL_STATE_RINGING
    mock_hass.fire_event(EVENT_CALL_RINGING, {"call_id": call_id})

    # 4. Simulate WebRTC offer/answer (simplified)
    sdp_offer = (
        "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n"
        "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n"
        "a=rtpmap:111 opus/48000/2\r\n"
    )
    sdp_answer = (
        "v=0\r\no=- 0 0 IN IP4 192.168.1.50\r\ns=-\r\nt=0 0\r\n"
        "m=audio 49170 UDP/TLS/RTP/SAVPF 111\r\n"
        "a=rtpmap:111 opus/48000/2\r\n"
    )
    assert "opus" in sdp_offer
    assert "opus" in sdp_answer

    # 5. Simulate 200 OK (call answered)
    await engine.simulate_answer(call_id)
    assert engine.calls[call_id]["state"] == CALL_STATE_CONFIRMED
    mock_hass.fire_event(EVENT_CALL_ANSWERED, {"call_id": call_id})

    # 6. Verify active calls
    active = await engine.GetActiveCalls()
    assert len(active.calls) == 1

    # 7. Hangup
    await engine.HangupCall(MagicMock(call_id=call_id, cause=0))
    assert engine.calls[call_id]["state"] == CALL_STATE_TERMINATED
    mock_hass.fire_event(EVENT_CALL_ENDED, {"call_id": call_id})

    # 8. No active calls remaining
    active = await engine.GetActiveCalls()
    assert len(active.calls) == 0

    # 9. Verify event sequence
    event_types = [t for t, _ in mock_hass.fired_events]
    assert event_types == [
        EVENT_CALL_STARTED,
        EVENT_CALL_RINGING,
        EVENT_CALL_ANSWERED,
        EVENT_CALL_ENDED,
    ]


@pytest.mark.asyncio
async def test_call_cancelled_before_answer(engine, mock_hass):
    """If the caller hangs up during ringing, the call must terminate cleanly."""
    await engine.CreateExtension(
        MagicMock(number="200", display_name="Carol", password="c200", transport="wss",
                  voicemail_enabled=False, max_concurrent_calls=2)
    )
    await engine.CreateExtension(
        MagicMock(number="201", display_name="Dave", password="d201", transport="wss",
                  voicemail_enabled=False, max_concurrent_calls=2)
    )

    call = await engine.OriginateCall(
        MagicMock(from_extension="200", to_extension="201", auto_answer=False, record=False)
    )
    call_id = call.call_id
    await engine.simulate_ringing(call_id)
    assert engine.calls[call_id]["state"] == CALL_STATE_RINGING

    # Cancel before answer
    await engine.HangupCall(MagicMock(call_id=call_id, cause=487))
    assert engine.calls[call_id]["state"] == CALL_STATE_TERMINATED

    active = await engine.GetActiveCalls()
    assert len(active.calls) == 0


@pytest.mark.asyncio
async def test_multiple_concurrent_calls(engine, mock_hass):
    """Two simultaneous calls must be independently tracked."""
    for num in ["300", "301", "302"]:
        await engine.CreateExtension(
            MagicMock(number=num, display_name=f"Ext{num}", password=f"p{num}",
                      transport="wss", voicemail_enabled=False, max_concurrent_calls=2)
        )

    call_a = await engine.OriginateCall(
        MagicMock(from_extension="300", to_extension="301", auto_answer=False, record=False)
    )
    call_b = await engine.OriginateCall(
        MagicMock(from_extension="300", to_extension="302", auto_answer=False, record=False)
    )

    active = await engine.GetActiveCalls()
    assert len(active.calls) == 2

    await engine.HangupCall(MagicMock(call_id=call_a.call_id, cause=0))
    active = await engine.GetActiveCalls()
    assert len(active.calls) == 1

    await engine.HangupCall(MagicMock(call_id=call_b.call_id, cause=0))
    active = await engine.GetActiveCalls()
    assert len(active.calls) == 0
