use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use dashmap::DashMap;
use futures::stream::SplitSink;
use futures::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::time;
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage, WebSocketStream};
use tracing::{debug, error, info, trace, warn};

use crate::sip::parser::{parse_sip_message, SipMessage};

// ---------------------------------------------------------------------------
// TransportType
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportType {
    UDP,
    TCP,
    TLS,
    WS,
    WSS,
}

impl std::fmt::Display for TransportType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportType::UDP => write!(f, "UDP"),
            TransportType::TCP => write!(f, "TCP"),
            TransportType::TLS => write!(f, "TLS"),
            TransportType::WS => write!(f, "WS"),
            TransportType::WSS => write!(f, "WSS"),
        }
    }
}

impl TransportType {
    /// Infer transport type from a URI string / transport parameter.
    pub fn from_uri(uri: &str) -> Self {
        let lower = uri.to_lowercase();
        if lower.contains("transport=wss") || lower.starts_with("wss://") {
            TransportType::WSS
        } else if lower.contains("transport=ws") || lower.starts_with("ws://") {
            TransportType::WS
        } else if lower.contains("transport=tls") || lower.starts_with("sips:") {
            TransportType::TLS
        } else if lower.contains("transport=tcp") {
            TransportType::TCP
        } else {
            TransportType::UDP
        }
    }
}

// ---------------------------------------------------------------------------
// SipTransport trait
// ---------------------------------------------------------------------------

/// A transport capable of sending and receiving SIP messages.
#[async_trait]
pub trait SipTransport: Send + Sync {
    /// Send a serialized SIP message to the given address.
    async fn send(&self, message: &SipMessage, addr: SocketAddr) -> Result<()>;

    /// Receive the next inbound SIP message together with the sender address.
    async fn recv(&self) -> Result<(SipMessage, SocketAddr)>;
}

// ---------------------------------------------------------------------------
// UdpTransport
// ---------------------------------------------------------------------------

/// SIP over UDP -- one datagram per message, no framing needed.
pub struct UdpTransport {
    socket: Arc<UdpSocket>,
    /// Inbound messages are forwarded here by the listener task.
    inbound_rx: Mutex<mpsc::Receiver<(SipMessage, SocketAddr)>>,
    /// Max message size for receive buffer.
    max_message_size: usize,
}

impl UdpTransport {
    /// Bind a UDP socket and spawn the receive loop.
    pub async fn bind(addr: SocketAddr, max_message_size: usize) -> Result<Self> {
        let socket = UdpSocket::bind(addr)
            .await
            .context(format!("UDP bind on {}", addr))?;
        let socket = Arc::new(socket);
        info!(addr = %addr, "SIP/UDP transport bound");

        let (tx, rx) = mpsc::channel::<(SipMessage, SocketAddr)>(4096);

        // Spawn the receive loop.
        let recv_socket = Arc::clone(&socket);
        let max_sz = max_message_size;
        tokio::spawn(async move {
            let mut buf = vec![0u8; max_sz];
            loop {
                match recv_socket.recv_from(&mut buf).await {
                    Ok((len, peer)) => {
                        if len > max_sz {
                            warn!(peer = %peer, len, "Oversized UDP datagram dropped");
                            continue;
                        }
                        let data = &buf[..len];
                        match parse_sip_message(data) {
                            Ok((_, msg)) => {
                                if tx.send((msg, peer)).await.is_err() {
                                    // Channel closed -- transport is shutting down.
                                    break;
                                }
                            }
                            Err(e) => {
                                debug!(peer = %peer, error = ?e, "Failed to parse UDP SIP message");
                            }
                        }
                    }
                    Err(e) => {
                        error!("UDP recv error: {}", e);
                        // Brief pause to avoid busy-loop on persistent errors.
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                }
            }
        });

        Ok(UdpTransport {
            socket,
            inbound_rx: Mutex::new(rx),
            max_message_size,
        })
    }
}

#[async_trait]
impl SipTransport for UdpTransport {
    async fn send(&self, message: &SipMessage, addr: SocketAddr) -> Result<()> {
        let data = message.to_bytes();
        self.socket
            .send_to(&data, addr)
            .await
            .context("UDP send_to")?;
        trace!(addr = %addr, len = data.len(), "Sent SIP/UDP");
        Ok(())
    }

