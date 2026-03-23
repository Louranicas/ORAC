//! # M09: Wire Protocol
//!
//! V2 wire protocol layer: frame validation, protocol state machine,
//! keepalive management, and message sequencing. Sits between the raw
//! `NDJSON` framing in [`m08_bus_types`] and the IPC client in [`m07_ipc_client`].
//!
//! ## Layer: L2 (Wire)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`, `m08_bus_types`
//!
//! ## Protocol State Machine
//!
//! ```text
//! Disconnected → Handshaking → Connected → Subscribing → Active
//!      ↑                                                    |
//!      └────────────────── (error/timeout) ─────────────────┘
//! ```
//!
//! ## Frame Validation
//!
//! - Client frames must not be received by the client
//! - Server frames must not be sent by the client
//! - Handshake must be the first frame sent
//! - Subscribe only valid after `Welcome`

use std::collections::VecDeque;
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::PaneId;
use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m2_wire::m08_bus_types::BusFrame;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Protocol version string.
pub const PROTOCOL_VERSION: &str = "2.0";

/// Default keepalive interval (seconds).
pub const DEFAULT_KEEPALIVE_SECS: u64 = 30;

/// Maximum frame size in bytes (64 KB).
pub const MAX_FRAME_SIZE: usize = 65_536;

/// Maximum frames queued for sending.
const MAX_SEND_QUEUE: usize = 1000;

/// Maximum receive buffer frames.
const MAX_RECV_BUFFER: usize = 500;

// ──────────────────────────────────────────────────────────────
// Protocol state
// ──────────────────────────────────────────────────────────────

/// State of the wire protocol connection.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProtocolState {
    /// Not connected.
    #[default]
    Disconnected,
    /// Handshake sent, awaiting `Welcome`.
    Handshaking,
    /// `Welcome` received, connection established.
    Connected,
    /// `Subscribe` sent, awaiting `Subscribed` confirmation.
    Subscribing,
    /// Fully active — sending/receiving events.
    Active,
    /// Connection closing (after `Disconnect` sent).
    Closing,
}

impl ProtocolState {
    /// Whether this state allows sending event/task frames.
    #[must_use]
    pub const fn can_send_data(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether this state allows receiving event frames.
    #[must_use]
    pub const fn can_receive_events(&self) -> bool {
        matches!(self, Self::Active | Self::Connected)
    }

    /// Whether this state is terminal.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Disconnected | Self::Closing)
    }
}

impl fmt::Display for ProtocolState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => f.write_str("disconnected"),
            Self::Handshaking => f.write_str("handshaking"),
            Self::Connected => f.write_str("connected"),
            Self::Subscribing => f.write_str("subscribing"),
            Self::Active => f.write_str("active"),
            Self::Closing => f.write_str("closing"),
        }
    }
}

/// Result of validating an incoming frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameValidation {
    /// Frame is valid for the current protocol state.
    Valid,
    /// Frame is unexpected for the current state.
    UnexpectedFrame {
        /// The frame type received.
        frame_type: String,
        /// The current protocol state.
        state: ProtocolState,
    },
    /// Frame exceeds maximum size.
    Oversized {
        /// Actual size in bytes.
        size: usize,
    },
    /// Frame failed deserialization.
    Malformed {
        /// Parse error description.
        reason: String,
    },
}

// ──────────────────────────────────────────────────────────────
// Wire protocol stats
// ──────────────────────────────────────────────────────────────

/// Aggregate wire protocol statistics.
#[derive(Debug, Clone, Default)]
pub struct WireStats {
    /// Total frames sent.
    pub frames_sent: u64,
    /// Total frames received.
    pub frames_received: u64,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Total validation errors.
    pub validation_errors: u64,
    /// Total keepalives sent.
    pub keepalives_sent: u64,
    /// Total state transitions.
    pub state_transitions: u64,
}

// ──────────────────────────────────────────────────────────────
// Wire protocol
// ──────────────────────────────────────────────────────────────

