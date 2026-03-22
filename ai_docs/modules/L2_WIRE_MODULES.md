---
title: "Layer 2: Wire — Module Documentation"
date: 2026-03-22
tags: [modules, wire, L2, orac-sidecar, ipc]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L2
modules: [m07, m08, m09]
---

# Layer 2: Wire (m07-m09)

> Unix socket IPC client connecting to the PV2 daemon bus. Always compiled.
> **Target LOC:** ~2,900 | **Target tests:** 60+
> **Source:** m07+m08 DROP-IN from PV2 M29+M30 / m09 NEW | **Phase:** 1

---

## Overview

L2 Wire provides the communication substrate between the ORAC sidecar and the PV2 daemon.
ORAC is a **client** of the PV2 bus -- it connects to the daemon's Unix domain socket,
performs a V2 handshake, subscribes to event patterns, and receives pushed events. It can
also submit tasks and cascade requests. The wire layer handles framing (NDJSON), reconnection
(exponential backoff), and protocol versioning (V2 with V1 compat shim).

Implementation order: m08 (types, no dependencies beyond L1) -> m07 (IPC client, depends on
m08) -> m09 (wire protocol adapter, depends on m08).

### Design Invariants (All Modules)

- ORAC is a **client**, not a server -- PV2 owns the socket at `/run/user/1000/pane-vortex-bus.sock`
- NDJSON framing: one JSON object per line, `\n` delimiter
- Handshake timeout: 5 seconds (reject slow connections)
- Reconnect backoff: exponential 100ms->5s (P17 -- never tight-loop)
- `BusTask` cap: 1000 concurrent tasks
- Lock ordering: always `AppState` before `BusState` (deadlock prevention)
- Frame variants must be exhaustively matched -- no `_ =>` wildcards

---

## m07 — IPC Client

**Source:** `src/m2_wire/m07_ipc_client.rs`
**LOC:** ~450
**Depends on:** L1 (`PaneId`, `PvError`, `PvResult`), m08 (`BusFrame`)
**Hot-Swap:** DROP-IN from PV2 M29 (`m29_ipc_bus.rs`), adapted for client role

### Design Decisions

- **Async tokio-based**: Uses `tokio::net::UnixStream` for non-blocking I/O. The connect and
  handshake operations are wrapped in `tokio::time::timeout` to enforce the 5-second limit.
- **Client, not server**: PV2's M29 is an IPC *server* (listener + accept loop). ORAC's m07
  inverts this: it connects *to* the daemon socket, sends `Handshake`, expects `Welcome` back.
  This is the primary adaptation from PV2.
- **Channel-based receive**: Received frames are delivered via a `tokio::sync::mpsc` channel
  with bounded capacity (256, A15). The sidecar tick loop consumes frames non-blockingly.
- **Exponential backoff reconnection (P17)**: On disconnection, the client waits 500ms, then
  doubles up to 5s. `_MAX_RECONNECT_ATTEMPTS = 10` before giving up. Never tight-loop (AP17).
- **Connection state FSM**: Three states: `Disconnected` -> `Connecting` -> `Connected`. State
  transitions are explicit, and `is_connected()` is a `const fn` check.
- **Builder pattern**: `IpcClient::new(pane_id)` with `with_socket_path()` for testing against
  non-default sockets.

### Types to Implement

