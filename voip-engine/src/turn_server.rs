//! Embedded TURN / STUN server (RFC 5389 / RFC 5766).
//!
//! Implements STUN Binding, TURN Allocate, Refresh, CreatePermission,
//! ChannelBind, Send/Data indications, plus HMAC-SHA1 message integrity,
//! CRC-32 fingerprint, per-IP rate limiting, and periodic allocation cleanup.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bytes::{BufMut, BytesMut};
use dashmap::DashMap;
use hmac::{Hmac, Mac};
use rand::Rng;
use sha1::Sha1;
use thiserror::Error;
use tokio::net::UdpSocket;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

use crate::config::TurnConfig;

type HmacSha1 = Hmac<Sha1>;

// ───────────────────── Constants ─────────────────────

const STUN_MAGIC_COOKIE: u32 = 0x2112_A442;
const FINGERPRINT_XOR: u32 = 0x5354_554E;
const DEFAULT_LIFETIME: u32 = 600;

// STUN message types
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_RESPONSE: u16 = 0x0101;
#[allow(dead_code)]
const BINDING_ERROR: u16 = 0x0111;
const ALLOCATE_REQUEST: u16 = 0x0003;
const ALLOCATE_RESPONSE: u16 = 0x0103;
const ALLOCATE_ERROR: u16 = 0x0113;
const REFRESH_REQUEST: u16 = 0x0004;
const REFRESH_RESPONSE: u16 = 0x0104;
const REFRESH_ERROR: u16 = 0x0114;
const CREATE_PERMISSION_REQUEST: u16 = 0x0008;
const CREATE_PERMISSION_RESPONSE: u16 = 0x0108;
const CREATE_PERMISSION_ERROR: u16 = 0x0118;
const CHANNEL_BIND_REQUEST: u16 = 0x0009;
const CHANNEL_BIND_RESPONSE: u16 = 0x0109;
const CHANNEL_BIND_ERROR: u16 = 0x0119;
const SEND_INDICATION: u16 = 0x0016;
const DATA_INDICATION: u16 = 0x0017;

// STUN attribute types
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;
const ATTR_USERNAME: u16 = 0x0006;
const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
const ATTR_ERROR_CODE: u16 = 0x0009;
#[allow(dead_code)]
const ATTR_UNKNOWN_ATTRIBUTES: u16 = 0x000A;
const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
const ATTR_LIFETIME: u16 = 0x000D;
const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
const ATTR_DATA: u16 = 0x0013;
const ATTR_REALM: u16 = 0x0014;
const ATTR_NONCE: u16 = 0x0015;
const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
const ATTR_SOFTWARE: u16 = 0x8022;
const ATTR_FINGERPRINT: u16 = 0x8028;

// ───────────────────── Errors ─────────────────────

#[derive(Debug, Error)]
pub enum TurnError {
    #[error("STUN parse error: {0}")]
    Parse(String),
    #[error("Authentication failed")]
    AuthFailed,
    #[error("Allocation not found")]
    AllocationNotFound,
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Rate limited")]
    RateLimited,
}

// ───────────────────── STUN attribute ─────────────────────

#[derive(Debug, Clone)]
pub enum StunAttribute {
    MappedAddress(SocketAddr),
    XorMappedAddress(SocketAddr),
    XorRelayedAddress(SocketAddr),
    XorPeerAddress(SocketAddr),
    Username(String),
    MessageIntegrity(Vec<u8>),
    ErrorCode { code: u16, reason: String },
    Realm(String),
    Nonce(String),
    Lifetime(u32),
    Data(Vec<u8>),
    RequestedTransport(u8),
    ChannelNumber(u16),
    Software(String),
    Fingerprint(u32),
    Unknown { attr_type: u16, data: Vec<u8> },
}

// ───────────────────── STUN message ─────────────────────

#[derive(Debug, Clone)]
pub struct StunMessage {
    pub message_type: u16,
    pub transaction_id: [u8; 12],
    pub attributes: Vec<StunAttribute>,
}

