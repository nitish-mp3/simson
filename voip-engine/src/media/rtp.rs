//! RTP / RTCP implementation (RFC 3550).
//!
//! Provides packet parsing, serialization, and a session manager that
//! handles sequence numbering, timestamping, jitter estimation, and
//! RTCP report generation.

use bytes::{BufMut, Bytes, BytesMut};
use parking_lot::Mutex;
use rand::Rng;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU64, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use tracing::{debug, trace, warn};

// ───────────────────────── Payload type constants ─────────────────────────

/// Well-known RTP payload type numbers.
pub struct PayloadType;

impl PayloadType {
    pub const PCMU: u8 = 0;
    pub const PCMA: u8 = 8;
    pub const G722: u8 = 9;
    pub const TELEPHONE_EVENT: u8 = 101;
    pub const OPUS_DYNAMIC: u8 = 111;

    /// Return the clock rate for a given payload type number.
    pub fn clock_rate(pt: u8) -> u32 {
        match pt {
            Self::PCMU | Self::PCMA => 8000,
            Self::G722 => 8000, // G.722 RTP clock rate is 8000 despite 16 kHz codec
            Self::TELEPHONE_EVENT => 8000,
            Self::OPUS_DYNAMIC => 48000,
            _ => 8000,
        }
    }
}

// ───────────────────────── Errors ─────────────────────────

#[derive(Debug, Error)]
pub enum RtpError {
    #[error("Packet too short: {0} bytes")]
    PacketTooShort(usize),
    #[error("Invalid RTP version: {0}")]
    InvalidVersion(u8),
    #[error("Invalid padding")]
    InvalidPadding,
    #[error("Invalid RTCP packet")]
    InvalidRtcp,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

// ───────────────────── RTP Header (RFC 3550 Section 5.1) ─────────────────────
//
//  0                   1                   2                   3
//  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |V=2|P|X|  CC   |M|     PT      |       sequence number         |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |                           timestamp                           |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |           synchronization source (SSRC) identifier            |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
// |            contributing source (CSRC) identifiers             |
// |                             ....                              |
// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

/// Parsed RTP fixed header plus CSRC list.
#[derive(Debug, Clone)]
pub struct RtpHeader {
    /// Protocol version (always 2).
    pub version: u8,
    /// Padding flag.
    pub padding: bool,
    /// Extension header present.
    pub extension: bool,
    /// Number of CSRC identifiers following the fixed header.
    pub csrc_count: u8,
    /// Marker bit.
    pub marker: bool,
    /// Payload type number.
    pub payload_type: u8,
    /// Sequence number (wraps at 65535).
    pub sequence_number: u16,
    /// Sampling-clock timestamp.
    pub timestamp: u32,
    /// Synchronization source identifier.
    pub ssrc: u32,
    /// Contributing source identifiers.
    pub csrc_list: Vec<u32>,
}

/// A complete RTP packet (header + payload).
#[derive(Debug, Clone)]
pub struct RtpPacket {
    pub header: RtpHeader,
    pub payload: Bytes,
}

// ───────────────────── RTCP types ─────────────────────

/// RTCP report block (shared by SR and RR).
#[derive(Debug, Clone)]
pub struct ReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    /// 24-bit cumulative packets lost.
    pub cumulative_lost: u32,
    /// Extended highest sequence number received.
    pub highest_seq: u32,
    /// Interarrival jitter (in timestamp units).
    pub jitter: u32,
    /// Middle 32 bits of last SR NTP timestamp.
    pub last_sr: u32,
    /// Delay since last SR (1/65536 s).
    pub delay_since_sr: u32,
}

/// SDES chunk within an RTCP SDES packet.
#[derive(Debug, Clone)]
pub struct SdesChunk {
    pub ssrc: u32,
    /// (item_type, text) pairs.  CNAME = 1, NAME = 2, etc.
    pub items: Vec<(u8, String)>,
}

/// Parsed RTCP packet.
#[derive(Debug, Clone)]
pub enum RtcpPacket {
    SenderReport {
        ssrc: u32,
        ntp_timestamp: u64,
        rtp_timestamp: u32,
        sender_packet_count: u32,
        sender_octet_count: u32,
        report_blocks: Vec<ReportBlock>,
    },
    ReceiverReport {
        ssrc: u32,
        report_blocks: Vec<ReportBlock>,
    },
    Sdes {
        chunks: Vec<SdesChunk>,
    },
    Bye {
        sources: Vec<u32>,
        reason: Option<String>,
    },
}

