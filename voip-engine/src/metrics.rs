//! Prometheus metrics for ha-voip engine.
//!
//! Collects and exposes operational metrics: call counts, registration stats,
//! TURN allocations, media quality, and system resource usage.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use prometheus::{
    Encoder, GaugeVec, HistogramOpts, HistogramVec, IntCounter, IntCounterVec, IntGauge,
    IntGaugeVec, Opts, Registry, TextEncoder,
};
use tracing::error;

/// Central metrics registry for the VoIP engine
pub struct VoipMetrics {
    registry: Registry,

    // Registration metrics
    pub active_registrations: IntGauge,
    pub registration_failures: IntCounter,

    // Call metrics
    pub active_calls: IntGauge,
    pub total_calls: IntCounter,
    pub call_drops: IntCounter,
    pub call_duration: HistogramVec,
    pub call_setup_time: HistogramVec,
    pub calls_by_status: IntCounterVec,

    // Media quality metrics
    pub packet_loss: HistogramVec,
    pub jitter: HistogramVec,
    pub mos_score: HistogramVec,

    // TURN metrics
    pub active_turn_allocations: IntGauge,
    pub turn_allocation_failures: IntCounter,
    pub turn_bytes_relayed: IntCounter,

    // System metrics
    pub cpu_usage: GaugeVec,
    pub memory_usage: IntGauge,
    pub uptime_seconds: IntGauge,

    // Transport metrics
    pub sip_messages_in: IntCounterVec,
    pub sip_messages_out: IntCounterVec,
    pub websocket_connections: IntGauge,
}

