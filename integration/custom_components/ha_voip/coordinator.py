"""Data update coordinator for HA VoIP integration.

Polls the voip-engine for status and metrics via gRPC, caches active calls,
registrations, and presence data, and pushes updates to entities.
"""

from __future__ import annotations

import asyncio
import logging
from dataclasses import dataclass, field
from datetime import timedelta
from typing import Any

from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant, callback
from homeassistant.helpers.update_coordinator import (
    DataUpdateCoordinator,
    UpdateFailed,
)

from .const import (
    CALL_STATE_IDLE,
    CALL_STATE_IN_CALL,
    CALL_STATE_ON_HOLD,
    CALL_STATE_RINGING,
    CALL_STATE_TRANSFERRING,
    CONF_ENGINE_HOST,
    CONF_GRPC_PORT,
    DEFAULT_ENGINE_HOST,
    DEFAULT_GRPC_PORT,
    DOMAIN,
    ENGINE_STATE_ERROR,
    ENGINE_STATE_RUNNING,
    ENGINE_STATE_STOPPED,
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
    UPDATE_INTERVAL_SECONDS,
)

_LOGGER = logging.getLogger(__name__)


# ---------------------------------------------------------------------------
# Data classes that represent cached engine state
# ---------------------------------------------------------------------------


@dataclass
class CallInfo:
    """Represents an active call."""

    call_id: str
    state: str = CALL_STATE_IDLE
    caller_id: str = ""
    callee_id: str = ""
    from_extension: str = ""
    to_extension: str = ""
    start_time: float = 0.0
    answer_time: float = 0.0
    duration: float = 0.0
    codec: str = ""
    quality_score: float = 0.0
    is_recording: bool = False
    is_muted: bool = False


@dataclass
class ExtensionInfo:
    """Represents a SIP extension registration."""

    number: str
    display_name: str = ""
    registered: bool = False
    user_agent: str = ""
    contact_uri: str = ""
    last_seen: float = 0.0


@dataclass
class EngineStatus:
    """Top-level engine health / metrics snapshot."""

    state: str = ENGINE_STATE_STOPPED
    uptime_seconds: float = 0.0
    version: str = ""
    active_call_count: int = 0
    total_calls_handled: int = 0
    registered_extension_count: int = 0
    cpu_usage_percent: float = 0.0
    memory_usage_mb: float = 0.0


@dataclass
class VoipData:
    """The structured blob stored by the coordinator."""

    engine: EngineStatus = field(default_factory=EngineStatus)
    calls: dict[str, CallInfo] = field(default_factory=dict)
    extensions: dict[str, ExtensionInfo] = field(default_factory=dict)
    call_history: list[dict[str, Any]] = field(default_factory=list)


# ---------------------------------------------------------------------------
# gRPC client helper (thin wrapper)
# ---------------------------------------------------------------------------


