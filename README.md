# ha-voip

**Production-grade VoIP for Home Assistant**

[![CI](https://github.com/ha-voip/ha-voip/actions/workflows/ci.yml/badge.svg)](https://github.com/ha-voip/ha-voip/actions/workflows/ci.yml)
[![Coverage](https://codecov.io/gh/ha-voip/ha-voip/branch/main/graph/badge.svg)](https://codecov.io/gh/ha-voip/ha-voip)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![HA Version](https://img.shields.io/badge/Home_Assistant-2024.1%2B-41BDF5.svg?logo=home-assistant)](https://www.home-assistant.io/)
[![HACS](https://img.shields.io/badge/HACS-Default-orange.svg)](https://hacs.xyz/)

---

ha-voip brings full-featured, carrier-quality voice-over-IP to Home Assistant. One install gives you a complete phone system: SIP registration, WebRTC browser calling, an embedded TURN server for NAT traversal, automatic TLS certificate management, and encrypted media -- all without external dependencies or cloud subscriptions.

## Feature Highlights

- **Single Install** -- One integration adds SIP server, media engine, TURN relay, and certificate manager. No external Asterisk, FreeSWITCH, or coturn required.
- **WebRTC Browser Calling** -- Make and receive calls directly from the Home Assistant dashboard. Works on desktop and mobile browsers with no plugins.
- **Embedded TURN Server** -- Built-in TURN/STUN relay with automatic credential rotation. Media always connects, even behind strict enterprise NATs and symmetric firewalls.
- **Automatic TLS** -- Provisions and renews TLS certificates via ACME (Let's Encrypt) or generates self-signed certificates for local-only deployments. SIP-over-TLS and SRTP are enabled by default.
- **NAT Traversal** -- Aggressive port fallback algorithm tries UDP, then TCP 443, then TURN-over-TLS 443, guaranteeing connectivity in virtually any network environment.
- **Encrypted Recordings** -- Call recordings are AES-256-GCM encrypted at rest. Playback requires the HA user's credential scope.
- **Home Assistant Native** -- Exposes `sensor`, `binary_sensor`, `switch`, and `media_player` entities. Automations can trigger calls, route incoming calls, play announcements, and log CDRs.
- **Prometheus & Grafana Ready** -- Ships pre-built dashboards and scrape configs for real-time monitoring of call quality, oRTP jitter, packet loss, and oRTP MOS estimates.

## Quickstart

### 1. Install the integration

```yaml
# In your Home Assistant configuration.yaml
# (or install via HACS -- see Installation below)
ha_voip:
  sip_port: 5060
  rtp_port_range: [10000, 20000]
  turn_enabled: true
```

### 2. Restart Home Assistant

```bash
ha core restart
```

### 3. Open the VoIP panel

Navigate to **Settings > Devices & Services > ha-voip** in Home Assistant. Register a SIP extension and click **Call** in the dashboard card to start your first WebRTC call.

## Architecture Overview

```
+-------------------------------------------------------------+
|                     Home Assistant Host                      |
|                                                              |
|  +-------------------+    +------------------------------+   |
|  |   HA Integration  |    |        VoIP Engine           |   |
|  |  (Python custom   |    |  +--------+ +----------+    |   |
|  |   component)      |<-->|  |  SIP   | |  Media   |    |   |
|  |                   |    |  | Stack  | |  Mixer   |    |   |
|  | - config flow     | WS |  +--------+ +----------+    |   |
|  | - entities        |    |  +--------+ +----------+    |   |
|  | - services        |    |  |  API   | |  RTP/    |    |   |
|  | - automations     |    |  | Server | |  SRTP    |    |   |
|  +-------------------+    |  +--------+ +----------+    |   |
|           |               +------------------------------+   |
|           |                          |                       |
|  +-------------------+    +------------------------------+   |
|  |   Frontend Panel  |    |     TURN / STUN Server       |   |
|  |  (Lit WebComponent |    |  - UDP 3478                  |   |
|  |   + WebRTC JS)    |    |  - TCP 443 fallback          |   |
|  +-------------------+    |  - TLS 443 relay             |   |
|                           +------------------------------+   |
|                                      |                       |
|  +---------------------------------------------------+      |
|  |            Certificate Manager                     |      |
|  |  - ACME / Let's Encrypt auto-renewal              |      |
|  |  - Self-signed fallback for .local domains        |      |
|  +---------------------------------------------------+      |
+-------------------------------------------------------------+
```

**Data path:** Browser --> WebRTC (DTLS-SRTP) --> TURN relay (if needed) --> VoIP Engine RTP --> SIP trunk or another extension.

## Requirements

| Requirement          | Minimum                                                  |
|----------------------|----------------------------------------------------------|
| Home Assistant       | 2024.1 or later                                          |
| Python               | 3.11+ (ships with HA OS)                                 |
| Browser              | Chrome 90+, Firefox 90+, Safari 15+, Edge 90+            |
| Operating System     | HA OS, Debian 12+, Ubuntu 22.04+, or any Docker host     |
| Network              | One open UDP port (default 3478) recommended; not required|
| RAM                  | 256 MB free (512 MB recommended for recording)           |
| Disk                 | 100 MB for the integration; more for call recordings     |

## Installation

### HACS (Recommended)

1. Open HACS in Home Assistant.
2. Click **Integrations** then the **+** button.
3. Search for **ha-voip** and click **Install**.
4. Restart Home Assistant.
5. Go to **Settings > Devices & Services > Add Integration > ha-voip**.

### Manual Installation

```bash
# Clone into the custom_components directory
cd /config/custom_components
git clone https://github.com/ha-voip/ha-voip.git ha_voip_repo
cp -r ha_voip_repo/integration/custom_components/ha_voip ./ha_voip

# Install the VoIP engine binary
ha_voip_repo/ci/scripts/install-engine.sh

# Restart Home Assistant
ha core restart
```

### Docker (Standalone Engine)

```bash
docker run -d \
  --name ha-voip-engine \
  --network host \
  -v /path/to/config:/etc/ha-voip \
  -v /path/to/certs:/etc/ha-voip/certs \
  -e HAVOIP_SIP_PORT=5060 \
  -e HAVOIP_TURN_ENABLED=true \
  ghcr.io/ha-voip/voip-engine:latest
```

### Home Assistant Add-on

1. In the Supervisor panel, open the **Add-on Store**.
2. Click the three-dot menu and choose **Repositories**.
3. Add `https://github.com/ha-voip/ha-voip-addon`.
4. Install the **ha-voip** add-on and start it.
5. The add-on auto-discovers the HA integration and links them.

## Configuration

All configuration is managed through `configuration.yaml` or the UI config flow.

```yaml
ha_voip:
  # ---- SIP Settings ----
  sip_port: 5060                   # SIP signaling port (UDP/TCP)
  sip_tls_port: 5061               # SIP-over-TLS port
  sip_transport: [udp, tcp, tls]   # Enabled transports, in preference order
  sip_realm: "home.local"          # SIP authentication realm

  # ---- RTP / Media ----
  rtp_port_range: [10000, 20000]   # Ephemeral port range for RTP streams
  codecs: [opus, pcma, pcmu]       # Allowed codecs in priority order
  recording_enabled: true          # Record calls to encrypted storage
  recording_path: "/config/recordings"
  recording_retention_days: 30     # Auto-purge recordings older than N days

  # ---- TURN / NAT ----
  turn_enabled: true               # Enable the built-in TURN server
  turn_port: 3478                  # Primary TURN port (UDP + TCP)
  turn_tls_port: 443               # TLS fallback port for restrictive networks
  turn_realm: "turn.home.local"
  turn_secret_rotation_hours: 24   # Rotate TURN credentials every N hours
  stun_servers:                    # Additional external STUN servers (optional)
    - "stun:stun.l.google.com:19302"

  # ---- TLS / Certificates ----
  tls_mode: "auto"                 # "auto" (ACME), "manual", or "self-signed"
  acme_email: "admin@example.com"  # Required when tls_mode is "auto"
  acme_directory: "https://acme-v02.api.letsencrypt.org/directory"
  cert_path: "/config/certs"

  # ---- Security ----
  srtp_required: true              # Reject calls that don't negotiate SRTP
  fail2ban_enabled: true           # Enable brute-force protection
  fail2ban_max_retries: 5
  fail2ban_ban_duration: 3600      # Ban duration in seconds
  allowed_networks:                # Optional IP allowlist (CIDR)
    - "192.168.0.0/16"
    - "10.0.0.0/8"

  # ---- Extensions ----
  extensions:
    - extension: 100
      name: "Living Room"
      password: "!secret voip_ext_100_password"
      ring_timeout: 30
    - extension: 101
      name: "Kitchen"
      password: "!secret voip_ext_101_password"

  # ---- SIP Trunking (optional) ----
  trunks:
    - name: "PSTN Gateway"
      host: "sip.provider.example.com"
      port: 5060
      transport: tls
      username: "!secret sip_trunk_user"
      password: "!secret sip_trunk_pass"
      register: true
      codecs: [pcma, pcmu]
      inbound_route: "extension:100"
```

### Configuration via UI

Every option above can also be set through the **ha-voip** integration panel at **Settings > Devices & Services > ha-voip > Configure**.

## Usage Examples

### Make a call from an automation

```yaml
automation:
  - alias: "Doorbell rings - call living room"
    trigger:
      - platform: state
        entity_id: binary_sensor.front_door_bell
        to: "on"
    action:
      - service: ha_voip.call
        data:
          from_extension: 100
          to_extension: 101
          announcement: "media-source://media/doorbell_chime.wav"
          timeout: 60
```

### Play an announcement on all extensions

```yaml
- service: ha_voip.announce
  data:
    message: "Alarm system armed. All doors locked."
    tts_engine: "google_translate"
    extensions: [100, 101, 102]
```

### Route incoming PSTN calls based on time

```yaml
automation:
  - alias: "Route incoming calls by time of day"
    trigger:
      - platform: event
        event_type: ha_voip_incoming_call
    condition:
      - condition: time
        after: "08:00:00"
        before: "22:00:00"
    action:
      - service: ha_voip.transfer
        data:
          call_id: "{{ trigger.event.data.call_id }}"
          to_extension: 100
  - alias: "Night-time voicemail"
    trigger:
      - platform: event
        event_type: ha_voip_incoming_call
    condition:
      - condition: time
        after: "22:00:00"
        before: "08:00:00"
    action:
      - service: ha_voip.voicemail
        data:
          call_id: "{{ trigger.event.data.call_id }}"
          greeting: "media-source://media/night_greeting.wav"
```

### Dashboard card (Lovelace)

```yaml
type: custom:ha-voip-card
entity: media_player.ha_voip_extension_100
show_dialpad: true
show_call_history: true
show_transfer_button: true
```

## Entities Exposed

| Entity                                      | Type            | Description                                |
|---------------------------------------------|-----------------|--------------------------------------------|
| `sensor.ha_voip_active_calls`               | `sensor`        | Number of currently active calls           |
| `sensor.ha_voip_ext_100_status`             | `sensor`        | Extension status (idle, ringing, in-call)  |
| `binary_sensor.ha_voip_engine_running`      | `binary_sensor` | Whether the VoIP engine process is alive   |
| `binary_sensor.ha_voip_tls_valid`           | `binary_sensor` | Whether the TLS certificate is valid       |
| `switch.ha_voip_dnd_100`                    | `switch`        | Do Not Disturb toggle per extension        |
| `media_player.ha_voip_extension_100`        | `media_player`  | Call control with play/pause/hangup        |
| `sensor.ha_voip_turn_active_allocations`    | `sensor`        | Number of active TURN allocations          |

## Development Setup

### Prerequisites

- Python 3.11+
- Node.js 18+ and npm 9+
- Rust 1.75+ (for the VoIP engine)
- Docker (optional, for containerized testing)

### Clone and bootstrap

```bash
git clone https://github.com/ha-voip/ha-voip.git
cd ha-voip

# Create a Python virtualenv for the HA integration
python3 -m venv .venv
source .venv/bin/activate
pip install -r integration/requirements.txt
pip install -r tests/requirements-dev.txt

# Build the VoIP engine
cd voip-engine
cargo build --release
cd ..

# Install frontend dependencies
cd frontend
npm ci
npm run build
cd ..
```

### Run the test suites

```bash
# Unit tests (engine)
cd voip-engine && cargo test && cd ..

# Unit tests (integration, Python)
pytest tests/unit/integration_py/ -v

# Unit tests (frontend)
cd frontend && npm test && cd ..

# Integration tests (requires Docker)
pytest tests/integration/ -v --docker

# Load tests
cd tests/load && k6 run call_load.js && cd ../..

# Fuzz tests (engine)
cd voip-engine && cargo fuzz run sip_parser -- -max_total_time=60 && cd ..
```

### Code quality

```bash
# Python linting and formatting
ruff check integration/ tests/
ruff format integration/ tests/

# Rust linting
cd voip-engine && cargo clippy --all-targets -- -D warnings && cd ..

# Frontend linting
cd frontend && npm run lint && cd ..
```

### Running locally with Home Assistant

```bash
# Option A: HA development container
cd integration
ln -s "$(pwd)/custom_components/ha_voip" /config/custom_components/ha_voip

# Option B: Use the devcontainer provided
code --folder-uri vscode-remote://dev-container+$(printf '%s' "$(pwd)" | xxd -p)/workspace
```

## Project Structure

```
ha-voip/
  ci/                  CI pipeline scripts and GitHub Actions workflows
    scripts/           Build, test, and release helper scripts
  docs/                Extended documentation (guides, API reference)
  examples/            Example configurations and automation blueprints
  frontend/            Lit-based WebComponent dashboard panel
    build/             Compiled panel assets
    src/               Panel source (TypeScript, CSS)
  integration/         Home Assistant custom component
    custom_components/
      ha_voip/
        entities/      Entity platform implementations
        translations/  Locale strings (en, de, fr, es, ...)
  migrations/          Database schema migration scripts
  monitoring/          Observability configurations
    grafana/           Pre-built Grafana dashboards (JSON)
    prometheus/        Prometheus scrape configs and alert rules
  ops/                 Deployment manifests
    docker/            Dockerfiles and docker-compose files
    k8s/               Kubernetes manifests and Helm charts
  provisioning/        Infrastructure provisioning
    certificate_manager/  ACME client and cert lifecycle scripts
  tests/               Test suites
    fuzz/              Fuzz test harnesses
    integration/       End-to-end integration tests
    load/              k6 load/performance tests
    unit/
      engine/          VoIP engine unit tests (Rust)
      frontend/        Frontend unit tests (Vitest)
      integration_py/  HA integration unit tests (pytest)
  voip-engine/         Core VoIP engine (Rust)
    src/
      api/             REST + WebSocket API server
      media/           RTP/SRTP media handling, codec negotiation
      sip/             SIP parser, transaction layer, registrar
```

## Contributing

Contributions are welcome. Please read the following before submitting a pull request:

1. **Open an issue first** for non-trivial changes to discuss the approach.
2. **Fork the repository** and create a feature branch from `main`.
3. **Follow existing code style** -- run `ruff`, `cargo clippy`, and `npm run lint` before committing.
4. **Write tests** -- every new feature and bug fix should include corresponding tests.
5. **Sign your commits** -- we require DCO sign-off (`git commit -s`).
6. **Keep PRs focused** -- one logical change per pull request.

See [SECURITY.md](SECURITY.md) for vulnerability reporting guidelines.

## License

Copyright ha-voip contributors.

Licensed under the Apache License, Version 2.0. You may obtain a copy of the license at:

<https://www.apache.org/licenses/LICENSE-2.0>

See the [LICENSE](LICENSE) file for the full text.

## Links

- [Architecture Documentation](architecture.md)
- [Security Policy](SECURITY.md)
- [Detailed Documentation](docs/)
- [Example Configurations](examples/)
- [Monitoring Dashboards](monitoring/)
- [Issue Tracker](https://github.com/ha-voip/ha-voip/issues)
- [Discussion Forum](https://github.com/ha-voip/ha-voip/discussions)
- [Home Assistant Community Thread](https://community.home-assistant.io/)