// ───────────────────── Free parse functions ─────────────────────

/// Parse a single RTP packet from raw bytes.
pub fn parse_rtp(data: &[u8]) -> Result<RtpPacket, RtpError> {
    if data.len() < 12 {
        return Err(RtpError::PacketTooShort(data.len()));
    }

    let version = (data[0] >> 6) & 0x03;
    if version != 2 {
        return Err(RtpError::InvalidVersion(version));
    }

    let padding = (data[0] >> 5) & 0x01 != 0;
    let extension = (data[0] >> 4) & 0x01 != 0;
    let csrc_count = data[0] & 0x0F;
    let marker = (data[1] >> 7) & 0x01 != 0;
    let payload_type = data[1] & 0x7F;
    let sequence_number = u16::from_be_bytes([data[2], data[3]]);
    let timestamp = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

    let mut offset = 12usize;

    // CSRC list
    let csrc_len = csrc_count as usize * 4;
    if data.len() < offset + csrc_len {
        return Err(RtpError::PacketTooShort(data.len()));
    }
    let mut csrc_list = Vec::with_capacity(csrc_count as usize);
    for _ in 0..csrc_count {
        csrc_list.push(u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]));
        offset += 4;
    }

    // Extension header -- skip over it
    if extension {
        if data.len() < offset + 4 {
            return Err(RtpError::PacketTooShort(data.len()));
        }
        let ext_words = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        offset += 4 + ext_words * 4;
        if offset > data.len() {
            return Err(RtpError::PacketTooShort(data.len()));
        }
    }

    // Payload end (handle padding)
    let payload_end = if padding && !data.is_empty() {
        let pad_len = data[data.len() - 1] as usize;
        if pad_len == 0 || data.len() < offset + pad_len {
            return Err(RtpError::InvalidPadding);
        }
        data.len() - pad_len
    } else {
        data.len()
    };

    let payload = Bytes::copy_from_slice(&data[offset..payload_end]);

    Ok(RtpPacket {
        header: RtpHeader {
            version,
            padding,
            extension,
            csrc_count,
            marker,
            payload_type,
            sequence_number,
            timestamp,
            ssrc,
            csrc_list,
        },
        payload,
    })
}

/// Parse one or more RTCP packets from a compound RTCP datagram.
pub fn parse_rtcp(data: &[u8]) -> Result<Vec<RtcpPacket>, RtpError> {
    let mut packets = Vec::new();
    let mut offset = 0;

    while offset + 4 <= data.len() {
        let rc = data[offset] & 0x1F;
        let pt = data[offset + 1];
        let length_words = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
        let total = (length_words + 1) * 4;

        if offset + total > data.len() {
            break;
        }

        let pkt_data = &data[offset..offset + total];

        match pt {
            200 => {
                // Sender Report
                if let Some(sr) = parse_sr(pkt_data, rc) {
                    packets.push(sr);
                }
            }
            201 => {
                // Receiver Report
                if let Some(rr) = parse_rr(pkt_data, rc) {
                    packets.push(rr);
                }
            }
            202 => {
                // SDES
                if let Some(sdes) = parse_sdes(pkt_data, rc) {
                    packets.push(sdes);
                }
            }
            203 => {
                // BYE
                packets.push(parse_bye(pkt_data, rc));
            }
            _ => {
                trace!(pt, "Unknown RTCP packet type, skipping");
            }
        }

        offset += total;
    }

    if packets.is_empty() && !data.is_empty() {
        return Err(RtpError::InvalidRtcp);
    }
    Ok(packets)
}

// ───────────────────── RTCP sub-parsers ─────────────────────

fn parse_report_block(data: &[u8]) -> ReportBlock {
    ReportBlock {
        ssrc: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
        fraction_lost: data[4],
        cumulative_lost: ((data[5] as u32) << 16) | ((data[6] as u32) << 8) | (data[7] as u32),
        highest_seq: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
        jitter: u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
        last_sr: u32::from_be_bytes([data[16], data[17], data[18], data[19]]),
        delay_since_sr: u32::from_be_bytes([data[20], data[21], data[22], data[23]]),
    }
}

