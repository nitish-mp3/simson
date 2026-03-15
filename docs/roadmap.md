# ha-voip Product Roadmap & Sprint Backlog

## Product Vision

ha-voip delivers production-grade VoIP to Home Assistant as a single-install integration.
Users get browser-to-browser calling, PSTN connectivity, and full PBX features without
manually configuring Asterisk, coturn, or external servers.

---

## Release Milestones

### v1.0.0 — MVP (Sprints 1-4)
- Local extension-to-extension calling via WebRTC
- Embedded TURN server with port 443 fallback
- Self-signed certificate manager for LAN use
- Lovelace card with dialpad, call controls, and call history
- Onboarding wizard (2-screen quick mode)
- SQLite database for metadata
- Docker Compose deployment
- Basic monitoring (Prometheus metrics, health endpoints)

### v1.1.0 — Production Hardening (Sprints 5-6)
- ACME/Let's Encrypt automatic certificates
- SIP trunk support (connect to PSTN providers)
- Voicemail with notification
- Encrypted call recordings (AES-256-GCM)
- Full Grafana dashboard
- HA Add-on packaging

### v1.2.0 — Enterprise Features (Sprints 7-9)
- Multi-site federation (mutual TLS)
- PostgreSQL support for large deployments
- Ring groups and call queues
- IVR (Interactive Voice Response) builder
- REST API for third-party integrations
- Kubernetes Helm chart
- Load testing benchmarks published

### v2.0.0 — Platform (Sprints 10-12)
- Video calling support
- SIP-to-WebRTC gateway for legacy phones
- Mobile companion app (PWA)
- Plugin architecture for custom PBX logic
- Hosted marketplace images
- SOC 2 compliance documentation

---

## Sprint Backlog (Jira-style)

### Sprint 1 — Foundation (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-001 | Set up repository scaffold and CI pipeline | S | P0 | Done |
| HV-002 | Implement SIP message parser (RFC 3261) | L | P0 | Done |
| HV-003 | Implement SIP dialog/transaction state machine | L | P0 | Done |
| HV-004 | Implement UDP/TCP SIP transport | M | P0 | Done |
| HV-005 | Implement RTP packet handling | M | P0 | Done |
| HV-006 | Implement jitter buffer | M | P1 | Done |
| HV-007 | Set up SQLite database layer with migrations | M | P0 | Done |
| HV-008 | Implement Prometheus metrics endpoint | S | P1 | Done |
| HV-009 | Implement health check endpoints | S | P1 | Done |
| HV-010 | Write unit tests for SIP parser (25+ cases) | M | P0 | Done |

### Sprint 2 — WebRTC & TURN (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-011 | Implement WebSocket SIP transport (RFC 7118) | M | P0 | Done |
| HV-012 | Implement SRTP/DTLS-SRTP key exchange | L | P0 | Done |
| HV-013 | Implement embedded TURN server (RFC 5766) | L | P0 | Done |
| HV-014 | Implement TURN port 443 TCP/TLS fallback | M | P0 | Done |
| HV-015 | Implement TURN credential generation (HMAC) | S | P0 | Done |
| HV-016 | Implement TURN rate limiting per source IP | S | P1 | Done |
| HV-017 | Implement gRPC control API | L | P0 | Done |
| HV-018 | Write TURN allocation tests | M | P0 | Done |
| HV-019 | Write TURN fallback integration test | M | P0 | Done |

### Sprint 3 — HA Integration (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-020 | Implement HA config flow (4 steps) | M | P0 | Done |
| HV-021 | Implement engine process manager | M | P0 | Done |
| HV-022 | Implement data update coordinator | M | P0 | Done |
| HV-023 | Implement HA services (make_call, hangup, transfer) | M | P0 | Done |
| HV-024 | Implement WebSocket API for frontend | M | P0 | Done |
| HV-025 | Implement extension entity | S | P0 | Done |
| HV-026 | Implement call state sensor entity | M | P0 | Done |
| HV-027 | Implement presence binary sensor | S | P1 | Done |
| HV-028 | Implement diagnostics support bundle | S | P1 | Done |
| HV-029 | Write HA integration unit tests (40+ cases) | M | P0 | Done |

### Sprint 4 — Frontend & UX (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-030 | Implement Lovelace VoIP card (Lit) | L | P0 | Done |
| HV-031 | Implement dialpad component with DTMF tones | M | P0 | Done |
| HV-032 | Implement call popup (incoming/active) | M | P0 | Done |
| HV-033 | Implement onboarding wizard (6 steps + quick mode) | L | P0 | Done |
| HV-034 | Implement WebRTC manager class | L | P0 | Done |
| HV-035 | Implement network diagnostics panel | M | P1 | Done |
| HV-036 | Implement audio device selection | S | P1 | Done |
| HV-037 | Responsive mobile layout | M | P1 | Done |
| HV-038 | Accessibility (ARIA labels, keyboard nav) | M | P1 | Done |
| HV-039 | Dark/light theme support | S | P2 | Done |

### Sprint 5 — Certificates & Security (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-040 | Implement ACME client (Let's Encrypt) | L | P0 | Done |
| HV-041 | Implement local CA for self-signed certs | M | P0 | Done |
| HV-042 | Implement certificate store with encryption | M | P0 | Done |
| HV-043 | Implement auto-renewal with zero-downtime reload | M | P0 | Done |
| HV-044 | Implement manual cert upload flow | S | P1 | Done |
| HV-045 | Implement call recording with AES-GCM encryption | M | P1 | Done |
| HV-046 | SIP parser fuzz testing harness | M | P0 | Done |
| HV-047 | Security scan integration in CI (SAST, deps) | M | P0 | Done |
| HV-048 | Write certificate integration tests | M | P0 | Done |

