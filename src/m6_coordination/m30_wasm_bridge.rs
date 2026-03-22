//! # M30: WASM Bridge
//!
//! FIFO/ring protocol bridge between ORAC sidecar and the Zellij
//! swarm-orchestrator WASM plugin. Reads commands from a named FIFO pipe
//! and writes events to a ring-buffered JSONL file.
//!
//! ## Layer: L6 (Coordination)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//!
//! ## Protocol
//!
//! ```text
//! WASM plugin  → /tmp/swarm-commands.pipe (FIFO)   → ORAC reads
//! ORAC         → /tmp/swarm-events.jsonl  (ring)   → WASM plugin reads
//!                (1000 line cap, oldest lines dropped)
//! ```
//!
//! ## Command Format (FIFO → ORAC)
//!
//! One JSON object per line:
//! ```json
//! {"cmd":"dispatch","pane":"fleet-1","task":"run tests"}
//! {"cmd":"status"}
//! {"cmd":"field_state"}
//! ```
//!
//! ## Event Format (ORAC → Ring)
//!
//! One JSON object per line:
//! ```json
//! {"event":"tick","tick":42,"r":0.993,"k":2.1}
//! {"event":"task_completed","task_id":"abc","pane":"fleet-1"}
//! ```

use std::collections::VecDeque;
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default FIFO path for incoming commands from WASM plugin.
pub const DEFAULT_FIFO_PATH: &str = "/tmp/swarm-commands.pipe";

/// Default ring file path for outgoing events to WASM plugin.
pub const DEFAULT_RING_PATH: &str = "/tmp/swarm-events.jsonl";

/// Maximum lines retained in the ring file.
pub const RING_LINE_CAP: usize = 1000;

/// Maximum command length in bytes.
const MAX_COMMAND_LEN: usize = 8192;

/// Maximum event length in bytes.
const MAX_EVENT_LEN: usize = 8192;

// ──────────────────────────────────────────────────────────────
// Command types
// ──────────────────────────────────────────────────────────────

/// A command received from the WASM plugin via FIFO.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd")]
pub enum WasmCommand {
    /// Dispatch a task to a pane.
    #[serde(rename = "dispatch")]
    Dispatch {
        /// Target pane identifier.
        pane: String,
        /// Task description.
        task: String,
    },
    /// Request current fleet status.
    #[serde(rename = "status")]
    Status,
    /// Request current field state (r, K, phases).
    #[serde(rename = "field_state")]
    FieldState,
    /// Request pane list.
    #[serde(rename = "list_panes")]
    ListPanes,
    /// Ping (keepalive).
    #[serde(rename = "ping")]
    Ping,
}

impl fmt::Display for WasmCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dispatch { pane, task } => write!(f, "dispatch({pane}, {task})"),
            Self::Status => f.write_str("status"),
            Self::FieldState => f.write_str("field_state"),
            Self::ListPanes => f.write_str("list_panes"),
            Self::Ping => f.write_str("ping"),
        }
    }
}

/// An event written to the ring file for the WASM plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEvent {
    /// Event type tag.
    pub event: String,
    /// Tick number.
    pub tick: u64,
    /// Event payload.
    pub data: serde_json::Value,
}

impl WasmEvent {
    /// Create a new WASM event.
    #[must_use]
    pub fn new(event: &str, tick: u64, data: serde_json::Value) -> Self {
        Self {
            event: event.to_owned(),
            tick,
            data,
        }
    }

    /// Create a simple tick event with r and K values.
    #[must_use]
    pub fn tick_event(tick: u64, r: f64, k: f64) -> Self {
        Self::new(
            "tick",
            tick,
            serde_json::json!({"r": r, "k": k}),
        )
    }

    /// Create a task completion event.
    #[must_use]
    pub fn task_completed(tick: u64, task_id: &str, pane: &str) -> Self {
        Self::new(
            "task_completed",
            tick,
            serde_json::json!({"task_id": task_id, "pane": pane}),
        )
    }

    /// Create a pong (keepalive response) event.
    #[must_use]
    pub fn pong(tick: u64) -> Self {
        Self::new("pong", tick, serde_json::Value::Null)
    }

    /// Serialize to a single JSONL line.
    ///
    /// # Errors
    /// Returns [`PvError::Json`] if serialization fails.
    pub fn to_jsonl(&self) -> PvResult<String> {
        serde_json::to_string(self).map_err(PvError::Json)
    }
}

