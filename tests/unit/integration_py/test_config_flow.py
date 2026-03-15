"""Unit tests for the HA VoIP config flow.

Tests the multi-step config flow (user -> network -> certificates -> extensions),
validation logic, options flow, and edge cases.
"""

from __future__ import annotations

from typing import Any
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from custom_components.ha_voip.config_flow import HaVoipConfigFlow, HaVoipOptionsFlow
from custom_components.ha_voip.const import (
    CERT_MODE_ACME,
    CERT_MODE_MANUAL,
    CERT_MODE_SELF_SIGNED,
    CODEC_G711_ULAW,
    CODEC_OPUS,
    CONF_ACME_DOMAIN,
    CONF_ACME_EMAIL,
    CONF_CERT_MODE,
    CONF_CERT_PATH,
    CONF_DEFAULT_CODEC,
    CONF_ENABLE_RECORDING,
    CONF_ENGINE_HOST,
    CONF_ENGINE_MODE,
    CONF_EXTENSIONS,
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
    DEFAULT_GRPC_PORT,
    DEFAULT_RTP_PORT_END,
    DEFAULT_RTP_PORT_START,
    DEFAULT_SIP_PORT,
    DEFAULT_WS_PORT,
    DOMAIN,
    ENGINE_MODE_LOCAL,
    ENGINE_MODE_REMOTE,
)

# We import the conftest fixtures implicitly via pytest.
# The `hass` fixture provides a MockHass instance.

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def _make_flow(hass) -> HaVoipConfigFlow:
    """Instantiate a config flow and attach the mock hass."""
    flow = HaVoipConfigFlow()
    flow.hass = hass
    return flow


# ---------------------------------------------------------------------------
# Step 1: User (engine mode selection)
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_step_user_shows_form(hass):
    """First call without input must show the form."""
    flow = _make_flow(hass)
    result = await flow.async_step_user(user_input=None)
    assert result["type"] == "form"
    assert result["step_id"] == "user"


@pytest.mark.asyncio
async def test_step_user_local_mode_proceeds(hass):
    """Selecting local mode must advance to the network step."""
    flow = _make_flow(hass)
    result = await flow.async_step_user(
        user_input={CONF_ENGINE_MODE: ENGINE_MODE_LOCAL}
    )
    assert result["type"] == "form"
    assert result["step_id"] == "network"


@pytest.mark.asyncio
async def test_step_user_remote_mode_proceeds(hass):
    """Selecting remote mode must advance to the network step."""
    flow = _make_flow(hass)
    result = await flow.async_step_user(
        user_input={CONF_ENGINE_MODE: ENGINE_MODE_REMOTE}
    )
    assert result["type"] == "form"
    assert result["step_id"] == "network"


# ---------------------------------------------------------------------------
# Step 2: Network
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_step_network_port_out_of_range(hass):
    """Ports below 1024 must be rejected."""
    flow = _make_flow(hass)
    await flow.async_step_user({CONF_ENGINE_MODE: ENGINE_MODE_LOCAL})
    result = await flow.async_step_network(
        user_input={
            CONF_SIP_PORT: 80,  # out of range
            CONF_GRPC_PORT: DEFAULT_GRPC_PORT,
            CONF_WS_PORT: DEFAULT_WS_PORT,
            CONF_RTP_PORT_START: DEFAULT_RTP_PORT_START,
            CONF_RTP_PORT_END: DEFAULT_RTP_PORT_END,
            CONF_STUN_SERVER: "stun:stun.l.google.com:19302",
            CONF_TURN_SERVER: "",
            CONF_TURN_USERNAME: "",
            CONF_TURN_PASSWORD: "",
        },
    )
    assert result["type"] == "form"
    assert result["step_id"] == "network"
    assert CONF_SIP_PORT in result.get("errors", {})


@pytest.mark.asyncio
async def test_step_network_rtp_range_invalid(hass):
    """RTP start >= end must be rejected."""
    flow = _make_flow(hass)
    await flow.async_step_user({CONF_ENGINE_MODE: ENGINE_MODE_LOCAL})
    result = await flow.async_step_network(
        user_input={
            CONF_SIP_PORT: DEFAULT_SIP_PORT,
            CONF_GRPC_PORT: DEFAULT_GRPC_PORT,
            CONF_WS_PORT: DEFAULT_WS_PORT,
            CONF_RTP_PORT_START: 20000,
            CONF_RTP_PORT_END: 10000,
            CONF_STUN_SERVER: "",
            CONF_TURN_SERVER: "",
            CONF_TURN_USERNAME: "",
            CONF_TURN_PASSWORD: "",
        },
    )
    assert result["type"] == "form"
    assert CONF_RTP_PORT_END in result.get("errors", {})


@pytest.mark.asyncio
async def test_step_network_valid_local_proceeds(hass):
    """Valid local network config must advance to certificates."""
    flow = _make_flow(hass)
    await flow.async_step_user({CONF_ENGINE_MODE: ENGINE_MODE_LOCAL})
    result = await flow.async_step_network(
        user_input={
            CONF_SIP_PORT: DEFAULT_SIP_PORT,
            CONF_GRPC_PORT: DEFAULT_GRPC_PORT,
            CONF_WS_PORT: DEFAULT_WS_PORT,
            CONF_RTP_PORT_START: DEFAULT_RTP_PORT_START,
            CONF_RTP_PORT_END: DEFAULT_RTP_PORT_END,
            CONF_STUN_SERVER: "stun:stun.l.google.com:19302",
            CONF_TURN_SERVER: "",
            CONF_TURN_USERNAME: "",
            CONF_TURN_PASSWORD: "",
        },
    )
    assert result["type"] == "form"
    assert result["step_id"] == "certificates"


