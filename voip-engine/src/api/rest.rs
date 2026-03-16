//! REST API handlers for ha-voip engine.
//!
//! Exposes the `ServiceState` via simple JSON endpoints so the Python
//! coordinator (and any HTTP client) can query calls, extensions, call
//! history, and relay WebRTC SDP/ICE without needing a gRPC client.
//!
//! All routes are mounted under /api/ by `build_rest_router`.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tracing::debug;
use uuid::Uuid;

use super::grpc::{proto, ServiceState};

// ── Shared state wrapper ──────────────────────────────────────────────────────

pub type RestState = Arc<ServiceState>;

// ── Helper: JSON error response ───────────────────────────────────────────────

fn err(status: StatusCode, msg: &str) -> impl IntoResponse {
    (status, Json(serde_json::json!({ "error": msg })))
}

// ── GET /api/calls ─────────────────────────────────────────────────────────────

async fn list_calls(State(state): State<RestState>) -> impl IntoResponse {
    let calls: Vec<serde_json::Value> = state
        .active_calls
        .iter()
        .map(|e| call_to_json(e.value()))
        .collect();
    Json(serde_json::json!({ "calls": calls }))
}

// ── GET /api/extensions ────────────────────────────────────────────────────────

async fn list_extensions(State(state): State<RestState>) -> impl IntoResponse {
    let exts: Vec<serde_json::Value> = state
        .extensions
        .iter()
        .map(|e| ext_to_json(e.value()))
        .collect();
    Json(serde_json::json!({ "extensions": exts }))
}

// ── GET /api/call-history ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

async fn call_history(
    State(state): State<RestState>,
    Query(q): Query<HistoryQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(50).min(500);
    let history = state.call_history.read().await;
    let calls: Vec<serde_json::Value> = history
        .iter()
        .rev()
        .take(limit)
        .map(call_to_json)
        .collect();
    Json(serde_json::json!({ "calls": calls }))
}

// ── POST /api/calls ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OriginateBody {
    from_extension: String,
    to_extension: String,
    #[serde(default)]
    caller_id: String,
    #[serde(default)]
    record: bool,
}

async fn originate_call(
    State(state): State<RestState>,
    Json(body): Json<OriginateBody>,
) -> impl IntoResponse {
    if body.from_extension.is_empty() || body.to_extension.is_empty() {
        return err(StatusCode::BAD_REQUEST, "from_extension and to_extension are required")
            .into_response();
    }

    let from_exists = state
        .extensions
        .iter()
        .any(|e| e.number == body.from_extension);
    if !from_exists {
        return err(
            StatusCode::NOT_FOUND,
            &format!("Extension {} not found", body.from_extension),
        )
        .into_response();
    }

    let call_id = Uuid::new_v4().to_string();
    let call = proto::CallInfo {
        call_id: call_id.clone(),
        from_uri: format!("sip:{}@local", body.from_extension),
        to_uri: format!("sip:{}@local", body.to_extension),
        state: proto::CallState::Trying as i32,
        started_at: None,
        answered_at: None,
        is_recording: body.record,
        is_muted: false,
        codec: "opus".into(),
        quality: Some(proto::CallQuality::default()),
    };
    state.active_calls.insert(call_id.clone(), call);
    debug!(call_id = %call_id, "REST: call originated");

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "call_id": call_id,
            "status": "initiated"
        })),
    )
        .into_response()
}

// ── POST /api/calls/:id/hangup ─────────────────────────────────────────────────

async fn hangup_call(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
) -> impl IntoResponse {
    match state.active_calls.remove(&call_id) {
        Some((_, mut call)) => {
            call.state = proto::CallState::Terminated as i32;
            state.call_history.write().await.push(call);
            debug!(call_id = %call_id, "REST: call hung up");
            Json(serde_json::json!({ "status": "ok" })).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "Call not found").into_response(),
    }
}

// ── POST /api/calls/:id/transfer ───────────────────────────────────────────────

#[derive(Deserialize)]
struct TransferBody {
    to_extension: String,
}

