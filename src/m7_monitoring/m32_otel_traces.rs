//! # M32: OpenTelemetry Trace Export
//!
//! Task lifecycle tracing across panes — span creation, context propagation,
//! and OTLP export. Each hook event and bridge call becomes a span.
//!
//! ## Layer: L7 (Monitoring)
//! ## Module: M32
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//! ## Feature: `monitoring`
//!
//! ## Exported Spans
//!
//! - `orac.hook.{event_type}` — per-hook processing span
//! - `orac.bridge.{service}` — per-bridge poll/post span
//! - `orac.task.{task_id}` — task lifecycle span (claim→dispatch→complete)
//! - `orac.tick` — per-tick field integration span
//!
//! ## Design
//!
//! Uses a lightweight in-process trace store (no external collector required).
//! Traces can be exported via the `/traces` endpoint or OTLP when configured.

use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::now_secs;

use crate::m1_core::m01_core_types::{PaneId, TaskId};
use crate::m1_core::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Maximum retained spans in the ring buffer.
const MAX_SPANS: usize = 10_000;

/// Maximum attributes per span.
const MAX_ATTRIBUTES: usize = 32;

/// Maximum span name length (bytes).
const MAX_SPAN_NAME_LEN: usize = 256;

/// Default export batch size.
const DEFAULT_BATCH_SIZE: usize = 100;

/// Span status: unset (default).
const STATUS_UNSET: u8 = 0;

/// Span status: OK.
const STATUS_OK: u8 = 1;

/// Span status: Error.
const STATUS_ERROR: u8 = 2;

// ──────────────────────────────────────────────────────────────
// Trace ID / Span ID
// ──────────────────────────────────────────────────────────────

/// 128-bit trace identifier (W3C Trace Context compatible).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId([u8; 16]);

impl TraceId {
    /// Generate a new random trace ID.
    #[must_use]
    pub fn new() -> Self {
        let mut bytes = [0u8; 16];
        // Simple PRNG seeded from system time — not cryptographic
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(42, |d| {
                // Use secs + nanos to avoid u128→u64 truncation
                d.as_secs().wrapping_mul(1_000_000_000).wrapping_add(u64::from(d.subsec_nanos()))
            });
        let mut state = seed;
        for chunk in bytes.chunks_exact_mut(8) {
            state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let be = state.to_le_bytes();
            chunk.copy_from_slice(&be);
        }
        Self(bytes)
    }

    /// Create from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Hex-encoded trace ID (32 chars).
    #[must_use]
    pub fn to_hex(&self) -> String {
        use std::fmt::Write;
        self.0.iter().fold(String::with_capacity(self.0.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    /// Whether this is a zero (invalid) trace ID.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }
}

impl Default for TraceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TraceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// 64-bit span identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpanId([u8; 8]);

impl SpanId {
    /// Generate a new random span ID.
    #[must_use]
    pub fn new() -> Self {
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(99, |d| {
                // Use secs + nanos to avoid u128→u64 truncation
                d.as_secs().wrapping_mul(1_000_000_000).wrapping_add(u64::from(d.subsec_nanos()))
            });
        Self(seed.to_le_bytes())
    }

    /// Create from raw bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }

    /// Hex-encoded span ID (16 chars).
    #[must_use]
    pub fn to_hex(&self) -> String {
        use std::fmt::Write;
        self.0.iter().fold(String::with_capacity(self.0.len() * 2), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
    }

    /// Whether this is a zero (invalid) span ID.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|&b| b == 0)
    }
}

impl Default for SpanId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SpanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

// ──────────────────────────────────────────────────────────────
// Span kind
// ──────────────────────────────────────────────────────────────

/// Span kind (mirrors `OTel` `SpanKind`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanKind {
    /// Internal operation.
    #[default]
    Internal,
    /// Incoming request (hook handler).
    Server,
    /// Outgoing request (bridge poll/post).
    Client,
}

// ──────────────────────────────────────────────────────────────
// Span status
// ──────────────────────────────────────────────────────────────

/// Status of a completed span.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpanStatus {
    /// Status not set.
    #[default]
    Unset,
    /// Span completed successfully.
    Ok,
    /// Span completed with an error.
    Error {
        /// Human-readable error message.
        message: String,
    },
}

impl SpanStatus {
    /// Numeric code for serialization.
    #[must_use]
    pub const fn code(&self) -> u8 {
        match self {
            Self::Unset => STATUS_UNSET,
            Self::Ok => STATUS_OK,
            Self::Error { .. } => STATUS_ERROR,
        }
    }