// ──────────────────────────────────────────────────────────────
// Ring buffer
// ──────────────────────────────────────────────────────────────

/// In-memory ring buffer for outbound events (mirroring the on-disk ring file).
///
/// Maintains a bounded `VecDeque` of serialized JSONL lines. When the cap
/// is reached, the oldest line is dropped (FIFO eviction).
#[derive(Debug)]
pub struct EventRingBuffer {
    /// Buffered JSONL lines.
    lines: VecDeque<String>,
    /// Maximum lines.
    cap: usize,
    /// Total events written (monotonically increasing).
    total_written: u64,
}

impl EventRingBuffer {
    /// Create a new ring buffer with the given capacity.
    #[must_use]
    pub fn new(cap: usize) -> Self {
        Self {
            lines: VecDeque::with_capacity(cap.min(RING_LINE_CAP)),
            cap,
            total_written: 0,
        }
    }

    /// Write an event to the ring buffer.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if the serialized event exceeds `MAX_EVENT_LEN`.
    pub fn write_event(&mut self, event: &WasmEvent) -> PvResult<()> {
        let line = event.to_jsonl()?;
        if line.len() > MAX_EVENT_LEN {
            return Err(PvError::ConfigValidation(
                format!("event too large: {} bytes > {MAX_EVENT_LEN}", line.len()),
            ));
        }
        if self.lines.len() >= self.cap {
            self.lines.pop_front();
        }
        self.lines.push_back(line);
        self.total_written += 1;
        Ok(())
    }

    /// Get all current lines (for flushing to disk).
    #[must_use]
    pub fn lines(&self) -> &VecDeque<String> {
        &self.lines
    }

    /// Number of lines currently buffered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Total events written since creation.
    #[must_use]
    pub const fn total_written(&self) -> u64 {
        self.total_written
    }

    /// Clear all buffered lines.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// Render all lines as a single string (for writing to disk).
    #[must_use]
    pub fn to_file_content(&self) -> String {
        let mut out = String::with_capacity(self.lines.len() * 100);
        for line in &self.lines {
            out.push_str(line);
            out.push('\n');
        }
        out
    }
}

// ──────────────────────────────────────────────────────────────
// Command parser
// ──────────────────────────────────────────────────────────────

/// Parse a raw FIFO line into a `WasmCommand`.
///
/// # Errors
/// Returns [`PvError::BusProtocol`] if the line is malformed or too long.
pub fn parse_command(line: &str) -> PvResult<WasmCommand> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err(PvError::BusProtocol("empty command".into()));
    }
    if trimmed.len() > MAX_COMMAND_LEN {
        return Err(PvError::BusProtocol(
            format!("command too large: {} bytes > {MAX_COMMAND_LEN}", trimmed.len()),
        ));
    }
    serde_json::from_str(trimmed).map_err(|e| PvError::BusProtocol(
        format!("invalid command JSON: {e}"),
    ))
}

// ──────────────────────────────────────────────────────────────
// WASM Bridge
// ──────────────────────────────────────────────────────────────

/// Bridge state and statistics.
#[derive(Debug, Clone, Default)]
pub struct WasmBridgeStats {
    /// Total commands received.
    pub commands_received: u64,
    /// Total events written.
    pub events_written: u64,
    /// Total parse errors.
    pub parse_errors: u64,
    /// Total oversized events rejected.
    pub oversized_rejected: u64,
}

/// WASM bridge managing FIFO/ring communication with the Zellij plugin.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct WasmBridge {
    /// FIFO path for incoming commands.
    fifo_path: String,
    /// Ring file path for outgoing events.
    ring_path: String,
    /// Outbound event ring buffer.
    ring: RwLock<EventRingBuffer>,
    /// Inbound command queue (parsed, pending processing).
    command_queue: RwLock<VecDeque<WasmCommand>>,
    /// Statistics.
    stats: RwLock<WasmBridgeStats>,
}

impl fmt::Debug for WasmBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WasmBridge")
            .field("fifo_path", &self.fifo_path)
            .field("ring_path", &self.ring_path)
            .field("ring_len", &self.ring.read().len())
            .field("command_queue_len", &self.command_queue.read().len())
            .finish_non_exhaustive()
    }
}

