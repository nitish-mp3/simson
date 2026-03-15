"""Constants for the HA VoIP integration."""

from __future__ import annotations

from typing import Final

# ---------------------------------------------------------------------------
# Domain
# ---------------------------------------------------------------------------
DOMAIN: Final = "ha_voip"
NAME: Final = "HA VoIP"

# ---------------------------------------------------------------------------
# Configuration keys
# ---------------------------------------------------------------------------
CONF_ENGINE_MODE: Final = "engine_mode"
CONF_ENGINE_HOST: Final = "engine_host"
CONF_ENGINE_PORT: Final = "engine_port"
CONF_GRPC_PORT: Final = "grpc_port"
CONF_SIP_PORT: Final = "sip_port"
CONF_RTP_PORT_START: Final = "rtp_port_start"
CONF_RTP_PORT_END: Final = "rtp_port_end"
CONF_WS_PORT: Final = "ws_port"
CONF_EXTERNAL_HOST: Final = "external_host"
CONF_STUN_SERVER: Final = "stun_server"
CONF_TURN_SERVER: Final = "turn_server"
CONF_TURN_USERNAME: Final = "turn_username"
CONF_TURN_PASSWORD: Final = "turn_password"
CONF_CERT_MODE: Final = "cert_mode"
CONF_CERT_PATH: Final = "cert_path"
CONF_KEY_PATH: Final = "key_path"
CONF_ACME_DOMAIN: Final = "acme_domain"
CONF_ACME_EMAIL: Final = "acme_email"
CONF_EXTENSIONS: Final = "extensions"
CONF_DEFAULT_CODEC: Final = "default_codec"
CONF_ENABLE_RECORDING: Final = "enable_recording"
CONF_RECORDING_PATH: Final = "recording_path"
CONF_LOG_LEVEL: Final = "log_level"
CONF_ENGINE_BINARY_PATH: Final = "engine_binary_path"

# ---------------------------------------------------------------------------
# Engine modes
# ---------------------------------------------------------------------------
ENGINE_MODE_LOCAL: Final = "local"
ENGINE_MODE_REMOTE: Final = "remote"

# ---------------------------------------------------------------------------
# Certificate modes
# ---------------------------------------------------------------------------
CERT_MODE_ACME: Final = "acme"
CERT_MODE_MANUAL: Final = "manual"
CERT_MODE_SELF_SIGNED: Final = "self_signed"

# ---------------------------------------------------------------------------
# Default values
# ---------------------------------------------------------------------------
DEFAULT_ENGINE_HOST: Final = "127.0.0.1"
DEFAULT_ENGINE_PORT: Final = 8585
DEFAULT_GRPC_PORT: Final = 50051
DEFAULT_SIP_PORT: Final = 5060
DEFAULT_RTP_PORT_START: Final = 10000
DEFAULT_RTP_PORT_END: Final = 20000
DEFAULT_WS_PORT: Final = 8586
DEFAULT_STUN_SERVER: Final = "stun:stun.l.google.com:19302"
DEFAULT_LOG_LEVEL: Final = "info"
DEFAULT_RECORDING_PATH: Final = "/config/recordings/voip"

# Port fallback sequence when default SIP port is in use
SIP_PORT_FALLBACK: Final[list[int]] = [5060, 5061, 5062, 15060, 15061]

# ---------------------------------------------------------------------------
# Codec settings
# ---------------------------------------------------------------------------
CODEC_OPUS: Final = "opus"
CODEC_G711_ULAW: Final = "g711_ulaw"
CODEC_G711_ALAW: Final = "g711_alaw"
CODEC_G722: Final = "g722"

DEFAULT_CODEC: Final = CODEC_OPUS
SUPPORTED_CODECS: Final[list[str]] = [
    CODEC_OPUS,
    CODEC_G711_ULAW,
    CODEC_G711_ALAW,
    CODEC_G722,
]

