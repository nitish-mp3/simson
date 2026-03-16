"""Data update coordinator for HA VoIP integration.

Polls the voip-engine for status via HTTP health endpoints, caches active calls,
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
    CONF_EXTERNAL_HOST,
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
# Engine HTTP client
# ---------------------------------------------------------------------------


class EngineClient:
    """Async HTTP client for communicating with the voip-engine.

    Uses the engine's HTTP health/metrics endpoints (port 8080) for status
    monitoring and REST-style endpoints for call-control and data queries.
    """

    HTTP_PORT = 8080

    def __init__(self, host: str, port: int) -> None:
        self._host = host
        self._port = port
        self._connected = False
        self._session: Any | None = None
        self._last_health: dict[str, Any] = {}

    # -- lifecycle -----------------------------------------------------------

    async def _get_session(self) -> Any:
        """Return a reusable aiohttp session."""
        import aiohttp  # type: ignore[import-untyped]

        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession(
                timeout=aiohttp.ClientTimeout(total=5.0)
            )
        return self._session

    def _url(self, path: str) -> str:
        """Build an engine HTTP URL."""
        return f"http://{self._host}:{self.HTTP_PORT}{path}"

    async def connect(self) -> None:
        """Verify engine is reachable via its HTTP health endpoint."""
        session = await self._get_session()

        candidates = [self._host]
        if "127.0.0.1" not in candidates:
            candidates.append("127.0.0.1")

        for candidate in candidates:
            url = f"http://{candidate}:{self.HTTP_PORT}/health/live"
            try:
                async with session.get(url) as resp:
                    if resp.status == 200:
                        self._host = candidate
                        self._connected = True
                        try:
                            self._last_health = await resp.json(
                                content_type=None
                            )
                        except Exception:  # noqa: BLE001
                            self._last_health = {}
                        _LOGGER.info("Connected to voip-engine at %s", url)
                        return
            except Exception:  # noqa: BLE001
                continue

        _LOGGER.warning(
            "Cannot reach voip-engine health endpoint on any candidate: %s",
            candidates,
        )
        self._connected = False

    async def disconnect(self) -> None:
        """Clean up the HTTP session."""
        if self._session and not self._session.closed:
            await self._session.close()
        self._session = None
        self._connected = False

    @property
    def connected(self) -> bool:
        """Return True when the engine appears healthy."""
        return self._connected

    # -- HTTP helpers --------------------------------------------------------

    async def _get_json(self, path: str) -> dict[str, Any] | None:
        """GET a JSON endpoint; return parsed dict or None on failure."""
        session = await self._get_session()
        try:
            async with session.get(self._url(path)) as resp:
                if resp.status == 200:
                    self._connected = True
                    return await resp.json(content_type=None)
                _LOGGER.debug("Engine %s returned HTTP %s", path, resp.status)
                return None
        except Exception as exc:  # noqa: BLE001
            _LOGGER.debug("Engine %s failed: %s", path, exc)
            return None

    async def _post_json(
        self, path: str, payload: dict[str, Any]
    ) -> dict[str, Any] | None:
        """POST JSON to an engine endpoint; return parsed response or None."""
        session = await self._get_session()
        try:
            async with session.post(
                self._url(path), json=payload
            ) as resp:
                if resp.status in (200, 201):
                    self._connected = True
                    return await resp.json(content_type=None)
                _LOGGER.debug(
                    "Engine POST %s returned HTTP %s", path, resp.status
                )
                return None
        except Exception as exc:  # noqa: BLE001
            _LOGGER.debug("Engine POST %s failed: %s", path, exc)
            return None

    # -- Status / health -----------------------------------------------------

    async def get_status(self) -> dict[str, Any]:
        """Return engine status from /health/ready."""
        session = await self._get_session()
        url = self._url("/health/ready")
        try:
            async with session.get(url) as resp:
                if resp.status == 200:
                    self._connected = True
                    data = await resp.json(content_type=None)
                    self._last_health = data or {}
                    return {
                        "state": ENGINE_STATE_RUNNING,
                        "version": (data or {}).get("version", ""),
                        "uptime_seconds": (data or {}).get(
                            "uptime_seconds", 0
                        ),
                    }
                self._connected = False
                raise ConnectionError(
                    f"Engine health check returned HTTP {resp.status}"
                )
        except ConnectionError:
            raise
        except Exception as exc:  # noqa: BLE001
            self._connected = False
            raise ConnectionError(f"Health check failed: {exc}") from exc

    # -- Call queries --------------------------------------------------------

    async def get_calls(self) -> list[dict[str, Any]]:
        """Fetch active calls from the engine."""
        if not self._connected:
            return []
        data = await self._get_json("/api/calls")
        if data is None:
            return []
        if isinstance(data, list):
            return data
        return data.get("calls", [])

    async def get_extensions(self) -> list[dict[str, Any]]:
        """Fetch registered extensions from the engine."""
        if not self._connected:
            return []
        data = await self._get_json("/api/extensions")
        if data is None:
            return []
        if isinstance(data, list):
            return data
        return data.get("extensions", [])

    async def get_call_history(self, limit: int = 50) -> list[dict[str, Any]]:
        """Fetch call history from the engine."""
        if not self._connected:
            return []
        data = await self._get_json(f"/api/call-history?limit={limit}")
        if data is None:
            return []
        if isinstance(data, list):
            return data
        return data.get("calls", data.get("history", []))

    # -- Call control --------------------------------------------------------

    async def make_call(
        self,
        target: str,
        from_extension: str,
        caller_id: str = "",
    ) -> dict[str, Any]:
        """Initiate a call via the engine."""
        if not self._connected:
            raise ConnectionError("Engine is not connected")
        result = await self._post_json(
            "/api/calls",
            {
                "from_extension": from_extension,
                "to_extension": target,
                "caller_id": caller_id,
            },
        )
        if result is None:
            raise ConnectionError("Engine did not accept the call request")
        return {
            "call_id": result.get("call_id", ""),
            "status": result.get("status", "initiated"),
        }

    async def hangup_call(self, call_id: str) -> bool:
        """Hang up a call."""
        if not self._connected:
            return False
        result = await self._post_json(
            f"/api/calls/{call_id}/hangup", {"call_id": call_id}
        )
        return result is not None

    async def transfer_call(self, call_id: str, target: str) -> bool:
        """Transfer a call to another extension."""
        if not self._connected:
            return False
        result = await self._post_json(
            f"/api/calls/{call_id}/transfer",
            {"call_id": call_id, "to_extension": target},
        )
        return result is not None

    async def toggle_recording(self, call_id: str) -> bool:
        """Toggle call recording."""
        if not self._connected:
            return False
        result = await self._post_json(
            f"/api/calls/{call_id}/recording", {"call_id": call_id}
        )
        return result is not None

    async def toggle_mute(self, call_id: str) -> bool:
        """Toggle call mute."""
        if not self._connected:
            return False
        result = await self._post_json(
            f"/api/calls/{call_id}/mute", {"call_id": call_id}
        )
        return result is not None

    async def send_dtmf(self, call_id: str, digits: str) -> bool:
        """Send DTMF tones on a call."""
        if not self._connected:
            return False
        result = await self._post_json(
            f"/api/calls/{call_id}/dtmf",
            {"call_id": call_id, "digits": digits},
        )
        return result is not None

    async def relay_sdp(
        self, call_id: str, sdp: str, sdp_type: str
    ) -> dict[str, Any]:
        """Relay an SDP offer/answer to the engine."""
        if not self._connected:
            raise ConnectionError("Engine is not connected")
        result = await self._post_json(
            "/api/webrtc/sdp",
            {"call_id": call_id, "sdp": sdp, "sdp_type": sdp_type},
        )
        return result or {}

    async def relay_ice_candidate(
        self, call_id: str, candidate: str, sdp_mid: str, sdp_mline_index: int
    ) -> bool:
        """Relay an ICE candidate to the engine."""
        if not self._connected:
            return False
        result = await self._post_json(
            "/api/webrtc/ice",
            {
                "call_id": call_id,
                "candidate": candidate,
                "sdp_mid": sdp_mid,
                "sdp_mline_index": sdp_mline_index,
            },
        )
        return result is not None

    async def answer_call(self, call_id: str) -> bool:
        """Answer an incoming call."""
        if not self._connected:
            return False
        result = await self._post_json(f"/api/calls/{call_id}/answer", {})
        return result is not None

    async def hold_call(self, call_id: str, hold: bool = True) -> bool:
        """Hold or resume a call."""
        if not self._connected:
            return False
        result = await self._post_json(
            f"/api/calls/{call_id}/hold", {"hold": hold}
        )
        return result is not None


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
        # Explicit remote-mode host takes priority; fall back to the external_host
        # the user set in the config flow (their LAN IP), which the add-on's
        # host_network services are accessible at from the HA core container.
        host = (
            config_entry.data.get(CONF_ENGINE_HOST)
            or config_entry.options.get(CONF_ENGINE_HOST)
            or config_entry.data.get(CONF_EXTERNAL_HOST)
            or config_entry.options.get(CONF_EXTERNAL_HOST)
        )
        if not host or host in ("", "0.0.0.0"):
            host = DEFAULT_ENGINE_HOST
        port = config_entry.data.get(CONF_GRPC_PORT, DEFAULT_GRPC_PORT)
        self.engine_client = EngineClient(host, port)
        # Backward-compatible alias so websocket_api.py / services.py still work
        self.grpc_client = self.engine_client
        self._event_task: asyncio.Task[None] | None = None
        self._previous_engine_state: str = ENGINE_STATE_STOPPED

    # -- Public helpers -------------------------------------------------------

    async def async_connect(self) -> None:
        """Open the health-check connection and start the event stream."""
        await self.engine_client.connect()
        self._start_event_stream()

    async def async_disconnect(self) -> None:
        """Tear down connection and background tasks."""
        if self._event_task is not None:
            self._event_task.cancel()
            try:
                await self._event_task
            except asyncio.CancelledError:
                pass
            self._event_task = None
        await self.engine_client.disconnect()

    # -- DataUpdateCoordinator override ---------------------------------------

    async def _async_update_data(self) -> VoipData:
        """Fetch latest state from voip-engine."""
        data = VoipData()

        # Engine status
        try:
            raw_status = await self.engine_client.get_status()
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
            raise UpdateFailed("Cannot reach voip-engine health endpoint") from None

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
            raw_calls = await self.engine_client.get_calls()
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
            raw_exts = await self.engine_client.get_extensions()
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
            data.call_history = await self.engine_client.get_call_history()
        except Exception as exc:  # noqa: BLE001
            _LOGGER.debug("Failed to fetch call history: %s", exc)

        return data

    # -- Event stream ---------------------------------------------------------

    def _start_event_stream(self) -> None:
        """Start a background task that listens to the engine event stream."""
        if self._event_task is not None:
            return
        self._event_task = self.hass.async_create_task(
            self._listen_events(), "ha_voip_event_stream"
        )

    async def _listen_events(self) -> None:
        """Long-running coroutine that monitors engine connectivity."""
        _LOGGER.debug("Event stream listener started")
        reconnect_delay = 2.0
        max_reconnect_delay = 60.0

        while True:
            try:
                if not self.engine_client.connected:
                    await self.engine_client.connect()
                    reconnect_delay = 2.0

                # In production this would consume a streaming RPC.
                # For now we just sleep — the poll cycle handles updates.
                await asyncio.sleep(max_reconnect_delay)

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
