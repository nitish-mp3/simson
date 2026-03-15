//! Unit tests for the SIP message parser.
//!
//! Covers request parsing (INVITE, REGISTER, BYE, ACK, CANCEL, OPTIONS),
//! response parsing (200 OK, 180 Ringing, 401 Unauthorized), header
//! extraction, SDP body handling, URI parsing, authentication digest
//! headers, and malformed-message edge cases.

#[cfg(test)]
mod tests {
    use crate::sip::parser::{
        parse_sip_message, SipHeader, SipMessage, SipMethod, SipRequest, SipResponse,
    };

    // ───────────────────────── Helper builders ─────────────────────────

    fn invite_request() -> String {
        concat!(
            "INVITE sip:bob@biloxi.example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bK776asdhds\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@biloxi.example.com>\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n",
            "CSeq: 314159 INVITE\r\n",
            "Contact: <sip:alice@pc33.atlanta.example.com>\r\n",
            "Content-Type: application/sdp\r\n",
            "Content-Length: 147\r\n",
            "\r\n",
            "v=0\r\n",
            "o=alice 53655765 2353687637 IN IP4 pc33.atlanta.example.com\r\n",
            "s=-\r\n",
            "c=IN IP4 pc33.atlanta.example.com\r\n",
            "t=0 0\r\n",
            "m=audio 3456 RTP/AVP 0 8 97\r\n",
            "a=rtpmap:97 opus/48000/2\r\n",
        )
        .to_string()
    }

