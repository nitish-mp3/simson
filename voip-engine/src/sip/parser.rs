use nom::{
    branch::alt,
    bytes::complete::{tag_no_case, take_while, take_while1},
    character::complete::{char, crlf, digit1, space0, space1},
    combinator::recognize,
    sequence::tuple,
    IResult,
};
use std::collections::HashMap;
use std::fmt;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Constants & limits
// ---------------------------------------------------------------------------

/// Maximum allowed SIP message size in bytes (64 KB).
pub const MAX_MESSAGE_SIZE: usize = 65_536;
/// Maximum number of headers we will accept.
const MAX_HEADER_COUNT: usize = 100;
/// Maximum length of a single URI.
const MAX_URI_LENGTH: usize = 4096;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
pub enum SipParseError {
    #[error("Message exceeds maximum size of {max} bytes (got {actual})")]
    MessageTooLarge { max: usize, actual: usize },
    #[error("Malformed SIP message: {0}")]
    Malformed(String),
    #[error("Unsupported SIP version: {0}")]
    UnsupportedVersion(String),
    #[error("Invalid header: {0}")]
    InvalidHeader(String),
    #[error("Missing required header: {0}")]
    MissingHeader(String),
    #[error("Too many headers ({0})")]
    TooManyHeaders(usize),
    #[error("URI too long ({0} bytes)")]
    UriTooLong(usize),
    #[error("Parse error")]
    NomError,
}

// ---------------------------------------------------------------------------
// SipMethod
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SipMethod {
    Register,
    Invite,
    Ack,
    Bye,
    Cancel,
    Options,
    Subscribe,
    Notify,
    Refer,
    Info,
    Update,
    Message,
    Prack,
}

impl SipMethod {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "REGISTER" => Some(SipMethod::Register),
            "INVITE" => Some(SipMethod::Invite),
            "ACK" => Some(SipMethod::Ack),
            "BYE" => Some(SipMethod::Bye),
            "CANCEL" => Some(SipMethod::Cancel),
            "OPTIONS" => Some(SipMethod::Options),
            "SUBSCRIBE" => Some(SipMethod::Subscribe),
            "NOTIFY" => Some(SipMethod::Notify),
            "REFER" => Some(SipMethod::Refer),
            "INFO" => Some(SipMethod::Info),
            "UPDATE" => Some(SipMethod::Update),
            "MESSAGE" => Some(SipMethod::Message),
            "PRACK" => Some(SipMethod::Prack),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            SipMethod::Register => "REGISTER",
            SipMethod::Invite => "INVITE",
            SipMethod::Ack => "ACK",
            SipMethod::Bye => "BYE",
            SipMethod::Cancel => "CANCEL",
            SipMethod::Options => "OPTIONS",
            SipMethod::Subscribe => "SUBSCRIBE",
            SipMethod::Notify => "NOTIFY",
            SipMethod::Refer => "REFER",
            SipMethod::Info => "INFO",
            SipMethod::Update => "UPDATE",
            SipMethod::Message => "MESSAGE",
            SipMethod::Prack => "PRACK",
        }
    }
}

impl fmt::Display for SipMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// SipUri
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SipUri {
    pub scheme: String,
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub parameters: HashMap<String, Option<String>>,
    pub headers: HashMap<String, String>,
}

impl SipUri {
    pub fn new(scheme: &str, host: &str) -> Self {
        SipUri {
            scheme: scheme.to_string(),
            user: None,
            host: host.to_string(),
            port: None,
            parameters: HashMap::new(),
            headers: HashMap::new(),
        }
    }

    pub fn transport(&self) -> Option<&str> {
        self.parameters.get("transport").and_then(|v| v.as_deref())
    }
}