    async fn recv(&self) -> Result<(SipMessage, SocketAddr)> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("UDP transport channel closed"))
    }
}

// ---------------------------------------------------------------------------
// TcpTransport
// ---------------------------------------------------------------------------

/// State for a single accepted TCP connection.
struct TcpConnection {
    writer: Mutex<tokio::io::WriteHalf<TcpStream>>,
    created_at: std::time::Instant,
}

/// SIP over TCP -- Content-Length-based framing, connection pool, keepalive.
pub struct TcpTransport {
    /// Outbound connection pool keyed by remote address.
    pool: Arc<DashMap<SocketAddr, Arc<TcpConnection>>>,
    /// Inbound messages.
    inbound_rx: Mutex<mpsc::Receiver<(SipMessage, SocketAddr)>>,
    inbound_tx: mpsc::Sender<(SipMessage, SocketAddr)>,
    max_message_size: usize,
    /// How long an idle connection is kept alive (seconds).
    max_idle_secs: u64,
}

impl TcpTransport {
    /// Bind a TCP listener, start accepting connections.
    pub async fn bind(addr: SocketAddr, max_message_size: usize) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .context(format!("TCP bind on {}", addr))?;
        info!(addr = %addr, "SIP/TCP transport listening");

        let (tx, rx) = mpsc::channel::<(SipMessage, SocketAddr)>(4096);
        let pool: Arc<DashMap<SocketAddr, Arc<TcpConnection>>> = Arc::new(DashMap::new());
        let max_sz = max_message_size;

