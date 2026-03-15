# HA VoIP Onboarding Guide

This guide walks you through installing HA VoIP, completing the setup wizard, making your first call, and configuring common deployment scenarios.

---

## Prerequisites

Before you begin, ensure you have:

| Requirement | Minimum | Recommended |
|---|---|---|
| Home Assistant | 2024.1+ | Latest stable |
| Browser | Chrome 90+, Firefox 95+, Safari 16+ | Chrome / Edge latest |
| Network | LAN access to HA | Static IP or DNS for HA host |
| Ports (LAN-only) | UDP 5060, UDP 10000-20000 | Same |
| Ports (remote) | Above + TCP 443, UDP/TCP 3478 | Same |
| RAM for engine | 256 MB | 512 MB |
| Disk | 50 MB (engine binary) | 500 MB (with recordings) |

For PSTN (external phone line) setups you also need an Asterisk or FreeSWITCH instance with a configured SIP trunk to your provider.

---

## Installation Methods

### Method 1: Home Assistant Add-on (recommended)

1. Open **Settings > Add-ons > Add-on Store** in your Home Assistant UI.
2. Click the three-dot menu and select **Repositories**.
3. Add the repository URL:
   ```
   https://github.com/ha-voip/ha-voip-addon
   ```
4. Find **HA VoIP Engine** in the store and click **Install**.
5. After installation completes, click **Start**.
6. The add-on exposes the gRPC port (default 50051) on localhost. No manual port mapping is required.

### Method 2: HACS (Custom Component only)

This method installs the HA integration component but requires you to run the voip-engine binary separately (e.g. via Docker).

1. Open HACS in the Home Assistant sidebar.
2. Click **Integrations > Explore & Download Repositories**.
3. Search for **HA VoIP** and click **Download**.
4. Restart Home Assistant.
5. Proceed to the onboarding wizard (below) and select **Remote Engine** mode, pointing to the host where you run voip-engine.

### Method 3: Docker Compose (standalone)

Use this method when running Home Assistant Core (non-supervised) or when you want full control over the engine deployment.

```yaml
# docker-compose.yml
version: "3.9"
services:
  voip-engine:
    image: ghcr.io/ha-voip/voip-engine:latest
    restart: unless-stopped
    network_mode: host          # Needed for RTP port range
    volumes:
      - ./config.yaml:/etc/voip-engine/config.yaml:ro
      - voip-data:/var/lib/voip-engine
    environment:
      VOIP__LOGGING__LEVEL: info

volumes:
  voip-data:
```

Start with:
```bash
docker compose up -d
```

Then install the custom component via HACS or manually and configure it to point at the engine host.

---

## Onboarding Wizard Walkthrough

After installing the integration, navigate to **Settings > Devices & Services > Add Integration** and search for **HA VoIP**. The setup wizard has four steps.

### Step 1: Engine Mode

**Screenshot description:** A radio-button selector with two options: "Local Engine (bundled)" and "Remote Engine".

- **Local Engine** -- The integration manages the voip-engine process. Best for add-on or single-host setups.
- **Remote Engine** -- Point to an existing voip-engine instance on another host.  Enter the hostname/IP and gRPC port.

Select the appropriate mode and click **Submit**.

### Step 2: Network Configuration

**Screenshot description:** A form with fields for SIP port, gRPC port, WebSocket port, RTP port range start/end, STUN server, and optional TURN server fields.

| Field | Default | Notes |
|---|---|---|
| SIP Port | 5060 (auto-detected) | If 5060 is in use, the wizard suggests 5061 or 15060 |
| gRPC Port | 50051 | Communication between HA and the engine |
| WebSocket Port | 8586 | Used by the Lovelace card for real-time updates |
| RTP Port Start | 10000 | Must be a range of at least 100 ports |
| RTP Port End | 20000 | Upper bound of the RTP port range |
| STUN Server | stun:stun.l.google.com:19302 | Used for NAT traversal |
| TURN Server | (empty) | Required for symmetric NAT or remote access |
| TURN Username | (empty) | Static credential username |
| TURN Password | (empty) | Static credential password |