impl WasmBridge {
    /// Create a new WASM bridge with default paths.
    #[must_use]
    pub fn new() -> Self {
        Self::with_paths(DEFAULT_FIFO_PATH, DEFAULT_RING_PATH)
    }

    /// Create a new WASM bridge with custom paths.
    #[must_use]
    pub fn with_paths(fifo_path: &str, ring_path: &str) -> Self {
        Self {
            fifo_path: fifo_path.to_owned(),
            ring_path: ring_path.to_owned(),
            ring: RwLock::new(EventRingBuffer::new(RING_LINE_CAP)),
            command_queue: RwLock::new(VecDeque::with_capacity(64)),
            stats: RwLock::new(WasmBridgeStats::default()),
        }
    }

    /// Ingest a raw FIFO line, parse it, and queue the command.
    ///
    /// # Errors
    /// Returns [`PvError::BusProtocol`] if the line is malformed.
    pub fn ingest_command(&self, line: &str) -> PvResult<WasmCommand> {
        match parse_command(line) {
            Ok(cmd) => {
                self.command_queue.write().push_back(cmd.clone());
                self.stats.write().commands_received += 1;
                Ok(cmd)
            }
            Err(e) => {
                self.stats.write().parse_errors += 1;
                Err(e)
            }
        }
    }

    /// Dequeue the next pending command.
    #[must_use]
    pub fn next_command(&self) -> Option<WasmCommand> {
        self.command_queue.write().pop_front()
    }

    /// Write an event to the ring buffer.
    ///
    /// # Errors
    /// Returns [`PvError`] if the event is too large or serialization fails.
    pub fn write_event(&self, event: &WasmEvent) -> PvResult<()> {
        match self.ring.write().write_event(event) {
            Ok(()) => {
                self.stats.write().events_written += 1;
                Ok(())
            }
            Err(e) => {
                self.stats.write().oversized_rejected += 1;
                Err(e)
            }
        }
    }

    /// Get the ring buffer content for flushing to disk.
    #[must_use]
    pub fn ring_content(&self) -> String {
        self.ring.read().to_file_content()
    }

    /// Number of events in the ring buffer.
    #[must_use]
    pub fn ring_len(&self) -> usize {
        self.ring.read().len()
    }

    /// Number of pending commands.
    #[must_use]
    pub fn command_queue_len(&self) -> usize {
        self.command_queue.read().len()
    }

    /// FIFO path.
    #[must_use]
    pub fn fifo_path(&self) -> &str {
        &self.fifo_path
    }

    /// Ring file path.
    #[must_use]
    pub fn ring_path(&self) -> &str {
        &self.ring_path
    }

    /// Statistics.
    #[must_use]
    pub fn stats(&self) -> WasmBridgeStats {
        self.stats.read().clone()
    }

    /// Clear all state.
    pub fn reset(&self) {
        self.ring.write().clear();
        self.command_queue.write().clear();
        *self.stats.write() = WasmBridgeStats::default();
    }
}

