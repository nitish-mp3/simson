use std::sync::Arc;
use std::time::{Duration, Instant};

use dashmap::DashMap;
use rand::Rng;
use tokio::sync::watch;
use tokio::time;
use tracing::{debug, info, trace, warn};

use super::parser::{
    SipHeader, SipMessage, SipMethod, parse_sip_message,
};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// SIP dialog states per RFC 3261 Section 12.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogState {
    /// No provisional response with a To tag yet.
    Initial,
    /// 1xx with a To tag received -- early dialog.
    Early,
    /// 2xx received and acknowledged -- dialog fully established.
    Confirmed,
    /// BYE completed or error -- dialog torn down.
    Terminated,
}

/// SIP transaction states per RFC 3261 Section 17.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    Trying,
    Proceeding,
    Completed,
    /// INVITE-server only: 2xx sent and ACK awaited.
    Confirmed,
    Terminated,
}

/// Whether we initiated the dialog or are the responder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Initiator,
    Responder,
}

// ---------------------------------------------------------------------------
// Dialog
// ---------------------------------------------------------------------------

/// Unique key for a dialog.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogId {
    pub call_id: String,
    pub local_tag: String,
    pub remote_tag: String,
}

/// A SIP dialog (typically created by INVITE / 2xx).
#[derive(Debug, Clone)]
pub struct Dialog {
    pub call_id: String,
    pub local_tag: String,
    pub remote_tag: Option<String>,
    pub local_uri: String,
    pub remote_uri: String,
    pub remote_target: String,
    pub local_seq: u32,
    pub remote_seq: u32,
    pub route_set: Vec<String>,
    pub state: DialogState,
    pub created_at: Instant,
    pub updated_at: Instant,
    pub direction: Direction,
}

impl Dialog {
    /// Produce the dialog-id triple.  Returns `None` if `remote_tag` is still unknown.
    pub fn dialog_id(&self) -> Option<DialogId> {
        self.remote_tag.as_ref().map(|rt| DialogId {
            call_id: self.call_id.clone(),
            local_tag: self.local_tag.clone(),
            remote_tag: rt.clone(),
        })
    }

    /// Advance the state when a response is **received** (UAC side).
    pub fn on_response_received(&mut self, status_code: u16, response: &SipMessage) {
        self.updated_at = Instant::now();

        // Learn remote tag from the first response that carries one.
        if self.remote_tag.is_none() {
            if let Some(tag) = response.to_tag() {
                self.remote_tag = Some(tag.to_string());
            }
        }

        match (self.state, status_code) {
            (DialogState::Initial, 100) => { /* stay Initial */ }
            (DialogState::Initial, 101..=199) => {
                if response.to_tag().is_some() {
                    self.state = DialogState::Early;
                    debug!(call_id = %self.call_id, "Dialog -> Early");
                }
            }
            (DialogState::Initial | DialogState::Early, 200..=299) => {
                self.state = DialogState::Confirmed;
                debug!(call_id = %self.call_id, "Dialog -> Confirmed");
            }
            (DialogState::Initial | DialogState::Early, 300..=699) => {
                self.state = DialogState::Terminated;
                debug!(call_id = %self.call_id, code = status_code, "Dialog -> Terminated (error)");
            }
            (DialogState::Early, 101..=199) => { /* stay Early */ }
            _ => {}
        }
    }

    /// Advance the state when a response is **sent** (UAS side).
    pub fn on_response_sent(&mut self, status_code: u16) {
        self.updated_at = Instant::now();
        match (self.state, status_code) {
            (DialogState::Initial, 100) => {}
            (DialogState::Initial, 101..=199) => {
                self.state = DialogState::Early;
            }
            (DialogState::Initial | DialogState::Early, 200..=299) => {
                self.state = DialogState::Confirmed;
                info!(call_id = %self.call_id, "Dialog confirmed (UAS)");
            }
            (_, 300..=699) => {
                self.state = DialogState::Terminated;
            }
            _ => {}
        }
    }

    /// Mark the dialog terminated (e.g. after BYE).
    pub fn terminate(&mut self) {
        self.state = DialogState::Terminated;
        self.updated_at = Instant::now();
        info!(call_id = %self.call_id, "Dialog terminated");
    }

    /// Allocate the next local CSeq number.
    pub fn next_local_seq(&mut self) -> u32 {
        self.local_seq += 1;
        self.local_seq
    }