fn parse_sr(data: &[u8], rc: u8) -> Option<RtcpPacket> {
    if data.len() < 28 {
        return None;
    }
    let ssrc = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let ntp_hi = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let ntp_lo = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    let ntp_timestamp = ((ntp_hi as u64) << 32) | (ntp_lo as u64);
    let rtp_timestamp = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
    let sender_packet_count = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
    let sender_octet_count = u32::from_be_bytes([data[24], data[25], data[26], data[27]]);

    let mut blocks = Vec::new();
    let mut off = 28;
    for _ in 0..rc {
        if off + 24 > data.len() {
            break;
        }
        blocks.push(parse_report_block(&data[off..off + 24]));
        off += 24;
    }

    Some(RtcpPacket::SenderReport {
        ssrc,
        ntp_timestamp,
        rtp_timestamp,
        sender_packet_count,
        sender_octet_count,
        report_blocks: blocks,
    })
}

fn parse_rr(data: &[u8], rc: u8) -> Option<RtcpPacket> {
    if data.len() < 8 {
        return None;
    }
    let ssrc = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let mut blocks = Vec::new();
    let mut off = 8;
    for _ in 0..rc {
        if off + 24 > data.len() {
            break;
        }
        blocks.push(parse_report_block(&data[off..off + 24]));
        off += 24;
    }
    Some(RtcpPacket::ReceiverReport {
        ssrc,
        report_blocks: blocks,
    })
}

fn parse_sdes(data: &[u8], sc: u8) -> Option<RtcpPacket> {
    let mut chunks = Vec::new();
    let mut off = 4;

    for _ in 0..sc {
        if off + 4 > data.len() {
            break;
        }
        let ssrc = u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
        off += 4;

        let mut items = Vec::new();
        loop {
            if off >= data.len() {
                break;
            }
            let item_type = data[off];
            if item_type == 0 {
                // End of items; skip to next 32-bit boundary
                off += 1;
                off = (off + 3) & !3;
                break;
            }
            off += 1;
            if off >= data.len() {
                break;
            }
            let len = data[off] as usize;
            off += 1;
            if off + len > data.len() {
                break;
            }
            let text = String::from_utf8_lossy(&data[off..off + len]).to_string();
            items.push((item_type, text));
            off += len;
        }

        chunks.push(SdesChunk { ssrc, items });
    }

    Some(RtcpPacket::Sdes { chunks })
}

fn parse_bye(data: &[u8], sc: u8) -> RtcpPacket {
    let mut sources = Vec::new();
    let mut off = 4;
    for _ in 0..sc {
        if off + 4 > data.len() {
            break;
        }
        sources.push(u32::from_be_bytes([
            data[off],
            data[off + 1],
            data[off + 2],
            data[off + 3],
        ]));
        off += 4;
    }
    let reason = if off < data.len() {
        let len = data[off] as usize;
        if off + 1 + len <= data.len() {
            String::from_utf8(data[off + 1..off + 1 + len].to_vec()).ok()
        } else {
            None
        }
    } else {
        None
    };
    RtcpPacket::Bye { sources, reason }
}

// ───────────────────── Serialization ─────────────────────

impl RtpPacket {
    /// Build a new RTP packet with the given parameters.
    pub fn new(
        payload_type: u8,
        seq: u16,
        timestamp: u32,
        ssrc: u32,
        marker: bool,
        payload: Bytes,
    ) -> Self {
        RtpPacket {
            header: RtpHeader {
                version: 2,
                padding: false,
                extension: false,
                csrc_count: 0,
                marker,
                payload_type,
                sequence_number: seq,
                timestamp,
                ssrc,
                csrc_list: Vec::new(),
            },
            payload,
        }
    }

    /// Serialize to wire format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let h = &self.header;
        let capacity = 12 + h.csrc_list.len() * 4 + self.payload.len();
        let mut buf = BytesMut::with_capacity(capacity);

        let byte0 = (h.version << 6)
            | ((h.padding as u8) << 5)
            | ((h.extension as u8) << 4)
            | (h.csrc_count & 0x0F);
        let byte1 = ((h.marker as u8) << 7) | (h.payload_type & 0x7F);

        buf.put_u8(byte0);
        buf.put_u8(byte1);
        buf.put_u16(h.sequence_number);
        buf.put_u32(h.timestamp);
        buf.put_u32(h.ssrc);