impl VoipMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        // Registration metrics
        let active_registrations = IntGauge::with_opts(
            Opts::new("voip_active_registrations", "Number of currently registered extensions")
        ).unwrap();
        let registration_failures = IntCounter::with_opts(
            Opts::new("voip_registration_failures_total", "Total registration failures")
        ).unwrap();

        // Call metrics
        let active_calls = IntGauge::with_opts(
            Opts::new("voip_active_calls", "Number of currently active calls")
        ).unwrap();
        let total_calls = IntCounter::with_opts(
            Opts::new("voip_total_calls", "Total number of calls processed")
        ).unwrap();
        let call_drops = IntCounter::with_opts(
            Opts::new("voip_call_drops_total", "Total number of dropped calls")
        ).unwrap();
        let call_duration = HistogramVec::new(
            HistogramOpts::new("voip_call_duration_seconds", "Call duration in seconds")
                .buckets(vec![5.0, 15.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0]),
            &["direction"],
        ).unwrap();
        let call_setup_time = HistogramVec::new(
            HistogramOpts::new("voip_call_setup_seconds", "Call setup time in seconds")
                .buckets(vec![0.1, 0.25, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0]),
            &["transport"],
        ).unwrap();
        let calls_by_status = IntCounterVec::new(
            Opts::new("voip_calls_by_status_total", "Calls by final status"),
            &["status"],
        ).unwrap();

        // Media quality
        let packet_loss = HistogramVec::new(
            HistogramOpts::new("voip_packet_loss_ratio", "Packet loss ratio per call")
                .buckets(vec![0.001, 0.005, 0.01, 0.02, 0.03, 0.05, 0.10, 0.20]),
            &["direction"],
        ).unwrap();
        let jitter = HistogramVec::new(
            HistogramOpts::new("voip_jitter_ms", "Jitter in milliseconds")
                .buckets(vec![1.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0]),
            &["direction"],
        ).unwrap();
        let mos_score = HistogramVec::new(
            HistogramOpts::new("voip_mos_score", "Estimated MOS quality score")
                .buckets(vec![1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.3, 4.5, 5.0]),
            &["codec"],
        ).unwrap();

        // TURN metrics
        let active_turn_allocations = IntGauge::with_opts(
            Opts::new("voip_turn_active_allocations", "Active TURN allocations")
        ).unwrap();
        let turn_allocation_failures = IntCounter::with_opts(
            Opts::new("voip_turn_allocation_failures_total", "TURN allocation failures")
        ).unwrap();
        let turn_bytes_relayed = IntCounter::with_opts(
            Opts::new("voip_turn_bytes_relayed_total", "Total bytes relayed through TURN")
        ).unwrap();

        // System metrics
        let cpu_usage = GaugeVec::new(
            Opts::new("voip_cpu_usage_ratio", "CPU usage ratio"),
            &["core"],
        ).unwrap();
        let memory_usage = IntGauge::with_opts(
            Opts::new("voip_memory_usage_bytes", "Memory usage in bytes")
        ).unwrap();
        let uptime_seconds = IntGauge::with_opts(
            Opts::new("voip_uptime_seconds", "Engine uptime in seconds")
        ).unwrap();

        // Transport metrics
        let sip_messages_in = IntCounterVec::new(
            Opts::new("voip_sip_messages_received_total", "SIP messages received"),
            &["method", "transport"],
        ).unwrap();
        let sip_messages_out = IntCounterVec::new(
            Opts::new("voip_sip_messages_sent_total", "SIP messages sent"),
            &["method", "transport"],
        ).unwrap();
        let websocket_connections = IntGauge::with_opts(
            Opts::new("voip_websocket_connections", "Active WebSocket connections")
        ).unwrap();

        // Register all metrics
        let metrics_list: Vec<Box<dyn prometheus::core::Collector>> = vec![
            Box::new(active_registrations.clone()),
            Box::new(registration_failures.clone()),
            Box::new(active_calls.clone()),
            Box::new(total_calls.clone()),
            Box::new(call_drops.clone()),
            Box::new(call_duration.clone()),
            Box::new(call_setup_time.clone()),
            Box::new(calls_by_status.clone()),
            Box::new(packet_loss.clone()),
            Box::new(jitter.clone()),
            Box::new(mos_score.clone()),
            Box::new(active_turn_allocations.clone()),
            Box::new(turn_allocation_failures.clone()),
            Box::new(turn_bytes_relayed.clone()),
            Box::new(cpu_usage.clone()),
            Box::new(memory_usage.clone()),
            Box::new(uptime_seconds.clone()),
            Box::new(sip_messages_in.clone()),
            Box::new(sip_messages_out.clone()),
            Box::new(websocket_connections.clone()),
        ];

        for m in metrics_list {
            registry.register(m).unwrap();
        }

        Self {
            registry,
            active_registrations,
            registration_failures,
            active_calls,
            total_calls,
            call_drops,
            call_duration,
            call_setup_time,
            calls_by_status,
            packet_loss,
            jitter,
            mos_score,
            active_turn_allocations,
            turn_allocation_failures,
            turn_bytes_relayed,
            cpu_usage,
            memory_usage,
            uptime_seconds,
            sip_messages_in,
            sip_messages_out,
            websocket_connections,
        }
    }

    /// Record a call completion with metrics
    pub fn record_call_end(
        &self,
        duration_secs: f64,
        direction: &str,
        status: &str,
        loss_ratio: f64,
        jitter_ms: f64,
    ) {
        self.call_duration.with_label_values(&[direction]).observe(duration_secs);
        self.calls_by_status.with_label_values(&[status]).inc();
        self.packet_loss.with_label_values(&[direction]).observe(loss_ratio);
        self.jitter.with_label_values(&[direction]).observe(jitter_ms);

        // Estimate MOS from loss and jitter (simplified E-model)
        let effective_latency = jitter_ms * 2.0 + 10.0; // simplified
        let r_factor = 93.2 - effective_latency * 0.024 - loss_ratio * 100.0 * 2.5;
        let mos = if r_factor < 0.0 {
            1.0
        } else if r_factor > 100.0 {
            4.5
        } else {
            1.0 + 0.035 * r_factor + r_factor * (r_factor - 60.0) * (100.0 - r_factor) * 7.0e-6
        };
        self.mos_score.with_label_values(&["opus"]).observe(mos);
    }

    /// Update system-level metrics (CPU, memory)
    pub fn update_system_metrics(&self) {
        // Memory: read from /proc/self/status on Linux, or use a fallback
        #[cfg(target_os = "linux")]
        {
            if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
                for line in status.lines() {
                    if line.starts_with("VmRSS:") {
                        if let Some(kb_str) = line.split_whitespace().nth(1) {
                            if let Ok(kb) = kb_str.parse::<i64>() {
                                self.memory_usage.set(kb * 1024);
                            }
                        }
                    }
                }
            }
        }

        // Fallback for non-Linux: report 0
        #[cfg(not(target_os = "linux"))]
        {
            // Memory tracking not available on this platform
        }
    }

    /// Gather all metrics as Prometheus text format
    pub fn gather_text(&self) -> Result<String, String> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .map_err(|e| e.to_string())?;
        String::from_utf8(buffer).map_err(|e| e.to_string())
    }
}

/// Axum handler for /metrics endpoint
pub async fn metrics_handler(metrics: Arc<VoipMetrics>) -> impl IntoResponse {
    match metrics.gather_text() {
        Ok(text) => (
            StatusCode::OK,
            [("content-type", "text/plain; version=0.0.4; charset=utf-8")],
            text,
        )
            .into_response(),
        Err(e) => {
            error!("Failed to gather metrics: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to gather metrics").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let m = VoipMetrics::new();
        m.active_calls.set(5);
        assert_eq!(m.active_calls.get(), 5);

        m.total_calls.inc();
        m.total_calls.inc();
        assert_eq!(m.total_calls.get(), 2);
    }

    #[test]
    fn test_metrics_gather() {
        let m = VoipMetrics::new();
        m.active_calls.set(3);
        m.active_registrations.set(10);
        let text = m.gather_text().unwrap();
        assert!(text.contains("voip_active_calls 3"));
        assert!(text.contains("voip_active_registrations 10"));
    }

    #[test]
    fn test_record_call_end() {
        let m = VoipMetrics::new();
        m.record_call_end(120.0, "outbound", "completed", 0.01, 15.0);
        let text = m.gather_text().unwrap();
        assert!(text.contains("voip_call_duration_seconds"));
        assert!(text.contains("voip_mos_score"));
    }
}