impl StunMessage {
    /// Parse a STUN message from raw bytes.
    pub fn parse(data: &[u8]) -> Result<Self, TurnError> {
        if data.len() < 20 {
            return Err(TurnError::Parse("Message too short".into()));
        }

        let msg_type = u16::from_be_bytes([data[0], data[1]]);
        if msg_type & 0xC000 != 0 {
            return Err(TurnError::Parse("Not a STUN message".into()));
        }

        let msg_len = u16::from_be_bytes([data[2], data[3]]) as usize;
        let cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        if cookie != STUN_MAGIC_COOKIE {
            return Err(TurnError::Parse("Invalid magic cookie".into()));
        }

        let mut txn_id = [0u8; 12];
        txn_id.copy_from_slice(&data[8..20]);

        if data.len() < 20 + msg_len {
            return Err(TurnError::Parse("Truncated message".into()));
        }

        let mut attrs = Vec::new();
        let mut offset = 20;
        let end = 20 + msg_len;

        while offset + 4 <= end {
            let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let attr_len = u16::from_be_bytes([data[offset + 2], data[offset + 3]]) as usize;
            offset += 4;

            if offset + attr_len > end {
                break;
            }

            let attr_data = &data[offset..offset + attr_len];

            let attr = match attr_type {
                ATTR_MAPPED_ADDRESS => {
                    parse_address(attr_data, false, &txn_id).map(StunAttribute::MappedAddress)
                }
                ATTR_XOR_MAPPED_ADDRESS => {
                    parse_address(attr_data, true, &txn_id).map(StunAttribute::XorMappedAddress)
                }
                ATTR_XOR_RELAYED_ADDRESS => {
                    parse_address(attr_data, true, &txn_id).map(StunAttribute::XorRelayedAddress)
                }
                ATTR_XOR_PEER_ADDRESS => {
                    parse_address(attr_data, true, &txn_id).map(StunAttribute::XorPeerAddress)
                }
                ATTR_USERNAME => Some(StunAttribute::Username(
                    String::from_utf8_lossy(attr_data).into(),
                )),
                ATTR_MESSAGE_INTEGRITY => Some(StunAttribute::MessageIntegrity(attr_data.to_vec())),
                ATTR_ERROR_CODE if attr_data.len() >= 4 => {
                    let cls = (attr_data[2] & 0x07) as u16;
                    let num = attr_data[3] as u16;
                    Some(StunAttribute::ErrorCode {
                        code: cls * 100 + num,
                        reason: String::from_utf8_lossy(&attr_data[4..]).into(),
                    })
                }
                ATTR_REALM => Some(StunAttribute::Realm(
                    String::from_utf8_lossy(attr_data).into(),
                )),
                ATTR_NONCE => Some(StunAttribute::Nonce(
                    String::from_utf8_lossy(attr_data).into(),
                )),
                ATTR_LIFETIME if attr_data.len() >= 4 => Some(StunAttribute::Lifetime(
                    u32::from_be_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]),
                )),
                ATTR_DATA => Some(StunAttribute::Data(attr_data.to_vec())),
                ATTR_REQUESTED_TRANSPORT if !attr_data.is_empty() => {
                    Some(StunAttribute::RequestedTransport(attr_data[0]))
                }
                ATTR_CHANNEL_NUMBER if attr_data.len() >= 2 => Some(StunAttribute::ChannelNumber(
                    u16::from_be_bytes([attr_data[0], attr_data[1]]),
                )),
                ATTR_SOFTWARE => Some(StunAttribute::Software(
                    String::from_utf8_lossy(attr_data).into(),
                )),
                ATTR_FINGERPRINT if attr_data.len() >= 4 => Some(StunAttribute::Fingerprint(
                    u32::from_be_bytes([attr_data[0], attr_data[1], attr_data[2], attr_data[3]]),
                )),
                _ => Some(StunAttribute::Unknown {
                    attr_type,
                    data: attr_data.to_vec(),
                }),
            };

            if let Some(a) = attr {
                attrs.push(a);
            }

            offset += (attr_len + 3) & !3; // pad to 4-byte boundary
        }

        Ok(StunMessage {
            message_type: msg_type,
            transaction_id: txn_id,
            attributes: attrs,
        })
    }

    /// Serialize without MESSAGE-INTEGRITY or FINGERPRINT.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut body = BytesMut::new();
        for attr in &self.attributes {
            encode_attr(&mut body, attr, &self.transaction_id);
        }
        let mut buf = BytesMut::with_capacity(20 + body.len());
        buf.put_u16(self.message_type);
        buf.put_u16(body.len() as u16);
        buf.put_u32(STUN_MAGIC_COOKIE);
        buf.put_slice(&self.transaction_id);
        buf.put_slice(&body);
        buf.to_vec()
    }

    /// Serialize and append MESSAGE-INTEGRITY + FINGERPRINT.
    pub fn to_bytes_with_integrity(&self, key: &[u8]) -> Vec<u8> {
        let mut body = BytesMut::new();
        for attr in &self.attributes {
            encode_attr(&mut body, attr, &self.transaction_id);
        }

        // Temporarily set length to include MI (24 bytes).
        let mi_len = (body.len() + 24) as u16;
        let mut hdr = BytesMut::with_capacity(20);
        hdr.put_u16(self.message_type);
        hdr.put_u16(mi_len);
        hdr.put_u32(STUN_MAGIC_COOKIE);
        hdr.put_slice(&self.transaction_id);

        let mut tmp = Vec::with_capacity(hdr.len() + body.len());
        tmp.extend_from_slice(&hdr);
        tmp.extend_from_slice(&body);

        let hmac = compute_hmac_sha1(key, &tmp);
        body.put_u16(ATTR_MESSAGE_INTEGRITY);
        body.put_u16(20);
        body.put_slice(&hmac);

        // Fingerprint (8 bytes total).
        let final_len = (body.len() + 8) as u16;
        let mut out = BytesMut::with_capacity(20 + body.len() + 8);
        out.put_u16(self.message_type);
        out.put_u16(final_len);
        out.put_u32(STUN_MAGIC_COOKIE);
        out.put_slice(&self.transaction_id);
        out.put_slice(&body);

        let crc = crc32fast::hash(&out) ^ FINGERPRINT_XOR;
        out.put_u16(ATTR_FINGERPRINT);
        out.put_u16(4);
        out.put_u32(crc);

        out.to_vec()
    }

    // ── Accessors ──

    pub fn get_username(&self) -> Option<&str> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::Username(u) => Some(u.as_str()),
            _ => None,
        })
    }

    pub fn get_realm(&self) -> Option<&str> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::Realm(r) => Some(r.as_str()),
            _ => None,
        })
    }

    pub fn get_nonce(&self) -> Option<&str> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::Nonce(n) => Some(n.as_str()),
            _ => None,
        })
    }

    pub fn get_message_integrity(&self) -> Option<&[u8]> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::MessageIntegrity(m) => Some(m.as_slice()),
            _ => None,
        })
    }

    pub fn get_lifetime(&self) -> Option<u32> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::Lifetime(l) => Some(*l),
            _ => None,
        })
    }

    pub fn get_xor_peer_address(&self) -> Option<SocketAddr> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::XorPeerAddress(a) => Some(*a),
            _ => None,
        })
    }

    pub fn get_channel_number(&self) -> Option<u16> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::ChannelNumber(c) => Some(*c),
            _ => None,
        })
    }

    pub fn get_data(&self) -> Option<&[u8]> {
        self.attributes.iter().find_map(|a| match a {
            StunAttribute::Data(d) => Some(d.as_slice()),
            _ => None,
        })
    }
}

