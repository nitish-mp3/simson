# HA VoIP Demo Setup Guide

Step-by-step instructions for running the full HA VoIP stack locally with Docker Compose, making a test call, testing TURN fallback, and viewing metrics.

---

## Prerequisites

- Docker 24+ and Docker Compose v2
- A machine with at least 2 GB of RAM
- Two browser tabs (Chrome recommended) for testing calls

---

## 1. Start the Demo Stack

Create a working directory and save the following `docker-compose.yml`:

```yaml
version: "3.9"

services:
  # ─────────────────────────────────────────────────────────────────
  # VoIP Engine
  # ─────────────────────────────────────────────────────────────────
  voip-engine:
    image: ghcr.io/ha-voip/voip-engine:latest
    container_name: voip-engine
    restart: unless-stopped
    network_mode: host
    volumes:
      - ./demo-config.yaml:/etc/voip-engine/config.yaml:ro
      - engine-data:/var/lib/voip-engine
    environment:
      VOIP__LOGGING__LEVEL: debug
      VOIP__LOGGING__FORMAT: text

  # ─────────────────────────────────────────────────────────────────
  # Home Assistant (for full integration testing)
  # ─────────────────────────────────────────────────────────────────
  homeassistant:
    image: ghcr.io/home-assistant/home-assistant:stable
    container_name: homeassistant
    restart: unless-stopped
    network_mode: host
    volumes:
      - ha-config:/config
    environment:
      TZ: "UTC"

  # ─────────────────────────────────────────────────────────────────
  # Prometheus (metrics collection)
  # ─────────────────────────────────────────────────────────────────
  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    restart: unless-stopped
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro
    command:
      - "--config.file=/etc/prometheus/prometheus.yml"
      - "--storage.tsdb.retention.time=7d"

  # ─────────────────────────────────────────────────────────────────
  # Grafana (metrics visualisation)
  # ─────────────────────────────────────────────────────────────────
  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: "admin"
    volumes:
      - grafana-data:/var/lib/grafana

volumes:
  engine-data:
  ha-config:
  grafana-data:
```

Create the engine configuration `demo-config.yaml`:

```yaml
sip:
  domain: "demo.local"
  udp_port: 5060
  enable_udp: true
  enable_tcp: true
  enable_ws: true
  enable_tls: false
  enable_wss: false

media:
  rtp_port_start: 10000
  rtp_port_end: 10100
  codecs: [opus, pcmu, pcma]
  enable_srtp: true

turn:
  enabled: true
  port: 3478
  tls_port: 5349
  alt_port: 8443
  realm: "demo.local"
  shared_secret: "demo-shared-secret-do-not-use-in-production"
  max_allocations_per_ip: 20
  rate_limit_per_sec: 100

database:
  backend: sqlite
  sqlite_path: /var/lib/voip-engine/demo.db

api:
  grpc_port: 50051
  http_port: 8080
  bind_addr: "0.0.0.0"
  api_keys:
    - "demo-api-key-for-testing-only"

logging:
  level: debug
  format: text

recording:
  enabled: true
  directory: /var/lib/voip-engine/recordings
  format: opus
  encrypt: false
```

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 10s

scrape_configs:
  - job_name: voip-engine
    static_configs:
      - targets: ["localhost:8080"]
```

Start everything:

```bash
docker compose up -d
```

Wait about 30 seconds for all services to initialise.

---

## 2. Create Demo Extensions

```bash
# Extension 100 - Alice
grpcurl -plaintext -d '{
  "number": "100",
  "display_name": "Alice",
  "password": "alice123",
  "transport": "wss",
  "voicemail_enabled": true,
  "max_concurrent_calls": 2
}' localhost:50051 voip.VoipService/CreateExtension

