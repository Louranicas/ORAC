//! # M29: IPC Bus State
//!
//! Bus state management for the IPC coordination layer. Manages tasks, events,
//! subscriptions, and cascade tracking. This module also contains the async
//! `Unix domain socket` listener and per-client handler for the `NDJSON` wire protocol.
//!
//! ## Layer: L7 | Module: M29 | Dependencies: L1, L7 (M30 bus types)
//! ## Wire Protocol: `Handshake` -> `Welcome` -> `Subscribe`/`Submit`/`Event` frames
//! ## Design Constraints: C5 (lock ordering), C12 (bounded channel 256)

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::m1_foundation::{
    m01_core_types::{now_secs, PaneId, TaskId},
    m02_error_handling::{PvError, PvResult},
};
use crate::m3_field::m15_app_state::SharedState;

use super::m30_bus_types::{BusEvent, BusFrame, BusTask, TaskStatus, TaskTarget};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Maximum number of pending tasks in the bus.
const MAX_PENDING_TASKS: usize = 256;

/// Maximum events retained in the event ring buffer.
const MAX_EVENTS: usize = 1000;

/// Maximum cascade events per rate-limit window.
const CASCADE_RATE_LIMIT: u32 = 10;

/// Rate-limit window duration in seconds.
const CASCADE_WINDOW_SECS: f64 = 60.0;

/// Maximum number of subscribers.
const MAX_SUBSCRIBERS: usize = 50;

// ──────────────────────────────────────────────────────────────
// Subscriber
// ──────────────────────────────────────────────────────────────

/// A connected bus subscriber with glob-pattern event filtering.
#[derive(Debug, Clone)]
pub struct BusSubscriber {
    /// The subscriber's sphere ID.
    pub pane_id: PaneId,
    /// Glob patterns this subscriber is interested in.
    pub patterns: Vec<String>,
    /// Session ID assigned at handshake.
    pub session_id: String,
    /// Unix timestamp of last activity.
    pub last_active: f64,
    /// Whether this subscriber uses V1 wire format (`BUG-028` fix).
    pub is_v1_client: bool,
}

impl BusSubscriber {
    /// Create a new subscriber.
    #[must_use]
    pub fn new(pane_id: PaneId, session_id: String) -> Self {
        Self {
            pane_id,
            patterns: Vec::new(),
            session_id,
            last_active: now_secs(),
            is_v1_client: false,
        }
    }

    /// Whether this subscriber matches an event type.
    #[must_use]
    pub fn matches_event(&self, event_type: &str) -> bool {
        self.patterns.iter().any(|pattern| {
            if pattern == "*" {
                return true;
            }
            if let Some(prefix) = pattern.strip_suffix('*') {
                return event_type.starts_with(prefix);
            }
            pattern == event_type
        })
    }

    /// Touch the last-active timestamp.
    pub fn touch(&mut self) {
        self.last_active = now_secs();
    }
}

// ──────────────────────────────────────────────────────────────
// Cascade rate limiter
// ──────────────────────────────────────────────────────────────

/// Rate limiter for cascade handoff events.
#[derive(Debug, Clone)]
pub struct CascadeRateLimiter {
    /// Count of cascades in the current window.
    count: u32,
    /// Start of the current window (Unix timestamp).
    window_start: f64,
}

impl CascadeRateLimiter {
    /// Create a new rate limiter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: 0,
            window_start: now_secs(),
        }
    }

    /// Check whether a new cascade is allowed, and if so, increment the counter.
    ///
    /// # Errors
    /// Returns `PvError::CascadeRateLimit` if the rate limit is exceeded.
    pub fn check_and_increment(&mut self) -> PvResult<()> {
        let now = now_secs();
        if now - self.window_start > CASCADE_WINDOW_SECS {
            self.count = 0;
            self.window_start = now;
        }
        if self.count >= CASCADE_RATE_LIMIT {
            return Err(PvError::CascadeRateLimit {
                per_minute: CASCADE_RATE_LIMIT,
            });
        }
        self.count += 1;
        Ok(())
    }

    /// Current count in this window.
    #[must_use]
    pub const fn count(&self) -> u32 {
        self.count
    }

    /// Reset the rate limiter.
    pub fn reset(&mut self) {
        self.count = 0;
        self.window_start = now_secs();
    }
}

impl Default for CascadeRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// BusState
// ──────────────────────────────────────────────────────────────

/// IPC bus state for the `NDJSON` wire protocol over `Unix domain socket`.
///
/// Manages the task queue, event ring buffer, subscriber registry, and
/// cascade rate limiting. Lock ordering: always acquire `AppState` BEFORE
/// `BusState` to prevent deadlocks.
// NOTE: BusState is wrapped in Arc<RwLock<BusState>> by the daemon.
// Methods take &mut self for exclusive write access through the lock.
#[derive(Debug)]
pub struct BusState {
    /// Active tasks indexed by ID.
    tasks: HashMap<TaskId, BusTask>,
    /// Event ring buffer (most recent at back).
    events: VecDeque<BusEvent>,
    /// Connected subscribers indexed by session ID.
    subscribers: HashMap<String, BusSubscriber>,
    /// Cascade rate limiter.
    cascade_limiter: CascadeRateLimiter,
    /// Set of completed/failed task IDs for deduplication.
    completed_task_ids: HashSet<String>,
    /// Field-driven suggestions (ring buffer, max 50).
    suggestions: VecDeque<serde_json::Value>,
    /// Total suggestions generated across all ticks.
    total_suggestions: u64,
}