        for c in &h.csrc_list {
            buf.put_u32(*c);
        }

        buf.put_slice(&self.payload);
        buf.to_vec()
    }
}

impl RtcpPacket {
    /// Serialize this RTCP packet to wire format.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            RtcpPacket::SenderReport {
                ssrc,
                ntp_timestamp,
                rtp_timestamp,
                sender_packet_count,
                sender_octet_count,
                report_blocks,
            } => {
                let rc = report_blocks.len() as u8;
                let words = 6 + rc as u16 * 6;
                let mut buf = BytesMut::with_capacity((words as usize + 1) * 4);
                buf.put_u8(0x80 | rc);
                buf.put_u8(200);
                buf.put_u16(words);
                buf.put_u32(*ssrc);
                buf.put_u32((*ntp_timestamp >> 32) as u32);
                buf.put_u32(*ntp_timestamp as u32);
                buf.put_u32(*rtp_timestamp);
                buf.put_u32(*sender_packet_count);
                buf.put_u32(*sender_octet_count);
                for rb in report_blocks {
                    write_report_block(&mut buf, rb);
                }
                buf.to_vec()
            }
            RtcpPacket::ReceiverReport {
                ssrc,
                report_blocks,
            } => {
                let rc = report_blocks.len() as u8;
                let words = 1 + rc as u16 * 6;
                let mut buf = BytesMut::with_capacity((words as usize + 1) * 4);
                buf.put_u8(0x80 | rc);
                buf.put_u8(201);
                buf.put_u16(words);
                buf.put_u32(*ssrc);
                for rb in report_blocks {
                    write_report_block(&mut buf, rb);
                }
                buf.to_vec()
            }
            RtcpPacket::Sdes { chunks } => {
                let sc = chunks.len() as u8;
                let mut body = BytesMut::new();
                for chunk in chunks {
                    body.put_u32(chunk.ssrc);
                    for (item_type, text) in &chunk.items {
                        body.put_u8(*item_type);
                        let text_bytes = text.as_bytes();
                        body.put_u8(text_bytes.len() as u8);
                        body.put_slice(text_bytes);
                    }
                    body.put_u8(0); // end of items
                    // Pad to 32-bit boundary
                    while body.len() % 4 != 0 {
                        body.put_u8(0);
                    }
                }
                let words = (body.len() / 4) as u16;
                let mut buf = BytesMut::with_capacity(4 + body.len());
                buf.put_u8(0x80 | sc);
                buf.put_u8(202);
                buf.put_u16(words);
                buf.put_slice(&body);
                buf.to_vec()
            }
            RtcpPacket::Bye { sources, reason } => {
                let sc = sources.len() as u8;
                let reason_bytes = reason.as_ref().map(|r| r.as_bytes().to_vec());
                let reason_words = reason_bytes
                    .as_ref()
                    .map(|r| (1 + r.len() + 3) / 4)
                    .unwrap_or(0);
                let words = (sources.len() + reason_words) as u16;
                let mut buf = BytesMut::with_capacity((words as usize + 1) * 4);
                buf.put_u8(0x80 | sc);
                buf.put_u8(203);
                buf.put_u16(words);
                for s in sources {
                    buf.put_u32(*s);
                }
                if let Some(rb) = &reason_bytes {
                    buf.put_u8(rb.len() as u8);
                    buf.put_slice(rb);
                    let pad = reason_words * 4 - 1 - rb.len();
                    for _ in 0..pad {
                        buf.put_u8(0);
                    }
                }
                buf.to_vec()
            }
        }
    }
}

fn write_report_block(buf: &mut BytesMut, rb: &ReportBlock) {
    buf.put_u32(rb.ssrc);
    buf.put_u8(rb.fraction_lost);
    buf.put_u8((rb.cumulative_lost >> 16) as u8);
    buf.put_u8((rb.cumulative_lost >> 8) as u8);
    buf.put_u8(rb.cumulative_lost as u8);
    buf.put_u32(rb.highest_seq);
    buf.put_u32(rb.jitter);
    buf.put_u32(rb.last_sr);
    buf.put_u32(rb.delay_since_sr);
}

// ───────────────────── Sequence number helpers ─────────────────────

