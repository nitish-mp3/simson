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
from pathlib import Path
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

# Internal registration guards
_FRONTEND_REGISTERED = False
_WS_COMMANDS_REGISTERED = False

_CARD_URL = f"/{DOMAIN}/voip-card.js"
_JSSIP_URL = f"/{DOMAIN}/jssip.min.js"
_PHONE_URL = f"/{DOMAIN}/phone.html"
_PANEL_PATH = "voip-phone"


def _register_frontend(hass: HomeAssistant) -> None:
    """Serve the www/ folder, register Lovelace resource, and add sidebar panel."""
    global _FRONTEND_REGISTERED  # noqa: PLW0603
    if _FRONTEND_REGISTERED:
        return
    _FRONTEND_REGISTERED = True

    www = Path(__file__).parent / "www"

    # Register voip-card.js for Lovelace
    js_path = str(www / "voip-card.js")
    if Path(js_path).is_file():
        hass.http.register_static_path(_CARD_URL, js_path, cache_headers=False)
        _LOGGER.info("Registered Lovelace card at %s", _CARD_URL)
        hass.async_create_task(_async_add_lovelace_resource(hass))
    else:
        _LOGGER.warning("voip-card.js not found at %s", js_path)

    # Register jssip.min.js (bundled locally — no CDN dependency)
    jssip_path = str(www / "jssip.min.js")
    if Path(jssip_path).is_file():
        hass.http.register_static_path(_JSSIP_URL, jssip_path, cache_headers=True)
        _LOGGER.info("Registered JsSIP library at %s", _JSSIP_URL)
    else:
        _LOGGER.warning(
            "jssip.min.js not found at %s — phone.html SIP features unavailable",
            jssip_path,
        )

    # Register standalone phone.html and add it as a sidebar panel
    html_path = str(www / "phone.html")
    if Path(html_path).is_file():
        hass.http.register_static_path(_PHONE_URL, html_path, cache_headers=False)
        _LOGGER.info("Registered phone UI at %s", _PHONE_URL)
        try:
            from homeassistant.components import frontend  # noqa: PLC0415
            frontend.async_register_built_in_panel(
                hass,
                component_name="iframe",
                sidebar_title="VoIP Phone",
                sidebar_icon="mdi:phone",
                frontend_url_path=_PANEL_PATH,
                config={"url": _PHONE_URL},
                require_admin=False,
            )
            _LOGGER.info("Registered VoIP Phone sidebar panel at /%s", _PANEL_PATH)
        except Exception:  # noqa: BLE001
            _LOGGER.debug("Could not register sidebar panel — phone available at %s", _PHONE_URL)
    else:
        _LOGGER.warning("phone.html not found at %s", html_path)


async def _async_add_lovelace_resource(hass: HomeAssistant) -> None:
    """Add the card JS as a Lovelace resource if not already present."""
    try:
        from homeassistant.components.lovelace import (  # noqa: PLC0415
            ResourceStorageCollection,
        )
        from homeassistant.components.lovelace.const import (  # noqa: PLC0415
            DOMAIN as LL_DOMAIN,
        )

        ll_data = hass.data.get(LL_DOMAIN)
        if ll_data is None:
            return
        resources: ResourceStorageCollection | None = getattr(ll_data, "resources", None)
        if resources is None:
            return

        # Check if already registered
        for item in resources.async_items():
            if item.get("url", "").endswith("voip-card.js"):
                return

        await resources.async_create_item({"res_type": "module", "url": _CARD_URL})
        _LOGGER.info("Registered Lovelace resource %s", _CARD_URL)
    except Exception:  # noqa: BLE001
        _LOGGER.debug(
            "Could not auto-register Lovelace resource — add manually: "
            "Settings → Dashboards → Resources → %s (JavaScript Module)",
            _CARD_URL,
        )


# ---------------------------------------------------------------------------
# Setup
# ---------------------------------------------------------------------------


async def async_setup(hass: HomeAssistant, config: dict) -> bool:
    """Set up the HA VoIP component."""
    hass.data.setdefault(DOMAIN, {})

    # 0. Serve the Lovelace card JS from custom_components/<domain>/www/
    _register_frontend(hass)

    # Register WebSocket API commands once so the card can connect even if the
    # integration reloads or an entry fails to fully initialize.
    global _WS_COMMANDS_REGISTERED  # noqa: PLW0603
    if not _WS_COMMANDS_REGISTERED:
        async_register_websocket_api(hass)
        _WS_COMMANDS_REGISTERED = True

    return True


async def async_setup_entry(hass: HomeAssistant, entry: ConfigEntry) -> bool:
    """Set up HA VoIP from a config entry.

    This is the primary entry point called by Home Assistant when the
    integration is loaded.
    """
    _LOGGER.info("Setting up HA VoIP integration (entry=%s)", entry.entry_id)

    hass.data.setdefault(DOMAIN, {})

    # Ensure WebSocket commands are registered even if the integration
    # is reloaded multiple times.
    global _WS_COMMANDS_REGISTERED  # noqa: PLW0603
    if not _WS_COMMANDS_REGISTERED:
        async_register_websocket_api(hass)
        _WS_COMMANDS_REGISTERED = True

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

    # 2. Data update coordinator (HTTP health polling + event monitoring)
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

    # 6. Listen for options updates so we can reconfigure at runtime
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

    # 6. Remove sidebar panel
    try:
        from homeassistant.components import frontend  # noqa: PLC0415
        frontend.async_remove_panel(hass, _PANEL_PATH)
    except Exception:  # noqa: BLE001
        pass

    # 7. Reset frontend registration flag so reload re-registers
    global _FRONTEND_REGISTERED  # noqa: PLW0603
    _FRONTEND_REGISTERED = False

    # 8. Clean up hass.data
    hass.data.pop(DOMAIN, None)

    _LOGGER.info("HA VoIP integration unloaded")
    return True