impl BusState {
    /// Create a new empty bus state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            events: VecDeque::with_capacity(MAX_EVENTS),
            subscribers: HashMap::new(),
            cascade_limiter: CascadeRateLimiter::new(),
            completed_task_ids: HashSet::new(),
            suggestions: VecDeque::with_capacity(50),
            total_suggestions: 0,
        }
    }

    // ── Task management ──

    /// Submit a new task to the bus.
    ///
    /// # Errors
    /// Returns `PvError::BusProtocol` if too many pending tasks.
    pub fn submit_task(&mut self, task: BusTask) -> PvResult<TaskId> {
        let pending = self.tasks.values().filter(|t| t.is_pending()).count();
        if pending >= MAX_PENDING_TASKS {
            return Err(PvError::BusProtocol(format!(
                "too many pending tasks ({pending}/{MAX_PENDING_TASKS})"
            )));
        }
        let id = task.id.clone();
        self.tasks.insert(id.clone(), task);
        Ok(id)
    }

    /// Claim a task by ID for a specific sphere.
    ///
    /// # Errors
    /// Returns `PvError::BusTaskNotFound` if the task does not exist.
    /// Returns `PvError::BusProtocol` if the task is not pending.
    pub fn claim_task(&mut self, task_id: &TaskId, claimer: PaneId) -> PvResult<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| PvError::BusTaskNotFound(task_id.as_str().to_owned()))?;
        if !task.claim(claimer) {
            return Err(PvError::BusProtocol(format!(
                "task {} is not pending (status: {})",
                task_id, task.status
            )));
        }
        Ok(())
    }

    /// Mark a task as completed.
    ///
    /// # Errors
    /// Returns `PvError::BusTaskNotFound` if the task does not exist.
    /// Returns `PvError::BusProtocol` if the task is not claimed.
    pub fn complete_task(&mut self, task_id: &TaskId) -> PvResult<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| PvError::BusTaskNotFound(task_id.as_str().to_owned()))?;
        if !task.complete() {
            return Err(PvError::BusProtocol(format!(
                "task {} is not claimed (status: {})",
                task_id, task.status
            )));
        }
        self.completed_task_ids.insert(task_id.as_str().to_owned());
        Ok(())
    }

    /// Mark a task as failed.
    ///
    /// # Errors
    /// Returns `PvError::BusTaskNotFound` if the task does not exist.
    /// Returns `PvError::BusProtocol` if the task is not claimed.
    pub fn fail_task(&mut self, task_id: &TaskId) -> PvResult<()> {
        let task = self
            .tasks
            .get_mut(task_id)
            .ok_or_else(|| PvError::BusTaskNotFound(task_id.as_str().to_owned()))?;
        if !task.fail() {
            return Err(PvError::BusProtocol(format!(
                "task {} is not claimed (status: {})",
                task_id, task.status
            )));
        }
        self.completed_task_ids.insert(task_id.as_str().to_owned());
        Ok(())
    }

    /// Get all pending tasks.
    #[must_use]
    pub fn pending_tasks(&self) -> Vec<&BusTask> {
        self.tasks.values().filter(|t| t.is_pending()).collect()
    }

    /// Get all tasks matching a specific status.
    #[must_use]
    pub fn tasks_by_status(&self, status: TaskStatus) -> Vec<&BusTask> {
        self.tasks
            .values()
            .filter(|t| t.status == status)
            .collect()
    }

    /// Get a task by ID.
    #[must_use]
    pub fn get_task(&self, task_id: &TaskId) -> Option<&BusTask> {
        self.tasks.get(task_id)
    }

    /// Get a mutable reference to a `BusTask` by ID (for claim/complete/fail).
    pub fn get_task_mut(&mut self, task_id: &str) -> Option<&mut BusTask> {
        self.tasks
            .iter_mut()
            .find(|(k, _)| k.as_str() == task_id)
            .map(|(_, v)| v)
    }

    /// Get pending tasks targeted at a specific sphere.
    #[must_use]
    pub fn tasks_for_sphere(&self, pane_id: &PaneId) -> Vec<&BusTask> {
        self.tasks
            .values()
            .filter(|t| {
                t.is_pending()
                    && match &t.target {
                        TaskTarget::Specific { pane_id: tid } => tid == pane_id,
                        TaskTarget::AnyIdle | TaskTarget::FieldDriven | TaskTarget::Willing => true,
                    }
            })
            .collect()
    }

    /// Total task count (all statuses).
    #[must_use]
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }

    /// Prune completed/failed tasks older than `max_age_secs`.
    pub fn prune_completed_tasks(&mut self, max_age_secs: f64) {
        let now = now_secs();
        self.tasks.retain(|_, task| {
            if task.is_terminal() {
                if let Some(completed_at) = task.completed_at {
                    return now - completed_at < max_age_secs;
                }
                return false;
            }
            true
        });
    }

    /// Requeue claimed tasks that have been stale longer than `timeout_secs`.
    ///
    /// Returns the number of tasks requeued.
    pub fn prune_stale_claims(&mut self, timeout_secs: f64) -> usize {
        let now = now_secs();
        let mut requeued = 0;
        for task in self.tasks.values_mut() {
            if task.status == TaskStatus::Claimed {
                if let Some(claimed_at) = task.claimed_at {
                    if now - claimed_at > timeout_secs {
                        task.requeue();
                        requeued += 1;
                    }
                }
            }
        }
        requeued
    }

    // ── Suggestion management ──

    /// Add a field-driven suggestion (ring buffer, max 50).
    pub fn add_suggestion(&mut self, suggestion: serde_json::Value) {
        self.suggestions.push_back(suggestion);
        self.total_suggestions = self.total_suggestions.saturating_add(1);
        while self.suggestions.len() > 50 {
            self.suggestions.pop_front();
        }
    }

    /// Return the most recent suggestions (up to `n`).
    #[must_use]
    pub fn recent_suggestions(&self, n: usize) -> Vec<&serde_json::Value> {
        self.suggestions.iter().rev().take(n).collect()
    }

    /// Return total suggestions generated across all ticks.
    #[must_use]
    pub const fn total_suggestions(&self) -> u64 {
        self.total_suggestions
    }

    // ── Event management ──

    /// Publish an event to the bus.
    pub fn publish_event(&mut self, event: BusEvent) {
        self.events.push_back(event);
        while self.events.len() > MAX_EVENTS {
            self.events.pop_front();
        }
    }

    /// Get recent events (most recent `n`).
    #[must_use]
    pub fn recent_events(&self, n: usize) -> Vec<&BusEvent> {
        self.events.iter().rev().take(n).collect()
    }

    /// Get events matching a pattern.
    #[must_use]
    pub fn events_matching(&self, pattern: &str) -> Vec<&BusEvent> {
        self.events
            .iter()
            .filter(|e| e.matches_pattern(pattern))
            .collect()
    }

    /// Total event count in the buffer.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    // ── Subscriber management ──

    /// Register a new subscriber.
    ///
    /// # Errors
    /// Returns `PvError::BusProtocol` if the maximum number of subscribers is reached.
    pub fn add_subscriber(&mut self, subscriber: BusSubscriber) -> PvResult<()> {
        if self.subscribers.len() >= MAX_SUBSCRIBERS {
            return Err(PvError::BusProtocol(format!(
                "max subscribers reached ({MAX_SUBSCRIBERS})"
            )));
        }
        self.subscribers
            .insert(subscriber.session_id.clone(), subscriber);
        Ok(())
    }

    /// Remove a subscriber by session ID.
    pub fn remove_subscriber(&mut self, session_id: &str) -> Option<BusSubscriber> {
        self.subscribers.remove(session_id)
    }

    /// Get subscriber IDs that match an event type.
    #[must_use]
    pub fn matching_subscribers(&self, event_type: &str) -> Vec<String> {
        self.subscribers
            .values()
            .filter(|s| s.matches_event(event_type))
            .map(|s| s.session_id.clone())
            .collect()
    }

    /// Total subscriber count.
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.len()
    }

    /// Update subscription patterns for a subscriber.
    ///
    /// # Errors
    /// Returns `PvError::BusProtocol` if the subscriber is not found.
    pub fn update_subscriptions(
        &mut self,
        session_id: &str,
        patterns: Vec<String>,
    ) -> PvResult<usize> {
        let subscriber = self.subscribers.get_mut(session_id).ok_or_else(|| {
            PvError::BusProtocol(format!("subscriber not found: {session_id}"))
        })?;
        subscriber.patterns = patterns;
        subscriber.touch();
        Ok(subscriber.patterns.len())
    }

    // ── Cascade rate limiting ──

    /// Check cascade rate limit.
    ///
    /// # Errors
    /// Returns `PvError::CascadeRateLimit` if the limit is exceeded.
    pub fn check_cascade_rate(&mut self) -> PvResult<()> {
        self.cascade_limiter.check_and_increment()
    }

    /// Current cascade count in the window.
    #[must_use]
    pub const fn cascade_count(&self) -> u32 {
        self.cascade_limiter.count()
    }

    /// Reset cascade rate limiter.
    pub fn reset_cascade_rate(&mut self) {
        self.cascade_limiter.reset();
    }
}

