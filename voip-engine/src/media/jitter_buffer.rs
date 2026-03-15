//! Adaptive jitter buffer for RTP media streams.
//!
//! Packets are inserted by RTP sequence number and played out in order
//! after an adaptive delay that tracks the measured network jitter using
//! an exponential moving average.  Missing packets trigger packet loss
//! concealment (silence insertion or repetition of the previous frame).

use bytes::Bytes;
use rand::Rng;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

use super::rtp::{seq_compare, RtpPacket};

// ───────────────────── Statistics ─────────────────────

/// Statistics collected by the jitter buffer.
#[derive(Debug, Clone, Default)]
pub struct JitterStats {
    pub packets_received: u64,
    pub packets_lost: u64,
    pub packets_late: u64,
    pub packets_reordered: u64,
    pub current_jitter_ms: f64,
    pub avg_jitter_ms: f64,
    pub loss_rate: f64,
}

// ───────────────────── Buffer entry ─────────────────────

/// An entry stored in the jitter buffer.
#[derive(Debug, Clone)]
pub struct JitterBufferEntry {
    pub packet: RtpPacket,
    pub received_at: Instant,
    pub expected_at: Instant,
}

// ───────────────────── Jitter buffer ─────────────────────

/// Adaptive jitter buffer.
///
/// Packets are keyed by their 16-bit RTP sequence number inside a
/// `BTreeMap`.  The buffer adapts its target playout delay based on
/// an exponentially weighted moving average of the measured
/// interarrival jitter.
pub struct JitterBuffer {
    /// Ordered map of sequence number -> buffered entry.
    buffer: BTreeMap<u16, JitterBufferEntry>,

    // ── Delay bounds ──
    target_delay_ms: f64,
    min_delay_ms: f64,
    max_delay_ms: f64,
    current_delay_ms: f64,

    // ── Playout state ──
    last_played_seq: Option<u16>,
    last_played_timestamp: Option<u32>,
    last_playout_time: Option<Instant>,

    // ── Jitter estimation ──
    jitter_estimate_ms: f64,
    prev_arrival: Option<Instant>,
    prev_rtp_ts: Option<u32>,

    // ── Codec parameters ──
    clock_rate: u32,
    packet_interval_ms: f64,

    // ── Concealment state ──
    last_good_payload: Option<Bytes>,

    // ── Book-keeping ──
    stats: JitterStats,
    max_buffer_size: usize,
    /// Circular buffer of recently seen sequence numbers for dup detection.
    seen_seqs: Vec<u16>,
    seen_capacity: usize,
    /// Cumulative jitter sum for average computation.
    jitter_sum: f64,
    jitter_count: u64,
    /// When the buffer was created / first packet arrived.
    first_arrival: Option<Instant>,
}

impl JitterBuffer {
    /// Create a new adaptive jitter buffer.
    ///
    /// * `min_delay_ms` / `max_delay_ms` -- bounds for the adaptive delay.
    /// * `clock_rate` -- codec sampling rate (e.g. 8000 for G.711).
    /// * `packet_interval_ms` -- nominal inter-packet interval (e.g. 20 ms).
    pub fn new(
        min_delay_ms: f64,
        max_delay_ms: f64,
        clock_rate: u32,
        packet_interval_ms: f64,
    ) -> Self {
        let initial_delay = (min_delay_ms * 2.0).min(max_delay_ms);
        JitterBuffer {
            buffer: BTreeMap::new(),
            target_delay_ms: initial_delay,
            min_delay_ms,
            max_delay_ms,
            current_delay_ms: initial_delay,
            last_played_seq: None,
            last_played_timestamp: None,
            last_playout_time: None,
            jitter_estimate_ms: 0.0,
            prev_arrival: None,
            prev_rtp_ts: None,
            clock_rate,
            packet_interval_ms,
            last_good_payload: None,
            stats: JitterStats::default(),
            max_buffer_size: ((max_delay_ms / packet_interval_ms) * 2.0) as usize + 64,
            seen_seqs: Vec::with_capacity(512),
            seen_capacity: 512,
            jitter_sum: 0.0,
            jitter_count: 0,
            first_arrival: None,
        }
    }

    // ───────────── Public API ─────────────

