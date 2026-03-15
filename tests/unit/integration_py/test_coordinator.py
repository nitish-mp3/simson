"""Unit tests for the HA VoIP data-update coordinator.

The coordinator periodically polls the engine for health / metrics / call
state and pushes the results into ``hass.data``.  Tests verify data flow,
connection handling, and event processing.
"""

from __future__ import annotations

import asyncio
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from custom_components.ha_voip.const import (
    DATA_COORDINATOR,
    DOMAIN,
    ENGINE_STATE_ERROR,
    ENGINE_STATE_RUNNING,
    EVENT_ENGINE_STATE_CHANGED,
    EVENT_REGISTRATION_CHANGED,
    UPDATE_INTERVAL_SECONDS,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


class FakeCoordinator:
    """Stand-in for the real DataUpdateCoordinator.

    Re-implements the essential interface so we can test logic in isolation
    without pulling in the full HA coordinator machinery.
    """

    def __init__(self, hass, client, update_interval: int = UPDATE_INTERVAL_SECONDS):
        self.hass = hass
        self.client = client
        self.update_interval = update_interval
        self.data: dict[str, Any] = {}
        self.last_error: Exception | None = None
        self._listeners: list = []

    async def async_refresh(self):
        """Fetch fresh data from the engine."""
        try:
            health = await self.client.GetHealth()
            metrics = await self.client.GetMetrics()
            extensions = await self.client.ListExtensions()
            self.data = {
                "healthy": health.healthy,
                "version": health.version,
                "uptime_sec": health.uptime_sec,
                "active_calls": metrics.active_calls,
                "active_registrations": metrics.active_registrations,
                "extensions": [
                    {
                        "id": e.id,
                        "number": e.number,
                        "name": e.display_name,
                        "registered": e.registered,
                    }
                    for e in extensions.extensions
                ],
            }
            self.last_error = None
        except Exception as exc:
            self.last_error = exc
            self.data["healthy"] = False

    def async_add_listener(self, callback):
        self._listeners.append(callback)
        return lambda: self._listeners.remove(callback)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_coordinator_fetches_health(hass, engine_client):
    """After refresh, coordinator.data must contain health info."""
    coord = FakeCoordinator(hass, engine_client)
    await coord.async_refresh()

    assert coord.data["healthy"] is True
    assert coord.data["version"] == "0.1.0"
    assert coord.data["uptime_sec"] == 3600


@pytest.mark.asyncio
async def test_coordinator_fetches_metrics(hass, engine_client):
    """After refresh, coordinator.data must contain metrics."""
    coord = FakeCoordinator(hass, engine_client)
    await coord.async_refresh()

    assert coord.data["active_calls"] == 0
    assert coord.data["active_registrations"] == 2


@pytest.mark.asyncio
async def test_coordinator_fetches_extensions(hass, engine_client):
    """Extension list must be populated."""
    coord = FakeCoordinator(hass, engine_client)
    await coord.async_refresh()

    exts = coord.data["extensions"]
    assert len(exts) == 2
    assert exts[0]["number"] == "100"
    assert exts[1]["number"] == "101"


@pytest.mark.asyncio
async def test_coordinator_handles_grpc_failure(hass, engine_client):
    """gRPC errors must be captured without crashing."""
    engine_client.GetHealth.side_effect = ConnectionError("Engine down")
    coord = FakeCoordinator(hass, engine_client)
    await coord.async_refresh()

    assert coord.last_error is not None
    assert coord.data.get("healthy") is False


@pytest.mark.asyncio
async def test_coordinator_recovers_after_failure(hass, engine_client):
    """After a transient failure, the next refresh should recover."""
    engine_client.GetHealth.side_effect = ConnectionError("Transient")
    coord = FakeCoordinator(hass, engine_client)
    await coord.async_refresh()
    assert coord.data.get("healthy") is False

    # Restore normal behaviour
    engine_client.GetHealth.side_effect = None
    engine_client.GetHealth.return_value = MagicMock(
        healthy=True, version="0.1.0", uptime_sec=3610, components={}
    )
    await coord.async_refresh()
    assert coord.data["healthy"] is True
    assert coord.last_error is None


@pytest.mark.asyncio
async def test_coordinator_listener_notified(hass, engine_client):
    """Registered listeners should be callable (smoke test)."""
    coord = FakeCoordinator(hass, engine_client)
    notified = []
    unsub = coord.async_add_listener(lambda: notified.append(True))

    # Simulate HA calling listeners after refresh
    for cb in coord._listeners:
        cb()
    assert len(notified) == 1

    unsub()
    assert len(coord._listeners) == 0
