//! # M30: Bus Types
//!
//! `BusFrame` (11 kinds), `BusTask`, `TaskTarget` (`specific|any_idle|field_driven|willing`),
//! `BusEvent` (typed events across namespaces). `Serde` internally-tagged enums (S01).
//!
//! ## Layer: L7 | Module: M30 | Dependencies: L1 (M01)
//! ## Schema: [`bus_frame.schema.json`](../../.claude/schemas/bus_frame.schema.json)

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::{now_secs, PaneId, TaskId};

// ──────────────────────────────────────────────────────────────
// Task target
// ──────────────────────────────────────────────────────────────

/// Target selection strategy for a bus task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskTarget {
    /// Route to a specific sphere by ID.
    Specific {
        /// The target sphere's ID.
        pane_id: PaneId,
    },
    /// Route to any idle sphere.
    #[default]
    AnyIdle,
    /// Route using field-driven heuristics (chimera routing).
    FieldDriven,
    /// Route to any sphere that declares willingness.
    Willing,
}

impl fmt::Display for TaskTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Specific { pane_id } => write!(f, "Specific({pane_id})"),
            Self::AnyIdle => write!(f, "AnyIdle"),
            Self::FieldDriven => write!(f, "FieldDriven"),
            Self::Willing => write!(f, "Willing"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Task status
// ──────────────────────────────────────────────────────────────

/// Lifecycle status of a bus task.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task has been submitted but not yet claimed.
    #[default]
    Pending,
    /// A sphere has claimed the task.
    Claimed,
    /// The task has been completed successfully.
    Completed,
    /// The task failed during execution.
    Failed,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Claimed => write!(f, "Claimed"),
            Self::Completed => write!(f, "Completed"),
            Self::Failed => write!(f, "Failed"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Bus task
// ──────────────────────────────────────────────────────────────

/// A task submitted to the IPC bus for fleet coordination.
///
/// Tasks flow through the lifecycle: [`Pending`] -> [`Claimed`] -> [`Completed`] | [`Failed`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusTask {
    /// Unique task identifier.
    pub id: TaskId,
    /// Human-readable description of the work to be done.
    pub description: String,
    /// Target selection strategy.
    pub target: TaskTarget,
    /// Current lifecycle status.
    pub status: TaskStatus,
    /// ID of the sphere that submitted this task.
    pub submitted_by: PaneId,
    /// ID of the sphere that claimed this task (if any).
    pub claimed_by: Option<PaneId>,
    /// Unix timestamp when the task was submitted.
    pub submitted_at: f64,
    /// Unix timestamp when the task was claimed (if any).
    pub claimed_at: Option<f64>,
    /// Unix timestamp when the task was completed (if any).
    pub completed_at: Option<f64>,
}

impl BusTask {
    /// Create a new pending task.
    #[must_use]
    pub fn new(description: String, target: TaskTarget, submitted_by: PaneId) -> Self {
        Self {
            id: TaskId::new(),
            description,
            target,
            status: TaskStatus::Pending,
            submitted_by,
            claimed_by: None,
            submitted_at: now_secs(),
            claimed_at: None,
            completed_at: None,
        }
    }

    /// Whether the task is still pending (unclaimed).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        matches!(self.status, TaskStatus::Pending)
    }

    /// Whether the task has reached a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self.status, TaskStatus::Completed | TaskStatus::Failed)
    }

    /// Claim this task for a sphere. Returns `false` if already claimed.
    pub fn claim(&mut self, claimer: PaneId) -> bool {
        if self.status != TaskStatus::Pending {
            return false;
        }
        self.status = TaskStatus::Claimed;
        self.claimed_by = Some(claimer);
        self.claimed_at = Some(now_secs());
        true
    }

    /// Requeue a claimed task back to pending (for stale claim recovery).
    pub fn requeue(&mut self) -> bool {
        if self.status != TaskStatus::Claimed {
            return false;
        }
        self.status = TaskStatus::Pending;
        self.claimed_by = None;
        self.claimed_at = None;
        true
    }

    /// Mark this task as completed.
    pub fn complete(&mut self) -> bool {
        if self.status != TaskStatus::Claimed {
            return false;
        }
        self.status = TaskStatus::Completed;
        self.completed_at = Some(now_secs());
        true
    }

    /// Mark this task as failed.
    pub fn fail(&mut self) -> bool {
        if self.status != TaskStatus::Claimed {
            return false;
        }
        self.status = TaskStatus::Failed;
        self.completed_at = Some(now_secs());
        true
    }

    /// Elapsed time since submission (seconds).
    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        now_secs() - self.submitted_at
    }
}