    /// Is this dialog still alive?
    pub fn is_active(&self) -> bool {
        !matches!(self.state, DialogState::Terminated)
    }

    /// How long the dialog has existed.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }
}

// ---------------------------------------------------------------------------
// Transaction
// ---------------------------------------------------------------------------

/// A single SIP transaction (client or server, INVITE or non-INVITE).
#[derive(Debug)]
pub struct Transaction {
    /// Branch parameter from Via -- serves as the transaction id.
    pub id: String,
    pub method: SipMethod,
    pub state: TransactionState,
    pub request: SipMessage,
    pub last_response: Option<SipMessage>,
    /// When the next retransmission should fire.
    pub retransmit_timer: Instant,
    /// Absolute deadline after which the transaction times out.
    pub timeout_timer: Instant,
    pub retransmit_count: u32,
    /// Current retransmit interval; doubles on each retransmit per RFC 3261.
    pub retransmit_interval: Duration,
    /// Whether this is a server-side transaction.
    pub is_server: bool,
}

impl Transaction {
    // ---- client INVITE (RFC 3261 Section 17.1.1) ----

    /// Create a client INVITE transaction.
    /// Timer A starts at T1 (500 ms default), Timer B = 64*T1.
    pub fn new_client_invite(request: SipMessage, branch: &str, t1: Duration) -> Self {
        let now = Instant::now();
        Transaction {
            id: branch.to_string(),
            method: SipMethod::Invite,
            state: TransactionState::Trying,
            request,
            last_response: None,
            retransmit_timer: now + t1,
            timeout_timer: now + t1 * 64,
            retransmit_count: 0,
            retransmit_interval: t1,
            is_server: false,
        }
    }

    // ---- client non-INVITE (RFC 3261 Section 17.1.2) ----

    /// Create a client non-INVITE transaction.
    /// Timer E starts at T1, Timer F = 64*T1.
    pub fn new_client_non_invite(
        request: SipMessage,
        branch: &str,
        method: SipMethod,
        t1: Duration,
    ) -> Self {
        let now = Instant::now();
        Transaction {
            id: branch.to_string(),
            method,
            state: TransactionState::Trying,
            request,
            last_response: None,
            retransmit_timer: now + t1,
            timeout_timer: now + t1 * 64,
            retransmit_count: 0,
            retransmit_interval: t1,
            is_server: false,
        }
    }

    // ---- server INVITE (RFC 3261 Section 17.2.1) ----

    /// Create a server INVITE transaction.
    /// Timer G starts at T1 for retransmitting the final response.
    /// Timer H = 64*T1 (wait for ACK).
    pub fn new_server_invite(request: SipMessage, branch: &str, t1: Duration) -> Self {
        let now = Instant::now();
        Transaction {
            id: branch.to_string(),
            method: SipMethod::Invite,
            state: TransactionState::Trying,
            request,
            last_response: None,
            retransmit_timer: now + t1,
            timeout_timer: now + t1 * 64,
            retransmit_count: 0,
            retransmit_interval: t1,
            is_server: true,
        }
    }

    // ---- server non-INVITE (RFC 3261 Section 17.2.2) ----

    pub fn new_server_non_invite(
        request: SipMessage,
        branch: &str,
        method: SipMethod,
        t1: Duration,
    ) -> Self {
        let now = Instant::now();
        Transaction {
            id: branch.to_string(),
            method,
            state: TransactionState::Trying,
            request,
            last_response: None,
            retransmit_timer: now + t1,
            timeout_timer: now + t1 * 64,
            retransmit_count: 0,
            retransmit_interval: t1,
            is_server: true,
        }
    }

    // ---- state transitions on received response (client side) ----