impl fmt::Display for SipUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:", self.scheme)?;
        if let Some(ref u) = self.user {
            write!(f, "{}@", u)?;
        }
        write!(f, "{}", self.host)?;
        if let Some(p) = self.port {
            write!(f, ":{}", p)?;
        }
        for (k, v) in &self.parameters {
            match v {
                Some(val) => write!(f, ";{}={}", k, val)?,
                None => write!(f, ";{}", k)?,
            }
        }
        if !self.headers.is_empty() {
            let mut first = true;
            for (k, v) in &self.headers {
                write!(f, "{}{}={}", if first { '?' } else { '&' }, k, v)?;
                first = false;
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SipHeader (typed)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SipHeader {
    Via {
        protocol: String,
        transport: String,
        host: String,
        port: Option<u16>,
        branch: Option<String>,
        rport: Option<u16>,
        received: Option<String>,
    },
    From {
        display_name: Option<String>,
        uri: SipUri,
        tag: Option<String>,
    },
    To {
        display_name: Option<String>,
        uri: SipUri,
        tag: Option<String>,
    },
    CallId(String),
    CSeq {
        seq: u32,
        method: SipMethod,
    },
    Contact {
        display_name: Option<String>,
        uri: SipUri,
        params: HashMap<String, Option<String>>,
    },
    ContentType(String),
    ContentLength(usize),
    MaxForwards(u32),
    UserAgent(String),
    Allow(Vec<SipMethod>),
    Supported(Vec<String>),
    WwwAuthenticate {
        realm: String,
        nonce: String,
        algorithm: Option<String>,
        qop: Option<String>,
        opaque: Option<String>,
    },
    Authorization {
        username: String,
        realm: String,
        nonce: String,
        uri: String,
        response: String,
        algorithm: Option<String>,
        cnonce: Option<String>,
        nc: Option<String>,
        qop: Option<String>,
    },
    Other {
        name: String,
        value: String,
    },
}

impl SipHeader {
    /// Return the canonical header name.
    pub fn name(&self) -> &str {
        match self {
            SipHeader::Via { .. } => "Via",
            SipHeader::From { .. } => "From",
            SipHeader::To { .. } => "To",
            SipHeader::CallId(_) => "Call-ID",
            SipHeader::CSeq { .. } => "CSeq",
            SipHeader::Contact { .. } => "Contact",
            SipHeader::ContentType(_) => "Content-Type",
            SipHeader::ContentLength(_) => "Content-Length",
            SipHeader::MaxForwards(_) => "Max-Forwards",
            SipHeader::UserAgent(_) => "User-Agent",
            SipHeader::Allow(_) => "Allow",
            SipHeader::Supported(_) => "Supported",
            SipHeader::WwwAuthenticate { .. } => "WWW-Authenticate",
            SipHeader::Authorization { .. } => "Authorization",
            SipHeader::Other { ref name, .. } => name.as_str(),
        }
    }
}

impl fmt::Display for SipHeader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipHeader::Via {
                protocol,
                transport,
                host,
                port,
                branch,
                rport,
                received,
            } => {
                write!(f, "Via: {}/{} {}", protocol, transport, host)?;
                if let Some(p) = port {
                    write!(f, ":{}", p)?;
                }
                if let Some(b) = branch {
                    write!(f, ";branch={}", b)?;
                }
                if let Some(r) = rport {
                    write!(f, ";rport={}", r)?;
                }
                if let Some(r) = received {
                    write!(f, ";received={}", r)?;
                }
                Ok(())
            }
            SipHeader::From {
                display_name,
                uri,
                tag,
            } => {
                write!(f, "From: ")?;
                if let Some(dn) = display_name {
                    write!(f, "\"{}\" ", dn)?;
                }
                write!(f, "<{}>", uri)?;
                if let Some(t) = tag {
                    write!(f, ";tag={}", t)?;
                }
                Ok(())
            }
            SipHeader::To {
                display_name,
                uri,
                tag,
            } => {
                write!(f, "To: ")?;
                if let Some(dn) = display_name {
                    write!(f, "\"{}\" ", dn)?;
                }
                write!(f, "<{}>", uri)?;
                if let Some(t) = tag {
                    write!(f, ";tag={}", t)?;
                }
                Ok(())
            }
            SipHeader::CallId(id) => write!(f, "Call-ID: {}", id),
            SipHeader::CSeq { seq, method } => write!(f, "CSeq: {} {}", seq, method),
            SipHeader::Contact {
                display_name,
                uri,
                params,
            } => {
                write!(f, "Contact: ")?;
                if let Some(dn) = display_name {
                    write!(f, "\"{}\" ", dn)?;
                }
                write!(f, "<{}>", uri)?;
                for (k, v) in params {
                    match v {
                        Some(val) => write!(f, ";{}={}", k, val)?,
                        None => write!(f, ";{}", k)?,
                    }
                }
                Ok(())
            }
            SipHeader::ContentType(ct) => write!(f, "Content-Type: {}", ct),
            SipHeader::ContentLength(len) => write!(f, "Content-Length: {}", len),
            SipHeader::MaxForwards(mf) => write!(f, "Max-Forwards: {}", mf),
            SipHeader::UserAgent(ua) => write!(f, "User-Agent: {}", ua),
            SipHeader::Allow(methods) => {
                write!(f, "Allow: ")?;
                let s: Vec<String> = methods.iter().map(|m| m.to_string()).collect();
                write!(f, "{}", s.join(", "))
            }
            SipHeader::Supported(exts) => write!(f, "Supported: {}", exts.join(", ")),
            SipHeader::WwwAuthenticate {
                realm,
                nonce,
                algorithm,
                qop,
                opaque,
            } => {
                write!(f, "WWW-Authenticate: Digest realm=\"{}\", nonce=\"{}\"", realm, nonce)?;
                if let Some(a) = algorithm {
                    write!(f, ", algorithm={}", a)?;
                }
                if let Some(q) = qop {
                    write!(f, ", qop=\"{}\"", q)?;
                }
                if let Some(o) = opaque {
                    write!(f, ", opaque=\"{}\"", o)?;
                }
                Ok(())
            }
            SipHeader::Authorization {
                username,
                realm,
                nonce,
                uri,
                response,
                algorithm,
                cnonce,
                nc,
                qop,
            } => {
                write!(
                    f,
                    "Authorization: Digest username=\"{}\", realm=\"{}\", nonce=\"{}\", uri=\"{}\", response=\"{}\"",
                    username, realm, nonce, uri, response
                )?;
                if let Some(a) = algorithm {
                    write!(f, ", algorithm={}", a)?;
                }
                if let Some(c) = cnonce {
                    write!(f, ", cnonce=\"{}\"", c)?;
                }
                if let Some(n) = nc {
                    write!(f, ", nc={}", n)?;
                }
                if let Some(q) = qop {
                    write!(f, ", qop={}", q)?;
                }
                Ok(())
            }
            SipHeader::Other { name, value } => write!(f, "{}: {}", name, value),
        }
    }
}

// ---------------------------------------------------------------------------
// SipRequest / SipResponse / SipMessage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SipRequest {
    pub method: SipMethod,
    pub uri: SipUri,
    pub version: String,
    pub headers: Vec<SipHeader>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SipResponse {
    pub version: String,
    pub status_code: u16,
    pub reason: String,
    pub headers: Vec<SipHeader>,
    pub body: Option<String>,
}

#[derive(Debug, Clone)]
pub enum SipMessage {
    Request(SipRequest),
    Response(SipResponse),
}

impl SipMessage {
    // ---- builders ----

    pub fn new_request(method: SipMethod, uri: SipUri) -> SipRequestBuilder {
        SipRequestBuilder {
            method,
            uri,
            headers: Vec::new(),
            body: None,
        }
    }

    pub fn new_response(status_code: u16, reason: &str) -> SipResponseBuilder {
        SipResponseBuilder {
            status_code,
            reason: reason.to_string(),
            headers: Vec::new(),
            body: None,
        }
    }

    // ---- accessors ----

    pub fn headers(&self) -> &[SipHeader] {
        match self {
            SipMessage::Request(r) => &r.headers,
            SipMessage::Response(r) => &r.headers,
        }
    }

    pub fn headers_mut(&mut self) -> &mut Vec<SipHeader> {
        match self {
            SipMessage::Request(r) => &mut r.headers,
            SipMessage::Response(r) => &mut r.headers,
        }
    }

    pub fn body(&self) -> Option<&str> {
        match self {
            SipMessage::Request(r) => r.body.as_deref(),
            SipMessage::Response(r) => r.body.as_deref(),
        }
    }

    pub fn is_request(&self) -> bool {
        matches!(self, SipMessage::Request(_))
    }

    pub fn is_response(&self) -> bool {
        matches!(self, SipMessage::Response(_))
    }

    pub fn method(&self) -> Option<&SipMethod> {
        match self {
            SipMessage::Request(r) => Some(&r.method),
            SipMessage::Response(_) => self.cseq_method(),
        }
    }

    pub fn status_code(&self) -> Option<u16> {
        match self {
            SipMessage::Response(r) => Some(r.status_code),
            _ => None,
        }
    }

    /// Return the Call-ID value.
    pub fn call_id(&self) -> Option<&str> {
        for h in self.headers() {
            if let SipHeader::CallId(ref id) = h {
                return Some(id.as_str());
            }
        }
        None
    }

    /// Return the CSeq sequence number.
    pub fn cseq_seq(&self) -> Option<u32> {
        for h in self.headers() {
            if let SipHeader::CSeq { seq, .. } = h {
                return Some(*seq);
            }
        }
        None
    }

    /// Return the CSeq method.
    pub fn cseq_method(&self) -> Option<&SipMethod> {
        for h in self.headers() {
            if let SipHeader::CSeq { method, .. } = h {
                return Some(method);
            }
        }
        None
    }

    /// Return the From tag.
    pub fn from_tag(&self) -> Option<&str> {
        for h in self.headers() {
            if let SipHeader::From { tag, .. } = h {
                return tag.as_deref();
            }
        }
        None
    }

    /// Return the To tag.
    pub fn to_tag(&self) -> Option<&str> {
        for h in self.headers() {
            if let SipHeader::To { tag, .. } = h {
                return tag.as_deref();
            }
        }
        None
    }

    /// Return the top Via branch parameter.
    pub fn via_branch(&self) -> Option<&str> {
        for h in self.headers() {
            if let SipHeader::Via { branch, .. } = h {
                return branch.as_deref();
            }
        }
        None
    }