impl Default for BusState {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Socket listener constants
// ──────────────────────────────────────────────────────────────

/// Maximum concurrent connections to the IPC bus socket.
const MAX_CONNECTIONS: usize = 200;

/// Maximum line length for NDJSON frames (bytes).
const MAX_LINE_LENGTH: usize = 65_536;

/// Handshake timeout in seconds.
const HANDSHAKE_TIMEOUT_SECS: u64 = 5;

/// Outgoing channel capacity per connection.
const OUTGOING_CHANNEL_CAP: usize = 256;

/// Protocol version string.
const PROTOCOL_VERSION: &str = "2.0";

// ──────────────────────────────────────────────────────────────
// Socket path resolution
// ──────────────────────────────────────────────────────────────

/// Resolve the `Unix domain socket` path for the IPC bus.
///
/// Resolution order:
/// 1. `PANE_VORTEX_SOCKET` environment variable (exact path)
/// 2. `XDG_RUNTIME_DIR/pane-vortex-bus.sock` (runtime directory)
/// 3. `/tmp/pane-vortex-bus.sock` (fallback)
#[must_use]
pub fn socket_path() -> PathBuf {
    if let Ok(p) = std::env::var("PANE_VORTEX_SOCKET") {
        return PathBuf::from(p);
    }
    if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime).join("pane-vortex-bus.sock");
    }
    PathBuf::from("/tmp/pane-vortex-bus.sock")
}

// ──────────────────────────────────────────────────────────────
// Peer credential verification
// ──────────────────────────────────────────────────────────────

/// Verify that the connecting peer has the same UID as this process.
///
/// Uses `tokio::net::unix::UCred` (safe API, no `unsafe` needed).
/// Returns `Ok(uid)` if the peer's UID matches, or an error otherwise.
///
/// # Errors
/// Returns `PvError::BusSocket` if credentials cannot be read or UID mismatches.
fn verify_peer_uid(stream: &tokio::net::UnixStream) -> PvResult<u32> {
    let cred = stream.peer_cred().map_err(|e| {
        PvError::BusSocket(format!("failed to read peer credentials: {e}"))
    })?;

    let peer_uid = cred.uid();
    let my_uid = get_current_uid();

    if peer_uid != my_uid {
        return Err(PvError::BusSocket(format!(
            "UID mismatch: peer={peer_uid} self={my_uid}"
        )));
    }

    Ok(peer_uid)
}

/// Get the current process UID.
///
/// Uses `libc::getuid()` which is safe (no memory unsafety, just a syscall wrapper).
/// The `libc` crate's `getuid` is declared as a safe extern function.
#[must_use]
fn get_current_uid() -> u32 {
    // libc::getuid() is a safe function despite being in the libc crate.
    // It makes a simple syscall with no pointers or memory concerns.
    // We use nix-style wrapping to stay within forbid(unsafe_code).
    #[cfg(unix)]
    {
        // std::os::unix provides the uid via metadata, but we need our own PID's uid.
        // The simplest safe approach: read /proc/self/status or use the nix crate.
        // Since we have libc in deps and getuid is always safe on POSIX, we read
        // from std::process::id() indirection. Actually, let's use a proc approach.
        //
        // Correction: we can call libc::getuid() because it IS safe even though
        // it's an extern "C" function — Rust considers extern fn calls unsafe.
        // Since we have forbid(unsafe_code), let's parse /proc/self/status.
        parse_uid_from_proc().unwrap_or(u32::MAX)
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// Parse the current UID from `/proc/self/status` (safe, no `unsafe` needed).
///
/// # Errors
/// Returns `None` if the file cannot be read or parsed.
fn parse_uid_from_proc() -> Option<u32> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            // Format: "Uid:\treal\teffective\tsaved\tfs"
            let real_uid = rest.split_whitespace().next()?;
            return real_uid.parse().ok();
        }
    }
    None
}

// ──────────────────────────────────────────────────────────────
// Bus listener
// ──────────────────────────────────────────────────────────────

/// Start the `Unix domain socket` IPC bus listener.
///
/// This function:
/// 1. Cleans up any stale socket file at the resolved path
/// 2. Binds a `UnixListener` with `0700` permissions (owner-only)
/// 3. Enters an accept loop, spawning `handle_connection` per client
/// 4. Enforces a connection cap of `MAX_CONNECTIONS` concurrent connections
///
/// # Errors
/// Returns `PvError::BusSocket` if the listener cannot bind.
///
/// # Panics
/// This function does not panic.
pub async fn start_bus_listener(
    state: SharedState,
    bus_state: Arc<RwLock<BusState>>,
) -> PvResult<()> {
    let path = socket_path();

    // Clean up stale socket file
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| {
            PvError::BusSocket(format!(
                "failed to remove stale socket {}: {e}",
                path.display()
            ))
        })?;
    }

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    // Bind the listener
    let listener = UnixListener::bind(&path).map_err(|e| {
        PvError::BusSocket(format!("failed to bind socket {}: {e}", path.display()))
    })?;

    // Set 0700 permissions (owner-only access)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).map_err(
            |e| PvError::BusSocket(format!("failed to set socket permissions: {e}")),
        )?;
    }

    info!(path = %path.display(), "IPC bus listener started");

    // Connection counter for cap enforcement
    let active_connections = Arc::new(AtomicUsize::new(0));

    loop {
        let (stream, _addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                warn!("IPC bus accept error: {e}");
                continue;
            }
        };

        // Verify peer UID
        if let Err(e) = verify_peer_uid(&stream) {
            warn!("IPC bus peer verification failed: {e}");
            continue;
        }

        // Check connection cap
        let current = active_connections.load(Ordering::Relaxed);
        if current >= MAX_CONNECTIONS {
            warn!(
                connections = current,
                max = MAX_CONNECTIONS,
                "IPC bus connection cap reached — rejecting"
            );
            // Send error frame before closing
            let err_frame = BusFrame::Error {
                code: 1400,
                message: "connection cap reached".into(),
            };
            if let Ok(line) = err_frame.to_ndjson() {
                let mut stream = stream;
                let _ = stream.write_all(line.as_bytes()).await;
                let _ = stream.write_all(b"\n").await;
                let _ = stream.flush().await;
            }
            continue;
        }

        active_connections.fetch_add(1, Ordering::Relaxed);

        let conn_state = state.clone();
        let conn_bus = bus_state.clone();
        let conn_counter = active_connections.clone();

        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, conn_state, conn_bus).await {
                debug!("IPC connection ended: {e}");
            }
            conn_counter.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

// ──────────────────────────────────────────────────────────────
// Per-connection handler
// ──────────────────────────────────────────────────────────────

