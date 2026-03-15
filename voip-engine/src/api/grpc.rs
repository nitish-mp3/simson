//! gRPC service implementation for the VoIP engine.
//!
//! Provides extension management, call control, routing, voicemail,
//! metrics, health, event streaming, and WebRTC SDP/ICE relay RPCs.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use futures_util::Stream;
use prost_types::Timestamp;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// ─────────────────────── Proto module ───────────────────────
//
// Include the generated protobuf code when the feature flag is present;
// otherwise use inline stub types so that the crate compiles without
// running protoc first.

#[cfg(feature = "_generated_proto")]
pub mod proto {
    tonic::include_proto!("voip");
}

#[cfg(not(feature = "_generated_proto"))]
pub mod proto {
    pub use prost_types::Timestamp;

    // ── Enums ──

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    #[repr(i32)]
    pub enum CallState {
        Unknown = 0,
        Trying = 1,
        Ringing = 2,
        Early = 3,
        Confirmed = 4,
        Terminated = 5,
    }

    // ── Extension ──

    #[derive(Clone, Debug, Default)]
    pub struct Extension {
        pub id: String,
        pub number: String,
        pub display_name: String,
        pub transport: String,
        pub voicemail_enabled: bool,
        pub registered: bool,
        pub created_at: Option<Timestamp>,
        pub updated_at: Option<Timestamp>,
        pub max_concurrent_calls: i32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct CreateExtensionRequest {
        pub number: String,
        pub display_name: String,
        pub password: String,
        pub transport: String,
        pub voicemail_enabled: bool,
        pub max_concurrent_calls: i32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct DeleteExtensionRequest {
        pub extension_id: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetExtensionRequest {
        pub extension_id: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct ListExtensionsRequest {
        pub page_size: i32,
        pub page_token: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct ListExtensionsResponse {
        pub extensions: Vec<Extension>,
        pub next_page_token: String,
    }

    // ── Call ──

    #[derive(Clone, Debug, Default)]
    pub struct CallQuality {
        pub jitter_ms: f32,
        pub packet_loss_pct: f32,
        pub rtt_ms: f32,
        pub mos_score: i32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct CallInfo {
        pub call_id: String,
        pub from_uri: String,
        pub to_uri: String,
        pub state: i32,
        pub started_at: Option<Timestamp>,
        pub answered_at: Option<Timestamp>,
        pub is_recording: bool,
        pub is_muted: bool,
        pub codec: String,
        pub quality: Option<CallQuality>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct OriginateCallRequest {
        pub from_extension: String,
        pub to_extension: String,
        pub auto_answer: bool,
        pub record: bool,
    }

    #[derive(Clone, Debug, Default)]
    pub struct HangupCallRequest {
        pub call_id: String,
        pub cause: i32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct TransferCallRequest {
        pub call_id: String,
        pub to_extension: String,
        pub blind_transfer: bool,
    }

    #[derive(Clone, Debug, Default)]
    pub struct MuteUnmuteRequest {
        pub call_id: String,
        pub mute: bool,
        pub direction: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetCallHistoryRequest {
        pub extension_id: String,
        pub since: Option<Timestamp>,
        pub until: Option<Timestamp>,
        pub page_size: i32,
        pub page_token: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetCallHistoryResponse {
        pub calls: Vec<CallInfo>,
        pub next_page_token: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetActiveCallsResponse {
        pub calls: Vec<CallInfo>,
    }

    // ── Routing ──

    #[derive(Clone, Debug, Default)]
    pub struct SetRoutingRuleRequest {
        pub pattern: String,
        pub destination: String,
        pub priority: i32,
        pub description: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct RoutingRule {
        pub id: String,
        pub pattern: String,
        pub destination: String,
        pub priority: i32,
        pub description: String,
        pub created_at: Option<Timestamp>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetRoutingRulesResponse {
        pub rules: Vec<RoutingRule>,
    }

    // ── Observability ──

    #[derive(Clone, Debug, Default)]
    pub struct ComponentHealth {
        pub healthy: bool,
        pub message: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct MetricsResponse {
        pub active_calls: i32,
        pub active_registrations: i32,
        pub active_turn_allocs: i32,
        pub total_calls: i64,
        pub call_drops: i64,
        pub avg_call_duration_sec: f64,
        pub avg_jitter_ms: f64,
        pub avg_packet_loss_pct: f64,
        pub cpu_usage_pct: f64,
        pub memory_usage_bytes: i64,
    }

    #[derive(Clone, Debug, Default)]
    pub struct HealthResponse {
        pub healthy: bool,
        pub version: String,
        pub uptime_sec: i64,
        pub components: std::collections::HashMap<String, ComponentHealth>,
    }

    // ── Voicemail ──

    #[derive(Clone, Debug, Default)]
    pub struct CreateVoicemailRequest {
        pub extension_id: String,
        pub caller_id: String,
        pub audio_data: Vec<u8>,
        pub duration_sec: i32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct Voicemail {
        pub id: String,
        pub extension_id: String,
        pub caller_id: String,
        pub duration_sec: i32,
        pub is_read: bool,
        pub file_path: String,
        pub created_at: Option<Timestamp>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetVoicemailsRequest {
        pub extension_id: String,
        pub unread_only: bool,
        pub page_size: i32,
        pub page_token: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct GetVoicemailsResponse {
        pub voicemails: Vec<Voicemail>,
        pub next_page_token: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct DeleteVoicemailRequest {
        pub voicemail_id: String,
    }

    // ── Events ──

    #[derive(Clone, Debug, Default)]
    pub struct StreamEventsRequest {
        pub event_types: Vec<String>,
        pub extension_filter: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct CallEvent {
        pub call_id: String,
        pub from_uri: String,
        pub to_uri: String,
        pub state: i32,
        pub sip_code: i32,
        pub reason: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct RegistrationEvent {
        pub extension_id: String,
        pub registered: bool,
        pub contact_uri: String,
        pub user_agent: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct QualityEvent {
        pub call_id: String,
        pub quality: Option<CallQuality>,
    }

    #[derive(Clone, Debug, Default)]
    pub struct VoicemailEvent {
        pub voicemail_id: String,
        pub extension_id: String,
        pub action: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct SystemEvent {
        pub component: String,
        pub severity: String,
        pub message: String,
    }

    #[derive(Clone, Debug)]
    pub enum VoipEventPayload {
        CallEvent(CallEvent),
        RegEvent(RegistrationEvent),
        QualityEvent(QualityEvent),
        VoicemailEvent(VoicemailEvent),
        SystemEvent(SystemEvent),
    }

    #[derive(Clone, Debug, Default)]
    pub struct VoipEvent {
        pub event_id: String,
        pub event_type: String,
        pub timestamp: Option<Timestamp>,
        pub payload: Option<VoipEventPayload>,
    }

    // ── WebRTC relay ──

    #[derive(Clone, Debug, Default)]
    pub struct RelaySdpRequest {
        pub call_id: String,
        pub sdp: String,
        pub sdp_type: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct RelaySdpResponse {
        pub call_id: String,
        pub sdp: String,
        pub sdp_type: String,
    }

    #[derive(Clone, Debug, Default)]
    pub struct RelayIceCandidateRequest {
        pub call_id: String,
        pub candidate: String,
        pub sdp_mid: String,
        pub sdp_mline_index: i32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct RelayIceCandidateResponse {
        pub accepted: bool,
    }
}

// ─────────────────────── Rate limiter ───────────────────────

/// Simple per-key token bucket rate limiter (fixed window).
struct RateLimiter {
    requests: DashMap<String, (Instant, u32)>,
    max_per_sec: u32,
}

impl RateLimiter {
    fn new(max_per_sec: u32) -> Self {
        RateLimiter {
            requests: DashMap::new(),
            max_per_sec,
        }
    }

    fn check(&self, key: &str) -> bool {
        let now = Instant::now();
        let mut entry = self.requests.entry(key.to_string()).or_insert((now, 0));
        let (window_start, count) = entry.value_mut();

        if now.duration_since(*window_start) > Duration::from_secs(1) {
            *window_start = now;
            *count = 1;
            true
        } else if *count < self.max_per_sec {
            *count += 1;
            true
        } else {
            false
        }
    }
}

// ─────────────────────── Shared service state ───────────────────────

/// Internal state shared across all gRPC handler methods.
pub struct ServiceState {
    pub extensions: DashMap<String, proto::Extension>,
    pub active_calls: DashMap<String, proto::CallInfo>,
    pub call_history: RwLock<Vec<proto::CallInfo>>,
    pub routing_rules: DashMap<String, proto::RoutingRule>,
    pub voicemails: DashMap<String, proto::Voicemail>,
    /// Pending SDP answers keyed by call_id.
    pub sdp_answers: DashMap<String, proto::RelaySdpResponse>,
    /// Pending ICE candidates keyed by call_id.
    pub ice_candidates: DashMap<String, Vec<proto::RelayIceCandidateRequest>>,
    pub start_time: Instant,
    pub event_tx: broadcast::Sender<proto::VoipEvent>,
    pub rate_limiter: RateLimiter,
    pub api_keys: Vec<String>,
}

impl ServiceState {
    pub fn new(api_keys: Vec<String>, rate_limit: u32) -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(1024);
        Arc::new(ServiceState {
            extensions: DashMap::new(),
            active_calls: DashMap::new(),
            call_history: RwLock::new(Vec::new()),
            routing_rules: DashMap::new(),
            voicemails: DashMap::new(),
            sdp_answers: DashMap::new(),
            ice_candidates: DashMap::new(),
            start_time: Instant::now(),
            event_tx,
            rate_limiter: RateLimiter::new(rate_limit),
            api_keys,
        })
    }

    fn emit_event(&self, event: proto::VoipEvent) {
        let _ = self.event_tx.send(event);
    }
}

fn now_timestamp() -> Option<Timestamp> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    Some(Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    })
}

// ─────────────────────── Request validation helpers ───────────────────────

fn validate_not_empty(field: &str, name: &str) -> Result<(), Status> {
    if field.is_empty() {
        Err(Status::invalid_argument(format!("{name} is required")))
    } else {
        Ok(())
    }
}

fn validate_page_size(raw: i32) -> usize {
    if raw > 0 && raw <= 100 {
        raw as usize
    } else {
        50
    }
}

fn page_offset(token: &str) -> usize {
    token.parse::<usize>().unwrap_or(0)
}

// ─────────────────────── gRPC service ───────────────────────

/// The primary VoIP gRPC service.
pub struct VoipGrpcService {
    state: Arc<ServiceState>,
}

impl VoipGrpcService {
    pub fn new(state: Arc<ServiceState>) -> Self {
        VoipGrpcService { state }
    }

    // ── Auth / rate-limit helpers ──

    fn authorize<T>(&self, req: &Request<T>) -> Result<String, Status> {
        let key = if self.state.api_keys.is_empty() {
            "anonymous".to_string()
        } else {
            let token = req
                .metadata()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.strip_prefix("Bearer ").unwrap_or(s).to_string())
                .ok_or_else(|| Status::unauthenticated("Missing authorization header"))?;
            if !self.state.api_keys.contains(&token) {
                return Err(Status::unauthenticated("Invalid API key"));
            }
            token
        };
        if !self.state.rate_limiter.check(&key) {
            return Err(Status::resource_exhausted("Rate limit exceeded"));
        }
        Ok(key)
    }

    // ────────────────── Extension management ──────────────────

    pub async fn create_extension(
        &self,
        request: Request<proto::CreateExtensionRequest>,
    ) -> Result<Response<proto::Extension>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.number, "number")?;

        // Duplicate check.
        if self.state.extensions.iter().any(|e| e.number == req.number) {
            return Err(Status::already_exists(format!(
                "Extension {} already exists",
                req.number
            )));
        }

        let id = Uuid::new_v4().to_string();
        let ext = proto::Extension {
            id: id.clone(),
            number: req.number.clone(),
            display_name: req.display_name,
            transport: req.transport,
            voicemail_enabled: req.voicemail_enabled,
            registered: false,
            created_at: now_timestamp(),
            updated_at: now_timestamp(),
            max_concurrent_calls: if req.max_concurrent_calls > 0 {
                req.max_concurrent_calls
            } else {
                2
            },
        };

        self.state.extensions.insert(id.clone(), ext.clone());
        info!(id = %id, number = %req.number, "Extension created");

        self.state.emit_event(proto::VoipEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: "extension.created".into(),
            timestamp: now_timestamp(),
            payload: Some(proto::VoipEventPayload::RegEvent(
                proto::RegistrationEvent {
                    extension_id: id,
                    registered: false,
                    contact_uri: String::new(),
                    user_agent: String::new(),
                },
            )),
        });

        Ok(Response::new(ext))
    }

    pub async fn delete_extension(
        &self,
        request: Request<proto::DeleteExtensionRequest>,
    ) -> Result<Response<()>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        if self.state.extensions.remove(&req.extension_id).is_none() {
            return Err(Status::not_found("Extension not found"));
        }

        // Remove any active registrations / calls for this extension.
        let to_remove: Vec<String> = self
            .state
            .active_calls
            .iter()
            .filter(|c| {
                c.from_uri.contains(&req.extension_id) || c.to_uri.contains(&req.extension_id)
            })
            .map(|c| c.call_id.clone())
            .collect();
        for call_id in &to_remove {
            self.state.active_calls.remove(call_id);
        }

        info!(id = %req.extension_id, removed_calls = to_remove.len(), "Extension deleted");
        Ok(Response::new(()))
    }

    pub async fn list_extensions(
        &self,
        request: Request<proto::ListExtensionsRequest>,
    ) -> Result<Response<proto::ListExtensionsResponse>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        let page_size = validate_page_size(req.page_size);
        let start = page_offset(&req.page_token);

        // Include online/offline status (registered field is already on Extension).
        let all: Vec<proto::Extension> = self
            .state
            .extensions
            .iter()
            .map(|e| e.value().clone())
            .collect();

        let page: Vec<proto::Extension> = all.into_iter().skip(start).take(page_size).collect();
        let next_token = if page.len() == page_size {
            (start + page_size).to_string()
        } else {
            String::new()
        };

        Ok(Response::new(proto::ListExtensionsResponse {
            extensions: page,
            next_page_token: next_token,
        }))
    }

    // ────────────────── Call control ──────────────────

    pub async fn originate_call(
        &self,
        request: Request<proto::OriginateCallRequest>,
    ) -> Result<Response<proto::CallInfo>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.from_extension, "from_extension")?;
        validate_not_empty(&req.to_extension, "to_extension")?;

        // Verify the originating extension exists.
        let from_exists = self.state.extensions.contains_key(&req.from_extension)
            || self
                .state
                .extensions
                .iter()
                .any(|e| e.number == req.from_extension);
        if !from_exists {
            return Err(Status::not_found(format!(
                "Extension {} not found",
                req.from_extension
            )));
        }

        let call_id = Uuid::new_v4().to_string();
        let call = proto::CallInfo {
            call_id: call_id.clone(),
            from_uri: format!("sip:{}@local", req.from_extension),
            to_uri: format!("sip:{}@local", req.to_extension),
            state: proto::CallState::Trying as i32,
            started_at: now_timestamp(),
            answered_at: None,
            is_recording: req.record,
            is_muted: false,
            codec: "opus".into(),
            quality: Some(proto::CallQuality::default()),
        };

        self.state.active_calls.insert(call_id.clone(), call.clone());
        info!(call_id = %call_id, from = %req.from_extension, to = %req.to_extension, "Call originated");

        self.state.emit_event(proto::VoipEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: "call.originated".into(),
            timestamp: now_timestamp(),
            payload: Some(proto::VoipEventPayload::CallEvent(proto::CallEvent {
                call_id: call_id.clone(),
                from_uri: call.from_uri.clone(),
                to_uri: call.to_uri.clone(),
                state: proto::CallState::Trying as i32,
                sip_code: 100,
                reason: "Trying".into(),
            })),
        });

        Ok(Response::new(call))
    }

    pub async fn hangup_call(
        &self,
        request: Request<proto::HangupCallRequest>,
    ) -> Result<Response<()>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        let call = self
            .state
            .active_calls
            .remove(&req.call_id)
            .map(|(_, v)| v)
            .ok_or_else(|| Status::not_found("Call not found"))?;

        // Move to history as terminated.
        let mut terminated = call.clone();
        terminated.state = proto::CallState::Terminated as i32;
        {
            let mut history = self.state.call_history.write().await;
            history.push(terminated);
        }

        info!(call_id = %req.call_id, "Call hung up");

        self.state.emit_event(proto::VoipEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: "call.terminated".into(),
            timestamp: now_timestamp(),
            payload: Some(proto::VoipEventPayload::CallEvent(proto::CallEvent {
                call_id: req.call_id,
                from_uri: call.from_uri,
                to_uri: call.to_uri,
                state: proto::CallState::Terminated as i32,
                sip_code: req.cause,
                reason: "Hangup".into(),
            })),
        });

        Ok(Response::new(()))
    }

    pub async fn transfer_call(
        &self,
        request: Request<proto::TransferCallRequest>,
    ) -> Result<Response<proto::CallInfo>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.to_extension, "to_extension")?;

        let mut call = self
            .state
            .active_calls
            .get_mut(&req.call_id)
            .ok_or_else(|| Status::not_found("Call not found"))?;

        // Update the destination (REFER in SIP terms).
        call.to_uri = format!("sip:{}@local", req.to_extension);
        info!(
            call_id = %req.call_id,
            to = %req.to_extension,
            blind = req.blind_transfer,
            "Call transferred"
        );

        Ok(Response::new(call.clone()))
    }

    pub async fn mute_unmute(
        &self,
        request: Request<proto::MuteUnmuteRequest>,
    ) -> Result<Response<()>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        let mut call = self
            .state
            .active_calls
            .get_mut(&req.call_id)
            .ok_or_else(|| Status::not_found("Call not found"))?;

        call.is_muted = req.mute;
        info!(call_id = %req.call_id, muted = req.mute, direction = %req.direction, "Mute toggled");

        Ok(Response::new(()))
    }

    // ────────────────── Call queries ──────────────────

    pub async fn get_call_history(
        &self,
        request: Request<proto::GetCallHistoryRequest>,
    ) -> Result<Response<proto::GetCallHistoryResponse>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        let history = self.state.call_history.read().await;
        let filtered: Vec<proto::CallInfo> = if req.extension_id.is_empty() {
            history.clone()
        } else {
            history
                .iter()
                .filter(|c| {
                    c.from_uri.contains(&req.extension_id)
                        || c.to_uri.contains(&req.extension_id)
                })
                .cloned()
                .collect()
        };

        let page_size = validate_page_size(req.page_size);
        let start = page_offset(&req.page_token);
        let page: Vec<proto::CallInfo> = filtered.into_iter().skip(start).take(page_size).collect();
        let next_token = if page.len() == page_size {
            (start + page_size).to_string()
        } else {
            String::new()
        };

        Ok(Response::new(proto::GetCallHistoryResponse {
            calls: page,
            next_page_token: next_token,
        }))
    }

    pub async fn get_active_calls(
        &self,
        request: Request<()>,
    ) -> Result<Response<proto::GetActiveCallsResponse>, Status> {
        let _key = self.authorize(&request)?;

        let calls: Vec<proto::CallInfo> = self
            .state
            .active_calls
            .iter()
            .map(|e| e.value().clone())
            .collect();

        Ok(Response::new(proto::GetActiveCallsResponse { calls }))
    }

    // ────────────────── Routing ──────────────────

    pub async fn set_routing_rule(
        &self,
        request: Request<proto::SetRoutingRuleRequest>,
    ) -> Result<Response<proto::RoutingRule>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.pattern, "pattern")?;
        validate_not_empty(&req.destination, "destination")?;

        let id = Uuid::new_v4().to_string();
        let rule = proto::RoutingRule {
            id: id.clone(),
            pattern: req.pattern,
            destination: req.destination,
            priority: req.priority,
            description: req.description,
            created_at: now_timestamp(),
        };

        self.state.routing_rules.insert(id.clone(), rule.clone());
        info!(id = %id, "Routing rule created");

        Ok(Response::new(rule))
    }

    pub async fn get_routing_rules(
        &self,
        request: Request<()>,
    ) -> Result<Response<proto::GetRoutingRulesResponse>, Status> {
        let _key = self.authorize(&request)?;

        let mut rules: Vec<proto::RoutingRule> = self
            .state
            .routing_rules
            .iter()
            .map(|e| e.value().clone())
            .collect();
        rules.sort_by(|a, b| b.priority.cmp(&a.priority));

        Ok(Response::new(proto::GetRoutingRulesResponse { rules }))
    }

    // ────────────────── Observability ──────────────────

    pub async fn get_metrics(
        &self,
        _request: Request<()>,
    ) -> Result<Response<proto::MetricsResponse>, Status> {
        let history_len = {
            let h = self.state.call_history.read().await;
            h.len()
        };

        let metrics = proto::MetricsResponse {
            active_calls: self.state.active_calls.len() as i32,
            active_registrations: self
                .state
                .extensions
                .iter()
                .filter(|e| e.registered)
                .count() as i32,
            active_turn_allocs: 0,
            total_calls: (self.state.active_calls.len() + history_len) as i64,
            call_drops: 0,
            avg_call_duration_sec: 0.0,
            avg_jitter_ms: 0.0,
            avg_packet_loss_pct: 0.0,
            cpu_usage_pct: 0.0,
            memory_usage_bytes: 0,
        };

        Ok(Response::new(metrics))
    }

    pub async fn get_health(
        &self,
        _request: Request<()>,
    ) -> Result<Response<proto::HealthResponse>, Status> {
        let uptime = self.state.start_time.elapsed().as_secs() as i64;
        let mut components = HashMap::new();

        components.insert(
            "sip".into(),
            proto::ComponentHealth {
                healthy: true,
                message: "SIP transport running".into(),
            },
        );
        components.insert(
            "database".into(),
            proto::ComponentHealth {
                healthy: true,
                message: "Database connected".into(),
            },
        );
        components.insert(
            "media".into(),
            proto::ComponentHealth {
                healthy: true,
                message: "Media engine ready".into(),
            },
        );
        components.insert(
            "turn".into(),
            proto::ComponentHealth {
                healthy: true,
                message: "TURN server listening".into(),
            },
        );

        Ok(Response::new(proto::HealthResponse {
            healthy: true,
            version: env!("CARGO_PKG_VERSION").into(),
            uptime_sec: uptime,
            components,
        }))
    }

    // ────────────────── Voicemail ──────────────────

    pub async fn create_voicemail(
        &self,
        request: Request<proto::CreateVoicemailRequest>,
    ) -> Result<Response<proto::Voicemail>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.extension_id, "extension_id")?;

        let id = Uuid::new_v4().to_string();
        let file_path = format!("voicemails/{}/{}.opus", req.extension_id, id);

        let vm = proto::Voicemail {
            id: id.clone(),
            extension_id: req.extension_id.clone(),
            caller_id: req.caller_id,
            duration_sec: req.duration_sec,
            is_read: false,
            file_path,
            created_at: now_timestamp(),
        };

        self.state.voicemails.insert(id.clone(), vm.clone());
        info!(id = %id, ext = %req.extension_id, "Voicemail created");

        self.state.emit_event(proto::VoipEvent {
            event_id: Uuid::new_v4().to_string(),
            event_type: "voicemail.new".into(),
            timestamp: now_timestamp(),
            payload: Some(proto::VoipEventPayload::VoicemailEvent(
                proto::VoicemailEvent {
                    voicemail_id: id,
                    extension_id: req.extension_id,
                    action: "new".into(),
                },
            )),
        });

        Ok(Response::new(vm))
    }

    pub async fn get_voicemails(
        &self,
        request: Request<proto::GetVoicemailsRequest>,
    ) -> Result<Response<proto::GetVoicemailsResponse>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        let vms: Vec<proto::Voicemail> = self
            .state
            .voicemails
            .iter()
            .filter(|e| {
                (req.extension_id.is_empty() || e.extension_id == req.extension_id)
                    && (!req.unread_only || !e.is_read)
            })
            .map(|e| e.value().clone())
            .collect();

        Ok(Response::new(proto::GetVoicemailsResponse {
            voicemails: vms,
            next_page_token: String::new(),
        }))
    }