    /// Return the Content-Length value.
    pub fn content_length(&self) -> Option<usize> {
        for h in self.headers() {
            if let SipHeader::ContentLength(len) = h {
                return Some(*len);
            }
        }
        None
    }

    /// Return all Via headers.
    pub fn via_headers(&self) -> Vec<&SipHeader> {
        self.headers()
            .iter()
            .filter(|h| matches!(h, SipHeader::Via { .. }))
            .collect()
    }

    /// Return the From URI.
    pub fn from_uri(&self) -> Option<&SipUri> {
        for h in self.headers() {
            if let SipHeader::From { uri, .. } = h {
                return Some(uri);
            }
        }
        None
    }

    /// Return the To URI.
    pub fn to_uri(&self) -> Option<&SipUri> {
        for h in self.headers() {
            if let SipHeader::To { uri, .. } = h {
                return Some(uri);
            }
        }
        None
    }

    /// Return the Contact URI.
    pub fn contact_uri(&self) -> Option<&SipUri> {
        for h in self.headers() {
            if let SipHeader::Contact { uri, .. } = h {
                return Some(uri);
            }
        }
        None
    }

    /// Find the first Other header with a given name (case-insensitive).
    pub fn other_header(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        for h in self.headers() {
            if let SipHeader::Other {
                name: ref n,
                ref value,
            } = h
            {
                if n.to_lowercase() == lower {
                    return Some(value.as_str());
                }
            }
        }
        None
    }

    /// Serialize to SIP wire format bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = String::with_capacity(2048);
        match self {
            SipMessage::Request(r) => {
                buf.push_str(&format!("{} {} {}\r\n", r.method, r.uri, r.version));
                for h in &r.headers {
                    buf.push_str(&h.to_string());
                    buf.push_str("\r\n");
                }
                buf.push_str("\r\n");
                if let Some(ref body) = r.body {
                    buf.push_str(body);
                }
            }
            SipMessage::Response(r) => {
                buf.push_str(&format!("{} {} {}\r\n", r.version, r.status_code, r.reason));
                for h in &r.headers {
                    buf.push_str(&h.to_string());
                    buf.push_str("\r\n");
                }
                buf.push_str("\r\n");
                if let Some(ref body) = r.body {
                    buf.push_str(body);
                }
            }
        }
        buf.into_bytes()
    }
}

impl fmt::Display for SipMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SipMessage::Request(r) => {
                writeln!(f, "{} {} {}", r.method, r.uri, r.version)?;
                for h in &r.headers {
                    writeln!(f, "{}", h)?;
                }
                writeln!(f)?;
                if let Some(ref b) = r.body {
                    write!(f, "{}", b)?;
                }
            }
            SipMessage::Response(r) => {
                writeln!(f, "{} {} {}", r.version, r.status_code, r.reason)?;
                for h in &r.headers {
                    writeln!(f, "{}", h)?;
                }
                writeln!(f)?;
                if let Some(ref b) = r.body {
                    write!(f, "{}", b)?;
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Builders
// ---------------------------------------------------------------------------

pub struct SipRequestBuilder {
    method: SipMethod,
    uri: SipUri,
    headers: Vec<SipHeader>,
    body: Option<String>,
}

impl SipRequestBuilder {
    pub fn header(mut self, header: SipHeader) -> Self {
        self.headers.push(header);
        self
    }

    pub fn body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    pub fn build(mut self) -> SipMessage {
        let body_len = self.body.as_ref().map(|b| b.len()).unwrap_or(0);
        // Ensure Content-Length is set
        let has_cl = self
            .headers
            .iter()
            .any(|h| matches!(h, SipHeader::ContentLength(_)));
        if !has_cl {
            self.headers.push(SipHeader::ContentLength(body_len));
        }
        SipMessage::Request(SipRequest {
            method: self.method,
            uri: self.uri,
            version: "SIP/2.0".to_string(),
            headers: self.headers,
            body: self.body,
        })
    }
}

pub struct SipResponseBuilder {
    status_code: u16,
    reason: String,
    headers: Vec<SipHeader>,
    body: Option<String>,
}

impl SipResponseBuilder {
    pub fn header(mut self, header: SipHeader) -> Self {
        self.headers.push(header);
        self
    }

    pub fn body(mut self, body: String) -> Self {
        self.body = Some(body);
        self
    }

    /// Copy Via, From, To, Call-ID, CSeq from a request.
    pub fn copy_headers_from(mut self, request: &SipMessage) -> Self {
        for h in request.headers() {
            match h {
                SipHeader::Via { .. }
                | SipHeader::From { .. }
                | SipHeader::To { .. }
                | SipHeader::CallId(_)
                | SipHeader::CSeq { .. } => {
                    self.headers.push(h.clone());
                }
                _ => {}
            }
        }
        self
    }

    pub fn build(mut self) -> SipMessage {
        let body_len = self.body.as_ref().map(|b| b.len()).unwrap_or(0);
        let has_cl = self
            .headers
            .iter()
            .any(|h| matches!(h, SipHeader::ContentLength(_)));
        if !has_cl {
            self.headers.push(SipHeader::ContentLength(body_len));
        }
        SipMessage::Response(SipResponse {
            version: "SIP/2.0".to_string(),
            status_code: self.status_code,
            reason: self.reason,
            headers: self.headers,
            body: self.body,
        })
    }
}

// ---------------------------------------------------------------------------
// SDP types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct SdpSession {
    pub version: u32,
    pub origin: String,
    pub session_name: String,
    pub connection: Option<String>,
    pub timing: Option<String>,
    pub media_descriptions: Vec<SdpMediaDescription>,
}

#[derive(Debug, Clone)]
pub struct SdpMediaDescription {
    pub media_type: String,
    pub port: u16,
    pub protocol: String,
    pub formats: Vec<String>,
    pub attributes: Vec<(String, Option<String>)>,
}

// ---------------------------------------------------------------------------
// SIP URI parser (nom-based)
// ---------------------------------------------------------------------------

/// Characters valid in the user-info portion of a SIP URI.
fn is_user_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')' | '%' | '+' | '&' | '='
        )
}

/// Characters valid in the host portion.
fn is_host_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':')
}

/// Characters valid in a URI parameter value.
fn is_param_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '-' | '_' | '.' | '!' | '~' | '*' | '\'' | '(' | ')' | '%' | '+' | ':'
        )
}