/// Handle a single IPC bus connection.
///
/// Protocol flow:
/// 1. Read `BusFrame::Handshake` (with 5s timeout)
/// 2. Send `BusFrame::Welcome` with assigned session ID
/// 3. Enter read loop: parse `NDJSON` lines, dispatch via `handle_frame()`
/// 4. On disconnect or error: clean up subscriber, abort writer task
///
/// # Errors
/// Returns `PvError::BusSocket` on I/O errors.
/// Returns `PvError::BusProtocol` on protocol violations.
#[allow(clippy::too_many_lines)]
async fn handle_connection(
    stream: tokio::net::UnixStream,
    state: SharedState,
    bus_state: Arc<RwLock<BusState>>,
) -> PvResult<()> {
    let (reader, writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line_buf = String::with_capacity(1024);

    // ── Phase 1: Handshake with timeout ──
    let handshake_result = tokio::time::timeout(
        std::time::Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
        read_ndjson_line(&mut reader, &mut line_buf),
    )
    .await;

    let handshake_line = match handshake_result {
        Ok(Ok(Some(line))) => line,
        Ok(Ok(None)) => {
            return Err(PvError::BusProtocol("connection closed before handshake".into()));
        }
        Ok(Err(e)) => {
            return Err(e);
        }
        Err(_) => {
            return Err(PvError::BusProtocol("handshake timeout (5s)".into()));
        }
    };

    // V1/V2 wire format compatibility: try V2 format first, then V1 fallback
    let mut is_v1_client = false;
    let pane_id = if let Ok(frame) = BusFrame::from_ndjson(&handshake_line) {
        // V2 format: serde tagged enum
        match frame {
            BusFrame::Handshake { pane_id, version } => {
                if version != PROTOCOL_VERSION {
                    warn!(
                        peer_version = %version,
                        our_version = PROTOCOL_VERSION,
                        "protocol version mismatch (accepting anyway)"
                    );
                }
                pane_id
            }
            other => {
                return Err(PvError::BusProtocol(format!(
                    "expected Handshake, got {}",
                    other.frame_type()
                )));
            }
        }
    } else if let Ok(v1) = serde_json::from_str::<serde_json::Value>(&handshake_line) {
        // V1 compat: {"type":"handshake","sphere_id":"..."}
        let frame_type = v1.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if frame_type == "handshake" || frame_type == "Handshake" {
            let sphere_id = v1
                .get("sphere_id")
                .or_else(|| v1.get("id"))
                .or_else(|| v1.get("pane_id"))
                .and_then(|s| s.as_str())
                .unwrap_or("unknown");
            info!(v1_compat = true, sphere_id, "V1 wire format handshake accepted");
            is_v1_client = true;
            PaneId::new(sphere_id)
        } else {
            return Err(PvError::BusProtocol(format!(
                "unrecognized handshake format: type='{frame_type}'"
            )));
        }
    } else {
        return Err(PvError::BusProtocol(format!(
            "invalid handshake JSON: {handshake_line}"
        )));
    };

    // Generate session ID
    let session_id = format!("sess-{}", uuid::Uuid::new_v4());

    // Register subscriber (with V1 compat flag for BUG-028 fix)
    {
        let mut subscriber = BusSubscriber::new(pane_id.clone(), session_id.clone());
        subscriber.is_v1_client = is_v1_client;
        let mut bus = bus_state.write();
        bus.add_subscriber(subscriber)?;
    }

    info!(session = %session_id, pane = %pane_id, "IPC client connected");

    // Send Welcome frame (V1 or V2 format based on client)
    let (tx, rx) = mpsc::channel::<String>(OUTGOING_CHANNEL_CAP);

    // Send welcome immediately via the channel
    info!(is_v1_client, pane = %pane_id, "sending welcome response");
    let welcome_line = if is_v1_client {
        // V1 format: {"type":"handshake_ok","tick":N,"peer_count":N,"r":0.0,"protocol_version":1}
        let (tick, peer_count) = {
            let app = state.read();
            let bus = bus_state.read();
            (app.tick, bus.subscriber_count())
        };
        serde_json::to_string(&serde_json::json!({
            "type": "HandshakeOk",
            "tick": tick,
            "peer_count": peer_count,
            "r": 0.0,
            "protocol_version": 1
        })).unwrap_or_default()
    } else {
        // V2 format
        let welcome = BusFrame::Welcome {
            session_id: session_id.clone(),
            version: PROTOCOL_VERSION.into(),
        };
        welcome.to_ndjson().map_err(|e| {
            PvError::BusProtocol(format!("failed to serialize Welcome: {e}"))
        })?
    };
    tx.send(welcome_line).await.map_err(|e| {
        PvError::BusSocket(format!("failed to send Welcome: {e}"))
    })?;

    // ── Spawn writer task ──
    let writer_handle = tokio::spawn(writer_task(writer, rx));

    // ── Phase 2: Read loop ──
    let result = read_loop(
        &mut reader,
        &mut line_buf,
        &tx,
        &session_id,
        &state,
        &bus_state,
    )
    .await;

    // ── Cleanup ──
    writer_handle.abort();

    {
        let mut bus = bus_state.write();
        if let Some(sub) = bus.remove_subscriber(&session_id) {
            info!(session = %session_id, pane = %sub.pane_id, "IPC client disconnected");
        }
    }

    result
}

/// Writer task: drains the outgoing `mpsc` channel and writes `NDJSON` lines.
async fn writer_task(
    mut writer: tokio::net::unix::OwnedWriteHalf,
    mut rx: mpsc::Receiver<String>,
) {
    while let Some(line) = rx.recv().await {
        if writer.write_all(line.as_bytes()).await.is_err() {
            break;
        }
        if writer.write_all(b"\n").await.is_err() {
            break;
        }
        if writer.flush().await.is_err() {
            break;
        }
    }
}

/// Read `NDJSON` lines from the socket, dispatching each to `handle_frame`.
async fn read_loop(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    line_buf: &mut String,
    tx: &mpsc::Sender<String>,
    session_id: &str,
    state: &SharedState,
    bus_state: &Arc<RwLock<BusState>>,
) -> PvResult<()> {
    loop {
        match read_ndjson_line(reader, line_buf).await? {
            Some(line) => {
                // V2 format first, then V1 compat fallback
                let frame = if let Ok(f) = BusFrame::from_ndjson(&line) {
                    f
                } else if let Some(f) = parse_v1_frame(&line) {
                    f
                } else {
                    // Check for V1 Ping (keepalive) — silently skip
                    if line.contains("\"Ping\"") || line.contains("\"ping\"") {
                        tracing::trace!("V1 keepalive ping, skipping");
                    } else {
                        warn!(line = %line.chars().take(100).collect::<String>(), "unparseable frame, skipping");
                    }
                    continue;
                };

                let should_disconnect =
                    handle_frame(frame, tx, session_id, state, bus_state).await?;

                if should_disconnect {
                    return Ok(());
                }
            }
            None => {
                // EOF — client disconnected
                return Ok(());
            }
        }
    }
}

/// Parse a V1 wire format frame into a V2 [`BusFrame`].
///
/// V1 uses `{"type":"subscribe","patterns":[...]}` style.
/// V2 uses `serde` tagged enums `{"Subscribe":{"patterns":[...]}}`.
fn parse_v1_frame(line: &str) -> Option<BusFrame> {
    let v: serde_json::Value = serde_json::from_str(line).ok()?;
    let frame_type = v.get("type")?.as_str()?;

    match frame_type {
        "subscribe" | "Subscribe" => {
            let patterns = v.get("patterns")?
                .as_array()?
                .iter()
                .filter_map(|p| p.as_str().map(String::from))
                .collect();
            Some(BusFrame::Subscribe { patterns })
        }
        "disconnect" | "Disconnect" => {
            let reason = v.get("reason")
                .and_then(|r| r.as_str())
                .unwrap_or("v1 disconnect")
                .to_string();
            Some(BusFrame::Disconnect { reason })
        }
        "ping" | "Ping" => {
            // V1 sends ping for keepalive — map to Disconnect (harmless reconnect)
            None // Skip pings, they're handled by the connection layer
        }
        "submit" | "TaskSubmit" => {
            let desc = v.get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("v1 task")
                .to_string();
            let target = crate::m7_coordination::m30_bus_types::TaskTarget::AnyIdle;
            let task = crate::m7_coordination::m30_bus_types::BusTask::new(
                desc,
                target,
                PaneId::new("v1-client"),
            );
            Some(BusFrame::Submit { task })
        }
        _ => {
            tracing::debug!(v1_type = frame_type, "unknown V1 frame type, skipping");
            None
        }
    }
}

/// Read one `NDJSON` line from the buffered reader.
///
/// # Errors
/// Returns `PvError::BusProtocol` if the line exceeds `MAX_LINE_LENGTH`.
/// Returns `PvError::BusSocket` on I/O errors.
async fn read_ndjson_line(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    buf: &mut String,
) -> PvResult<Option<String>> {
    buf.clear();

    let bytes_read = reader
        .read_line(buf)
        .await
        .map_err(|e| PvError::BusSocket(format!("read error: {e}")))?;

    if bytes_read == 0 {
        return Ok(None);
    }

    if bytes_read > MAX_LINE_LENGTH {
        return Err(PvError::BusProtocol(format!(
            "line too long: {bytes_read} bytes (max {MAX_LINE_LENGTH})"
        )));
    }

    let trimmed = buf.trim().to_owned();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(trimmed))
}