```rust
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
/// Manages a single Unix socket connection with NDJSON framing.
/// Received frames are delivered via an mpsc channel for non-blocking
/// consumption by the sidecar tick loop.
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
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `IpcClient::new` | `fn new(pane_id: PaneId) -> Self` | Create client with default socket path |
| `IpcClient::with_socket_path` | `fn with_socket_path(pane_id: PaneId, path: impl Into<PathBuf>) -> Self` | Create with custom socket path |
| `IpcClient::connect` | `async fn connect(&mut self) -> PvResult<()>` | Connect + handshake (5s timeout) |
| `IpcClient::subscribe` | `async fn subscribe(&mut self, patterns: Vec<String>) -> PvResult<usize>` | Subscribe to event patterns |
| `IpcClient::send_frame` | `async fn send_frame(&mut self, frame: &BusFrame) -> PvResult<()>` | Send NDJSON frame to server |
| `IpcClient::submit_task` | `async fn submit_task(&mut self, task: BusTask) -> PvResult<TaskId>` | Submit task, await `TaskSubmitted` |
| `IpcClient::disconnect` | `async fn disconnect(&mut self, reason: &str) -> PvResult<()>` | Graceful disconnect with reason |
| `IpcClient::is_connected` | `const fn is_connected(&self) -> bool` | Connection state check |
| `IpcClient::connection_state` | `const fn connection_state(&self) -> ConnectionState` | Full state |
| `IpcClient::socket_path` | `fn socket_path(&self) -> &Path` | Configured socket path |
| `IpcClient::pane_id` | `fn pane_id(&self) -> &PaneId` | Client identity |
| `IpcClient::session_id` | `fn session_id(&self) -> Option<&str>` | Server-assigned session ID |

### Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_SOCKET_PATH` | `/run/user/1000/pane-vortex-bus.sock` | PV2 daemon bus socket |
| `PROTOCOL_VERSION` | `"2.0"` | V2 wire protocol identifier |
| `HANDSHAKE_TIMEOUT_SECS` | 5 | Maximum handshake wait time |
| `_RECV_CHANNEL_SIZE` | 256 | Bounded channel capacity (A15) |
| `_MAX_RECONNECT_ATTEMPTS` | 10 | Reconnect attempts before giving up |
| `_INITIAL_BACKOFF_MS` | 500 | Initial reconnection delay |

### Wire Protocol Sequence

```
ORAC (Client)                          PV2 (Server)
    │                                      │
    ├── connect(socket_path) ──────────────►│
    │                                      │
    ├── Handshake{pane_id, "2.0"} ────────►│
    │                                      │
    │◄──── Welcome{session_id, "2.0"} ─────┤
    │                                      │
    ├── Subscribe{["field.*", "task.*"]} ──►│
    │                                      │
    │◄──── Subscribed{count: 2} ───────────┤
    │                                      │
    │◄──── Event{field.tick, ...} ─────────┤  (push)
    │◄──── Event{sphere.registered, ...} ──┤  (push)
    │                                      │
    ├── Submit{task} ──────────────────────►│
    │                                      │
    │◄──── TaskSubmitted{task_id} ─────────┤
    │                                      │
    ├── Disconnect{reason} ────────────────►│
    │                                      │
```

### Tests

| Test | Validates |
|------|-----------|
| `client_starts_disconnected` | `IpcClient::new()` has state `Disconnected` |
| `client_custom_socket_path` | `with_socket_path()` stores custom path |
| `client_default_socket_path` | Default path is `DEFAULT_SOCKET_PATH` |
| `client_pane_id_preserved` | `pane_id()` returns the ID passed to `new()` |
| `client_session_id_none_initially` | `session_id()` is `None` before connect |
| `connect_timeout_on_missing_socket` | Connect to non-existent path returns `BusSocket` error |
| `connect_handshake_timeout` | Slow server triggers `BusSocket` with timeout message |
| `connect_handshake_success` | Full connect+handshake with mock server sets state to `Connected` |
| `subscribe_returns_count` | Subscribe returns the number of active subscriptions |
| `submit_task_returns_task_id` | Submit returns the `TaskId` from `TaskSubmitted` |
| `disconnect_resets_state` | After disconnect, state is `Disconnected`, session_id is `None` |
| `send_frame_serializes_ndjson` | Frame is serialized as single JSON line |
| `reconnect_backoff_doubles` | Each retry doubles the wait up to 5s cap |

### Cross-References

- [[Pane-Vortex IPC Bus -- Session 019b]] -- PV2 server-side design
- [[Session 050 -- Sidecar Deep Dive]] -- client vs server inversion
- `.claude/schemas/bus_frame.schema.json` -- 5 frame types
- `.claude/patterns.json` P17 (IPC reconnect backoff)
- `ai_docs/ANTI_PATTERNS.md` A15 (unbounded channels)
- `ai_docs/GOLD_STANDARD_PATTERNS.md` P3 (Result everywhere), P4 (scoped lock guards)
- PV2 source: `pane-vortex-v2/src/m7_coordination/m29_ipc_bus.rs`

---

## m08 — Bus Types

