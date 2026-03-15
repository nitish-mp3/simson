"""HA VoIP integration for Home Assistant.

This custom component provides full VoIP (Voice over IP) functionality:
- SIP extension management
- Call control (make, answer, hang up, transfer, hold)
- WebRTC media relay via a bundled or remote voip-engine
- Real-time call state sensors and presence binary sensors
- WebSocket API for the frontend card
"""

from __future__ import annotations

import logging
from typing import Any

from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant

from .const import (
    CONF_ENGINE_MODE,
    DATA_COORDINATOR,
    DATA_ENGINE_MANAGER,
    DATA_UNSUB_LISTENERS,
    DOMAIN,
    ENGINE_MODE_LOCAL,
    PLATFORMS,
)
from .coordinator import VoipDataUpdateCoordinator
from .engine_manager import EngineManager
from .services import async_register_services, async_unregister_services
from .websocket_api import async_register_websocket_api

_LOGGER = logging.getLogger(__name__)

# Type alias for the dict stored under hass.data[DOMAIN][entry.entry_id]
type HaVoipData = dict[str, Any]


# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------


async def async_setup_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Set up HA VoIP from a config entry.

    This is the primary entry point called by Home Assistant when the
    integration is loaded.
    """
    _LOGGER.info("Setting up HA VoIP integration (entry=%s)", entry.entry_id)

    hass.data.setdefault(DOMAIN, {})

    # 1. Engine process management (local mode only)
    engine_manager: EngineManager | None = None
    if entry.data.get(CONF_ENGINE_MODE) == ENGINE_MODE_LOCAL:
        engine_manager = EngineManager(hass, entry)
        try:
            await engine_manager.async_start()
        except Exception:
            _LOGGER.exception("Failed to start voip-engine")
            # We still continue -- the coordinator will report it as
            # unreachable and the user can fix paths / restart.

    # 2. Data update coordinator (gRPC polling + event streaming)
    coordinator = VoipDataUpdateCoordinator(hass, entry)
    await coordinator.async_connect()
    await coordinator.async_config_entry_first_refresh()

    # 3. Store references
    hass.data[DOMAIN] = {
        DATA_COORDINATOR: coordinator,
        DATA_ENGINE_MANAGER: engine_manager,
        DATA_UNSUB_LISTENERS: [],
    }

    # 4. Forward platform setup (sensor, binary_sensor)
    await hass.config_entries.async_forward_entry_setups(entry, PLATFORMS)

    # 5. Register services
    await async_register_services(hass)

    # 6. Register WebSocket API commands
    async_register_websocket_api(hass)

    # 7. Listen for options updates so we can reconfigure at runtime
    unsub_options = entry.add_update_listener(_async_options_updated)
    hass.data[DOMAIN][DATA_UNSUB_LISTENERS].append(unsub_options)

    _LOGGER.info("HA VoIP integration setup complete")
    return True


async def _async_options_updated(
    hass: HomeAssistant, entry: ConfigEntry
) -> None:
    """Handle options update -- reload the integration."""
    _LOGGER.info("HA VoIP options changed, reloading")
    await hass.config_entries.async_reload(entry.entry_id)


# ---------------------------------------------------------------------------
# Unload
# ---------------------------------------------------------------------------


async def async_unload_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Unload a HA VoIP config entry."""
    _LOGGER.info("Unloading HA VoIP integration (entry=%s)", entry.entry_id)

    # 1. Unload platforms
    unload_ok = await hass.config_entries.async_unload_platforms(entry, PLATFORMS)
    if not unload_ok:
        return False

    # 2. Cancel event listeners
    domain_data: HaVoipData | None = hass.data.get(DOMAIN)
    if domain_data:
        for unsub in domain_data.get(DATA_UNSUB_LISTENERS, []):
            unsub()

        # 3. Disconnect coordinator
        coordinator: VoipDataUpdateCoordinator | None = domain_data.get(
            DATA_COORDINATOR
        )
        if coordinator:
            await coordinator.async_disconnect()

        # 4. Stop engine (local mode)
        engine_mgr: EngineManager | None = domain_data.get(DATA_ENGINE_MANAGER)
        if engine_mgr:
            await engine_mgr.async_stop()

    # 5. Unregister services
    await async_unregister_services(hass)

    # 6. Clean up hass.data
    hass.data.pop(DOMAIN, None)

    _LOGGER.info("HA VoIP integration unloaded")
    return True