/// Compare two 16-bit sequence numbers with wrap-around awareness.
/// Returns positive if `a` is ahead of `b`, negative if behind, 0 if equal.
pub fn seq_compare(a: u16, b: u16) -> i16 {
    a.wrapping_sub(b) as i16
}

// ───────────────────── RTP Session Statistics ─────────────────────

/// Per-session reception statistics used for RTCP report generation.
#[derive(Debug, Default)]
pub struct RtpStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub packets_lost: u64,
    /// Interarrival jitter estimate (RFC 3550 A.8), in seconds.
    pub jitter: f64,
    pub last_seq: u16,
    pub max_seq: u16,
    pub base_seq: u16,
    /// Sequence number wrap-around (cycle) count.
    pub seq_cycles: u32,
    /// Previous transit time for jitter computation.
    last_transit: Option<f64>,
    /// Previous packet arrival instant.
    last_arrival: Option<Instant>,
}

impl RtpStats {
    /// Update jitter estimate per RFC 3550 Section 6.4.1.
    fn update_jitter(&mut self, transit_diff: f64) {
        self.jitter += (transit_diff.abs() - self.jitter) / 16.0;
    }

    /// Extended highest sequence number received.
    pub fn extended_max_seq(&self) -> u32 {
        (self.seq_cycles as u32) * 65536 + self.max_seq as u32
    }

    /// Cumulative packet loss.
    pub fn cumulative_loss(&self) -> u32 {
        let expected = self.extended_max_seq().wrapping_sub(self.base_seq as u32) + 1;
        let received = self.packets_received as u32;
        expected.saturating_sub(received)
    }

    /// Fraction lost since previous interval.
    pub fn fraction_lost(&self, prev_expected: u32, prev_received: u32) -> u8 {
        let expected = self.extended_max_seq().wrapping_sub(self.base_seq as u32) + 1;
        let received = self.packets_received as u32;
        let exp_interval = expected.wrapping_sub(prev_expected);
        let rcv_interval = received.wrapping_sub(prev_received);
        if exp_interval == 0 || rcv_interval >= exp_interval {
            0
        } else {
            let lost = exp_interval - rcv_interval;
            ((lost * 256) / exp_interval) as u8
        }
    }
}

// ───────────────────── RTP Session ─────────────────────

/// An RTP session manages packet creation and reception statistics for one
/// media stream.  The atomic counters allow `send_rtp` to be called from
/// shared references (`&self`) without external locking.
pub struct RtpSession {
    pub local_ssrc: u32,
    pub remote_ssrc: Mutex<Option<u32>>,
    /// Next outgoing sequence number (atomic for lock-free send path).
    pub seq_number: AtomicU16,
    /// Next outgoing timestamp (atomic for lock-free send path).
    pub timestamp: AtomicU32,
    /// Packets sent counter.
    pub packet_count: AtomicU64,
    /// Octets sent counter.
    pub octet_count: AtomicU64,
    /// Reception statistics (behind a mutex since jitter computation
    /// requires mutable floating-point state).
    stats: Mutex<RtpStats>,
    /// Clock rate of the negotiated codec.
    pub clock_rate: u32,
}

impl RtpSession {
    /// Create a new RTP session with a random SSRC and initial sequence number.
    pub fn new(clock_rate: u32) -> Self {
        let mut rng = rand::thread_rng();
        RtpSession {
            local_ssrc: rng.gen(),
            remote_ssrc: Mutex::new(None),
            seq_number: AtomicU16::new(rng.gen::<u16>() & 0x7FFF),
            timestamp: AtomicU32::new(rng.gen()),
            packet_count: AtomicU64::new(0),
            octet_count: AtomicU64::new(0),
            stats: Mutex::new(RtpStats::default()),
            clock_rate,
        }
    }

    /// Build and return the next outgoing RTP packet.
    ///
    /// Atomically increments the sequence number and advances the timestamp
    /// by `samples` units of the codec clock.
    pub fn send_rtp(&self, payload: Bytes, payload_type: u8, marker: bool) -> RtpPacket {
        let seq = self.seq_number.fetch_add(1, Ordering::Relaxed);
        let ts = self.timestamp.load(Ordering::Relaxed);

        self.packet_count.fetch_add(1, Ordering::Relaxed);
        self.octet_count
            .fetch_add(payload.len() as u64, Ordering::Relaxed);

        trace!(seq, ts, pt = payload_type, marker, "TX RTP");

        RtpPacket::new(payload_type, seq, ts, self.local_ssrc, marker, payload)
    }