### Sprint 6 — Deployment & Ops (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-049 | Docker multi-stage builds (engine + integration) | M | P0 | Done |
| HV-050 | Docker Compose full stack deployment | M | P0 | Done |
| HV-051 | Kubernetes deployment manifests | M | P1 | Done |
| HV-052 | Grafana dashboard templates | M | P1 | Done |
| HV-053 | Prometheus alert rules | S | P1 | Done |
| HV-054 | Backup/restore manager | M | P1 | Done |
| HV-055 | Network connectivity tester (NAT detection) | M | P1 | Done |
| HV-056 | Provisioning REST API | M | P1 | Done |
| HV-057 | Load test harness (50/100/250 concurrent calls) | M | P1 | Done |
| HV-058 | Documentation (admin guide, security, API ref) | L | P0 | Done |

### Sprint 7 — SIP Trunking (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-059 | SIP trunk registration (outbound gateway) | L | P0 | Backlog |
| HV-060 | Inbound DID routing rules | M | P0 | Backlog |
| HV-061 | Codec negotiation (Opus <-> G.711 transcoding) | L | P0 | Backlog |
| HV-062 | DTMF RFC 2833/SIP INFO support | M | P1 | Backlog |
| HV-063 | Asterisk/FreeSWITCH interop testing | M | P1 | Backlog |
| HV-064 | E911/emergency calling compliance notes | S | P1 | Backlog |

### Sprint 8 — Federation (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-065 | Federation protocol (mTLS between sites) | L | P1 | Backlog |
| HV-066 | Central extension registry | M | P1 | Backlog |
| HV-067 | Cross-site call routing | L | P1 | Backlog |
| HV-068 | Federation admin UI | M | P2 | Backlog |
| HV-069 | PostgreSQL support + connection pooling | M | P1 | Backlog |
| HV-070 | Multi-node media clustering | L | P2 | Backlog |

### Sprint 9 — Advanced PBX (2 weeks)

| ID | Title | Size | Priority | Status |
|----|-------|------|----------|--------|
| HV-071 | Ring groups (simultaneous, sequential, random) | M | P1 | Backlog |
| HV-072 | Call queues with hold music | L | P2 | Backlog |
| HV-073 | IVR builder (DTMF menu trees) | L | P2 | Backlog |
| HV-074 | Voicemail-to-email with transcription | M | P1 | Backlog |
| HV-075 | Call forwarding (unconditional, busy, no-answer) | M | P1 | Backlog |
| HV-076 | Do Not Disturb scheduling | S | P2 | Backlog |
| HV-077 | BLF (Busy Lamp Field) support | S | P2 | Backlog |

---

## Size Legend

| Size | Estimate | Description |
|------|----------|-------------|
| S    | 1-2 days | Well-defined, single-file change |
| M    | 3-5 days | Multi-file, requires testing |
| L    | 5-10 days | Complex feature, multiple components |

## Priority Legend

| Priority | Meaning |
|----------|---------|
| P0 | Must have — blocks release |
| P1 | Should have — important for adoption |
| P2 | Nice to have — improves competitiveness |

---

## Security Audit Checklist

Before public release, complete the following audits:

### SIP Stack
- [ ] Fuzz test SIP parser with 1M+ inputs (no crashes)
- [ ] Fuzz test SDP parser with 1M+ inputs (no crashes)
- [ ] Verify RFC 3261 compliance for all SIP methods
- [ ] Test SIP authentication (digest challenge, credential rotation)
- [ ] Test malformed header injection resistance
- [ ] Test oversized message handling (DoS protection)
- [ ] Verify transaction timer compliance

### Media Path
- [ ] Verify SRTP encryption is mandatory (no RTP allowed)
- [ ] Verify DTLS handshake validates certificates
- [ ] Test key derivation correctness (RFC 3711)
- [ ] Test replay attack protection (sliding window)
- [ ] Verify recording encryption (AES-256-GCM)
- [ ] Test recording key rotation

### TURN Server
- [ ] Test credential validation (long-term auth)
- [ ] Test ephemeral credential expiry
- [ ] Test allocation rate limiting
- [ ] Test permission enforcement
- [ ] Test allocation quota per IP
- [ ] Verify relay data isolation between clients
- [ ] Test TURN over TLS certificate validation

### Certificate Manager
- [ ] Test ACME challenge verification
- [ ] Test certificate renewal automation
- [ ] Verify private key file permissions (0600)
- [ ] Test certificate chain validation
- [ ] Test self-signed CA security properties
- [ ] Verify PKCS#12 export security

### HA Integration
- [ ] Test config flow input validation (injection attacks)
- [ ] Test WebSocket API authentication enforcement
- [ ] Test API token scope restrictions
- [ ] Verify no secrets in diagnostics export
- [ ] Test gRPC channel encryption
- [ ] Verify engine binary path validation

### Frontend
- [ ] Test XSS resistance in all user inputs
- [ ] Verify WebRTC API security (getUserMedia permissions)
- [ ] Test CSP compatibility
- [ ] Verify no credentials in client-side storage
- [ ] Test TURN credential rotation in browser

### Recommended Third-Party Penetration Tests
1. **Network penetration test** — SIP port scanning, TURN abuse, RTP injection
2. **Web application penetration test** — WebSocket API, REST provisioning API
3. **Fuzzing campaign** — Extended SIP/SDP fuzzing with AFL++ or libFuzzer
4. **TLS audit** — Certificate chain validation, cipher suite hardening
5. **Authentication bypass** — SIP digest auth, TURN credentials, HA tokens
6. **DoS resilience** — SIP flood, INVITE flood, TURN allocation exhaustion
7. **Data exfiltration** — Recording access controls, database injection