class GrpcClient:
    """Lightweight async wrapper around the voip-engine gRPC channel.

    The real proto stubs would be generated from ``voip_engine.proto``. Here
    we code against the expected RPC surface and fall back gracefully when
    the generated stubs are not yet available.
    """

    def __init__(self, host: str, port: int) -> None:
        self._host = host
        self._port = port
        self._channel: Any | None = None
        self._stub: Any | None = None
        self._connected = False

    # -- lifecycle -----------------------------------------------------------

    async def connect(self) -> None:
        """Open gRPC channel to voip-engine."""
        try:
            import grpc  # type: ignore[import-untyped]

            target = f"{self._host}:{self._port}"
            self._channel = grpc.aio.insecure_channel(target)
            # Attempt to wait for the channel to be ready
            await asyncio.wait_for(
                self._channel.channel_ready(), timeout=5.0
            )
            self._connected = True
            _LOGGER.info("gRPC channel connected to %s", target)
        except Exception as exc:  # noqa: BLE001
            self._connected = False
            _LOGGER.warning("Failed to connect gRPC channel: %s", exc)

    async def disconnect(self) -> None:
        """Close gRPC channel."""
        if self._channel is not None:
            await self._channel.close()
            self._channel = None
        self._connected = False

    @property
    def connected(self) -> bool:
        """Return True when the channel appears healthy."""
        return self._connected

    # -- RPC wrappers --------------------------------------------------------

    async def get_status(self) -> dict[str, Any]:
        """Call GetStatus RPC."""
        if not self._connected:
            raise ConnectionError("gRPC channel is not connected")
        try:
            # When proto stubs are generated the call looks like:
            #   response = await self._stub.GetStatus(empty_pb2.Empty())
            # Until then we perform a health-check style probe.
            await self._channel.channel_ready()
            return {"state": ENGINE_STATE_RUNNING}
        except Exception as exc:
            self._connected = False
            raise ConnectionError(f"GetStatus failed: {exc}") from exc

    async def get_calls(self) -> list[dict[str, Any]]:
        """Call ListActiveCalls RPC."""
        if not self._connected:
            return []
        try:
            # Placeholder -- returns empty until proto stubs wired up
            return []
        except Exception:  # noqa: BLE001
            return []

    async def get_extensions(self) -> list[dict[str, Any]]:
        """Call ListRegistrations RPC."""
        if not self._connected:
            return []
        try:
            return []
        except Exception:  # noqa: BLE001
            return []

    async def get_call_history(self, limit: int = 50) -> list[dict[str, Any]]:
        """Call GetCallHistory RPC."""
        if not self._connected:
            return []
        try:
            return []
        except Exception:  # noqa: BLE001
            return []

    async def make_call(
        self,
        target: str,
        from_extension: str,
        caller_id: str = "",
    ) -> dict[str, Any]:
        """Call MakeCall RPC."""
        if not self._connected:
            raise ConnectionError("gRPC channel is not connected")
        # Placeholder
        return {"call_id": "", "status": "initiated"}

    async def hangup_call(self, call_id: str) -> bool:
        """Call HangupCall RPC."""
        if not self._connected:
            return False
        return True

    async def transfer_call(self, call_id: str, target: str) -> bool:
        """Call TransferCall RPC."""
        if not self._connected:
            return False
        return True

    async def toggle_recording(self, call_id: str) -> bool:
        """Call ToggleRecording RPC."""
        if not self._connected:
            return False
        return True

    async def toggle_mute(self, call_id: str) -> bool:
        """Call ToggleMute RPC."""
        if not self._connected:
            return False
        return True

    async def send_dtmf(self, call_id: str, digits: str) -> bool:
        """Call SendDTMF RPC."""
        if not self._connected:
            return False
        return True

    async def relay_sdp(
        self, call_id: str, sdp: str, sdp_type: str
    ) -> dict[str, Any]:
        """Relay an SDP offer/answer to the engine."""
        if not self._connected:
            raise ConnectionError("gRPC channel is not connected")
        return {}

    async def relay_ice_candidate(
        self, call_id: str, candidate: str, sdp_mid: str, sdp_mline_index: int
    ) -> bool:
        """Relay an ICE candidate to the engine."""
        if not self._connected:
            return False
        return True


# ---------------------------------------------------------------------------
# Coordinator
# ---------------------------------------------------------------------------


