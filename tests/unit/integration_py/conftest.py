"""Shared pytest fixtures for the HA VoIP integration test suite."""

from __future__ import annotations

import asyncio
from collections.abc import AsyncGenerator, Generator
from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

# ---------------------------------------------------------------------------
# Minimal Home Assistant mock objects
# ---------------------------------------------------------------------------


class MockConfigEntry:
    """Minimal stand-in for homeassistant.config_entries.ConfigEntry."""

    def __init__(
        self,
        domain: str = "ha_voip",
        data: dict[str, Any] | None = None,
        options: dict[str, Any] | None = None,
        entry_id: str = "test_entry_id",
        title: str = "HA VoIP",
        version: int = 1,
        unique_id: str | None = "ha_voip",
    ) -> None:
        self.domain = domain
        self.data = data or self._default_data()
        self.options = options or {}
        self.entry_id = entry_id
        self.title = title
        self.version = version
        self.unique_id = unique_id
        self.state = "loaded"

    @staticmethod
    def _default_data() -> dict[str, Any]:
        return {
            "engine_mode": "local",
            "sip_port": 5060,
            "grpc_port": 50051,
            "ws_port": 8586,
            "rtp_port_start": 10000,
            "rtp_port_end": 20000,
            "external_host": "192.168.1.50",
            "stun_server": "stun:stun.l.google.com:19302",
            "turn_server": "",
            "turn_username": "",
            "turn_password": "",
            "cert_mode": "self_signed",
            "cert_path": "",
            "key_path": "",
            "acme_domain": "",
            "acme_email": "",
            "extensions": [
                {"number": "100", "name": "Alice", "password": "secret100"},
                {"number": "101", "name": "Bob", "password": "secret101"},
            ],
            "default_codec": "opus",
            "enable_recording": False,
            "recording_path": "/config/recordings/voip",
            "log_level": "info",
        }


class MockHass:
    """Minimal stand-in for homeassistant.core.HomeAssistant."""

    def __init__(self) -> None:
        self.data: dict[str, Any] = {}
        self.states = MockStates()
        self.bus = MockEventBus()
        self.config_entries = MockConfigEntries()
        self.config = MockHassConfig()
        self.services = MockServiceRegistry()
        self.loop = asyncio.get_event_loop()
        self._jobs: list[Any] = []

    async def async_add_executor_job(self, func, *args):
        """Run a sync function in the executor (calls directly in tests)."""
        return func(*args)

    async def async_create_task(self, coro):
        return await coro

    def async_add_job(self, target, *args):
        self._jobs.append((target, args))


class MockStates:
    """Mock hass.states."""

    def __init__(self) -> None:
        self._states: dict[str, Any] = {}

    def get(self, entity_id: str) -> Any:
        return self._states.get(entity_id)

    def async_set(self, entity_id: str, state: str, attributes: dict | None = None):
        self._states[entity_id] = MagicMock(
            entity_id=entity_id, state=state, attributes=attributes or {}
        )


class MockEventBus:
    """Mock hass.bus."""

    def __init__(self) -> None:
        self.fired_events: list[tuple[str, dict]] = []
        self._listeners: dict[str, list] = {}

    def async_fire(self, event_type: str, event_data: dict | None = None):
        self.fired_events.append((event_type, event_data or {}))

    def async_listen(self, event_type: str, callback):
        self._listeners.setdefault(event_type, []).append(callback)
        return lambda: self._listeners[event_type].remove(callback)


class MockConfigEntries:
    """Mock hass.config_entries."""

    def __init__(self) -> None:
        self._entries: list[MockConfigEntry] = []

    def async_entries(self, domain: str | None = None):
        if domain:
            return [e for e in self._entries if e.domain == domain]
        return list(self._entries)

    async def async_reload(self, entry_id: str):
        pass


class MockHassConfig:
    """Mock hass.config."""

    def __init__(self) -> None:
        self.components: set[str] = {"ha_voip", "websocket_api"}
        self.config_dir = "/config"
        self.internal_url = "http://192.168.1.50:8123"
        self.external_url = "https://my.duckdns.org:8123"


class MockServiceRegistry:
    """Mock hass.services."""

    def __init__(self) -> None:
        self._services: dict[str, dict[str, Any]] = {}

    def async_register(self, domain: str, service: str, handler, schema=None):
        self._services.setdefault(domain, {})[service] = handler

    def has_service(self, domain: str, service: str) -> bool:
        return service in self._services.get(domain, {})

    async def async_call(self, domain: str, service: str, service_data: dict | None = None):
        handler = self._services.get(domain, {}).get(service)
        if handler is None:
            raise ValueError(f"Service {domain}.{service} not registered")
        call = MagicMock(data=service_data or {})
        if asyncio.iscoroutinefunction(handler):
            await handler(call)
        else:
            handler(call)


# ---------------------------------------------------------------------------
# gRPC client mock
# ---------------------------------------------------------------------------


def _build_mock_grpc_client() -> AsyncMock:
    """Return an AsyncMock that mimics the generated VoipService gRPC stub."""
    client = AsyncMock()

    # Extension RPCs
    client.CreateExtension.return_value = MagicMock(
        id="ext-001",
        number="100",
        display_name="Alice",
        transport="wss",
        registered=False,
    )
    client.ListExtensions.return_value = MagicMock(
        extensions=[
            MagicMock(id="ext-001", number="100", display_name="Alice", registered=True),
            MagicMock(id="ext-002", number="101", display_name="Bob", registered=False),
        ],
        next_page_token="",
    )
    client.GetExtension.return_value = MagicMock(
        id="ext-001", number="100", display_name="Alice", registered=True,
    )
    client.DeleteExtension.return_value = None

    # Call RPCs
    client.OriginateCall.return_value = MagicMock(
        call_id="call-uuid-001",
        from_uri="sip:100@homeassistant.local",
        to_uri="sip:101@homeassistant.local",
        state=1,  # TRYING
    )
    client.HangupCall.return_value = None
    client.TransferCall.return_value = MagicMock(
        call_id="call-uuid-001",
        state=4,  # CONFIRMED
    )
    client.GetActiveCalls.return_value = MagicMock(calls=[])

    # Health / metrics
    client.GetHealth.return_value = MagicMock(
        healthy=True,
        version="0.1.0",
        uptime_sec=3600,
        components={},
    )
    client.GetMetrics.return_value = MagicMock(
        active_calls=0,
        active_registrations=2,
        active_turn_allocs=0,
    )

    return client


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def hass() -> MockHass:
    """Return a mock Home Assistant instance."""
    return MockHass()


@pytest.fixture
def config_entry() -> MockConfigEntry:
    """Return a mock ConfigEntry with default VoIP settings."""
    return MockConfigEntry()


@pytest.fixture
def engine_client() -> AsyncMock:
    """Return a mock gRPC VoipService client."""
    return _build_mock_grpc_client()


@pytest.fixture
def hass_ws(hass: MockHass) -> AsyncMock:
    """Return a mock WebSocket connection attached to *hass*."""
    ws = AsyncMock()
    ws.hass = hass
    ws.send_json = AsyncMock()
    ws.receive_json = AsyncMock(return_value={"type": "result", "success": True})
    return ws
