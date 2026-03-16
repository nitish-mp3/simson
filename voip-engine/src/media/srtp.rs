//! SRTP / SRTCP implementation (RFC 3711).
//!
//! Provides AES-128-CM encryption, HMAC-SHA1-80 authentication, replay
//! protection with a sliding window, and key derivation from DTLS-SRTP
//! master key material.

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes128;
use bytes::{BufMut, Bytes, BytesMut};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use thiserror::Error;
use tracing::{debug, trace, warn};

use super::rtp::RtpPacket;

type HmacSha1 = Hmac<Sha1>;

// ───────────────────── Constants (RFC 3711) ─────────────────────

/// AES-128-CM key length in bytes.
const SRTP_AES128_KEY_LEN: usize = 16;
/// SRTP salt length in bytes.
const SRTP_SALT_LEN: usize = 14;
/// HMAC-SHA1-80 authentication tag length (10 bytes = 80 bits).
const SRTP_AUTH_TAG_LEN: usize = 10;
/// Default anti-replay window size.
const REPLAY_WINDOW_SIZE: u64 = 1024;

// Key derivation labels (RFC 3711 Section 4.3.1).
const LABEL_RTP_CIPHER: u8 = 0x00;
const LABEL_RTP_AUTH: u8 = 0x01;
const LABEL_RTP_SALT: u8 = 0x02;
const LABEL_RTCP_CIPHER: u8 = 0x03;
const LABEL_RTCP_AUTH: u8 = 0x04;
const LABEL_RTCP_SALT: u8 = 0x05;

// ───────────────────── Errors ─────────────────────

#[derive(Debug, Error)]
pub enum SrtpError {
    #[error("Key derivation failed: {0}")]
    KeyDerivation(String),
    #[error("Encryption failed: {0}")]
    Encryption(String),
    #[error("Decryption failed: {0}")]
    Decryption(String),
    #[error("Authentication tag mismatch")]
    AuthenticationFailed,
    #[error("Replay detected for index {0}")]
    ReplayDetected(u64),
    #[error("Session not initialised")]
    NotInitialized,
    #[error("Invalid key material: expected {expected} bytes, got {actual}")]
    InvalidKeyLength { expected: usize, actual: usize },
}

// ───────────────────── AES-128-CM keystream (RFC 3711 Section 4.1.1) ─────────────────────

/// Generate an AES-128-CM keystream and XOR it with `data` in place.
///
/// The IV is 128 bits.  The counter is formed by treating the IV as a
/// 128-bit big-endian integer and incrementing the least-significant 16
/// bits for each successive AES block.
fn aes128_cm_crypt(key: &[u8], iv: &[u8; 16], data: &[u8]) -> Vec<u8> {
    let cipher = Aes128::new(key.into());
    let mut output = Vec::with_capacity(data.len());
    let mut counter = *iv;
    let mut pos = 0;

    while pos < data.len() {
        // Encrypt the counter to produce a keystream block.
        let mut block = counter.into();
        cipher.encrypt_block(&mut block);
        let ks: [u8; 16] = block.into();

        let chunk = (data.len() - pos).min(16);
        for i in 0..chunk {
            output.push(data[pos + i] ^ ks[i]);
        }
        pos += chunk;

        // Increment the 128-bit counter (big-endian).
        for j in (0..16).rev() {
            counter[j] = counter[j].wrapping_add(1);
            if counter[j] != 0 {
                break;
            }
        }
    }

    output
}

// ───────────────────── Key derivation (RFC 3711 Section 4.3) ─────────────────────

/// SRTP key derivation function using AES-128-CM as the PRF.
///
/// `r` is the key derivation rate index, normally 0 when the key
/// derivation rate is 0 (no periodic re-keying).
fn srtp_kdf(
    master_key: &[u8],
    master_salt: &[u8],
    label: u8,
    _kdr_index: u64,
    output_len: usize,
) -> Result<Vec<u8>, SrtpError> {
    // x = label * 2^48  (label is placed at byte offset 7 of a 14-byte
    //     value, so effectively it occupies bits 48..55).
    let mut x = [0u8; 14];
    x[7] = label;
    // When key_derivation_rate == 0 the "r" value is zero, so x stays as-is.

    // IV = (master_salt XOR x) || 00 00
    let mut iv = [0u8; 16];
    for i in 0..14 {
        iv[i] = master_salt.get(i).copied().unwrap_or(0) ^ x[i];
    }
    // iv[14] and iv[15] remain zero.

    // Generate `output_len` bytes of keystream using AES-128-CM.
    let zeros = vec![0u8; output_len];
    let derived = aes128_cm_crypt(master_key, &iv, &zeros);
    Ok(derived)
}