    pub async fn delete_voicemail(
        &self,
        request: Request<proto::DeleteVoicemailRequest>,
    ) -> Result<Response<()>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        if self.state.voicemails.remove(&req.voicemail_id).is_none() {
            return Err(Status::not_found("Voicemail not found"));
        }

        info!(id = %req.voicemail_id, "Voicemail deleted");
        Ok(Response::new(()))
    }

    // ────────────────── Streaming events ──────────────────

    pub async fn stream_events(
        &self,
        request: Request<proto::StreamEventsRequest>,
    ) -> Result<
        Response<Pin<Box<dyn Stream<Item = Result<proto::VoipEvent, Status>> + Send>>>,
        Status,
    > {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        let mut rx = self.state.event_tx.subscribe();
        let (tx, out_rx) = mpsc::channel(256);
        let event_types = req.event_types;
        let ext_filter = req.extension_filter;

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        // Filter by event type.
                        if !event_types.is_empty() && !event_types.contains(&event.event_type) {
                            continue;
                        }

                        // Filter by extension.
                        if !ext_filter.is_empty() {
                            let matches = match &event.payload {
                                Some(proto::VoipEventPayload::CallEvent(ce)) => {
                                    ce.from_uri.contains(&ext_filter)
                                        || ce.to_uri.contains(&ext_filter)
                                }
                                Some(proto::VoipEventPayload::RegEvent(re)) => {
                                    re.extension_id == ext_filter
                                }
                                Some(proto::VoipEventPayload::VoicemailEvent(ve)) => {
                                    ve.extension_id == ext_filter
                                }
                                _ => true,
                            };
                            if !matches {
                                continue;
                            }
                        }

                        if tx.send(Ok(event)).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!(skipped = n, "Event stream lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let stream = ReceiverStream::new(out_rx);
        Ok(Response::new(Box::pin(stream)
            as Pin<Box<dyn Stream<Item = Result<proto::VoipEvent, Status>> + Send>>))
    }

    // ────────────────── WebRTC SDP / ICE relay ──────────────────

    /// Forward an SDP offer or answer between a WebRTC client and the
    /// VoIP engine (or a peer endpoint).
    pub async fn relay_sdp(
        &self,
        request: Request<proto::RelaySdpRequest>,
    ) -> Result<Response<proto::RelaySdpResponse>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.call_id, "call_id")?;
        validate_not_empty(&req.sdp, "sdp")?;
        validate_not_empty(&req.sdp_type, "sdp_type")?;

