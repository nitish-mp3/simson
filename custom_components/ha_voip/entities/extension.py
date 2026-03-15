"""SIP Extension entity for HA VoIP.

Each configured SIP extension is represented as a device in the HA device
registry with associated sensors and binary sensors.
"""

from __future__ import annotations

import logging
from typing import Any

from homeassistant.config_entries import ConfigEntry
from homeassistant.helpers.device_registry import DeviceInfo
from homeassistant.helpers.update_coordinator import CoordinatorEntity

from ..const import (
    DOMAIN,
    ENTITY_PREFIX_EXTENSION,
    MANUFACTURER,
    MODEL_EXTENSION,
)
from ..coordinator import ExtensionInfo, VoipData, VoipDataUpdateCoordinator

_LOGGER = logging.getLogger(__name__)


class VoipExtensionDevice(CoordinatorEntity[VoipDataUpdateCoordinator]):
    """Base entity that represents a SIP extension as an HA device.

    Concrete sensor / binary_sensor entities should inherit from this class
    so they all share the same device entry.
    """

    _attr_has_entity_name = True

    def __init__(
        self,
        coordinator: VoipDataUpdateCoordinator,
        config_entry: ConfigEntry,
        extension_number: str,
        display_name: str,
    ) -> None:
        """Initialize the extension device entity."""
        super().__init__(coordinator)
        self._extension_number = extension_number
        self._display_name = display_name
        self._config_entry = config_entry

        # Unique ID scoped to the config entry + extension
        self._attr_unique_id = (
            f"{config_entry.entry_id}_{ENTITY_PREFIX_EXTENSION}_{extension_number}"
        )

    # -- Device info ----------------------------------------------------------

    @property
    def device_info(self) -> DeviceInfo:
        """Return device info so all extension entities group together."""
        return DeviceInfo(
            identifiers={(DOMAIN, f"{self._config_entry.entry_id}_{self._extension_number}")},
            name=f"VoIP Extension {self._extension_number} ({self._display_name})",
            manufacturer=MANUFACTURER,
            model=MODEL_EXTENSION,
            sw_version="1.0.0",
            via_device=(DOMAIN, self._config_entry.entry_id),
        )

    # -- Convenience accessors ------------------------------------------------

    @property
    def extension_number(self) -> str:
        """Return the SIP extension number."""
        return self._extension_number

    @property
    def _extension_info(self) -> ExtensionInfo | None:
        """Return the cached ExtensionInfo from the coordinator, if any."""
        data: VoipData | None = self.coordinator.data
        if data is None:
            return None
        return data.extensions.get(self._extension_number)

    @property
    def registered(self) -> bool:
        """Return True if the extension is currently registered."""
        info = self._extension_info
        return info.registered if info else False

    @property
    def display_name(self) -> str:
        """Return the display name of the extension."""
        info = self._extension_info
        return info.display_name if info else self._display_name

    @property
    def codec_preferences(self) -> list[str]:
        """Return the codec preference list.

        In a full implementation this would come from the engine's per-extension
        configuration.  For now we return the integration default.
        """
        return [
            self._config_entry.options.get(
                "default_codec",
                self._config_entry.data.get("default_codec", "opus"),
            )
        ]

    @property
    def extra_state_attributes(self) -> dict[str, Any]:
        """Return additional state attributes."""
        info = self._extension_info
        attrs: dict[str, Any] = {
            "extension_number": self._extension_number,
            "registered": self.registered,
            "display_name": self.display_name,
            "codec_preferences": self.codec_preferences,
        }
        if info:
            attrs["user_agent"] = info.user_agent
            attrs["contact_uri"] = info.contact_uri
            attrs["last_seen"] = info.last_seen
        return attrs