**Source:** `src/m2_wire/m08_bus_types.rs`
**LOC:** ~800
**Depends on:** L1 (`PaneId`, `TaskId`, `now_secs`)
**Hot-Swap:** DROP-IN from PV2 M30 (`m30_bus_types.rs`)

### Design Decisions

- **`BusFrame` as serde internally-tagged enum**: `#[serde(tag = "type")]` produces JSON like
  `{"type":"Handshake","pane_id":"orac","version":"2.0"}`. Exhaustive matching required -- no
  `_ =>` wildcard arms (compiler-enforced).
- **11 frame variants**: Covers the complete client-server lifecycle: `Handshake`, `Welcome`,
  `Subscribe`, `Subscribed`, `Submit`, `TaskSubmitted`, `Event`, `Cascade`, `CascadeAck`,
  `Disconnect`, `Error`. Client frames and server frames are distinguished by `is_client_frame()`
  / `is_server_frame()` methods.
- **Task lifecycle FSM**: `TaskStatus` enforces `Pending -> Claimed -> Completed|Failed`.
  Transition methods (`claim()`, `complete()`, `fail()`, `requeue()`) return `bool` to indicate
  whether the transition was valid -- invalid transitions are silently rejected, not panicked.
- **`TaskTarget` for dispatch strategy**: 4 variants: `Specific{pane_id}`, `AnyIdle` (default),
  `FieldDriven` (chimera routing), `Willing` (consent-gated). Internally-tagged serde.
- **`BusEvent` with glob pattern matching**: `matches_pattern()` supports `*` (match all),
  `prefix*` (prefix match), and exact match. Used by the subscription filter.
- **NDJSON serialization**: `to_ndjson()` and `from_ndjson()` are convenience wrappers around
  `serde_json::to_string()` / `from_str()`. No trailing newline in the output -- the caller
  adds `\n`.

### Types to Implement

```rust
/// Target selection strategy for a bus task.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TaskTarget {
    /// Route to a specific sphere by ID.
    Specific { pane_id: PaneId },
    /// Route to any idle sphere.
    #[default]
    AnyIdle,
    /// Route using field-driven heuristics (chimera routing).
    FieldDriven,
    /// Route to any sphere that declares willingness.
    Willing,
}

/// Lifecycle status of a bus task.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    #[default] Pending,
    Claimed,
    Completed,
    Failed,
}

/// A task submitted to the IPC bus for fleet coordination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusTask {
    pub id: TaskId,
    pub description: String,
    pub target: TaskTarget,
    pub status: TaskStatus,
    pub submitted_by: PaneId,
    pub claimed_by: Option<PaneId>,
    pub submitted_at: f64,
    pub claimed_at: Option<f64>,
    pub completed_at: Option<f64>,
}

/// A typed event published on the bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusEvent {
    pub event_type: String,
    pub data: serde_json::Value,
    pub tick: u64,
    pub timestamp: f64,
}

/// IPC bus frame -- the NDJSON wire protocol message type.
/// Each variant maps to one line of NDJSON on the Unix domain socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum BusFrame {
    // Client -> Server
    Handshake { pane_id: PaneId, version: String },
    Subscribe { patterns: Vec<String> },
    Submit { task: BusTask },
    Disconnect { reason: String },

    // Server -> Client
    Welcome { session_id: String, version: String },
    Subscribed { count: usize },
    TaskSubmitted { task_id: TaskId },
    Event { event: BusEvent },
    Error { code: u16, message: String },

    // Bidirectional
    Cascade { source: PaneId, target: PaneId, brief: String },
    CascadeAck { source: PaneId, target: PaneId, accepted: bool },
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `BusTask::new` | `fn new(description: String, target: TaskTarget, submitted_by: PaneId) -> Self` | Create pending task with UUID |
| `BusTask::is_pending` | `const fn is_pending(&self) -> bool` | Check if task is unclaimed |
| `BusTask::is_terminal` | `const fn is_terminal(&self) -> bool` | Check if `Completed` or `Failed` |
| `BusTask::claim` | `fn claim(&mut self, claimer: PaneId) -> bool` | Transition `Pending -> Claimed` |
| `BusTask::complete` | `fn complete(&mut self) -> bool` | Transition `Claimed -> Completed` |
| `BusTask::fail` | `fn fail(&mut self) -> bool` | Transition `Claimed -> Failed` |
| `BusTask::requeue` | `fn requeue(&mut self) -> bool` | Transition `Claimed -> Pending` (stale recovery) |
| `BusTask::elapsed_secs` | `fn elapsed_secs(&self) -> f64` | Time since submission |
| `BusEvent::new` | `fn new(event_type: String, data: Value, tick: u64) -> Self` | Create event with timestamp |
| `BusEvent::text` | `fn text(event_type: &str, message: &str, tick: u64) -> Self` | Create simple text event |
| `BusEvent::matches_pattern` | `fn matches_pattern(&self, pattern: &str) -> bool` | Glob matching for subscriptions |
| `BusFrame::to_ndjson` | `fn to_ndjson(&self) -> Result<String, serde_json::Error>` | Serialize to NDJSON line |
| `BusFrame::from_ndjson` | `fn from_ndjson(line: &str) -> Result<Self, serde_json::Error>` | Deserialize from NDJSON line |
| `BusFrame::is_client_frame` | `const fn is_client_frame(&self) -> bool` | Check frame origin |
| `BusFrame::is_server_frame` | `const fn is_server_frame(&self) -> bool` | Check frame origin |
| `BusFrame::frame_type` | `const fn frame_type(&self) -> &'static str` | Frame type name for logging |