// ───────────────────── Attribute encoding ─────────────────────

fn encode_attr(buf: &mut BytesMut, attr: &StunAttribute, txn: &[u8; 12]) {
    match attr {
        StunAttribute::XorMappedAddress(addr) => {
            let d = encode_xor_addr(*addr, txn);
            buf.put_u16(ATTR_XOR_MAPPED_ADDRESS);
            buf.put_u16(d.len() as u16);
            buf.put_slice(&d);
        }
        StunAttribute::XorRelayedAddress(addr) => {
            let d = encode_xor_addr(*addr, txn);
            buf.put_u16(ATTR_XOR_RELAYED_ADDRESS);
            buf.put_u16(d.len() as u16);
            buf.put_slice(&d);
        }
        StunAttribute::XorPeerAddress(addr) => {
            let d = encode_xor_addr(*addr, txn);
            buf.put_u16(ATTR_XOR_PEER_ADDRESS);
            buf.put_u16(d.len() as u16);
            buf.put_slice(&d);
        }
        StunAttribute::MappedAddress(addr) => {
            let d = encode_plain_addr(*addr);
            buf.put_u16(ATTR_MAPPED_ADDRESS);
            buf.put_u16(d.len() as u16);
            buf.put_slice(&d);
        }
        StunAttribute::Username(s) => write_string_attr(buf, ATTR_USERNAME, s),
        StunAttribute::Realm(s) => write_string_attr(buf, ATTR_REALM, s),
        StunAttribute::Nonce(s) => write_string_attr(buf, ATTR_NONCE, s),
        StunAttribute::Software(s) => write_string_attr(buf, ATTR_SOFTWARE, s),
        StunAttribute::ErrorCode { code, reason } => {
            let cls = (code / 100) as u8;
            let num = (code % 100) as u8;
            let rb = reason.as_bytes();
            let alen = 4 + rb.len();
            buf.put_u16(ATTR_ERROR_CODE);
            buf.put_u16(alen as u16);
            buf.put_u16(0);
            buf.put_u8(cls);
            buf.put_u8(num);
            buf.put_slice(rb);
            pad4(buf, alen);
        }
        StunAttribute::Lifetime(secs) => {
            buf.put_u16(ATTR_LIFETIME);
            buf.put_u16(4);
            buf.put_u32(*secs);
        }
        StunAttribute::Data(d) => {
            buf.put_u16(ATTR_DATA);
            buf.put_u16(d.len() as u16);
            buf.put_slice(d);
            pad4(buf, d.len());
        }
        StunAttribute::RequestedTransport(proto) => {
            buf.put_u16(ATTR_REQUESTED_TRANSPORT);
            buf.put_u16(4);
            buf.put_u8(*proto);
            buf.put_u8(0);
            buf.put_u8(0);
            buf.put_u8(0);
        }
        StunAttribute::ChannelNumber(ch) => {
            buf.put_u16(ATTR_CHANNEL_NUMBER);
            buf.put_u16(4);
            buf.put_u16(*ch);
            buf.put_u16(0);
        }
        StunAttribute::MessageIntegrity(m) => {
            buf.put_u16(ATTR_MESSAGE_INTEGRITY);
            buf.put_u16(m.len() as u16);
            buf.put_slice(m);
        }
        StunAttribute::Fingerprint(fp) => {
            buf.put_u16(ATTR_FINGERPRINT);
            buf.put_u16(4);
            buf.put_u32(*fp);
        }
        StunAttribute::Unknown { attr_type, data } => {
            buf.put_u16(*attr_type);
            buf.put_u16(data.len() as u16);
            buf.put_slice(data);
            pad4(buf, data.len());
        }
    }
}

fn write_string_attr(buf: &mut BytesMut, attr_type: u16, s: &str) {
    let b = s.as_bytes();
    buf.put_u16(attr_type);
    buf.put_u16(b.len() as u16);
    buf.put_slice(b);
    pad4(buf, b.len());
}

fn pad4(buf: &mut BytesMut, len: usize) {
    let rem = len % 4;
    if rem != 0 {
        for _ in 0..(4 - rem) {
            buf.put_u8(0);
        }
    }
}

// ───────────────────── Address helpers ─────────────────────