// ───────────────────── HMAC-SHA1 auth tag ─────────────────────

/// Compute the SRTP authentication tag (HMAC-SHA1, truncated to 80 bits).
fn compute_auth_tag(auth_key: &[u8], data: &[u8], roc: u32) -> Result<Vec<u8>, SrtpError> {
    let mut mac = <HmacSha1 as Mac>::new_from_slice(auth_key)
        .map_err(|e| SrtpError::Encryption(e.to_string()))?;
    mac.update(data);
    mac.update(&roc.to_be_bytes());
    let result = mac.finalize().into_bytes();
    Ok(result[..SRTP_AUTH_TAG_LEN].to_vec())
}

/// Compute the SRTCP authentication tag (ROC is already embedded in the
/// packet as the SRTCP index, so we pass 0).
fn compute_rtcp_auth_tag(auth_key: &[u8], data: &[u8]) -> Result<Vec<u8>, SrtpError> {
    let mut mac = <HmacSha1 as Mac>::new_from_slice(auth_key)
        .map_err(|e| SrtpError::Encryption(e.to_string()))?;
    mac.update(data);
    let result = mac.finalize().into_bytes();
    Ok(result[..SRTP_AUTH_TAG_LEN].to_vec())
}

// ───────────────────── Replay protection ─────────────────────

/// Sliding-window replay protection (RFC 3711 Section 3.3.2).
struct ReplayWindow {
    top: u64,
    bitmap: u128,
    window_size: u64,
}

impl ReplayWindow {
    fn new(window_size: u64) -> Self {
        ReplayWindow {
            top: 0,
            bitmap: 0,
            window_size,
        }
    }

    /// Check whether `index` is acceptable and, if so, mark it as seen.
    fn check_and_update(&mut self, index: u64) -> bool {
        if self.top == 0 && self.bitmap == 0 {
            self.top = index;
            self.bitmap = 1;
            return true;
        }

        if index > self.top {
            let shift = index - self.top;
            if shift < 128 {
                self.bitmap <<= shift;
                self.bitmap |= 1;
            } else {
                self.bitmap = 1;
            }
            self.top = index;
            true
        } else if self.top - index >= self.window_size {
            false // too old
        } else {
            let offset = self.top - index;
            if offset >= 128 {
                return false;
            }
            let mask = 1u128 << offset;
            if self.bitmap & mask != 0 {
                false // already seen
            } else {
                self.bitmap |= mask;
                true
            }
        }
    }
}

/// Stand-alone replay check (does not mutate state).  Useful when you
/// only want to test without committing.
pub fn replay_check(top: u64, bitmap: u128, window_size: u64, index: u64) -> bool {
    if top == 0 && bitmap == 0 {
        return true;
    }
    if index > top {
        return true;
    }
    if top - index >= window_size {
        return false;
    }
    let offset = top - index;
    if offset >= 128 {
        return false;
    }
    bitmap & (1u128 << offset) == 0
}

// ───────────────────── SRTP Context ─────────────────────

/// SRTP cryptographic context for one direction (send or receive).
///
/// Created from a 16-byte master key and a 14-byte master salt (typically
/// extracted from DTLS-SRTP key material).
pub struct SrtpContext {
    // ── Master material ──
    pub master_key: Vec<u8>,
    pub master_salt: Vec<u8>,

    // ── Derived session keys ──
    session_key: Vec<u8>,
    session_salt: Vec<u8>,
    session_auth_key: Vec<u8>,
    rtcp_session_key: Vec<u8>,
    rtcp_session_salt: Vec<u8>,
    rtcp_session_auth_key: Vec<u8>,

    // ── Rollover counter ──
    /// 32-bit extension of the 16-bit RTP sequence number.
    pub roc: u32,
    pub last_seq: u16,

    // ── SRTCP index ──
    srtcp_index: u32,

