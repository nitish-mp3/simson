"""Call state sensors and presence binary sensors for HA VoIP.

Provides:
- VoipCallStateSensor  -- per-extension call state (idle/ringing/in_call/...)
- VoipPresenceSensor   -- binary sensor for extension availability
- VoipCallHistorySensor -- sensor with recent call history as attribute
"""

from __future__ import annotations

import logging
from datetime import datetime, timezone
from typing import Any

from homeassistant.components.binary_sensor import (
    BinarySensorDeviceClass,
    BinarySensorEntity,
)
from homeassistant.components.sensor import (
    SensorDeviceClass,
    SensorEntity,
    SensorStateClass,
)
from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant, callback
from homeassistant.helpers.entity_platform import AddEntitiesCallback

from ..const import (
    CALL_STATE_IDLE,
    CALL_STATE_IN_CALL,
    CALL_STATE_ON_HOLD,
    CALL_STATE_RINGING,
    CALL_STATE_TRANSFERRING,
    CONF_EXTENSIONS,
    DOMAIN,
    ENTITY_PREFIX_CALL,
    ENTITY_PREFIX_PRESENCE,
)
from ..coordinator import CallInfo, VoipData, VoipDataUpdateCoordinator
from .extension import VoipExtensionDevice

_LOGGER = logging.getLogger(__name__)

# Valid call states for the state machine
CALL_STATES: list[str] = [
    CALL_STATE_IDLE,
    CALL_STATE_RINGING,
    CALL_STATE_IN_CALL,
    CALL_STATE_ON_HOLD,
    CALL_STATE_TRANSFERRING,
]


# ---------------------------------------------------------------------------
# Platform setup helpers (called from sensor.py / binary_sensor.py)
# ---------------------------------------------------------------------------