fn encode_xor_addr(addr: SocketAddr, txn: &[u8; 12]) -> Vec<u8> {
    let ck = STUN_MAGIC_COOKIE.to_be_bytes();
    match addr {
        SocketAddr::V4(v4) => {
            let port = v4.port() ^ (STUN_MAGIC_COOKIE >> 16) as u16;
            let ip = v4.ip().octets();
            let mut out = vec![0u8; 8];
            out[1] = 0x01;
            out[2..4].copy_from_slice(&port.to_be_bytes());
            for i in 0..4 {
                out[4 + i] = ip[i] ^ ck[i];
            }
            out
        }
        SocketAddr::V6(v6) => {
            let port = v6.port() ^ (STUN_MAGIC_COOKIE >> 16) as u16;
            let ip = v6.ip().octets();
            let mut out = vec![0u8; 20];
            out[1] = 0x02;
            out[2..4].copy_from_slice(&port.to_be_bytes());
            for i in 0..4 {
                out[4 + i] = ip[i] ^ ck[i];
            }
            for i in 0..12 {
                out[8 + i] = ip[4 + i] ^ txn[i];
            }
            out
        }
    }
}

fn encode_plain_addr(addr: SocketAddr) -> Vec<u8> {
    match addr {
        SocketAddr::V4(v4) => {
            let mut out = vec![0u8; 8];
            out[1] = 0x01;
            out[2..4].copy_from_slice(&v4.port().to_be_bytes());
            out[4..8].copy_from_slice(&v4.ip().octets());
            out
        }
        SocketAddr::V6(v6) => {
            let mut out = vec![0u8; 20];
            out[1] = 0x02;
            out[2..4].copy_from_slice(&v6.port().to_be_bytes());
            out[4..20].copy_from_slice(&v6.ip().octets());
            out
        }
    }
}

fn parse_address(data: &[u8], xor: bool, txn: &[u8; 12]) -> Option<SocketAddr> {
    if data.len() < 8 {
        return None;
    }
    let family = data[1];
    let raw_port = u16::from_be_bytes([data[2], data[3]]);
    let port = if xor {
        raw_port ^ (STUN_MAGIC_COOKIE >> 16) as u16
    } else {
        raw_port
    };

    match family {
        0x01 => {
            let ck = STUN_MAGIC_COOKIE.to_be_bytes();
            let mut ip = [data[4], data[5], data[6], data[7]];
            if xor {
                for i in 0..4 {
                    ip[i] ^= ck[i];
                }
            }
            Some(SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
                port,
            ))
        }
        0x02 if data.len() >= 20 => {
            let mut ip = [0u8; 16];
            ip.copy_from_slice(&data[4..20]);
            if xor {
                let ck = STUN_MAGIC_COOKIE.to_be_bytes();
                for i in 0..4 {
                    ip[i] ^= ck[i];
                }
                for i in 0..12 {
                    ip[4 + i] ^= txn[i];
                }
            }
            Some(SocketAddr::new(
                IpAddr::V6(std::net::Ipv6Addr::from(ip)),
                port,
            ))
        }
        _ => None,
    }
}

// ───────────────────── HMAC-SHA1 / CRC-32 ─────────────────────

fn compute_hmac_sha1(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha1::new_from_slice(key).expect("HMAC-SHA1 key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Compute the STUN FINGERPRINT value (CRC-32 XOR 0x5354554E).
pub fn compute_fingerprint(data: &[u8]) -> u32 {
    crc32fast::hash(data) ^ FINGERPRINT_XOR
}

// ───────────────────── Credential provider ─────────────────────

/// Trait for validating TURN credentials.
pub trait CredentialProvider: Send + Sync {
    /// Validate long-term credentials.  Returns the HMAC key if valid.
    fn validate_long_term(
        &self,
        username: &str,
        realm: &str,
        nonce: &str,
    ) -> Option<Vec<u8>>;

    /// Generate ephemeral credentials for a WebRTC client.
    fn generate_ephemeral(&self, extension_id: &str) -> (String, String, u32);
}

/// Credential provider backed by the TURN config (static users + shared secret).
pub struct ConfigCredentialProvider {
    /// username -> HMAC key
    static_keys: HashMap<String, Vec<u8>>,
    realm: String,
    shared_secret: Option<String>,
    nonces: DashMap<String, Instant>,
}

impl ConfigCredentialProvider {
    pub fn from_config(config: &TurnConfig) -> Self {
        let mut static_keys = HashMap::new();
        for user in &config.users {
            // Use HMAC-SHA1(password, username:realm) as key for simplicity.
            let key = compute_hmac_sha1(
                user.password.as_bytes(),
                format!("{}:{}", user.username, config.realm).as_bytes(),
            );
            static_keys.insert(user.username.clone(), key);
        }
        ConfigCredentialProvider {
            static_keys,
            realm: config.realm.clone(),
            shared_secret: config.shared_secret.clone(),
            nonces: DashMap::new(),
        }
    }

    /// Generate a fresh nonce string.
    pub fn generate_nonce(&self) -> String {
        let mut rng = rand::thread_rng();
        let nonce: String = (0..24)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect();
        self.nonces.insert(nonce.clone(), Instant::now());
        nonce
    }

    fn validate_nonce(&self, nonce: &str) -> bool {
        self.nonces
            .get(nonce)
            .map(|t| t.elapsed() < Duration::from_secs(3600))
            .unwrap_or(false)
    }
}

impl CredentialProvider for ConfigCredentialProvider {
    fn validate_long_term(&self, username: &str, _realm: &str, nonce: &str) -> Option<Vec<u8>> {
        if !self.validate_nonce(nonce) {
            return None;
        }

        // Static credentials.
        if let Some(key) = self.static_keys.get(username) {
            return Some(key.clone());
        }

        // Ephemeral credentials (username = "expiry_timestamp:id").
        if let Some(secret) = &self.shared_secret {
            if let Some(ts_str) = username.split(':').next() {
                if let Ok(ts) = ts_str.parse::<u64>() {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    if ts > now {
                        let key = compute_hmac_sha1(secret.as_bytes(), username.as_bytes());
                        return Some(key);
                    }
                }
            }
        }

        None
    }

    fn generate_ephemeral(&self, extension_id: &str) -> (String, String, u32) {
        let ttl = 86400u32;
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            + ttl as u64;
        let username = format!("{expiry}:{extension_id}");
        let secret = self.shared_secret.as_deref().unwrap_or("default-secret");
        let password_raw = compute_hmac_sha1(secret.as_bytes(), username.as_bytes());
        let password = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &password_raw,
        );
        (username, password, ttl)
    }
}

// ───────────────────── Allocation ─────────────────────

/// Key identifying a TURN allocation.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct AllocationKey {
    pub client_addr: SocketAddr,
    pub protocol: u8,
}