    // ── Replay windows ──
    /// Bitmask-based sliding replay window for inbound RTP.
    replay_window: ReplayWindow,
    rtcp_replay_window: ReplayWindow,

    initialized: bool,
}

impl SrtpContext {
    /// Create a new SRTP context from DTLS-SRTP key material.
    ///
    /// * `master_key`  -- 16 bytes.
    /// * `master_salt` -- 14 bytes.
    pub fn new(master_key: &[u8], master_salt: &[u8]) -> Result<Self, SrtpError> {
        if master_key.len() != SRTP_AES128_KEY_LEN {
            return Err(SrtpError::InvalidKeyLength {
                expected: SRTP_AES128_KEY_LEN,
                actual: master_key.len(),
            });
        }
        if master_salt.len() != SRTP_SALT_LEN {
            return Err(SrtpError::InvalidKeyLength {
                expected: SRTP_SALT_LEN,
                actual: master_salt.len(),
            });
        }

        let session_key = srtp_kdf(master_key, master_salt, LABEL_RTP_CIPHER, 0, 16)?;
        let session_auth_key = srtp_kdf(master_key, master_salt, LABEL_RTP_AUTH, 0, 20)?;
        let session_salt = srtp_kdf(master_key, master_salt, LABEL_RTP_SALT, 0, 14)?;
        let rtcp_session_key = srtp_kdf(master_key, master_salt, LABEL_RTCP_CIPHER, 0, 16)?;
        let rtcp_session_auth_key = srtp_kdf(master_key, master_salt, LABEL_RTCP_AUTH, 0, 20)?;
        let rtcp_session_salt = srtp_kdf(master_key, master_salt, LABEL_RTCP_SALT, 0, 14)?;

        debug!("SRTP session keys derived successfully");

        Ok(SrtpContext {
            master_key: master_key.to_vec(),
            master_salt: master_salt.to_vec(),
            session_key,
            session_salt,
            session_auth_key,
            rtcp_session_key,
            rtcp_session_salt,
            rtcp_session_auth_key,
            roc: 0,
            last_seq: 0,
            srtcp_index: 0,
            replay_window: ReplayWindow::new(REPLAY_WINDOW_SIZE),
            rtcp_replay_window: ReplayWindow::new(REPLAY_WINDOW_SIZE),
            initialized: true,
        })
    }

    // ───────────── RTP protect / unprotect ─────────────

    /// Encrypt an RTP packet to produce an SRTP packet.
    ///
    /// Layout: `RTP-header || encrypted-payload || auth-tag(10)`
    pub fn protect_rtp(&mut self, packet: &RtpPacket) -> Result<Vec<u8>, SrtpError> {
        if !self.initialized {
            return Err(SrtpError::NotInitialized);
        }

        let rtp_data = packet.to_bytes();
        let header_len = self.rtp_header_len(&rtp_data)?;
        let seq = packet.header.sequence_number;
        let ssrc = packet.header.ssrc;

        self.update_roc_send(seq);
        let index = ((self.roc as u64) << 16) | (seq as u64);
        let iv = self.make_rtp_iv(ssrc, index);

        // Encrypt payload only (header is authenticated but not encrypted).
        let encrypted_payload =
            aes128_cm_crypt(&self.session_key, &iv, &rtp_data[header_len..]);

        let mut out = BytesMut::with_capacity(header_len + encrypted_payload.len() + SRTP_AUTH_TAG_LEN);
        out.put_slice(&rtp_data[..header_len]);
        out.put_slice(&encrypted_payload);

        // Authentication tag over header + encrypted payload.
        let tag = compute_auth_tag(&self.session_auth_key, &out, self.roc)?;
        out.put_slice(&tag);

        trace!(seq, ssrc, roc = self.roc, "SRTP protect");
        Ok(out.to_vec())
    }

    /// Decrypt an SRTP packet to produce a plain RTP packet.
    pub fn unprotect_rtp(&mut self, data: &[u8]) -> Result<RtpPacket, SrtpError> {
        if !self.initialized {
            return Err(SrtpError::NotInitialized);
        }
        if data.len() < 12 + SRTP_AUTH_TAG_LEN {
            return Err(SrtpError::Decryption("SRTP packet too short".into()));
        }

        let data_len = data.len() - SRTP_AUTH_TAG_LEN;
        let authenticated = &data[..data_len];
        let received_tag = &data[data_len..];

        let seq = u16::from_be_bytes([data[2], data[3]]);
        let ssrc = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);