### Task Status FSM

```
           ┌─────────┐
           │ Pending  │◄──── requeue()
           └────┬─────┘         │
                │ claim()       │
                ▼               │
           ┌─────────┐         │
           │ Claimed  │─────────┘
           └──┬────┬──┘
    complete()│    │fail()
              ▼    ▼
     ┌───────────┐ ┌────────┐
     │ Completed │ │ Failed │
     └───────────┘ └────────┘
         (terminal)   (terminal)
```

### Tests

| Test | Validates |
|------|-----------|
| `task_target_default_is_any_idle` | `TaskTarget::default() == AnyIdle` |
| `task_target_specific_display` | `Specific{pane_id}` displays as `Specific(pane_id)` |
| `task_status_default_is_pending` | `TaskStatus::default() == Pending` |
| `bus_task_new_is_pending` | New task has `Pending` status, no claimer |
| `bus_task_claim_from_pending` | `claim()` returns true, sets status and claimer |
| `bus_task_claim_from_claimed` | Double claim returns false |
| `bus_task_complete_from_claimed` | `complete()` returns true, sets timestamp |
| `bus_task_complete_from_pending` | `complete()` from `Pending` returns false |
| `bus_task_fail_from_claimed` | `fail()` returns true |
| `bus_task_requeue_from_claimed` | `requeue()` clears claimer, resets to `Pending` |
| `bus_task_requeue_from_pending` | `requeue()` from `Pending` returns false |
| `bus_task_is_terminal` | `Completed` and `Failed` are terminal |
| `bus_task_elapsed_secs` | Returns positive value |
| `bus_event_matches_wildcard` | `"*"` matches everything |
| `bus_event_matches_prefix` | `"field.*"` matches `"field.tick"` |
| `bus_event_matches_exact` | `"field.tick"` matches `"field.tick"` only |
| `bus_event_no_match` | `"sphere.*"` does not match `"field.tick"` |
| `bus_frame_handshake_roundtrip` | Serialize -> deserialize preserves Handshake |
| `bus_frame_welcome_roundtrip` | Serialize -> deserialize preserves Welcome |
| `bus_frame_event_roundtrip` | Serialize -> deserialize preserves Event |
| `bus_frame_cascade_roundtrip` | Serialize -> deserialize preserves Cascade |
| `bus_frame_is_client_frame` | `Handshake`, `Subscribe`, `Submit`, `Disconnect` are client frames |
| `bus_frame_is_server_frame` | `Welcome`, `Subscribed`, `TaskSubmitted`, `Event`, `Error` are server frames |
| `bus_frame_ndjson_no_trailing_newline` | `to_ndjson()` output contains no `\n` |
| `bus_frame_type_name` | Each variant returns the correct type string |
| `bus_frame_error_with_code` | Error frame preserves code and message |

