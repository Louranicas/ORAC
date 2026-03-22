//! # M07: IPC Client
//!
//! Async `tokio`-based Unix socket client that connects to the PV2 daemon's
//! IPC bus at `/run/user/1000/pane-vortex-bus.sock`. This is the inverse of
//! PV2's server — ORAC is a **client** of the bus, subscribing to events and
//! submitting tasks.
//!
//! ## Layer: L2 (Wire) | Module: M07
//! ## Dependencies: L1 (`m01_core_types`, `m02_error_handling`), L2 (`m08_bus_types`)
//!
//! ## Wire Protocol (V2)
//! - Transport: `Unix domain socket`, `NDJSON` framing (one JSON object per line)
//! - Handshake: Client sends `Handshake{pane_id, version}`, server replies `Welcome`
//! - Subscribe: Client sends `Subscribe{patterns}`, server replies `Subscribed{count}`
//! - Events: Server pushes `Event{event}` frames matching subscriptions
//! - Tasks: Client sends `Submit{task}`, server replies `TaskSubmitted{task_id}`
//!
//! ## Design
//! - Configurable socket path (default: `/run/user/1000/pane-vortex-bus.sock`)
//! - Persistent socket halves stored after handshake for ongoing I/O
//! - All send/receive methods are async (tokio)
//! - `subscribe` sends `Subscribe` frame and awaits `Subscribed` response
//! - `send_frame` serializes to NDJSON and writes to the socket
//! - `recv_frame` reads a line from the socket and parses NDJSON

use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;

use crate::m1_core::m01_core_types::PaneId;
use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m2_wire::m08_bus_types::BusFrame;
use crate::m2_wire::m09_wire_protocol::MAX_FRAME_SIZE;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default PV2 IPC bus socket path.
pub const DEFAULT_SOCKET_PATH: &str = "/run/user/1000/pane-vortex-bus.sock";

/// ORAC's protocol version string sent during handshake.
const PROTOCOL_VERSION: &str = "2.0";

/// Handshake timeout (seconds).
const HANDSHAKE_TIMEOUT_SECS: u64 = 5;

/// Subscribe response timeout (seconds).
const SUBSCRIBE_TIMEOUT_SECS: u64 = 5;

/// Receive timeout for individual frame reads (seconds).
const RECV_TIMEOUT_SECS: u64 = 30;

// ──────────────────────────────────────────────────────────────
// IPC Client
// ──────────────────────────────────────────────────────────────

/// Connection state of the IPC client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected.
    Disconnected,
    /// Handshake in progress.
    Connecting,
    /// Fully connected and welcomed.
    Connected,
}

/// Async IPC client for connecting to the PV2 daemon bus.
///
/// Manages a single Unix socket connection with `NDJSON` framing.
/// After [`connect`](Self::connect), the socket reader and writer halves
/// are stored for use by [`subscribe`](Self::subscribe),
/// [`send_frame`](Self::send_frame), and [`recv_frame`](Self::recv_frame).
///
/// # Thread Safety
/// The client struct itself holds configuration and state. All I/O methods
/// require `&mut self` for exclusive socket access.
pub struct IpcClient {
    /// Path to the PV2 bus socket.
    socket_path: PathBuf,
    /// ORAC's identity on the bus.
    pane_id: PaneId,
    /// Current connection state.
    state: ConnectionState,
    /// Session ID assigned by the server (if connected).
    session_id: Option<String>,
    /// Event subscription patterns.
    subscriptions: Vec<String>,
    /// Writer half of the connected socket (populated after handshake).
    writer: Option<OwnedWriteHalf>,
    /// Buffered reader half of the connected socket (populated after handshake).
    reader: Option<BufReader<OwnedReadHalf>>,
}

impl std::fmt::Debug for IpcClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IpcClient")
            .field("socket_path", &self.socket_path)
            .field("pane_id", &self.pane_id)
            .field("state", &self.state)
            .field("session_id", &self.session_id)
            .field("subscriptions", &self.subscriptions)
            .field("has_writer", &self.writer.is_some())
            .field("has_reader", &self.reader.is_some())
            .finish()
    }
}

