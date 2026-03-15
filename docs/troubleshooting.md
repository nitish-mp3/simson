# HA VoIP Troubleshooting Guide

This document covers the most common issues encountered when deploying and using HA VoIP, along with symptoms, diagnostic steps, and solutions.

---

## Table of Contents

1. [No Audio / One-Way Audio](#1-no-audio--one-way-audio)
2. [WSS Connection Failures](#2-wss-connection-failures)
3. [ICE Gathering Failures](#3-ice-gathering-failures)
4. [TURN Allocation Failures](#4-turn-allocation-failures)
5. [Certificate Issues](#5-certificate-issues)
6. [NAT Traversal Problems](#6-nat-traversal-problems)
7. [Browser Compatibility](#7-browser-compatibility)
8. [Firewall Configuration](#8-firewall-configuration)
9. [Support Bundle Creation](#9-support-bundle-creation)

---

## 1. No Audio / One-Way Audio

### Symptoms

- Call connects (state shows "In Call") but one or both sides hear silence.
- Audio works in one direction only.
- Intermittent audio dropouts.

### Diagnosis Steps

1. **Open the diagnostics panel** in the VoIP card (gear icon) and run the network test.
2. **Check ICE candidates:** In Chrome, navigate to `chrome://webrtc-internals` and inspect the active PeerConnection.  Look at the nominated ICE candidate pair.
   - If the local candidate is `typ host` and the remote is `typ srflx`, one-way audio is likely caused by asymmetric NAT.
   - If no candidate pair is nominated, see [ICE Gathering Failures](#3-ice-gathering-failures).
3. **Check the engine logs:**
   ```bash
   docker logs voip-engine 2>&1 | grep -i "rtp\|media\|srtp"
   ```
   Look for `SRTP unprotect failed` (key mismatch) or `No RTP packets received`.
4. **Verify firewall rules:** Ensure UDP ports 10000-20000 are open bidirectionally between the HA host and the client network.

### Solutions

| Cause | Fix |
|---|---|
| Firewall blocks RTP | Open UDP 10000-20000 on host firewall and any intermediate NAT device |
| Symmetric NAT | Configure a TURN server (see [NAT Traversal](#6-nat-traversal-problems)) |
| SRTP key mismatch | Ensure both sides negotiate DTLS-SRTP (check `a=fingerprint` in SDP) |
| Wrong external IP | Set `external_host` in the integration config to the correct public IP |
| Codec mismatch | Verify both endpoints support at least one common codec (Opus recommended) |
| Browser autoplay policy | Click inside the VoIP card after page load to satisfy autoplay requirements |

---

## 2. WSS Connection Failures

### Symptoms

- VoIP card shows "Connecting..." indefinitely.
- Browser console shows `WebSocket connection to 'wss://...' failed`.
- Card displays "Engine offline".

### Diagnosis Steps

1. Open browser DevTools (F12) > Console tab. Look for WebSocket error messages.
2. Open the Network tab, filter by "WS", and check the status code:
   - **101** = successful upgrade (normal).
   - **403** = authentication failure.
   - **502/504** = reverse proxy issue.
3. Test raw WebSocket connectivity:
   ```bash
   wscat -c wss://your-ha-host:8123/api/websocket
   ```
4. Check TLS certificate validity in the browser address bar.

### Solutions

| Cause | Fix |
|---|---|
| Self-signed cert not trusted | Import the CA into the browser, or use ACME for a trusted cert |
| Reverse proxy not forwarding WSS | Add WebSocket upgrade directives to nginx/Caddy config |
| Wrong port | Verify `ws_port` in integration config matches the engine's actual listen port |
| Authentication expired | Re-authenticate the HA session (reload the page) |
| Mixed content block | If HA uses HTTPS, the WSS endpoint must also use TLS |

**Nginx example for WSS proxying:**

```nginx
location /api/voip/ws {
    proxy_pass http://127.0.0.1:8586;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
    proxy_read_timeout 86400;
}
```

---

## 3. ICE Gathering Failures

### Symptoms

- Call never connects (stuck at "Trying" or "Ringing").
- Diagnostics panel shows "ICE gathering: failed".
- `chrome://webrtc-internals` shows no `srflx` or `relay` candidates.

### Diagnosis Steps

1. Run the built-in diagnostics (VoIP card gear icon > Run Test).
2. Check that the STUN server is reachable:
   ```bash
   stun stun.l.google.com:19302
   ```
3. If using TURN, verify the TURN server is reachable:
   ```bash
   turnutils_uclient -T -u username -w password turn-server:3478
   ```
4. Check for corporate firewalls that block UDP entirely.

### Solutions

| Cause | Fix |
|---|---|
| STUN server unreachable | Change to a reachable STUN server or run your own |
| UDP blocked by firewall | Enable the TURN server with TCP and TLS fallback |
| No TURN configured | Add a TURN server in the integration options |
| Browser WebRTC disabled | Ensure `media.peerconnection.enabled` is `true` (Firefox) |

---

## 4. TURN Allocation Failures

### Symptoms

- Engine logs show `TURN allocation rejected: 401 Unauthorized` or `438 Stale Nonce`.
- Diagnostics show "TURN: fail".
- Calls fail only when clients are on different networks.

### Diagnosis Steps

1. Check engine logs for TURN-specific errors:
   ```bash
   docker logs voip-engine 2>&1 | grep -i "turn\|alloc\|stun"
   ```
2. Verify credentials:
   ```bash
   turnutils_uclient -T -u testuser -w testpass your-host:3478
   ```
3. Check rate limits -- the default is 50 requests/sec/IP.
4. Check the maximum allocations per IP (default 10).

### Solutions

| Cause | Fix |
|---|---|
| Invalid credentials | Update TURN username/password in integration options |
| Expired ephemeral credentials | Verify the shared secret matches between engine and clients |
| Rate limited | Increase `turn.rate_limit_per_sec` in engine config |
| Max allocations reached | Increase `turn.max_allocations_per_ip` |
| Port 3478 blocked | Enable TLS fallback on port 5349 or 443 |
| Stale nonce (438) | This is normal; the client should retry with the new nonce automatically |

---

## 5. Certificate Issues

### Symptoms

- `ERR_CERT_AUTHORITY_INVALID` in the browser.
- ACME certificate request fails with `urn:ietf:params:acme:error:unauthorized`.
- Engine fails to start with `TLS error: certificate not found`.

### Diagnosis Steps

1. Check the certificate files exist and are readable:
   ```bash
   ls -la /config/certs/cert.pem /config/certs/key.pem
   openssl x509 -in /config/certs/cert.pem -noout -dates
   ```
2. Verify the certificate chain is complete:
   ```bash
   openssl verify -CAfile /etc/ssl/certs/ca-certificates.crt /config/certs/cert.pem
   ```
3. For ACME issues, check engine logs for challenge validation errors.

### Solutions

| Cause | Fix |
|---|---|
| Self-signed warning | Expected; import the CA or switch to ACME |
| ACME challenge fails | Ensure port 80 is reachable from the internet, or use DNS-01 challenge |
| Certificate expired | Restart the engine to trigger renewal, or manually renew |
| Wrong file permissions | Ensure the engine process can read the cert/key files (`chmod 644 cert.pem`) |
| Key/cert mismatch | Regenerate both from the same CSR |
| Missing intermediate CA | Append the intermediate certificate to `cert.pem` |

---

## 6. NAT Traversal Problems

### Symptoms

- Calls work on the same LAN but fail across networks.
- Only `host` candidates appear in ICE gathering.
- TURN relay candidates appear but the call has high latency or drops.

### Diagnosis Steps

1. Determine your NAT type:
   ```bash
   stun -v stun.l.google.com:19302
   ```
   Look for "Full Cone", "Restricted Cone", "Port Restricted Cone", or "Symmetric".
2. If symmetric NAT is detected, TURN is mandatory.
3. Check that the engine's `external_host` config matches your public IP.

### Solutions

| NAT Type | Solution |
|---|---|
| Full Cone | STUN alone is sufficient |
| Restricted Cone | STUN usually works; TURN as fallback |
| Port Restricted Cone | STUN usually works; TURN recommended |
| Symmetric | **TURN is required** -- configure a TURN server |
| Double NAT | Use TURN; consider DMZ or port forwarding to eliminate one NAT layer |
| Carrier-grade NAT (CGNAT) | TURN is the only option; use TLS 443 for best compatibility |

---

## 7. Browser Compatibility

### Supported Browsers

| Browser | Minimum Version | Notes |
|---|---|---|
| Chrome / Edge | 90+ | Full support, recommended |
| Firefox | 95+ | Full support |
| Safari (macOS) | 16+ | Full support |
| Safari (iOS) | 16.4+ | Requires user gesture to start audio |
| Samsung Internet | 18+ | Full support |

### Known Issues

- **Safari iOS:** Audio playback requires a user tap (autoplay policy). The VoIP card handles this by requiring a tap on the answer button.
- **Firefox Private Mode:** WebRTC may be restricted. Check `about:config` > `media.peerconnection.enabled`.
- **Brave:** Shields may block WebRTC. Add an exception for the HA domain.
- **Older browsers:** WebRTC and the `AudioContext` API are required. Browsers without these APIs cannot use HA VoIP.

---

## 8. Firewall Configuration

### Minimum Ports

| Port | Protocol | Direction | Purpose |
|---|---|---|---|
| 5060 | UDP + TCP | Inbound | SIP signalling |
| 5061 | TCP (TLS) | Inbound | SIP over TLS |
| 8586 | TCP | Inbound | WebSocket (engine to HA) |
| 50051 | TCP | Inbound | gRPC (HA to engine) |
| 10000-20000 | UDP | Bidirectional | RTP media |
| 3478 | UDP + TCP | Inbound | STUN / TURN |
| 5349 | TCP (TLS) | Inbound | TURN over TLS |
| 443 | TCP (TLS) | Inbound | TURN over TLS (alt port) |

### iptables Example (Linux)

```bash
# SIP
iptables -A INPUT -p udp --dport 5060 -j ACCEPT
iptables -A INPUT -p tcp --dport 5060 -j ACCEPT
iptables -A INPUT -p tcp --dport 5061 -j ACCEPT

# RTP
iptables -A INPUT -p udp --dport 10000:20000 -j ACCEPT

# TURN
iptables -A INPUT -p udp --dport 3478 -j ACCEPT
iptables -A INPUT -p tcp --dport 3478 -j ACCEPT
iptables -A INPUT -p tcp --dport 5349 -j ACCEPT

# gRPC (restrict to localhost if engine is local)
iptables -A INPUT -p tcp --dport 50051 -s 127.0.0.1 -j ACCEPT
```

### Windows Defender Firewall

Open **Windows Defender Firewall > Advanced Settings > Inbound Rules** and create rules for the ports listed above.  For the RTP range, specify the range `10000-20000` in the port field.

---

## 9. Support Bundle Creation

When filing a bug report, include a support bundle to help maintainers diagnose the issue.

### Automatic (via HA)

1. Go to **Settings > System > Repairs**.
2. Find the HA VoIP integration and click **Download Diagnostics**.
3. The download includes:
   - Integration configuration (passwords redacted)
   - Engine health and metrics
   - Active call state
   - Extension registration status
   - Recent engine log lines (last 200)

### Manual

Collect the following files and logs:

```bash
# Engine logs (last 500 lines)
docker logs --tail 500 voip-engine > engine.log 2>&1

# Engine config (redact passwords!)
cp /etc/voip-engine/config.yaml config_redacted.yaml
sed -i 's/password:.*/password: REDACTED/' config_redacted.yaml

# Engine health
curl http://localhost:8080/health > health.json

# Engine metrics
curl http://localhost:8080/metrics > metrics.txt

# HA integration log
grep -i "ha_voip" /config/home-assistant.log > ha_voip.log
```

Archive all files and attach them to your GitHub issue.

### WebRTC Diagnostics

In Chrome, navigate to `chrome://webrtc-internals`, reproduce the issue, then click **Download the PeerConnection updates and stats data**. Attach the resulting JSON file to the bug report.