    /// Insert a received RTP packet into the buffer.
    ///
    /// Returns `true` if the packet was accepted, `false` if it was a
    /// duplicate or arrived too late.
    pub fn insert(&mut self, packet: RtpPacket) -> bool {
        let seq = packet.header.sequence_number;
        let now = Instant::now();
        self.stats.packets_received += 1;

        // Duplicate detection
        if self.seen_seqs.contains(&seq) {
            trace!(seq, "Duplicate packet dropped");
            return false;
        }
        if self.seen_seqs.len() >= self.seen_capacity {
            self.seen_seqs.remove(0);
        }
        self.seen_seqs.push(seq);

        // First packet initialisation
        if self.first_arrival.is_none() {
            self.first_arrival = Some(now);
            self.last_played_seq = Some(seq);
        }

        // Detect late arrivals (significantly behind the playout head)
        if let Some(next) = self.last_played_seq {
            let diff = seq_compare(seq, next);
            if diff < -10 {
                self.stats.packets_late += 1;
                trace!(seq, next, "Late packet dropped");
                return false;
            }
            if diff < 0 && diff > -10 {
                self.stats.packets_reordered += 1;
                trace!(seq, "Reordered packet accepted");
            }
        }

        // Jitter estimation (RFC 3550)
        if let (Some(prev_arrival), Some(prev_ts)) = (self.prev_arrival, self.prev_rtp_ts) {
            let arrival_diff_ms = now.duration_since(prev_arrival).as_secs_f64() * 1000.0;
            let rtp_diff_ms = packet
                .header
                .timestamp
                .wrapping_sub(prev_ts) as f64
                / self.clock_rate as f64
                * 1000.0;
            let transit_diff = (arrival_diff_ms - rtp_diff_ms).abs();
            // Exponential moving average (alpha = 1/16)
            self.jitter_estimate_ms += (transit_diff - self.jitter_estimate_ms) / 16.0;
            self.jitter_sum += self.jitter_estimate_ms;
            self.jitter_count += 1;
        }
        self.prev_arrival = Some(now);
        self.prev_rtp_ts = Some(packet.header.timestamp);

        // Adapt delay
        self.adapt_delay();

        // Compute expected playout time
        let expected_at = now + Duration::from_secs_f64(self.current_delay_ms / 1000.0);

        // Buffer overflow protection
        if self.buffer.len() >= self.max_buffer_size {
            if let Some(&oldest) = self.buffer.keys().next() {
                self.buffer.remove(&oldest);
                warn!(seq = oldest, "Buffer overflow, dropped oldest");
            }
        }

        self.buffer.insert(
            seq,
            JitterBufferEntry {
                packet,
                received_at: now,
                expected_at,
            },
        );

        true
    }

    /// Retrieve the next packet ready for playout.
    ///
    /// Returns `Some(packet)` when the next expected sequence number is
    /// available and its playout time has arrived.  If the packet is
    /// missing but later packets exist, the buffer advances the playout
    /// head and returns `None` (the caller should use `handle_loss` for
    /// concealment).
    pub fn next_packet(&mut self) -> Option<RtpPacket> {
        let next_seq = self.last_played_seq?;
        let now = Instant::now();

        if let Some(entry) = self.buffer.get(&next_seq) {
            if now >= entry.expected_at {
                let entry = self.buffer.remove(&next_seq).unwrap();
                self.last_played_seq = Some(next_seq.wrapping_add(1));
                self.last_played_timestamp = Some(entry.packet.header.timestamp);
                self.last_playout_time = Some(now);
                self.last_good_payload = Some(entry.packet.payload.clone());
                return Some(entry.packet);
            }
            // Not ready yet
            return None;
        }

        // Packet is missing.  If a later packet exists and enough time has
        // passed, skip ahead.
        let has_later = self
            .buffer
            .keys()
            .any(|&s| seq_compare(s, next_seq) > 0);

        if has_later {
            let deadline = self
                .last_playout_time
                .unwrap_or(now)
                + Duration::from_secs_f64(self.packet_interval_ms / 1000.0);

            if now >= deadline {
                self.stats.packets_lost += 1;
                self.last_played_seq = Some(next_seq.wrapping_add(1));
                self.last_playout_time = Some(now);
                // Caller should call handle_loss(seq) for concealment data
            }
        }

        None
    }

    /// Adapt the target playout delay based on measured jitter.
    pub fn adapt_delay(&mut self) {
        // Target = 2x jitter + one packet interval as safety margin
        let desired = self.jitter_estimate_ms * 2.0 + self.packet_interval_ms;
        let clamped = desired.clamp(self.min_delay_ms, self.max_delay_ms);

        // Smoothly move 10 % toward the desired value each adaptation step
        self.current_delay_ms += (clamped - self.current_delay_ms) * 0.1;
        self.target_delay_ms = self.current_delay_ms;

        self.stats.current_jitter_ms = self.jitter_estimate_ms;
        if self.jitter_count > 0 {
            self.stats.avg_jitter_ms = self.jitter_sum / self.jitter_count as f64;
        }
    }

    /// Return a copy of the current statistics.
    pub fn get_stats(&self) -> JitterStats {
        let mut s = self.stats.clone();
        if s.packets_received > 0 {
            s.loss_rate = s.packets_lost as f64 / s.packets_received as f64;
        }
        s
    }