    /// Feed a response into a **client** transaction and return the new state.
    pub fn on_response(&mut self, status_code: u16, response: SipMessage) -> TransactionState {
        let is_invite = self.method == SipMethod::Invite;
        match (is_invite, self.state, status_code) {
            // --- INVITE client ---
            (true, TransactionState::Trying, 100..=199) => {
                self.state = TransactionState::Proceeding;
                self.last_response = Some(response);
            }
            (true, TransactionState::Proceeding, 100..=199) => {
                self.last_response = Some(response);
            }
            (true, TransactionState::Trying | TransactionState::Proceeding, 200..=299) => {
                // 2xx terminates the INVITE client transaction directly
                // (the ACK for 2xx is generated at the TU/dialog layer).
                self.state = TransactionState::Terminated;
                self.last_response = Some(response);
            }
            (true, TransactionState::Trying | TransactionState::Proceeding, 300..=699) => {
                self.state = TransactionState::Completed;
                self.last_response = Some(response);
            }
            (true, TransactionState::Completed, 300..=699) => {
                // retransmission of final response -- absorb
            }

            // --- non-INVITE client ---
            (false, TransactionState::Trying, 100..=199) => {
                self.state = TransactionState::Proceeding;
                self.last_response = Some(response);
            }
            (false, TransactionState::Proceeding, 100..=199) => {
                self.last_response = Some(response);
            }
            (false, TransactionState::Trying | TransactionState::Proceeding, 200..=699) => {
                self.state = TransactionState::Completed;
                self.last_response = Some(response);
            }

            _ => {
                trace!(branch = %self.id, state = ?self.state, code = status_code, "Response ignored");
            }
        }
        self.state
    }

    // ---- state transitions on received request (server side) ----

    /// Feed a response code that the server TU is sending into the server
    /// transaction.  Returns the new state.
    pub fn on_send_response(&mut self, status_code: u16, response: SipMessage) -> TransactionState {
        let is_invite = self.method == SipMethod::Invite;
        match (is_invite, self.state, status_code) {
            // --- INVITE server ---
            (true, TransactionState::Trying, 100..=199) => {
                self.state = TransactionState::Proceeding;
                self.last_response = Some(response);
            }
            (true, TransactionState::Proceeding, 100..=199) => {
                self.last_response = Some(response);
            }
            (true, TransactionState::Trying | TransactionState::Proceeding, 200..=299) => {
                // For 2xx, the TU is responsible for retransmission;
                // the transaction moves to Terminated per RFC 6026.
                self.state = TransactionState::Terminated;
                self.last_response = Some(response);
            }
            (true, TransactionState::Trying | TransactionState::Proceeding, 300..=699) => {
                self.state = TransactionState::Completed;
                self.last_response = Some(response);
                // Start Timer G for retransmitting the response (UDP)
                let now = Instant::now();
                self.retransmit_timer = now + self.retransmit_interval;
            }

            // --- non-INVITE server ---
            (false, TransactionState::Trying, 100..=199) => {
                self.state = TransactionState::Proceeding;
                self.last_response = Some(response);
            }
            (false, TransactionState::Trying | TransactionState::Proceeding, 200..=699) => {
                self.state = TransactionState::Completed;
                self.last_response = Some(response);
            }

            _ => {}
        }
        self.state
    }

    /// Mark INVITE-server transaction as Confirmed (ACK received for 3xx-6xx).
    pub fn on_ack_received(&mut self) {
        if self.method == SipMethod::Invite
            && self.is_server
            && self.state == TransactionState::Completed
        {
            self.state = TransactionState::Confirmed;
            // Timer I fires to absorb ACK retransmissions; for simplicity
            // we rely on the transaction manager cleanup loop.
        }
    }

    /// Should the request/response be retransmitted right now?
    pub fn needs_retransmit(&self, now: Instant) -> bool {
        if now < self.retransmit_timer {
            return false;
        }
        match (self.is_server, self.method == SipMethod::Invite, self.state) {
            // Client INVITE Trying: retransmit the request
            (false, true, TransactionState::Trying) => true,
            // Client non-INVITE Trying/Proceeding: retransmit the request
            (false, false, TransactionState::Trying | TransactionState::Proceeding) => true,
            // Server INVITE Completed: retransmit the final response (Timer G)
            (true, true, TransactionState::Completed) => true,
            _ => false,
        }
    }

    /// Double the retransmit interval (capped at T2) and advance the timer.
    pub fn bump_retransmit(&mut self, t2: Duration) {
        self.retransmit_count += 1;
        self.retransmit_interval = std::cmp::min(self.retransmit_interval * 2, t2);
        self.retransmit_timer = Instant::now() + self.retransmit_interval;
    }

    /// Has the absolute timeout elapsed?
    pub fn is_timed_out(&self, now: Instant) -> bool {
        now >= self.timeout_timer
    }

    /// Force the transaction into Terminated.
    pub fn terminate(&mut self) {
        self.state = TransactionState::Terminated;
    }
}

// ---------------------------------------------------------------------------
// DialogManager
// ---------------------------------------------------------------------------