    /// Whether this is an error status.
    #[must_use]
    pub const fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

// ──────────────────────────────────────────────────────────────
// Span attribute
// ──────────────────────────────────────────────────────────────

/// A key-value attribute on a span.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanAttribute {
    /// Attribute key (dot-delimited, e.g. `"orac.pane.id"`).
    pub key: String,
    /// Attribute value.
    pub value: AttributeValue,
}

/// Typed attribute value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttributeValue {
    /// String value.
    Str(String),
    /// Integer value.
    Int(i64),
    /// Floating-point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
}

impl std::fmt::Display for AttributeValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Str(s) => f.write_str(s),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(v) => write!(f, "{v:.6}"),
            Self::Bool(b) => write!(f, "{b}"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Span event
// ──────────────────────────────────────────────────────────────

/// A timestamped event within a span (exception, log, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    /// Event name.
    pub name: String,
    /// Unix timestamp (seconds with fractional).
    pub timestamp_secs: f64,
    /// Event attributes.
    pub attributes: Vec<SpanAttribute>,
}

// ──────────────────────────────────────────────────────────────
// Span
// ──────────────────────────────────────────────────────────────

/// A completed trace span.
///
/// Immutable once finished — stored in the ring buffer for export.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Trace this span belongs to.
    pub trace_id: TraceId,
    /// Unique span identifier.
    pub span_id: SpanId,
    /// Parent span (if any).
    pub parent_span_id: Option<SpanId>,
    /// Span name (e.g. `"orac.hook.PostToolUse"`).
    pub name: String,
    /// Span kind.
    pub kind: SpanKind,
    /// Start time (Unix seconds with fractional).
    pub start_secs: f64,
    /// End time (Unix seconds with fractional).
    pub end_secs: f64,
    /// Completion status.
    pub status: SpanStatus,
    /// Key-value attributes.
    pub attributes: Vec<SpanAttribute>,
    /// Timestamped events.
    pub events: Vec<SpanEvent>,
}

impl Span {
    /// Duration of this span.
    #[must_use]
    pub fn duration(&self) -> Duration {
        let d = (self.end_secs - self.start_secs).max(0.0);
        Duration::from_secs_f64(d)
    }

    /// Duration in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> f64 {
        (self.end_secs - self.start_secs).max(0.0) * 1000.0
    }

    /// Whether this is a root span (no parent).
    #[must_use]
    pub const fn is_root(&self) -> bool {
        self.parent_span_id.is_none()
    }
}

// ──────────────────────────────────────────────────────────────
// SpanBuilder (active span under construction)
// ──────────────────────────────────────────────────────────────

/// Builder for constructing a span before it completes.
#[derive(Debug)]
pub struct SpanBuilder {
    /// Trace ID.
    trace_id: TraceId,
    /// Span ID.
    span_id: SpanId,
    /// Optional parent span ID.
    parent_span_id: Option<SpanId>,
    /// Span name.
    name: String,
    /// Span kind.
    kind: SpanKind,
    /// Start time.
    start_secs: f64,
    /// Accumulated attributes.
    attributes: Vec<SpanAttribute>,
    /// Accumulated events.
    events: Vec<SpanEvent>,
}

impl SpanBuilder {
    /// Start a new span with the given name.
    ///
    /// # Errors
    /// Returns `PvError::StringTooLong` if the name exceeds `MAX_SPAN_NAME_LEN`.
    pub fn start(name: impl Into<String>) -> PvResult<Self> {
        let name = name.into();
        if name.len() > MAX_SPAN_NAME_LEN {
            return Err(PvError::StringTooLong {
                field: "span_name",
                len: name.len(),
                max: MAX_SPAN_NAME_LEN,
            });
        }
        Ok(Self {
            trace_id: TraceId::new(),
            span_id: SpanId::new(),
            parent_span_id: None,
            name,
            kind: SpanKind::Internal,
            start_secs: now_secs(),
            attributes: Vec::new(),
            events: Vec::new(),
        })
    }

