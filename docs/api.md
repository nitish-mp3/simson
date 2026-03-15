# HA VoIP API Reference

Complete reference for every API surface exposed by HA VoIP: the gRPC engine API, the Home Assistant WebSocket API, the REST provisioning endpoints, HA services, and HA events.

---

## Table of Contents

1. [gRPC API (voip-engine)](#1-grpc-api)
2. [WebSocket API (HA frontend)](#2-websocket-api)
3. [REST Provisioning API](#3-rest-provisioning-api)
4. [Home Assistant Services](#4-home-assistant-services)
5. [Home Assistant Events](#5-home-assistant-events)
6. [Authentication and Authorization](#6-authentication-and-authorization)
7. [Rate Limits](#7-rate-limits)
8. [Error Codes](#8-error-codes)

---

## 1. gRPC API

The voip-engine exposes a gRPC service on port 50051 (configurable). The protobuf definition is in `voip-engine/proto/voip.proto`.

### Service: `VoipService`

#### Extension Management

| RPC | Request | Response | Description |
|---|---|---|---|
| `CreateExtension` | `CreateExtensionRequest` | `Extension` | Register a new SIP extension |
| `DeleteExtension` | `DeleteExtensionRequest` | `google.protobuf.Empty` | Remove an extension |
| `ListExtensions` | `ListExtensionsRequest` | `ListExtensionsResponse` | Paginated extension list |
| `GetExtension` | `GetExtensionRequest` | `Extension` | Get a single extension by ID |

**`CreateExtensionRequest`**
```protobuf
message CreateExtensionRequest {
  string number          = 1;  // e.g. "100"
  string display_name    = 2;  // e.g. "Alice"
  string password        = 3;  // SIP auth password
  string transport       = 4;  // "udp" | "tcp" | "tls" | "wss"
  bool   voicemail_enabled = 5;
  int32  max_concurrent_calls = 6;
}
```

**`Extension` (response)**
```protobuf
message Extension {
  string id           = 1;
  string number       = 2;
  string display_name = 3;
  string transport    = 4;
  bool   voicemail_enabled = 5;
  bool   registered   = 6;
  google.protobuf.Timestamp created_at = 7;
  google.protobuf.Timestamp updated_at = 8;
  int32  max_concurrent_calls = 9;
}
```

#### Call Control

| RPC | Request | Response | Description |
|---|---|---|---|
| `OriginateCall` | `OriginateCallRequest` | `CallInfo` | Start a new call between two extensions |
| `HangupCall` | `HangupCallRequest` | `Empty` | Terminate an active call |
| `TransferCall` | `TransferCallRequest` | `CallInfo` | Blind or attended transfer |
| `MuteUnmute` | `MuteUnmuteRequest` | `Empty` | Mute or unmute a call leg |

**`OriginateCallRequest`**
```protobuf
message OriginateCallRequest {
  string from_extension = 1;
  string to_extension   = 2;
  bool   auto_answer    = 3;
  bool   record         = 4;
}
```

**`CallInfo`**
```protobuf
message CallInfo {
  string    call_id       = 1;
  string    from_uri      = 2;
  string    to_uri        = 3;
  CallState state         = 4;
  Timestamp started_at    = 5;
  Timestamp answered_at   = 6;
  bool      is_recording  = 7;
  bool      is_muted      = 8;
  string    codec         = 9;
  CallQuality quality     = 10;
}
```

**`CallState` enum**
```
CALL_STATE_UNKNOWN    = 0
CALL_STATE_TRYING     = 1
CALL_STATE_RINGING    = 2
CALL_STATE_EARLY      = 3
CALL_STATE_CONFIRMED  = 4
CALL_STATE_TERMINATED = 5
```

#### Call Queries

| RPC | Request | Response | Description |
|---|---|---|---|
| `GetCallHistory` | `GetCallHistoryRequest` | `GetCallHistoryResponse` | Paginated call history |
| `GetActiveCalls` | `Empty` | `GetActiveCallsResponse` | List all active calls |

#### Routing

| RPC | Request | Response | Description |
|---|---|---|---|
| `SetRoutingRule` | `SetRoutingRuleRequest` | `RoutingRule` | Create or update a routing rule |
| `GetRoutingRules` | `Empty` | `GetRoutingRulesResponse` | List all routing rules |

**`SetRoutingRuleRequest`**
```protobuf
message SetRoutingRuleRequest {
  string pattern     = 1;  // Regex or prefix
  string destination = 2;  // Target extension or trunk URI
  int32  priority    = 3;  // Lower = higher priority
  string description = 4;
}
```

#### Observability

| RPC | Request | Response | Description |
|---|---|---|---|
| `GetMetrics` | `Empty` | `MetricsResponse` | Engine performance metrics |
| `GetHealth` | `Empty` | `HealthResponse` | Engine health status |

**`MetricsResponse`**
```protobuf
message MetricsResponse {
  int32  active_calls          = 1;
  int32  active_registrations  = 2;
  int32  active_turn_allocs    = 3;
  int64  total_calls           = 4;
  int64  call_drops            = 5;
  double avg_call_duration_sec = 6;
  double avg_jitter_ms         = 7;
  double avg_packet_loss_pct   = 8;
  double cpu_usage_pct         = 9;
  int64  memory_usage_bytes    = 10;
}
```

#### Voicemail

| RPC | Request | Response | Description |
|---|---|---|---|
| `CreateVoicemail` | `CreateVoicemailRequest` | `Voicemail` | Store a voicemail message |
| `GetVoicemails` | `GetVoicemailsRequest` | `GetVoicemailsResponse` | List voicemails for an extension |
| `DeleteVoicemail` | `DeleteVoicemailRequest` | `Empty` | Delete a voicemail |

#### Real-Time Events

| RPC | Request | Response | Description |
|---|---|---|---|
| `StreamEvents` | `StreamEventsRequest` | `stream VoipEvent` | Server-side streaming of real-time events |

Filter by event type or extension:
```protobuf
message StreamEventsRequest {
  repeated string event_types = 1;
  string extension_filter = 2;
}
```

---

## 2. WebSocket API

The HA VoIP integration registers WebSocket commands under the `voip/` prefix on the Home Assistant WebSocket API (`ws://ha-host:8123/api/websocket`).

### Commands (Client -> Server)

#### `voip/subscribe`
Subscribe to real-time VoIP events (call state changes, extension updates, incoming calls).

```json
{
  "id": 1,
  "type": "voip/subscribe"
}
```

Response: `{"id": 1, "type": "result", "success": true}`

After subscribing, the server pushes events:
```json
{
  "id": 1,
  "type": "event",
  "event": {
    "event": "call_state",
    "data": {
      "id": "call-001",
      "state": "ringing",
      "direction": "inbound",
      "remoteNumber": "102",
      "remoteName": "Carol"
    }
  }
}
```

#### `voip/call`
Initiate an outbound call.

```json
{
  "id": 2,
  "type": "voip/call",
  "number": "101"
}
```

Response:
```json
{
  "id": 2,
  "type": "result",
  "success": true,
  "result": {"call_id": "call-001"}
}
```

#### `voip/answer`
Answer an incoming call.

```json
{"id": 3, "type": "voip/answer", "call_id": "call-001"}
```

#### `voip/hangup`
Terminate a call.

```json
{"id": 4, "type": "voip/hangup", "call_id": "call-001"}
```

#### `voip/hold`
Place a call on hold or resume.

```json
{"id": 5, "type": "voip/hold", "call_id": "call-001", "hold": true}
```

#### `voip/transfer`
Transfer a call to another extension.

```json
{"id": 6, "type": "voip/transfer", "call_id": "call-001", "target": "103"}
```

#### `voip/mute`
Mute or unmute the microphone.

```json
{"id": 7, "type": "voip/mute", "call_id": "call-001", "mute": true}
```

#### `voip/dtmf`
Send a DTMF digit.

```json
{"id": 8, "type": "voip/dtmf", "call_id": "call-001", "digit": "5"}
```

#### `voip/webrtc_offer`
Send a WebRTC SDP offer to establish media.

```json
{
  "id": 9,
  "type": "voip/webrtc_offer",
  "call_id": "call-001",
  "sdp": "v=0\r\no=- ..."
}
```

Response includes the SDP answer:
```json
{
  "id": 9,
  "type": "result",
  "success": true,
  "result": {"sdp": "v=0\r\no=- ...", "type": "answer"}
}
```

#### `voip/webrtc_candidate`
Send an ICE candidate.

```json
{
  "id": 10,
  "type": "voip/webrtc_candidate",
  "call_id": "call-001",
  "candidate": {
    "candidate": "candidate:1 1 udp 2122260223 ...",
    "sdpMLineIndex": 0,
    "sdpMid": "audio"
  }
}
```

#### `voip/extensions`
Retrieve the list of configured extensions and their status.

```json
{"id": 11, "type": "voip/extensions"}
```

#### `voip/history`
Retrieve call history.

```json
{"id": 12, "type": "voip/history"}
```

#### `voip/diagnostics`
Run a diagnostic check and return results.

```json
{"id": 13, "type": "voip/diagnostics"}
```

### Events (Server -> Client)

Events are pushed to subscribers after `voip/subscribe`:

| Event Type | Payload | Description |
|---|---|---|
| `call_state` | `CallState` | Call state changed |
| `incoming_call` | `{call_id, caller_number, caller_name}` | New incoming call |
| `extensions` | `Extension[]` | Extension list updated |
| `history` | `CallHistoryEntry[]` | Call history updated |
| `webrtc_offer` | `{call_id, sdp}` | Remote SDP offer received |
| `webrtc_answer` | `{call_id, sdp}` | Remote SDP answer received |
| `webrtc_candidate` | `{call_id, candidate}` | Remote ICE candidate received |

---

## 3. REST Provisioning API

The engine exposes an HTTP API on port 8080 (configurable) for health checks, metrics, and provisioning.

### Endpoints

#### `GET /health`
Returns engine health status.

```json
{
  "healthy": true,
  "version": "0.1.0",
  "uptime_sec": 86400,
  "components": {
    "sip": {"healthy": true, "message": ""},
    "turn": {"healthy": true, "message": ""},
    "database": {"healthy": true, "message": ""}
  }
}
```

#### `GET /metrics`
Returns Prometheus-format metrics.

```
# HELP voip_active_calls Number of active calls
# TYPE voip_active_calls gauge
voip_active_calls 3
# HELP voip_active_registrations Number of registered extensions
# TYPE voip_active_registrations gauge
voip_active_registrations 5
# HELP voip_call_setup_duration_seconds Call setup time histogram
# TYPE voip_call_setup_duration_seconds histogram
voip_call_setup_duration_seconds_bucket{le="0.1"} 42
voip_call_setup_duration_seconds_bucket{le="0.5"} 98
voip_call_setup_duration_seconds_bucket{le="1.0"} 100
```

#### `GET /api/v1/extensions`
List all extensions (JSON).

#### `POST /api/v1/extensions`
Create an extension.

```json
{
  "number": "100",
  "display_name": "Alice",
  "password": "secret",
  "transport": "wss"
}
```

#### `DELETE /api/v1/extensions/{id}`
Delete an extension.

#### `GET /api/v1/calls`
List active calls.

#### `POST /api/v1/calls`
Originate a call.

#### `DELETE /api/v1/calls/{call_id}`
Hang up a call.

---

## 4. Home Assistant Services

Services are registered under the `ha_voip` domain.

### `ha_voip.make_call`
Start a call between two extensions.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `from_extension` | string | Yes | Calling extension number |
| `to_extension` | string | Yes | Called extension number |
| `auto_answer` | boolean | No | Auto-answer on the receiving end |
| `record` | boolean | No | Record the call |

### `ha_voip.hangup`
End an active call.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `call_id` | string | Yes | The call ID to terminate |

### `ha_voip.transfer`
Transfer a call to another extension.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `call_id` | string | Yes | The call to transfer |
| `target_extension` | string | Yes | Destination extension |
| `blind` | boolean | No | If true, blind transfer (default). If false, attended. |

### `ha_voip.record_toggle`
Toggle recording on an active call.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `call_id` | string | Yes | The call ID |

### `ha_voip.mute_toggle`
Toggle mute on an active call.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `call_id` | string | Yes | The call ID |

### `ha_voip.send_dtmf`
Send DTMF digits on an active call.

| Parameter | Type | Required | Description |
|---|---|---|---|
| `call_id` | string | Yes | The call ID |
| `digits` | string | Yes | DTMF digits (e.g. "1234#") |

---

## 5. Home Assistant Events

Events are fired on the HA event bus under the `ha_voip_` prefix.

| Event | Payload | Description |
|---|---|---|
| `ha_voip_call_started` | `{call_id, from_uri, to_uri}` | A new call has been initiated |
| `ha_voip_call_ringing` | `{call_id, from_uri, to_uri}` | The remote side is ringing |
| `ha_voip_call_answered` | `{call_id, from_uri, to_uri, codec}` | The call has been answered |
| `ha_voip_call_ended` | `{call_id, duration_sec, hangup_cause}` | The call has ended |
| `ha_voip_call_held` | `{call_id}` | The call was placed on hold |
| `ha_voip_call_resumed` | `{call_id}` | The call was taken off hold |
| `ha_voip_call_transferred` | `{call_id, target_extension}` | The call was transferred |
| `ha_voip_registration_changed` | `{extension_id, registered, contact_uri}` | Extension registration state changed |
| `ha_voip_engine_state_changed` | `{state, message}` | Engine state changed (running, error, etc.) |
| `ha_voip_dtmf_received` | `{call_id, digit}` | A DTMF digit was received |

---

## 6. Authentication and Authorization

### gRPC API
- **API key authentication:** Pass a bearer token in the `authorization` metadata header:
  ```
  authorization: Bearer <api_key>
  ```
  API keys are configured in `api.api_keys` in the engine config.
- **mTLS:** When `api.enable_mtls` is true, clients must present a valid client certificate.

### WebSocket API
- Authenticated via the standard Home Assistant WebSocket authentication flow (long-lived access token or session cookie).

### REST API
- Same API key as gRPC, passed via the `Authorization: Bearer <key>` HTTP header.

### TURN
- **Static credentials:** Username/password pairs configured in `turn.users`.
- **Ephemeral credentials:** Time-limited credentials derived from a shared secret using HMAC-SHA1 (RFC 8489 long-term credentials with a time-limited username of the form `timestamp:userid`).

---

## 7. Rate Limits

| Endpoint | Default Limit | Config Key |
|---|---|---|
| gRPC API | 100 req/sec per API key | `api.rate_limit_per_sec` |
| REST API | 100 req/sec per API key | `api.rate_limit_per_sec` |
| TURN | 50 req/sec per IP | `turn.rate_limit_per_sec` |
| WebSocket | No explicit limit (HA-managed) | -- |

When a rate limit is exceeded:
- gRPC returns status code `RESOURCE_EXHAUSTED` (8).
- REST returns HTTP `429 Too Many Requests`.
- TURN returns STUN error code `429`.

---

## 8. Error Codes

### gRPC Status Codes

| Code | Name | HA VoIP Usage |
|---|---|---|
| 0 | OK | Success |
| 3 | INVALID_ARGUMENT | Invalid extension number, missing required field |
| 5 | NOT_FOUND | Extension or call not found |
| 6 | ALREADY_EXISTS | Extension number already registered |
| 7 | PERMISSION_DENIED | Invalid API key or insufficient privileges |
| 8 | RESOURCE_EXHAUSTED | Rate limit exceeded, max concurrent calls reached |
| 13 | INTERNAL | Unexpected engine error |
| 14 | UNAVAILABLE | Engine shutting down or overloaded |
| 16 | UNAUTHENTICATED | Missing or invalid authentication |

### SIP Response Codes (used in `HangupCallRequest.cause`)

| Code | Meaning |
|---|---|
| 200 | OK (normal answer) |
| 400 | Bad Request |
| 401 | Unauthorized |
| 403 | Forbidden |
| 404 | Not Found (extension does not exist) |
| 408 | Request Timeout |
| 480 | Temporarily Unavailable |
| 486 | Busy Here |
| 487 | Request Terminated (CANCEL) |
| 488 | Not Acceptable Here (codec mismatch) |
| 500 | Server Internal Error |
| 503 | Service Unavailable |
| 603 | Decline |