/// Parse a SIP/SIPS URI.  Consumes `sip:user@host:port;params?headers`.
pub fn parse_sip_uri(input: &str) -> IResult<&str, SipUri> {
    // scheme
    let (input, scheme) = alt((tag_no_case("sips"), tag_no_case("sip")))(input)?;
    let (input, _) = char(':')(input)?;

    // Grab the entire URI body up to a terminator (space, >, comma, end).
    let uri_end = input
        .find(|c: char| c == ' ' || c == '>' || c == ',' || c == '\r' || c == '\n')
        .unwrap_or(input.len());

    if uri_end > MAX_URI_LENGTH {
        return Err(nom::Err::Failure(nom::error::Error::new(
            input,
            nom::error::ErrorKind::TooLarge,
        )));
    }

    let uri_body = &input[..uri_end];
    let remaining = &input[uri_end..];

    // Split on ? for headers portion
    let (main_part, headers_part) = match uri_body.find('?') {
        Some(pos) => (&uri_body[..pos], Some(&uri_body[pos + 1..])),
        None => (uri_body, None),
    };

    // Determine user and host+params
    let (user, host_params) = match main_part.find('@') {
        Some(pos) => (Some(&main_part[..pos]), &main_part[pos + 1..]),
        None => (None, main_part),
    };

    // Split host from parameters (first ; that is not inside brackets)
    let (host_port_str, params_str) = split_host_params(host_params);

    // Parse host and port
    let (host, port) = parse_host_port(host_port_str);

    // Parse parameters
    let mut parameters = HashMap::new();
    if let Some(ps) = params_str {
        for segment in ps.split(';') {
            if segment.is_empty() {
                continue;
            }
            match segment.find('=') {
                Some(eq) => {
                    parameters.insert(
                        segment[..eq].to_lowercase(),
                        Some(segment[eq + 1..].to_string()),
                    );
                }
                None => {
                    parameters.insert(segment.to_lowercase(), None);
                }
            }
        }
    }

    // Parse headers
    let mut uri_headers = HashMap::new();
    if let Some(hs) = headers_part {
        for pair in hs.split('&') {
            if let Some(eq) = pair.find('=') {
                uri_headers.insert(pair[..eq].to_string(), pair[eq + 1..].to_string());
            }
        }
    }

    Ok((
        remaining,
        SipUri {
            scheme: scheme.to_lowercase(),
            user: user.map(|s| s.to_string()),
            host,
            port,
            parameters,
            headers: uri_headers,
        },
    ))
}

/// Split "host:port" from ";param1;param2" in the host+params portion.
fn split_host_params(input: &str) -> (&str, Option<&str>) {
    // IPv6 bracket awareness
    if input.starts_with('[') {
        if let Some(bracket_end) = input.find(']') {
            let after = &input[bracket_end + 1..];
            match after.find(';') {
                Some(pos) => (&input[..bracket_end + 1 + pos], Some(&after[pos + 1..])),
                None => (input, None),
            }
        } else {
            (input, None)
        }
    } else {
        match input.find(';') {
            Some(pos) => (&input[..pos], Some(&input[pos + 1..])),
            None => (input, None),
        }
    }
}

/// Parse host and optional port from a string like "example.com:5060" or "[::1]:5060".
fn parse_host_port(input: &str) -> (String, Option<u16>) {
    if input.starts_with('[') {
        // IPv6
        if let Some(bracket_end) = input.find(']') {
            let host = input[1..bracket_end].to_string();
            let after = &input[bracket_end + 1..];
            let port = after
                .strip_prefix(':')
                .and_then(|p| p.parse::<u16>().ok());
            (host, port)
        } else {
            (input.to_string(), None)
        }
    } else if let Some(colon) = input.rfind(':') {
        let port_str = &input[colon + 1..];
        match port_str.parse::<u16>() {
            Ok(port) => (input[..colon].to_string(), Some(port)),
            Err(_) => (input.to_string(), None),
        }
    } else {
        (input.to_string(), None)
    }
}

// ---------------------------------------------------------------------------
// Name-addr parser (for From / To / Contact)
// ---------------------------------------------------------------------------

/// Parse a name-addr or addr-spec:
///   `"Display" <sip:user@host>;tag=xxx`
///   `<sip:user@host>;tag=xxx`
///   `sip:user@host;tag=xxx`
///
/// Returns (display_name, uri, outer_params).
pub fn parse_name_addr(
    input: &str,
) -> IResult<&str, (Option<String>, SipUri, HashMap<String, Option<String>>)> {
    let trimmed = input.trim();

    // Try display-name + angle-bracket form
    if let Some(lt_pos) = trimmed.find('<') {
        let display = trimmed[..lt_pos].trim();
        let display_name = if display.is_empty() {
            None
        } else {
            // Strip surrounding quotes if present
            let dn = display.trim_matches('"').trim();
            if dn.is_empty() {
                None
            } else {
                Some(dn.to_string())
            }
        };

        let after_lt = &trimmed[lt_pos + 1..];
        let gt_pos = after_lt
            .find('>')
            .ok_or_else(|| nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char)))?;
        let uri_str = &after_lt[..gt_pos];
        let after_gt = after_lt[gt_pos + 1..].trim();

        // Parse the URI
        let (_, uri) = parse_sip_uri(uri_str)?;

        // Parse outer parameters (;tag=... etc.)
        let mut params = HashMap::new();
        let mut param_remaining = after_gt;
        while let Some(rest) = param_remaining.strip_prefix(';') {
            let end = rest
                .find(|c: char| c == ';' || c == ',' || c == '\r' || c == '\n')
                .unwrap_or(rest.len());
            let seg = &rest[..end];
            match seg.find('=') {
                Some(eq) => {
                    params.insert(
                        seg[..eq].to_lowercase(),
                        Some(seg[eq + 1..].to_string()),
                    );
                }
                None => {
                    if !seg.is_empty() {
                        params.insert(seg.to_lowercase(), None);
                    }
                }
            }
            param_remaining = &rest[end..];
        }

        Ok(("", (display_name, uri, params)))
    } else {
        // addr-spec form: sip:user@host;tag=xxx
        // The URI parameters and the header parameters are intermixed.
        // Per RFC 3261, for addr-spec without angle brackets the header params
        // (like tag) are part of the URI parameters.  We parse the URI and
        // then extract known header-level params (tag, etc.) from URI params.
        let (rest, mut uri) = parse_sip_uri(trimmed)?;

        let mut params = HashMap::new();
        // Move "tag" from URI params to header-level params
        if let Some(tag_val) = uri.parameters.remove("tag") {
            params.insert("tag".to_string(), tag_val);
        }

        Ok((rest, (None, uri, params)))
    }
}

// ---------------------------------------------------------------------------
// Via header value parser
// ---------------------------------------------------------------------------

/// Parse a Via header value: `SIP/2.0/UDP host:port;branch=xxx;rport;received=1.2.3.4`
pub fn parse_via_header(value: &str) -> SipHeader {
    let trimmed = value.trim();

    // Protocol/Version/Transport  e.g. "SIP/2.0/UDP"
    let (protocol, transport, rest) = match trimmed.find(' ') {
        Some(sp) => {
            let proto_part = &trimmed[..sp];
            let rest = trimmed[sp..].trim_start();
            // Split protocol on last '/'
            let parts: Vec<&str> = proto_part.splitn(3, '/').collect();
            if parts.len() == 3 {
                (
                    format!("{}/{}", parts[0], parts[1]),
                    parts[2].to_string(),
                    rest,
                )
            } else {
                (proto_part.to_string(), "UDP".to_string(), rest)
            }
        }
        None => (
            "SIP/2.0".to_string(),
            "UDP".to_string(),
            trimmed,
        ),
    };

    // Split host:port from parameters at first ';'
    let (host_port_str, params_str) = match rest.find(';') {
        Some(pos) => (&rest[..pos], Some(&rest[pos + 1..])),
        None => (rest, None),
    };

    let (host, port) = parse_host_port(host_port_str.trim());

    // Parse via parameters
    let mut branch: Option<String> = None;
    let mut rport: Option<u16> = None;
    let mut received: Option<String> = None;

    if let Some(ps) = params_str {
        for segment in ps.split(';') {
            let seg = segment.trim();
            if seg.is_empty() {
                continue;
            }
            if let Some(eq) = seg.find('=') {
                let key = seg[..eq].trim().to_lowercase();
                let val = seg[eq + 1..].trim();
                match key.as_str() {
                    "branch" => branch = Some(val.to_string()),
                    "rport" => rport = val.parse::<u16>().ok(),
                    "received" => received = Some(val.to_string()),
                    _ => {}
                }
            } else {
                let key = seg.to_lowercase();
                if key == "rport" {
                    // bare rport flag: use 0 to indicate present without value
                    rport = Some(0);
                }
            }
        }
    }

    SipHeader::Via {
        protocol,
        transport,
        host,
        port,
        branch,
        rport,
        received,
    }
}