impl IpcClient {
    /// Create a new IPC client with the given identity.
    ///
    /// Does not connect immediately — call [`connect`](Self::connect) to
    /// establish the socket connection.
    #[must_use]
    pub fn new(pane_id: PaneId) -> Self {
        Self {
            socket_path: PathBuf::from(DEFAULT_SOCKET_PATH),
            pane_id,
            state: ConnectionState::Disconnected,
            session_id: None,
            subscriptions: Vec::new(),
            writer: None,
            reader: None,
        }
    }

    /// Create a new IPC client with a custom socket path.
    #[must_use]
    pub fn with_socket_path(pane_id: PaneId, socket_path: impl Into<PathBuf>) -> Self {
        Self {
            socket_path: socket_path.into(),
            pane_id,
            state: ConnectionState::Disconnected,
            session_id: None,
            subscriptions: Vec::new(),
            writer: None,
            reader: None,
        }
    }

    /// The configured socket path.
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// The client's identity on the bus.
    #[must_use]
    pub fn pane_id(&self) -> &PaneId {
        &self.pane_id
    }

    /// Current connection state.
    #[must_use]
    pub const fn connection_state(&self) -> ConnectionState {
        self.state
    }

    /// Whether the client is connected.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self.state, ConnectionState::Connected)
    }

    /// Session ID from the last successful handshake (if any).
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Connect to the PV2 bus socket and perform handshake.
    ///
    /// On success, stores the socket reader and writer halves for use by
    /// [`subscribe`](Self::subscribe), [`send_frame`](Self::send_frame),
    /// and [`recv_frame`](Self::recv_frame).
    ///
    /// # Errors
    /// - [`PvError::BusSocket`] if the socket cannot be opened.
    /// - [`PvError::BusProtocol`] if the handshake fails or times out.
    pub async fn connect(&mut self) -> PvResult<()> {
        self.state = ConnectionState::Connecting;

        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
            UnixStream::connect(&self.socket_path),
        )
        .await
        .map_err(|_| PvError::BusSocket(format!(
            "handshake timeout after {}s connecting to {}",
            HANDSHAKE_TIMEOUT_SECS,
            self.socket_path.display(),
        )))?
        .map_err(|e| PvError::BusSocket(format!(
            "failed to connect to {}: {e}",
            self.socket_path.display(),
        )))?;

        let (read_half, write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        let mut writer = write_half;

        // Send handshake
        let handshake = BusFrame::Handshake {
            pane_id: self.pane_id.clone(),
            version: PROTOCOL_VERSION.to_owned(),
        };
        write_ndjson_frame(&mut writer, &handshake).await?;

        // Read welcome
        let welcome = read_ndjson_frame(&mut reader, HANDSHAKE_TIMEOUT_SECS).await?;

        match welcome {
            BusFrame::Welcome { session_id, .. } => {
                self.session_id = Some(session_id);
                self.state = ConnectionState::Connected;
                self.writer = Some(writer);
                self.reader = Some(reader);
                Ok(())
            }
            BusFrame::Error { code, message } => {
                self.state = ConnectionState::Disconnected;
                Err(PvError::BusProtocol(format!(
                    "server rejected handshake: [{code}] {message}"
                )))
            }
            other => {
                self.state = ConnectionState::Disconnected;
                Err(PvError::BusProtocol(format!(
                    "expected Welcome, got {}", other.frame_type()
                )))
            }
        }
    }

    /// Subscribe to event patterns on the bus.
    ///
    /// Sends a `Subscribe` frame with the given patterns and awaits the
    /// server's `Subscribed` response confirming the active subscription count.
    ///
    /// Must be called after [`connect`](Self::connect). Patterns use glob
    /// syntax (e.g. `"field.*"`, `"sphere.registered"`, `"*"`).
    ///
    /// # Errors
    /// - [`PvError::BusSocket`] if not connected or socket write fails.
    /// - [`PvError::BusProtocol`] if the server rejects or times out.
    pub async fn subscribe(&mut self, patterns: &[String]) -> PvResult<usize> {
        let writer = self.connected_writer()?;

        let frame = BusFrame::Subscribe {
            patterns: patterns.to_vec(),
        };
        write_ndjson_frame(writer, &frame).await?;

        let reader = self.reader.as_mut()
            .ok_or_else(|| PvError::BusSocket("no reader available".into()))?;
        let response = read_ndjson_frame(reader, SUBSCRIBE_TIMEOUT_SECS).await?;

        match response {
            BusFrame::Subscribed { count } => {
                self.subscriptions.extend(patterns.iter().cloned());
                Ok(count)
            }
            BusFrame::Error { code, message } => {
                Err(PvError::BusProtocol(format!(
                    "subscribe rejected: [{code}] {message}"
                )))
            }
            other => {
                Err(PvError::BusProtocol(format!(
                    "expected Subscribed, got {}", other.frame_type()
                )))
            }
        }
    }

    /// Send a raw [`BusFrame`] to the server.
    ///
    /// Serializes the frame to `NDJSON` and writes it to the socket.
    /// The frame size is validated against [`MAX_FRAME_SIZE`] before sending.
    ///
    /// # Errors
    /// - [`PvError::BusSocket`] if not connected or write fails.
    /// - [`PvError::BusProtocol`] if serialization fails or frame exceeds size limit.
    pub async fn send_frame(&mut self, frame: &BusFrame) -> PvResult<()> {
        let writer = self.connected_writer()?;
        write_ndjson_frame(writer, frame).await
    }

    /// Receive the next [`BusFrame`] from the server.
    ///
    /// Reads a single `NDJSON` line from the socket and deserializes it.
    /// Times out after [`RECV_TIMEOUT_SECS`] seconds.
    ///
    /// # Errors
    /// - [`PvError::BusSocket`] if not connected, the socket is closed, or read times out.
    /// - [`PvError::BusProtocol`] if the line cannot be parsed as a valid [`BusFrame`].
    pub async fn recv_frame(&mut self) -> PvResult<BusFrame> {
        if !self.is_connected() {
            return Err(PvError::BusSocket("not connected".into()));
        }
        let reader = self.reader.as_mut()
            .ok_or_else(|| PvError::BusSocket("no reader available".into()))?;
        read_ndjson_frame(reader, RECV_TIMEOUT_SECS).await
    }

    /// Gracefully disconnect from the bus.
    ///
    /// Sends a `Disconnect` frame (best-effort) and drops the socket halves.
    ///
    /// # Errors
    /// Returns `Ok(())` even if the disconnect frame cannot be sent (the
    /// socket is closed regardless).
    pub async fn disconnect(&mut self) -> PvResult<()> {
        if !self.is_connected() {
            return Ok(());
        }

        // Best-effort: send Disconnect frame before closing
        if let Some(writer) = self.writer.as_mut() {
            let frame = BusFrame::Disconnect {
                reason: "orac-sidecar shutdown".to_owned(),
            };
            // Ignore write errors — we're closing anyway
            let _ = write_ndjson_frame(writer, &frame).await;
        }

        // Drop socket halves (closes the connection)
        self.writer = None;
        self.reader = None;
        self.state = ConnectionState::Disconnected;
        self.session_id = None;
        self.subscriptions.clear();
        Ok(())
    }

    /// Current active subscription patterns.
    #[must_use]
    pub fn subscriptions(&self) -> &[String] {
        &self.subscriptions
    }

    /// Get a mutable reference to the writer, or error if not connected.
    ///
    /// # Errors
    /// Returns [`PvError::BusSocket`] if not connected or writer is absent.
    fn connected_writer(&mut self) -> PvResult<&mut OwnedWriteHalf> {
        if !self.is_connected() {
            return Err(PvError::BusSocket("not connected".into()));
        }
        self.writer.as_mut()
            .ok_or_else(|| PvError::BusSocket("no writer available".into()))
    }
}