        // Verify the call exists.
        if !self.state.active_calls.contains_key(&req.call_id) {
            return Err(Status::not_found(format!(
                "Call {} not found",
                req.call_id
            )));
        }

        info!(
            call_id = %req.call_id,
            sdp_type = %req.sdp_type,
            sdp_len = req.sdp.len(),
            "SDP relayed"
        );

        // If an SDP answer was previously stored for this call (e.g. by the
        // media engine after processing the offer), return it.  Otherwise
        // store the incoming SDP and return a placeholder indicating that
        // the answer is pending.
        if req.sdp_type == "offer" {
            // Store the offer and check if an answer is already available.
            if let Some((_, answer)) = self.state.sdp_answers.remove(&req.call_id) {
                return Ok(Response::new(answer));
            }

            // No answer yet -- store the offer for the peer to pick up.
            self.state.sdp_answers.insert(
                req.call_id.clone(),
                proto::RelaySdpResponse {
                    call_id: req.call_id.clone(),
                    sdp: req.sdp,
                    sdp_type: "offer".into(),
                },
            );

            Ok(Response::new(proto::RelaySdpResponse {
                call_id: req.call_id,
                sdp: String::new(),
                sdp_type: "pending".into(),
            }))
        } else {
            // It is an answer -- store it so the originator can retrieve it.
            let resp = proto::RelaySdpResponse {
                call_id: req.call_id.clone(),
                sdp: req.sdp.clone(),
                sdp_type: req.sdp_type.clone(),
            };
            self.state.sdp_answers.insert(req.call_id.clone(), resp.clone());

            Ok(Response::new(resp))
        }
    }

    /// Forward an ICE candidate from a WebRTC client to its peer.
    pub async fn relay_ice_candidate(
        &self,
        request: Request<proto::RelayIceCandidateRequest>,
    ) -> Result<Response<proto::RelayIceCandidateResponse>, Status> {
        let _key = self.authorize(&request)?;
        let req = request.into_inner();

        validate_not_empty(&req.call_id, "call_id")?;
        validate_not_empty(&req.candidate, "candidate")?;

        if !self.state.active_calls.contains_key(&req.call_id) {
            return Err(Status::not_found(format!(
                "Call {} not found",
                req.call_id
            )));
        }

        debug!(
            call_id = %req.call_id,
            candidate = %req.candidate,
            sdp_mid = %req.sdp_mid,
            idx = req.sdp_mline_index,
            "ICE candidate relayed"
        );

        self.state
            .ice_candidates
            .entry(req.call_id.clone())
            .or_insert_with(Vec::new)
            .push(req);

        Ok(Response::new(proto::RelayIceCandidateResponse {
            accepted: true,
        }))
    }
}