# ---------------------------------------------------------------------------
# Step 3: Certificates
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_step_certificates_self_signed_proceeds(hass):
    """Self-signed mode requires no extra fields."""
    flow = _make_flow(hass)
    await flow.async_step_user({CONF_ENGINE_MODE: ENGINE_MODE_LOCAL})
    await flow.async_step_network(
        user_input={
            CONF_SIP_PORT: DEFAULT_SIP_PORT,
            CONF_GRPC_PORT: DEFAULT_GRPC_PORT,
            CONF_WS_PORT: DEFAULT_WS_PORT,
            CONF_RTP_PORT_START: DEFAULT_RTP_PORT_START,
            CONF_RTP_PORT_END: DEFAULT_RTP_PORT_END,
            CONF_STUN_SERVER: "",
            CONF_TURN_SERVER: "",
            CONF_TURN_USERNAME: "",
            CONF_TURN_PASSWORD: "",
        },
    )
    result = await flow.async_step_certificates(
        user_input={
            CONF_CERT_MODE: CERT_MODE_SELF_SIGNED,
            CONF_CERT_PATH: "",
            CONF_KEY_PATH: "",
            CONF_ACME_DOMAIN: "",
            CONF_ACME_EMAIL: "",
        },
    )
    assert result["type"] == "form"
    assert result["step_id"] == "extensions"


@pytest.mark.asyncio
async def test_step_certificates_manual_missing_cert(hass):
    """Manual mode with empty cert path must error."""
    flow = _make_flow(hass)
    flow._data[CONF_ENGINE_MODE] = ENGINE_MODE_LOCAL
    result = await flow.async_step_certificates(
        user_input={
            CONF_CERT_MODE: CERT_MODE_MANUAL,
            CONF_CERT_PATH: "",
            CONF_KEY_PATH: "/ssl/key.pem",
            CONF_ACME_DOMAIN: "",
            CONF_ACME_EMAIL: "",
        },
    )
    assert result["type"] == "form"
    assert CONF_CERT_PATH in result.get("errors", {})


@pytest.mark.asyncio
async def test_step_certificates_acme_missing_domain(hass):
    """ACME mode with empty domain must error."""
    flow = _make_flow(hass)
    flow._data[CONF_ENGINE_MODE] = ENGINE_MODE_LOCAL
    result = await flow.async_step_certificates(
        user_input={
            CONF_CERT_MODE: CERT_MODE_ACME,
            CONF_CERT_PATH: "",
            CONF_KEY_PATH: "",
            CONF_ACME_DOMAIN: "",
            CONF_ACME_EMAIL: "admin@example.com",
        },
    )
    assert result["type"] == "form"
    assert CONF_ACME_DOMAIN in result.get("errors", {})


# ---------------------------------------------------------------------------
# Step 4: Extensions & entry creation
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_step_extensions_invalid_format(hass):
    """Lines without a comma must fail validation."""
    flow = _make_flow(hass)
    flow._data[CONF_ENGINE_MODE] = ENGINE_MODE_LOCAL
    result = await flow.async_step_extensions(
        user_input={CONF_EXTENSIONS: "100 Alice secret"},
    )
    assert result["type"] == "form"
    assert CONF_EXTENSIONS in result.get("errors", {})


@pytest.mark.asyncio
async def test_step_extensions_empty(hass):
    """No extensions provided must fail."""
    flow = _make_flow(hass)
    flow._data[CONF_ENGINE_MODE] = ENGINE_MODE_LOCAL
    result = await flow.async_step_extensions(
        user_input={CONF_EXTENSIONS: ""},
    )
    assert result["type"] == "form"
    assert CONF_EXTENSIONS in result.get("errors", {})


# ---------------------------------------------------------------------------
# Options flow
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_options_flow_shows_form(config_entry):
    """Opening the options flow must show the form."""
    flow = HaVoipOptionsFlow(config_entry)
    flow.hass = MagicMock()
    result = await flow.async_step_init(user_input=None)
    assert result["type"] == "form"
    assert result["step_id"] == "init"


@pytest.mark.asyncio
async def test_options_flow_unsupported_codec(config_entry):
    """Choosing an unsupported codec must show an error."""
    flow = HaVoipOptionsFlow(config_entry)
    flow.hass = MagicMock()
    result = await flow.async_step_init(
        user_input={
            CONF_DEFAULT_CODEC: "speex",
            CONF_ENABLE_RECORDING: False,
            CONF_RECORDING_PATH: "/config/recordings/voip",
            CONF_LOG_LEVEL: "info",
            CONF_STUN_SERVER: "",
            CONF_TURN_SERVER: "",
            CONF_TURN_USERNAME: "",
            CONF_TURN_PASSWORD: "",
            "engine_binary_path": "",
        },
    )
    assert result["type"] == "form"
    assert CONF_DEFAULT_CODEC in result.get("errors", {})