/// Manages the set of active SIP dialogs.
pub struct DialogManager {
    /// Key = (call_id, local_tag, remote_tag).
    dialogs: DashMap<DialogId, Dialog>,
    domain: String,
}

impl DialogManager {
    pub fn new(domain: String) -> Self {
        DialogManager {
            dialogs: DashMap::new(),
            domain,
        }
    }

    /// Create a dialog from an outgoing INVITE request + its 2xx response
    /// (UAC perspective).
    pub fn create_dialog_from_response(
        &self,
        request: &SipMessage,
        response: &SipMessage,
    ) -> Option<Dialog> {
        let call_id = request.call_id()?.to_string();
        let local_tag = request.from_tag()?.to_string();
        let remote_tag = response.to_tag().map(|s| s.to_string());
        let local_uri = format_from_header(request);
        let remote_uri = format_to_header(request);
        let remote_target = response
            .contact_uri()
            .map(|u| u.to_string())
            .unwrap_or_default();
        let cseq = request.cseq_seq().unwrap_or(1);

        let now = Instant::now();
        let dialog = Dialog {
            call_id: call_id.clone(),
            local_tag: local_tag.clone(),
            remote_tag: remote_tag.clone(),
            local_uri,
            remote_uri,
            remote_target,
            local_seq: cseq,
            remote_seq: 0,
            route_set: extract_record_routes(response),
            state: if remote_tag.is_some() {
                DialogState::Confirmed
            } else {
                DialogState::Early
            },
            created_at: now,
            updated_at: now,
            direction: Direction::Initiator,
        };

        if let Some(id) = dialog.dialog_id() {
            self.dialogs.insert(id, dialog.clone());
            info!(call_id = %call_id, "UAC dialog created");
        }

        Some(dialog)
    }

    /// Create a dialog from an incoming INVITE request (UAS perspective).
    /// The `local_tag` is generated automatically.
    pub fn create_dialog_from_request(&self, request: &SipMessage) -> Option<Dialog> {
        let call_id = request.call_id()?.to_string();
        let remote_tag = request.from_tag().map(|s| s.to_string());
        let local_tag = generate_tag();
        let local_uri = format_to_header(request);
        let remote_uri = format_from_header(request);
        let remote_target = request
            .contact_uri()
            .map(|u| u.to_string())
            .unwrap_or_default();
        let cseq = request.cseq_seq().unwrap_or(1);

        let now = Instant::now();
        let dialog = Dialog {
            call_id: call_id.clone(),
            local_tag: local_tag.clone(),
            remote_tag: remote_tag.clone(),
            local_uri,
            remote_uri,
            remote_target,
            local_seq: 0,
            remote_seq: cseq,
            route_set: extract_record_routes(request),
            state: DialogState::Initial,
            created_at: now,
            updated_at: now,
            direction: Direction::Responder,
        };

        if let Some(id) = dialog.dialog_id() {
            self.dialogs.insert(id, dialog.clone());
        } else {
            // Remote tag might not be known yet; store by call_id + local_tag
            let temp_id = DialogId {
                call_id: call_id.clone(),
                local_tag: local_tag.clone(),
                remote_tag: remote_tag.clone().unwrap_or_default(),
            };
            self.dialogs.insert(temp_id, dialog.clone());
        }

        info!(call_id = %call_id, "UAS dialog created");
        Some(dialog)
    }

    /// Find the dialog that matches an inbound message (by Call-ID + tags).
    pub fn match_dialog(&self, message: &SipMessage) -> Option<Dialog> {
        let call_id = message.call_id()?;
        // Determine local and remote tags based on direction.
        // For incoming requests: From-tag is the remote tag.
        // For incoming responses: To-tag is the remote tag (for us the UAC).
        // We search all dialogs with matching call_id and compare tags.
        for entry in self.dialogs.iter() {
            if entry.call_id != call_id {
                continue;
            }
            let d = entry.value();
            match (message.is_request(), d.direction) {
                // Incoming request to UAS: From-tag = remote, To-tag = local
                (true, Direction::Responder) => {
                    let from_tag = message.from_tag().unwrap_or("");
                    let to_tag = message.to_tag().unwrap_or("");
                    if (d.remote_tag.as_deref() == Some(from_tag) || d.remote_tag.is_none())
                        && (to_tag.is_empty() || d.local_tag == to_tag)
                    {
                        return Some(d.clone());
                    }
                }
                // Incoming response to UAC: To-tag = remote, From-tag = local
                (false, Direction::Initiator) => {
                    let from_tag = message.from_tag().unwrap_or("");
                    let to_tag = message.to_tag().unwrap_or("");
                    if d.local_tag == from_tag
                        && (d.remote_tag.as_deref() == Some(to_tag)
                            || d.remote_tag.is_none()
                            || to_tag.is_empty())
                    {
                        return Some(d.clone());
                    }
                }
                // Also try the opposite direction for mid-dialog requests
                _ => {
                    let from_tag = message.from_tag().unwrap_or("");
                    let to_tag = message.to_tag().unwrap_or("");
                    if (d.local_tag == to_tag || to_tag.is_empty())
                        && (d.remote_tag.as_deref() == Some(from_tag)
                            || d.remote_tag.is_none())
                    {
                        return Some(d.clone());
                    }
                }
            }
        }
        None
    }