/// V2 wire protocol manager.
///
/// Manages protocol state transitions, frame validation, keepalive
/// scheduling, and send/receive queues.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct WireProtocol {
    /// Current protocol state.
    state: RwLock<ProtocolState>,
    /// Client's pane ID.
    pane_id: PaneId,
    /// Session ID assigned by server (set after `Welcome`).
    session_id: RwLock<Option<String>>,
    /// Active subscription patterns.
    subscriptions: RwLock<Vec<String>>,
    /// Outbound frame queue.
    send_queue: RwLock<VecDeque<String>>,
    /// Inbound frame buffer (validated, pending consumption).
    recv_buffer: RwLock<VecDeque<BusFrame>>,
    /// Keepalive interval (seconds).
    keepalive_secs: u64,
    /// Last activity timestamp (seconds since epoch).
    last_activity: RwLock<f64>,
    /// Aggregate statistics.
    stats: RwLock<WireStats>,
}

impl fmt::Debug for WireProtocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WireProtocol")
            .field("state", &*self.state.read())
            .field("pane_id", &self.pane_id)
            .field("subscriptions", &self.subscriptions.read().len())
            .finish_non_exhaustive()
    }
}

impl WireProtocol {
    /// Create a new wire protocol manager for the given pane ID.
    #[must_use]
    pub fn new(pane_id: PaneId) -> Self {
        Self {
            state: RwLock::new(ProtocolState::Disconnected),
            pane_id,
            session_id: RwLock::new(None),
            subscriptions: RwLock::new(Vec::new()),
            send_queue: RwLock::new(VecDeque::with_capacity(64)),
            recv_buffer: RwLock::new(VecDeque::with_capacity(64)),
            keepalive_secs: DEFAULT_KEEPALIVE_SECS,
            last_activity: RwLock::new(0.0),
            stats: RwLock::new(WireStats::default()),
        }
    }

    /// Create a wire protocol manager with custom keepalive interval.
    #[must_use]
    pub fn with_keepalive(pane_id: PaneId, keepalive_secs: u64) -> Self {
        let mut wp = Self::new(pane_id);
        wp.keepalive_secs = keepalive_secs;
        wp
    }

    // ── State accessors ──

    /// Current protocol state.
    #[must_use]
    pub fn state(&self) -> ProtocolState {
        *self.state.read()
    }

    /// Session ID assigned by server, if connected.
    #[must_use]
    pub fn session_id(&self) -> Option<String> {
        self.session_id.read().clone()
    }

    /// Active subscription patterns.
    #[must_use]
    pub fn subscriptions(&self) -> Vec<String> {
        self.subscriptions.read().clone()
    }