// ──────────────────────────────────────────────────────────────
// Bus event
// ──────────────────────────────────────────────────────────────

/// A typed event published on the bus.
///
/// Events are fire-and-forget notifications that subscribers can filter by `event_type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    /// Event type string (namespace.action, e.g. "field.tick", "sphere.registered").
    pub event_type: String,
    /// Event payload as JSON value.
    pub data: serde_json::Value,
    /// Tick at which the event was published.
    pub tick: u64,
    /// Unix timestamp when the event was created.
    pub timestamp: f64,
}

impl BusEvent {
    /// Create a new bus event.
    #[must_use]
    pub fn new(event_type: String, data: serde_json::Value, tick: u64) -> Self {
        Self {
            event_type,
            data,
            tick,
            timestamp: now_secs(),
        }
    }

    /// Create a simple text event.
    #[must_use]
    pub fn text(event_type: &str, message: &str, tick: u64) -> Self {
        Self::new(
            event_type.to_owned(),
            serde_json::Value::String(message.to_owned()),
            tick,
        )
    }

    /// Whether this event matches a glob pattern (supports `*` wildcard).
    #[must_use]
    pub fn matches_pattern(&self, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if let Some(prefix) = pattern.strip_suffix('*') {
            return self.event_type.starts_with(prefix);
        }
        self.event_type == pattern
    }
}

// ──────────────────────────────────────────────────────────────
// Bus frame (wire protocol)
// ──────────────────────────────────────────────────────────────

/// IPC bus frame — the `NDJSON` wire protocol message type.
///
/// Each variant maps to one line of `NDJSON` on the `Unix domain socket`.
/// Tagged with `"type"` field for `Serde` internally-tagged enum.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BusFrame {
    /// Client → Server: initial handshake with identity.
    Handshake {
        /// Client's sphere ID.
        pane_id: PaneId,
        /// Protocol version (e.g. "2.0").
        version: String,
    },

    /// Server → Client: handshake accepted.
    Welcome {
        /// Assigned session ID.
        session_id: String,
        /// Server's protocol version.
        version: String,
    },

    /// Client → Server: subscribe to event patterns.
    Subscribe {
        /// Glob patterns to subscribe to (e.g. "field.*", "sphere.registered").
        patterns: Vec<String>,
    },

    /// Server → Client: subscription confirmed.
    Subscribed {
        /// Number of active subscriptions for this client.
        count: usize,
    },

    /// Client → Server: submit a task for fleet dispatch.
    Submit {
        /// The task to submit.
        task: BusTask,
    },

    /// Server → Client: task submission acknowledged.
    TaskSubmitted {
        /// ID of the submitted task.
        task_id: TaskId,
    },

    /// Server → Client: event notification (matching a subscription).
    Event {
        /// The event.
        event: BusEvent,
    },

    /// Client/Server: cascade handoff request.
    Cascade {
        /// Source sphere ID.
        source: PaneId,
        /// Target sphere ID.
        target: PaneId,
        /// Handoff brief (markdown).
        brief: String,
    },

    /// Client/Server: cascade acknowledgement.
    CascadeAck {
        /// Source sphere ID.
        source: PaneId,
        /// Target sphere ID.
        target: PaneId,
        /// Whether the cascade was accepted.
        accepted: bool,
    },

    /// Client → Server: graceful disconnect.
    Disconnect {
        /// Reason for disconnecting.
        reason: String,
    },

    /// Server → Client: error notification.
    Error {
        /// Error code.
        code: u16,
        /// Human-readable error message.
        message: String,
    },
}