        // Accept loop
        let accept_tx = tx.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        debug!(peer = %peer, "New SIP/TCP connection");
                        let ch = accept_tx.clone();
                        let msz = max_sz;
                        tokio::spawn(async move {
                            Self::handle_inbound_stream(stream, peer, ch, msz).await;
                        });
                    }
                    Err(e) => {
                        error!("TCP accept error: {}", e);
                    }
                }
            }
        });

        // Keepalive / idle cleanup task
        let pool_ref = Arc::clone(&pool);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let idle_limit = 300u64; // 5 minutes
                pool_ref.retain(|_, conn: &mut Arc<TcpConnection>| {
                    conn.created_at.elapsed().as_secs() < idle_limit
                });
            }
        });

        Ok(TcpTransport {
            pool,
            inbound_rx: Mutex::new(rx),
            inbound_tx: tx,
            max_message_size,
            max_idle_secs: 300,
        })
    }

    /// Read SIP messages from an inbound TCP stream using Content-Length framing.
    async fn handle_inbound_stream(
        mut stream: TcpStream,
        peer: SocketAddr,
        tx: mpsc::Sender<(SipMessage, SocketAddr)>,
        max_size: usize,
    ) {
        let mut buf = BytesMut::with_capacity(8192);
        let mut tmp = vec![0u8; 4096];

        loop {
            match stream.read(&mut tmp).await {
                Ok(0) => {
                    debug!(peer = %peer, "TCP connection closed");
                    break;
                }
                Ok(n) => {
                    buf.extend_from_slice(&tmp[..n]);
                }
                Err(e) => {
                    debug!(peer = %peer, error = %e, "TCP read error");
                    break;
                }
            }

            // CRLF keepalive: if the buffer contains only \r\n, discard it.
            while buf.starts_with(b"\r\n") && buf.len() == 2 {
                buf.clear();
                continue;
            }

            if buf.len() > max_size {
                warn!(peer = %peer, "TCP buffer overflow -- dropping connection");
                break;
            }

            // Extract complete messages.
            loop {
                let data = buf.as_ref();
                let sep_pos = match find_header_end(data) {
                    Some(p) => p,
                    None => break, // need more data
                };

                let content_length = extract_content_length(&data[..sep_pos]).unwrap_or(0);
                let total = sep_pos + 4 + content_length; // 4 = \r\n\r\n
                if data.len() < total {
                    break; // need more data
                }

                let msg_bytes = buf.split_to(total);
                match parse_sip_message(&msg_bytes) {
                    Ok((_, msg)) => {
                        if tx.send((msg, peer)).await.is_err() {
                            return; // channel closed
                        }
                    }
                    Err(e) => {
                        debug!(peer = %peer, error = ?e, "Failed to parse TCP SIP message");
                    }
                }
            }
        }
    }

    /// Get or create an outbound connection to `addr`.
    async fn get_or_connect(&self, addr: SocketAddr) -> Result<Arc<TcpConnection>> {
        // Check pool
        if let Some(entry) = self.pool.get(&addr) {
            if entry.created_at.elapsed().as_secs() < self.max_idle_secs {
                return Ok(entry.clone());
            }
            drop(entry);
            self.pool.remove(&addr);
        }

        // New connection
        let stream = TcpStream::connect(addr)
            .await
            .context(format!("TCP connect to {}", addr))?;
        debug!(addr = %addr, "Outbound TCP connection established");

        let (reader, writer) = tokio::io::split(stream);
        let conn = Arc::new(TcpConnection {
            writer: Mutex::new(writer),
            created_at: std::time::Instant::now(),
        });
        self.pool.insert(addr, conn.clone());

        // Spawn reader for data coming back on this outbound connection.
        let tx = self.inbound_tx.clone();
        let max_sz = self.max_message_size;
        tokio::spawn(async move {
            let mut reader = reader;
            let mut buf = BytesMut::with_capacity(8192);
            let mut tmp = vec![0u8; 4096];
            loop {
                match reader.read(&mut tmp).await {
                    Ok(0) => break,
                    Ok(n) => {
                        buf.extend_from_slice(&tmp[..n]);
                        // Try to extract messages
                        loop {
                            let data = buf.as_ref();
                            let sep_pos = match find_header_end(data) {
                                Some(p) => p,
                                None => break,
                            };
                            let cl = extract_content_length(&data[..sep_pos]).unwrap_or(0);
                            let total = sep_pos + 4 + cl;
                            if data.len() < total {
                                break;
                            }
                            let msg_bytes = buf.split_to(total);
                            if let Ok((_, msg)) = parse_sip_message(&msg_bytes) {
                                if tx.send((msg, addr)).await.is_err() {
                                    return;
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(conn)
    }
}

#[async_trait]
impl SipTransport for TcpTransport {
    async fn send(&self, message: &SipMessage, addr: SocketAddr) -> Result<()> {
        let conn = self.get_or_connect(addr).await?;
        let data = message.to_bytes();
        let mut writer = conn.writer.lock().await;
        writer.write_all(&data).await.context("TCP write")?;
        trace!(addr = %addr, len = data.len(), "Sent SIP/TCP");
        Ok(())
    }

    async fn recv(&self) -> Result<(SipMessage, SocketAddr)> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("TCP transport channel closed"))
    }
}

// ---------------------------------------------------------------------------
// WsTransport (RFC 7118)
// ---------------------------------------------------------------------------

type WsSink = Arc<Mutex<SplitSink<WebSocketStream<TcpStream>, WsMessage>>>;

/// SIP over WebSocket per RFC 7118.
pub struct WsTransport {
    /// Registry of active WebSocket connections for sending.
    sinks: Arc<DashMap<SocketAddr, WsSink>>,
    /// Inbound parsed messages.
    inbound_rx: Mutex<mpsc::Receiver<(SipMessage, SocketAddr)>>,
    inbound_tx: mpsc::Sender<(SipMessage, SocketAddr)>,
}

impl WsTransport {
    /// Bind a WebSocket listener, start accepting connections.
    pub async fn bind(addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .await
            .context(format!("WS bind on {}", addr))?;
        info!(addr = %addr, "SIP/WS transport listening");

        let (tx, rx) = mpsc::channel::<(SipMessage, SocketAddr)>(4096);
        let sinks: Arc<DashMap<SocketAddr, WsSink>> = Arc::new(DashMap::new());

        let accept_tx = tx.clone();
        let accept_sinks = Arc::clone(&sinks);

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        let ch = accept_tx.clone();
                        let registry = Arc::clone(&accept_sinks);
                        tokio::spawn(async move {
                            Self::handle_ws_connection(stream, peer, ch, registry).await;
                        });
                    }
                    Err(e) => {
                        error!("WebSocket accept error: {}", e);
                    }
                }
            }
        });

        // Ping/pong keepalive task
        let ping_sinks: Arc<DashMap<SocketAddr, WsSink>> = Arc::clone(&sinks);
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;
                let mut to_remove = Vec::new();
                for entry in ping_sinks.iter() {
                    let addr = *entry.key();
                    let sink = entry.value().clone();
                    let mut guard = sink.lock().await;
                    if guard
                        .send(WsMessage::Ping(vec![0x53, 0x49, 0x50])) // "SIP"
                        .await
                        .is_err()
                    {
                        to_remove.push(addr);
                    }
                }
                for addr in to_remove {
                    ping_sinks.remove(&addr);
                    debug!(addr = %addr, "Removed dead WebSocket connection");
                }
            }
        });

        Ok(WsTransport {
            sinks,
            inbound_rx: Mutex::new(rx),
            inbound_tx: tx,
        })
    }

    async fn handle_ws_connection(
        stream: TcpStream,
        peer: SocketAddr,
        tx: mpsc::Sender<(SipMessage, SocketAddr)>,
        sinks: Arc<DashMap<SocketAddr, WsSink>>,
    ) {
        let ws_stream = match accept_async(stream).await {
            Ok(ws) => ws,
            Err(e) => {
                debug!(peer = %peer, error = %e, "WebSocket handshake failed");
                return;
            }
        };
        debug!(peer = %peer, "SIP/WS connection established");

        let (sink, mut ws_rx) = ws_stream.split();
        let ws_sink: WsSink = Arc::new(Mutex::new(sink));
        sinks.insert(peer, ws_sink);

        while let Some(result) = ws_rx.next().await {
            match result {
                Ok(WsMessage::Text(text)) => {
                    let data = text.as_bytes();
                    match parse_sip_message(data) {
                        Ok((_, msg)) => {
                            if tx.send((msg, peer)).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            debug!(peer = %peer, error = ?e, "Failed to parse WS text SIP message");
                        }
                    }
                }
                Ok(WsMessage::Binary(data)) => {
                    match parse_sip_message(&data) {
                        Ok((_, msg)) => {
                            if tx.send((msg, peer)).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            debug!(peer = %peer, error = ?e, "Failed to parse WS binary SIP message");
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    debug!(peer = %peer, "WebSocket close frame received");
                    break;
                }
                Ok(WsMessage::Pong(_)) => {
                    trace!(peer = %peer, "WS pong");
                }
                Ok(_) => {} // Ping handled by tungstenite
                Err(e) => {
                    debug!(peer = %peer, error = %e, "WebSocket read error");
                    break;
                }
            }
        }

        sinks.remove(&peer);
        debug!(peer = %peer, "WebSocket connection ended");
    }
}

#[async_trait]
impl SipTransport for WsTransport {
    async fn send(&self, message: &SipMessage, addr: SocketAddr) -> Result<()> {
        let sink = self
            .sinks
            .get(&addr)
            .map(|e| e.value().clone())
            .ok_or_else(|| anyhow::anyhow!("No WebSocket connection to {}", addr))?;

        let data = message.to_bytes();
        let text = String::from_utf8_lossy(&data).into_owned();
        let mut guard = sink.lock().await;
        guard
            .send(WsMessage::Text(text))
            .await
            .context("WebSocket send")?;
        trace!(addr = %addr, "Sent SIP/WS");
        Ok(())
    }

    async fn recv(&self) -> Result<(SipMessage, SocketAddr)> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("WS transport channel closed"))
    }
}