/// A TURN relay allocation.
#[derive(Debug)]
pub struct Allocation {
    pub relay_addr: SocketAddr,
    pub relay_socket: Arc<UdpSocket>,
    pub permissions: DashMap<IpAddr, Instant>,
    pub channels: DashMap<u16, SocketAddr>,
    pub created_at: Instant,
    pub expires_at: Instant,
    pub username: String,
}

impl Allocation {
    fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }

    fn has_permission(&self, ip: &IpAddr) -> bool {
        self.permissions
            .get(ip)
            .map(|t| Instant::now() < *t)
            .unwrap_or(false)
    }
}

// ───────────────────── IP rate limiter ─────────────────────

struct IpRateLimiter {
    counts: DashMap<IpAddr, (Instant, u32)>,
    max_per_sec: u32,
}

impl IpRateLimiter {
    fn new(max_per_sec: u32) -> Self {
        IpRateLimiter {
            counts: DashMap::new(),
            max_per_sec,
        }
    }

    fn check(&self, ip: &IpAddr) -> bool {
        let now = Instant::now();
        let mut entry = self.counts.entry(*ip).or_insert((now, 0));
        let (start, count) = entry.value_mut();
        if now.duration_since(*start) > Duration::from_secs(1) {
            *start = now;
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

// ───────────────────── TURN server ─────────────────────

/// Embedded STUN / TURN server.
pub struct TurnServer {
    pub allocations: DashMap<AllocationKey, Allocation>,
    pub credentials: Arc<dyn CredentialProvider>,
    pub config: TurnConfig,
    rate_limiter: IpRateLimiter,
    pub shutdown: Arc<Notify>,
    running: AtomicBool,
    next_relay_port: AtomicU16,
}

impl TurnServer {
    pub fn new(config: TurnConfig, credentials: Arc<dyn CredentialProvider>) -> Arc<Self> {
        let rl = IpRateLimiter::new(config.rate_limit_per_sec);
        Arc::new(TurnServer {
            allocations: DashMap::new(),
            credentials,
            rate_limiter: rl,
            next_relay_port: AtomicU16::new(config.relay_port_start),
            config,
            shutdown: Arc::new(Notify::new()),
            running: AtomicBool::new(false),
        })
    }

    /// Check whether the TURN server is currently running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }

    /// Start the TURN server on the configured UDP port.
    pub async fn start(self: &Arc<Self>, bind_addr: &str) -> Result<JoinHandle<()>, TurnError> {
        let addr = format!("{bind_addr}:{}", self.config.port);
        let socket = Arc::new(UdpSocket::bind(&addr).await?);
        self.running.store(true, Ordering::Release);
        info!(addr = %addr, "TURN server listening (UDP)");

        // Periodic cleanup task.
        let cleanup = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                cleanup.cleanup_expired_allocations();
            }
        });

        // Main receive loop.
        let server = Arc::clone(self);
        let sock = Arc::clone(&socket);
        let handle = tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                tokio::select! {
                    result = sock.recv_from(&mut buf) => {
                        match result {
                            Ok((len, src)) => {
                                let data = buf[..len].to_vec();
                                let srv = Arc::clone(&server);
                                let s = Arc::clone(&sock);
                                tokio::spawn(async move {
                                    if let Err(e) = srv.handle_packet(&data, src, &s).await {
                                        debug!(src = %src, err = %e, "Packet error");
                                    }
                                });
                            }
                            Err(e) => error!(err = %e, "UDP recv error"),
                        }
                    }
                    _ = server.shutdown.notified() => {
                        info!("TURN server shutting down");
                        break;
                    }
                }
            }
        });

        Ok(handle)
    }

    /// Signal the server to stop.
    pub fn stop(&self) {
        self.shutdown.notify_waiters();
    }

    /// Process a single incoming packet.
    async fn handle_packet(
        &self,
        data: &[u8],
        src: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), TurnError> {
        if !self.rate_limiter.check(&src.ip()) {
            return Err(TurnError::RateLimited);
        }

        // ChannelData (first two bits = 01).
        if data.len() >= 4 && (data[0] & 0xC0) == 0x40 {
            return self.handle_channel_data(data, src).await;
        }

        let msg = StunMessage::parse(data)?;

        match msg.message_type {
            BINDING_REQUEST => self.handle_binding_request(&msg, src, socket).await,
            ALLOCATE_REQUEST => self.handle_allocate_request(&msg, src, socket).await,
            REFRESH_REQUEST => self.handle_refresh_request(&msg, src, socket).await,
            CREATE_PERMISSION_REQUEST => {
                self.handle_create_permission(&msg, src, socket).await
            }
            CHANNEL_BIND_REQUEST => self.handle_channel_bind(&msg, src, socket).await,
            SEND_INDICATION => self.handle_send_indication(&msg, src).await,
            _ => {
                trace!(t = msg.message_type, "Unknown STUN type");
                Ok(())
            }
        }
    }

    // ────────────── Binding ──────────────

    async fn handle_binding_request(
        &self,
        msg: &StunMessage,
        src: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), TurnError> {
        let resp = StunMessage {
            message_type: BINDING_RESPONSE,
            transaction_id: msg.transaction_id,
            attributes: vec![
                StunAttribute::XorMappedAddress(src),
                StunAttribute::Software("voip-engine TURN".into()),
            ],
        };
        socket.send_to(&resp.to_bytes(), src).await?;
        trace!(src = %src, "Binding response");
        Ok(())
    }

    // ────────────── Allocate ──────────────

    async fn handle_allocate_request(
        &self,
        msg: &StunMessage,
        src: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), TurnError> {
        let key = match self.authenticate(msg) {
            Some(k) => k,
            None => {
                self.send_error(socket, src, msg, ALLOCATE_ERROR, 401, "Unauthorized")
                    .await?;
                return Ok(());
            }
        };

        let akey = AllocationKey {
            client_addr: src,
            protocol: 17,
        };

        if self.allocations.contains_key(&akey) {
            self.send_error_mi(socket, src, msg, ALLOCATE_ERROR, 437, "Allocation Mismatch", &key)
                .await?;
            return Ok(());
        }

        let ip_count = self
            .allocations
            .iter()
            .filter(|a| a.key().client_addr.ip() == src.ip())
            .count();
        if ip_count >= self.config.max_allocations_per_ip {
            self.send_error_mi(socket, src, msg, ALLOCATE_ERROR, 486, "Quota Reached", &key)
                .await?;
            return Ok(());
        }

        let relay_port = self.next_relay_port();
        let relay_socket = match UdpSocket::bind(format!("0.0.0.0:{relay_port}")).await {
            Ok(s) => Arc::new(s),
            Err(e) => {
                error!(port = relay_port, err = %e, "Relay bind failed");
                self.send_error_mi(
                    socket, src, msg, ALLOCATE_ERROR, 508, "Insufficient Capacity", &key,
                )
                .await?;
                return Ok(());
            }
        };

        let relay_addr = relay_socket.local_addr()?;
        let lifetime = msg
            .get_lifetime()
            .unwrap_or(DEFAULT_LIFETIME)
            .min(self.config.allocation_lifetime_sec as u32);

        let alloc = Allocation {
            relay_addr,
            relay_socket: Arc::clone(&relay_socket),
            permissions: DashMap::new(),
            channels: DashMap::new(),
            created_at: Instant::now(),
            expires_at: Instant::now() + Duration::from_secs(lifetime as u64),
            username: msg.get_username().unwrap_or("").into(),
        };

        self.allocations.insert(akey, alloc);

        // Spawn task that reads from the relay socket and sends DATA indications
        // back to the client.
        let client_socket = Arc::clone(socket);
        let client_addr = src;
        tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                match relay_socket.recv_from(&mut buf).await {
                    Ok((len, peer)) => {
                        let txn: [u8; 12] = rand::thread_rng().gen();
                        let ind = StunMessage {
                            message_type: DATA_INDICATION,
                            transaction_id: txn,
                            attributes: vec![
                                StunAttribute::XorPeerAddress(peer),
                                StunAttribute::Data(buf[..len].to_vec()),
                            ],
                        };
                        let _ = client_socket.send_to(&ind.to_bytes(), client_addr).await;
                    }
                    Err(_) => break,
                }
            }
        });

        let resp = StunMessage {
            message_type: ALLOCATE_RESPONSE,
            transaction_id: msg.transaction_id,
            attributes: vec![
                StunAttribute::XorRelayedAddress(relay_addr),
                StunAttribute::XorMappedAddress(src),
                StunAttribute::Lifetime(lifetime),
                StunAttribute::Software("voip-engine TURN".into()),
            ],
        };
        socket
            .send_to(&resp.to_bytes_with_integrity(&key), src)
            .await?;
        info!(src = %src, relay = %relay_addr, lifetime, "Allocation created");
        Ok(())
    }

    // ────────────── Refresh ──────────────

    async fn handle_refresh_request(
        &self,
        msg: &StunMessage,
        src: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), TurnError> {
        let key = match self.authenticate(msg) {
            Some(k) => k,
            None => {
                self.send_error(socket, src, msg, REFRESH_ERROR, 401, "Unauthorized")
                    .await?;
                return Ok(());
            }
        };

        let akey = AllocationKey {
            client_addr: src,
            protocol: 17,
        };
        let lifetime = msg.get_lifetime().unwrap_or(DEFAULT_LIFETIME);

        if lifetime == 0 {
            self.allocations.remove(&akey);
            info!(src = %src, "Allocation deleted (refresh 0)");
        } else if let Some(mut a) = self.allocations.get_mut(&akey) {
            a.expires_at = Instant::now() + Duration::from_secs(lifetime.min(3600) as u64);
            debug!(src = %src, lifetime, "Allocation refreshed");
        } else {
            self.send_error_mi(socket, src, msg, REFRESH_ERROR, 437, "Allocation Mismatch", &key)
                .await?;
            return Ok(());
        }

        let resp = StunMessage {
            message_type: REFRESH_RESPONSE,
            transaction_id: msg.transaction_id,
            attributes: vec![StunAttribute::Lifetime(lifetime.min(3600))],
        };
        socket
            .send_to(&resp.to_bytes_with_integrity(&key), src)
            .await?;
        Ok(())
    }

    // ────────────── CreatePermission ──────────────

    async fn handle_create_permission(
        &self,
        msg: &StunMessage,
        src: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), TurnError> {
        let key = match self.authenticate(msg) {
            Some(k) => k,
            None => {
                self.send_error(socket, src, msg, CREATE_PERMISSION_ERROR, 401, "Unauthorized")
                    .await?;
                return Ok(());
            }
        };

        let peer = msg
            .get_xor_peer_address()
            .ok_or_else(|| TurnError::Parse("Missing XOR-PEER-ADDRESS".into()))?;

        let akey = AllocationKey {
            client_addr: src,
            protocol: 17,
        };

        if let Some(a) = self.allocations.get(&akey) {
            a.permissions
                .insert(peer.ip(), Instant::now() + Duration::from_secs(300));
            debug!(src = %src, peer = %peer.ip(), "Permission created");
        } else {
            self.send_error_mi(
                socket, src, msg, CREATE_PERMISSION_ERROR, 437, "Allocation Mismatch", &key,
            )
            .await?;
            return Ok(());
        }

        let resp = StunMessage {
            message_type: CREATE_PERMISSION_RESPONSE,
            transaction_id: msg.transaction_id,
            attributes: Vec::new(),
        };
        socket
            .send_to(&resp.to_bytes_with_integrity(&key), src)
            .await?;
        Ok(())
    }

    // ────────────── ChannelBind ──────────────

    async fn handle_channel_bind(
        &self,
        msg: &StunMessage,
        src: SocketAddr,
        socket: &Arc<UdpSocket>,
    ) -> Result<(), TurnError> {
        let key = match self.authenticate(msg) {
            Some(k) => k,
            None => {
                self.send_error(socket, src, msg, CHANNEL_BIND_ERROR, 401, "Unauthorized")
                    .await?;
                return Ok(());
            }
        };

        let channel = msg
            .get_channel_number()
            .ok_or_else(|| TurnError::Parse("Missing CHANNEL-NUMBER".into()))?;
        let peer = msg
            .get_xor_peer_address()
            .ok_or_else(|| TurnError::Parse("Missing XOR-PEER-ADDRESS".into()))?;

        if !(0x4000..=0x7FFE).contains(&channel) {
            self.send_error_mi(
                socket, src, msg, CHANNEL_BIND_ERROR, 400, "Bad Channel", &key,
            )
            .await?;
            return Ok(());
        }

        let akey = AllocationKey {
            client_addr: src,
            protocol: 17,
        };

        if let Some(a) = self.allocations.get(&akey) {
            a.channels.insert(channel, peer);
            a.permissions
                .insert(peer.ip(), Instant::now() + Duration::from_secs(600));
            debug!(src = %src, channel, peer = %peer, "Channel bound");
        } else {
            self.send_error_mi(
                socket, src, msg, CHANNEL_BIND_ERROR, 437, "Allocation Mismatch", &key,
            )
            .await?;
            return Ok(());
        }

        let resp = StunMessage {
            message_type: CHANNEL_BIND_RESPONSE,
            transaction_id: msg.transaction_id,
            attributes: Vec::new(),
        };
        socket
            .send_to(&resp.to_bytes_with_integrity(&key), src)
            .await?;
        Ok(())
    }

    // ────────────── Send Indication ──────────────

    async fn handle_send_indication(
        &self,
        msg: &StunMessage,
        src: SocketAddr,
    ) -> Result<(), TurnError> {
        let peer = msg
            .get_xor_peer_address()
            .ok_or_else(|| TurnError::Parse("Missing XOR-PEER-ADDRESS".into()))?;
        let payload = msg
            .get_data()
            .ok_or_else(|| TurnError::Parse("Missing DATA".into()))?;

        let akey = AllocationKey {
            client_addr: src,
            protocol: 17,
        };

        if let Some(a) = self.allocations.get(&akey) {
            if !a.has_permission(&peer.ip()) {
                warn!(src = %src, peer = %peer, "No permission");
                return Ok(());
            }
            a.relay_socket.send_to(payload, peer).await?;
            trace!(src = %src, peer = %peer, len = payload.len(), "Relayed");
        }
        Ok(())
    }

    // ────────────── ChannelData ──────────────

    async fn handle_channel_data(&self, data: &[u8], src: SocketAddr) -> Result<(), TurnError> {
        if data.len() < 4 {
            return Err(TurnError::Parse("ChannelData too short".into()));
        }
        let channel = u16::from_be_bytes([data[0], data[1]]);
        let length = u16::from_be_bytes([data[2], data[3]]) as usize;
        if data.len() < 4 + length {
            return Err(TurnError::Parse("ChannelData truncated".into()));
        }

        let akey = AllocationKey {
            client_addr: src,
            protocol: 17,
        };

        if let Some(a) = self.allocations.get(&akey) {
            if let Some(peer) = a.channels.get(&channel) {
                a.relay_socket.send_to(&data[4..4 + length], *peer).await?;
            }
        }
        Ok(())
    }

    // ────────────── Helpers ──────────────

    fn authenticate(&self, msg: &StunMessage) -> Option<Vec<u8>> {
        let username = msg.get_username()?;
        let realm = msg.get_realm()?;
        let nonce = msg.get_nonce()?;
        let _mi = msg.get_message_integrity()?;
        self.credentials.validate_long_term(username, realm, nonce)
    }

    async fn send_error(
        &self,
        socket: &Arc<UdpSocket>,
        dst: SocketAddr,
        req: &StunMessage,
        err_type: u16,
        code: u16,
        reason: &str,
    ) -> Result<(), TurnError> {
        let mut attrs = vec![StunAttribute::ErrorCode {
            code,
            reason: reason.into(),
        }];
        if code == 401 {
            attrs.push(StunAttribute::Realm(self.config.realm.clone()));
            let nonce: String = {
                let mut rng = rand::thread_rng();
                (0..24)
                    .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
                    .collect()
            };
            attrs.push(StunAttribute::Nonce(nonce));
        }
        let resp = StunMessage {
            message_type: err_type,
            transaction_id: req.transaction_id,
            attributes: attrs,
        };
        socket.send_to(&resp.to_bytes(), dst).await?;
        Ok(())
    }

    async fn send_error_mi(
        &self,
        socket: &Arc<UdpSocket>,
        dst: SocketAddr,
        req: &StunMessage,
        err_type: u16,
        code: u16,
        reason: &str,
        key: &[u8],
    ) -> Result<(), TurnError> {
        let resp = StunMessage {
            message_type: err_type,
            transaction_id: req.transaction_id,
            attributes: vec![StunAttribute::ErrorCode {
                code,
                reason: reason.into(),
            }],
        };
        socket
            .send_to(&resp.to_bytes_with_integrity(key), dst)
            .await?;
        Ok(())
    }

    /// Remove expired allocations.
    pub fn cleanup_expired_allocations(&self) {
        let expired: Vec<AllocationKey> = self
            .allocations
            .iter()
            .filter(|a| a.is_expired())
            .map(|a| a.key().clone())
            .collect();
        for k in &expired {
            self.allocations.remove(k);
        }
        if !expired.is_empty() {
            info!(count = expired.len(), "Expired allocations cleaned");
        }
    }

    fn next_relay_port(&self) -> u16 {
        let port = self
            .next_relay_port
            .fetch_add(1, Ordering::Relaxed);
        if port >= self.config.relay_port_end {
            self.next_relay_port
                .store(self.config.relay_port_start, Ordering::Relaxed);
            self.config.relay_port_start
        } else {
            port
        }
    }

    pub fn allocation_count(&self) -> usize {
        self.allocations.len()
    }
}