impl BusFrame {
    /// Serialize this frame to an `NDJSON` line (no trailing newline).
    ///
    /// # Errors
    /// Returns `serde_json::Error` if serialization fails.
    pub fn to_ndjson(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Deserialize a frame from an `NDJSON` line.
    ///
    /// # Errors
    /// Returns `serde_json::Error` if deserialization fails.
    pub fn from_ndjson(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }

    /// Whether this frame is a client-originated message.
    #[must_use]
    pub const fn is_client_frame(&self) -> bool {
        matches!(
            self,
            Self::Handshake { .. }
                | Self::Subscribe { .. }
                | Self::Submit { .. }
                | Self::Disconnect { .. }
        )
    }

    /// Whether this frame is a server-originated message.
    #[must_use]
    pub const fn is_server_frame(&self) -> bool {
        matches!(
            self,
            Self::Welcome { .. }
                | Self::Subscribed { .. }
                | Self::TaskSubmitted { .. }
                | Self::Event { .. }
                | Self::Error { .. }
        )
    }

    /// Frame type name for logging.
    #[must_use]
    pub const fn frame_type(&self) -> &'static str {
        match self {
            Self::Handshake { .. } => "Handshake",
            Self::Welcome { .. } => "Welcome",
            Self::Subscribe { .. } => "Subscribe",
            Self::Subscribed { .. } => "Subscribed",
            Self::Submit { .. } => "Submit",
            Self::TaskSubmitted { .. } => "TaskSubmitted",
            Self::Event { .. } => "Event",
            Self::Cascade { .. } => "Cascade",
            Self::CascadeAck { .. } => "CascadeAck",
            Self::Disconnect { .. } => "Disconnect",
            Self::Error { .. } => "Error",
        }
    }
}

