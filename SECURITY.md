# Security Policy

ha-voip handles real-time voice communications and, by design, processes sensitive audio data, network credentials, and cryptographic material. We treat every security report with the highest priority.

## Supported Versions

| Version  | Supported          | Notes                                    |
|----------|--------------------|------------------------------------------|
| 1.x      | Yes                | Current stable release -- full support   |
| 0.9.x    | Yes                | Final pre-release series -- security patches only |
| < 0.9    | No                 | End of life -- please upgrade            |

Security patches are back-ported to all supported branches. Critical fixes ship within 48 hours of confirmation.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Instead, report vulnerabilities through one of these private channels:

### Email

Send a detailed report to:

```
security@ha-voip.dev
```

Encrypt your message with our PGP public key if the report contains exploit code, credentials, or other sensitive data:

```
PGP Key Fingerprint: 4A7B 9C3E D1F2 8056 E4A3  BC91 7D5F 2E68 A0C4 13B7
PGP Key Server:      keys.openpgp.org
Key ID:              A0C413B7
```

To retrieve the key:

```bash
gpg --keyserver keys.openpgp.org --recv-keys A0C413B7
```

### GitHub Security Advisories

You may also use GitHub's private vulnerability reporting feature:

1. Go to the [Security Advisories page](https://github.com/ha-voip/ha-voip/security/advisories).
2. Click **New draft security advisory**.
3. Fill in the details and submit.

### What to Include in Your Report

Please provide as much of the following as possible:

- **Description** of the vulnerability and its potential impact.
- **Affected component** (voip-engine, integration, frontend, TURN server, certificate manager).
- **Affected version(s)** and configuration.
- **Steps to reproduce**, including any scripts, packet captures, or proof-of-concept code.
- **Environment details**: operating system, Home Assistant version, network topology.
- **Suggested severity** using CVSS 3.1 if known.
- **Suggested fix** if you have one.

### Response Timeline

| Stage                          | Target Time       |
|--------------------------------|-------------------|
| Acknowledgment of report       | Within 24 hours   |
| Initial triage and severity    | Within 48 hours   |
| Status update to reporter      | Within 5 days     |
| Patch development              | Within 14 days    |
| Coordinated public disclosure  | Within 90 days    |

If the issue is critical (CVSS >= 9.0), we target patch release within 48 hours of confirmation and coordinate early notification with downstream package maintainers.

## Responsible Disclosure

We follow a coordinated disclosure process:

1. **Reporter** sends a private report via the channels above.
2. **ha-voip maintainers** acknowledge receipt and begin triage.
3. **Both parties agree** on a disclosure date (default: 90 days from report, earlier if a patch is ready).
4. **Maintainers develop and test** a fix on a private branch.
5. **Maintainers release** the patched version and publish a GitHub Security Advisory (GHSA) with CVE assignment.
6. **Reporter is credited** in the advisory (unless they request anonymity).

We will never take legal action against researchers who follow this responsible disclosure process.

## Security Measures Implemented

### Transport Security

- **SIP-over-TLS (SIPS)**: All SIP signaling defaults to TLS 1.3 with strong cipher suites. Plaintext SIP on port 5060 is available for LAN-only deployments but is disabled by default when a TLS certificate is present.
- **SRTP (RFC 3711)**: Media encryption is mandatory by default. The `srtp_required` configuration flag causes the engine to reject any call that fails SRTP negotiation. Supported profiles: `AEAD_AES_256_GCM`, `AES_256_CM_HMAC_SHA1_80`, `AES_128_CM_HMAC_SHA1_80`.
- **DTLS 1.2 / 1.3**: WebRTC data channels and media use DTLS key exchange. The engine generates per-session DTLS certificates with ECDSA P-256 keys.
- **TLS certificate management**: The built-in certificate manager provisions certificates via ACME (Let's Encrypt) with automatic renewal 30 days before expiry. It falls back to self-signed certificates for `.local` domains.

### Authentication and Access Control

- **SIP digest authentication**: All SIP registrations and calls require digest authentication (RFC 8760) with SHA-256. MD5 digest is disabled.
- **TURN credential rotation**: TURN relay credentials are time-limited HMAC tokens rotated every 24 hours (configurable). Stale credentials are rejected immediately.
- **Home Assistant user scoping**: Integration entities inherit HA's user and role model. Call recordings are accessible only to users with the appropriate scope.
- **Brute-force protection**: An integrated fail2ban-style mechanism bans IP addresses after configurable failed authentication attempts. Ban events are logged and optionally surfaced as HA persistent notifications.

### Data Protection

- **Encrypted recordings**: Call recordings are encrypted at rest using AES-256-GCM. The encryption key is derived from the HA instance's internal secret combined with a per-recording random nonce. Recordings cannot be decrypted outside the HA instance.
- **No cloud dependency**: All processing -- SIP, media, TURN relay, certificate management -- runs locally. No audio or metadata is sent to external services unless the user explicitly configures a SIP trunk.
- **Secure memory handling**: The VoIP engine (Rust) uses zeroing allocators for cryptographic key material. Audio buffers are zeroed on deallocation. No audio data is written to swap (mlockall is used where the OS supports it).

### Network Security

- **IP allowlisting**: The `allowed_networks` configuration restricts SIP registration and TURN allocation to specified CIDR ranges.
- **Rate limiting**: The SIP stack enforces per-IP rate limits on REGISTER, INVITE, and OPTIONS requests (default: 20 requests/second per IP, configurable).
- **TURN allocation limits**: The TURN server limits the number of concurrent allocations per authenticated user (default: 5) and the total bandwidth per allocation (default: 1 Mbps).
- **Port fallback without exposure**: The NAT traversal algorithm tries multiple ports in sequence but never opens additional listening ports on the host beyond those configured.

### Logging and Auditing

- **Security event logging**: Authentication failures, SRTP negotiation failures, certificate events, and ban/unban actions are logged at WARNING or CRITICAL level with structured JSON payloads.
- **Call detail records (CDR)**: Every call is recorded in the CDR database with timestamps, participants, duration, oRTP quality metrics, and the encryption profile used. CDRs are available as HA long-term statistics.
- **Prometheus metrics**: Security-relevant metrics (failed auth count, active bans, SRTP negotiation success rate, certificate expiry countdown) are exported on the `/metrics` endpoint for alerting.

## Security Audit Checklist

The following checklist is executed before every release. It is also available for operators performing their own review.

### SIP Stack

- [ ] Fuzz testing of the SIP parser with AFL++/cargo-fuzz (minimum 1 million iterations, zero crashes).
- [ ] SIP message injection tests: oversized headers, malformed Via, null bytes in URI, UTF-8 edge cases.
- [ ] SIP authentication bypass tests: replay attacks, nonce reuse, algorithm downgrade.
- [ ] REGISTER flood test: verify rate limiter and fail2ban engage before resource exhaustion.
- [ ] INVITE with spoofed From header: verify authentication is always enforced.
- [ ] BYE/CANCEL forgery: verify transaction matching prevents session teardown by unauthenticated parties.

### Media Path

- [ ] SRTP key negotiation: verify SDES keys are never logged and that oRTP rejects unencrypted fallback.
- [ ] DTLS certificate validation: verify peer certificate fingerprints match SDP `a=fingerprint` attributes.
- [ ] RTP injection: verify SSRC and sequence number validation rejects injected packets.
- [ ] Codec fuzzing: verify malformed Opus, PCMA, and PCMU frames do not crash the media mixer.
- [ ] Recording encryption: verify recordings are unreadable without the HA instance secret.

### TURN Server

- [ ] Unauthenticated allocation attempts are rejected.
- [ ] Expired HMAC credentials are rejected immediately.
- [ ] Allocation limit enforcement: verify the server refuses allocations beyond the per-user limit.
- [ ] Data relay between two allocations is impossible without a valid permission/channel binding.
- [ ] TURN-over-TLS: verify only TLS 1.2+ is accepted and weak ciphers are rejected.

### Certificate Manager

- [ ] ACME challenge validation: verify HTTP-01 and DNS-01 challenges complete correctly.
- [ ] Private key file permissions: verify keys are created with 0600 and owned by the engine process user.
- [ ] Certificate renewal: verify renewal triggers 30 days before expiry and that the SIP/TURN listeners reload without downtime.
- [ ] Self-signed fallback: verify self-signed certificates include the correct SAN entries.

### Integration (Home Assistant)

- [ ] Config flow input validation: verify all user inputs are sanitized (no path traversal, no injection).
- [ ] Service call authorization: verify `ha_voip.call`, `ha_voip.transfer`, and `ha_voip.announce` respect HA user permissions.
- [ ] Secrets handling: verify passwords and TURN secrets are stored in HA's encrypted credential store, never in `configuration.yaml` in plaintext unless the user explicitly writes them there.
- [ ] WebSocket API: verify the engine API WebSocket requires HA authentication tokens and rejects unauthenticated connections.

### Frontend Panel

- [ ] Content Security Policy: verify the panel sets strict CSP headers and does not use `unsafe-inline` or `unsafe-eval`.
- [ ] WebRTC permissions: verify the panel requests microphone access only when the user initiates a call.
- [ ] Cross-origin isolation: verify the panel does not load external scripts, fonts, or stylesheets from third-party CDNs.
- [ ] XSS testing: verify caller ID, extension names, and call history entries are sanitized before rendering.

## Recommended Third-Party Penetration Tests

We recommend the following test scenarios for organizations conducting independent security assessments of their ha-voip deployment.

### Network-Level Tests

1. **Port scan and service fingerprinting**: Identify all listening ports (SIP, RTP range, TURN, HTTPS) and verify only expected services are exposed. Use `nmap -sV -sU -p- <host>`.
2. **TLS configuration audit**: Verify TLS versions, cipher suites, and certificate chain validity using `testssl.sh` or Qualys SSL Labs (for publicly reachable instances).
3. **NAT traversal bypass**: Attempt to reach the RTP port range directly from the WAN, bypassing the TURN relay. Verify that without valid TURN credentials, direct RTP connections to the engine are not possible from external networks.

### Application-Level Tests

4. **SIP registration brute-force**: Attempt rapid REGISTER requests with incorrect credentials from multiple source IPs. Verify that the fail2ban mechanism activates and that the engine does not degrade under load.
5. **SIP INVITE without registration**: Send an INVITE to a valid extension without prior REGISTER. Verify the engine responds with `403 Forbidden`.
6. **SRTP downgrade attack**: Initiate a call with an SDP offer that omits SRTP and offers only plain RTP. Verify the engine rejects the call when `srtp_required` is true.
7. **TURN credential forgery**: Craft a TURN Allocate request with an expired or forged HMAC credential. Verify the server rejects it with `401 Unauthorized`.

### Data Exfiltration Tests

8. **Recording access without HA credentials**: Attempt to access the recording storage path via the file system, the HA API, and the engine API without proper authentication. Verify all paths require valid credentials.
9. **CDR data exposure**: Query the engine API for CDR data without HA authentication. Verify the API enforces authentication.
10. **Memory dump analysis**: If the tester has host-level access, dump the engine process memory and search for plaintext audio buffers, SIP passwords, or TURN secrets. Verify that sensitive data is zeroed after use.

### Denial of Service Tests

11. **SIP flood**: Send a sustained flood of OPTIONS, REGISTER, and INVITE messages. Measure the engine's response time degradation and verify it remains responsive to legitimate requests via rate limiting.
12. **RTP flood**: Send a high-rate stream of UDP packets to a port in the RTP range. Verify the engine drops packets from unauthenticated sources without excessive CPU consumption.
13. **TURN allocation exhaustion**: Attempt to allocate the maximum number of TURN relays. Verify the server enforces limits and does not exhaust file descriptors or memory.

## Contact

For security questions that are not vulnerability reports, reach out to the maintainers at:

```
maintainers@ha-voip.dev
```

For real-time coordination during an active incident, request access to the private `#security` channel on the ha-voip Discord server by emailing the address above.
