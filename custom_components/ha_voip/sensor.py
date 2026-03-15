"""Sensor platform for HA VoIP.

Delegates entity creation to the entities sub-package.
"""

from __future__ import annotations

from homeassistant.config_entries import ConfigEntry
from homeassistant.core import HomeAssistant
from homeassistant.helpers.entity_platform import AddEntitiesCallback

from .entities.call_sensor import async_setup_call_sensors


async def async_setup_entry(
    hass: HomeAssistant,
    config_entry: ConfigEntry,
    async_add_entities: AddEntitiesCallback,
) -> None:
    """Set up HA VoIP sensor entities."""
    await async_setup_call_sensors(hass, config_entry, async_add_entities)