impl Default for WasmBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Command parsing ──

    #[test]
    fn parse_dispatch_command() {
        let cmd = parse_command(r#"{"cmd":"dispatch","pane":"fleet-1","task":"run tests"}"#).unwrap();
        assert!(matches!(cmd, WasmCommand::Dispatch { .. }));
    }

    #[test]
    fn parse_status_command() {
        let cmd = parse_command(r#"{"cmd":"status"}"#).unwrap();
        assert!(matches!(cmd, WasmCommand::Status));
    }

    #[test]
    fn parse_field_state_command() {
        let cmd = parse_command(r#"{"cmd":"field_state"}"#).unwrap();
        assert!(matches!(cmd, WasmCommand::FieldState));
    }

    #[test]
    fn parse_list_panes_command() {
        let cmd = parse_command(r#"{"cmd":"list_panes"}"#).unwrap();
        assert!(matches!(cmd, WasmCommand::ListPanes));
    }

    #[test]
    fn parse_ping_command() {
        let cmd = parse_command(r#"{"cmd":"ping"}"#).unwrap();
        assert!(matches!(cmd, WasmCommand::Ping));
    }

    #[test]
    fn parse_empty_rejected() {
        assert!(parse_command("").is_err());
        assert!(parse_command("   ").is_err());
    }

    #[test]
    fn parse_invalid_json_rejected() {
        assert!(parse_command("not json").is_err());
    }

    #[test]
    fn parse_oversized_rejected() {
        let huge = format!(r#"{{"cmd":"status","pad":"{}"}}"#, "x".repeat(MAX_COMMAND_LEN));
        assert!(parse_command(&huge).is_err());
    }

    #[test]
    fn parse_unknown_command_rejected() {
        assert!(parse_command(r#"{"cmd":"destroy_everything"}"#).is_err());
    }

    #[test]
    fn command_display() {
        assert_eq!(WasmCommand::Status.to_string(), "status");
        assert_eq!(WasmCommand::Ping.to_string(), "ping");
        let d = WasmCommand::Dispatch {
            pane: "p1".into(),
            task: "t1".into(),
        };
        assert!(d.to_string().contains("p1"));
    }

    // ── Event creation ──

    #[test]
    fn tick_event_serializes() {
        let event = WasmEvent::tick_event(42, 0.993, 2.1);
        let json = event.to_jsonl().unwrap();
        assert!(json.contains("\"tick\""));
        assert!(json.contains("0.993"));
    }

    #[test]
    fn task_completed_event() {
        let event = WasmEvent::task_completed(10, "task-1", "fleet-1");
        let json = event.to_jsonl().unwrap();
        assert!(json.contains("task_completed"));
        assert!(json.contains("task-1"));
    }

    #[test]
    fn pong_event() {
        let event = WasmEvent::pong(5);
        let json = event.to_jsonl().unwrap();
        assert!(json.contains("pong"));
    }

    #[test]
    fn event_new_generic() {
        let event = WasmEvent::new("custom", 99, serde_json::json!({"key": "val"}));
        assert_eq!(event.event, "custom");
        assert_eq!(event.tick, 99);
    }

    // ── Ring buffer ──

    #[test]
    fn ring_buffer_basic() {
        let mut ring = EventRingBuffer::new(10);
        let event = WasmEvent::tick_event(1, 0.5, 1.0);
        ring.write_event(&event).unwrap();
        assert_eq!(ring.len(), 1);
        assert!(!ring.is_empty());
        assert_eq!(ring.total_written(), 1);
    }

    #[test]
    fn ring_buffer_bounded() {
        let mut ring = EventRingBuffer::new(3);
        for i in 0..5 {
            ring.write_event(&WasmEvent::tick_event(i, 0.5, 1.0)).unwrap();
        }
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.total_written(), 5);
    }

    #[test]
    fn ring_buffer_fifo_eviction() {
        let mut ring = EventRingBuffer::new(2);
        ring.write_event(&WasmEvent::tick_event(1, 0.1, 1.0)).unwrap();
        ring.write_event(&WasmEvent::tick_event(2, 0.2, 1.0)).unwrap();
        ring.write_event(&WasmEvent::tick_event(3, 0.3, 1.0)).unwrap();

        // Oldest (tick 1) should be evicted
        let content = ring.to_file_content();
        assert!(!content.contains("\"tick\":1"));
        assert!(content.contains("\"tick\":3"));
    }

    #[test]
    fn ring_buffer_clear() {
        let mut ring = EventRingBuffer::new(10);
        ring.write_event(&WasmEvent::tick_event(1, 0.5, 1.0)).unwrap();
        ring.clear();
        assert!(ring.is_empty());
    }

    #[test]
    fn ring_buffer_to_file_content() {
        let mut ring = EventRingBuffer::new(10);
        ring.write_event(&WasmEvent::pong(1)).unwrap();
        ring.write_event(&WasmEvent::pong(2)).unwrap();
        let content = ring.to_file_content();
        let line_count = content.lines().count();
        assert_eq!(line_count, 2);
    }

    #[test]
    fn ring_buffer_lines_accessor() {
        let mut ring = EventRingBuffer::new(10);
        ring.write_event(&WasmEvent::pong(1)).unwrap();
        assert_eq!(ring.lines().len(), 1);
    }

    // ── WASM Bridge ──

    #[test]
    fn bridge_default_paths() {
        let bridge = WasmBridge::new();
        assert_eq!(bridge.fifo_path(), DEFAULT_FIFO_PATH);
        assert_eq!(bridge.ring_path(), DEFAULT_RING_PATH);
    }

    #[test]
    fn bridge_custom_paths() {
        let bridge = WasmBridge::with_paths("/tmp/test.pipe", "/tmp/test.jsonl");
        assert_eq!(bridge.fifo_path(), "/tmp/test.pipe");
        assert_eq!(bridge.ring_path(), "/tmp/test.jsonl");
    }

    #[test]
    fn bridge_ingest_command() {
        let bridge = WasmBridge::new();
        let cmd = bridge.ingest_command(r#"{"cmd":"status"}"#).unwrap();
        assert!(matches!(cmd, WasmCommand::Status));
        assert_eq!(bridge.command_queue_len(), 1);
        assert_eq!(bridge.stats().commands_received, 1);
    }

    #[test]
    fn bridge_ingest_invalid() {
        let bridge = WasmBridge::new();
        assert!(bridge.ingest_command("bad").is_err());
        assert_eq!(bridge.stats().parse_errors, 1);
    }

    #[test]
    fn bridge_next_command_fifo() {
        let bridge = WasmBridge::new();
        bridge.ingest_command(r#"{"cmd":"status"}"#).unwrap();
        bridge.ingest_command(r#"{"cmd":"ping"}"#).unwrap();

        let first = bridge.next_command().unwrap();
        assert!(matches!(first, WasmCommand::Status));

        let second = bridge.next_command().unwrap();
        assert!(matches!(second, WasmCommand::Ping));

        assert!(bridge.next_command().is_none());
    }

    #[test]
    fn bridge_write_event() {
        let bridge = WasmBridge::new();
        let event = WasmEvent::tick_event(1, 0.99, 2.0);
        bridge.write_event(&event).unwrap();
        assert_eq!(bridge.ring_len(), 1);
        assert_eq!(bridge.stats().events_written, 1);
    }

    #[test]
    fn bridge_ring_content() {
        let bridge = WasmBridge::new();
        bridge.write_event(&WasmEvent::pong(1)).unwrap();
        let content = bridge.ring_content();
        assert!(content.contains("pong"));
    }

    #[test]
    fn bridge_ring_bounded() {
        let bridge = WasmBridge::new();
        for i in 0..RING_LINE_CAP + 50 {
            bridge
                .write_event(&WasmEvent::tick_event(i as u64, 0.5, 1.0))
                .unwrap();
        }
        assert_eq!(bridge.ring_len(), RING_LINE_CAP);
    }

    #[test]
    fn bridge_reset() {
        let bridge = WasmBridge::new();
        bridge.ingest_command(r#"{"cmd":"ping"}"#).unwrap();
        bridge.write_event(&WasmEvent::pong(1)).unwrap();

        bridge.reset();
        assert_eq!(bridge.ring_len(), 0);
        assert_eq!(bridge.command_queue_len(), 0);
        assert_eq!(bridge.stats().commands_received, 0);
    }

    #[test]
    fn bridge_default_impl() {
        let bridge = WasmBridge::default();
        assert_eq!(bridge.fifo_path(), DEFAULT_FIFO_PATH);
    }

    #[test]
    fn bridge_debug_output() {
        let bridge = WasmBridge::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("WasmBridge"));
        assert!(debug.contains("swarm-commands"));
    }

    #[test]
    fn bridge_multiple_commands_and_events() {
        let bridge = WasmBridge::new();

        bridge.ingest_command(r#"{"cmd":"status"}"#).unwrap();
        bridge.ingest_command(r#"{"cmd":"field_state"}"#).unwrap();
        bridge.ingest_command(r#"{"cmd":"ping"}"#).unwrap();

        bridge.write_event(&WasmEvent::pong(1)).unwrap();
        bridge.write_event(&WasmEvent::tick_event(2, 0.99, 1.5)).unwrap();

        let stats = bridge.stats();
        assert_eq!(stats.commands_received, 3);
        assert_eq!(stats.events_written, 2);
        assert_eq!(stats.parse_errors, 0);
    }

    #[test]
    fn dispatch_command_fields() {
        let cmd = parse_command(r#"{"cmd":"dispatch","pane":"fleet-2","task":"cargo test"}"#).unwrap();
        if let WasmCommand::Dispatch { pane, task } = cmd {
            assert_eq!(pane, "fleet-2");
            assert_eq!(task, "cargo test");
        } else {
            panic!("expected Dispatch");
        }
    }

    #[test]
    fn constants_correct() {
        assert_eq!(DEFAULT_FIFO_PATH, "/tmp/swarm-commands.pipe");
        assert_eq!(DEFAULT_RING_PATH, "/tmp/swarm-events.jsonl");
        assert_eq!(RING_LINE_CAP, 1000);
    }
}