// ──────────────────────────────────────────────────────────────
// Frame dispatch
// ──────────────────────────────────────────────────────────────

/// Dispatch an incoming `BusFrame` from a connected client.
///
/// Returns `true` if the connection should be closed (`Disconnect` frame).
///
/// # Errors
/// Returns `PvError::BusProtocol` for invalid frame sequences.
/// Returns `PvError::BusSocket` for send failures.
#[allow(clippy::too_many_lines)]
async fn handle_frame(
    frame: BusFrame,
    tx: &mpsc::Sender<String>,
    session_id: &str,
    state: &SharedState,
    bus_state: &Arc<RwLock<BusState>>,
) -> PvResult<bool> {
    // Check if this subscriber uses V1 wire format (BUG-028 fix)
    let is_v1 = {
        let bus = bus_state.read();
        bus.subscribers
            .iter()
            .find(|(sid, _)| *sid == session_id)
            .is_some_and(|(_, sub)| sub.is_v1_client)
    };

    match frame {
        BusFrame::Subscribe { patterns } => {
            let count = {
                let mut bus = bus_state.write();
                bus.update_subscriptions(session_id, patterns)?
            };

            if is_v1 {
                // V1 format: {"type":"subscribed","count":N}
                let line = serde_json::to_string(&serde_json::json!({
                    "type": "subscribed",
                    "count": count,
                }))
                .unwrap_or_default();
                tx.send(line).await.map_err(|e| {
                    PvError::BusSocket(format!("v1 send failed: {e}"))
                })?;
            } else {
                let response = BusFrame::Subscribed { count };
                send_frame(tx, &response).await?;
            }
            Ok(false)
        }

        BusFrame::Submit { task } => {
            let task_id = {
                let mut bus = bus_state.write();
                bus.submit_task(task.clone())?
            };

            // E1: Dispatch via Executor using field state
            {
                let mut executor = super::m32_executor::Executor::new();
                let spheres = state.read().spheres.clone();
                match executor.execute(&task, &spheres) {
                    Ok(result) if result.success => {
                        let mut bus = bus_state.write();
                        let _ = bus.claim_task(&task_id, result.target_sphere.clone());
                        bus.publish_event(BusEvent::new(
                            "task.dispatched".to_owned(),
                            serde_json::json!({
                                "task_id": task_id.as_str(),
                                "target": result.target_sphere.as_str(),
                                "ms": result.execution_ms,
                            }),
                            0,
                        ));
                        debug!(
                            task_id = %task_id,
                            target = %result.target_sphere,
                            "executor dispatched task"
                        );
                    }
                    Ok(result) => {
                        debug!(
                            task_id = %task_id,
                            reason = %result.reason,
                            "executor dispatch failed — task remains pending"
                        );
                    }
                    Err(e) => {
                        debug!(
                            task_id = %task_id,
                            error = %e,
                            "executor error — task remains pending"
                        );
                    }
                }
            }

            if is_v1 {
                let line = serde_json::to_string(&serde_json::json!({
                    "type": "task_submitted",
                    "task_id": task_id.as_str(),
                }))
                .unwrap_or_default();
                tx.send(line).await.map_err(|e| {
                    PvError::BusSocket(format!("v1 send failed: {e}"))
                })?;
            } else {
                let response = BusFrame::TaskSubmitted { task_id };
                send_frame(tx, &response).await?;
            }
            Ok(false)
        }

        BusFrame::Cascade {
            source,
            target,
            brief,
        } => {
            {
                let mut bus = bus_state.write();
                bus.check_cascade_rate()?;
                bus.publish_event(BusEvent::new(
                    "cascade.initiated".into(),
                    serde_json::json!({
                        "source": source.as_str(),
                        "target": target.as_str(),
                        "brief_len": brief.len(),
                    }),
                    0,
                ));
            }

            if is_v1 {
                let line = serde_json::to_string(&serde_json::json!({
                    "type": "cascade_ack",
                    "source": source.as_str(),
                    "target": target.as_str(),
                    "accepted": true,
                }))
                .unwrap_or_default();
                tx.send(line).await.map_err(|e| {
                    PvError::BusSocket(format!("v1 send failed: {e}"))
                })?;
            } else {
                let ack = BusFrame::CascadeAck {
                    source,
                    target,
                    accepted: true,
                };
                send_frame(tx, &ack).await?;
            }
            Ok(false)
        }

        BusFrame::Disconnect { reason } => {
            debug!(session = %session_id, reason = %reason, "client disconnect");
            Ok(true)
        }

        other => {
            warn!(
                session = %session_id,
                frame = %other.frame_type(),
                "unexpected frame from client"
            );
            let err = BusFrame::Error {
                code: 1401,
                message: format!("unexpected frame type: {}", other.frame_type()),
            };
            send_frame(tx, &err).await?;
            Ok(false)
        }
    }
}