    /// Generate concealment data for a missing packet.
    ///
    /// Strategy:
    /// 1. If a previous good payload exists, repeat it with 6 dB attenuation.
    /// 2. Otherwise generate comfort noise (low-level random samples).
    pub fn handle_loss(&self, _seq: u16) -> Vec<u8> {
        if let Some(ref last) = self.last_good_payload {
            // Repeat last frame with ~6 dB attenuation (halve amplitudes).
            let mut data = last.to_vec();
            for sample in &mut data {
                *sample = (*sample as i8 / 2) as u8;
            }
            data
        } else {
            // Comfort noise: frame-sized buffer of near-silence.
            let frame_size =
                (self.clock_rate as f64 * self.packet_interval_ms / 1000.0) as usize;
            let mut data = vec![0u8; frame_size];
            let mut rng = rand::thread_rng();
            for sample in &mut data {
                // mu-law midpoint is 0xFF / 0x7F; add tiny random noise.
                *sample = 0x7F_u8.wrapping_add(rng.gen_range(0..3));
            }
            data
        }
    }

    /// Flush the entire buffer (e.g. on codec change or call end).
    pub fn flush(&mut self) {
        self.buffer.clear();
        self.last_played_seq = None;
        self.last_played_timestamp = None;
        self.last_playout_time = None;
        self.first_arrival = None;
        self.prev_arrival = None;
        self.prev_rtp_ts = None;
        self.last_good_payload = None;
        self.seen_seqs.clear();
        self.stats = JitterStats::default();
        self.jitter_estimate_ms = 0.0;
        self.jitter_sum = 0.0;
        self.jitter_count = 0;
        debug!("Jitter buffer flushed");
    }

    /// Update the minimum / maximum delay bounds at runtime.
    pub fn set_delay_bounds(&mut self, min_ms: f64, max_ms: f64) {
        self.min_delay_ms = min_ms;
        self.max_delay_ms = max_ms;
        // Re-clamp the current delay
        self.current_delay_ms = self.current_delay_ms.clamp(min_ms, max_ms);
        self.target_delay_ms = self.current_delay_ms;
        // Update max buffer size
        self.max_buffer_size = ((max_ms / self.packet_interval_ms) * 2.0) as usize + 64;
        debug!(min_ms, max_ms, "Delay bounds updated");
    }

    /// Current buffer depth in packets.
    pub fn depth(&self) -> usize {
        self.buffer.len()
    }

    /// Current buffer depth in milliseconds.
    pub fn depth_ms(&self) -> f64 {
        self.buffer.len() as f64 * self.packet_interval_ms
    }

    /// Current target playout delay in milliseconds.
    pub fn target_delay_ms(&self) -> f64 {
        self.target_delay_ms
    }
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pkt(seq: u16, ts: u32) -> RtpPacket {
        RtpPacket::new(0, seq, ts, 12345, false, Bytes::from(vec![0x80u8; 160]))
    }

    #[test]
    fn insert_and_stats() {
        let mut jb = JitterBuffer::new(20.0, 200.0, 8000, 20.0);
        for i in 0..5u16 {
            assert!(jb.insert(make_pkt(i, i as u32 * 160)));
        }
        assert_eq!(jb.get_stats().packets_received, 5);
        assert_eq!(jb.depth(), 5);
    }

    #[test]
    fn duplicate_rejected() {
        let mut jb = JitterBuffer::new(20.0, 200.0, 8000, 20.0);
        let pkt = make_pkt(100, 16000);
        assert!(jb.insert(pkt.clone()));
        assert!(!jb.insert(pkt));
    }

    #[test]
    fn reorder_tracked() {
        let mut jb = JitterBuffer::new(20.0, 200.0, 8000, 20.0);
        jb.insert(make_pkt(0, 0));
        jb.insert(make_pkt(2, 320));
        jb.insert(make_pkt(1, 160));
        assert_eq!(jb.get_stats().packets_reordered, 1);
    }

    #[test]
    fn flush_clears() {
        let mut jb = JitterBuffer::new(20.0, 200.0, 8000, 20.0);
        for i in 0..10u16 {
            jb.insert(make_pkt(i, i as u32 * 160));
        }
        assert_eq!(jb.depth(), 10);
        jb.flush();
        assert_eq!(jb.depth(), 0);
    }

    #[test]
    fn set_delay_bounds_clamps() {
        let mut jb = JitterBuffer::new(10.0, 500.0, 8000, 20.0);
        jb.current_delay_ms = 300.0;
        jb.set_delay_bounds(50.0, 100.0);
        assert!(jb.current_delay_ms <= 100.0);
    }

    #[test]
    fn handle_loss_generates_data() {
        let jb = JitterBuffer::new(20.0, 200.0, 8000, 20.0);
        let data = jb.handle_loss(5);
        // Comfort noise: should be frame_size = 8000 * 0.02 = 160 bytes
        assert_eq!(data.len(), 160);
    }

    #[test]
    fn handle_loss_repeats_last() {
        let mut jb = JitterBuffer::new(20.0, 200.0, 8000, 20.0);
        jb.last_good_payload = Some(Bytes::from(vec![0x40u8; 160]));
        let data = jb.handle_loss(5);
        assert_eq!(data.len(), 160);
        // Attenuated: 0x40 as i8 = 64, /2 = 32 = 0x20
        assert_eq!(data[0], 0x20);
    }
}