    /// Advance the outgoing timestamp by the given number of samples.
    pub fn advance_timestamp(&self, samples: u32) {
        self.timestamp.fetch_add(samples, Ordering::Relaxed);
    }

    /// Process a received RTP packet: extract the payload and update stats.
    pub fn recv_rtp(&self, packet: &RtpPacket) -> Result<Bytes, RtpError> {
        self.update_stats(packet);
        Ok(packet.payload.clone())
    }

    /// Update reception statistics from an incoming packet.
    pub fn update_stats(&self, packet: &RtpPacket) {
        let mut stats = self.stats.lock();
        let seq = packet.header.sequence_number;

        // Learn remote SSRC on first packet
        {
            let mut remote = self.remote_ssrc.lock();
            if remote.is_none() {
                *remote = Some(packet.header.ssrc);
                stats.base_seq = seq;
                stats.max_seq = seq;
                debug!(ssrc = packet.header.ssrc, "Learned remote SSRC");
            }
        }

        stats.packets_received += 1;
        stats.bytes_received += packet.payload.len() as u64;

        // Detect sequence number wrap-around
        if seq < stats.max_seq && stats.max_seq.wrapping_sub(seq) > 0x8000 {
            stats.seq_cycles += 1;
            debug!(cycles = stats.seq_cycles, "Sequence number wrap-around");
        }
        if seq_compare(seq, stats.max_seq) > 0 {
            stats.max_seq = seq;
        }
        stats.last_seq = seq;

        // Jitter calculation (RFC 3550 A.8)
        let now = Instant::now();
        if let Some(last_arrival) = stats.last_arrival {
            let arrival_diff = now.duration_since(last_arrival).as_secs_f64();
            let rtp_diff = packet
                .header
                .timestamp
                .wrapping_sub(self.timestamp.load(Ordering::Relaxed)) as f64
                / self.clock_rate as f64;
            let transit = arrival_diff - rtp_diff;
            if let Some(last_transit) = stats.last_transit {
                let d = transit - last_transit;
                stats.update_jitter(d);
            }
            stats.last_transit = Some(transit);
        }
        stats.last_arrival = Some(now);
    }

    /// Generate a Sender Report RTCP packet.
    pub fn generate_sr(&self) -> RtcpPacket {
        let stats = self.stats.lock();
        let remote_ssrc = *self.remote_ssrc.lock();

        let report_blocks = if let Some(rssrc) = remote_ssrc {
            vec![ReportBlock {
                ssrc: rssrc,
                fraction_lost: 0,
                cumulative_lost: stats.cumulative_loss(),
                highest_seq: stats.extended_max_seq(),
                jitter: (stats.jitter * self.clock_rate as f64) as u32,
                last_sr: 0,
                delay_since_sr: 0,
            }]
        } else {
            Vec::new()
        };

        RtcpPacket::SenderReport {
            ssrc: self.local_ssrc,
            ntp_timestamp: Self::ntp_now(),
            rtp_timestamp: self.timestamp.load(Ordering::Relaxed),
            sender_packet_count: self.packet_count.load(Ordering::Relaxed) as u32,
            sender_octet_count: self.octet_count.load(Ordering::Relaxed) as u32,
            report_blocks,
        }
    }

    /// Generate a Receiver Report RTCP packet.
    pub fn generate_rr(&self) -> RtcpPacket {
        let stats = self.stats.lock();
        let remote_ssrc = *self.remote_ssrc.lock();

        let report_blocks = if let Some(rssrc) = remote_ssrc {
            vec![ReportBlock {
                ssrc: rssrc,
                fraction_lost: 0,
                cumulative_lost: stats.cumulative_loss(),
                highest_seq: stats.extended_max_seq(),
                jitter: (stats.jitter * self.clock_rate as f64) as u32,
                last_sr: 0,
                delay_since_sr: 0,
            }]
        } else {
            Vec::new()
        };

        RtcpPacket::ReceiverReport {
            ssrc: self.local_ssrc,
            report_blocks,
        }
    }