    /// Set the trace ID (for context propagation).
    #[must_use]
    pub const fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = trace_id;
        self
    }

    /// Set the parent span ID.
    #[must_use]
    pub const fn with_parent(mut self, parent: SpanId) -> Self {
        self.parent_span_id = Some(parent);
        self
    }

    /// Set the span kind.
    #[must_use]
    pub const fn with_kind(mut self, kind: SpanKind) -> Self {
        self.kind = kind;
        self
    }

    /// Add a string attribute.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if attribute limit exceeded.
    pub fn set_str(&mut self, key: impl Into<String>, value: impl Into<String>) -> PvResult<()> {
        if self.attributes.len() >= MAX_ATTRIBUTES {
            return Err(PvError::ConfigValidation(format!(
                "span attribute limit ({MAX_ATTRIBUTES}) exceeded"
            )));
        }
        self.attributes.push(SpanAttribute {
            key: key.into(),
            value: AttributeValue::Str(value.into()),
        });
        Ok(())
    }

    /// Add an integer attribute.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if attribute limit exceeded.
    pub fn set_int(&mut self, key: impl Into<String>, value: i64) -> PvResult<()> {
        if self.attributes.len() >= MAX_ATTRIBUTES {
            return Err(PvError::ConfigValidation(format!(
                "span attribute limit ({MAX_ATTRIBUTES}) exceeded"
            )));
        }
        self.attributes.push(SpanAttribute {
            key: key.into(),
            value: AttributeValue::Int(value),
        });
        Ok(())
    }

    /// Add a float attribute.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if attribute limit exceeded.
    pub fn set_float(&mut self, key: impl Into<String>, value: f64) -> PvResult<()> {
        if self.attributes.len() >= MAX_ATTRIBUTES {
            return Err(PvError::ConfigValidation(format!(
                "span attribute limit ({MAX_ATTRIBUTES}) exceeded"
            )));
        }
        self.attributes.push(SpanAttribute {
            key: key.into(),
            value: AttributeValue::Float(value),
        });
        Ok(())
    }

    /// Add a boolean attribute.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if attribute limit exceeded.
    pub fn set_bool(&mut self, key: impl Into<String>, value: bool) -> PvResult<()> {
        if self.attributes.len() >= MAX_ATTRIBUTES {
            return Err(PvError::ConfigValidation(format!(
                "span attribute limit ({MAX_ATTRIBUTES}) exceeded"
            )));
        }
        self.attributes.push(SpanAttribute {
            key: key.into(),
            value: AttributeValue::Bool(value),
        });
        Ok(())
    }

    /// Add a pane ID attribute.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if attribute limit exceeded.
    pub fn set_pane(&mut self, pane: &PaneId) -> PvResult<()> {
        self.set_str("orac.pane.id", pane.as_str())
    }

    /// Add a task ID attribute.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if attribute limit exceeded.
    pub fn set_task(&mut self, task: &TaskId) -> PvResult<()> {
        self.set_str("orac.task.id", task.as_str())
    }

    /// Add a timestamped event.
    pub fn add_event(&mut self, name: impl Into<String>) {
        self.events.push(SpanEvent {
            name: name.into(),
            timestamp_secs: now_secs(),
            attributes: Vec::new(),
        });
    }

    /// Finish the span with OK status.
    #[must_use]
    pub fn finish_ok(self) -> Span {
        Span {
            trace_id: self.trace_id,
            span_id: self.span_id,
            parent_span_id: self.parent_span_id,
            name: self.name,
            kind: self.kind,
            start_secs: self.start_secs,
            end_secs: now_secs(),
            status: SpanStatus::Ok,
            attributes: self.attributes,
            events: self.events,
        }
    }

    /// Finish the span with Error status.
    #[must_use]
    pub fn finish_error(self, message: impl Into<String>) -> Span {
        Span {
            trace_id: self.trace_id,
            span_id: self.span_id,
            parent_span_id: self.parent_span_id,
            name: self.name,
            kind: self.kind,
            start_secs: self.start_secs,
            end_secs: now_secs(),
            status: SpanStatus::Error {
                message: message.into(),
            },
            attributes: self.attributes,
            events: self.events,
        }
    }

    /// Finish the span with Unset status.
    #[must_use]
    pub fn finish(self) -> Span {
        Span {
            trace_id: self.trace_id,
            span_id: self.span_id,
            parent_span_id: self.parent_span_id,
            name: self.name,
            kind: self.kind,
            start_secs: self.start_secs,
            end_secs: now_secs(),
            status: SpanStatus::Unset,
            attributes: self.attributes,
            events: self.events,
        }
    }

    /// Get the span ID (for creating child spans).
    #[must_use]
    pub const fn span_id(&self) -> SpanId {
        self.span_id
    }

    /// Get the trace ID.
    #[must_use]
    pub const fn trace_id(&self) -> TraceId {
        self.trace_id
    }
}