# ---------------------------------------------------------------------------
# Service names
# ---------------------------------------------------------------------------
SERVICE_MAKE_CALL: Final = "make_call"
SERVICE_HANGUP: Final = "hangup"
SERVICE_TRANSFER: Final = "transfer"
SERVICE_RECORD_TOGGLE: Final = "record_toggle"
SERVICE_MUTE_TOGGLE: Final = "mute_toggle"
SERVICE_SEND_DTMF: Final = "send_dtmf"

# ---------------------------------------------------------------------------
# Event names (fired on the HA event bus)
# ---------------------------------------------------------------------------
EVENT_CALL_STARTED: Final = f"{DOMAIN}_call_started"
EVENT_CALL_ENDED: Final = f"{DOMAIN}_call_ended"
EVENT_CALL_RINGING: Final = f"{DOMAIN}_call_ringing"
EVENT_CALL_ANSWERED: Final = f"{DOMAIN}_call_answered"
EVENT_CALL_HELD: Final = f"{DOMAIN}_call_held"
EVENT_CALL_RESUMED: Final = f"{DOMAIN}_call_resumed"
EVENT_CALL_TRANSFERRED: Final = f"{DOMAIN}_call_transferred"
EVENT_REGISTRATION_CHANGED: Final = f"{DOMAIN}_registration_changed"
EVENT_ENGINE_STATE_CHANGED: Final = f"{DOMAIN}_engine_state_changed"
EVENT_DTMF_RECEIVED: Final = f"{DOMAIN}_dtmf_received"

# ---------------------------------------------------------------------------
# Entity prefixes / identifiers
# ---------------------------------------------------------------------------
ENTITY_PREFIX_CALL: Final = "call"
ENTITY_PREFIX_EXTENSION: Final = "extension"
ENTITY_PREFIX_PRESENCE: Final = "presence"
ENTITY_PREFIX_ENGINE: Final = "engine"

# ---------------------------------------------------------------------------
# Call states (for the call state sensor)
# ---------------------------------------------------------------------------
CALL_STATE_IDLE: Final = "idle"
CALL_STATE_RINGING: Final = "ringing"
CALL_STATE_IN_CALL: Final = "in_call"
CALL_STATE_ON_HOLD: Final = "on_hold"
CALL_STATE_TRANSFERRING: Final = "transferring"

# ---------------------------------------------------------------------------
# Engine states
# ---------------------------------------------------------------------------
ENGINE_STATE_STOPPED: Final = "stopped"
ENGINE_STATE_STARTING: Final = "starting"
ENGINE_STATE_RUNNING: Final = "running"
ENGINE_STATE_ERROR: Final = "error"
ENGINE_STATE_RESTARTING: Final = "restarting"

# ---------------------------------------------------------------------------
# Data keys stored in hass.data[DOMAIN]
# ---------------------------------------------------------------------------
DATA_COORDINATOR: Final = "coordinator"
DATA_ENGINE_MANAGER: Final = "engine_manager"
DATA_UNSUB_LISTENERS: Final = "unsub_listeners"
DATA_GRPC_CHANNEL: Final = "grpc_channel"

# ---------------------------------------------------------------------------
# Update intervals
# ---------------------------------------------------------------------------
UPDATE_INTERVAL_SECONDS: Final = 5
ENGINE_HEALTH_CHECK_INTERVAL: Final = 10
ENGINE_RESTART_DELAY: Final = 3
ENGINE_MAX_RESTART_ATTEMPTS: Final = 5

# ---------------------------------------------------------------------------
# Platforms
# ---------------------------------------------------------------------------
PLATFORMS: Final[list[str]] = ["sensor", "binary_sensor"]

# ---------------------------------------------------------------------------
# Misc
# ---------------------------------------------------------------------------
MANUFACTURER: Final = "HA VoIP"
MODEL_ENGINE: Final = "VoIP Engine"
MODEL_EXTENSION: Final = "SIP Extension"
