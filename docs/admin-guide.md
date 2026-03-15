# HA VoIP Administration Guide

Comprehensive guide for system administrators covering certificate management, port configuration, NAT traversal, user management, recording, backup, monitoring, federation, security, and performance tuning.

---

## Table of Contents

1. [Certificate Management](#1-certificate-management)
2. [Port Configuration](#2-port-configuration)
3. [NAT Traversal Configuration](#3-nat-traversal-configuration)
4. [User and Extension Management](#4-user-and-extension-management)
5. [Recording Management](#5-recording-management)
6. [Backup and Restore](#6-backup-and-restore)
7. [Monitoring Setup](#7-monitoring-setup)
8. [Federation Setup](#8-federation-setup)
9. [Security Hardening](#9-security-hardening)
10. [Performance Tuning](#10-performance-tuning)

---

## 1. Certificate Management

### Self-Signed Certificates

The engine generates a self-signed certificate at startup when no certificate is configured. This is stored at:
```
/var/lib/voip-engine/certs/cert.pem
/var/lib/voip-engine/certs/key.pem
```

To regenerate:
```bash
rm /var/lib/voip-engine/certs/cert.pem
systemctl restart voip-engine
```

### ACME (Let's Encrypt)

Configure ACME in the engine config:
```yaml
tls:
  cert_path: /var/lib/voip-engine/certs/cert.pem
  key_path: /var/lib/voip-engine/certs/key.pem

acme:
  enabled: true
  domain: voip.example.com
  email: admin@example.com
  ca_url: https://acme-v02.api.letsencrypt.org/directory
  challenge_type: http-01      # or dns-01
  dns_provider: cloudflare     # if using dns-01
  dns_api_token: "..."         # provider-specific
```

The engine automatically renews certificates 30 days before expiry. Renewal is logged at the `info` level.

### Manual Certificate Upload

1. Place your PEM files in an accessible directory.
2. Update the engine config:
   ```yaml
   tls:
     cert_path: /ssl/fullchain.pem
     key_path: /ssl/privkey.pem
   ```
3. If using a certificate chain, concatenate the intermediate CA to the end of `fullchain.pem`.
4. Restart the engine.

### Certificate Rotation

When rotating certificates:
1. Place the new cert and key files alongside the old ones (e.g. `cert-new.pem`).
2. Update `cert_path` and `key_path` in the config.
3. Send `SIGHUP` to the engine process for a zero-downtime reload:
   ```bash
   kill -HUP $(pidof voip-engine)
   ```
   Active calls are not interrupted; new connections use the new certificate.

---

## 2. Port Configuration

### Default Ports

| Port | Protocol | Config Key | Default |
|---|---|---|---|
| SIP UDP | UDP | `sip.udp_port` | 5060 |
| SIP TCP | TCP | `sip.tcp_port` | 5060 |
| SIP TLS | TCP | `sip.tls_port` | 5061 |
| SIP WS | TCP | `sip.ws_port` | 8088 |
| SIP WSS | TCP | `sip.wss_port` | 8089 |
| RTP start | UDP | `media.rtp_port_start` | 10000 |
| RTP end | UDP | `media.rtp_port_end` | 20000 |
| TURN | UDP+TCP | `turn.port` | 3478 |
| TURN TLS | TCP | `turn.tls_port` | 5349 |
| TURN alt | TCP | `turn.alt_port` | 443 |
| gRPC | TCP | `api.grpc_port` | 50051 |
| HTTP | TCP | `api.http_port` | 8080 |

### Binding to Specific Interfaces

By default, all services bind to `0.0.0.0`. To restrict:
```yaml
sip:
  bind_addr: "192.168.1.50"

api:
  bind_addr: "127.0.0.1"    # gRPC only on localhost
```

### Port Conflicts

If port 5060 is occupied (e.g. by another PBX), use an alternative:
```yaml
sip:
  udp_port: 15060
  tcp_port: 15060
  tls_port: 15061
```

Update the HA integration config to match.

---

## 3. NAT Traversal Configuration

### STUN

STUN is used for server-reflexive candidate discovery. Configure one or more STUN servers:

```yaml
# In the HA integration options or engine config
stun_servers:
  - stun:stun.l.google.com:19302
  - stun:stun1.l.google.com:19302
```

### TURN (Embedded)

The engine includes a built-in TURN server:

```yaml
turn:
  enabled: true
  port: 3478
  tls_port: 5349
  alt_port: 443
  realm: voip.example.com
  shared_secret: "a-long-random-secret-at-least-32-chars"
  max_allocations_per_ip: 10
  allocation_lifetime_sec: 600
  relay_port_start: 49152
  relay_port_end: 65535
  rate_limit_per_sec: 50
```

### TURN (External)

To use an external TURN server (e.g. coturn):

```yaml
# In the HA integration options
turn_server: "turn:turn.example.com:3478"
turn_username: "havoip"
turn_password: "turnpassword"
```

### External Host Discovery

Set the public IP or hostname so that SDP offers contain the correct address:
```yaml
sip:
  domain: voip.example.com   # Or the public IP
```

In the HA integration config, set `external_host` to your public IP or domain.

---

## 4. User and Extension Management

### Creating Extensions

Via gRPC (programmatic):
```bash
grpcurl -plaintext -d '{
  "number": "100",
  "display_name": "Alice",
  "password": "StrongP@ss1",
  "transport": "wss",
  "voicemail_enabled": true,
  "max_concurrent_calls": 2
}' localhost:50051 voip.VoipService/CreateExtension
```

Via the HA integration config flow: add extensions in the format `number, name, password` during setup.

### Listing Extensions

```bash
grpcurl -plaintext localhost:50051 voip.VoipService/ListExtensions
```

### Deleting Extensions

```bash
grpcurl -plaintext -d '{"extension_id": "ext-001"}' \
  localhost:50051 voip.VoipService/DeleteExtension
```

### Password Changes

Delete and recreate the extension, or update the password in the database directly:
```sql
UPDATE extensions SET password_hash = ? WHERE number = '100';
```

### Extension Numbering Conventions

| Range | Purpose |
|---|---|
| 100-199 | User extensions |
| 200-299 | Conference rooms |
| 300-399 | Ring groups |
| 900-999 | Service codes (voicemail, parking, etc.) |

---

## 5. Recording Management

### Enabling Recording

```yaml
recording:
  enabled: true
  directory: /var/lib/voip-engine/recordings
  format: opus            # or "wav"
  encrypt: false
  max_disk_mb: 5000       # 5 GB limit, 0 = unlimited
  retention_days: 90      # Auto-delete after 90 days, 0 = keep forever
```

### Encrypted Recordings

```yaml
recording:
  enabled: true
  encrypt: true
  encryption_key: "base64-encoded-32-byte-aes-key"
```

Generate a key:
```bash
openssl rand -base64 32
```

Decrypt a recording:
```bash
voip-engine decrypt-recording \
  --key "base64key" \
  --input recording-001.opus.enc \
  --output recording-001.opus
```

### Storage Management

Monitor disk usage:
```bash
du -sh /var/lib/voip-engine/recordings/
```

The engine automatically purges recordings older than `retention_days`. Manual cleanup:
```bash
find /var/lib/voip-engine/recordings -name "*.opus" -mtime +90 -delete
```

---

## 6. Backup and Restore

### What to Back Up

| Item | Path | Description |
|---|---|---|
| Configuration | `/etc/voip-engine/config.yaml` | Engine config |
| Database | `/var/lib/voip-engine/voip-engine.db` | SQLite database (extensions, call history, routing rules) |
| Certificates | `/var/lib/voip-engine/certs/` | TLS certificates and keys |
| Recordings | `/var/lib/voip-engine/recordings/` | Call recordings |
| HA config | `/config/.storage/core.config_entries` | HA integration config entry |

### Backup Script

```bash
#!/bin/bash
BACKUP_DIR="/backup/voip-$(date +%Y%m%d)"
mkdir -p "$BACKUP_DIR"

# Stop engine for consistent DB backup
systemctl stop voip-engine

cp /etc/voip-engine/config.yaml "$BACKUP_DIR/"
cp /var/lib/voip-engine/voip-engine.db "$BACKUP_DIR/"
cp -r /var/lib/voip-engine/certs/ "$BACKUP_DIR/"
tar czf "$BACKUP_DIR/recordings.tar.gz" /var/lib/voip-engine/recordings/

systemctl start voip-engine
echo "Backup completed: $BACKUP_DIR"
```

### Restore

1. Stop the engine.
2. Copy the backed-up files to their original locations.
3. Start the engine.
4. In HA, go to Settings > Devices & Services and verify the integration reconnects.

---

## 7. Monitoring Setup

### Prometheus

The engine exports Prometheus metrics on the HTTP port (default 8080):

```yaml
# prometheus.yml
scrape_configs:
  - job_name: voip-engine
    scrape_interval: 15s
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: /metrics
```

### Key Metrics

| Metric | Type | Description |
|---|---|---|
| `voip_active_calls` | Gauge | Current active calls |
| `voip_active_registrations` | Gauge | Registered extensions |
| `voip_active_turn_allocations` | Gauge | Active TURN allocations |
| `voip_total_calls` | Counter | Total calls since start |
| `voip_call_drops` | Counter | Dropped calls |
| `voip_call_setup_duration_seconds` | Histogram | Call setup time |
| `voip_rtp_jitter_ms` | Histogram | RTP jitter |
| `voip_rtp_packet_loss_percent` | Histogram | Packet loss |
| `voip_process_cpu_seconds_total` | Counter | Process CPU usage |
| `voip_process_resident_memory_bytes` | Gauge | RSS memory |

### Grafana Dashboard

Import the provided dashboard from `monitoring/grafana-dashboard.json`:

1. Open Grafana > Dashboards > Import.
2. Upload or paste the JSON.
3. Select your Prometheus data source.

### Alerting Rules

Example Prometheus alerting rules:

```yaml
groups:
  - name: voip
    rules:
      - alert: VoIPEngineDown
        expr: up{job="voip-engine"} == 0
        for: 1m
        annotations:
          summary: "VoIP engine is down"

      - alert: HighCallDropRate
        expr: rate(voip_call_drops[5m]) > 0.1
        for: 5m
        annotations:
          summary: "Call drop rate exceeds 10%"

      - alert: HighJitter
        expr: histogram_quantile(0.95, rate(voip_rtp_jitter_ms_bucket[5m])) > 50
        for: 5m
        annotations:
          summary: "P95 jitter exceeds 50ms"
```

---

## 8. Federation Setup

Federation allows multiple HA VoIP instances to call each other across sites.

### DNS SRV Records

Publish SRV records for your domain:
```
_sip._udp.voip.example.com. 3600 IN SRV 10 10 5060 sip1.voip.example.com.
_sip._tcp.voip.example.com. 3600 IN SRV 10 10 5060 sip1.voip.example.com.
_sips._tcp.voip.example.com. 3600 IN SRV 10 10 5061 sip1.voip.example.com.
```

### Trunk Configuration

Add a federation trunk in the engine config or via the routing API:

```bash
grpcurl -plaintext -d '{
  "pattern": "^2[0-9]{2}$",
  "destination": "sip:site-b.example.com:5060",
  "priority": 10,
  "description": "Route 2xx extensions to Site B"
}' localhost:50051 voip.VoipService/SetRoutingRule
```

### Cross-Site TLS

For secure federation, enable TLS on both engines and configure mutual TLS:
```yaml
tls:
  cert_path: /ssl/cert.pem
  key_path: /ssl/key.pem
  ca_path: /ssl/federation-ca.pem
  require_client_cert: true
```

---

## 9. Security Hardening

### Restrict API Access

- Bind gRPC to localhost only: `api.bind_addr: "127.0.0.1"`
- Use strong API keys (at least 32 characters, random).
- Enable mTLS for gRPC: `api.enable_mtls: true`

### Disable Unused Transports

```yaml
sip:
  enable_udp: false   # If only using WebSocket
  enable_tcp: false
  enable_tls: false
  enable_ws: false
  enable_wss: true    # WSS only
```

### Strong SIP Passwords

- Minimum 12 characters with mixed case, numbers, and symbols.
- The engine enforces digest authentication (RFC 2617 / RFC 7616).

### Rate Limiting

Configure aggressive rate limits if exposed to the internet:
```yaml
api:
  rate_limit_per_sec: 20

turn:
  rate_limit_per_sec: 10
  max_allocations_per_ip: 3
```

### Log Auditing

Enable JSON logging for machine-parseable audit trails:
```yaml
logging:
  level: info
  format: json
  file: /var/log/voip-engine/engine.log
```

Rotate logs with logrotate:
```
/var/log/voip-engine/engine.log {
    daily
    rotate 30
    compress
    missingok
    notifempty
    postrotate
        kill -USR1 $(pidof voip-engine) 2>/dev/null || true
    endscript
}
```

---

## 10. Performance Tuning

### System Limits

Increase file descriptor limits for the engine process:
```bash
# /etc/security/limits.d/voip-engine.conf
voip-engine soft nofile 65536
voip-engine hard nofile 65536
```

Or in the systemd unit:
```ini
[Service]
LimitNOFILE=65536
```

### RTP Port Range

A larger port range allows more concurrent calls:
```yaml
media:
  rtp_port_start: 10000
  rtp_port_end: 30000    # Supports up to 10,000 concurrent calls
```

Each call uses 2 ports (RTP + RTCP).

### Database

For deployments with more than 100 extensions or high call volume, switch to PostgreSQL:
```yaml
database:
  backend: postgres
  postgres_url: "postgresql://voip:password@localhost/voip_engine"
  max_connections: 20
```

### Memory

The engine uses approximately:
- 50 MB base
- 2 MB per active call (with SRTP)
- 0.5 MB per TURN allocation

For 250 concurrent calls, allocate at least 1 GB of RAM.

### CPU

The engine is async and scales well across cores. Typical usage:
- Idle: < 1% of one core
- 50 concurrent calls: ~10% of one core
- 250 concurrent calls: ~50% of one core

For very high call volumes, run multiple engine instances behind a SIP load balancer (e.g. Kamailio).

### Jitter Buffer Tuning

```yaml
media:
  jitter_buffer_min_ms: 20    # Low-latency LAN
  jitter_buffer_max_ms: 200   # Absorb network jitter
```

For Wi-Fi or mobile networks, increase `jitter_buffer_max_ms` to 300-500.

### Connection Pool

Increase the gRPC connection pool for high HA-to-engine traffic:
```yaml
api:
  max_connections: 10   # Default is sufficient for most deployments
```