        let est_roc = self.estimate_roc(seq);
        let index = ((est_roc as u64) << 16) | (seq as u64);

        // Verify authentication.
        let computed_tag = compute_auth_tag(&self.session_auth_key, authenticated, est_roc)?;
        if computed_tag != received_tag {
            return Err(SrtpError::AuthenticationFailed);
        }

        // Replay check.
        if !self.replay_window.check_and_update(index) {
            return Err(SrtpError::ReplayDetected(index));
        }

        self.update_roc_recv(seq);

        // Decrypt payload.
        let header_len = self.rtp_header_len(&data[..data_len])?;
        let iv = self.make_rtp_iv(ssrc, index);
        let decrypted = aes128_cm_crypt(&self.session_key, &iv, &data[header_len..data_len]);

        // Reconstruct the plain RTP packet.
        let mut plain = Vec::with_capacity(header_len + decrypted.len());
        plain.extend_from_slice(&data[..header_len]);
        plain.extend_from_slice(&decrypted);

        super::rtp::parse_rtp(&plain).map_err(|e| SrtpError::Decryption(e.to_string()))
    }

    // ───────────── RTCP protect / unprotect ─────────────

    /// Encrypt an RTCP packet to produce an SRTCP packet.
    ///
    /// Layout: `RTCP-header(8) || encrypted-tail || SRTCP-index(4) || auth-tag(10)`
    pub fn protect_rtcp(&mut self, rtcp_data: &[u8]) -> Result<Vec<u8>, SrtpError> {
        if !self.initialized {
            return Err(SrtpError::NotInitialized);
        }
        if rtcp_data.len() < 8 {
            return Err(SrtpError::Encryption("RTCP packet too short".into()));
        }

        let ssrc = u32::from_be_bytes([rtcp_data[4], rtcp_data[5], rtcp_data[6], rtcp_data[7]]);
        let index = self.srtcp_index;
        self.srtcp_index += 1;

        let e_flag_index = index | 0x8000_0000; // E-flag set = encrypted

        let iv = self.make_rtcp_iv(ssrc, index);
        let encrypted_tail = aes128_cm_crypt(&self.rtcp_session_key, &iv, &rtcp_data[8..]);

        let mut out = BytesMut::with_capacity(8 + encrypted_tail.len() + 4 + SRTP_AUTH_TAG_LEN);
        out.put_slice(&rtcp_data[..8]);
        out.put_slice(&encrypted_tail);
        out.put_u32(e_flag_index);

        let tag = compute_rtcp_auth_tag(&self.rtcp_session_auth_key, &out)?;
        out.put_slice(&tag);

        Ok(out.to_vec())
    }

    /// Decrypt an SRTCP packet to produce a plain RTCP packet.
    pub fn unprotect_rtcp(&mut self, data: &[u8]) -> Result<Vec<u8>, SrtpError> {
        if !self.initialized {
            return Err(SrtpError::NotInitialized);
        }
        if data.len() < 8 + 4 + SRTP_AUTH_TAG_LEN {
            return Err(SrtpError::Decryption("SRTCP packet too short".into()));
        }

        let data_len = data.len() - SRTP_AUTH_TAG_LEN;
        let authenticated = &data[..data_len];
        let received_tag = &data[data_len..];

        let computed_tag = compute_rtcp_auth_tag(&self.rtcp_session_auth_key, authenticated)?;
        if computed_tag != received_tag {
            return Err(SrtpError::AuthenticationFailed);
        }

        let idx_off = data_len - 4;
        let srtcp_idx_with_e = u32::from_be_bytes([
            data[idx_off],
            data[idx_off + 1],
            data[idx_off + 2],
            data[idx_off + 3],
        ]);
        let index = srtcp_idx_with_e & 0x7FFF_FFFF;

        if !self.rtcp_replay_window.check_and_update(index as u64) {
            return Err(SrtpError::ReplayDetected(index as u64));
        }

        let ssrc = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let iv = self.make_rtcp_iv(ssrc, index);
        let decrypted = aes128_cm_crypt(&self.rtcp_session_key, &iv, &data[8..idx_off]);

        let mut plain = Vec::with_capacity(8 + decrypted.len());
        plain.extend_from_slice(&data[..8]);
        plain.extend_from_slice(&decrypted);

        Ok(plain)
    }

    // ───────────── Internal helpers ─────────────

    /// Compute the RTP fixed-header length (including CSRC and extension).
    fn rtp_header_len(&self, data: &[u8]) -> Result<usize, SrtpError> {
        if data.len() < 12 {
            return Err(SrtpError::Encryption("RTP too short".into()));
        }
        let cc = (data[0] & 0x0F) as usize;
        let has_ext = (data[0] >> 4) & 0x01 != 0;
        let mut off = 12 + cc * 4;
        if has_ext {
            if data.len() < off + 4 {
                return Err(SrtpError::Encryption("Extension header truncated".into()));
            }
            let ext_words = u16::from_be_bytes([data[off + 2], data[off + 3]]) as usize;
            off += 4 + ext_words * 4;
        }
        if off > data.len() {
            return Err(SrtpError::Encryption("Header exceeds packet".into()));
        }
        Ok(off)
    }

    /// Build the 128-bit IV for RTP AES-128-CM.
    fn make_rtp_iv(&self, ssrc: u32, index: u64) -> [u8; 16] {
        let mut iv = [0u8; 16];
        let ssrc_bytes = ssrc.to_be_bytes();
        for i in 0..4 {
            iv[4 + i] = ssrc_bytes[i] ^ self.session_salt.get(4 + i).copied().unwrap_or(0);
        }
        let idx_bytes = index.to_be_bytes();
        for i in 0..6 {
            iv[8 + i] = idx_bytes[2 + i] ^ self.session_salt.get(8 + i).copied().unwrap_or(0);
        }
        iv
    }

    /// Build the 128-bit IV for RTCP AES-128-CM.
    fn make_rtcp_iv(&self, ssrc: u32, index: u32) -> [u8; 16] {
        let mut iv = [0u8; 16];
        let ssrc_bytes = ssrc.to_be_bytes();
        for i in 0..4 {
            iv[4 + i] = ssrc_bytes[i] ^ self.rtcp_session_salt.get(4 + i).copied().unwrap_or(0);
        }
        let idx_bytes = index.to_be_bytes();
        for i in 0..4 {
            iv[10 + i] = idx_bytes[i] ^ self.rtcp_session_salt.get(10 + i).copied().unwrap_or(0);
        }
        iv
    }

    /// Update ROC when sending (sequence always increments).
    fn update_roc_send(&mut self, seq: u16) {
        if self.last_seq > 0xF000 && seq < 0x1000 {
            self.roc += 1;
            debug!(roc = self.roc, "ROC incremented (send)");
        }
        self.last_seq = seq;
    }

    /// Update ROC after receiving (sequence may jump).
    fn update_roc_recv(&mut self, seq: u16) {
        if self.last_seq > 0xF000 && seq < 0x1000 {
            self.roc += 1;
            debug!(roc = self.roc, "ROC incremented (recv)");
        }
        self.last_seq = seq;
    }

    /// Estimate the ROC for a received sequence number without mutating state.
    fn estimate_roc(&self, seq: u16) -> u32 {
        if self.last_seq > 0xF000 && seq < 0x1000 {
            self.roc + 1
        } else if self.last_seq < 0x1000 && seq > 0xF000 {
            self.roc.saturating_sub(1)
        } else {
            self.roc
        }
    }
}