// ──────────────────────────────────────────────────────────────
// TraceStore (ring buffer of completed spans)
// ──────────────────────────────────────────────────────────────

/// In-process trace store with a bounded ring buffer.
///
/// Thread-safe via interior mutability (`parking_lot::RwLock`).
/// Spans are evicted FIFO when the buffer is full.
#[derive(Debug)]
pub struct TraceStore {
    /// Interior-mutable state.
    state: RwLock<TraceStoreState>,
}

#[derive(Debug)]
struct TraceStoreState {
    /// Ring buffer of completed spans.
    spans: VecDeque<Span>,
    /// Maximum buffer capacity.
    capacity: usize,
    /// Total spans recorded (including evicted).
    total_recorded: u64,
    /// Total error spans recorded.
    total_errors: u64,
    /// Total spans dropped (evicted from buffer).
    total_dropped: u64,
}

impl TraceStore {
    /// Create a new trace store with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(TraceStoreState {
                spans: VecDeque::with_capacity(MAX_SPANS),
                capacity: MAX_SPANS,
                total_recorded: 0,
                total_errors: 0,
                total_dropped: 0,
            }),
        }
    }

    /// Create a trace store with a custom capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(1);
        Self {
            state: RwLock::new(TraceStoreState {
                spans: VecDeque::with_capacity(cap),
                capacity: cap,
                total_recorded: 0,
                total_errors: 0,
                total_dropped: 0,
            }),
        }
    }

    /// Record a completed span.
    pub fn record(&self, span: Span) {
        let mut state = self.state.write();
        if state.spans.len() >= state.capacity {
            state.spans.pop_front();
            state.total_dropped = state.total_dropped.saturating_add(1);
        }
        if span.status.is_error() {
            state.total_errors = state.total_errors.saturating_add(1);
        }
        state.total_recorded = state.total_recorded.saturating_add(1);
        state.spans.push_back(span);
    }

    /// Get the most recent `n` spans.
    #[must_use]
    pub fn recent(&self, n: usize) -> Vec<Span> {
        let state = self.state.read();
        state
            .spans
            .iter()
            .rev()
            .take(n)
            .cloned()
            .collect()
    }

    /// Get spans belonging to a specific trace.
    #[must_use]
    pub fn by_trace(&self, trace_id: &TraceId) -> Vec<Span> {
        let state = self.state.read();
        state
            .spans
            .iter()
            .filter(|s| s.trace_id == *trace_id)
            .cloned()
            .collect()
    }

    /// Get spans for a specific pane (via `orac.pane.id` attribute).
    #[must_use]
    pub fn by_pane(&self, pane_id: &PaneId) -> Vec<Span> {
        let target = pane_id.as_str();
        let state = self.state.read();
        state
            .spans
            .iter()
            .filter(|s| {
                s.attributes.iter().any(|a| {
                    a.key == "orac.pane.id"
                        && matches!(&a.value, AttributeValue::Str(v) if v == target)
                })
            })
            .cloned()
            .collect()
    }

    /// Get error spans only.
    #[must_use]
    pub fn errors(&self) -> Vec<Span> {
        let state = self.state.read();
        state
            .spans
            .iter()
            .filter(|s| s.status.is_error())
            .cloned()
            .collect()
    }

    /// Current number of spans in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.state.read().spans.len()
    }

    /// Whether the buffer is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state.read().spans.is_empty()
    }

    /// Total number of spans recorded (including evicted).
    #[must_use]
    pub fn total_recorded(&self) -> u64 {
        self.state.read().total_recorded
    }

    /// Total number of error spans.
    #[must_use]
    pub fn total_errors(&self) -> u64 {
        self.state.read().total_errors
    }

    /// Total number of spans evicted from the buffer.
    #[must_use]
    pub fn total_dropped(&self) -> u64 {
        self.state.read().total_dropped
    }

    /// Buffer capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.state.read().capacity
    }

    /// Clear all spans and reset counters.
    pub fn clear(&self) {
        let mut state = self.state.write();
        state.spans.clear();
        state.total_recorded = 0;
        state.total_errors = 0;
        state.total_dropped = 0;
    }

    /// Drain up to `batch_size` spans for export (oldest first).
    #[must_use]
    pub fn drain_batch(&self, batch_size: usize) -> Vec<Span> {
        let size = batch_size.min(DEFAULT_BATCH_SIZE);
        let mut state = self.state.write();
        let n = size.min(state.spans.len());
        state.spans.drain(..n).collect()
    }

    /// Get a summary of the trace store state.
    #[must_use]
    pub fn summary(&self) -> TraceStoreSummary {
        let state = self.state.read();
        TraceStoreSummary {
            buffered: state.spans.len(),
            capacity: state.capacity,
            total_recorded: state.total_recorded,
            total_errors: state.total_errors,
            total_dropped: state.total_dropped,
        }
    }
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary statistics for the trace store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStoreSummary {
    /// Current buffered span count.
    pub buffered: usize,
    /// Buffer capacity.
    pub capacity: usize,
    /// Total spans recorded since start.
    pub total_recorded: u64,
    /// Total error spans.
    pub total_errors: u64,
    /// Total spans evicted.
    pub total_dropped: u64,
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TraceId ──

    #[test]
    fn trace_id_new_is_non_zero() {
        let id = TraceId::new();
        assert!(!id.is_zero());
    }

    #[test]
    fn trace_id_from_bytes_zero() {
        let id = TraceId::from_bytes([0; 16]);
        assert!(id.is_zero());
    }

    #[test]
    fn trace_id_hex_is_32_chars() {
        let id = TraceId::new();
        assert_eq!(id.to_hex().len(), 32);
    }

    #[test]
    fn trace_id_display_matches_hex() {
        let id = TraceId::new();
        assert_eq!(format!("{id}"), id.to_hex());
    }

    #[test]
    fn trace_id_from_bytes_roundtrip() {
        let bytes = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let id = TraceId::from_bytes(bytes);
        assert_eq!(id.to_hex(), "0102030405060708090a0b0c0d0e0f10");
    }

    #[test]
    fn trace_id_default_is_non_zero() {
        let id = TraceId::default();
        assert!(!id.is_zero());
    }

    // ── SpanId ──

    #[test]
    fn span_id_new_is_non_zero() {
        let id = SpanId::new();
        assert!(!id.is_zero());
    }

    #[test]
    fn span_id_from_bytes_zero() {
        let id = SpanId::from_bytes([0; 8]);
        assert!(id.is_zero());
    }

    #[test]
    fn span_id_hex_is_16_chars() {
        let id = SpanId::new();
        assert_eq!(id.to_hex().len(), 16);
    }

    #[test]
    fn span_id_display_matches_hex() {
        let id = SpanId::new();
        assert_eq!(format!("{id}"), id.to_hex());
    }

    #[test]
    fn span_id_default_is_non_zero() {
        let id = SpanId::default();
        assert!(!id.is_zero());
    }

    // ── SpanKind ──

    #[test]
    fn span_kind_default_is_internal() {
        assert_eq!(SpanKind::default(), SpanKind::Internal);
    }

    // ── SpanStatus ──

    #[test]
    fn status_unset_code() {
        assert_eq!(SpanStatus::Unset.code(), STATUS_UNSET);
    }

    #[test]
    fn status_ok_code() {
        assert_eq!(SpanStatus::Ok.code(), STATUS_OK);
    }

    #[test]
    fn status_error_code() {
        let s = SpanStatus::Error { message: "fail".into() };
        assert_eq!(s.code(), STATUS_ERROR);
    }

    #[test]
    fn status_error_is_error() {
        let s = SpanStatus::Error { message: "oops".into() };
        assert!(s.is_error());
    }

    #[test]
    fn status_ok_not_error() {
        assert!(!SpanStatus::Ok.is_error());
    }

    #[test]
    fn status_unset_not_error() {
        assert!(!SpanStatus::Unset.is_error());
    }

    #[test]
    fn status_default_is_unset() {
        assert_eq!(SpanStatus::default(), SpanStatus::Unset);
    }

    // ── AttributeValue ──

    #[test]
    fn attr_str_display() {
        let v = AttributeValue::Str("hello".into());
        assert_eq!(format!("{v}"), "hello");
    }

    #[test]
    fn attr_int_display() {
        let v = AttributeValue::Int(42);
        assert_eq!(format!("{v}"), "42");
    }

    #[test]
    fn attr_float_display() {
        let v = AttributeValue::Float(3.14);
        assert!(format!("{v}").starts_with("3.14"));
    }

    #[test]
    fn attr_bool_display() {
        let v = AttributeValue::Bool(true);
        assert_eq!(format!("{v}"), "true");
    }

    // ── SpanBuilder ──

    #[test]
    fn builder_start_ok() {
        let b = SpanBuilder::start("test.span");
        assert!(b.is_ok());
    }

    #[test]
    fn builder_start_too_long() {
        let name = "x".repeat(MAX_SPAN_NAME_LEN + 1);
        let b = SpanBuilder::start(name);
        assert!(b.is_err());
    }

    #[test]
    fn builder_with_kind() {
        let b = SpanBuilder::start("test").unwrap().with_kind(SpanKind::Server);
        assert_eq!(b.kind, SpanKind::Server);
    }

    #[test]
    fn builder_with_parent() {
        let parent = SpanId::new();
        let b = SpanBuilder::start("test").unwrap().with_parent(parent);
        assert_eq!(b.parent_span_id, Some(parent));
    }

    #[test]
    fn builder_with_trace_id() {
        let tid = TraceId::from_bytes([1; 16]);
        let b = SpanBuilder::start("test").unwrap().with_trace_id(tid);
        assert_eq!(b.trace_id, tid);
    }

    #[test]
    fn builder_set_str() {
        let mut b = SpanBuilder::start("test").unwrap();
        assert!(b.set_str("key", "value").is_ok());
        assert_eq!(b.attributes.len(), 1);
    }

    #[test]
    fn builder_set_int() {
        let mut b = SpanBuilder::start("test").unwrap();
        assert!(b.set_int("count", 5).is_ok());
    }

    #[test]
    fn builder_set_float() {
        let mut b = SpanBuilder::start("test").unwrap();
        assert!(b.set_float("ratio", 0.5).is_ok());
    }

    #[test]
    fn builder_set_bool() {
        let mut b = SpanBuilder::start("test").unwrap();
        assert!(b.set_bool("active", true).is_ok());
    }

    #[test]
    fn builder_set_pane() {
        let mut b = SpanBuilder::start("test").unwrap();
        let pane = PaneId::new("fleet-alpha");
        assert!(b.set_pane(&pane).is_ok());
    }

    #[test]
    fn builder_set_task() {
        let mut b = SpanBuilder::start("test").unwrap();
        let task = TaskId::from_existing("task-001");
        assert!(b.set_task(&task).is_ok());
    }

    #[test]
    fn builder_add_event() {
        let mut b = SpanBuilder::start("test").unwrap();
        b.add_event("checkpoint");
        assert_eq!(b.events.len(), 1);
    }

    #[test]
    fn builder_attribute_limit() {
        let mut b = SpanBuilder::start("test").unwrap();
        for i in 0..MAX_ATTRIBUTES {
            assert!(b.set_str(format!("key_{i}"), "val").is_ok());
        }
        assert!(b.set_str("overflow", "val").is_err());
    }

    #[test]
    fn builder_finish_ok() {
        let b = SpanBuilder::start("test").unwrap();
        let span = b.finish_ok();
        assert_eq!(span.status, SpanStatus::Ok);
        assert_eq!(span.name, "test");
    }

    #[test]
    fn builder_finish_error() {
        let b = SpanBuilder::start("test").unwrap();
        let span = b.finish_error("timeout");
        assert!(span.status.is_error());
    }

    #[test]
    fn builder_finish_unset() {
        let b = SpanBuilder::start("test").unwrap();
        let span = b.finish();
        assert_eq!(span.status, SpanStatus::Unset);
    }

    #[test]
    fn builder_span_id_accessor() {
        let b = SpanBuilder::start("test").unwrap();
        let sid = b.span_id();
        assert!(!sid.is_zero());
    }

    #[test]
    fn builder_trace_id_accessor() {
        let b = SpanBuilder::start("test").unwrap();
        let tid = b.trace_id();
        assert!(!tid.is_zero());
    }

    // ── Span ──

    #[test]
    fn span_duration_non_negative() {
        let b = SpanBuilder::start("test").unwrap();
        let span = b.finish_ok();
        assert!(span.duration_ms() >= 0.0);
    }

    #[test]
    fn span_is_root_when_no_parent() {
        let b = SpanBuilder::start("test").unwrap();
        let span = b.finish_ok();
        assert!(span.is_root());
    }

    #[test]
    fn span_not_root_with_parent() {
        let parent = SpanId::new();
        let b = SpanBuilder::start("test").unwrap().with_parent(parent);
        let span = b.finish_ok();
        assert!(!span.is_root());
    }

    // ── TraceStore ──

    #[test]
    fn store_new_is_empty() {
        let store = TraceStore::new();
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn store_default_is_empty() {
        let store = TraceStore::default();
        assert!(store.is_empty());
    }

    #[test]
    fn store_capacity_default() {
        let store = TraceStore::new();
        assert_eq!(store.capacity(), MAX_SPANS);
    }

    #[test]
    fn store_with_capacity() {
        let store = TraceStore::with_capacity(50);
        assert_eq!(store.capacity(), 50);
    }

    #[test]
    fn store_with_capacity_minimum_one() {
        let store = TraceStore::with_capacity(0);
        assert_eq!(store.capacity(), 1);
    }

    #[test]
    fn store_record_increases_len() {
        let store = TraceStore::new();
        let span = SpanBuilder::start("test").unwrap().finish_ok();
        store.record(span);
        assert_eq!(store.len(), 1);
        assert_eq!(store.total_recorded(), 1);
    }

    #[test]
    fn store_record_error_counted() {
        let store = TraceStore::new();
        let span = SpanBuilder::start("test").unwrap().finish_error("fail");
        store.record(span);
        assert_eq!(store.total_errors(), 1);
    }

    #[test]
    fn store_evicts_when_full() {
        let store = TraceStore::with_capacity(3);
        for i in 0..5 {
            let span = SpanBuilder::start(format!("span-{i}")).unwrap().finish_ok();
            store.record(span);
        }
        assert_eq!(store.len(), 3);
        assert_eq!(store.total_recorded(), 5);
        assert_eq!(store.total_dropped(), 2);
    }

    #[test]
    fn store_recent_returns_newest_first() {
        let store = TraceStore::new();
        for i in 0..5 {
            let span = SpanBuilder::start(format!("span-{i}")).unwrap().finish_ok();
            store.record(span);
        }
        let recent = store.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].name, "span-4");
        assert_eq!(recent[1].name, "span-3");
        assert_eq!(recent[2].name, "span-2");
    }

    #[test]
    fn store_recent_more_than_available() {
        let store = TraceStore::new();
        let span = SpanBuilder::start("only").unwrap().finish_ok();
        store.record(span);
        let recent = store.recent(100);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn store_by_trace() {
        let store = TraceStore::new();
        let tid = TraceId::from_bytes([42; 16]);

        let span1 = SpanBuilder::start("a").unwrap().with_trace_id(tid).finish_ok();
        let span2 = SpanBuilder::start("b").unwrap().finish_ok();
        let span3 = SpanBuilder::start("c").unwrap().with_trace_id(tid).finish_ok();

        store.record(span1);
        store.record(span2);
        store.record(span3);

        let found = store.by_trace(&tid);
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn store_by_pane() {
        let store = TraceStore::new();

        let mut b1 = SpanBuilder::start("a").unwrap();
        b1.set_pane(&PaneId::new("fleet-alpha")).unwrap();
        store.record(b1.finish_ok());

        let mut b2 = SpanBuilder::start("b").unwrap();
        b2.set_pane(&PaneId::new("fleet-beta")).unwrap();
        store.record(b2.finish_ok());

        let mut b3 = SpanBuilder::start("c").unwrap();
        b3.set_pane(&PaneId::new("fleet-alpha")).unwrap();
        store.record(b3.finish_ok());

        let found = store.by_pane(&PaneId::new("fleet-alpha"));
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn store_errors_only() {
        let store = TraceStore::new();
        store.record(SpanBuilder::start("ok").unwrap().finish_ok());
        store.record(SpanBuilder::start("err").unwrap().finish_error("bad"));
        store.record(SpanBuilder::start("ok2").unwrap().finish_ok());

        let errs = store.errors();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].name, "err");
    }

    #[test]
    fn store_clear_resets_everything() {
        let store = TraceStore::new();
        store.record(SpanBuilder::start("test").unwrap().finish_ok());
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.total_recorded(), 0);
        assert_eq!(store.total_errors(), 0);
        assert_eq!(store.total_dropped(), 0);
    }

    #[test]
    fn store_drain_batch() {
        let store = TraceStore::new();
        for i in 0..10 {
            store.record(SpanBuilder::start(format!("s-{i}")).unwrap().finish_ok());
        }
        let batch = store.drain_batch(5);
        assert_eq!(batch.len(), 5);
        assert_eq!(store.len(), 5);
        assert_eq!(batch[0].name, "s-0");
    }

    #[test]
    fn store_drain_batch_clamps_to_default() {
        let store = TraceStore::new();
        for i in 0..200 {
            store.record(SpanBuilder::start(format!("s-{i}")).unwrap().finish_ok());
        }
        let batch = store.drain_batch(500);
        assert_eq!(batch.len(), DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn store_summary() {
        let store = TraceStore::with_capacity(5);
        store.record(SpanBuilder::start("a").unwrap().finish_ok());
        store.record(SpanBuilder::start("b").unwrap().finish_error("x"));
        let summary = store.summary();
        assert_eq!(summary.buffered, 2);
        assert_eq!(summary.capacity, 5);
        assert_eq!(summary.total_recorded, 2);
        assert_eq!(summary.total_errors, 1);
        assert_eq!(summary.total_dropped, 0);
    }

    // ── Thread safety ──

    #[test]
    fn trace_store_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TraceStore>();
    }

    #[test]
    fn trace_store_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TraceStore>();
    }

    #[test]
    fn span_builder_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SpanBuilder>();
    }

    // ── SpanEvent ──

    #[test]
    fn span_event_creation() {
        let evt = SpanEvent {
            name: "checkpoint".into(),
            timestamp_secs: 1234.5,
            attributes: vec![],
        };
        assert_eq!(evt.name, "checkpoint");
    }

    // ── Constants ──

    #[test]
    fn max_spans_reasonable() {
        assert!(MAX_SPANS >= 1000);
        assert!(MAX_SPANS <= 100_000);
    }

    #[test]
    fn max_attributes_reasonable() {
        assert!(MAX_ATTRIBUTES >= 8);
        assert!(MAX_ATTRIBUTES <= 128);
    }

    #[test]
    fn max_span_name_len_reasonable() {
        assert!(MAX_SPAN_NAME_LEN >= 64);
        assert!(MAX_SPAN_NAME_LEN <= 1024);
    }

    #[test]
    fn default_batch_size_reasonable() {
        assert!(DEFAULT_BATCH_SIZE >= 10);
        assert!(DEFAULT_BATCH_SIZE <= 1000);
    }

    // ── now_secs helper ──

    #[test]
    fn now_secs_plausible() {
        let t = now_secs();
        // After 2026-01-01
        assert!(t > 1_767_225_600.0);
    }

    // ── Span serde ──

    #[test]
    fn span_serializes_to_json() {
        let span = SpanBuilder::start("test").unwrap().finish_ok();
        let json = serde_json::to_string(&span);
        assert!(json.is_ok());
    }

    #[test]
    fn span_roundtrip_json() {
        let span = SpanBuilder::start("test").unwrap().finish_ok();
        let json = serde_json::to_string(&span).unwrap();
        let back: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "test");
    }

    #[test]
    fn trace_store_summary_serializes() {
        let summary = TraceStoreSummary {
            buffered: 10,
            capacity: 100,
            total_recorded: 50,
            total_errors: 2,
            total_dropped: 0,
        };
        let json = serde_json::to_string(&summary);
        assert!(json.is_ok());
    }
}