### Cross-References

- [[Pane-Vortex IPC Bus -- Session 019b]] -- PV2 bus architecture
- [[Session 050 -- Sidecar Deep Dive]] -- frame inventory
- `.claude/schemas/bus_frame.schema.json` -- 5 frame types
- `.claude/schemas/bus_event.schema.json` -- 24 event types
- `ai_docs/GOLD_STANDARD_PATTERNS.md` P9 (event emission on state transitions)
- PV2 source: `pane-vortex-v2/src/m7_coordination/m30_bus_types.rs`

---

## m09 — Wire Protocol

**Source:** `src/m2_wire/m09_wire_protocol.rs`
**LOC:** ~200 (currently scaffold stub)
**Depends on:** L1 (`m01_core_types`), m08 (`BusFrame`, `BusEvent`)
**Hot-Swap:** NEW (ORAC-specific wire protocol adapter)

### Design Decisions

- **V2 native, V1 compat shim**: The primary protocol is V2 (NDJSON, internally-tagged enums).
  A V1 compatibility layer translates old-format frames for backward compatibility with V1
  swarm-sidecar consumers if any still exist.
- **Keepalive mechanism**: Periodic keepalive frames prevent idle connection drops. The server
  sends a keepalive every 30 seconds; the client replies within 5 seconds or is considered
  disconnected.
- **Subscription filter engine**: Centralizes the glob-matching logic for event subscriptions.
  Supports `*` (all), `namespace.*` (prefix), and exact match. Used by m07 to filter incoming
  events before delivering to the tick loop.
- **Frame validation**: Before dispatching a received frame, m09 validates: (a) the frame
  deserializes as valid `BusFrame`, (b) server frames are expected in the current state (no
  `Welcome` when already connected), (c) event types match at least one subscription.
- **Protocol version negotiation**: During handshake, client sends version `"2.0"`. If server
  returns a different version, the V1 compat layer is activated. Version mismatch is logged
  but not rejected (forward compatibility).

### Types to Implement