// ───────────────────── DTLS-SRTP key extraction ─────────────────────

/// Placeholder for DTLS-SRTP integration.
///
/// After a DTLS handshake completes the two peers share keying material
/// from which SRTP master keys and salts are extracted (RFC 5764).
pub struct DtlsSrtpContext;

impl DtlsSrtpContext {
    /// Extract two SRTP contexts (send + receive) from DTLS keying material.
    ///
    /// `material` must be at least 60 bytes:
    ///   client_key(16) || server_key(16) || client_salt(14) || server_salt(14)
    ///
    /// `is_server` indicates whether we are the DTLS server (determines which
    /// key/salt pair is used for sending vs receiving).
    pub fn extract_srtp_keys(
        material: &[u8],
        is_server: bool,
    ) -> Result<(SrtpContext, SrtpContext), SrtpError> {
        if material.len() < 60 {
            return Err(SrtpError::InvalidKeyLength {
                expected: 60,
                actual: material.len(),
            });
        }

        let client_key = &material[0..16];
        let server_key = &material[16..32];
        let client_salt = &material[32..46];
        let server_salt = &material[46..60];

        let (send_ctx, recv_ctx) = if is_server {
            (
                SrtpContext::new(server_key, server_salt)?,
                SrtpContext::new(client_key, client_salt)?,
            )
        } else {
            (
                SrtpContext::new(client_key, client_salt)?,
                SrtpContext::new(server_key, server_salt)?,
            )
        };

        debug!("DTLS-SRTP keys extracted");
        Ok((send_ctx, recv_ctx))
    }
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_window_basics() {
        let mut w = ReplayWindow::new(64);
        assert!(w.check_and_update(1));
        assert!(w.check_and_update(2));
        assert!(w.check_and_update(3));
        assert!(!w.check_and_update(1)); // replay
        assert!(!w.check_and_update(2));
        assert!(w.check_and_update(100));
        assert!(!w.check_and_update(1)); // too old
    }

