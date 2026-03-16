"""VoIP engine process manager.

Responsible for starting, stopping, and health-monitoring the voip-engine
binary.  Handles configuration generation, log forwarding, and automatic
restart on failure.
"""

from __future__ import annotations

import asyncio
import json
import logging
import os
import platform
import signal
import sys
from pathlib import Path
from typing import Any

from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant

from .const import (
    CERT_MODE_ACME,
    CERT_MODE_MANUAL,
    CERT_MODE_SELF_SIGNED,
    CONF_ACME_DOMAIN,
    CONF_ACME_EMAIL,
    CONF_CERT_MODE,
    CONF_CERT_PATH,
    CONF_DEFAULT_CODEC,
    CONF_ENABLE_RECORDING,
    CONF_ENGINE_BINARY_PATH,
    CONF_EXTENSIONS,
    CONF_EXTERNAL_HOST,
    CONF_GRPC_PORT,
    CONF_KEY_PATH,
    CONF_LOG_LEVEL,
    CONF_RECORDING_PATH,
    CONF_RTP_PORT_END,
    CONF_RTP_PORT_START,
    CONF_SIP_PORT,
    CONF_STUN_SERVER,
    CONF_TURN_PASSWORD,
    CONF_TURN_SERVER,
    CONF_TURN_USERNAME,
    CONF_WS_PORT,
    DEFAULT_ENGINE_HOST,
    DEFAULT_GRPC_PORT,
    DEFAULT_LOG_LEVEL,
    DEFAULT_RECORDING_PATH,
    DEFAULT_RTP_PORT_END,
    DEFAULT_RTP_PORT_START,
    DEFAULT_SIP_PORT,
    DEFAULT_STUN_SERVER,
    DEFAULT_WS_PORT,
    DOMAIN,
    ENGINE_HEALTH_CHECK_INTERVAL,
    ENGINE_MAX_RESTART_ATTEMPTS,
    ENGINE_RESTART_DELAY,
    ENGINE_STATE_ERROR,
    ENGINE_STATE_RESTARTING,
    ENGINE_STATE_RUNNING,
    ENGINE_STATE_STARTING,
    ENGINE_STATE_STOPPED,
    EVENT_ENGINE_STATE_CHANGED,
)

_LOGGER = logging.getLogger(__name__)


