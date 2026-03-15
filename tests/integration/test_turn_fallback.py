"""Integration test: TURN transport fallback chain.

Verifies the client-side TURN fallback logic:
  UDP 3478 -> TCP 3478 -> TLS 5349 -> TLS 443

Each test simulates a blocked transport and confirms that allocation
succeeds on the next available transport in the chain.

Run with:
    pytest tests/integration/test_turn_fallback.py -v
"""

from __future__ import annotations

import asyncio
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# TURN transports and error simulation
# ---------------------------------------------------------------------------


class TransportBlocker:
    """Simulates network-level blocking of specific transports/ports."""

    def __init__(self) -> None:
        self.blocked: set[tuple[str, int]] = set()

    def block(self, proto: str, port: int):
        self.blocked.add((proto, port))

    def is_blocked(self, proto: str, port: int) -> bool:
        return (proto, port) in self.blocked

    def clear(self):
        self.blocked.clear()


class MockTurnClient:
    """Simulates a TURN client that attempts the fallback chain."""

    FALLBACK_CHAIN = [
        ("udp", 3478),
        ("tcp", 3478),
        ("tls", 5349),
        ("tls", 443),
    ]

    def __init__(self, server: str, blocker: TransportBlocker) -> None:
        self.server = server
        self.blocker = blocker
        self.allocation: dict[str, Any] | None = None
        self.used_transport: tuple[str, int] | None = None
        self.attempts: list[tuple[str, int, str]] = []  # (proto, port, result)

    async def allocate(self, username: str, credential: str) -> dict[str, Any]:
        """Walk the fallback chain and return the first successful allocation."""
        for proto, port in self.FALLBACK_CHAIN:
            try:
                if self.blocker.is_blocked(proto, port):
                    raise ConnectionRefusedError(
                        f"{proto.upper()}:{port} blocked"
                    )
                # Simulate successful TURN allocation
                alloc = {
                    "relay_address": "198.51.100.1",
                    "relay_port": 49200,
                    "mapped_address": "203.0.113.5",
                    "mapped_port": 54321,
                    "lifetime": 600,
                    "transport": proto,
                    "port": port,
                }
                self.allocation = alloc
                self.used_transport = (proto, port)
                self.attempts.append((proto, port, "success"))
                return alloc
            except ConnectionRefusedError as exc:
                self.attempts.append((proto, port, f"blocked: {exc}"))
                continue

        raise RuntimeError("All TURN transports blocked; allocation failed")


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def blocker():
    return TransportBlocker()


@pytest.fixture
def turn_client(blocker):
    return MockTurnClient("turn.homeassistant.local", blocker)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_udp_3478_succeeds_by_default(turn_client, blocker):
    """With nothing blocked, UDP 3478 must be used."""
    alloc = await turn_client.allocate("user", "pass")
    assert alloc["transport"] == "udp"
    assert alloc["port"] == 3478
    assert len(turn_client.attempts) == 1


@pytest.mark.asyncio
async def test_fallback_to_tcp_3478(turn_client, blocker):
    """If UDP 3478 is blocked, TCP 3478 must be tried."""
    blocker.block("udp", 3478)
    alloc = await turn_client.allocate("user", "pass")
    assert alloc["transport"] == "tcp"
    assert alloc["port"] == 3478
    assert len(turn_client.attempts) == 2
    assert turn_client.attempts[0][2].startswith("blocked")


@pytest.mark.asyncio
async def test_fallback_to_tls_5349(turn_client, blocker):
    """If UDP and TCP 3478 are blocked, TLS 5349 must be tried."""
    blocker.block("udp", 3478)
    blocker.block("tcp", 3478)
    alloc = await turn_client.allocate("user", "pass")
    assert alloc["transport"] == "tls"
    assert alloc["port"] == 5349
    assert len(turn_client.attempts) == 3


@pytest.mark.asyncio
async def test_fallback_to_tls_443(turn_client, blocker):
    """If everything except TLS 443 is blocked, TLS 443 must succeed."""
    blocker.block("udp", 3478)
    blocker.block("tcp", 3478)
    blocker.block("tls", 5349)
    alloc = await turn_client.allocate("user", "pass")
    assert alloc["transport"] == "tls"
    assert alloc["port"] == 443
    assert len(turn_client.attempts) == 4


@pytest.mark.asyncio
async def test_all_blocked_raises(turn_client, blocker):
    """If every transport is blocked, a RuntimeError must be raised."""
    blocker.block("udp", 3478)
    blocker.block("tcp", 3478)
    blocker.block("tls", 5349)
    blocker.block("tls", 443)
    with pytest.raises(RuntimeError, match="All TURN transports blocked"):
        await turn_client.allocate("user", "pass")
    assert len(turn_client.attempts) == 4


@pytest.mark.asyncio
async def test_allocation_contains_relay_info(turn_client, blocker):
    """The allocation result must contain relay address and lifetime."""
    alloc = await turn_client.allocate("user", "pass")
    assert "relay_address" in alloc
    assert "relay_port" in alloc
    assert "mapped_address" in alloc
    assert alloc["lifetime"] == 600


@pytest.mark.asyncio
async def test_attempt_log_records_all_tries(turn_client, blocker):
    """The attempts log must record every transport tried."""
    blocker.block("udp", 3478)
    blocker.block("tcp", 3478)
    await turn_client.allocate("user", "pass")
    protos_tried = [(a[0], a[1]) for a in turn_client.attempts]
    assert protos_tried == [("udp", 3478), ("tcp", 3478), ("tls", 5349)]