    #[test]
    fn replay_check_fn() {
        assert!(replay_check(0, 0, 64, 5)); // first packet
        assert!(replay_check(10, 0b111, 64, 15)); // ahead of top
        assert!(!replay_check(100, 0b1, 64, 30)); // too old
    }

    #[test]
    fn srtp_protect_unprotect_rtp() {
        let key = [0x01u8; 16];
        let salt = [0x02u8; 14];

        let mut ctx_tx = SrtpContext::new(&key, &salt).unwrap();
        let mut ctx_rx = SrtpContext::new(&key, &salt).unwrap();

        let pkt = RtpPacket::new(0, 1, 160, 0x12345678, false, Bytes::from_static(b"audio"));
        let protected = ctx_tx.protect_rtp(&pkt).unwrap();

        // Protected data should differ from plain.
        assert_ne!(protected, pkt.to_bytes());

        let recovered = ctx_rx.unprotect_rtp(&protected).unwrap();
        assert_eq!(recovered.payload, Bytes::from_static(b"audio"));
        assert_eq!(recovered.header.sequence_number, 1);
    }

    #[test]
    fn srtp_replay_rejected() {
        let key = [0x03u8; 16];
        let salt = [0x04u8; 14];

        let mut ctx_tx = SrtpContext::new(&key, &salt).unwrap();
        let mut ctx_rx = SrtpContext::new(&key, &salt).unwrap();

        let pkt = RtpPacket::new(0, 42, 160, 0xAABBCCDD, false, Bytes::from_static(b"test"));
        let protected = ctx_tx.protect_rtp(&pkt).unwrap();

        ctx_rx.unprotect_rtp(&protected).unwrap();
        assert!(ctx_rx.unprotect_rtp(&protected).is_err()); // replay
    }

    #[test]
    fn srtcp_protect_unprotect() {
        let key = [0x05u8; 16];
        let salt = [0x06u8; 14];

        let mut ctx_tx = SrtpContext::new(&key, &salt).unwrap();
        let mut ctx_rx = SrtpContext::new(&key, &salt).unwrap();

        let rtcp = vec![
            0x80, 0xC8, 0x00, 0x06, 0x12, 0x34, 0x56, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x10, 0x00,
        ];

        let protected = ctx_tx.protect_rtcp(&rtcp).unwrap();
        let recovered = ctx_rx.unprotect_rtcp(&protected).unwrap();
        assert_eq!(recovered, rtcp);
    }

    #[test]
    fn dtls_srtp_extract_keys() {
        let material = vec![0xAA; 60];
        let (send, recv) = DtlsSrtpContext::extract_srtp_keys(&material, false).unwrap();
        assert!(send.initialized);
        assert!(recv.initialized);
    }

    #[test]
    fn aes_cm_deterministic() {
        let key = [0x11u8; 16];
        let iv = [0u8; 16];
        let data = [0u8; 32];
        let c1 = aes128_cm_crypt(&key, &iv, &data);
        let c2 = aes128_cm_crypt(&key, &iv, &data);
        assert_eq!(c1, c2);
        // Decrypting the ciphertext should yield the original plaintext.
        let plain = aes128_cm_crypt(&key, &iv, &c1);
        assert_eq!(plain, data);
    }
}