// ---------------------------------------------------------------------------
// Auth parameter helpers
// ---------------------------------------------------------------------------

/// Parse Digest auth parameters: `realm="x", nonce="y", ...`
fn parse_auth_params(input: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    // Skip "Digest " prefix if present
    let body = input
        .strip_prefix("Digest ")
        .or_else(|| input.strip_prefix("digest "))
        .unwrap_or(input)
        .trim();

    let mut remaining = body;
    while !remaining.is_empty() {
        // skip leading whitespace/commas
        remaining = remaining.trim_start_matches(|c: char| c == ',' || c == ' ');
        if remaining.is_empty() {
            break;
        }

        // key
        let eq = match remaining.find('=') {
            Some(pos) => pos,
            None => break,
        };
        let key = remaining[..eq].trim().to_lowercase();
        remaining = &remaining[eq + 1..];

        // value (quoted or unquoted)
        let value;
        if remaining.starts_with('"') {
            remaining = &remaining[1..];
            let end = remaining.find('"').unwrap_or(remaining.len());
            value = remaining[..end].to_string();
            remaining = if end < remaining.len() {
                &remaining[end + 1..]
            } else {
                ""
            };
        } else {
            let end = remaining
                .find(|c: char| c == ',' || c == ' ')
                .unwrap_or(remaining.len());
            value = remaining[..end].to_string();
            remaining = &remaining[end..];
        }

        result.insert(key, value);
    }
    result
}

// ---------------------------------------------------------------------------
// Nom parsers for start line
// ---------------------------------------------------------------------------

/// Token characters per RFC 3261.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '-' | '.' | '!' | '%' | '*' | '_' | '+' | '`' | '\'' | '~'
        )
}

fn parse_method_token(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_ascii_alphabetic())(input)
}

fn parse_sip_version(input: &str) -> IResult<&str, &str> {
    recognize(tuple((tag_no_case("SIP/"), digit1, char('.'), digit1)))(input)
}

/// Parse `METHOD sip:user@host SIP/2.0\r\n`
fn parse_request_line(input: &str) -> IResult<&str, (SipMethod, SipUri, String)> {
    let (input, method_str) = parse_method_token(input)?;
    let (input, _) = space1(input)?;
    let (input, uri) = parse_sip_uri(input)?;
    let (input, _) = space1(input)?;
    let (input, version) = parse_sip_version(input)?;
    let (input, _) = crlf(input)?;

    let method = SipMethod::from_str(method_str).unwrap_or_else(|| {
        // We already validated it was alphabetic; treat unknown methods as an
        // error at a higher level.  For robustness, map to Options.
        SipMethod::Options
    });

    Ok((input, (method, uri, version.to_string())))
}

/// Parse `SIP/2.0 200 OK\r\n`
fn parse_status_line(input: &str) -> IResult<&str, (u16, String, String)> {
    let (input, version) = parse_sip_version(input)?;
    let (input, _) = space1(input)?;
    let (input, code_str) = digit1(input)?;
    let (input, _) = space0(input)?;
    let (input, reason) = take_while(|c: char| c != '\r' && c != '\n')(input)?;
    let (input, _) = crlf(input)?;

    let code: u16 = code_str.parse().unwrap_or(0);
    Ok((input, (code, reason.to_string(), version.to_string())))
}

// ---------------------------------------------------------------------------
// Header parser
// ---------------------------------------------------------------------------

/// Compact-form header name mapping (RFC 3261 Section 7.3.3).
fn expand_compact_name(name: &str) -> &str {
    match name {
        "i" => "Call-ID",
        "m" => "Contact",
        "e" => "Content-Encoding",
        "l" => "Content-Length",
        "c" => "Content-Type",
        "f" => "From",
        "s" => "Subject",
        "k" => "Supported",
        "t" => "To",
        "v" => "Via",
        _ => name,
    }
}

/// Parse raw headers (name: value with folding) and return typed `SipHeader`s.
fn parse_headers(input: &str) -> IResult<&str, Vec<SipHeader>> {
    let mut headers = Vec::new();
    let mut remaining = input;

    loop {
        // End of headers
        if remaining.starts_with("\r\n") || remaining.is_empty() {
            break;
        }
        if headers.len() >= MAX_HEADER_COUNT {
            return Err(nom::Err::Failure(nom::error::Error::new(
                input,
                nom::error::ErrorKind::TooLarge,
            )));
        }

        // Parse "Name: value\r\n" with optional continuation lines
        let colon = match remaining.find(':') {
            Some(p) => p,
            None => break,
        };
        let raw_name = remaining[..colon].trim();
        remaining = &remaining[colon + 1..];

        // Collect value across continuation lines
        let mut value_buf = String::new();
        loop {
            let line_end = remaining
                .find("\r\n")
                .unwrap_or(remaining.len());
            value_buf.push_str(remaining[..line_end].trim());
            if line_end + 2 <= remaining.len() {
                remaining = &remaining[line_end + 2..];
            } else {
                remaining = "";
                break;
            }
            // Check for continuation (next line starts with SP or HTAB)
            if remaining.starts_with(' ') || remaining.starts_with('\t') {
                value_buf.push(' ');
            } else {
                break;
            }
        }

        let canonical = expand_compact_name(raw_name);
        let header = classify_header(canonical, &value_buf);
        headers.push(header);
    }

    Ok((remaining, headers))
}

