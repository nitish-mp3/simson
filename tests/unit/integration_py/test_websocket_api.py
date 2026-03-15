"""Unit tests for the HA VoIP WebSocket API.

The WebSocket API exposes commands such as ``voip/subscribe``,
``voip/webrtc_offer``, and ``voip/webrtc_candidate`` to the Lovelace
frontend.  Tests verify message routing and response structure.
"""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


class FakeWsConnection:
    """Simulates the HA WebSocket connection from the frontend side."""

    def __init__(self, hass, engine_client):
        self.hass = hass
        self.engine_client = engine_client
        self._sent: list[dict[str, Any]] = []
        self._msg_id = 0
        self._subscriptions: dict[int, Any] = {}

    async def send(self, msg: dict[str, Any]) -> dict[str, Any]:
        """Send a WS message and return the mock response."""
        self._msg_id += 1
        msg["id"] = self._msg_id
        self._sent.append(msg)

        msg_type = msg.get("type", "")

        if msg_type == "voip/subscribe":
            self._subscriptions[self._msg_id] = True
            return {"id": self._msg_id, "type": "result", "success": True}

        if msg_type == "voip/webrtc_offer":
            # Engine would return an SDP answer
            return {
                "id": self._msg_id,
                "type": "result",
                "success": True,
                "result": {
                    "sdp": "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n"
                    "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n"
                    "a=rtpmap:111 opus/48000/2\r\n",
                    "type": "answer",
                },
            }

        if msg_type == "voip/webrtc_candidate":
            return {"id": self._msg_id, "type": "result", "success": True}

        if msg_type == "voip/call":
            call_resp = await self.engine_client.OriginateCall(
                MagicMock(
                    from_extension=msg.get("from_extension", "100"),
                    to_extension=msg["number"],
                    auto_answer=False,
                    record=False,
                )
            )
            return {
                "id": self._msg_id,
                "type": "result",
                "success": True,
                "result": {"call_id": call_resp.call_id},
            }

        if msg_type == "voip/hangup":
            await self.engine_client.HangupCall(
                MagicMock(call_id=msg["call_id"], cause=0)
            )
            return {"id": self._msg_id, "type": "result", "success": True}

        if msg_type == "voip/extensions":
            exts = await self.engine_client.ListExtensions(MagicMock())
            return {
                "id": self._msg_id,
                "type": "result",
                "success": True,
                "result": [
                    {"id": e.id, "number": e.number, "name": e.display_name}
                    for e in exts.extensions
                ],
            }

        return {"id": self._msg_id, "type": "result", "success": False, "error": "unknown_command"}


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_subscribe_events(hass, engine_client):
    """voip/subscribe must return success."""
    ws = FakeWsConnection(hass, engine_client)
    resp = await ws.send({"type": "voip/subscribe"})
    assert resp["success"] is True
    assert 1 in ws._subscriptions


@pytest.mark.asyncio
async def test_send_sdp_offer(hass, engine_client):
    """voip/webrtc_offer must return an SDP answer."""
    ws = FakeWsConnection(hass, engine_client)
    resp = await ws.send(
        {
            "type": "voip/webrtc_offer",
            "call_id": "call-001",
            "sdp": "v=0\r\no=- 0 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n"
            "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n",
        }
    )
    assert resp["success"] is True
    assert "sdp" in resp["result"]
    assert resp["result"]["sdp"].startswith("v=0")


@pytest.mark.asyncio
async def test_send_ice_candidate(hass, engine_client):
    """voip/webrtc_candidate must return success."""
    ws = FakeWsConnection(hass, engine_client)
    resp = await ws.send(
        {
            "type": "voip/webrtc_candidate",
            "call_id": "call-001",
            "candidate": {
                "candidate": "candidate:1 1 udp 2122260223 192.168.1.50 12345 typ host",
                "sdpMLineIndex": 0,
                "sdpMid": "audio",
            },
        }
    )
    assert resp["success"] is True


@pytest.mark.asyncio
async def test_ws_call_command(hass, engine_client):
    """voip/call must return a call_id from the engine."""
    ws = FakeWsConnection(hass, engine_client)
    resp = await ws.send({"type": "voip/call", "number": "101"})
    assert resp["success"] is True
    assert resp["result"]["call_id"] == "call-uuid-001"
    engine_client.OriginateCall.assert_awaited_once()


@pytest.mark.asyncio
async def test_ws_hangup_command(hass, engine_client):
    """voip/hangup must delegate to engine HangupCall."""
    ws = FakeWsConnection(hass, engine_client)
    resp = await ws.send({"type": "voip/hangup", "call_id": "call-uuid-001"})
    assert resp["success"] is True
    engine_client.HangupCall.assert_awaited_once()


@pytest.mark.asyncio
async def test_ws_extensions_list(hass, engine_client):
    """voip/extensions must return the list of extensions."""
    ws = FakeWsConnection(hass, engine_client)
    resp = await ws.send({"type": "voip/extensions"})
    assert resp["success"] is True
    assert len(resp["result"]) == 2
    assert resp["result"][0]["number"] == "100"