// ───────────────────── Tests ─────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stun_parse_roundtrip() {
        let msg = StunMessage {
            message_type: BINDING_REQUEST,
            transaction_id: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            attributes: vec![],
        };
        let data = msg.to_bytes();
        let parsed = StunMessage::parse(&data).unwrap();
        assert_eq!(parsed.message_type, BINDING_REQUEST);
        assert_eq!(parsed.transaction_id, msg.transaction_id);
    }

    #[test]
    fn xor_address_roundtrip() {
        let addr: SocketAddr = "192.168.1.100:12345".parse().unwrap();
        let txn = [0u8; 12];
        let msg = StunMessage {
            message_type: BINDING_RESPONSE,
            transaction_id: txn,
            attributes: vec![StunAttribute::XorMappedAddress(addr)],
        };
        let data = msg.to_bytes();
        let parsed = StunMessage::parse(&data).unwrap();
        match &parsed.attributes[0] {
            StunAttribute::XorMappedAddress(a) => assert_eq!(*a, addr),
            _ => panic!("Expected XorMappedAddress"),
        }
    }

    #[test]
    fn fingerprint_deterministic() {
        let data = b"test fingerprint data";
        assert_eq!(compute_fingerprint(data), compute_fingerprint(data));
    }

    #[test]
    fn hmac_sha1_correct_length() {
        let h = compute_hmac_sha1(b"key", b"data");
        assert_eq!(h.len(), 20);
    }

    #[test]
    fn rate_limiter_works() {
        let rl = IpRateLimiter::new(3);
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(rl.check(&ip));
        assert!(rl.check(&ip));
        assert!(rl.check(&ip));
        assert!(!rl.check(&ip));
    }

    #[test]
    fn parse_too_short() {
        assert!(StunMessage::parse(&[0u8; 10]).is_err());
    }

    #[test]
    fn parse_bad_cookie() {
        let mut data = vec![0u8; 20];
        data[0] = 0x00;
        data[1] = 0x01;
        data[4..8].copy_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        assert!(StunMessage::parse(&data).is_err());
    }
}