// ---------------------------------------------------------------------------
// TransportManager
// ---------------------------------------------------------------------------

/// Manages all SIP transport instances and routes messages to the correct one.
pub struct TransportManager {
    udp: Option<Arc<UdpTransport>>,
    tcp: Option<Arc<TcpTransport>>,
    ws: Option<Arc<WsTransport>>,

    /// Unified inbound channel that aggregates all transports.
    inbound_tx: mpsc::Sender<(SipMessage, SocketAddr, TransportType)>,
    inbound_rx: Mutex<mpsc::Receiver<(SipMessage, SocketAddr, TransportType)>>,

    /// Signal for graceful shutdown.
    shutdown_tx: watch::Sender<bool>,
}

impl TransportManager {
    /// Create a new manager without any transports started yet.
    pub fn new() -> (Self, watch::Receiver<bool>) {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (inbound_tx, inbound_rx) = mpsc::channel(8192);

        (
            TransportManager {
                udp: None,
                tcp: None,
                ws: None,
                inbound_tx,
                inbound_rx: Mutex::new(inbound_rx),
                shutdown_tx,
            },
            shutdown_rx,
        )
    }

    /// Start all configured transports.
    ///
    /// Pass `None` for any transport you do not wish to enable.
    pub async fn start(
        &mut self,
        udp_addr: Option<SocketAddr>,
        tcp_addr: Option<SocketAddr>,
        ws_addr: Option<SocketAddr>,
        max_message_size: usize,
    ) -> Result<()> {
        if let Some(addr) = udp_addr {
            let transport = UdpTransport::bind(addr, max_message_size).await?;
            let transport = Arc::new(transport);
            self.udp = Some(Arc::clone(&transport));

            // Bridge into the unified channel.
            let tx = self.inbound_tx.clone();
            tokio::spawn(async move {
                loop {
                    match transport.recv().await {
                        Ok((msg, peer)) => {
                            if tx.send((msg, peer, TransportType::UDP)).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        if let Some(addr) = tcp_addr {
            let transport = TcpTransport::bind(addr, max_message_size).await?;
            let transport = Arc::new(transport);
            self.tcp = Some(Arc::clone(&transport));

            let tx = self.inbound_tx.clone();
            tokio::spawn(async move {
                loop {
                    match transport.recv().await {
                        Ok((msg, peer)) => {
                            if tx.send((msg, peer, TransportType::TCP)).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        if let Some(addr) = ws_addr {
            let transport = WsTransport::bind(addr).await?;
            let transport = Arc::new(transport);
            self.ws = Some(Arc::clone(&transport));

            let tx = self.inbound_tx.clone();
            tokio::spawn(async move {
                loop {
                    match transport.recv().await {
                        Ok((msg, peer)) => {
                            if tx.send((msg, peer, TransportType::WS)).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        info!("TransportManager started");
        Ok(())
    }

    /// Send a SIP message over the specified transport to the given address.
    pub async fn send_message(
        &self,
        message: &SipMessage,
        transport_type: TransportType,
        addr: SocketAddr,
    ) -> Result<()> {
        match transport_type {
            TransportType::UDP => {
                let t = self
                    .udp
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("UDP transport not started"))?;
                t.send(message, addr).await
            }
            TransportType::TCP | TransportType::TLS => {
                let t = self
                    .tcp
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("TCP transport not started"))?;
                t.send(message, addr).await
            }
            TransportType::WS | TransportType::WSS => {
                let t = self
                    .ws
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("WS transport not started"))?;
                t.send(message, addr).await
            }
        }
    }

    /// Receive the next inbound SIP message from any transport.
    pub async fn recv_message(&self) -> Result<(SipMessage, SocketAddr, TransportType)> {
        let mut rx = self.inbound_rx.lock().await;
        rx.recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("All transport channels closed"))
    }

    /// Signal all transports to shut down.
    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
        info!("TransportManager shutdown signal sent");
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the byte offset of `\r\n\r\n` (header/body separator).
fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Extract the Content-Length value from the header portion of the message.
fn extract_content_length(headers: &[u8]) -> Option<usize> {
    let text = std::str::from_utf8(headers).ok()?;
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with("content-length:") || lower.starts_with("l:") {
            let val = line.split(':').nth(1)?.trim();
            return val.parse().ok();
        }
    }
    None
}