impl fmt::Display for BusFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BusFrame::{}", self.frame_type())
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

    // ── TaskTarget ──

    #[test]
    fn task_target_default_is_any_idle() {
        assert_eq!(TaskTarget::default(), TaskTarget::AnyIdle);
    }

    #[test]
    fn task_target_specific_display() {
        let t = TaskTarget::Specific {
            pane_id: pid("alpha"),
        };
        assert!(format!("{t}").contains("alpha"));
    }

    #[test]
    fn task_target_any_idle_display() {
        assert_eq!(format!("{}", TaskTarget::AnyIdle), "AnyIdle");
    }

    #[test]
    fn task_target_field_driven_display() {
        assert_eq!(format!("{}", TaskTarget::FieldDriven), "FieldDriven");
    }

    #[test]
    fn task_target_willing_display() {
        assert_eq!(format!("{}", TaskTarget::Willing), "Willing");
    }

    #[test]
    fn task_target_serde_specific() {
        let t = TaskTarget::Specific {
            pane_id: pid("test"),
        };
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn task_target_serde_any_idle() {
        let t = TaskTarget::AnyIdle;
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn task_target_serde_field_driven() {
        let t = TaskTarget::FieldDriven;
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    #[test]
    fn task_target_serde_willing() {
        let t = TaskTarget::Willing;
        let json = serde_json::to_string(&t).unwrap();
        let back: TaskTarget = serde_json::from_str(&json).unwrap();
        assert_eq!(t, back);
    }

    // ── TaskStatus ──

    #[test]
    fn task_status_default_is_pending() {
        assert_eq!(TaskStatus::default(), TaskStatus::Pending);
    }

    #[test]
    fn task_status_display() {
        assert_eq!(format!("{}", TaskStatus::Pending), "Pending");
        assert_eq!(format!("{}", TaskStatus::Claimed), "Claimed");
        assert_eq!(format!("{}", TaskStatus::Completed), "Completed");
        assert_eq!(format!("{}", TaskStatus::Failed), "Failed");
    }

    #[test]
    fn task_status_serde_roundtrip() {
        for status in &[
            TaskStatus::Pending,
            TaskStatus::Claimed,
            TaskStatus::Completed,
            TaskStatus::Failed,
        ] {
            let json = serde_json::to_string(status).unwrap();
            let back: TaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*status, back);
        }
    }

    // ── BusTask ──

    #[test]
    fn bus_task_new_is_pending() {
        let task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("alpha"));
        assert!(task.is_pending());
        assert!(!task.is_terminal());
    }

    #[test]
    fn bus_task_new_has_unique_id() {
        let t1 = BusTask::new("a".into(), TaskTarget::AnyIdle, pid("x"));
        let t2 = BusTask::new("b".into(), TaskTarget::AnyIdle, pid("x"));
        assert_ne!(t1.id.as_str(), t2.id.as_str());
    }

    #[test]
    fn bus_task_new_no_claimer() {
        let task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(task.claimed_by.is_none());
        assert!(task.completed_at.is_none());
    }

    #[test]
    fn bus_task_new_submitted_at_recent() {
        let task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(task.submitted_at > 1_700_000_000.0);
    }

    #[test]
    fn bus_task_claim_success() {
        let mut task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(task.claim(pid("claimer")));
        assert_eq!(task.status, TaskStatus::Claimed);
        assert_eq!(task.claimed_by.as_ref().map(PaneId::as_str), Some("claimer"));
    }

    #[test]
    fn bus_task_claim_already_claimed_fails() {
        let mut task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(task.claim(pid("a")));
        assert!(!task.claim(pid("b")));
    }

    #[test]
    fn bus_task_complete_success() {
        let mut task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        task.claim(pid("a"));
        assert!(task.complete());
        assert_eq!(task.status, TaskStatus::Completed);
        assert!(task.completed_at.is_some());
        assert!(task.is_terminal());
    }

    #[test]
    fn bus_task_complete_pending_fails() {
        let mut task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(!task.complete());
    }

    #[test]
    fn bus_task_fail_success() {
        let mut task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        task.claim(pid("a"));
        assert!(task.fail());
        assert_eq!(task.status, TaskStatus::Failed);
        assert!(task.is_terminal());
    }

    #[test]
    fn bus_task_fail_pending_fails() {
        let mut task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(!task.fail());
    }

    #[test]
    fn bus_task_elapsed_positive() {
        let task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(task.elapsed_secs() >= 0.0);
    }

    #[test]
    fn bus_task_serde_roundtrip() {
        let task = BusTask::new("do stuff".into(), TaskTarget::FieldDriven, pid("alpha"));
        let json = serde_json::to_string(&task).unwrap();
        let back: BusTask = serde_json::from_str(&json).unwrap();
        assert_eq!(back.description, "do stuff");
        assert_eq!(back.submitted_by.as_str(), "alpha");
    }

    #[test]
    fn bus_task_lifecycle_complete() {
        let mut task = BusTask::new("work".into(), TaskTarget::AnyIdle, pid("x"));
        assert!(task.is_pending());
        task.claim(pid("worker"));
        assert!(!task.is_pending());
        assert!(!task.is_terminal());
        task.complete();
        assert!(task.is_terminal());
    }

    #[test]
    fn bus_task_lifecycle_fail() {
        let mut task = BusTask::new("work".into(), TaskTarget::AnyIdle, pid("x"));
        task.claim(pid("worker"));
        task.fail();
        assert!(task.is_terminal());
        assert_eq!(task.status, TaskStatus::Failed);
    }

    // ── BusEvent ──

    #[test]
    fn bus_event_new() {
        let ev = BusEvent::new("field.tick".into(), serde_json::json!({"r": 0.95}), 42);
        assert_eq!(ev.event_type, "field.tick");
        assert_eq!(ev.tick, 42);
        assert!(ev.timestamp > 1_700_000_000.0);
    }

    #[test]
    fn bus_event_text() {
        let ev = BusEvent::text("test.event", "hello", 10);
        assert_eq!(ev.event_type, "test.event");
        assert_eq!(ev.data, serde_json::Value::String("hello".into()));
    }

    #[test]
    fn bus_event_matches_wildcard() {
        let ev = BusEvent::text("field.tick", "x", 0);
        assert!(ev.matches_pattern("*"));
    }

    #[test]
    fn bus_event_matches_prefix() {
        let ev = BusEvent::text("field.tick", "x", 0);
        assert!(ev.matches_pattern("field.*"));
    }

    #[test]
    fn bus_event_matches_exact() {
        let ev = BusEvent::text("field.tick", "x", 0);
        assert!(ev.matches_pattern("field.tick"));
    }

    #[test]
    fn bus_event_no_match() {
        let ev = BusEvent::text("field.tick", "x", 0);
        assert!(!ev.matches_pattern("sphere.*"));
    }

    #[test]
    fn bus_event_no_match_partial() {
        let ev = BusEvent::text("field.tick", "x", 0);
        assert!(!ev.matches_pattern("field.tock"));
    }

    #[test]
    fn bus_event_serde_roundtrip() {
        let ev = BusEvent::new("sphere.registered".into(), serde_json::json!({"id": "x"}), 5);
        let json = serde_json::to_string(&ev).unwrap();
        let back: BusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_type, "sphere.registered");
        assert_eq!(back.tick, 5);
    }

    // ── BusFrame ──

    #[test]
    fn bus_frame_handshake() {
        let frame = BusFrame::Handshake {
            pane_id: pid("alpha"),
            version: "2.0".into(),
        };
        assert!(frame.is_client_frame());
        assert!(!frame.is_server_frame());
        assert_eq!(frame.frame_type(), "Handshake");
    }

    #[test]
    fn bus_frame_welcome() {
        let frame = BusFrame::Welcome {
            session_id: "sess-1".into(),
            version: "2.0".into(),
        };
        assert!(frame.is_server_frame());
        assert!(!frame.is_client_frame());
        assert_eq!(frame.frame_type(), "Welcome");
    }

    #[test]
    fn bus_frame_subscribe() {
        let frame = BusFrame::Subscribe {
            patterns: vec!["field.*".into()],
        };
        assert!(frame.is_client_frame());
        assert_eq!(frame.frame_type(), "Subscribe");
    }

    #[test]
    fn bus_frame_subscribed() {
        let frame = BusFrame::Subscribed { count: 3 };
        assert!(frame.is_server_frame());
        assert_eq!(frame.frame_type(), "Subscribed");
    }

    #[test]
    fn bus_frame_submit() {
        let task = BusTask::new("test".into(), TaskTarget::AnyIdle, pid("x"));
        let frame = BusFrame::Submit { task };
        assert!(frame.is_client_frame());
        assert_eq!(frame.frame_type(), "Submit");
    }

    #[test]
    fn bus_frame_task_submitted() {
        let frame = BusFrame::TaskSubmitted {
            task_id: TaskId::new(),
        };
        assert!(frame.is_server_frame());
        assert_eq!(frame.frame_type(), "TaskSubmitted");
    }

    #[test]
    fn bus_frame_event() {
        let ev = BusEvent::text("test", "data", 0);
        let frame = BusFrame::Event { event: ev };
        assert!(frame.is_server_frame());
        assert_eq!(frame.frame_type(), "Event");
    }

    #[test]
    fn bus_frame_cascade() {
        let frame = BusFrame::Cascade {
            source: pid("a"),
            target: pid("b"),
            brief: "handoff".into(),
        };
        // Cascade is bidirectional
        assert!(!frame.is_client_frame());
        assert!(!frame.is_server_frame());
        assert_eq!(frame.frame_type(), "Cascade");
    }

    #[test]
    fn bus_frame_cascade_ack() {
        let frame = BusFrame::CascadeAck {
            source: pid("a"),
            target: pid("b"),
            accepted: true,
        };
        assert_eq!(frame.frame_type(), "CascadeAck");
    }

    #[test]
    fn bus_frame_disconnect() {
        let frame = BusFrame::Disconnect {
            reason: "done".into(),
        };
        assert!(frame.is_client_frame());
        assert_eq!(frame.frame_type(), "Disconnect");
    }

    #[test]
    fn bus_frame_error() {
        let frame = BusFrame::Error {
            code: 1400,
            message: "socket error".into(),
        };
        assert!(frame.is_server_frame());
        assert_eq!(frame.frame_type(), "Error");
    }

    #[test]
    fn bus_frame_display() {
        let frame = BusFrame::Handshake {
            pane_id: pid("x"),
            version: "2.0".into(),
        };
        assert_eq!(format!("{frame}"), "BusFrame::Handshake");
    }

    // ── NDJSON round-trips ──

    #[test]
    fn ndjson_roundtrip_handshake() {
        let frame = BusFrame::Handshake {
            pane_id: pid("alpha"),
            version: "2.0".into(),
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        assert_eq!(back.frame_type(), "Handshake");
    }

    #[test]
    fn ndjson_roundtrip_welcome() {
        let frame = BusFrame::Welcome {
            session_id: "sess-42".into(),
            version: "2.0".into(),
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        assert_eq!(back.frame_type(), "Welcome");
    }

    #[test]
    fn ndjson_roundtrip_subscribe() {
        let frame = BusFrame::Subscribe {
            patterns: vec!["field.*".into(), "sphere.*".into()],
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::Subscribe { patterns } = back {
            assert_eq!(patterns.len(), 2);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_roundtrip_submit() {
        let task = BusTask::new("do work".into(), TaskTarget::FieldDriven, pid("sub"));
        let frame = BusFrame::Submit { task };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::Submit { task } = back {
            assert_eq!(task.description, "do work");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_roundtrip_event() {
        let ev = BusEvent::new("test.ev".into(), serde_json::json!(42), 10);
        let frame = BusFrame::Event { event: ev };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::Event { event } = back {
            assert_eq!(event.event_type, "test.ev");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_roundtrip_cascade() {
        let frame = BusFrame::Cascade {
            source: pid("a"),
            target: pid("b"),
            brief: "handoff context here".into(),
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::Cascade { source, target, brief } = back {
            assert_eq!(source.as_str(), "a");
            assert_eq!(target.as_str(), "b");
            assert_eq!(brief, "handoff context here");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_roundtrip_cascade_ack() {
        let frame = BusFrame::CascadeAck {
            source: pid("a"),
            target: pid("b"),
            accepted: false,
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::CascadeAck { accepted, .. } = back {
            assert!(!accepted);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_roundtrip_disconnect() {
        let frame = BusFrame::Disconnect {
            reason: "session ended".into(),
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::Disconnect { reason } = back {
            assert_eq!(reason, "session ended");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_roundtrip_error() {
        let frame = BusFrame::Error {
            code: 1401,
            message: "protocol violation".into(),
        };
        let line = frame.to_ndjson().unwrap();
        let back = BusFrame::from_ndjson(&line).unwrap();
        if let BusFrame::Error { code, message } = back {
            assert_eq!(code, 1401);
            assert_eq!(message, "protocol violation");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn ndjson_invalid_json_fails() {
        let result = BusFrame::from_ndjson("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn ndjson_empty_fails() {
        let result = BusFrame::from_ndjson("");
        assert!(result.is_err());
    }

    // ── Additional coverage ──

    #[test]
    fn bus_task_specific_target() {
        let task = BusTask::new(
            "targeted work".into(),
            TaskTarget::Specific { pane_id: pid("target-1") },
            pid("submitter"),
        );
        if let TaskTarget::Specific { pane_id } = &task.target {
            assert_eq!(pane_id.as_str(), "target-1");
        } else {
            panic!("wrong target variant");
        }
    }

    #[test]
    fn bus_task_willing_target() {
        let task = BusTask::new("open work".into(), TaskTarget::Willing, pid("sub"));
        assert_eq!(task.target, TaskTarget::Willing);
    }

    #[test]
    fn bus_event_empty_type() {
        let ev = BusEvent::text("", "data", 0);
        assert_eq!(ev.event_type, "");
        assert!(ev.matches_pattern("*"));
    }

    #[test]
    fn bus_event_matches_exact_no_wildcard() {
        let ev = BusEvent::text("a.b.c", "x", 0);
        assert!(ev.matches_pattern("a.b.c"));
        assert!(!ev.matches_pattern("a.b"));
    }

    #[test]
    fn bus_frame_all_11_types_counted() {
        // Verify we have all 11 frame types
        let frames: Vec<BusFrame> = vec![
            BusFrame::Handshake { pane_id: pid("a"), version: "2.0".into() },
            BusFrame::Welcome { session_id: "s".into(), version: "2.0".into() },
            BusFrame::Subscribe { patterns: vec![] },
            BusFrame::Subscribed { count: 0 },
            BusFrame::Submit { task: BusTask::new("t".into(), TaskTarget::AnyIdle, pid("a")) },
            BusFrame::TaskSubmitted { task_id: TaskId::new() },
            BusFrame::Event { event: BusEvent::text("t", "d", 0) },
            BusFrame::Cascade { source: pid("a"), target: pid("b"), brief: "b".into() },
            BusFrame::CascadeAck { source: pid("a"), target: pid("b"), accepted: true },
            BusFrame::Disconnect { reason: "r".into() },
            BusFrame::Error { code: 0, message: "m".into() },
        ];
        assert_eq!(frames.len(), 11);
    }
}
