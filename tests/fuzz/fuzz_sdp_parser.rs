//! Fuzz harness for the SDP parser.
//!
//! Run:
//!     cargo +nightly fuzz run fuzz_sdp_parser -- -max_len=16384
//!
//! Feeds arbitrary byte slices to `parse_sdp` and verifies that the
//! function never panics.  A structured-fuzzing mode generates
//! semi-valid SDP bodies to exercise deeper parser paths.

#![no_main]

use libfuzzer_sys::fuzz_target;

use voip_engine::sip::sdp::parse_sdp;

fuzz_target!(|data: &[u8]| {
    let _ = parse_sdp(data);
});

// ---------------------------------------------------------------------------
// Structured SDP fuzzing
// ---------------------------------------------------------------------------

#[cfg(feature = "structured_fuzz")]
mod structured {
    use libfuzzer_sys::fuzz_target;
    use libfuzzer_sys::arbitrary::{self, Arbitrary};
    use voip_engine::sip::sdp::parse_sdp;

    #[derive(Debug, Arbitrary)]
    struct FuzzSdp {
        version: u8,
        origin_user: String,
        origin_sess_id: u64,
        origin_sess_ver: u64,
        origin_addr: String,
        session_name: String,
        connection_addr: String,
        media_port: u16,
        media_proto: FuzzMediaProto,
        payload_types: Vec<u8>,
        attributes: Vec<FuzzAttribute>,
    }

    #[derive(Debug, Arbitrary)]
    enum FuzzMediaProto {
        RtpAvp,
        RtpSavp,
        RtpSavpf,
        UdpTlsRtpSavpf,
        Other(String),
    }

    #[derive(Debug, Arbitrary)]
    struct FuzzAttribute {
        key: String,
        value: Option<String>,
    }

    impl FuzzSdp {
        fn to_bytes(&self) -> Vec<u8> {
            let proto = match &self.media_proto {
                FuzzMediaProto::RtpAvp => "RTP/AVP",
                FuzzMediaProto::RtpSavp => "RTP/SAVP",
                FuzzMediaProto::RtpSavpf => "RTP/SAVPF",
                FuzzMediaProto::UdpTlsRtpSavpf => "UDP/TLS/RTP/SAVPF",
                FuzzMediaProto::Other(s) => s.as_str(),
            };

            let pts: String = self
                .payload_types
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(" ");

            let mut sdp = format!(
                "v={version}\r\n\
                 o={user} {sid} {sver} IN IP4 {addr}\r\n\
                 s={sname}\r\n\
                 c=IN IP4 {caddr}\r\n\
                 t=0 0\r\n\
                 m=audio {port} {proto} {pts}\r\n",
                version = self.version,
                user = self.origin_user,
                sid = self.origin_sess_id,
                sver = self.origin_sess_ver,
                addr = self.origin_addr,
                sname = self.session_name,
                caddr = self.connection_addr,
                port = self.media_port,
            );

            for attr in &self.attributes {
                match &attr.value {
                    Some(v) => sdp.push_str(&format!("a={}:{}\r\n", attr.key, v)),
                    None => sdp.push_str(&format!("a={}\r\n", attr.key)),
                }
            }

            sdp.into_bytes()
        }
    }

    fuzz_target!(|msg: FuzzSdp| {
        let data = msg.to_bytes();
        let _ = parse_sdp(&data);
    });
}
