//! Unit tests for RTP header construction, parsing, RTCP packets,
//! sequence number wrap-around, and SSRC generation.

#[cfg(test)]
mod tests {
    use crate::media::rtp::{RtpHeader, RtpPacket, RtcpSenderReport, RtcpReceiverReport};

    // ═══════════════════════════════════════════════════════════════════
    //  1. RTP header construction
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_rtp_header_default_values() {
        let hdr = RtpHeader::new(0, 0, 160, 0x12345678);
        assert_eq!(hdr.version(), 2, "RTP version must be 2");
        assert!(!hdr.padding());
        assert!(!hdr.extension());
        assert_eq!(hdr.csrc_count(), 0);
        assert!(!hdr.marker());
    }

    #[test]
    fn test_rtp_header_payload_type() {
        // PCMU = PT 0, PCMA = PT 8, Opus dynamic = PT 111
        for pt in [0u8, 8, 111] {
            let hdr = RtpHeader::new(pt, 1, 160, 0xAABBCCDD);
            assert_eq!(hdr.payload_type(), pt);
        }
    }

    #[test]
    fn test_rtp_header_marker_bit() {
        let mut hdr = RtpHeader::new(0, 0, 160, 0x11223344);
        hdr.set_marker(true);
        assert!(hdr.marker());
        hdr.set_marker(false);
        assert!(!hdr.marker());
    }

    // ═══════════════════════════════════════════════════════════════════
    //  2. RTP header serialisation / parsing round-trip
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_rtp_serialize_and_parse() {
        let original = RtpHeader::new(111, 42, 960, 0xDEADBEEF);
        let bytes = original.to_bytes();
        assert!(bytes.len() >= 12, "Minimum RTP header is 12 bytes");

        let parsed = RtpHeader::from_bytes(&bytes).expect("Round-trip must succeed");
        assert_eq!(parsed.version(), 2);
        assert_eq!(parsed.payload_type(), 111);
        assert_eq!(parsed.sequence_number(), 42);
        assert_eq!(parsed.timestamp(), 960);
        assert_eq!(parsed.ssrc(), 0xDEADBEEF);
    }

    #[test]
    fn test_rtp_packet_with_payload() {
        let payload = vec![0x80u8; 160]; // 20 ms of silence in PCMU
        let pkt = RtpPacket::new(0, 1, 160, 0xCAFEBABE, payload.clone());
        let serialized = pkt.to_bytes();

        let parsed = RtpPacket::from_bytes(&serialized).expect("Packet round-trip must succeed");
        assert_eq!(parsed.header().payload_type(), 0);
        assert_eq!(parsed.payload(), payload.as_slice());
    }

    // ═══════════════════════════════════════════════════════════════════
    //  3. RTCP packet creation
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_rtcp_sender_report_creation() {
        let sr = RtcpSenderReport::new(
            0xDEADBEEF,  // SSRC
            100,         // NTP timestamp (high 32 bits)
            200,         // NTP timestamp (low 32 bits)
            3200,        // RTP timestamp
            50,          // sender packet count
            8000,        // sender octet count
        );
        let bytes = sr.to_bytes();
        // RTCP SR header: V=2, PT=200, length in 32-bit words
        assert!(bytes.len() >= 28, "SR must be at least 28 bytes");
        assert_eq!(bytes[0] >> 6, 2, "RTCP version must be 2");
        assert_eq!(bytes[1], 200, "Payload type for SR is 200");
    }

    #[test]
    fn test_rtcp_receiver_report_creation() {
        let rr = RtcpReceiverReport::new(
            0xCAFEBABE,  // reporter SSRC
            0xDEADBEEF,  // source SSRC
            5,           // fraction lost (0-255)
            10,          // cumulative packets lost
            1234,        // highest sequence number received
            15,          // inter-arrival jitter
            0,           // last SR timestamp
            0,           // delay since last SR
        );
        let bytes = rr.to_bytes();
        assert!(bytes.len() >= 32, "RR with one report block must be >= 32 bytes");
        assert_eq!(bytes[1], 201, "Payload type for RR is 201");
    }

    // ═══════════════════════════════════════════════════════════════════
    //  4. Sequence number handling
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_sequence_number_increment() {
        let h1 = RtpHeader::new(0, 65534, 160, 0x1);
        assert_eq!(h1.sequence_number(), 65534);

        let h2 = RtpHeader::new(0, 65535, 320, 0x1);
        assert_eq!(h2.sequence_number(), 65535);

        // Wrap-around: 65535 + 1 = 0
        let h3 = RtpHeader::new(0, 0, 480, 0x1);
        assert_eq!(h3.sequence_number(), 0);
    }

    #[test]
    fn test_sequence_number_wrap_detection() {
        // A utility should detect that seq 0 after 65535 is a wrap, not reorder.
        let before: u16 = 65535;
        let after: u16 = 0;
        let diff = after.wrapping_sub(before);
        assert_eq!(diff, 1, "wrapping_sub must yield 1 for normal wrap");
    }

    // ═══════════════════════════════════════════════════════════════════
    //  5. SSRC generation
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_ssrc_generation_is_random() {
        let ssrc1 = RtpHeader::generate_ssrc();
        let ssrc2 = RtpHeader::generate_ssrc();
        // Collision probability is ~1 in 4 billion; this assertion is safe.
        assert_ne!(ssrc1, ssrc2, "Two generated SSRCs must differ");
    }

    #[test]
    fn test_ssrc_is_32_bit() {
        let ssrc = RtpHeader::generate_ssrc();
        // Ensure it fits in u32 (trivially true by type, but verify no truncation)
        assert!(ssrc <= u32::MAX);
    }
}