If you selected **Remote Engine** mode, you will see host and gRPC port fields instead of the full network form. The wizard validates connectivity before proceeding.

### Step 3: Certificate Configuration

**Screenshot description:** A dropdown with three options: "Self-Signed (easiest)", "Auto ACME (Let's Encrypt)", and "Manual Certificate".

- **Self-Signed** -- The engine generates a self-signed certificate at startup. Browsers will show a certificate warning on first use. Suitable for LAN-only deployments.
- **Auto ACME** -- Enter your public domain and email. The engine uses the ACME protocol (Let's Encrypt) to obtain and renew a trusted certificate automatically.  Requires port 80 or DNS challenge access.
- **Manual Certificate** -- Provide paths to existing PEM certificate and key files.

### Step 4: Extension Setup

**Screenshot description:** A multi-line text area with a placeholder showing the format "100, Alice, secret123".

Enter one extension per line in the format:
```
number, display_name, password
```

Example:
```
100, Alice, MyStr0ngP@ss!
101, Bob, An0therP@ss!
102, Front Door, doorbell123
```

Click **Submit** to create the integration entry. The engine starts, registers the extensions, and the VoIP card becomes available.

---

## First Call Setup

1. **Add the VoIP card** to a Lovelace dashboard:
   - Edit dashboard > Add Card > search "VoIP" > select **HA VoIP Card**.
   - Configure the card with a title and your extensions.

2. **Allow microphone access.** The browser will prompt for microphone permission the first time.

3. **Make a test call:**
   - Open the VoIP card on two browser tabs (or two devices).
   - Register as extension 100 on one and 101 on the other.
   - From extension 100, dial `101` and click the call button.
   - The other tab should ring. Click answer.
   - Verify two-way audio.

4. **Check diagnostics:** Open the card's diagnostics panel (gear icon) to run the built-in network test. It will verify STUN connectivity, microphone access, and codec support.

---

## Common Setup Scenarios

### LAN-Only (Simplest)

- Engine mode: Local
- Certificate: Self-Signed
- STUN: `stun:stun.l.google.com:19302` (or a LAN STUN if fully offline)
- TURN: not required (all devices on the same subnet)
- Firewall: allow UDP 5060 and UDP 10000-20000 between HA host and client devices

### Remote Access via Nabu Casa / Reverse Proxy

- Engine mode: Local
- Certificate: Self-Signed or ACME (if directly exposed)
- External host: set to the public hostname or Nabu Casa URL
- TURN: **required** -- configure a TURN server or use the embedded one
  - Open UDP 3478 and TCP 443 on your firewall
  - Set the TURN server field to `turn:your-public-ip:3478`
- The Lovelace card connects via the HA WebSocket (proxied through Nabu Casa / nginx)

### With PSTN Gateway (Asterisk)

- Complete the LAN-only setup first.
- Add an Asterisk SIP trunk pointing to the HA VoIP engine (see `examples/asterisk_integration.yaml`).
- Configure a routing rule in the engine to send external numbers (e.g. `^[0-9]{10,}$`) to the Asterisk trunk.
- Incoming PSTN calls arrive at Asterisk, which forwards them via SIP INVITE to the engine, which rings the assigned extension.

---

## Troubleshooting First-Time Setup

| Problem | Likely cause | Fix |
|---|---|---|
| "Engine unreachable" during setup | gRPC port blocked or engine not running | Verify the engine is running (`docker logs voip-engine`) and the gRPC port (50051) is reachable |
| "Port in use" warning | Another service uses 5060 | Accept the suggested alternative port or stop the conflicting service |
| No audio after connecting | Firewall blocks RTP ports | Open UDP 10000-20000 between HA and client |
| One-way audio | Symmetric NAT without TURN | Configure a TURN server |
| Browser shows certificate warning | Self-signed cert | Expected for LAN; use ACME for production |
| Card does not appear in Lovelace | Frontend resource not loaded | Clear browser cache, ensure the integration is loaded in Settings > Devices & Services |
| "Microphone blocked" | Browser permission denied | Click the lock icon in the address bar and allow microphone access |

For more detailed troubleshooting, see [troubleshooting.md](troubleshooting.md).