    /// Update dialog state given an inbound message.
    pub fn update_dialog(&self, message: &SipMessage) -> Option<DialogState> {
        let call_id = message.call_id()?;
        for mut entry in self.dialogs.iter_mut() {
            if entry.call_id != call_id {
                continue;
            }
            if let Some(code) = message.status_code() {
                if entry.direction == Direction::Initiator {
                    entry.on_response_received(code, message);
                } else {
                    entry.on_response_sent(code);
                }
                return Some(entry.state);
            }
            // Handle BYE
            if message.method() == Some(&SipMethod::Bye) {
                entry.terminate();
                return Some(DialogState::Terminated);
            }
            return Some(entry.state);
        }
        None
    }

    /// Terminate the dialog identified by `call_id`.
    pub fn terminate_dialog(&self, call_id: &str) {
        for mut entry in self.dialogs.iter_mut() {
            if entry.call_id == call_id {
                entry.terminate();
            }
        }
    }

    /// Build the set of response messages the server should send for an
    /// incoming request.  This is a simplified dispatcher -- a full
    /// implementation would consult registrar/location logic.
    pub fn process_request(&self, message: &SipMessage) -> Vec<SipMessage> {
        let mut responses = Vec::new();
        let method = match message.method() {
            Some(m) => m.clone(),
            None => return responses,
        };

        match method {
            SipMethod::Invite => {
                // Auto-generate 100 Trying
                let trying = SipMessage::new_response(100, "Trying")
                    .copy_headers_from(message)
                    .build();
                responses.push(trying);
            }
            SipMethod::Bye => {
                if let Some(call_id) = message.call_id() {
                    self.terminate_dialog(call_id);
                }
                let ok = SipMessage::new_response(200, "OK")
                    .copy_headers_from(message)
                    .build();
                responses.push(ok);
            }
            SipMethod::Cancel => {
                let ok = SipMessage::new_response(200, "OK")
                    .copy_headers_from(message)
                    .build();
                responses.push(ok);
            }
            SipMethod::Options => {
                let ok = SipMessage::new_response(200, "OK")
                    .copy_headers_from(message)
                    .header(SipHeader::Allow(vec![
                        SipMethod::Invite,
                        SipMethod::Ack,
                        SipMethod::Bye,
                        SipMethod::Cancel,
                        SipMethod::Options,
                        SipMethod::Register,
                    ]))
                    .build();
                responses.push(ok);
            }
            _ => {}
        }

        responses
    }

    /// Handle an inbound response, updating dialog and returning the new state.
    pub fn process_response(&self, message: &SipMessage) -> Option<DialogState> {
        self.update_dialog(message)
    }

    /// Get a snapshot of a dialog by ID.
    pub fn get_dialog(&self, id: &DialogId) -> Option<Dialog> {
        self.dialogs.get(id).map(|d| d.clone())
    }

    /// Find a dialog by call-id (returns the first match).
    pub fn find_by_call_id(&self, call_id: &str) -> Option<Dialog> {
        self.dialogs
            .iter()
            .find(|e| e.call_id == call_id)
            .map(|e| e.value().clone())
    }

    /// Number of active dialogs.
    pub fn active_count(&self) -> usize {
        self.dialogs.iter().filter(|e| e.is_active()).count()
    }

    /// Remove terminated dialogs.
    pub fn cleanup(&self) {
        self.dialogs.retain(|_, d| d.is_active());
    }
}

// ---------------------------------------------------------------------------
// TransactionManager
// ---------------------------------------------------------------------------

