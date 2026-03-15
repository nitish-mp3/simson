"""Config flow for HA VoIP integration."""

from __future__ import annotations

import asyncio
import logging
import socket
from typing import Any

import voluptuous as vol

from homeassistant.config_entries import (
    ConfigEntry,
    ConfigFlow,
    ConfigFlowResult,
    OptionsFlow,
)
from homeassistant.core import callback
from homeassistant.helpers import config_validation as cv

from .const import (
    CERT_MODE_ACME,
    CERT_MODE_MANUAL,
    CERT_MODE_SELF_SIGNED,
    CODEC_OPUS,
    CONF_ACME_DOMAIN,
    CONF_ACME_EMAIL,
    CONF_CERT_MODE,
    CONF_CERT_PATH,
    CONF_DEFAULT_CODEC,
    CONF_ENABLE_RECORDING,
    CONF_ENGINE_BINARY_PATH,
    CONF_ENGINE_HOST,
    CONF_ENGINE_MODE,
    CONF_ENGINE_PORT,
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
    DEFAULT_ENGINE_PORT,
    DEFAULT_GRPC_PORT,
    DEFAULT_LOG_LEVEL,
    DEFAULT_RECORDING_PATH,
    DEFAULT_RTP_PORT_END,
    DEFAULT_RTP_PORT_START,
    DEFAULT_SIP_PORT,
    DEFAULT_STUN_SERVER,
    DEFAULT_WS_PORT,
    DOMAIN,
    ENGINE_MODE_LOCAL,
    ENGINE_MODE_REMOTE,
    SIP_PORT_FALLBACK,
    SUPPORTED_CODECS,
)

_LOGGER = logging.getLogger(__name__)


def _detect_local_ip() -> str:
    """Detect the primary local IP address."""
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.settimeout(0.1)
        # Doesn't actually send anything; just triggers route lookup
        sock.connect(("8.8.8.8", 80))
        ip = sock.getsockname()[0]
        sock.close()
        return ip
    except OSError:
        return "0.0.0.0"


def _check_port_available(port: int, host: str = "0.0.0.0") -> bool:
    """Return True if the given UDP/TCP port is available."""
    for family in (socket.SOCK_STREAM, socket.SOCK_DGRAM):
        try:
            sock = socket.socket(socket.AF_INET, family)
            sock.settimeout(0.1)
            sock.bind((host, port))
            sock.close()
        except OSError:
            return False
    return True


def _find_available_sip_port() -> int:
    """Walk the fallback sequence and return the first free SIP port."""
    for port in SIP_PORT_FALLBACK:
        if _check_port_available(port):
            return port
    return DEFAULT_SIP_PORT


async def _validate_grpc_connection(host: str, port: int) -> bool:
    """Attempt a lightweight connection check to a gRPC endpoint."""
    try:
        _, writer = await asyncio.wait_for(
            asyncio.open_connection(host, port),
            timeout=5.0,
        )
        writer.close()
        await writer.wait_closed()
        return True
    except (OSError, asyncio.TimeoutError):
        return False


# -------------------------------------------------------------------------
# Main config flow
# -------------------------------------------------------------------------