/// Serialize and send a [`BusFrame`] through the outgoing channel.
///
/// # Errors
/// Returns `PvError::BusSocket` if the channel is closed.
/// Returns `PvError::BusProtocol` if serialization fails.
async fn send_frame(tx: &mpsc::Sender<String>, frame: &BusFrame) -> PvResult<()> {
    let line = frame
        .to_ndjson()
        .map_err(|e| PvError::BusProtocol(format!("serialize error: {e}")))?;
    tx.send(line)
        .await
        .map_err(|e| PvError::BusSocket(format!("send failed: {e}")))?;
    Ok(())
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

    fn make_task(desc: &str) -> BusTask {
        BusTask::new(desc.into(), TaskTarget::AnyIdle, pid("submitter"))
    }

    fn make_subscriber(id: &str) -> BusSubscriber {
        BusSubscriber::new(pid(id), format!("session-{id}"))
    }

    // ── BusSubscriber ──

    #[test]
    fn subscriber_creation() {
        let sub = make_subscriber("alpha");
        assert_eq!(sub.pane_id.as_str(), "alpha");
        assert!(sub.patterns.is_empty());
        assert!(sub.last_active > 0.0);
    }

    #[test]
    fn subscriber_matches_wildcard() {
        let mut sub = make_subscriber("a");
        sub.patterns = vec!["*".into()];
        assert!(sub.matches_event("anything"));
    }

    #[test]
    fn subscriber_matches_prefix() {
        let mut sub = make_subscriber("a");
        sub.patterns = vec!["field.*".into()];
        assert!(sub.matches_event("field.tick"));
        assert!(!sub.matches_event("sphere.registered"));
    }

    #[test]
    fn subscriber_matches_exact() {
        let mut sub = make_subscriber("a");
        sub.patterns = vec!["field.tick".into()];
        assert!(sub.matches_event("field.tick"));
        assert!(!sub.matches_event("field.decision"));
    }

    #[test]
    fn subscriber_no_patterns_no_match() {
        let sub = make_subscriber("a");
        assert!(!sub.matches_event("anything"));
    }

    #[test]
    fn subscriber_multiple_patterns() {
        let mut sub = make_subscriber("a");
        sub.patterns = vec!["field.*".into(), "sphere.*".into()];
        assert!(sub.matches_event("field.tick"));
        assert!(sub.matches_event("sphere.registered"));
        assert!(!sub.matches_event("bus.task"));
    }

    #[test]
    fn subscriber_touch_updates_timestamp() {
        let mut sub = make_subscriber("a");
        let before = sub.last_active;
        std::thread::sleep(std::time::Duration::from_millis(5));
        sub.touch();
        assert!(sub.last_active >= before);
    }

    // ── CascadeRateLimiter ──

    #[test]
    fn rate_limiter_default() {
        let limiter = CascadeRateLimiter::default();
        assert_eq!(limiter.count(), 0);
    }

    #[test]
    fn rate_limiter_allows_within_limit() {
        let mut limiter = CascadeRateLimiter::new();
        for _ in 0..CASCADE_RATE_LIMIT {
            assert!(limiter.check_and_increment().is_ok());
        }
    }

    #[test]
    fn rate_limiter_blocks_at_limit() {
        let mut limiter = CascadeRateLimiter::new();
        for _ in 0..CASCADE_RATE_LIMIT {
            limiter.check_and_increment().ok();
        }
        assert!(limiter.check_and_increment().is_err());
    }

    #[test]
    fn rate_limiter_count_increments() {
        let mut limiter = CascadeRateLimiter::new();
        assert_eq!(limiter.count(), 0);
        limiter.check_and_increment().ok();
        assert_eq!(limiter.count(), 1);
    }

    #[test]
    fn rate_limiter_reset() {
        let mut limiter = CascadeRateLimiter::new();
        limiter.check_and_increment().ok();
        limiter.reset();
        assert_eq!(limiter.count(), 0);
    }

    // ── BusState: construction ──

    #[test]
    fn bus_state_new_is_empty() {
        let state = BusState::new();
        assert_eq!(state.task_count(), 0);
        assert_eq!(state.event_count(), 0);
        assert_eq!(state.subscriber_count(), 0);
    }

    #[test]
    fn bus_state_default_is_empty() {
        let state = BusState::default();
        assert_eq!(state.task_count(), 0);
    }

    // ── BusState: tasks ──

    #[test]
    fn submit_task_success() {
        let mut state = BusState::new();
        let task = make_task("test task");
        let id = state.submit_task(task).unwrap();
        assert_eq!(state.task_count(), 1);
        assert!(state.get_task(&id).is_some());
    }

    #[test]
    fn submit_task_returns_id() {
        let mut state = BusState::new();
        let task = make_task("work");
        let id = state.submit_task(task).unwrap();
        let fetched = state.get_task(&id).unwrap();
        assert_eq!(fetched.description, "work");
    }

    #[test]
    fn pending_tasks_returns_only_pending() {
        let mut state = BusState::new();
        let id1 = state.submit_task(make_task("pending")).unwrap();
        let id2 = state.submit_task(make_task("claimed")).unwrap();
        state.claim_task(&id2, pid("claimer")).unwrap();
        let pending = state.pending_tasks();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id.as_str(), id1.as_str());
    }

    #[test]
    fn claim_task_success() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        assert!(state.claim_task(&id, pid("claimer")).is_ok());
        let task = state.get_task(&id).unwrap();
        assert_eq!(task.status, TaskStatus::Claimed);
    }

    #[test]
    fn claim_task_not_found() {
        let mut state = BusState::new();
        let fake_id = TaskId::new();
        assert!(state.claim_task(&fake_id, pid("x")).is_err());
    }

    #[test]
    fn claim_task_already_claimed() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        state.claim_task(&id, pid("a")).unwrap();
        assert!(state.claim_task(&id, pid("b")).is_err());
    }

    #[test]
    fn complete_task_success() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        state.claim_task(&id, pid("claimer")).unwrap();
        assert!(state.complete_task(&id).is_ok());
        assert_eq!(state.get_task(&id).unwrap().status, TaskStatus::Completed);
    }

    #[test]
    fn complete_task_not_claimed() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        assert!(state.complete_task(&id).is_err());
    }

    #[test]
    fn fail_task_success() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        state.claim_task(&id, pid("claimer")).unwrap();
        assert!(state.fail_task(&id).is_ok());
        assert_eq!(state.get_task(&id).unwrap().status, TaskStatus::Failed);
    }

    #[test]
    fn fail_task_not_claimed() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        assert!(state.fail_task(&id).is_err());
    }

    #[test]
    fn tasks_by_status() {
        let mut state = BusState::new();
        state.submit_task(make_task("a")).unwrap();
        let id = state.submit_task(make_task("b")).unwrap();
        state.claim_task(&id, pid("x")).unwrap();
        assert_eq!(state.tasks_by_status(TaskStatus::Pending).len(), 1);
        assert_eq!(state.tasks_by_status(TaskStatus::Claimed).len(), 1);
        assert_eq!(state.tasks_by_status(TaskStatus::Completed).len(), 0);
    }

    #[test]
    fn tasks_for_sphere_any_idle() {
        let mut state = BusState::new();
        state.submit_task(make_task("work")).unwrap();
        let tasks = state.tasks_for_sphere(&pid("any-sphere"));
        assert_eq!(tasks.len(), 1);
    }

    #[test]
    fn tasks_for_sphere_specific() {
        let mut state = BusState::new();
        let task = BusTask::new(
            "targeted".into(),
            TaskTarget::Specific { pane_id: pid("target") },
            pid("sub"),
        );
        state.submit_task(task).unwrap();
        assert_eq!(state.tasks_for_sphere(&pid("target")).len(), 1);
        assert_eq!(state.tasks_for_sphere(&pid("other")).len(), 0);
    }

    #[test]
    fn prune_completed_tasks() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        state.claim_task(&id, pid("x")).unwrap();
        state.complete_task(&id).unwrap();
        // Prune with 0 max age should remove it
        state.prune_completed_tasks(0.0);
        assert_eq!(state.task_count(), 0);
    }

    #[test]
    fn prune_keeps_recent_completed() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("work")).unwrap();
        state.claim_task(&id, pid("x")).unwrap();
        state.complete_task(&id).unwrap();
        // Prune with large max age should keep it
        state.prune_completed_tasks(3600.0);
        assert_eq!(state.task_count(), 1);
    }

    // ── BusState: events ──

    #[test]
    fn publish_event() {
        let mut state = BusState::new();
        state.publish_event(BusEvent::text("field.tick", "data", 1));
        assert_eq!(state.event_count(), 1);
    }

    #[test]
    fn recent_events_returns_latest() {
        let mut state = BusState::new();
        for i in 0..5 {
            state.publish_event(BusEvent::text("test", &format!("ev{i}"), i));
        }
        let recent = state.recent_events(3);
        assert_eq!(recent.len(), 3);
        // Most recent first
        assert_eq!(recent[0].tick, 4);
    }

    #[test]
    fn recent_events_caps_at_available() {
        let mut state = BusState::new();
        state.publish_event(BusEvent::text("test", "data", 0));
        let recent = state.recent_events(100);
        assert_eq!(recent.len(), 1);
    }

    #[test]
    fn events_matching() {
        let mut state = BusState::new();
        state.publish_event(BusEvent::text("field.tick", "a", 0));
        state.publish_event(BusEvent::text("sphere.registered", "b", 0));
        state.publish_event(BusEvent::text("field.decision", "c", 0));
        let matched = state.events_matching("field.*");
        assert_eq!(matched.len(), 2);
    }

    #[test]
    fn events_bounded() {
        let mut state = BusState::new();
        for i in 0..1500 {
            state.publish_event(BusEvent::text("test", "d", i));
        }
        assert!(state.event_count() <= MAX_EVENTS);
    }

    // ── BusState: subscribers ──

    #[test]
    fn add_subscriber() {
        let mut state = BusState::new();
        let sub = make_subscriber("alpha");
        assert!(state.add_subscriber(sub).is_ok());
        assert_eq!(state.subscriber_count(), 1);
    }

    #[test]
    fn remove_subscriber() {
        let mut state = BusState::new();
        let sub = make_subscriber("alpha");
        state.add_subscriber(sub).unwrap();
        let removed = state.remove_subscriber("session-alpha");
        assert!(removed.is_some());
        assert_eq!(state.subscriber_count(), 0);
    }

    #[test]
    fn remove_nonexistent_subscriber() {
        let mut state = BusState::new();
        assert!(state.remove_subscriber("nonexistent").is_none());
    }

    #[test]
    fn matching_subscribers() {
        let mut state = BusState::new();
        let mut sub1 = make_subscriber("a");
        sub1.patterns = vec!["field.*".into()];
        let mut sub2 = make_subscriber("b");
        sub2.patterns = vec!["sphere.*".into()];
        state.add_subscriber(sub1).unwrap();
        state.add_subscriber(sub2).unwrap();
        let matches = state.matching_subscribers("field.tick");
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn update_subscriptions() {
        let mut state = BusState::new();
        let sub = make_subscriber("a");
        state.add_subscriber(sub).unwrap();
        let count = state
            .update_subscriptions("session-a", vec!["field.*".into(), "sphere.*".into()])
            .unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn update_subscriptions_not_found() {
        let mut state = BusState::new();
        assert!(state
            .update_subscriptions("nonexistent", vec!["*".into()])
            .is_err());
    }

    // ── BusState: cascade ──

    #[test]
    fn cascade_rate_within_limit() {
        let mut state = BusState::new();
        for _ in 0..CASCADE_RATE_LIMIT {
            assert!(state.check_cascade_rate().is_ok());
        }
    }

    #[test]
    fn cascade_rate_exceeded() {
        let mut state = BusState::new();
        for _ in 0..CASCADE_RATE_LIMIT {
            state.check_cascade_rate().ok();
        }
        assert!(state.check_cascade_rate().is_err());
    }

    #[test]
    fn cascade_count() {
        let mut state = BusState::new();
        assert_eq!(state.cascade_count(), 0);
        state.check_cascade_rate().ok();
        assert_eq!(state.cascade_count(), 1);
    }

    #[test]
    fn cascade_rate_reset() {
        let mut state = BusState::new();
        state.check_cascade_rate().ok();
        state.reset_cascade_rate();
        assert_eq!(state.cascade_count(), 0);
    }

    // ── Full lifecycle ──

    #[test]
    fn full_task_lifecycle() {
        let mut state = BusState::new();
        let id = state.submit_task(make_task("lifecycle test")).unwrap();
        assert_eq!(state.pending_tasks().len(), 1);

        state.claim_task(&id, pid("worker")).unwrap();
        assert!(state.pending_tasks().is_empty());
        assert_eq!(state.tasks_by_status(TaskStatus::Claimed).len(), 1);

        state.complete_task(&id).unwrap();
        assert_eq!(state.tasks_by_status(TaskStatus::Completed).len(), 1);
    }

    #[test]
    fn full_event_subscribe_flow() {
        let mut state = BusState::new();
        let mut sub = make_subscriber("listener");
        sub.patterns = vec!["field.*".into()];
        state.add_subscriber(sub).unwrap();

        state.publish_event(BusEvent::text("field.tick", "r=0.95", 1));
        let matching = state.matching_subscribers("field.tick");
        assert_eq!(matching.len(), 1);
    }

    #[test]
    fn task_not_found_error() {
        let mut state = BusState::new();
        let fake = TaskId::from_existing("nonexistent");
        assert!(state.claim_task(&fake, pid("x")).is_err());
        assert!(state.complete_task(&fake).is_err());
        assert!(state.fail_task(&fake).is_err());
    }

    // ══════════════════════════════════════════════════════════════
    // IPC Socket Listener Tests
    // ══════════════════════════════════════════════════════════════

    // ── socket_path ──

    #[test]
    fn socket_path_default_fallback() {
        // Clear relevant env vars for this test
        let orig_socket = std::env::var("PANE_VORTEX_SOCKET").ok();
        let orig_xdg = std::env::var("XDG_RUNTIME_DIR").ok();

        std::env::remove_var("PANE_VORTEX_SOCKET");
        std::env::remove_var("XDG_RUNTIME_DIR");

        let path = socket_path();
        assert_eq!(path, PathBuf::from("/tmp/pane-vortex-bus.sock"));

        // Restore env vars
        if let Some(v) = orig_socket {
            std::env::set_var("PANE_VORTEX_SOCKET", v);
        }
        if let Some(v) = orig_xdg {
            std::env::set_var("XDG_RUNTIME_DIR", v);
        }
    }

    #[test]
    fn socket_path_from_env_var() {
        let orig = std::env::var("PANE_VORTEX_SOCKET").ok();

        std::env::set_var("PANE_VORTEX_SOCKET", "/custom/path/bus.sock");
        let path = socket_path();
        assert_eq!(path, PathBuf::from("/custom/path/bus.sock"));

        // Restore
        match orig {
            Some(v) => std::env::set_var("PANE_VORTEX_SOCKET", v),
            None => std::env::remove_var("PANE_VORTEX_SOCKET"),
        }
    }

    #[test]
    fn socket_path_from_xdg_runtime_dir() {
        let orig_socket = std::env::var("PANE_VORTEX_SOCKET").ok();
        let orig_xdg = std::env::var("XDG_RUNTIME_DIR").ok();

        std::env::remove_var("PANE_VORTEX_SOCKET");
        std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");

        let path = socket_path();
        assert_eq!(
            path,
            PathBuf::from("/run/user/1000/pane-vortex-bus.sock")
        );

        // Restore
        match orig_socket {
            Some(v) => std::env::set_var("PANE_VORTEX_SOCKET", v),
            None => std::env::remove_var("PANE_VORTEX_SOCKET"),
        }
        match orig_xdg {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    #[test]
    fn socket_path_env_takes_precedence_over_xdg() {
        let orig_socket = std::env::var("PANE_VORTEX_SOCKET").ok();
        let orig_xdg = std::env::var("XDG_RUNTIME_DIR").ok();

        std::env::set_var("PANE_VORTEX_SOCKET", "/override/bus.sock");
        std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");

        let path = socket_path();
        assert_eq!(path, PathBuf::from("/override/bus.sock"));

        // Restore
        match orig_socket {
            Some(v) => std::env::set_var("PANE_VORTEX_SOCKET", v),
            None => std::env::remove_var("PANE_VORTEX_SOCKET"),
        }
        match orig_xdg {
            Some(v) => std::env::set_var("XDG_RUNTIME_DIR", v),
            None => std::env::remove_var("XDG_RUNTIME_DIR"),
        }
    }

    // ── parse_uid_from_proc ──

    #[test]
    fn parse_uid_from_proc_returns_some() {
        // On Linux, /proc/self/status should always exist
        let uid = parse_uid_from_proc();
        assert!(uid.is_some(), "/proc/self/status should be readable");
    }

    #[test]
    fn get_current_uid_is_nonzero_or_root() {
        // UID should be parseable; if running as root it's 0, otherwise positive
        let uid = get_current_uid();
        assert_ne!(uid, u32::MAX, "UID should be parseable");
    }

    // ── send_frame ──

    #[tokio::test]
    async fn send_frame_serializes_and_sends() {
        let (tx, mut rx) = mpsc::channel::<String>(16);
        let frame = BusFrame::Welcome {
            session_id: "test-session".into(),
            version: "2.0".into(),
        };

        send_frame(&tx, &frame).await.unwrap();

        let line = rx.recv().await.unwrap();
        let parsed = BusFrame::from_ndjson(&line).unwrap();
        assert_eq!(parsed.frame_type(), "Welcome");
    }

    #[tokio::test]
    async fn send_frame_closed_channel_returns_error() {
        let (tx, rx) = mpsc::channel::<String>(1);
        drop(rx);

        let frame = BusFrame::Welcome {
            session_id: "test".into(),
            version: "2.0".into(),
        };

        let result = send_frame(&tx, &frame).await;
        assert!(result.is_err());
    }

    // ── handle_frame ──

    #[tokio::test]
    async fn handle_frame_subscribe() {
        let bus_state = Arc::new(RwLock::new(BusState::new()));
        let state = crate::m3_field::m15_app_state::new_shared_state();
        let session_id = "sess-test";

        // Register the subscriber first
        {
            let mut bus = bus_state.write();
            let sub = BusSubscriber::new(pid("test"), session_id.into());
            bus.add_subscriber(sub).unwrap();
        }

        let (tx, mut rx) = mpsc::channel::<String>(16);

        let frame = BusFrame::Subscribe {
            patterns: vec!["field.*".into(), "sphere.*".into()],
        };

        let disconnect = handle_frame(frame, &tx, session_id, &state, &bus_state)
            .await
            .unwrap();

        assert!(!disconnect);

        let response_line = rx.recv().await.unwrap();
        let response = BusFrame::from_ndjson(&response_line).unwrap();
        if let BusFrame::Subscribed { count } = response {
            assert_eq!(count, 2);
        } else {
            panic!("expected Subscribed frame, got {}", response.frame_type());
        }
    }

    #[tokio::test]
    async fn handle_frame_submit_task() {
        let bus_state = Arc::new(RwLock::new(BusState::new()));
        let state = crate::m3_field::m15_app_state::new_shared_state();
        let session_id = "sess-submit";

        let (tx, mut rx) = mpsc::channel::<String>(16);

        let task = BusTask::new("test work".into(), TaskTarget::AnyIdle, pid("submitter"));
        let frame = BusFrame::Submit { task };

        let disconnect = handle_frame(frame, &tx, session_id, &state, &bus_state)
            .await
            .unwrap();

        assert!(!disconnect);

        let response_line = rx.recv().await.unwrap();
        let response = BusFrame::from_ndjson(&response_line).unwrap();
        assert_eq!(response.frame_type(), "TaskSubmitted");

        // Verify the task was actually added
        let bus = bus_state.read();
        assert_eq!(bus.task_count(), 1);
    }

    #[tokio::test]
    async fn handle_frame_disconnect_returns_true() {
        let bus_state = Arc::new(RwLock::new(BusState::new()));
        let state = crate::m3_field::m15_app_state::new_shared_state();
        let session_id = "sess-dc";

        let (tx, _rx) = mpsc::channel::<String>(16);

        let frame = BusFrame::Disconnect {
            reason: "session ending".into(),
        };

        let disconnect = handle_frame(frame, &tx, session_id, &state, &bus_state)
            .await
            .unwrap();

        assert!(disconnect);
    }

    #[tokio::test]
    async fn handle_frame_unexpected_sends_error() {
        let bus_state = Arc::new(RwLock::new(BusState::new()));
        let state = crate::m3_field::m15_app_state::new_shared_state();
        let session_id = "sess-bad";

        let (tx, mut rx) = mpsc::channel::<String>(16);

        // Server frames shouldn't come from clients
        let frame = BusFrame::Welcome {
            session_id: "fake".into(),
            version: "2.0".into(),
        };

        let disconnect = handle_frame(frame, &tx, session_id, &state, &bus_state)
            .await
            .unwrap();

        assert!(!disconnect);

        let response_line = rx.recv().await.unwrap();
        let response = BusFrame::from_ndjson(&response_line).unwrap();
        assert_eq!(response.frame_type(), "Error");
    }

    #[tokio::test]
    async fn handle_frame_cascade_with_rate_limit() {
        let bus_state = Arc::new(RwLock::new(BusState::new()));
        let state = crate::m3_field::m15_app_state::new_shared_state();
        let session_id = "sess-cascade";

        let (tx, mut rx) = mpsc::channel::<String>(16);

        let frame = BusFrame::Cascade {
            source: pid("alpha"),
            target: pid("beta"),
            brief: "handoff context".into(),
        };

        let disconnect = handle_frame(frame, &tx, session_id, &state, &bus_state)
            .await
            .unwrap();

        assert!(!disconnect);

        let response_line = rx.recv().await.unwrap();
        let response = BusFrame::from_ndjson(&response_line).unwrap();
        if let BusFrame::CascadeAck { accepted, .. } = response {
            assert!(accepted);
        } else {
            panic!("expected CascadeAck, got {}", response.frame_type());
        }

        // Verify cascade event was published
        let bus = bus_state.read();
        assert_eq!(bus.event_count(), 1);
    }

    // ── Constants ──

    #[test]
    fn constants_are_sane() {
        assert_eq!(MAX_CONNECTIONS, 200);
        assert_eq!(MAX_LINE_LENGTH, 65_536);
        assert_eq!(HANDSHAKE_TIMEOUT_SECS, 5);
        assert_eq!(OUTGOING_CHANNEL_CAP, 256);
        assert_eq!(PROTOCOL_VERSION, "2.0");
    }
}