/// Manages the set of active SIP transactions and their timers.
pub struct TransactionManager {
    /// Key = branch (Via branch parameter).
    transactions: DashMap<String, Transaction>,
    /// RFC 3261 Timer T1 (round-trip time estimate, default 500 ms).
    t1: Duration,
    /// RFC 3261 Timer T2 (maximum retransmit interval, default 4 s).
    t2: Duration,
    /// RFC 3261 Timer T4 (maximum time a message will remain in the network, 5 s).
    t4: Duration,
}

impl TransactionManager {
    pub fn new(t1_ms: u64, t2_ms: u64, t4_ms: u64) -> Self {
        TransactionManager {
            transactions: DashMap::new(),
            t1: Duration::from_millis(t1_ms),
            t2: Duration::from_millis(t2_ms),
            t4: Duration::from_millis(t4_ms),
        }
    }

    /// Create and store a new **client** transaction for an outgoing request.
    /// Returns the branch that identifies the transaction.
    pub fn create_client_transaction(&self, request: SipMessage) -> String {
        let branch = generate_branch();
        let method = request.method().cloned().unwrap_or(SipMethod::Options);
        let tx = if method == SipMethod::Invite {
            Transaction::new_client_invite(request, &branch, self.t1)
        } else {
            Transaction::new_client_non_invite(request, &branch, method, self.t1)
        };
        debug!(branch = %branch, method = %tx.method, "Client transaction created");
        self.transactions.insert(branch.clone(), tx);
        branch
    }

    /// Create and store a new **server** transaction for an incoming request.
    pub fn create_server_transaction(&self, request: SipMessage) -> String {
        let branch = request
            .via_branch()
            .map(|s| s.to_string())
            .unwrap_or_else(generate_branch);
        let method = request.method().cloned().unwrap_or(SipMethod::Options);
        let tx = if method == SipMethod::Invite {
            Transaction::new_server_invite(request, &branch, self.t1)
        } else {
            Transaction::new_server_non_invite(request, &branch, method, self.t1)
        };
        debug!(branch = %branch, method = %tx.method, "Server transaction created");
        self.transactions.insert(branch.clone(), tx);
        branch
    }

    /// Feed an inbound response into the matching client transaction.
    pub fn process_client_response(
        &self,
        branch: &str,
        status_code: u16,
        response: SipMessage,
    ) -> Option<TransactionState> {
        let mut entry = self.transactions.get_mut(branch)?;
        let new_state = entry.on_response(status_code, response);
        Some(new_state)
    }

    /// Feed an outbound response into the matching server transaction.
    pub fn send_server_response(
        &self,
        branch: &str,
        status_code: u16,
        response: SipMessage,
    ) -> Option<TransactionState> {
        let mut entry = self.transactions.get_mut(branch)?;
        let new_state = entry.on_send_response(status_code, response);
        Some(new_state)
    }

    /// Notify the server INVITE transaction that an ACK was received.
    pub fn on_ack(&self, branch: &str) {
        if let Some(mut entry) = self.transactions.get_mut(branch) {
            entry.on_ack_received();
        }
    }

    /// Walk all transactions and collect the ones that need a retransmission
    /// right now.  Returns a vec of `(branch, message_to_retransmit)`.
    pub fn process_timer_event(&self) -> Vec<(String, SipMessage)> {
        let now = Instant::now();
        let mut retransmits = Vec::new();

        for mut entry in self.transactions.iter_mut() {
            let tx = entry.value_mut();

            // Timeout check first
            if tx.is_timed_out(now) {
                warn!(branch = %tx.id, method = %tx.method, "Transaction timed out");
                tx.terminate();
                continue;
            }

            if tx.needs_retransmit(now) {
                // Determine what to retransmit
                let msg = if tx.is_server {
                    // Server retransmits the last response (Timer G)
                    tx.last_response.clone()
                } else {
                    // Client retransmits the original request (Timer A / Timer E)
                    Some(tx.request.clone())
                };

                if let Some(m) = msg {
                    retransmits.push((tx.id.clone(), m));
                }

                tx.bump_retransmit(self.t2);
            }
        }

        retransmits
    }

    /// Get a snapshot of a transaction.
    pub fn get_transaction(&self, branch: &str) -> Option<TransactionState> {
        self.transactions.get(branch).map(|e| e.state)
    }

    /// Number of live transactions.
    pub fn active_count(&self) -> usize {
        self.transactions
            .iter()
            .filter(|e| e.state != TransactionState::Terminated)
            .count()
    }