    /// Current NTP timestamp (RFC 3550 wallclock).
    fn ntp_now() -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        // NTP epoch is 1900-01-01; offset from Unix epoch is 70 years.
        let ntp_secs = now.as_secs() + 2_208_988_800;
        let frac = ((now.subsec_nanos() as u64) << 32) / 1_000_000_000;
        (ntp_secs << 32) | frac
    }

    /// Read a snapshot of the reception statistics.
    pub fn get_stats(&self) -> RtpStats {
        let s = self.stats.lock();
        RtpStats {
            packets_sent: self.packet_count.load(Ordering::Relaxed),
            packets_received: s.packets_received,
            bytes_sent: self.octet_count.load(Ordering::Relaxed),
            bytes_received: s.bytes_received,
            packets_lost: s.packets_lost,
            jitter: s.jitter,
            last_seq: s.last_seq,
            max_seq: s.max_seq,
            base_seq: s.base_seq,
            seq_cycles: s.seq_cycles,
            last_transit: None,
            last_arrival: None,
        }
    }
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rtp_roundtrip() {
        let original = RtpPacket::new(0, 1234, 5678, 0xDEADBEEF, true, Bytes::from_static(b"hello"));
        let data = original.to_bytes();
        let parsed = parse_rtp(&data).expect("parse failed");

        assert_eq!(parsed.header.version, 2);
        assert_eq!(parsed.header.payload_type, 0);
        assert_eq!(parsed.header.sequence_number, 1234);
        assert_eq!(parsed.header.timestamp, 5678);
        assert_eq!(parsed.header.ssrc, 0xDEADBEEF);
        assert!(parsed.header.marker);
        assert_eq!(parsed.payload, Bytes::from_static(b"hello"));
    }

    #[test]
    fn rtp_too_short() {
        assert!(parse_rtp(&[0u8; 5]).is_err());
    }

    #[test]
    fn rtcp_sr_roundtrip() {
        let sr = RtcpPacket::SenderReport {
            ssrc: 12345,
            ntp_timestamp: 0x0102030405060708,
            rtp_timestamp: 9999,
            sender_packet_count: 100,
            sender_octet_count: 16000,
            report_blocks: Vec::new(),
        };
        let data = sr.to_bytes();
        let packets = parse_rtcp(&data).expect("parse failed");
        assert_eq!(packets.len(), 1);
        match &packets[0] {
            RtcpPacket::SenderReport { ssrc, sender_packet_count, .. } => {
                assert_eq!(*ssrc, 12345);
                assert_eq!(*sender_packet_count, 100);
            }
            _ => panic!("Expected SenderReport"),
        }
    }

    #[test]
    fn rtcp_bye_roundtrip() {
        let bye = RtcpPacket::Bye {
            sources: vec![111, 222],
            reason: Some("leaving".into()),
        };
        let data = bye.to_bytes();
        let packets = parse_rtcp(&data).expect("parse failed");
        assert_eq!(packets.len(), 1);
        match &packets[0] {
            RtcpPacket::Bye { sources, reason } => {
                assert_eq!(sources, &[111, 222]);
                assert_eq!(reason.as_deref(), Some("leaving"));
            }
            _ => panic!("Expected Bye"),
        }
    }

    #[test]
    fn seq_compare_wraps() {
        assert!(seq_compare(0, 65535) > 0); // 0 is "ahead" of 65535
        assert!(seq_compare(65535, 0) < 0);
        assert_eq!(seq_compare(100, 100), 0);
        assert!(seq_compare(200, 100) > 0);
    }

    #[test]
    fn session_send_increments_seq() {
        let session = RtpSession::new(8000);
        let p1 = session.send_rtp(Bytes::from_static(b"a"), PayloadType::PCMU, false);
        let p2 = session.send_rtp(Bytes::from_static(b"b"), PayloadType::PCMU, false);
        assert_eq!(
            p2.header.sequence_number,
            p1.header.sequence_number.wrapping_add(1)
        );
        assert_eq!(session.packet_count.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn session_generate_sr() {
        let session = RtpSession::new(8000);
        let _p = session.send_rtp(Bytes::from(vec![0u8; 160]), PayloadType::PCMU, false);
        let sr = session.generate_sr();
        match sr {
            RtcpPacket::SenderReport { sender_packet_count, .. } => {
                assert_eq!(sender_packet_count, 1);
            }
            _ => panic!("Expected SenderReport"),
        }
    }
}