// ──────────────────────────────────────────────────────────────
// NDJSON wire helpers
// ──────────────────────────────────────────────────────────────

/// Serialize a [`BusFrame`] to `NDJSON` and write it to the socket.
///
/// # Errors
/// - [`PvError::BusProtocol`] if serialization fails or frame exceeds [`MAX_FRAME_SIZE`].
/// - [`PvError::BusSocket`] if the socket write or flush fails.
async fn write_ndjson_frame(writer: &mut OwnedWriteHalf, frame: &BusFrame) -> PvResult<()> {
    let line = frame
        .to_ndjson()
        .map_err(|e| PvError::BusProtocol(format!("frame serialize: {e}")))?;

    if line.len() > MAX_FRAME_SIZE {
        return Err(PvError::BusProtocol(format!(
            "frame too large: {} bytes > {MAX_FRAME_SIZE}",
            line.len(),
        )));
    }

    writer
        .write_all(format!("{line}\n").as_bytes())
        .await
        .map_err(|e| PvError::BusSocket(format!("frame write: {e}")))?;
    writer
        .flush()
        .await
        .map_err(|e| PvError::BusSocket(format!("frame flush: {e}")))?;

    Ok(())
}

/// Read a single `NDJSON` line from the socket and parse it as a [`BusFrame`].
///
/// # Errors
/// - [`PvError::BusSocket`] if the read times out or the connection is closed.
/// - [`PvError::BusProtocol`] if the line cannot be parsed.
async fn read_ndjson_frame(
    reader: &mut BufReader<OwnedReadHalf>,
    timeout_secs: u64,
) -> PvResult<BusFrame> {
    let mut line = String::new();

    let bytes_read = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        reader.read_line(&mut line),
    )
    .await
    .map_err(|_| PvError::BusSocket(format!("read timeout after {timeout_secs}s")))?
    .map_err(|e| PvError::BusSocket(format!("socket read: {e}")))?;

    if bytes_read == 0 {
        return Err(PvError::BusSocket("connection closed by server".into()));
    }

    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(PvError::BusProtocol("empty frame received".into()));
    }

    if trimmed.len() > MAX_FRAME_SIZE {
        return Err(PvError::BusProtocol(format!(
            "frame too large: {} bytes > {MAX_FRAME_SIZE}",
            trimmed.len(),
        )));
    }

    BusFrame::from_ndjson(trimmed)
        .map_err(|e| PvError::BusProtocol(format!("frame parse: {e}")))
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    // ── Construction ──

    #[test]
    fn client_new_defaults() {
        let client = IpcClient::new(pid("orac-sidecar"));
        assert_eq!(client.pane_id().as_str(), "orac-sidecar");
        assert_eq!(client.socket_path(), Path::new(DEFAULT_SOCKET_PATH));
        assert_eq!(client.connection_state(), ConnectionState::Disconnected);
        assert!(!client.is_connected());
        assert!(client.session_id().is_none());
        assert!(client.writer.is_none());
        assert!(client.reader.is_none());
    }

    #[test]
    fn client_custom_socket_path() {
        let client = IpcClient::with_socket_path(
            pid("test"),
            "/tmp/test-bus.sock",
        );
        assert_eq!(client.socket_path(), Path::new("/tmp/test-bus.sock"));
    }

    #[test]
    fn client_subscriptions_initially_empty() {
        let client = IpcClient::new(pid("test"));
        assert!(client.subscriptions().is_empty());
    }

    // ── Connection state ──

    #[test]
    fn connection_state_disconnected() {
        let client = IpcClient::new(pid("test"));
        assert_eq!(client.connection_state(), ConnectionState::Disconnected);
        assert!(!client.is_connected());
    }

    // ── Async operations (without a real socket) ──

    #[tokio::test]
    async fn connect_missing_socket_fails() {
        let mut client = IpcClient::with_socket_path(
            pid("test"),
            "/tmp/nonexistent-orac-test-bus.sock",
        );
        let result = client.connect().await;
        assert!(result.is_err());
        assert!(!client.is_connected());
        assert!(client.writer.is_none());
        assert!(client.reader.is_none());
    }

    #[tokio::test]
    async fn subscribe_without_connect_fails() {
        let mut client = IpcClient::new(pid("test"));
        let result = client.subscribe(&["field.*".into()]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn send_frame_without_connect_fails() {
        let mut client = IpcClient::new(pid("test"));
        let frame = BusFrame::Subscribe {
            patterns: vec!["*".into()],
        };
        let result = client.send_frame(&frame).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn recv_frame_without_connect_fails() {
        let mut client = IpcClient::new(pid("test"));
        let result = client.recv_frame().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn disconnect_when_not_connected_ok() {
        let mut client = IpcClient::new(pid("test"));
        let result = client.disconnect().await;
        assert!(result.is_ok());
    }

    // ── Loopback integration: connect, subscribe, send, recv, disconnect ──

    #[tokio::test]
    async fn loopback_handshake_subscribe_disconnect() {
        let socket_path = "/tmp/orac-test-m07-loopback.sock";
        let _ = tokio::fs::remove_file(socket_path).await;

        let listener = tokio::net::UnixListener::bind(socket_path).unwrap();

        // Spawn mock server
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = writer;

            // Read Handshake
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let frame: BusFrame = serde_json::from_str(line.trim()).unwrap();
            assert!(matches!(frame, BusFrame::Handshake { .. }));

            // Send Welcome
            let welcome = BusFrame::Welcome {
                session_id: "test-session-001".into(),
                version: "2.0".into(),
            };
            let resp = serde_json::to_string(&welcome).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Read Subscribe
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            let frame: BusFrame = serde_json::from_str(line.trim()).unwrap();
            assert!(matches!(frame, BusFrame::Subscribe { .. }));

            // Send Subscribed
            let subscribed = BusFrame::Subscribed { count: 2 };
            let resp = serde_json::to_string(&subscribed).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Read a raw frame (the send_frame test sends Submit)
            line.clear();
            reader.read_line(&mut line).await.unwrap();
            let frame: BusFrame = serde_json::from_str(line.trim()).unwrap();
            assert!(matches!(frame, BusFrame::Cascade { .. }));

            // Send an Event for recv_frame to pick up
            let event = BusFrame::Event {
                event: crate::m2_wire::m08_bus_types::BusEvent::text(
                    "field.tick",
                    "test event",
                    42,
                ),
            };
            let resp = serde_json::to_string(&event).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Read Disconnect
            line.clear();
            let _ = reader.read_line(&mut line).await;
        });

        // Client side
        let mut client = IpcClient::with_socket_path(pid("orac-test"), socket_path);

        // Connect
        client.connect().await.unwrap();
        assert!(client.is_connected());
        assert_eq!(client.session_id(), Some("test-session-001"));
        assert!(client.writer.is_some());
        assert!(client.reader.is_some());

        // Subscribe
        let count = client
            .subscribe(&["field.*".into(), "task.*".into()])
            .await
            .unwrap();
        assert_eq!(count, 2);
        assert_eq!(client.subscriptions().len(), 2);

        // Send frame
        let cascade = BusFrame::Cascade {
            source: pid("orac-test"),
            target: pid("alpha"),
            brief: "test cascade".into(),
        };
        client.send_frame(&cascade).await.unwrap();

        // Recv frame
        let received = client.recv_frame().await.unwrap();
        assert!(matches!(received, BusFrame::Event { .. }));

        // Disconnect
        client.disconnect().await.unwrap();
        assert!(!client.is_connected());
        assert!(client.writer.is_none());
        assert!(client.reader.is_none());
        assert!(client.subscriptions().is_empty());

        let _ = server.await;
        let _ = tokio::fs::remove_file(socket_path).await;
    }

    #[tokio::test]
    async fn connect_rejected_by_server() {
        let socket_path = "/tmp/orac-test-m07-reject.sock";
        let _ = tokio::fs::remove_file(socket_path).await;

        let listener = tokio::net::UnixListener::bind(socket_path).unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = writer;

            // Read Handshake
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();

            // Send Error instead of Welcome
            let err = BusFrame::Error {
                code: 403,
                message: "unauthorized".into(),
            };
            let resp = serde_json::to_string(&err).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();
        });

        let mut client = IpcClient::with_socket_path(pid("orac-test"), socket_path);
        let result = client.connect().await;
        assert!(result.is_err());
        assert!(!client.is_connected());

        let _ = server.await;
        let _ = tokio::fs::remove_file(socket_path).await;
    }

    #[tokio::test]
    async fn subscribe_rejected_by_server() {
        let socket_path = "/tmp/orac-test-m07-sub-reject.sock";
        let _ = tokio::fs::remove_file(socket_path).await;

        let listener = tokio::net::UnixListener::bind(socket_path).unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = writer;

            // Handshake dance
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let welcome = BusFrame::Welcome {
                session_id: "s".into(),
                version: "2.0".into(),
            };
            let resp = serde_json::to_string(&welcome).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Read Subscribe
            line.clear();
            reader.read_line(&mut line).await.unwrap();

            // Reply with Error
            let err = BusFrame::Error {
                code: 400,
                message: "bad pattern".into(),
            };
            let resp = serde_json::to_string(&err).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();
        });

        let mut client = IpcClient::with_socket_path(pid("orac-test"), socket_path);
        client.connect().await.unwrap();

        let result = client.subscribe(&["***invalid***".into()]).await;
        assert!(result.is_err());

        let _ = server.await;
        let _ = tokio::fs::remove_file(socket_path).await;
    }

    #[tokio::test]
    async fn recv_frame_server_closes_connection() {
        let socket_path = "/tmp/orac-test-m07-eof.sock";
        let _ = tokio::fs::remove_file(socket_path).await;

        let listener = tokio::net::UnixListener::bind(socket_path).unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = writer;

            // Handshake
            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let welcome = BusFrame::Welcome {
                session_id: "s".into(),
                version: "2.0".into(),
            };
            let resp = serde_json::to_string(&welcome).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Close connection (drop writer)
            drop(writer);
        });

        let mut client = IpcClient::with_socket_path(pid("orac-test"), socket_path);
        client.connect().await.unwrap();

        // Give server time to close
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let result = client.recv_frame().await;
        assert!(result.is_err());

        let _ = server.await;
        let _ = tokio::fs::remove_file(socket_path).await;
    }

    #[tokio::test]
    async fn disconnect_clears_all_state() {
        let socket_path = "/tmp/orac-test-m07-disc-state.sock";
        let _ = tokio::fs::remove_file(socket_path).await;

        let listener = tokio::net::UnixListener::bind(socket_path).unwrap();

        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let (reader, writer) = stream.into_split();
            let mut reader = BufReader::new(reader);
            let mut writer = writer;

            let mut line = String::new();
            reader.read_line(&mut line).await.unwrap();
            let welcome = BusFrame::Welcome {
                session_id: "disc-test".into(),
                version: "2.0".into(),
            };
            let resp = serde_json::to_string(&welcome).unwrap();
            writer.write_all(format!("{resp}\n").as_bytes()).await.unwrap();
            writer.flush().await.unwrap();

            // Read whatever comes (Disconnect or EOF)
            line.clear();
            let _ = reader.read_line(&mut line).await;
        });

        let mut client = IpcClient::with_socket_path(pid("orac-test"), socket_path);
        client.connect().await.unwrap();
        assert!(client.is_connected());
        assert!(client.session_id().is_some());

        client.disconnect().await.unwrap();
        assert_eq!(client.connection_state(), ConnectionState::Disconnected);
        assert!(client.session_id().is_none());
        assert!(client.subscriptions().is_empty());
        assert!(client.writer.is_none());
        assert!(client.reader.is_none());

        let _ = server.await;
        let _ = tokio::fs::remove_file(socket_path).await;
    }

    // ── Constants ──

    #[test]
    fn default_socket_path_correct() {
        assert_eq!(DEFAULT_SOCKET_PATH, "/run/user/1000/pane-vortex-bus.sock");
    }

    #[test]
    fn protocol_version_is_v2() {
        assert_eq!(PROTOCOL_VERSION, "2.0");
    }

    #[test]
    fn debug_format_works() {
        let client = IpcClient::new(pid("test"));
        let debug = format!("{client:?}");
        assert!(debug.contains("IpcClient"));
        assert!(debug.contains("test"));
    }

    #[test]
    fn connected_writer_fails_when_disconnected() {
        let mut client = IpcClient::new(pid("test"));
        assert!(client.connected_writer().is_err());
    }
}