class HaVoipConfigFlow(ConfigFlow, domain=DOMAIN):
    """Handle a config flow for HA VoIP."""

    VERSION = 1

    def __init__(self) -> None:
        """Initialize the config flow."""
        self._data: dict[str, Any] = {}
        self._errors: dict[str, str] = {}

    # -- Step 1: Engine mode -------------------------------------------------

    async def async_step_user(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        """Step 1 -- choose local vs. remote engine."""
        if user_input is not None:
            self._data[CONF_ENGINE_MODE] = user_input[CONF_ENGINE_MODE]
            return await self.async_step_network()

        schema = vol.Schema(
            {
                vol.Required(
                    CONF_ENGINE_MODE, default=ENGINE_MODE_LOCAL
                ): vol.In(
                    {
                        ENGINE_MODE_LOCAL: "Local Engine (bundled)",
                        ENGINE_MODE_REMOTE: "Remote Engine",
                    }
                ),
            }
        )
        return self.async_show_form(
            step_id="user",
            data_schema=schema,
            errors=self._errors,
        )

    # -- Step 2: Network configuration ----------------------------------------

    async def async_step_network(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        """Step 2 -- network / port configuration."""
        self._errors = {}

        if user_input is not None:
            # Validate ports are in range
            for key in (
                CONF_SIP_PORT,
                CONF_GRPC_PORT,
                CONF_WS_PORT,
                CONF_RTP_PORT_START,
                CONF_RTP_PORT_END,
            ):
                val = user_input.get(key)
                if val is not None and not (1024 <= val <= 65535):
                    self._errors[key] = "port_out_of_range"

            if user_input.get(CONF_RTP_PORT_START, 0) >= user_input.get(
                CONF_RTP_PORT_END, 0
            ):
                self._errors[CONF_RTP_PORT_END] = "rtp_range_invalid"

            # For remote mode, validate the engine is reachable
            if (
                not self._errors
                and self._data[CONF_ENGINE_MODE] == ENGINE_MODE_REMOTE
            ):
                reachable = await _validate_grpc_connection(
                    user_input[CONF_ENGINE_HOST],
                    user_input[CONF_GRPC_PORT],
                )
                if not reachable:
                    self._errors["base"] = "engine_unreachable"

            if not self._errors:
                self._data.update(user_input)
                return await self.async_step_certificates()

        detected_ip = await self.hass.async_add_executor_job(_detect_local_ip)
        suggested_sip = await self.hass.async_add_executor_job(
            _find_available_sip_port
        )

        is_remote = self._data.get(CONF_ENGINE_MODE) == ENGINE_MODE_REMOTE

        schema_dict: dict[vol.Marker, Any] = {}
        if is_remote:
            schema_dict[vol.Required(CONF_ENGINE_HOST, default=DEFAULT_ENGINE_HOST)] = str
            schema_dict[vol.Required(CONF_GRPC_PORT, default=DEFAULT_GRPC_PORT)] = int
        else:
            schema_dict[vol.Optional(CONF_EXTERNAL_HOST, default=detected_ip)] = str
            schema_dict[vol.Required(CONF_SIP_PORT, default=suggested_sip)] = int
            schema_dict[vol.Required(CONF_GRPC_PORT, default=DEFAULT_GRPC_PORT)] = int
            schema_dict[vol.Required(CONF_WS_PORT, default=DEFAULT_WS_PORT)] = int
            schema_dict[
                vol.Required(CONF_RTP_PORT_START, default=DEFAULT_RTP_PORT_START)
            ] = int
            schema_dict[
                vol.Required(CONF_RTP_PORT_END, default=DEFAULT_RTP_PORT_END)
            ] = int
            schema_dict[
                vol.Optional(CONF_STUN_SERVER, default=DEFAULT_STUN_SERVER)
            ] = str
            schema_dict[vol.Optional(CONF_TURN_SERVER, default="")] = str
            schema_dict[vol.Optional(CONF_TURN_USERNAME, default="")] = str
            schema_dict[vol.Optional(CONF_TURN_PASSWORD, default="")] = str

        return self.async_show_form(
            step_id="network",
            data_schema=vol.Schema(schema_dict),
            errors=self._errors,
        )

    # -- Step 3: Certificate configuration ------------------------------------

    async def async_step_certificates(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        """Step 3 -- TLS / certificate configuration."""
        self._errors = {}

        if user_input is not None:
            mode = user_input[CONF_CERT_MODE]

            if mode == CERT_MODE_MANUAL:
                if not user_input.get(CONF_CERT_PATH):
                    self._errors[CONF_CERT_PATH] = "cert_path_required"
                if not user_input.get(CONF_KEY_PATH):
                    self._errors[CONF_KEY_PATH] = "key_path_required"

            if mode == CERT_MODE_ACME:
                if not user_input.get(CONF_ACME_DOMAIN):
                    self._errors[CONF_ACME_DOMAIN] = "acme_domain_required"
                if not user_input.get(CONF_ACME_EMAIL):
                    self._errors[CONF_ACME_EMAIL] = "acme_email_required"

            if not self._errors:
                self._data.update(user_input)
                return await self.async_step_extensions()

        schema = vol.Schema(
            {
                vol.Required(
                    CONF_CERT_MODE, default=CERT_MODE_SELF_SIGNED
                ): vol.In(
                    {
                        CERT_MODE_SELF_SIGNED: "Self-Signed (easiest)",
                        CERT_MODE_ACME: "Auto ACME (Let's Encrypt)",
                        CERT_MODE_MANUAL: "Manual Certificate",
                    }
                ),
                vol.Optional(CONF_CERT_PATH, default=""): str,
                vol.Optional(CONF_KEY_PATH, default=""): str,
                vol.Optional(CONF_ACME_DOMAIN, default=""): str,
                vol.Optional(CONF_ACME_EMAIL, default=""): str,
            }
        )
        return self.async_show_form(
            step_id="certificates",
            data_schema=schema,
            errors=self._errors,
        )

    # -- Step 4: Initial extension setup --------------------------------------

    async def async_step_extensions(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        """Step 4 -- map HA users to SIP extensions."""
        self._errors = {}

        if user_input is not None:
            raw = user_input.get(CONF_EXTENSIONS, "")
            extensions: list[dict[str, str]] = []
            for line in raw.splitlines():
                line = line.strip()
                if not line:
                    continue
                parts = [p.strip() for p in line.split(",")]
                if len(parts) < 2:
                    self._errors[CONF_EXTENSIONS] = "extension_format_invalid"
                    break
                extensions.append(
                    {"number": parts[0], "name": parts[1], "password": parts[2] if len(parts) > 2 else ""}
                )

            if not self._errors and not extensions:
                self._errors[CONF_EXTENSIONS] = "no_extensions"

            if not self._errors:
                self._data[CONF_EXTENSIONS] = extensions
                self._data.setdefault(CONF_DEFAULT_CODEC, CODEC_OPUS)
                self._data.setdefault(CONF_ENABLE_RECORDING, False)
                self._data.setdefault(CONF_RECORDING_PATH, DEFAULT_RECORDING_PATH)
                self._data.setdefault(CONF_LOG_LEVEL, DEFAULT_LOG_LEVEL)

                await self.async_set_unique_id(DOMAIN)
                self._abort_if_unique_id_configured()

                return self.async_create_entry(
                    title="HA VoIP",
                    data=self._data,
                )

        schema = vol.Schema(
            {
                vol.Required(CONF_EXTENSIONS): cv.string,
            }
        )
        return self.async_show_form(
            step_id="extensions",
            data_schema=schema,
            errors=self._errors,
            description_placeholders={
                "extension_format": "100, Alice, secret123\n101, Bob, password456"
            },
        )

    # -- Reauth flow ----------------------------------------------------------

    async def async_step_reauth(
        self, entry_data: dict[str, Any]
    ) -> ConfigFlowResult:
        """Handle re-authentication (credential rotation)."""
        return await self.async_step_reauth_confirm()

    async def async_step_reauth_confirm(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        """Confirm re-authentication with updated credentials."""
        self._errors = {}

        if user_input is not None:
            reauth_entry = self._get_reauth_entry()
            new_data = {**reauth_entry.data, **user_input}

            # Validate connectivity with new creds if remote
            if new_data.get(CONF_ENGINE_MODE) == ENGINE_MODE_REMOTE:
                reachable = await _validate_grpc_connection(
                    new_data[CONF_ENGINE_HOST],
                    new_data[CONF_GRPC_PORT],
                )
                if not reachable:
                    self._errors["base"] = "engine_unreachable"

            if not self._errors:
                return self.async_update_reload_and_abort(
                    reauth_entry,
                    data=new_data,
                )

        schema = vol.Schema(
            {
                vol.Required(CONF_ENGINE_HOST): str,
                vol.Required(CONF_GRPC_PORT, default=DEFAULT_GRPC_PORT): int,
            }
        )
        return self.async_show_form(
            step_id="reauth_confirm",
            data_schema=schema,
            errors=self._errors,
        )

    # -- Options flow entry point ---------------------------------------------

    @staticmethod
    @callback
    def async_get_options_flow(
        config_entry: ConfigEntry,
    ) -> HaVoipOptionsFlow:
        """Return the options flow handler."""
        return HaVoipOptionsFlow(config_entry)


# -------------------------------------------------------------------------
# Options flow  (runtime changes without removing the integration)
# -------------------------------------------------------------------------


class HaVoipOptionsFlow(OptionsFlow):
    """Handle option changes for HA VoIP."""

    def __init__(self, config_entry: ConfigEntry) -> None:
        """Initialize the options flow."""
        self._config_entry = config_entry
        self._errors: dict[str, str] = {}

    async def async_step_init(
        self, user_input: dict[str, Any] | None = None
    ) -> ConfigFlowResult:
        """First (and only) step of the options flow."""
        self._errors = {}

        if user_input is not None:
            # Validate codec choice
            if user_input.get(CONF_DEFAULT_CODEC) not in SUPPORTED_CODECS:
                self._errors[CONF_DEFAULT_CODEC] = "unsupported_codec"

            # Validate recording path is not empty when recording enabled
            if user_input.get(CONF_ENABLE_RECORDING) and not user_input.get(
                CONF_RECORDING_PATH
            ):
                self._errors[CONF_RECORDING_PATH] = "recording_path_required"

            if not self._errors:
                return self.async_create_entry(title="", data=user_input)

        current = self._config_entry.options or self._config_entry.data

        schema = vol.Schema(
            {
                vol.Optional(
                    CONF_DEFAULT_CODEC,
                    default=current.get(CONF_DEFAULT_CODEC, CODEC_OPUS),
                ): vol.In(SUPPORTED_CODECS),
                vol.Optional(
                    CONF_ENABLE_RECORDING,
                    default=current.get(CONF_ENABLE_RECORDING, False),
                ): bool,
                vol.Optional(
                    CONF_RECORDING_PATH,
                    default=current.get(CONF_RECORDING_PATH, DEFAULT_RECORDING_PATH),
                ): str,
                vol.Optional(
                    CONF_LOG_LEVEL,
                    default=current.get(CONF_LOG_LEVEL, DEFAULT_LOG_LEVEL),
                ): vol.In(["debug", "info", "warning", "error"]),
                vol.Optional(
                    CONF_STUN_SERVER,
                    default=current.get(CONF_STUN_SERVER, DEFAULT_STUN_SERVER),
                ): str,
                vol.Optional(
                    CONF_TURN_SERVER,
                    default=current.get(CONF_TURN_SERVER, ""),
                ): str,
                vol.Optional(
                    CONF_TURN_USERNAME,
                    default=current.get(CONF_TURN_USERNAME, ""),
                ): str,
                vol.Optional(
                    CONF_TURN_PASSWORD,
                    default=current.get(CONF_TURN_PASSWORD, ""),
                ): str,
                vol.Optional(
                    CONF_ENGINE_BINARY_PATH,
                    default=current.get(CONF_ENGINE_BINARY_PATH, ""),
                ): str,
            }
        )

        return self.async_show_form(
            step_id="init",
            data_schema=schema,
            errors=self._errors,
        )