    /// Purge terminated and timed-out transactions.
    pub fn cleanup(&self) {
        let now = Instant::now();
        self.transactions.retain(|_, tx| {
            if tx.state == TransactionState::Terminated {
                return false;
            }
            if tx.is_timed_out(now) {
                warn!(branch = %tx.id, "Removing timed-out transaction");
                return false;
            }
            true
        });
    }

    // ---- ID generators ----

    /// Generate a Via branch with the RFC 3261 magic cookie.
    pub fn generate_branch() -> String {
        generate_branch()
    }

    /// Generate a random tag suitable for From / To headers.
    pub fn generate_tag() -> String {
        generate_tag()
    }

    /// Generate a globally unique Call-ID.
    pub fn generate_call_id(domain: &str) -> String {
        generate_call_id(domain)
    }
}

// ---------------------------------------------------------------------------
// Free-standing ID generators (also used by DialogManager)
// ---------------------------------------------------------------------------

/// Branch with the RFC 3261 magic cookie `z9hG4bK` + random hex.
pub fn generate_branch() -> String {
    let mut rng = rand::thread_rng();
    let r: u64 = rng.gen();
    format!("z9hG4bK-{:016x}", r)
}

/// Random 12-hex-digit tag.
pub fn generate_tag() -> String {
    let mut rng = rand::thread_rng();
    let r: u64 = rng.gen();
    format!("{:012x}", r & 0xFFFF_FFFF_FFFF)
}