async def async_setup_call_sensors(
    hass: HomeAssistant,
    config_entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Create VoipCallStateSensor + VoipCallHistorySensor for each extension."""
    coordinator: VoipDataUpdateCoordinator = hass.data[DOMAIN]["coordinator"]
    extensions = config_entry.data.get(CONF_EXTENSIONS, [])

    entities: list[SensorEntity] = []
    for ext in extensions:
        number = ext["number"]
        name = ext.get("name", number)
        entities.append(
            VoipCallStateSensor(coordinator, config_entry, number, name)
        )

    # Single call history sensor (not per-extension)
    entities.append(VoipCallHistorySensor(coordinator, config_entry))

    async_add_entities(entities, update_before_add=True)


async def async_setup_presence_sensors(
    hass: HomeAssistant,
    config_entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Create VoipPresenceSensor binary sensors for each extension."""
    coordinator: VoipDataUpdateCoordinator = hass.data[DOMAIN]["coordinator"]
    extensions = config_entry.data.get(CONF_EXTENSIONS, [])

    entities: list[BinarySensorEntity] = []
    for ext in extensions:
        number = ext["number"]
        name = ext.get("name", number)
        entities.append(
            VoipPresenceSensor(coordinator, config_entry, number, name)
        )
    async_add_entities(entities, update_before_add=True)


# ---------------------------------------------------------------------------
# VoipCallStateSensor
# ---------------------------------------------------------------------------


class VoipCallStateSensor(VoipExtensionDevice, SensorEntity):
    """Sensor that tracks the active call state for one extension.

    States: idle | ringing | in_call | on_hold | transferring
    Attributes: caller_id, callee_id, duration, codec, quality_score
    """

    _attr_icon = "mdi:phone"
    _attr_device_class = SensorDeviceClass.ENUM
    _attr_options = CALL_STATES

    def __init__(
        self,
        coordinator: VoipDataUpdateCoordinator,
        config_entry: ConfigEntry,
        extension_number: str,
        display_name: str,
    ) -> None:
        """Initialize the call state sensor."""
        super().__init__(coordinator, config_entry, extension_number, display_name)
        self._attr_unique_id = (
            f"{config_entry.entry_id}_{ENTITY_PREFIX_CALL}_{extension_number}"
        )
        self._attr_name = f"Call State {extension_number}"

    # -- State ----------------------------------------------------------------

    @property
    def native_value(self) -> str:
        """Return the current call state for this extension."""
        call = self._active_call
        if call is None:
            return CALL_STATE_IDLE
        return call.state

    @property
    def _active_call(self) -> CallInfo | None:
        """Find the active call for this extension (if any)."""
        data: VoipData | None = self.coordinator.data
        if data is None:
            return None
        for call in data.calls.values():
            if (
                call.from_extension == self._extension_number
                or call.to_extension == self._extension_number
            ):
                return call
        return None

    # -- Attributes -----------------------------------------------------------

    @property
    def extra_state_attributes(self) -> dict[str, Any]:
        """Return call-specific attributes."""
        attrs = super().extra_state_attributes
        call = self._active_call
        if call is not None:
            attrs.update(
                {
                    "call_id": call.call_id,
                    "caller_id": call.caller_id,
                    "callee_id": call.callee_id,
                    "duration": round(call.duration, 1),
                    "codec": call.codec,
                    "quality_score": round(call.quality_score, 2),
                    "is_recording": call.is_recording,
                    "is_muted": call.is_muted,
                    "start_time": (
                        datetime.fromtimestamp(call.start_time, tz=timezone.utc).isoformat()
                        if call.start_time
                        else None
                    ),
                    "answer_time": (
                        datetime.fromtimestamp(call.answer_time, tz=timezone.utc).isoformat()
                        if call.answer_time
                        else None
                    ),
                }
            )
        return attrs

    @property
    def icon(self) -> str:
        """Return a dynamic icon based on call state."""
        state = self.native_value
        icons = {
            CALL_STATE_IDLE: "mdi:phone",
            CALL_STATE_RINGING: "mdi:phone-ring",
            CALL_STATE_IN_CALL: "mdi:phone-in-talk",
            CALL_STATE_ON_HOLD: "mdi:phone-paused",
            CALL_STATE_TRANSFERRING: "mdi:phone-forward",
        }
        return icons.get(state, "mdi:phone")


# ---------------------------------------------------------------------------
# VoipPresenceSensor  (binary_sensor)
# ---------------------------------------------------------------------------


class VoipPresenceSensor(VoipExtensionDevice, BinarySensorEntity):
    """Binary sensor indicating whether an extension is registered (online).

    on  = extension registered / available
    off = extension not registered / offline
    """

    _attr_device_class = BinarySensorDeviceClass.CONNECTIVITY

    def __init__(
        self,
        coordinator: VoipDataUpdateCoordinator,
        config_entry: ConfigEntry,
        extension_number: str,
        display_name: str,
    ) -> None:
        """Initialize the presence sensor."""
        super().__init__(coordinator, config_entry, extension_number, display_name)
        self._attr_unique_id = (
            f"{config_entry.entry_id}_{ENTITY_PREFIX_PRESENCE}_{extension_number}"
        )
        self._attr_name = f"Presence {extension_number}"

    @property
    def is_on(self) -> bool:
        """Return True if the extension is registered."""
        return self.registered

    @property
    def icon(self) -> str:
        """Return icon based on presence."""
        return "mdi:account-check" if self.is_on else "mdi:account-off"


# ---------------------------------------------------------------------------
# VoipCallHistorySensor
# ---------------------------------------------------------------------------


class VoipCallHistorySensor(SensorEntity):
    """Sensor whose state is the count of recent calls.

    The ``call_history`` attribute contains the last N call records.
    """

    _attr_icon = "mdi:phone-log"
    _attr_name = "VoIP Call History"
    _attr_state_class = SensorStateClass.TOTAL_INCREASING
    _attr_has_entity_name = True

    def __init__(
        self,
        coordinator: VoipDataUpdateCoordinator,
        config_entry: ConfigEntry,
    ) -> None:
        """Initialize the call history sensor."""
        self.coordinator = coordinator
        self._config_entry = config_entry
        self._attr_unique_id = f"{config_entry.entry_id}_call_history"

    @property
    def device_info(self) -> dict[str, Any]:
        """Return device info for the engine device."""
        from ..const import MANUFACTURER, MODEL_ENGINE  # noqa: PLC0415

        return {
            "identifiers": {(DOMAIN, self._config_entry.entry_id)},
            "name": "VoIP Engine",
            "manufacturer": MANUFACTURER,
            "model": MODEL_ENGINE,
        }

    @property
    def native_value(self) -> int:
        """Return the total number of calls in history."""
        data: VoipData | None = self.coordinator.data
        if data is None:
            return 0
        return len(data.call_history)

    @property
    def extra_state_attributes(self) -> dict[str, Any]:
        """Return recent call history as an attribute list."""
        data: VoipData | None = self.coordinator.data
        history = data.call_history if data else []
        return {"call_history": history[:50]}

    @callback
    def _handle_coordinator_update(self) -> None:
        """Handle updated data from the coordinator."""
        self.async_write_ha_state()

    async def async_added_to_hass(self) -> None:
        """Subscribe to coordinator updates."""
        self.async_on_remove(
            self.coordinator.async_add_listener(
                self._handle_coordinator_update
            )
        )