```rust
/// Wire protocol version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireVersion {
    /// V1: legacy swarm-sidecar format (JSON lines, different tag schema).
    V1,
    /// V2: NDJSON with serde internally-tagged enums.
    V2,
}

/// Subscription filter for incoming bus events.
#[derive(Debug, Clone)]
pub struct SubscriptionFilter {
    /// Active patterns (e.g. "field.*", "sphere.registered", "*").
    patterns: Vec<String>,
}

/// Keepalive configuration.
#[derive(Debug, Clone, Copy)]
pub struct KeepaliveConfig {
    /// Interval between keepalive frames (seconds).
    pub interval_secs: u64,
    /// Timeout for keepalive response (seconds).
    pub timeout_secs: u64,
}

/// V1 compatibility adapter for legacy frame format.
#[derive(Debug)]
pub struct V1Compat;
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `SubscriptionFilter::new` | `fn new() -> Self` | Create empty filter |
| `SubscriptionFilter::add_pattern` | `fn add_pattern(&mut self, pattern: impl Into<String>)` | Add subscription pattern |
| `SubscriptionFilter::matches` | `fn matches(&self, event: &BusEvent) -> bool` | Check if event matches any pattern |
| `SubscriptionFilter::pattern_count` | `fn pattern_count(&self) -> usize` | Number of active patterns |
| `KeepaliveConfig::default` | `fn default() -> Self` | 30s interval, 5s timeout |
| `V1Compat::translate_to_v2` | `fn translate_to_v2(line: &str) -> PvResult<BusFrame>` | Convert V1 frame to V2 |
| `V1Compat::translate_to_v1` | `fn translate_to_v1(frame: &BusFrame) -> PvResult<String>` | Convert V2 frame to V1 |
| `validate_frame_sequence` | `fn validate_frame_sequence(state: ConnectionState, frame: &BusFrame) -> PvResult<()>` | State-aware frame validation |
| `negotiate_version` | `fn negotiate_version(client: &str, server: &str) -> WireVersion` | Determine effective protocol version |

### Tests

| Test | Validates |
|------|-----------|
| `m09_wire_protocol_scaffold` | Scaffold test passes (current state) |
| `subscription_filter_empty` | Empty filter matches nothing |
| `subscription_filter_wildcard` | `"*"` matches all event types |
| `subscription_filter_prefix` | `"field.*"` matches `"field.tick"` but not `"sphere.registered"` |
| `subscription_filter_exact` | Exact match only matches exact string |
| `subscription_filter_multiple` | Multiple patterns OR together |
| `keepalive_default_config` | Default interval=30s, timeout=5s |
| `v1_compat_translate_handshake` | V1 handshake translates to V2 `BusFrame::Handshake` |
| `v1_compat_translate_event` | V1 event translates to V2 `BusFrame::Event` |
| `validate_frame_welcome_when_connecting` | `Welcome` valid in `Connecting` state |
| `validate_frame_welcome_when_connected` | `Welcome` invalid in `Connected` state |
| `validate_frame_event_when_connected` | `Event` valid in `Connected` state |
| `validate_frame_event_when_disconnected` | `Event` invalid in `Disconnected` state |
| `negotiate_version_both_v2` | Both `"2.0"` -> `WireVersion::V2` |
| `negotiate_version_server_v1` | Server `"1.0"` -> `WireVersion::V1` |

### Cross-References

- [[Pane-Vortex IPC Bus -- Session 019b]] -- V2 wire protocol specification
- [[Session 050 -- Sidecar Deep Dive]] -- V1/V2 mismatch that killed V1 sidecar for 17 hours
- `.claude/schemas/bus_frame.schema.json` -- frame type definitions
- `.claude/patterns.json` P17 (IPC reconnect backoff -- protocol layer feeds state to m07 backoff logic)
- ORAC_PLAN.md -- Phase 1 Detail, V2 wire protocol section
- V1 sidecar source: `~/claude-code-workspace/swarm-sidecar/` (753 LOC -- the system ORAC replaces)

---

## Layer 2 Integration Notes

### Lock Ordering (Critical)

When ORAC holds both `AppState` (from field_state) and `BusState` (from IPC), always acquire
`AppState` first. The PV2 codebase established this ordering after a deadlock bug in Session 021.
The sidecar inherits this invariant:

```rust
// CORRECT: AppState before BusState
let field_data = {
    let guard = app_state.read();
    guard.field.order.r
}; // guard dropped

{
    let mut bus = bus_state.write();
    bus.publish_event(BusEvent::text("field.tick", &field_data.to_string(), tick));
} // guard dropped
```

### Event Flow (m07 + m08 + m09)

1. m07 `IpcClient::connect()` opens Unix socket, sends `Handshake` frame (m08)
2. Server responds with `Welcome` frame -- m09 validates sequence
3. m07 `subscribe(["field.*", "task.*", "sphere.*"])` sends `Subscribe` frame
4. Server pushes `Event` frames -- m09 `SubscriptionFilter::matches()` filters
5. Matched events are delivered to tick loop via bounded mpsc channel (m07)
6. Tick loop processes events, updates `AppState` (field_state), may `Submit` tasks (m08)
7. On shutdown, m07 sends `Disconnect` frame with reason string

### Error Handling

All L2 errors use `PvError::BusSocket` (connection failures) or `PvError::BusProtocol`
(protocol violations). Both are classified as `is_retryable() == true` by the `ErrorClassifier`
trait (m02). The conductor (L6) uses this classification to decide whether to retry or
escalate.

### Pattern Compliance

| Pattern | Applies To | How |
|---------|-----------|-----|
| P01 (Phase wrapping) | Not directly -- L2 carries phase data, L4 wraps it | Pass-through |
| P02 (Error propagation) | All methods | `?` operator with `PvResult` |
| P06 (NaN guard) | Not directly -- L2 carries numeric data, L1 validates | Pass-through |
| P09 (Amortised prune) | Not directly -- task GC is in L6 tick loop | N/A |
| P10 (VecDeque for logs) | Event buffer | Bounded channel replaces VecDeque here |
| P14 (Hook sub-ms) | Not directly -- L3 hooks use L2 | Async non-blocking |
| P17 (IPC reconnect backoff) | m07 reconnection | Exponential 500ms->5s |
| A15 (Unbounded channels) | m07 receive channel | Bounded at 256 |