    fn register_request() -> String {
        concat!(
            "REGISTER sip:registrar.biloxi.example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP bobspc.biloxi.example.com:5060;branch=z9hG4bKnashds7\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@biloxi.example.com>\r\n",
            "From: Bob <sip:bob@biloxi.example.com>;tag=456248\r\n",
            "Call-ID: 843817637684230@998sdasdh09\r\n",
            "CSeq: 1826 REGISTER\r\n",
            "Contact: <sip:bob@192.0.2.4>\r\n",
            "Expires: 7200\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    fn bye_request() -> String {
        concat!(
            "BYE sip:bob@192.0.2.4 SIP/2.0\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bKnashds8\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@biloxi.example.com>;tag=a6c85cf\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n",
            "CSeq: 231 BYE\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    fn ack_request() -> String {
        concat!(
            "ACK sip:bob@192.0.2.4 SIP/2.0\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bKnashds9\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@biloxi.example.com>;tag=a6c85cf\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n",
            "CSeq: 314159 ACK\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    fn cancel_request() -> String {
        concat!(
            "CANCEL sip:bob@biloxi.example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bK776asdhds\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@biloxi.example.com>\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n",
            "CSeq: 314159 CANCEL\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    fn options_request() -> String {
        concat!(
            "OPTIONS sip:carol@chicago.example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bKhjhs8ass877\r\n",
            "Max-Forwards: 70\r\n",
            "To: <sip:carol@chicago.example.com>\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: opts-8234987234@pc33.atlanta.example.com\r\n",
            "CSeq: 63104 OPTIONS\r\n",
            "Contact: <sip:alice@pc33.atlanta.example.com>\r\n",
            "Accept: application/sdp\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    fn response_200_ok() -> String {
        concat!(
            "SIP/2.0 200 OK\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bK776asdhds;received=192.0.2.1\r\n",
            "To: Bob <sip:bob@biloxi.example.com>;tag=a6c85cf\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n",
            "CSeq: 314159 INVITE\r\n",
            "Contact: <sip:bob@192.0.2.4>\r\n",
            "Content-Type: application/sdp\r\n",
            "Content-Length: 131\r\n",
            "\r\n",
            "v=0\r\n",
            "o=bob 2890844527 2890844527 IN IP4 192.0.2.4\r\n",
            "s=-\r\n",
            "c=IN IP4 192.0.2.4\r\n",
            "t=0 0\r\n",
            "m=audio 49172 RTP/AVP 0\r\n",
            "a=rtpmap:0 PCMU/8000\r\n",
        )
        .to_string()
    }

    fn response_180_ringing() -> String {
        concat!(
            "SIP/2.0 180 Ringing\r\n",
            "Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bK776asdhds;received=192.0.2.1\r\n",
            "To: Bob <sip:bob@biloxi.example.com>;tag=a6c85cf\r\n",
            "From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n",
            "Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n",
            "CSeq: 314159 INVITE\r\n",
            "Contact: <sip:bob@192.0.2.4>\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    fn response_401_unauthorized() -> String {
        concat!(
            "SIP/2.0 401 Unauthorized\r\n",
            "Via: SIP/2.0/UDP bobspc.biloxi.example.com:5060;branch=z9hG4bKnashds7;received=192.0.2.4\r\n",
            "To: Bob <sip:bob@biloxi.example.com>;tag=2493k59kd\r\n",
            "From: Bob <sip:bob@biloxi.example.com>;tag=456248\r\n",
            "Call-ID: 843817637684230@998sdasdh09\r\n",
            "CSeq: 1826 REGISTER\r\n",
            "WWW-Authenticate: Digest realm=\"biloxi.example.com\", ",
            "qop=\"auth\", ",
            "nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\", ",
            "opaque=\"5ccc069c403ebaf9f0171e9517f40e41\"\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        )
        .to_string()
    }

    // ═══════════════════════════════════════════════════════════════════
    //  1. Request parsing
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_parse_invite_request() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).expect("INVITE should parse");
        match msg {
            SipMessage::Request(req) => {
                assert_eq!(req.method, SipMethod::Invite);
                assert_eq!(req.uri, "sip:bob@biloxi.example.com");
                assert_eq!(req.version, "SIP/2.0");
            }
            _ => panic!("Expected SipRequest, got SipResponse"),
        }
    }

    #[test]
    fn test_parse_register_request() {
        let raw = register_request();
        let msg = parse_sip_message(raw.as_bytes()).expect("REGISTER should parse");
        match msg {
            SipMessage::Request(req) => {
                assert_eq!(req.method, SipMethod::Register);
                assert_eq!(req.uri, "sip:registrar.biloxi.example.com");
            }
            _ => panic!("Expected SipRequest"),
        }
    }

    #[test]
    fn test_parse_bye_request() {
        let raw = bye_request();
        let msg = parse_sip_message(raw.as_bytes()).expect("BYE should parse");
        match msg {
            SipMessage::Request(req) => {
                assert_eq!(req.method, SipMethod::Bye);
                assert_eq!(req.uri, "sip:bob@192.0.2.4");
            }
            _ => panic!("Expected SipRequest"),
        }
    }

    #[test]
    fn test_parse_ack_request() {
        let raw = ack_request();
        let msg = parse_sip_message(raw.as_bytes()).expect("ACK should parse");
        match msg {
            SipMessage::Request(req) => {
                assert_eq!(req.method, SipMethod::Ack);
            }
            _ => panic!("Expected SipRequest"),
        }
    }

    #[test]
    fn test_parse_cancel_request() {
        let raw = cancel_request();
        let msg = parse_sip_message(raw.as_bytes()).expect("CANCEL should parse");
        match msg {
            SipMessage::Request(req) => {
                assert_eq!(req.method, SipMethod::Cancel);
            }
            _ => panic!("Expected SipRequest"),
        }
    }

    #[test]
    fn test_parse_options_request() {
        let raw = options_request();
        let msg = parse_sip_message(raw.as_bytes()).expect("OPTIONS should parse");
        match msg {
            SipMessage::Request(req) => {
                assert_eq!(req.method, SipMethod::Options);
                assert_eq!(req.uri, "sip:carol@chicago.example.com");
            }
            _ => panic!("Expected SipRequest"),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    //  2. Response parsing
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_parse_200_ok_response() {
        let raw = response_200_ok();
        let msg = parse_sip_message(raw.as_bytes()).expect("200 OK should parse");
        match msg {
            SipMessage::Response(resp) => {
                assert_eq!(resp.status_code, 200);
                assert_eq!(resp.reason_phrase, "OK");
            }
            _ => panic!("Expected SipResponse"),
        }
    }

    #[test]
    fn test_parse_180_ringing_response() {
        let raw = response_180_ringing();
        let msg = parse_sip_message(raw.as_bytes()).expect("180 Ringing should parse");
        match msg {
            SipMessage::Response(resp) => {
                assert_eq!(resp.status_code, 180);
                assert_eq!(resp.reason_phrase, "Ringing");
            }
            _ => panic!("Expected SipResponse"),
        }
    }

    #[test]
    fn test_parse_401_unauthorized_response() {
        let raw = response_401_unauthorized();
        let msg = parse_sip_message(raw.as_bytes()).expect("401 should parse");
        match msg {
            SipMessage::Response(resp) => {
                assert_eq!(resp.status_code, 401);
                assert_eq!(resp.reason_phrase, "Unauthorized");
            }
            _ => panic!("Expected SipResponse"),
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    //  3. Header parsing
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_via_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let via = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Via"))
            .expect("Via header must exist");
        assert!(via.value.contains("SIP/2.0/UDP"));
        assert!(via.value.contains("branch=z9hG4bK776asdhds"));
    }

    #[test]
    fn test_from_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let from = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("From"))
            .expect("From header must exist");
        assert!(from.value.contains("Alice"));
        assert!(from.value.contains("tag=1928301774"));
    }

    #[test]
    fn test_to_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let to = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("To"))
            .expect("To header must exist");
        assert!(to.value.contains("Bob"));
        assert!(to.value.contains("sip:bob@biloxi.example.com"));
    }

    #[test]
    fn test_call_id_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let cid = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Call-ID"))
            .expect("Call-ID header must exist");
        assert_eq!(cid.value.trim(), "a84b4c76e66710@pc33.atlanta.example.com");
    }

    #[test]
    fn test_cseq_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let cseq = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("CSeq"))
            .expect("CSeq header must exist");
        assert!(cseq.value.contains("314159"));
        assert!(cseq.value.contains("INVITE"));
    }

    #[test]
    fn test_contact_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let contact = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Contact"))
            .expect("Contact header must exist");
        assert!(contact.value.contains("sip:alice@pc33.atlanta.example.com"));
    }