/// UUID-based Call-ID.
pub fn generate_call_id(domain: &str) -> String {
    let id = uuid::Uuid::new_v4();
    format!("{}@{}", id, domain)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract Record-Route header values into the route set.
fn extract_record_routes(msg: &SipMessage) -> Vec<String> {
    msg.headers()
        .iter()
        .filter_map(|h| {
            if let SipHeader::Other { name, value } = h {
                if name.eq_ignore_ascii_case("Record-Route") {
                    return Some(value.clone());
                }
            }
            None
        })
        .collect()
}

/// Render the From header as a URI string (for dialog local/remote URI).
fn format_from_header(msg: &SipMessage) -> String {
    msg.from_uri()
        .map(|u| u.to_string())
        .unwrap_or_default()
}

/// Render the To header as a URI string.
fn format_to_header(msg: &SipMessage) -> String {
    msg.to_uri()
        .map(|u| u.to_string())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Periodic maintenance task
// ---------------------------------------------------------------------------

/// Spawn a background task that periodically cleans up terminated dialogs and
/// transactions.  Runs every 5 seconds until `shutdown` fires.
pub async fn run_maintenance(
    dialog_mgr: Arc<DialogManager>,
    tx_mgr: Arc<TransactionManager>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut interval = time::interval(Duration::from_secs(5));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                dialog_mgr.cleanup();
                tx_mgr.cleanup();
                trace!(
                    dialogs = dialog_mgr.active_count(),
                    transactions = tx_mgr.active_count(),
                    "SIP maintenance sweep"
                );
            }
            _ = shutdown.changed() => {
                info!("SIP maintenance task shutting down");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sip::parser::parse_sip_message;

    fn sample_invite() -> SipMessage {
        let raw = b"INVITE sip:bob@example.com SIP/2.0\r\n\
Via: SIP/2.0/UDP 10.0.0.1:5060;branch=z9hG4bK776asdhds\r\n\
From: Alice <sip:alice@example.com>;tag=1234\r\n\
To: Bob <sip:bob@example.com>\r\n\
Call-ID: test-call-1@example.com\r\n\
CSeq: 1 INVITE\r\n\
Contact: <sip:alice@10.0.0.1:5060>\r\n\
Content-Length: 0\r\n\
\r\n";
        let (_, msg) = parse_sip_message(raw).unwrap();
        msg
    }

    fn sample_200() -> SipMessage {
        let raw = b"SIP/2.0 200 OK\r\n\
Via: SIP/2.0/UDP 10.0.0.1:5060;branch=z9hG4bK776asdhds\r\n\
From: Alice <sip:alice@example.com>;tag=1234\r\n\
To: Bob <sip:bob@example.com>;tag=5678\r\n\
Call-ID: test-call-1@example.com\r\n\
CSeq: 1 INVITE\r\n\
Contact: <sip:bob@10.0.0.2:5060>\r\n\
Content-Length: 0\r\n\
\r\n";
        let (_, msg) = parse_sip_message(raw).unwrap();
        msg
    }

    #[test]
    fn test_client_invite_transaction_lifecycle() {
        let invite = sample_invite();
        let t1 = Duration::from_millis(500);
        let mut tx = Transaction::new_client_invite(invite, "z9hG4bK776asdhds", t1);
        assert_eq!(tx.state, TransactionState::Trying);

        // Feed 100 Trying
        let raw_100 = b"SIP/2.0 100 Trying\r\n\
Via: SIP/2.0/UDP 10.0.0.1:5060;branch=z9hG4bK776asdhds\r\n\
From: Alice <sip:alice@example.com>;tag=1234\r\n\
To: Bob <sip:bob@example.com>\r\n\
Call-ID: test-call-1@example.com\r\n\
CSeq: 1 INVITE\r\n\
Content-Length: 0\r\n\
\r\n";
        let (_, trying) = parse_sip_message(raw_100).unwrap();
        tx.on_response(100, trying);
        assert_eq!(tx.state, TransactionState::Proceeding);

        // Feed 200 OK
        let ok = sample_200();
        tx.on_response(200, ok);
        assert_eq!(tx.state, TransactionState::Terminated);
    }

    #[test]
    fn test_dialog_creation_uac() {
        let invite = sample_invite();
        let ok = sample_200();
        let mgr = DialogManager::new("example.com".to_string());

        let dialog = mgr
            .create_dialog_from_response(&invite, &ok)
            .expect("create dialog");
        assert_eq!(dialog.state, DialogState::Confirmed);
        assert_eq!(dialog.call_id, "test-call-1@example.com");
        assert_eq!(dialog.local_tag, "1234");
        assert_eq!(dialog.remote_tag, Some("5678".to_string()));
        assert_eq!(dialog.direction, Direction::Initiator);
    }

    #[test]
    fn test_dialog_creation_uas() {
        let invite = sample_invite();
        let mgr = DialogManager::new("example.com".to_string());

        let dialog = mgr
            .create_dialog_from_request(&invite)
            .expect("create dialog");
        assert_eq!(dialog.state, DialogState::Initial);
        assert_eq!(dialog.direction, Direction::Responder);
        assert!(!dialog.local_tag.is_empty());
    }

    #[test]
    fn test_dialog_terminate() {
        let mut dialog = Dialog {
            call_id: "test@example.com".to_string(),
            local_tag: "lt".to_string(),
            remote_tag: Some("rt".to_string()),
            local_uri: "sip:a@example.com".to_string(),
            remote_uri: "sip:b@example.com".to_string(),
            remote_target: "sip:b@10.0.0.2".to_string(),
            local_seq: 1,
            remote_seq: 0,
            route_set: Vec::new(),
            state: DialogState::Confirmed,
            created_at: Instant::now(),
            updated_at: Instant::now(),
            direction: Direction::Initiator,
        };
        assert!(dialog.is_active());
        dialog.terminate();
        assert!(!dialog.is_active());
        assert_eq!(dialog.state, DialogState::Terminated);
    }

    #[test]
    fn test_transaction_timeout() {
        let invite = sample_invite();
        let t1 = Duration::from_millis(1); // tiny for test
        let tx = Transaction::new_client_invite(invite, "branch", t1);
        // Timer B = 64 * T1 = 64 ms
        std::thread::sleep(Duration::from_millis(100));
        assert!(tx.is_timed_out(Instant::now()));
    }

    #[test]
    fn test_generate_branch_has_magic_cookie() {
        let branch = generate_branch();
        assert!(branch.starts_with("z9hG4bK"));
    }

    #[test]
    fn test_generate_tag_length() {
        let tag = generate_tag();
        assert_eq!(tag.len(), 12);
    }

    #[test]
    fn test_generate_call_id_contains_domain() {
        let cid = generate_call_id("voip.example.com");
        assert!(cid.ends_with("@voip.example.com"));
    }

    #[test]
    fn test_transaction_manager_roundtrip() {
        let mgr = TransactionManager::new(500, 4000, 5000);

        let invite = sample_invite();
        let branch = mgr.create_client_transaction(invite);
        assert!(!branch.is_empty());

        let state = mgr.get_transaction(&branch);
        assert_eq!(state, Some(TransactionState::Trying));

        let ok = sample_200();
        let new_state = mgr.process_client_response(&branch, 200, ok);
        assert_eq!(new_state, Some(TransactionState::Terminated));
    }
}