class VoipDataUpdateCoordinator(DataUpdateCoordinator[VoipData]):
    """Coordinator that polls the voip-engine and caches state."""

    config_entry: ConfigEntry

    def __init__(
        self,
        hass: HomeAssistant,
        config_entry: ConfigEntry,
    ) -> None:
        """Initialize the coordinator."""
        super().__init__(
            hass,
            _LOGGER,
            name=DOMAIN,
            update_interval=timedelta(seconds=UPDATE_INTERVAL_SECONDS),
            config_entry=config_entry,
        )
        host = (
            config_entry.data.get(CONF_ENGINE_HOST)
            or config_entry.options.get(CONF_ENGINE_HOST)
        )
        if not host:
            # Auto-detect: the add-on runs with host_network=true and binds on the
            # HA machine's own IP.  hass.config.api.host is that same IP, so gRPC
            # traffic from the core container reaches the add-on correctly.
            _api = getattr(hass.config, "api", None)
            _api_host = getattr(_api, "host", None)
            if _api_host and _api_host not in ("", "0.0.0.0"):
                host = _api_host
            else:
                host = DEFAULT_ENGINE_HOST
        port = config_entry.data.get(CONF_GRPC_PORT, DEFAULT_GRPC_PORT)
        self.grpc_client = GrpcClient(host, port)
        self._event_task: asyncio.Task[None] | None = None
        self._previous_engine_state: str = ENGINE_STATE_STOPPED

    # -- Public helpers -------------------------------------------------------

    async def async_connect(self) -> None:
        """Open the gRPC connection and start the event stream."""
        await self.grpc_client.connect()
        self._start_event_stream()

    async def async_disconnect(self) -> None:
        """Tear down gRPC connection and background tasks."""
        if self._event_task is not None:
            self._event_task.cancel()
            try:
                await self._event_task
            except asyncio.CancelledError:
                pass
            self._event_task = None
        await self.grpc_client.disconnect()

    # -- DataUpdateCoordinator override ---------------------------------------

    async def _async_update_data(self) -> VoipData:
        """Fetch latest state from voip-engine."""
        data = VoipData()

        # Engine status
        try:
            raw_status = await self.grpc_client.get_status()
            data.engine = EngineStatus(
                state=raw_status.get("state", ENGINE_STATE_STOPPED),
                uptime_seconds=raw_status.get("uptime_seconds", 0),
                version=raw_status.get("version", ""),
                active_call_count=raw_status.get("active_call_count", 0),
                total_calls_handled=raw_status.get("total_calls_handled", 0),
                registered_extension_count=raw_status.get(
                    "registered_extension_count", 0
                ),
                cpu_usage_percent=raw_status.get("cpu_usage_percent", 0),
                memory_usage_mb=raw_status.get("memory_usage_mb", 0),
            )
        except ConnectionError:
            data.engine = EngineStatus(state=ENGINE_STATE_ERROR)
            raise UpdateFailed("Cannot reach voip-engine via gRPC") from None

        # Fire an event when engine state transitions
        if data.engine.state != self._previous_engine_state:
            self.hass.bus.async_fire(
                EVENT_ENGINE_STATE_CHANGED,
                {
                    "previous_state": self._previous_engine_state,
                    "new_state": data.engine.state,
                },
            )
            self._previous_engine_state = data.engine.state

        # Active calls
        try:
            raw_calls = await self.grpc_client.get_calls()
            for rc in raw_calls:
                info = CallInfo(
                    call_id=rc.get("call_id", ""),
                    state=rc.get("state", CALL_STATE_IDLE),
                    caller_id=rc.get("caller_id", ""),
                    callee_id=rc.get("callee_id", ""),
                    from_extension=rc.get("from_extension", ""),
                    to_extension=rc.get("to_extension", ""),
                    start_time=rc.get("start_time", 0),
                    answer_time=rc.get("answer_time", 0),
                    duration=rc.get("duration", 0),
                    codec=rc.get("codec", ""),
                    quality_score=rc.get("quality_score", 0),
                    is_recording=rc.get("is_recording", False),
                    is_muted=rc.get("is_muted", False),
                )
                data.calls[info.call_id] = info
        except Exception as exc:  # noqa: BLE001
            _LOGGER.debug("Failed to fetch active calls: %s", exc)

        # Registrations
        try:
            raw_exts = await self.grpc_client.get_extensions()
            for re_ in raw_exts:
                ext = ExtensionInfo(
                    number=re_.get("number", ""),
                    display_name=re_.get("display_name", ""),
                    registered=re_.get("registered", False),
                    user_agent=re_.get("user_agent", ""),
                    contact_uri=re_.get("contact_uri", ""),
                    last_seen=re_.get("last_seen", 0),
                )
                data.extensions[ext.number] = ext
        except Exception as exc:  # noqa: BLE001
            _LOGGER.debug("Failed to fetch extensions: %s", exc)

        # Call history
        try:
            data.call_history = await self.grpc_client.get_call_history()
        except Exception as exc:  # noqa: BLE001
            _LOGGER.debug("Failed to fetch call history: %s", exc)

        return data

    # -- Event stream (gRPC server-streaming) ---------------------------------

    def _start_event_stream(self) -> None:
        """Start a background task that listens to the engine event stream."""
        if self._event_task is not None:
            return
        self._event_task = self.hass.async_create_task(
            self._listen_events(), "ha_voip_event_stream"
        )

    async def _listen_events(self) -> None:
        """Long-running coroutine that consumes engine events via gRPC streaming."""
        _LOGGER.debug("Event stream listener started")
        reconnect_delay = 2.0
        max_reconnect_delay = 60.0

        while True:
            try:
                if not self.grpc_client.connected:
                    await self.grpc_client.connect()

                # In production this would call a streaming RPC:
                #   async for event in self._stub.StreamEvents(request):
                #       self._dispatch_event(event)
                # For now we just sleep and let the poll cycle handle updates.
                await asyncio.sleep(UPDATE_INTERVAL_SECONDS)
                reconnect_delay = 2.0  # reset on success

            except asyncio.CancelledError:
                _LOGGER.debug("Event stream listener cancelled")
                return
            except Exception as exc:  # noqa: BLE001
                _LOGGER.warning(
                    "Event stream error, reconnecting in %.0fs: %s",
                    reconnect_delay,
                    exc,
                )
                await asyncio.sleep(reconnect_delay)
                reconnect_delay = min(
                    reconnect_delay * 2, max_reconnect_delay
                )

    @callback
    def _dispatch_event(self, event: dict[str, Any]) -> None:
        """Dispatch a single engine event onto the HA event bus."""
        event_type = event.get("type", "")
        event_data = event.get("data", {})

        mapping = {
            "call_started": EVENT_CALL_STARTED,
            "call_ringing": EVENT_CALL_RINGING,
            "call_answered": EVENT_CALL_ANSWERED,
            "call_ended": EVENT_CALL_ENDED,
            "call_held": EVENT_CALL_HELD,
            "call_resumed": EVENT_CALL_RESUMED,
            "call_transferred": EVENT_CALL_TRANSFERRED,
            "registration_changed": EVENT_REGISTRATION_CHANGED,
            "dtmf_received": EVENT_DTMF_RECEIVED,
        }

        ha_event = mapping.get(event_type)
        if ha_event:
            self.hass.bus.async_fire(ha_event, event_data)
            # Trigger an immediate data refresh so entities update quickly
            self.async_set_updated_data(self.data)
        else:
            _LOGGER.debug("Unknown engine event type: %s", event_type)