class EngineManager:
    """Manage the lifecycle of the voip-engine subprocess."""

    def __init__(
        self,
        hass: HomeAssistant,
        config_entry: ConfigEntry,
    ) -> None:
        """Initialize the engine manager."""
        self._hass = hass
        self._config_entry = config_entry
        self._process: asyncio.subprocess.Process | None = None
        self._state: str = ENGINE_STATE_STOPPED
        self._restart_count: int = 0
        self._health_task: asyncio.Task[None] | None = None
        self._log_task: asyncio.Task[None] | None = None
        self._should_run: bool = False

    # -- Properties -----------------------------------------------------------

    @property
    def state(self) -> str:
        """Return the current engine state."""
        return self._state

    @property
    def is_running(self) -> bool:
        """Return True if the engine process is alive."""
        return (
            self._process is not None
            and self._process.returncode is None
        )

    # -- Binary detection -----------------------------------------------------

    def _resolve_binary_path(self) -> str:
        """Locate the voip-engine binary.

        Lookup order:
        1. Explicit path in config / options.
        2. Add-on path (``/usr/share/hassio/addons/local/ha_voip/voip-engine``).
        3. Bundled alongside the custom component (``<component_dir>/../voip-engine/target/release/voip-engine``).
        4. Anywhere on ``$PATH`` (``voip-engine``).
        """
        # 1 -- explicit config
        explicit = (
            self._config_entry.options.get(CONF_ENGINE_BINARY_PATH)
            or self._config_entry.data.get(CONF_ENGINE_BINARY_PATH)
        )
        if explicit and os.path.isfile(explicit):
            return explicit

        # 2 -- add-on path
        addon_path = "/usr/share/hassio/addons/local/ha_voip/voip-engine"
        if os.path.isfile(addon_path):
            return addon_path

        # 3 -- sibling directory of custom_components
        component_dir = Path(__file__).resolve().parent
        sibling = component_dir.parents[1] / "voip-engine" / "target" / "release"
        suffix = ".exe" if platform.system() == "Windows" else ""
        candidate = sibling / f"voip-engine{suffix}"
        if candidate.is_file():
            return str(candidate)

        # 4 -- $PATH
        return "voip-engine"

    # -- Config generation for the engine binary ------------------------------

    def _build_engine_config(self) -> dict[str, Any]:
        """Generate a JSON config dict consumed by ``voip-engine --config``."""
        data = self._config_entry.data
        opts = self._config_entry.options

        def _get(key: str, default: Any = None) -> Any:
            return opts.get(key, data.get(key, default))

        config: dict[str, Any] = {
            "sip": {
                "port": _get(CONF_SIP_PORT, DEFAULT_SIP_PORT),
                "external_host": _get(CONF_EXTERNAL_HOST, ""),
            },
            "rtp": {
                "port_range_start": _get(CONF_RTP_PORT_START, DEFAULT_RTP_PORT_START),
                "port_range_end": _get(CONF_RTP_PORT_END, DEFAULT_RTP_PORT_END),
            },
            "grpc": {
                "host": DEFAULT_ENGINE_HOST,
                "port": _get(CONF_GRPC_PORT, DEFAULT_GRPC_PORT),
            },
            "websocket": {
                "port": _get(CONF_WS_PORT, DEFAULT_WS_PORT),
            },
            "media": {
                "default_codec": _get(CONF_DEFAULT_CODEC, "opus"),
                "recording_enabled": _get(CONF_ENABLE_RECORDING, False),
                "recording_path": _get(CONF_RECORDING_PATH, DEFAULT_RECORDING_PATH),
            },
            "network": {
                "stun_server": _get(CONF_STUN_SERVER, DEFAULT_STUN_SERVER),
                "turn_server": _get(CONF_TURN_SERVER, ""),
                "turn_username": _get(CONF_TURN_USERNAME, ""),
                "turn_password": _get(CONF_TURN_PASSWORD, ""),
            },
            "logging": {
                "level": _get(CONF_LOG_LEVEL, DEFAULT_LOG_LEVEL),
            },
            "extensions": _get(CONF_EXTENSIONS, []),
        }

        # TLS / certificate block
        cert_mode = _get(CONF_CERT_MODE, CERT_MODE_SELF_SIGNED)
        tls: dict[str, Any] = {"mode": cert_mode}
        if cert_mode == CERT_MODE_MANUAL:
            tls["cert_path"] = _get(CONF_CERT_PATH, "")
            tls["key_path"] = _get(CONF_KEY_PATH, "")
        elif cert_mode == CERT_MODE_ACME:
            tls["acme_domain"] = _get(CONF_ACME_DOMAIN, "")
            tls["acme_email"] = _get(CONF_ACME_EMAIL, "")
        config["tls"] = tls

        return config

    def _write_config_file(self) -> str:
        """Write the engine config to a temporary JSON file and return its path."""
        config_dir = Path(self._hass.config.path("voip_engine"))
        config_dir.mkdir(parents=True, exist_ok=True)
        config_path = config_dir / "engine_config.json"

        config = self._build_engine_config()
        config_path.write_text(json.dumps(config, indent=2), encoding="utf-8")
        _LOGGER.debug("Wrote engine config to %s", config_path)
        return str(config_path)

    # -- Start / Stop ---------------------------------------------------------

    async def async_start(self) -> None:
        """Start the voip-engine subprocess."""
        if self.is_running:
            _LOGGER.warning("Engine is already running")
            return

        self._should_run = True
        self._set_state(ENGINE_STATE_STARTING)
        self._restart_count = 0

        binary = await self._hass.async_add_executor_job(
            self._resolve_binary_path
        )
        config_path = await self._hass.async_add_executor_job(
            self._write_config_file
        )

        _LOGGER.info("Starting voip-engine: %s --config %s", binary, config_path)

        try:
            self._process = await asyncio.create_subprocess_exec(
                binary,
                "--config",
                config_path,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
            )
        except FileNotFoundError:
            _LOGGER.info(
                "voip-engine binary not found at '%s'; "
                "assuming engine is already running as an HA add-on.",
                binary,
            )
            self._set_state(ENGINE_STATE_RUNNING)
            return
        except OSError as exc:
            _LOGGER.error("Failed to start voip-engine: %s", exc)
            self._set_state(ENGINE_STATE_ERROR)
            return

        self._set_state(ENGINE_STATE_RUNNING)
        _LOGGER.info("voip-engine started (pid=%s)", self._process.pid)

        # Start background tasks
        self._log_task = self._hass.async_create_task(
            self._forward_logs(), "ha_voip_log_forward"
        )
        self._health_task = self._hass.async_create_task(
            self._health_loop(), "ha_voip_health_check"
        )

    async def async_stop(self) -> None:
        """Gracefully stop the voip-engine subprocess."""
        self._should_run = False

        # Cancel background tasks
        for task in (self._health_task, self._log_task):
            if task is not None:
                task.cancel()
                try:
                    await task
                except asyncio.CancelledError:
                    pass
        self._health_task = None
        self._log_task = None

        if self._process is None or self._process.returncode is not None:
            self._set_state(ENGINE_STATE_STOPPED)
            return

        _LOGGER.info("Stopping voip-engine (pid=%s)", self._process.pid)

        try:
            if sys.platform == "win32":
                self._process.terminate()
            else:
                self._process.send_signal(signal.SIGTERM)
            await asyncio.wait_for(self._process.wait(), timeout=10.0)
        except asyncio.TimeoutError:
            _LOGGER.warning("voip-engine did not exit in time, killing")
            self._process.kill()
            await self._process.wait()
        except ProcessLookupError:
            pass

        self._set_state(ENGINE_STATE_STOPPED)
        _LOGGER.info("voip-engine stopped")

    async def async_restart(self) -> None:
        """Restart the engine (stop then start)."""
        self._set_state(ENGINE_STATE_RESTARTING)
        await self.async_stop()
        self._should_run = True
        await asyncio.sleep(ENGINE_RESTART_DELAY)
        await self.async_start()

    # -- Health monitoring ----------------------------------------------------

    async def _health_loop(self) -> None:
        """Periodically check whether the engine process is still alive."""
        try:
            while self._should_run:
                await asyncio.sleep(ENGINE_HEALTH_CHECK_INTERVAL)
                if not self.is_running and self._should_run:
                    _LOGGER.warning(
                        "voip-engine exited unexpectedly (code=%s)",
                        self._process.returncode if self._process else "?",
                    )
                    await self._attempt_restart()
        except asyncio.CancelledError:
            return

    async def _attempt_restart(self) -> None:
        """Try to restart the engine up to MAX_RESTART_ATTEMPTS times."""
        if self._restart_count >= ENGINE_MAX_RESTART_ATTEMPTS:
            _LOGGER.error(
                "voip-engine has crashed %d times; giving up",
                self._restart_count,
            )
            self._set_state(ENGINE_STATE_ERROR)
            self._should_run = False
            return

        self._restart_count += 1
        delay = ENGINE_RESTART_DELAY * self._restart_count
        _LOGGER.info(
            "Restarting voip-engine (attempt %d/%d) in %ds",
            self._restart_count,
            ENGINE_MAX_RESTART_ATTEMPTS,
            delay,
        )
        self._set_state(ENGINE_STATE_RESTARTING)
        await asyncio.sleep(delay)
        await self.async_start()

    # -- Log forwarding -------------------------------------------------------

    async def _forward_logs(self) -> None:
        """Read engine stdout/stderr and forward to the HA logger."""
        if self._process is None:
            return

        async def _read_stream(
            stream: asyncio.StreamReader | None, level: int
        ) -> None:
            if stream is None:
                return
            while True:
                line = await stream.readline()
                if not line:
                    break
                decoded = line.decode("utf-8", errors="replace").rstrip()
                if decoded:
                    _LOGGER.log(level, "[voip-engine] %s", decoded)

        try:
            await asyncio.gather(
                _read_stream(self._process.stdout, logging.INFO),
                _read_stream(self._process.stderr, logging.WARNING),
            )
        except asyncio.CancelledError:
            return

    # -- Helpers --------------------------------------------------------------

    def _set_state(self, new_state: str) -> None:
        """Update internal state and fire an HA event."""
        old = self._state
        self._state = new_state
        if old != new_state:
            self._hass.bus.async_fire(
                EVENT_ENGINE_STATE_CHANGED,
                {"previous_state": old, "new_state": new_state},
            )