    #[test]
    fn test_content_type_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let ct = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Content-Type"))
            .expect("Content-Type header must exist");
        assert_eq!(ct.value.trim(), "application/sdp");
    }

    #[test]
    fn test_max_forwards_header_parsing() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let mf = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Max-Forwards"))
            .expect("Max-Forwards header must exist");
        assert_eq!(mf.value.trim(), "70");
    }

    // ═══════════════════════════════════════════════════════════════════
    //  4. SDP body parsing
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_sdp_body_present_in_invite() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let body = msg.body().expect("INVITE must carry an SDP body");
        assert!(body.contains("v=0"));
        assert!(body.contains("m=audio"));
        assert!(body.contains("a=rtpmap:97 opus/48000/2"));
    }

    #[test]
    fn test_sdp_body_present_in_200_ok() {
        let raw = response_200_ok();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let body = msg.body().expect("200 OK must carry an SDP body");
        assert!(body.contains("v=0"));
        assert!(body.contains("m=audio 49172 RTP/AVP 0"));
        assert!(body.contains("a=rtpmap:0 PCMU/8000"));
    }

    #[test]
    fn test_no_body_when_content_length_zero() {
        let raw = register_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let body = msg.body();
        assert!(
            body.is_none() || body.unwrap().trim().is_empty(),
            "REGISTER with Content-Length: 0 should have no body"
        );
    }

    // ═══════════════════════════════════════════════════════════════════
    //  5. URI parsing
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_sip_uri_user_host() {
        let raw = invite_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        if let SipMessage::Request(req) = msg {
            // URI should contain user@host form
            assert!(req.uri.starts_with("sip:"));
            let authority = req.uri.strip_prefix("sip:").unwrap();
            let parts: Vec<&str> = authority.split('@').collect();
            assert_eq!(parts.len(), 2);
            assert_eq!(parts[0], "bob");
            assert_eq!(parts[1], "biloxi.example.com");
        }
    }

    #[test]
    fn test_sip_uri_ip_address() {
        let raw = bye_request();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        if let SipMessage::Request(req) = msg {
            assert_eq!(req.uri, "sip:bob@192.0.2.4");
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    //  6. Authentication header parsing
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_www_authenticate_header() {
        let raw = response_401_unauthorized();
        let msg = parse_sip_message(raw.as_bytes()).unwrap();
        let headers = msg.headers();
        let auth = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("WWW-Authenticate"))
            .expect("WWW-Authenticate header must exist");
        assert!(auth.value.contains("Digest"));
        assert!(auth.value.contains("realm=\"biloxi.example.com\""));
        assert!(auth.value.contains("qop=\"auth\""));
        assert!(auth.value.contains("nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\""));
        assert!(auth.value.contains("opaque=\"5ccc069c403ebaf9f0171e9517f40e41\""));
    }

    #[test]
    fn test_authorization_header_round_trip() {
        let raw = concat!(
            "REGISTER sip:registrar.biloxi.example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP bobspc.biloxi.example.com:5060;branch=z9hG4bKnashds8\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@biloxi.example.com>\r\n",
            "From: Bob <sip:bob@biloxi.example.com>;tag=456249\r\n",
            "Call-ID: 843817637684230@998sdasdh09\r\n",
            "CSeq: 1827 REGISTER\r\n",
            "Authorization: Digest username=\"bob\", realm=\"biloxi.example.com\", ",
            "nonce=\"dcd98b7102dd2f0e8b11d0f600bfb0c093\", ",
            "uri=\"sip:registrar.biloxi.example.com\", ",
            "response=\"6629fae49393a05397450978507c4ef1\", ",
            "opaque=\"5ccc069c403ebaf9f0171e9517f40e41\"\r\n",
            "Contact: <sip:bob@192.0.2.4>\r\n",
            "Expires: 7200\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        );
        let msg = parse_sip_message(raw.as_bytes()).expect("Authorized REGISTER should parse");
        let headers = msg.headers();
        let auth = headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case("Authorization"))
            .expect("Authorization header must exist");
        assert!(auth.value.contains("username=\"bob\""));
        assert!(auth.value.contains("response=\"6629fae49393a05397450978507c4ef1\""));
    }

    // ═══════════════════════════════════════════════════════════════════
    //  7. Malformed message handling (fuzz-like edge cases)
    // ═══════════════════════════════════════════════════════════════════

    #[test]
    fn test_empty_input() {
        let result = parse_sip_message(b"");
        assert!(result.is_err(), "Empty input must fail gracefully");
    }

    #[test]
    fn test_garbage_input() {
        let result = parse_sip_message(b"\x00\xff\xfe\x80garbage");
        assert!(result.is_err(), "Binary garbage must fail gracefully");
    }

    #[test]
    fn test_missing_crlf_termination() {
        let raw = "INVITE sip:bob@example.com SIP/2.0\nVia: SIP/2.0/UDP 10.0.0.1;branch=z9hG4bKxyz\n\n";
        // Some parsers accept bare LF; either parse or error is acceptable
        let _ = parse_sip_message(raw.as_bytes());
        // No panic = success for this edge case
    }

    #[test]
    fn test_truncated_request_line() {
        let raw = b"INVITE sip:bob@example.com\r\n\r\n";
        let result = parse_sip_message(raw);
        assert!(
            result.is_err(),
            "Request line missing SIP version must fail"
        );
    }

    #[test]
    fn test_oversized_header_value() {
        let huge_value = "X".repeat(70_000);
        let raw = format!(
            "OPTIONS sip:test@example.com SIP/2.0\r\n\
             Via: SIP/2.0/UDP 10.0.0.1;branch=z9hG4bKtest\r\n\
             X-Huge: {}\r\n\
             Content-Length: 0\r\n\
             \r\n",
            huge_value
        );
        // The parser should either reject oversized messages or handle them
        // without panicking or allocating unbounded memory.
        let _ = parse_sip_message(raw.as_bytes());
    }

    #[test]
    fn test_unknown_method() {
        let raw = concat!(
            "PUBLISH sip:pres@example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP 10.0.0.1;branch=z9hG4bKpub\r\n",
            "Max-Forwards: 70\r\n",
            "To: <sip:pres@example.com>\r\n",
            "From: <sip:alice@example.com>;tag=abcdef\r\n",
            "Call-ID: pub-id@example.com\r\n",
            "CSeq: 1 PUBLISH\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        );
        let result = parse_sip_message(raw.as_bytes());
        // Unknown methods may either parse as a generic request or return an error.
        // The critical requirement is no panic.
        match result {
            Ok(SipMessage::Request(req)) => {
                // If the parser supports extension methods, verify the URI at minimum
                assert_eq!(req.uri, "sip:pres@example.com");
            }
            Err(_) => { /* acceptable */ }
            _ => panic!("Unexpected variant"),
        }
    }

    #[test]
    fn test_duplicate_headers() {
        let raw = concat!(
            "INVITE sip:bob@example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP proxy1.example.com;branch=z9hG4bK1\r\n",
            "Via: SIP/2.0/UDP proxy2.example.com;branch=z9hG4bK2\r\n",
            "Max-Forwards: 70\r\n",
            "To: Bob <sip:bob@example.com>\r\n",
            "From: Alice <sip:alice@example.com>;tag=aaa\r\n",
            "Call-ID: dup-via@example.com\r\n",
            "CSeq: 1 INVITE\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        );
        let msg = parse_sip_message(raw.as_bytes()).expect("Duplicate Via should parse");
        let via_count = msg
            .headers()
            .iter()
            .filter(|h| h.name.eq_ignore_ascii_case("Via"))
            .count();
        assert_eq!(via_count, 2, "Both Via headers must be preserved");
    }

    #[test]
    fn test_header_continuation_line() {
        // RFC 3261 7.3.1: Header field values can be extended over multiple
        // lines by preceding each extra line with at least one SP or HTAB.
        let raw = concat!(
            "OPTIONS sip:carol@example.com SIP/2.0\r\n",
            "Via: SIP/2.0/UDP 10.0.0.1;\r\n",
            " branch=z9hG4bKcont\r\n",
            "Max-Forwards: 70\r\n",
            "To: <sip:carol@example.com>\r\n",
            "From: <sip:alice@example.com>;tag=conttest\r\n",
            "Call-ID: cont@example.com\r\n",
            "CSeq: 1 OPTIONS\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        );
        let result = parse_sip_message(raw.as_bytes());
        // Continuation lines may or may not be supported; no panic is the requirement.
        match result {
            Ok(msg) => {
                let via = msg
                    .headers()
                    .iter()
                    .find(|h| h.name.eq_ignore_ascii_case("Via"))
                    .expect("Via must be present");
                assert!(via.value.contains("branch=z9hG4bKcont"));
            }
            Err(_) => { /* acceptable if continuation not supported */ }
        }
    }

    #[test]
    fn test_response_without_reason_phrase() {
        // Some implementations send "SIP/2.0 200\r\n" without reason phrase
        let raw = concat!(
            "SIP/2.0 200\r\n",
            "Via: SIP/2.0/UDP 10.0.0.1;branch=z9hG4bKnoreason\r\n",
            "To: <sip:bob@example.com>;tag=xyz\r\n",
            "From: <sip:alice@example.com>;tag=abc\r\n",
            "Call-ID: noreason@example.com\r\n",
            "CSeq: 1 INVITE\r\n",
            "Content-Length: 0\r\n",
            "\r\n",
        );
        let result = parse_sip_message(raw.as_bytes());
        // May parse with empty reason or error; no panic is required.
        match result {
            Ok(SipMessage::Response(resp)) => {
                assert_eq!(resp.status_code, 200);
            }
            Err(_) => { /* acceptable */ }
            _ => panic!("Unexpected variant"),
        }
    }
}