async fn transfer_call(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
    Json(body): Json<TransferBody>,
) -> impl IntoResponse {
    match state.active_calls.get_mut(&call_id) {
        Some(mut call) => {
            call.to_uri = format!("sip:{}@local", body.to_extension);
            debug!(call_id = %call_id, to = %body.to_extension, "REST: call transferred");
            Json(serde_json::json!({ "status": "ok" })).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "Call not found").into_response(),
    }
}

// ── POST /api/calls/:id/mute ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct MuteBody {
    #[serde(default)]
    mute: Option<bool>,
}

async fn toggle_mute(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
    Json(body): Json<MuteBody>,
) -> impl IntoResponse {
    match state.active_calls.get_mut(&call_id) {
        Some(mut call) => {
            call.is_muted = body.mute.unwrap_or(!call.is_muted);
            let muted = call.is_muted;
            debug!(call_id = %call_id, muted = muted, "REST: mute toggled");
            Json(serde_json::json!({ "status": "ok", "muted": muted })).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "Call not found").into_response(),
    }
}

// ── POST /api/calls/:id/recording ──────────────────────────────────────────────

async fn toggle_recording(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
) -> impl IntoResponse {
    match state.active_calls.get_mut(&call_id) {
        Some(mut call) => {
            call.is_recording = !call.is_recording;
            let recording = call.is_recording;
            debug!(call_id = %call_id, recording = recording, "REST: recording toggled");
            Json(serde_json::json!({ "status": "ok", "recording": recording })).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "Call not found").into_response(),
    }
}

// ── POST /api/calls/:id/dtmf ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct DtmfBody {
    digits: String,
}

async fn send_dtmf(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
    Json(body): Json<DtmfBody>,
) -> impl IntoResponse {
    if !state.active_calls.contains_key(&call_id) {
        return err(StatusCode::NOT_FOUND, "Call not found").into_response();
    }
    debug!(call_id = %call_id, digits = %body.digits, "REST: DTMF sent");
    Json(serde_json::json!({ "status": "ok" })).into_response()
}

// ── POST /api/webrtc/sdp ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct SdpBody {
    call_id: String,
    sdp: String,
    sdp_type: String,
}

async fn relay_sdp(
    State(state): State<RestState>,
    Json(body): Json<SdpBody>,
) -> impl IntoResponse {
    if body.call_id.is_empty() || body.sdp.is_empty() {
        return err(StatusCode::BAD_REQUEST, "call_id and sdp are required").into_response();
    }
    if !state.active_calls.contains_key(&body.call_id) {
        return err(StatusCode::NOT_FOUND, "Call not found").into_response();
    }

    if body.sdp_type == "offer" {
        if let Some((_, answer)) = state.sdp_answers.remove(&body.call_id) {
            return Json(serde_json::json!({
                "call_id": answer.call_id,
                "sdp": answer.sdp,
                "sdp_type": answer.sdp_type,
            }))
            .into_response();
        }
        state.sdp_answers.insert(
            body.call_id.clone(),
            proto::RelaySdpResponse {
                call_id: body.call_id.clone(),
                sdp: body.sdp,
                sdp_type: "offer".into(),
            },
        );
        Json(serde_json::json!({
            "call_id": body.call_id,
            "sdp": "",
            "sdp_type": "pending"
        }))
        .into_response()
    } else {
        let resp = proto::RelaySdpResponse {
            call_id: body.call_id.clone(),
            sdp: body.sdp.clone(),
            sdp_type: body.sdp_type.clone(),
        };
        state.sdp_answers.insert(body.call_id.clone(), resp);
        Json(serde_json::json!({
            "call_id": body.call_id,
            "sdp": body.sdp,
            "sdp_type": body.sdp_type,
        }))
        .into_response()
    }
}

// ── POST /api/webrtc/ice ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct IceBody {
    call_id: String,
    candidate: String,
    #[serde(default)]
    sdp_mid: String,
    #[serde(default)]
    sdp_mline_index: i32,
}