# Extension 101 - Bob
grpcurl -plaintext -d '{
  "number": "101",
  "display_name": "Bob",
  "password": "bob123",
  "transport": "wss",
  "voicemail_enabled": true,
  "max_concurrent_calls": 2
}' localhost:50051 voip.VoipService/CreateExtension
```

If `grpcurl` is not installed:
```bash
go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
```

---

## 3. Make a Test Call

### Option A: Via the Lovelace VoIP Card

1. Open Home Assistant at `http://localhost:8123`.
2. Complete the initial onboarding wizard.
3. Add the HA VoIP integration (Settings > Devices & Services > Add Integration > HA VoIP).
4. Add the VoIP card to a dashboard.
5. Open two browser tabs, register as extensions 100 and 101.
6. From extension 100, dial `101` and click Call.

### Option B: Via gRPC (headless)

```bash
# Originate a call from 100 to 101
grpcurl -plaintext -d '{
  "from_extension": "100",
  "to_extension": "101",
  "auto_answer": true,
  "record": true
}' localhost:50051 voip.VoipService/OriginateCall

# List active calls
grpcurl -plaintext localhost:50051 voip.VoipService/GetActiveCalls

# Hang up (replace call-id with the actual value)
grpcurl -plaintext -d '{"call_id": "CALL_ID_HERE"}' \
  localhost:50051 voip.VoipService/HangupCall
```

---

## 4. Test TURN Fallback

The TURN fallback chain is: UDP 3478 -> TCP 3478 -> TLS 5349 -> TLS 8443.

### Block UDP 3478 and verify TCP fallback:

```bash
# Block UDP TURN
sudo iptables -A INPUT -p udp --dport 3478 -j DROP

# Make a call -- it should use TCP 3478
# Check the engine logs:
docker logs voip-engine 2>&1 | grep -i "turn\|alloc"

# Remove the block
sudo iptables -D INPUT -p udp --dport 3478 -j DROP
```

### Block both UDP and TCP 3478:

```bash
sudo iptables -A INPUT -p udp --dport 3478 -j DROP
sudo iptables -A INPUT -p tcp --dport 3478 -j DROP

# The client should fall back to TLS 5349
# Verify in logs:
docker logs voip-engine 2>&1 | grep "5349"

# Clean up
sudo iptables -D INPUT -p udp --dport 3478 -j DROP
sudo iptables -D INPUT -p tcp --dport 3478 -j DROP
```

### Block everything except TLS 8443:

```bash
sudo iptables -A INPUT -p udp --dport 3478 -j DROP
sudo iptables -A INPUT -p tcp --dport 3478 -j DROP
sudo iptables -A INPUT -p tcp --dport 5349 -j DROP

# Should fall back to TLS 8443
docker logs voip-engine 2>&1 | grep "8443"

# Clean up
sudo iptables -D INPUT -p udp --dport 3478 -j DROP
sudo iptables -D INPUT -p tcp --dport 3478 -j DROP
sudo iptables -D INPUT -p tcp --dport 5349 -j DROP
```

---

## 5. View Metrics

### Prometheus

Open `http://localhost:9090` in your browser.

Useful queries:
```promql
# Active calls
voip_active_calls

# Call setup time (P95)
histogram_quantile(0.95, rate(voip_call_setup_duration_seconds_bucket[5m]))

# Packet loss
voip_rtp_packet_loss_percent

# Registrations
voip_active_registrations
```

### Grafana

1. Open `http://localhost:3000` (login: admin/admin).
2. Add Prometheus as a data source: Configuration > Data Sources > Add > Prometheus > URL: `http://localhost:9090`.
3. Import the dashboard from `monitoring/grafana-dashboard.json`, or create panels manually with the queries above.

### Engine Health Endpoint

```bash
curl -s http://localhost:8080/health | python3 -m json.tool
```

Output:
```json
{
  "healthy": true,
  "version": "0.1.0",
  "uptime_sec": 300,
  "components": {
    "sip": {"healthy": true, "message": ""},
    "turn": {"healthy": true, "message": ""},
    "database": {"healthy": true, "message": ""}
  }
}
```

### Raw Prometheus Metrics

```bash
curl -s http://localhost:8080/metrics
```

---

## 6. Tear Down

```bash
docker compose down -v
```

This removes all containers and volumes (including the database and recordings).
