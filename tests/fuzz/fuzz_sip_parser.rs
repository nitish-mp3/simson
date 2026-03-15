//! Fuzz harness for the SIP message parser.
//!
//! Uses `cargo-fuzz` (libFuzzer backend).
//!
//! Run:
//!     cargo +nightly fuzz run fuzz_sip_parser -- -max_len=65536
//!
//! The fuzzer feeds arbitrary byte slices to `parse_sip_message` and
//! verifies that the function never panics, regardless of input.

#![no_main]

use libfuzzer_sys::fuzz_target;

// The crate under test.  Adjust the path if your workspace layout differs.
use voip_engine::sip::parser::parse_sip_message;

fuzz_target!(|data: &[u8]| {
    // The only requirement is "no panic".  We intentionally ignore
    // Ok / Err — the fuzzer is looking for crashes, not correctness.
    let _ = parse_sip_message(data);
});

// ---------------------------------------------------------------------------
// Structured fuzzing: use `Arbitrary` to generate semi-valid SIP messages
// so the fuzzer can explore deeper parser paths.
// ---------------------------------------------------------------------------

#[cfg(feature = "structured_fuzz")]
mod structured {
    use libfuzzer_sys::fuzz_target;
    use libfuzzer_sys::arbitrary::{self, Arbitrary};
    use voip_engine::sip::parser::parse_sip_message;

    #[derive(Debug, Arbitrary)]
    struct FuzzSipMessage {
        method: FuzzMethod,
        user: String,
        host: String,
        branch: String,
        call_id: String,
        cseq: u32,
        body: Vec<u8>,
    }

    #[derive(Debug, Arbitrary)]
    enum FuzzMethod {
        Invite,
        Register,
        Bye,
        Ack,
        Cancel,
        Options,
        Other(String),
    }

    impl FuzzSipMessage {
        fn to_bytes(&self) -> Vec<u8> {
            let method_str = match &self.method {
                FuzzMethod::Invite => "INVITE",
                FuzzMethod::Register => "REGISTER",
                FuzzMethod::Bye => "BYE",
                FuzzMethod::Ack => "ACK",
                FuzzMethod::Cancel => "CANCEL",
                FuzzMethod::Options => "OPTIONS",
                FuzzMethod::Other(s) => s.as_str(),
            };
            let uri = format!("sip:{}@{}", self.user, self.host);
            let content_length = self.body.len();

            let mut msg = format!(
                "{method_str} {uri} SIP/2.0\r\n\
                 Via: SIP/2.0/UDP 10.0.0.1;branch={branch}\r\n\
                 Max-Forwards: 70\r\n\
                 To: <{uri}>\r\n\
                 From: <sip:fuzz@fuzzer.local>;tag=fuzz\r\n\
                 Call-ID: {call_id}\r\n\
                 CSeq: {cseq} {method_str}\r\n\
                 Content-Length: {content_length}\r\n\
                 \r\n",
                branch = self.branch,
                call_id = self.call_id,
                cseq = self.cseq,
            );
            let mut bytes = msg.into_bytes();
            bytes.extend_from_slice(&self.body);
            bytes
        }
    }

    fuzz_target!(|msg: FuzzSipMessage| {
        let data = msg.to_bytes();
        let _ = parse_sip_message(&data);
    });
}
