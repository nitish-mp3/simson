# HA VoIP Security Guide

This guide covers the security architecture of HA VoIP, TLS configuration, media encryption, authentication mechanisms, credential management, encrypted recordings, network security, firewall rules, and security audit procedures.

---

## Table of Contents

1. [TLS Configuration](#1-tls-configuration)
2. [SRTP and DTLS-SRTP](#2-srtp-and-dtls-srtp)
3. [Authentication Mechanisms](#3-authentication-mechanisms)
4. [Credential Rotation](#4-credential-rotation)
5. [Encrypted Recordings](#5-encrypted-recordings)
6. [Network Security](#6-network-security)
7. [Firewall Rules](#7-firewall-rules)
8. [Security Audit Procedures](#8-security-audit-procedures)

---

## 1. TLS Configuration

### Overview

HA VoIP uses TLS for:
- **SIP signalling** (SIP-over-TLS on port 5061, SIP-over-WSS on port 8089)
- **gRPC** (between HA integration and voip-engine)
- **TURN** (TURN-over-TLS on port 5349 and fallback 443)
- **HTTP API** (HTTPS for metrics and provisioning)

### TLS Versions

The engine supports TLS 1.2 and TLS 1.3. TLS 1.0 and 1.1 are disabled by default and cannot be enabled.

### Cipher Suites

Default cipher suite order (TLS 1.2):
```
TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384
TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256
TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256
TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384
TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256
TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256
```

TLS 1.3 cipher suites (always enabled when TLS 1.3 is negotiated):
```
TLS_AES_256_GCM_SHA384
TLS_AES_128_GCM_SHA256
TLS_CHACHA20_POLY1305_SHA256
```

### Certificate Requirements

- **Key type:** RSA 2048+ or ECDSA P-256/P-384
- **Signature algorithm:** SHA-256 or SHA-384 (SHA-1 is rejected)
- **Key usage:** Digital Signature, Key Encipherment
- **Extended key usage:** TLS Web Server Authentication
- **Subject Alternative Name (SAN):** Must include the domain or IP used for connections

### Mutual TLS (mTLS)

Enable mTLS for gRPC to ensure only authorized HA instances can communicate with the engine:

```yaml
tls:
  cert_path: /ssl/server-cert.pem
  key_path: /ssl/server-key.pem
  ca_path: /ssl/client-ca.pem
  require_client_cert: true

api:
  enable_mtls: true
```

Generate client certificates:
```bash
# Generate CA (one-time)
openssl req -new -x509 -days 3650 -keyout ca-key.pem -out ca-cert.pem \
  -subj "/CN=HA VoIP Internal CA"

# Generate client cert
openssl req -new -keyout client-key.pem -out client-csr.pem \
  -subj "/CN=ha-integration"
openssl x509 -req -in client-csr.pem -CA ca-cert.pem -CAkey ca-key.pem \
  -CAcreateserial -out client-cert.pem -days 365
```

---

## 2. SRTP and DTLS-SRTP

### Media Encryption

All media (audio) is encrypted using SRTP (Secure RTP). The engine supports two key exchange mechanisms:

| Method | Protocol | Usage |
|---|---|---|
| DTLS-SRTP | DTLS 1.2 over UDP | WebRTC calls (browser to engine) |
| SDES | SDP attributes | Legacy SIP device calls |

DTLS-SRTP is the default and recommended method. SDES is available for compatibility with older SIP phones but is less secure because keys are exchanged in the signalling path.

### Configuration

```yaml
media:
  enable_srtp: true       # Enable SRTP (default: true)
  srtp_profiles:
    - SRTP_AES128_CM_HMAC_SHA1_80   # Most compatible
    - SRTP_AEAD_AES_128_GCM          # More secure, less compatible
```

### DTLS Fingerprint Verification

The engine validates DTLS certificate fingerprints against the `a=fingerprint` attribute in the SDP offer/answer. Mismatched fingerprints cause the DTLS handshake to fail and the call to be rejected.

### Disabling Unencrypted Media

To reject calls that do not negotiate SRTP:
```yaml
media:
  require_srtp: true    # Reject non-SRTP calls
```

---

## 3. Authentication Mechanisms

### SIP Digest Authentication

Extensions authenticate to the SIP registrar using HTTP Digest authentication (RFC 2617 / RFC 7616):

1. Client sends REGISTER without credentials.
2. Engine responds with `401 Unauthorized` and a `WWW-Authenticate` header containing a `nonce`.
3. Client recomputes the response hash using `HA1 = MD5(username:realm:password)` and `HA2 = MD5(method:uri)`, then resends REGISTER with the `Authorization` header.
4. Engine validates the response hash.

The engine uses `qop=auth` and generates a new nonce for each challenge to prevent replay attacks.

### API Key Authentication

gRPC and REST APIs use bearer token authentication:
```
Authorization: Bearer <api_key>
```

API keys are configured in the engine config:
```yaml
api:
  api_keys:
    - "k8s-secret-ref-or-random-64-char-string-1"
    - "k8s-secret-ref-or-random-64-char-string-2"
```

Generate a secure API key:
```bash
openssl rand -hex 32
```

### TURN Credentials

#### Static Credentials
```yaml
turn:
  users:
    - username: "webrtc-user"
      password: "strong-random-password"
```

#### Ephemeral Credentials (Recommended)

Ephemeral credentials are time-limited and derived from a shared secret:

```yaml
turn:
  shared_secret: "a-very-long-random-string-at-least-32-bytes"
```

The credential generation algorithm:
1. Username = `unix_timestamp:user_id` (where timestamp is the expiry time).
2. Password = `Base64(HMAC-SHA1(shared_secret, username))`.

Clients request credentials from the engine, which generates them on-the-fly. Credentials typically expire after 24 hours.

---

## 4. Credential Rotation

### SIP Passwords

Rotate SIP passwords by deleting and recreating the extension:
```bash
grpcurl -plaintext -d '{"extension_id": "ext-001"}' \
  localhost:50051 voip.VoipService/DeleteExtension

grpcurl -plaintext -d '{
  "number": "100",
  "display_name": "Alice",
  "password": "NewStrongP@ss!",
  "transport": "wss"
}' localhost:50051 voip.VoipService/CreateExtension
```

### API Keys

1. Add a new API key to the config.
2. Reload the engine (`kill -HUP`).
3. Update all clients to use the new key.
4. Remove the old key from the config.
5. Reload again.

### TLS Certificates

See [Certificate Management](admin-guide.md#1-certificate-management) for zero-downtime rotation.

### TURN Shared Secret

1. Update `turn.shared_secret` in the config.
2. Reload the engine.
3. Existing allocations continue to work until they expire (default 600 seconds).
4. New allocations use the new secret.

### Rotation Schedule

| Credential | Recommended Rotation |
|---|---|
| SIP passwords | Every 90 days |
| API keys | Every 90 days |
| TLS certificates | Auto-renewed (ACME) or every 365 days |
| TURN shared secret | Every 30 days |
| TURN ephemeral credentials | Auto-expire every 24 hours |

---

## 5. Encrypted Recordings

### At-Rest Encryption

Call recordings are encrypted using AES-256-GCM before being written to disk:

```yaml
recording:
  enabled: true
  encrypt: true
  encryption_key: "base64-encoded-32-byte-key"
```

The encryption process:
1. A random 12-byte nonce is generated per recording.
2. The audio data is encrypted with AES-256-GCM using the configured key.
3. The nonce is prepended to the ciphertext.
4. The file is written with a `.enc` suffix (e.g. `recording-001.opus.enc`).

### Key Management

- Store the encryption key in a secrets manager (Vault, AWS KMS, etc.).
- If using the HA add-on, store the key in the add-on configuration (it is not persisted to the HA database).
- Rotate the key by updating the config and restarting. Old recordings remain encrypted with the old key and must be decrypted with it.

### Decrypting Recordings

```bash
voip-engine decrypt-recording \
  --key "base64-encoded-key" \
  --input /recordings/rec-001.opus.enc \
  --output /tmp/rec-001.opus
```

---

## 6. Network Security

### Principle of Least Privilege

- Bind signalling and management interfaces to specific IPs.
- Expose only the ports required for your deployment scenario.
- Use a dedicated VLAN for VoIP traffic if possible.

### Network Segmentation

Recommended topology:
```
[Internet] --> [Firewall] --> [DMZ: TURN server]
                          --> [LAN: HA + VoIP Engine]
                                    |
                              [VoIP VLAN: SIP phones]
```

### DoS Protection

- Enable rate limiting on the TURN and SIP ports.
- Use fail2ban or similar to block IPs after repeated authentication failures:

```ini
# /etc/fail2ban/filter.d/voip-engine.conf
[Definition]
failregex = SIP auth failed from <HOST>
            TURN auth failed from <HOST>
```

```ini
# /etc/fail2ban/jail.d/voip-engine.conf
[voip-engine]
enabled = true
filter = voip-engine
logpath = /var/log/voip-engine/engine.log
maxretry = 5
bantime = 3600
```

### SIP-Specific Protections

- **Max-Forwards:** The engine enforces `Max-Forwards` and drops messages with a value of 0.
- **Loop detection:** The engine checks Via headers for loops and responds with `482 Loop Detected`.
- **Message size limits:** Configurable via `sip.max_message_size` (default 65535 bytes). Oversized messages are dropped.

---

## 7. Firewall Rules

### Minimal LAN Deployment

```bash
# Allow SIP from local network only
iptables -A INPUT -p udp -s 192.168.1.0/24 --dport 5060 -j ACCEPT
iptables -A INPUT -p tcp -s 192.168.1.0/24 --dport 5060 -j ACCEPT

# RTP from local network
iptables -A INPUT -p udp -s 192.168.1.0/24 --dport 10000:20000 -j ACCEPT

# gRPC from localhost only
iptables -A INPUT -p tcp -s 127.0.0.1 --dport 50051 -j ACCEPT

# Drop everything else to these ports
iptables -A INPUT -p udp --dport 5060 -j DROP
iptables -A INPUT -p tcp --dport 5060 -j DROP
iptables -A INPUT -p tcp --dport 50051 -j DROP
```

### Remote Access Deployment

```bash
# TURN (must be reachable from the internet)
iptables -A INPUT -p udp --dport 3478 -j ACCEPT
iptables -A INPUT -p tcp --dport 3478 -j ACCEPT
iptables -A INPUT -p tcp --dport 5349 -j ACCEPT
iptables -A INPUT -p tcp --dport 443 -j ACCEPT

# TURN relay ports
iptables -A INPUT -p udp --dport 49152:65535 -j ACCEPT

# SIP and RTP from local network
iptables -A INPUT -p udp -s 192.168.1.0/24 --dport 5060 -j ACCEPT
iptables -A INPUT -p udp -s 192.168.1.0/24 --dport 10000:20000 -j ACCEPT
```

### nftables Alternative

```nft
table inet voip {
  chain input {
    type filter hook input priority 0; policy drop;

    # TURN
    udp dport 3478 accept
    tcp dport { 3478, 5349, 443 } accept
    udp dport 49152-65535 accept

    # SIP (LAN only)
    ip saddr 192.168.1.0/24 udp dport 5060 accept
    ip saddr 192.168.1.0/24 tcp dport 5060 accept

    # RTP (LAN only)
    ip saddr 192.168.1.0/24 udp dport 10000-20000 accept

    # gRPC (localhost only)
    ip saddr 127.0.0.1 tcp dport 50051 accept
  }
}
```

---

## 8. Security Audit Procedures

### Pre-Deployment Checklist

- [ ] TLS is enabled for all external-facing interfaces (SIP TLS, WSS, TURN TLS)
- [ ] SRTP is required for all media (`media.require_srtp: true`)
- [ ] gRPC is bound to localhost or uses mTLS
- [ ] API keys are at least 32 characters and randomly generated
- [ ] SIP passwords are at least 12 characters with complexity requirements
- [ ] Rate limiting is configured for TURN and API
- [ ] Unused SIP transports are disabled
- [ ] Recording encryption is enabled if recording is active
- [ ] Log level is set to `info` (not `debug` in production)
- [ ] Firewall rules restrict access to required ports only
- [ ] fail2ban or equivalent is configured

### Periodic Audit (Monthly)

1. **Review registered extensions:** Check for unknown or stale registrations.
   ```bash
   grpcurl -plaintext localhost:50051 voip.VoipService/ListExtensions
   ```

2. **Check for brute-force attempts:** Search engine logs for repeated auth failures.
   ```bash
   grep "auth failed" /var/log/voip-engine/engine.log | awk '{print $NF}' | sort | uniq -c | sort -rn | head
   ```

3. **Verify TLS certificates are valid and not expiring soon.**
   ```bash
   openssl x509 -in /ssl/cert.pem -noout -dates -subject
   ```

4. **Review API key usage:** If the engine logs API key hashes, check for unknown keys.

5. **Check TURN allocations:** Verify no unexpected allocations from unknown IPs.

6. **Update dependencies:** Check for security advisories in Rust crates and Python packages.

### Penetration Testing

For production deployments, engage a security team to test:

1. **SIP fuzzing:** Use tools like `SIPVicious` or the included fuzz harness to test parser resilience.
2. **TURN abuse:** Attempt to use the TURN server as an open relay without valid credentials.
3. **TLS downgrade:** Verify that TLS 1.0/1.1 connections are refused.
4. **Authentication bypass:** Attempt to register an extension or make a call without credentials.
5. **DoS resilience:** Verify rate limiting functions under high request volume.

### Incident Response

If a security incident is suspected:

1. **Capture logs immediately:** Preserve engine logs, HA logs, and firewall logs.
2. **Revoke credentials:** Rotate all API keys, SIP passwords, and TURN secrets.
3. **Block the source IP:** Add a firewall rule or fail2ban ban.
4. **Review recordings:** Check if any recordings were accessed.
5. **Notify users:** If credentials may have been compromised.
6. **File a security issue:** Report vulnerabilities privately to the project maintainers.