    /// Aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> WireStats {
        self.stats.read().clone()
    }

    /// Number of frames queued for sending.
    #[must_use]
    pub fn send_queue_len(&self) -> usize {
        self.send_queue.read().len()
    }

    /// Number of validated frames waiting to be consumed.
    #[must_use]
    pub fn recv_buffer_len(&self) -> usize {
        self.recv_buffer.read().len()
    }

    // ── Protocol operations ──

    /// Initiate handshake. Queues a `Handshake` frame and transitions to `Handshaking`.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] if not in `Disconnected` state.
    pub fn initiate_handshake(&self) -> PvResult<()> {
        let current = *self.state.read();
        if current != ProtocolState::Disconnected {
            return Err(PvError::BusProtocol(
                format!("cannot handshake in state {current}"),
            ));
        }

        let frame = BusFrame::Handshake {
            pane_id: self.pane_id.clone(),
            version: PROTOCOL_VERSION.to_owned(),
        };

        self.enqueue_frame(&frame)?;
        self.transition_to(ProtocolState::Handshaking);
        Ok(())
    }

    /// Subscribe to event patterns. Queues a `Subscribe` frame.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] if not in `Connected` or `Active` state.
    pub fn subscribe(&self, patterns: Vec<String>) -> PvResult<()> {
        let current = *self.state.read();
        if !matches!(current, ProtocolState::Connected | ProtocolState::Active) {
            return Err(PvError::BusProtocol(
                format!("cannot subscribe in state {current}"),
            ));
        }

        let frame = BusFrame::Subscribe {
            patterns: patterns.clone(),
        };

        self.enqueue_frame(&frame)?;
        self.subscriptions.write().extend(patterns);
        self.transition_to(ProtocolState::Subscribing);
        Ok(())
    }

    /// Submit a task frame for fleet dispatch.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] if not in `Active` state.
    pub fn submit_task(&self, task: crate::m2_wire::m08_bus_types::BusTask) -> PvResult<()> {
        if !self.state().can_send_data() {
            return Err(PvError::BusProtocol(
                format!("cannot submit task in state {}", self.state()),
            ));
        }

        let frame = BusFrame::Submit { task };
        self.enqueue_frame(&frame)
    }

    /// Initiate graceful disconnect.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] if already disconnected.
    pub fn disconnect(&self, reason: &str) -> PvResult<()> {
        if self.state().is_terminal() {
            return Err(PvError::BusProtocol("already disconnected".into()));
        }

        let frame = BusFrame::Disconnect {
            reason: reason.to_owned(),
        };
        self.enqueue_frame(&frame)?;
        self.transition_to(ProtocolState::Closing);
        Ok(())
    }

    // ── Frame processing ──

    /// Validate and process an incoming raw `NDJSON` line.
    ///
    /// Returns the validation result and, if valid, queues the frame
    /// in the receive buffer.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] on protocol violations.
    pub fn process_incoming(&self, raw: &str) -> PvResult<FrameValidation> {
        // Size check
        if raw.len() > MAX_FRAME_SIZE {
            self.stats.write().validation_errors += 1;
            return Ok(FrameValidation::Oversized { size: raw.len() });
        }

        // Parse
        let frame = match BusFrame::from_ndjson(raw) {
            Ok(f) => f,
            Err(e) => {
                self.stats.write().validation_errors += 1;
                return Ok(FrameValidation::Malformed {
                    reason: e.to_string(),
                });
            }
        };

        // Validate against protocol state
        let validation = self.validate_incoming(&frame);
        if let FrameValidation::Valid = &validation {
            self.apply_incoming(&frame);

            let mut recv = self.recv_buffer.write();
            if recv.len() >= MAX_RECV_BUFFER {
                recv.pop_front();
            }
            recv.push_back(frame);

            self.stats.write().frames_received += 1;
            #[allow(clippy::cast_precision_loss)]
            {
                self.stats.write().bytes_received += raw.len() as u64;
            }
            *self.last_activity.write() = crate::m1_core::m01_core_types::now_secs();
        } else {
            self.stats.write().validation_errors += 1;
        }

        Ok(validation)
    }

    /// Validate an incoming frame against the current protocol state.
    fn validate_incoming(&self, frame: &BusFrame) -> FrameValidation {
        let current = *self.state.read();

        match frame {
            BusFrame::Welcome { .. } => {
                if current != ProtocolState::Handshaking {
                    return FrameValidation::UnexpectedFrame {
                        frame_type: "Welcome".into(),
                        state: current,
                    };
                }
            }
            BusFrame::Subscribed { .. } => {
                if current != ProtocolState::Subscribing {
                    return FrameValidation::UnexpectedFrame {
                        frame_type: "Subscribed".into(),
                        state: current,
                    };
                }
            }
            BusFrame::Event { .. } => {
                if !current.can_receive_events() {
                    return FrameValidation::UnexpectedFrame {
                        frame_type: "Event".into(),
                        state: current,
                    };
                }
            }
            BusFrame::Error { .. } | BusFrame::TaskSubmitted { .. } | BusFrame::CascadeAck { .. } => {
                // These can arrive in most states
            }
            // Client frames should not be received
            BusFrame::Handshake { .. }
            | BusFrame::Subscribe { .. }
            | BusFrame::Submit { .. }
            | BusFrame::Disconnect { .. }
            | BusFrame::Cascade { .. } => {
                return FrameValidation::UnexpectedFrame {
                    frame_type: frame.frame_type().into(),
                    state: current,
                };
            }
        }

        FrameValidation::Valid
    }

    /// Apply state transitions from an incoming frame.
    fn apply_incoming(&self, frame: &BusFrame) {
        match frame {
            BusFrame::Welcome { session_id, .. } => {
                *self.session_id.write() = Some(session_id.clone());
                self.transition_to(ProtocolState::Connected);
            }
            BusFrame::Subscribed { .. } => {
                self.transition_to(ProtocolState::Active);
            }
            // All other frames (Error, TaskSubmitted, CascadeAck, etc.) don't change state
            _ => {}
        }
    }

    /// Dequeue the next outbound `NDJSON` line, if any.
    #[must_use]
    pub fn dequeue_send(&self) -> Option<String> {
        self.send_queue.write().pop_front()
    }

    /// Dequeue the next validated incoming frame, if any.
    #[must_use]
    pub fn dequeue_recv(&self) -> Option<BusFrame> {
        self.recv_buffer.write().pop_front()
    }

    /// Drain all validated incoming frames.
    #[must_use]
    pub fn drain_recv(&self) -> Vec<BusFrame> {
        self.recv_buffer.write().drain(..).collect()
    }

    // ── Keepalive ──

    /// Check whether a keepalive should be sent (based on elapsed time).
    #[must_use]
    pub fn needs_keepalive(&self, now: f64) -> bool {
        if !self.state().can_send_data() {
            return false;
        }
        let last = *self.last_activity.read();
        #[allow(clippy::cast_precision_loss)] // keepalive_secs is small (<3600)
        let interval = self.keepalive_secs as f64;
        (now - last) >= interval
    }

    /// Send a keepalive frame and record the activity.
    ///
    /// Increments `keepalives_sent` and updates `last_activity` timestamp.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] if not in `Active` state or if the
    /// send queue is full.
    pub fn send_keepalive(&self) -> PvResult<()> {
        if !self.state().can_send_data() {
            return Err(PvError::BusProtocol(
                format!("cannot send keepalive in state {}", self.state()),
            ));
        }

        // Keepalive is an empty event frame with type "keepalive"
        let frame = BusFrame::Event {
            event: crate::m2_wire::m08_bus_types::BusEvent::text("keepalive", "", 0),
        };
        self.enqueue_frame(&frame)?;
        self.stats.write().keepalives_sent += 1;
        Ok(())
    }

    /// Force a connection reset to `Disconnected`.
    pub fn force_reset(&self) {
        self.transition_to(ProtocolState::Disconnected);
        *self.session_id.write() = None;
        self.subscriptions.write().clear();
        self.send_queue.write().clear();
        self.recv_buffer.write().clear();
    }

    // ── Internal ──

    /// Serialize and enqueue a frame for sending.
    ///
    /// Returns `Err(`[`PvError::BusProtocol`]`)` with a `"queue full"` message
    /// if the send queue is at capacity, rather than silently dropping the
    /// oldest frame. Callers can decide how to handle back-pressure.
    fn enqueue_frame(&self, frame: &BusFrame) -> PvResult<()> {
        let json = frame.to_ndjson().map_err(|e| PvError::BusProtocol(e.to_string()))?;

        if json.len() > MAX_FRAME_SIZE {
            return Err(PvError::BusProtocol(
                format!("frame too large: {} bytes > {MAX_FRAME_SIZE}", json.len()),
            ));
        }

        let mut queue = self.send_queue.write();
        if queue.len() >= MAX_SEND_QUEUE {
            return Err(PvError::BusProtocol(
                format!("send queue full ({MAX_SEND_QUEUE} frames)"),
            ));
        }
        #[allow(clippy::cast_precision_loss)]
        {
            self.stats.write().bytes_sent += json.len() as u64;
        }
        queue.push_back(json);
        self.stats.write().frames_sent += 1;
        *self.last_activity.write() = crate::m1_core::m01_core_types::now_secs();

        Ok(())
    }

    /// Transition to a new state.
    fn transition_to(&self, new_state: ProtocolState) {
        *self.state.write() = new_state;
        self.stats.write().state_transitions += 1;
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_protocol() -> WireProtocol {
        WireProtocol::new(PaneId::new("orac-test"))
    }

    #[test]
    fn initial_state_is_disconnected() {
        let wp = make_protocol();
        assert_eq!(wp.state(), ProtocolState::Disconnected);
        assert!(wp.session_id().is_none());
    }

    #[test]
    fn initiate_handshake_transitions() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        assert_eq!(wp.state(), ProtocolState::Handshaking);
        assert_eq!(wp.send_queue_len(), 1);
    }

    #[test]
    fn handshake_produces_valid_ndjson() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        let line = wp.dequeue_send().unwrap();
        let frame: BusFrame = serde_json::from_str(&line).unwrap();
        assert!(matches!(frame, BusFrame::Handshake { .. }));
    }

    #[test]
    fn double_handshake_rejected() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        assert!(wp.initiate_handshake().is_err());
    }

    #[test]
    fn welcome_transitions_to_connected() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();

        let welcome = r#"{"type":"Welcome","session_id":"sess-1","version":"2.0"}"#;
        let result = wp.process_incoming(welcome).unwrap();
        assert_eq!(result, FrameValidation::Valid);
        assert_eq!(wp.state(), ProtocolState::Connected);
        assert_eq!(wp.session_id().as_deref(), Some("sess-1"));
    }

    #[test]
    fn welcome_in_wrong_state_rejected() {
        let wp = make_protocol();
        // Not handshaking
        let welcome = r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#;
        let result = wp.process_incoming(welcome).unwrap();
        assert!(matches!(result, FrameValidation::UnexpectedFrame { .. }));
    }

    #[test]
    fn subscribe_after_connected() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();

        let welcome = r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#;
        wp.process_incoming(welcome).unwrap();

        wp.subscribe(vec!["field.*".into(), "task.*".into()]).unwrap();
        assert_eq!(wp.state(), ProtocolState::Subscribing);
        assert_eq!(wp.subscriptions().len(), 2);
    }

    #[test]
    fn subscribe_before_connected_rejected() {
        let wp = make_protocol();
        assert!(wp.subscribe(vec!["field.*".into()]).is_err());
    }

    #[test]
    fn subscribed_transitions_to_active() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["field.*".into()]).unwrap();

        let subscribed = r#"{"type":"Subscribed","count":1}"#;
        let result = wp.process_incoming(subscribed).unwrap();
        assert_eq!(result, FrameValidation::Valid);
        assert_eq!(wp.state(), ProtocolState::Active);
    }

    #[test]
    fn event_received_in_active_state() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["field.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        let event = r#"{"type":"Event","event":{"event_type":"field.tick","data":42,"tick":1,"timestamp":0.0}}"#;
        let result = wp.process_incoming(event).unwrap();
        assert_eq!(result, FrameValidation::Valid);
        assert_eq!(wp.recv_buffer_len(), 3); // Welcome + Subscribed + Event
    }

    #[test]
    fn event_in_disconnected_rejected() {
        let wp = make_protocol();
        let event = r#"{"type":"Event","event":{"event_type":"field.tick","data":42,"tick":1,"timestamp":0.0}}"#;
        let result = wp.process_incoming(event).unwrap();
        assert!(matches!(result, FrameValidation::UnexpectedFrame { .. }));
    }

    #[test]
    fn client_frame_received_rejected() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();

        let handshake = r#"{"type":"Handshake","pane_id":"evil","version":"2.0"}"#;
        let result = wp.process_incoming(handshake).unwrap();
        assert!(matches!(result, FrameValidation::UnexpectedFrame { .. }));
    }

    #[test]
    fn malformed_json_detected() {
        let wp = make_protocol();
        let result = wp.process_incoming("not json at all").unwrap();
        assert!(matches!(result, FrameValidation::Malformed { .. }));
    }

    #[test]
    fn oversized_frame_detected() {
        let wp = make_protocol();
        let huge = "x".repeat(MAX_FRAME_SIZE + 1);
        let result = wp.process_incoming(&huge).unwrap();
        assert!(matches!(result, FrameValidation::Oversized { .. }));
    }

    #[test]
    fn submit_task_in_active_state() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["field.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        let task = crate::m2_wire::m08_bus_types::BusTask::new(
            "test task".into(),
            crate::m2_wire::m08_bus_types::TaskTarget::AnyIdle,
            PaneId::new("orac"),
        );
        assert!(wp.submit_task(task).is_ok());
    }

    #[test]
    fn submit_task_before_active_rejected() {
        let wp = make_protocol();
        let task = crate::m2_wire::m08_bus_types::BusTask::new(
            "test".into(),
            crate::m2_wire::m08_bus_types::TaskTarget::AnyIdle,
            PaneId::new("orac"),
        );
        assert!(wp.submit_task(task).is_err());
    }

    #[test]
    fn disconnect_sends_frame() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();

        wp.disconnect("shutting down").unwrap();
        assert_eq!(wp.state(), ProtocolState::Closing);
    }

    #[test]
    fn disconnect_when_disconnected_rejected() {
        let wp = make_protocol();
        assert!(wp.disconnect("test").is_err());
    }

    #[test]
    fn force_reset_clears_all() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();

        wp.force_reset();
        assert_eq!(wp.state(), ProtocolState::Disconnected);
        assert!(wp.session_id().is_none());
        assert!(wp.subscriptions().is_empty());
        assert_eq!(wp.send_queue_len(), 0);
        assert_eq!(wp.recv_buffer_len(), 0);
    }

    #[test]
    fn dequeue_send_fifo() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        let first = wp.dequeue_send();
        assert!(first.is_some());
        assert!(wp.dequeue_send().is_none());
    }

    #[test]
    fn dequeue_recv_fifo() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();

        let frame = wp.dequeue_recv();
        assert!(frame.is_some());
        assert!(matches!(frame.unwrap(), BusFrame::Welcome { .. }));
    }

    #[test]
    fn drain_recv_empties_buffer() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["f.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        let frames = wp.drain_recv();
        assert_eq!(frames.len(), 2);
        assert_eq!(wp.recv_buffer_len(), 0);
    }

    #[test]
    fn needs_keepalive_when_active() {
        let wp = WireProtocol::with_keepalive(PaneId::new("test"), 10);
        // Not active — should not need keepalive
        assert!(!wp.needs_keepalive(100.0));
    }

    #[test]
    fn stats_track_frames() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();

        let stats = wp.stats();
        assert_eq!(stats.frames_sent, 1); // handshake
        assert_eq!(stats.frames_received, 1); // welcome
        assert!(stats.bytes_sent > 0);
        assert!(stats.bytes_received > 0);
    }

    #[test]
    fn stats_track_validation_errors() {
        let wp = make_protocol();
        wp.process_incoming("bad json").unwrap();
        assert_eq!(wp.stats().validation_errors, 1);
    }

    #[test]
    fn stats_track_state_transitions() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        assert_eq!(wp.stats().state_transitions, 2);
    }

    #[test]
    fn protocol_state_display() {
        assert_eq!(ProtocolState::Disconnected.to_string(), "disconnected");
        assert_eq!(ProtocolState::Active.to_string(), "active");
        assert_eq!(ProtocolState::Closing.to_string(), "closing");
    }

    #[test]
    fn protocol_state_can_send() {
        assert!(!ProtocolState::Disconnected.can_send_data());
        assert!(!ProtocolState::Handshaking.can_send_data());
        assert!(!ProtocolState::Connected.can_send_data());
        assert!(ProtocolState::Active.can_send_data());
    }

    #[test]
    fn protocol_state_can_receive() {
        assert!(!ProtocolState::Disconnected.can_receive_events());
        assert!(ProtocolState::Connected.can_receive_events());
        assert!(ProtocolState::Active.can_receive_events());
    }

    #[test]
    fn protocol_state_terminal() {
        assert!(ProtocolState::Disconnected.is_terminal());
        assert!(ProtocolState::Closing.is_terminal());
        assert!(!ProtocolState::Active.is_terminal());
    }

    #[test]
    fn error_frame_doesnt_change_state() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();

        let state_before = wp.state();
        wp.process_incoming(r#"{"type":"Error","code":500,"message":"test"}"#).unwrap();
        assert_eq!(wp.state(), state_before);
    }

    #[test]
    fn full_handshake_flow() {
        let wp = make_protocol();

        // 1. Initiate
        wp.initiate_handshake().unwrap();
        assert_eq!(wp.state(), ProtocolState::Handshaking);

        // 2. Receive Welcome
        wp.process_incoming(r#"{"type":"Welcome","session_id":"orac-sess-1","version":"2.0"}"#).unwrap();
        assert_eq!(wp.state(), ProtocolState::Connected);
        assert_eq!(wp.session_id().as_deref(), Some("orac-sess-1"));

        // 3. Subscribe
        wp.subscribe(vec!["field.*".into(), "task.*".into()]).unwrap();
        assert_eq!(wp.state(), ProtocolState::Subscribing);

        // 4. Receive Subscribed
        wp.process_incoming(r#"{"type":"Subscribed","count":2}"#).unwrap();
        assert_eq!(wp.state(), ProtocolState::Active);

        // 5. Ready for events
        assert!(wp.state().can_send_data());
        assert!(wp.state().can_receive_events());
    }

    #[test]
    fn send_queue_bounded() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["f.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        // Fill the queue to capacity
        let mut errors = 0_usize;
        for _ in 0..MAX_SEND_QUEUE + 10 {
            let task = crate::m2_wire::m08_bus_types::BusTask::new(
                "test".into(),
                crate::m2_wire::m08_bus_types::TaskTarget::AnyIdle,
                PaneId::new("orac"),
            );
            if wp.submit_task(task).is_err() {
                errors += 1;
            }
        }
        assert!(wp.send_queue_len() <= MAX_SEND_QUEUE);
        // Queue was already partially filled (handshake + subscribe), so
        // errors should fire once we exceed capacity.
        assert!(errors > 0, "should reject when queue is full");
    }

    #[test]
    fn enqueue_full_returns_error_not_silent_drop() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["f.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        let sent_before = wp.stats().frames_sent;
        // Fill queue exactly to capacity
        while wp.send_queue_len() < MAX_SEND_QUEUE {
            let task = crate::m2_wire::m08_bus_types::BusTask::new(
                "filler".into(),
                crate::m2_wire::m08_bus_types::TaskTarget::AnyIdle,
                PaneId::new("orac"),
            );
            wp.submit_task(task).ok();
        }
        let sent_at_capacity = wp.stats().frames_sent;

        // Next submit should fail without incrementing frames_sent
        let task = crate::m2_wire::m08_bus_types::BusTask::new(
            "overflow".into(),
            crate::m2_wire::m08_bus_types::TaskTarget::AnyIdle,
            PaneId::new("orac"),
        );
        assert!(wp.submit_task(task).is_err());
        assert_eq!(wp.stats().frames_sent, sent_at_capacity);
        assert!(sent_at_capacity > sent_before);
    }

    #[test]
    fn recv_buffer_bounded() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["f.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        for i in 0..MAX_RECV_BUFFER + 10 {
            let event = format!(
                r#"{{"type":"Event","event":{{"event_type":"field.tick","data":{i},"tick":{i},"timestamp":0.0}}}}"#
            );
            wp.process_incoming(&event).unwrap();
        }
        assert!(wp.recv_buffer_len() <= MAX_RECV_BUFFER);
    }

    #[test]
    fn protocol_version_constant() {
        assert_eq!(PROTOCOL_VERSION, "2.0");
    }

    #[test]
    fn max_frame_size_reasonable() {
        assert_eq!(MAX_FRAME_SIZE, 65_536);
    }

    #[test]
    fn with_keepalive_sets_interval() {
        let wp = WireProtocol::with_keepalive(PaneId::new("test"), 60);
        assert_eq!(wp.keepalive_secs, 60);
    }

    #[test]
    fn send_keepalive_increments_counter() {
        let wp = make_protocol();
        wp.initiate_handshake().unwrap();
        wp.process_incoming(r#"{"type":"Welcome","session_id":"s","version":"2.0"}"#).unwrap();
        wp.subscribe(vec!["f.*".into()]).unwrap();
        wp.process_incoming(r#"{"type":"Subscribed","count":1}"#).unwrap();

        assert_eq!(wp.stats().keepalives_sent, 0);
        wp.send_keepalive().unwrap();
        assert_eq!(wp.stats().keepalives_sent, 1);
        wp.send_keepalive().unwrap();
        assert_eq!(wp.stats().keepalives_sent, 2);
    }

    #[test]
    fn send_keepalive_rejected_before_active() {
        let wp = make_protocol();
        assert!(wp.send_keepalive().is_err());
    }
}