async fn relay_ice(
    State(state): State<RestState>,
    Json(body): Json<IceBody>,
) -> impl IntoResponse {
    if body.call_id.is_empty() || body.candidate.is_empty() {
        return err(StatusCode::BAD_REQUEST, "call_id and candidate are required").into_response();
    }
    if !state.active_calls.contains_key(&body.call_id) {
        return err(StatusCode::NOT_FOUND, "Call not found").into_response();
    }
    state
        .ice_candidates
        .entry(body.call_id.clone())
        .or_insert_with(Vec::new)
        .push(proto::RelayIceCandidateRequest {
            call_id: body.call_id,
            candidate: body.candidate,
            sdp_mid: body.sdp_mid,
            sdp_mline_index: body.sdp_mline_index,
        });
    Json(serde_json::json!({ "accepted": true })).into_response()
}

// ── POST /api/calls/:id/answer ─────────────────────────────────────────────────

async fn answer_call(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
) -> impl IntoResponse {
    match state.active_calls.get_mut(&call_id) {
        Some(mut call) => {
            call.state = proto::CallState::Confirmed as i32;
            debug!(call_id = %call_id, "REST: call answered");
            Json(serde_json::json!({ "status": "answered" })).into_response()
        }
        None => err(StatusCode::NOT_FOUND, "Call not found").into_response(),
    }
}

// ── POST /api/calls/:id/hold ───────────────────────────────────────────────────

#[derive(Deserialize)]
struct HoldBody {
    #[serde(default = "default_true")]
    hold: bool,
}

fn default_true() -> bool {
    true
}

async fn hold_call(
    State(state): State<RestState>,
    Path(call_id): Path<String>,
    body: Option<Json<HoldBody>>,
) -> impl IntoResponse {
    let hold = body.map(|b| b.hold).unwrap_or(true);
    match state.active_calls.get_mut(&call_id) {
        Some(mut call) => {
            // 6 = on_hold (extension of the proto enum), 4 = confirmed/active
            call.state = if hold { 6 } else { proto::CallState::Confirmed as i32 };
            debug!(call_id = %call_id, hold = hold, "REST: call hold toggled");
            Json(serde_json::json!({
                "status": if hold { "on_hold" } else { "active" }
            }))
            .into_response()
        }
        None => err(StatusCode::NOT_FOUND, "Call not found").into_response(),
    }
}

// ── Serialisation helpers ─────────────────────────────────────────────────────

fn call_to_json(c: &proto::CallInfo) -> serde_json::Value {
    serde_json::json!({
        "call_id":      c.call_id,
        "from_uri":     c.from_uri,
        "to_uri":       c.to_uri,
        "state":        call_state_name(c.state),
        "is_recording": c.is_recording,
        "is_muted":     c.is_muted,
        "codec":        c.codec,
    })
}

fn ext_to_json(e: &proto::Extension) -> serde_json::Value {
    serde_json::json!({
        "id":           e.id,
        "number":       e.number,
        "display_name": e.display_name,
        "registered":   e.registered,
        "transport":    e.transport,
    })
}

fn call_state_name(state: i32) -> &'static str {
    match state {
        1 => "trying",
        2 => "ringing",
        3 => "early",
        4 => "confirmed",
        5 => "terminated",
        6 => "on_hold",
        _ => "unknown",
    }
}

// ── Router builder ────────────────────────────────────────────────────────────

/// Build the /api/* router. Call this and merge it into the main axum Router.
pub fn build_rest_router(state: Arc<ServiceState>) -> Router {
    Router::new()
        .route("/api/calls", get(list_calls).post(originate_call))
        .route("/api/calls/:id/hangup", post(hangup_call))
        .route("/api/calls/:id/answer", post(answer_call))
        .route("/api/calls/:id/hold", post(hold_call))
        .route("/api/calls/:id/transfer", post(transfer_call))
        .route("/api/calls/:id/mute", post(toggle_mute))
        .route("/api/calls/:id/recording", post(toggle_recording))
        .route("/api/calls/:id/dtmf", post(send_dtmf))
        .route("/api/extensions", get(list_extensions))
        .route("/api/call-history", get(call_history))
        .route("/api/webrtc/sdp", post(relay_sdp))
        .route("/api/webrtc/ice", post(relay_ice))
        .with_state(state)
}
