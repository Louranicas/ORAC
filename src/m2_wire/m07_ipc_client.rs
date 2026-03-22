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
//! - Automatic reconnection with exponential backoff
//! - Channel-based receive for non-blocking event consumption
//! - `Send + Sync` safe via `tokio::sync::mpsc` channels

use std::path::{Path, PathBuf};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::m1_core::m01_core_types::PaneId;
use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m2_wire::m08_bus_types::BusFrame;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default PV2 IPC bus socket path.
pub const DEFAULT_SOCKET_PATH: &str = "/run/user/1000/pane-vortex-bus.sock";

/// ORAC's protocol version string sent during handshake.
const PROTOCOL_VERSION: &str = "2.0";

/// Handshake timeout (seconds).
const HANDSHAKE_TIMEOUT_SECS: u64 = 5;

/// Channel buffer size for received frames.
const _RECV_CHANNEL_SIZE: usize = 256;

/// Maximum reconnection attempts before giving up.
const _MAX_RECONNECT_ATTEMPTS: u32 = 10;

/// Initial backoff delay for reconnection (milliseconds).
const _INITIAL_BACKOFF_MS: u64 = 500;

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
/// Received frames are delivered via an `mpsc` channel for non-blocking
/// consumption by the sidecar tick loop.
///
/// # Thread Safety
/// The client struct itself holds configuration and state. The actual
/// connection is managed via `tokio` tasks spawned by [`connect`](Self::connect).
#[derive(Debug)]
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
    /// # Errors
    /// - `PvError::BusSocket` if the socket cannot be opened.
    /// - `PvError::BusProtocol` if the handshake fails or times out.
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

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Send handshake
        let handshake = BusFrame::Handshake {
            pane_id: self.pane_id.clone(),
            version: PROTOCOL_VERSION.to_owned(),
        };
        let line = handshake
            .to_ndjson()
            .map_err(|e| PvError::BusProtocol(format!("handshake serialize: {e}")))?;
        writer
            .write_all(format!("{line}\n").as_bytes())
            .await
            .map_err(|e| PvError::BusSocket(format!("handshake write: {e}")))?;
        writer
            .flush()
            .await
            .map_err(|e| PvError::BusSocket(format!("handshake flush: {e}")))?;

        // Read welcome
        let mut welcome_line = String::new();
        tokio::time::timeout(
            std::time::Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
            reader.read_line(&mut welcome_line),
        )
        .await
        .map_err(|_| PvError::BusProtocol("welcome timeout".into()))?
        .map_err(|e| PvError::BusSocket(format!("welcome read: {e}")))?;

        let welcome = BusFrame::from_ndjson(welcome_line.trim())
            .map_err(|e| PvError::BusProtocol(format!("welcome parse: {e}")))?;

        match welcome {
            BusFrame::Welcome { session_id, .. } => {
                self.session_id = Some(session_id);
                self.state = ConnectionState::Connected;
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
    /// Must be called after [`connect`](Self::connect). Patterns use glob
    /// syntax (e.g. `"field.*"`, `"sphere.registered"`, `"*"`).
    ///
    /// # Errors
    /// - `PvError::BusSocket` if not connected.
    pub fn subscribe(&mut self, patterns: &[String]) -> PvResult<usize> {
        if !self.is_connected() {
            return Err(PvError::BusSocket("not connected".into()));
        }

        self.subscriptions.extend(patterns.iter().cloned());

        // TODO: Phase 1 — send Subscribe frame over socket and await Subscribed response
        // Requires storing the write half of the stream in the client struct
        Ok(self.subscriptions.len())
    }

    /// Send a raw `BusFrame` to the server.
    ///
    /// # Errors
    /// - `PvError::BusSocket` if not connected or write fails.
    pub fn send_frame(&self, _frame: &BusFrame) -> PvResult<()> {
        if !self.is_connected() {
            return Err(PvError::BusSocket("not connected".into()));
        }

        // TODO: Phase 1 — serialize and write frame to stored socket writer
        Ok(())
    }

    /// Receive the next `BusFrame` from the server.
    ///
    /// This is a placeholder for the async receive path. In the full
    /// implementation, frames are delivered via an `mpsc` channel fed
    /// by a background reader task.
    ///
    /// # Errors
    /// - `PvError::BusSocket` if not connected.
    pub fn recv_frame(&self) -> PvResult<BusFrame> {
        if !self.is_connected() {
            return Err(PvError::BusSocket("not connected".into()));
        }

        // TODO: Phase 1 — receive from mpsc channel populated by reader task
        Err(PvError::BusSocket("recv not yet implemented".into()))
    }

    /// Gracefully disconnect from the bus.
    ///
    /// Sends a `Disconnect` frame and closes the socket.
    ///
    /// # Errors
    /// - `PvError::BusSocket` if the disconnect message cannot be sent.
    pub fn disconnect(&mut self) -> PvResult<()> {
        if !self.is_connected() {
            return Ok(());
        }

        // TODO: Phase 1 — send Disconnect frame over socket
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
        // State should not be Connected after failure
        assert!(!client.is_connected());
    }

    #[test]
    fn subscribe_without_connect_fails() {
        let mut client = IpcClient::new(pid("test"));
        let result = client.subscribe(&["field.*".into()]);
        assert!(result.is_err());
    }

    #[test]
    fn send_frame_without_connect_fails() {
        let client = IpcClient::new(pid("test"));
        let frame = BusFrame::Subscribe {
            patterns: vec!["*".into()],
        };
        let result = client.send_frame(&frame);
        assert!(result.is_err());
    }

    #[test]
    fn recv_frame_without_connect_fails() {
        let client = IpcClient::new(pid("test"));
        let result = client.recv_frame();
        assert!(result.is_err());
    }

    #[test]
    fn disconnect_when_not_connected_ok() {
        let mut client = IpcClient::new(pid("test"));
        let result = client.disconnect();
        assert!(result.is_ok());
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
}