/// Convert a raw (name, value) pair into a typed SipHeader.
fn classify_header(name: &str, value: &str) -> SipHeader {
    match name.to_lowercase().as_str() {
        "via" => parse_via_header(value),
        "from" => {
            match parse_name_addr(value) {
                Ok((_, (dn, uri, params))) => SipHeader::From {
                    display_name: dn,
                    uri,
                    tag: params
                        .get("tag")
                        .and_then(|v| v.clone()),
                },
                Err(_) => SipHeader::Other {
                    name: "From".to_string(),
                    value: value.to_string(),
                },
            }
        }
        "to" => {
            match parse_name_addr(value) {
                Ok((_, (dn, uri, params))) => SipHeader::To {
                    display_name: dn,
                    uri,
                    tag: params
                        .get("tag")
                        .and_then(|v| v.clone()),
                },
                Err(_) => SipHeader::Other {
                    name: "To".to_string(),
                    value: value.to_string(),
                },
            }
        }
        "call-id" => SipHeader::CallId(value.trim().to_string()),
        "cseq" => {
            let parts: Vec<&str> = value.trim().splitn(2, ' ').collect();
            if parts.len() == 2 {
                if let (Ok(seq), Some(method)) =
                    (parts[0].parse::<u32>(), SipMethod::from_str(parts[1]))
                {
                    return SipHeader::CSeq { seq, method };
                }
            }
            SipHeader::Other {
                name: "CSeq".to_string(),
                value: value.to_string(),
            }
        }
        "contact" => {
            if value.trim() == "*" {
                // Wildcard contact
                return SipHeader::Contact {
                    display_name: None,
                    uri: SipUri::new("sip", "*"),
                    params: HashMap::new(),
                };
            }
            match parse_name_addr(value) {
                Ok((_, (dn, uri, params))) => SipHeader::Contact {
                    display_name: dn,
                    uri,
                    params,
                },
                Err(_) => SipHeader::Other {
                    name: "Contact".to_string(),
                    value: value.to_string(),
                },
            }
        }
        "content-type" => SipHeader::ContentType(value.trim().to_string()),
        "content-length" => {
            if let Ok(len) = value.trim().parse::<usize>() {
                SipHeader::ContentLength(len)
            } else {
                SipHeader::Other {
                    name: "Content-Length".to_string(),
                    value: value.to_string(),
                }
            }
        }
        "max-forwards" => {
            if let Ok(mf) = value.trim().parse::<u32>() {
                SipHeader::MaxForwards(mf)
            } else {
                SipHeader::Other {
                    name: "Max-Forwards".to_string(),
                    value: value.to_string(),
                }
            }
        }
        "user-agent" => SipHeader::UserAgent(value.trim().to_string()),
        "allow" => {
            let methods: Vec<SipMethod> = value
                .split(',')
                .filter_map(|s| SipMethod::from_str(s.trim()))
                .collect();
            SipHeader::Allow(methods)
        }
        "supported" => {
            let exts: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            SipHeader::Supported(exts)
        }
        "www-authenticate" => {
            let params = parse_auth_params(value);
            SipHeader::WwwAuthenticate {
                realm: params.get("realm").cloned().unwrap_or_default(),
                nonce: params.get("nonce").cloned().unwrap_or_default(),
                algorithm: params.get("algorithm").cloned(),
                qop: params.get("qop").cloned(),
                opaque: params.get("opaque").cloned(),
            }
        }
        "authorization" | "proxy-authorization" => {
            let params = parse_auth_params(value);
            SipHeader::Authorization {
                username: params.get("username").cloned().unwrap_or_default(),
                realm: params.get("realm").cloned().unwrap_or_default(),
                nonce: params.get("nonce").cloned().unwrap_or_default(),
                uri: params.get("uri").cloned().unwrap_or_default(),
                response: params.get("response").cloned().unwrap_or_default(),
                algorithm: params.get("algorithm").cloned(),
                cnonce: params.get("cnonce").cloned(),
                nc: params.get("nc").cloned(),
                qop: params.get("qop").cloned(),
            }
        }
        _ => SipHeader::Other {
            name: name.to_string(),
            value: value.to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// SDP parser
// ---------------------------------------------------------------------------

/// Parse an SDP body into structured session data.
pub fn parse_sdp_body(body: &str) -> Option<SdpSession> {
    let mut version = 0u32;
    let mut origin = String::new();
    let mut session_name = String::new();
    let mut connection = None;
    let mut timing = None;
    let mut media_descs: Vec<SdpMediaDescription> = Vec::new();

    for line in body.lines() {
        let line = line.trim_end_matches('\r');
        if line.len() < 2 || line.as_bytes().get(1) != Some(&b'=') {
            continue;
        }
        let type_char = line.as_bytes()[0] as char;
        let value = &line[2..];

        match type_char {
            'v' => version = value.parse().unwrap_or(0),
            'o' => origin = value.to_string(),
            's' => session_name = value.to_string(),
            'c' => {
                if media_descs.is_empty() {
                    connection = Some(value.to_string());
                }
                // Per-media connection is stored as an attribute for simplicity
            }
            't' => timing = Some(value.to_string()),
            'm' => {
                // m=audio 49170 RTP/AVP 0 8 97
                let parts: Vec<&str> = value.splitn(4, ' ').collect();
                if parts.len() >= 3 {
                    let port: u16 = parts[1].split('/').next().unwrap_or("0").parse().unwrap_or(0);
                    let protocol = parts[2].to_string();
                    let formats = if parts.len() > 3 {
                        parts[3]
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect()
                    } else {
                        Vec::new()
                    };
                    media_descs.push(SdpMediaDescription {
                        media_type: parts[0].to_string(),
                        port,
                        protocol,
                        formats,
                        attributes: Vec::new(),
                    });
                }
            }
            'a' => {
                let attr = if let Some(colon) = value.find(':') {
                    (value[..colon].to_string(), Some(value[colon + 1..].to_string()))
                } else {
                    (value.to_string(), None)
                };
                if let Some(m) = media_descs.last_mut() {
                    m.attributes.push(attr);
                }
                // Session-level attributes are dropped in this simplified parser
            }
            _ => {} // ignore unknown lines
        }
    }

    Some(SdpSession {
        version,
        origin,
        session_name,
        connection,
        timing,
        media_descriptions: media_descs,
    })
}

// ---------------------------------------------------------------------------
// Top-level message parser
// ---------------------------------------------------------------------------

/// Find the byte offset of the `\r\n\r\n` header-body separator.
fn find_header_boundary(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse a complete SIP message from a byte slice.
///
/// On success the returned `&[u8]` slice points to any remaining bytes after
/// the fully consumed message (useful for stream-oriented transports).
pub fn parse_sip_message(input: &[u8]) -> IResult<&[u8], SipMessage> {
    // ---- size guard ----
    if input.len() > MAX_MESSAGE_SIZE {
        return Err(nom::Err::Failure(nom::error::Error::new(
            input,
            nom::error::ErrorKind::TooLarge,
        )));
    }

    // ---- locate header / body boundary ----
    let sep_pos = find_header_boundary(input).ok_or(nom::Err::Incomplete(
        nom::Needed::Unknown,
    ))?;

    // Include the trailing CRLF of the last header (sep_pos points to the
    // first \r in the \r\n\r\n sequence; the first \r\n belongs to the last
    // header line, so we include it in the header text).
    let header_bytes = &input[..sep_pos + 2];
    let header_text = std::str::from_utf8(header_bytes).map_err(|_| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Char))
    })?;

    // ---- try request line, then status line ----
    let parse_result: IResult<&str, SipMessage> = alt((
        |i| {
            let (i, (method, uri, version)) = parse_request_line(i)?;
            let (i, headers) = parse_headers(i)?;
            Ok((
                i,
                SipMessage::Request(SipRequest {
                    method,
                    uri,
                    version,
                    headers,
                    body: None, // filled below
                }),
            ))
        },
        |i| {
            let (i, (status_code, reason, version)) = parse_status_line(i)?;
            let (i, headers) = parse_headers(i)?;
            Ok((
                i,
                SipMessage::Response(SipResponse {
                    version,
                    status_code,
                    reason,
                    headers,
                    body: None,
                }),
            ))
        },
    ))(header_text);

    let (_, mut message) = parse_result.map_err(|e| match e {
        nom::Err::Error(e) => {
            nom::Err::Error(nom::error::Error::new(input, e.code))
        }
        nom::Err::Failure(e) => {
            nom::Err::Failure(nom::error::Error::new(input, e.code))
        }
        nom::Err::Incomplete(n) => nom::Err::Incomplete(n),
    })?;

    // ---- body handling ----
    let after_sep = sep_pos + 4; // skip past \r\n\r\n
    let content_length = message.content_length().unwrap_or(0);
    let body_end = after_sep + content_length;

    if input.len() < body_end {
        return Err(nom::Err::Incomplete(nom::Needed::new(
            body_end - input.len(),
        )));
    }

    if content_length > 0 {
        let body_bytes = &input[after_sep..body_end];
        let body_str = String::from_utf8_lossy(body_bytes).into_owned();

        // Check for SDP content
        let is_sdp = message.headers().iter().any(|h| {
            if let SipHeader::ContentType(ref ct) = h {
                ct.to_lowercase().contains("application/sdp")
            } else {
                false
            }
        });

        let sdp = if is_sdp {
            parse_sdp_body(&body_str)
        } else {
            None
        };

        match &mut message {
            SipMessage::Request(ref mut r) => {
                r.body = Some(body_str);
            }
            SipMessage::Response(ref mut r) => {
                r.body = Some(body_str);
            }
        }

        // We don't store SDP directly on the message struct; callers use
        // parse_sdp_body on the body text when needed.  This keeps the
        // core types simpler and avoids double-storage.
    }

    let remaining = &input[body_end..];
    Ok((remaining, message))
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- test 1: parse INVITE request ----
    #[test]
    fn test_parse_invite_request() {
        let raw = b"INVITE sip:bob@biloxi.example.com SIP/2.0\r\n\
Via: SIP/2.0/UDP pc33.atlanta.example.com;branch=z9hG4bKnashds8\r\n\
From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n\
To: Bob <sip:bob@biloxi.example.com>\r\n\
Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n\
CSeq: 314159 INVITE\r\n\
Contact: <sip:alice@pc33.atlanta.example.com>\r\n\
Max-Forwards: 70\r\n\
Content-Length: 0\r\n\
\r\n";

        let (remaining, msg) = parse_sip_message(raw).expect("parse INVITE");
        assert!(remaining.is_empty());
        assert!(msg.is_request());
        assert_eq!(msg.method(), Some(&SipMethod::Invite));
        assert_eq!(
            msg.call_id(),
            Some("a84b4c76e66710@pc33.atlanta.example.com")
        );
        assert_eq!(msg.from_tag(), Some("1928301774"));
        assert!(msg.to_tag().is_none());

        if let SipMessage::Request(ref r) = msg {
            assert_eq!(r.uri.host, "biloxi.example.com");
            assert_eq!(r.uri.user.as_deref(), Some("bob"));
        }
    }

    // ---- test 2: parse 200 OK response ----
    #[test]
    fn test_parse_200_ok() {
        let raw = b"SIP/2.0 200 OK\r\n\
Via: SIP/2.0/UDP server10.biloxi.example.com;branch=z9hG4bK4b43c2ff8.1\r\n\
From: Alice <sip:alice@atlanta.example.com>;tag=1928301774\r\n\
To: Bob <sip:bob@biloxi.example.com>;tag=a6c85cf\r\n\
Call-ID: a84b4c76e66710@pc33.atlanta.example.com\r\n\
CSeq: 314159 INVITE\r\n\
Contact: <sip:bob@192.0.2.4>\r\n\
Content-Length: 0\r\n\
\r\n";

        let (_, msg) = parse_sip_message(raw).expect("parse 200");
        assert!(msg.is_response());
        assert_eq!(msg.status_code(), Some(200));
        assert_eq!(msg.to_tag(), Some("a6c85cf"));
        assert_eq!(msg.cseq_method(), Some(&SipMethod::Invite));
    }

    // ---- test 3: reject oversized messages ----
    #[test]
    fn test_reject_oversized() {
        let big = vec![b'A'; MAX_MESSAGE_SIZE + 1];
        let result = parse_sip_message(&big);
        assert!(result.is_err());
    }

    // ---- test 4: parse SIP URI ----
    #[test]
    fn test_parse_sip_uri() {
        let (rest, uri) = parse_sip_uri("sip:alice@atlanta.com:5060;transport=tcp").unwrap();
        assert!(rest.is_empty());
        assert_eq!(uri.scheme, "sip");
        assert_eq!(uri.user.as_deref(), Some("alice"));
        assert_eq!(uri.host, "atlanta.com");
        assert_eq!(uri.port, Some(5060));
        assert_eq!(
            uri.parameters.get("transport"),
            Some(&Some("tcp".to_string()))
        );
    }

    // ---- test 5: parse SIPS URI ----
    #[test]
    fn test_parse_sips_uri() {
        let (_, uri) = parse_sip_uri("sips:bob@secure.example.org").unwrap();
        assert_eq!(uri.scheme, "sips");
        assert_eq!(uri.user.as_deref(), Some("bob"));
        assert_eq!(uri.host, "secure.example.org");
        assert!(uri.port.is_none());
    }

    // ---- test 6: SipUri Display round-trip ----
    #[test]
    fn test_sip_uri_display() {
        let uri = SipUri {
            scheme: "sip".to_string(),
            user: Some("alice".to_string()),
            host: "example.com".to_string(),
            port: Some(5060),
            parameters: HashMap::new(),
            headers: HashMap::new(),
        };
        let s = uri.to_string();
        assert!(s.contains("sip:alice@example.com:5060"));
    }

    // ---- test 7: parse name-addr with display name ----
    #[test]
    fn test_parse_name_addr() {
        let (_, (dn, uri, params)) =
            parse_name_addr("\"Alice\" <sip:alice@atlanta.com>;tag=9fxced76sl").unwrap();
        assert_eq!(dn.as_deref(), Some("Alice"));
        assert_eq!(uri.user.as_deref(), Some("alice"));
        assert_eq!(params.get("tag"), Some(&Some("9fxced76sl".to_string())));
    }

    // ---- test 8: parse Via header ----
    #[test]
    fn test_parse_via_header() {
        let hdr =
            parse_via_header("SIP/2.0/UDP 10.0.0.1:5060;branch=z9hG4bK776asdhds;rport;received=192.168.1.1");
        if let SipHeader::Via {
            protocol,
            transport,
            host,
            port,
            branch,
            rport,
            received,
        } = hdr
        {
            assert_eq!(protocol, "SIP/2.0");
            assert_eq!(transport, "UDP");
            assert_eq!(host, "10.0.0.1");
            assert_eq!(port, Some(5060));
            assert_eq!(branch.as_deref(), Some("z9hG4bK776asdhds"));
            assert_eq!(rport, Some(0)); // bare rport
            assert_eq!(received.as_deref(), Some("192.168.1.1"));
        } else {
            panic!("Expected Via header");
        }
    }

    // ---- test 9: parse SDP body ----
    #[test]
    fn test_parse_sdp() {
        let sdp_text = "v=0\r\n\
o=alice 2890844526 2890844526 IN IP4 host.atlanta.example.com\r\n\
s=-\r\n\
c=IN IP4 host.atlanta.example.com\r\n\
t=0 0\r\n\
m=audio 49170 RTP/AVP 0 8 97\r\n\
a=rtpmap:0 PCMU/8000\r\n\
a=rtpmap:8 PCMA/8000\r\n\
a=rtpmap:97 opus/48000/2\r\n";

        let sdp = parse_sdp_body(sdp_text).expect("parse SDP");
        assert_eq!(sdp.version, 0);
        assert_eq!(sdp.media_descriptions.len(), 1);
        assert_eq!(sdp.media_descriptions[0].port, 49170);
        assert_eq!(sdp.media_descriptions[0].formats, vec!["0", "8", "97"]);
        assert_eq!(sdp.media_descriptions[0].attributes.len(), 3);
    }

    // ---- test 10: builder round-trip ----
    #[test]
    fn test_builder_round_trip() {
        let uri = SipUri::new("sip", "example.com");
        let msg = SipMessage::new_request(SipMethod::Register, uri)
            .header(SipHeader::Via {
                protocol: "SIP/2.0".to_string(),
                transport: "UDP".to_string(),
                host: "127.0.0.1".to_string(),
                port: Some(5060),
                branch: Some("z9hG4bKtest123".to_string()),
                rport: None,
                received: None,
            })
            .header(SipHeader::From {
                display_name: Some("Test".to_string()),
                uri: SipUri::new("sip", "example.com"),
                tag: Some("fromtag".to_string()),
            })
            .header(SipHeader::To {
                display_name: None,
                uri: SipUri::new("sip", "example.com"),
                tag: None,
            })
            .header(SipHeader::CallId("test-call-id@127.0.0.1".to_string()))
            .header(SipHeader::CSeq {
                seq: 1,
                method: SipMethod::Register,
            })
            .build();

        let bytes = msg.to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("REGISTER sip:example.com SIP/2.0\r\n"));
        assert!(text.contains("Call-ID: test-call-id@127.0.0.1"));
    }

    // ---- test 11: compact header names ----
    #[test]
    fn test_compact_headers() {
        let raw = b"REGISTER sip:registrar.example.com SIP/2.0\r\n\
v: SIP/2.0/UDP 10.0.0.1:5060;branch=z9hG4bKcompact\r\n\
f: <sip:alice@example.com>;tag=abc\r\n\
t: <sip:alice@example.com>\r\n\
i: compact-call-id@example.com\r\n\
CSeq: 1 REGISTER\r\n\
l: 0\r\n\
\r\n";

        let (_, msg) = parse_sip_message(raw).expect("parse compact");
        assert_eq!(msg.call_id(), Some("compact-call-id@example.com"));
        assert_eq!(msg.from_tag(), Some("abc"));
        assert_eq!(msg.content_length(), Some(0));
    }

    // ---- test 12: parse REGISTER with body ----
    #[test]
    fn test_message_with_body() {
        let body = "v=0\r\no=test 1 1 IN IP4 0.0.0.0\r\ns=-\r\n";
        let body_len = body.len();
        let raw = format!(
            "INVITE sip:bob@example.com SIP/2.0\r\n\
Via: SIP/2.0/UDP 10.0.0.1;branch=z9hG4bKbody\r\n\
From: <sip:alice@example.com>;tag=t1\r\n\
To: <sip:bob@example.com>\r\n\
Call-ID: body-test@example.com\r\n\
CSeq: 1 INVITE\r\n\
Content-Type: application/sdp\r\n\
Content-Length: {}\r\n\
\r\n{}",
            body_len, body
        );

        let (_, msg) = parse_sip_message(raw.as_bytes()).expect("parse with body");
        assert_eq!(msg.body(), Some(body));
    }

    // ---- test 13: parse WWW-Authenticate ----
    #[test]
    fn test_parse_www_authenticate() {
        let raw = b"SIP/2.0 401 Unauthorized\r\n\
Via: SIP/2.0/UDP 10.0.0.1;branch=z9hG4bKauth\r\n\
From: <sip:alice@example.com>;tag=t1\r\n\
To: <sip:alice@example.com>;tag=t2\r\n\
Call-ID: auth-test@example.com\r\n\
CSeq: 1 REGISTER\r\n\
WWW-Authenticate: Digest realm=\"example.com\", nonce=\"abc123\", algorithm=MD5, qop=\"auth\"\r\n\
Content-Length: 0\r\n\
\r\n";

        let (_, msg) = parse_sip_message(raw).expect("parse 401");
        let auth_hdr = msg
            .headers()
            .iter()
            .find(|h| matches!(h, SipHeader::WwwAuthenticate { .. }));
        assert!(auth_hdr.is_some());
        if let Some(SipHeader::WwwAuthenticate {
            realm,
            nonce,
            algorithm,
            qop,
            ..
        }) = auth_hdr
        {
            assert_eq!(realm, "example.com");
            assert_eq!(nonce, "abc123");
            assert_eq!(algorithm.as_deref(), Some("MD5"));
            assert_eq!(qop.as_deref(), Some("auth"));
        }
    }

    // ---- test 14: to_bytes serialization ----
    #[test]
    fn test_to_bytes_response() {
        let msg = SipMessage::new_response(180, "Ringing")
            .header(SipHeader::Via {
                protocol: "SIP/2.0".to_string(),
                transport: "UDP".to_string(),
                host: "10.0.0.1".to_string(),
                port: None,
                branch: Some("z9hG4bKring".to_string()),
                rport: None,
                received: None,
            })
            .header(SipHeader::From {
                display_name: None,
                uri: SipUri::new("sip", "alice.example.com"),
                tag: Some("a".to_string()),
            })
            .header(SipHeader::To {
                display_name: None,
                uri: SipUri::new("sip", "bob.example.com"),
                tag: Some("b".to_string()),
            })
            .header(SipHeader::CallId("ring@example.com".to_string()))
            .header(SipHeader::CSeq {
                seq: 1,
                method: SipMethod::Invite,
            })
            .build();

        let bytes = msg.to_bytes();
        let text = String::from_utf8_lossy(&bytes);
        assert!(text.starts_with("SIP/2.0 180 Ringing\r\n"));
        assert!(text.contains("Content-Length: 0\r\n"));
    }

    // ---- test 15: SipMethod display ----
    #[test]
    fn test_sip_method_display() {
        assert_eq!(SipMethod::Invite.to_string(), "INVITE");
        assert_eq!(SipMethod::Register.to_string(), "REGISTER");
        assert_eq!(SipMethod::Prack.to_string(), "PRACK");
    }
}
