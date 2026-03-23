//! # M10: Hook Server
//!
//! Axum HTTP server on `:8133` handling Claude Code hook events.
//! This is the **keystone** of ORAC — replacing all bash hook scripts
//! with sub-millisecond HTTP endpoints.
//!
//! ## Layer: L3 (Hooks)
//! ## Module: M10
//! ## Dependencies: L1 (`m01_core_types`, `m02_error_handling`, `m03_config`)
//!
//! ## Endpoints
//!
//! | Route | Hook Event | Purpose |
//! |-------|-----------|---------|
//! | `GET /health` | — | Liveness probe |
//! | `POST /hooks/SessionStart` | `SessionStart` | Register sphere, hydrate |
//! | `POST /hooks/PostToolUse` | `PostToolUse` | Memory, status, task poll |
//! | `POST /hooks/PreToolUse` | `PreToolUse` | Thermal gate |
//! | `POST /hooks/UserPromptSubmit` | `UserPromptSubmit` | Field state injection |
//! | `POST /hooks/Stop` | `Stop` | Deregister, crystallize |
//! | `POST /hooks/PermissionRequest` | `PermissionRequest` | Auto-approve policy |
//!
//! ## Design
//!
//! - All handlers return `HookResponse` with optional `systemMessage`
//! - Fire-and-forget PV2 calls via `tokio::spawn` (non-blocking)
//! - `OracState` is `Arc`-wrapped, shared across all handlers
//! - `ureq` for synchronous HTTP to PV2/SYNTHEX/POVM/RM via `spawn_blocking`

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::m1_core::field_state::{self, FieldState, SharedState};
use crate::m1_core::m01_core_types::{PaneId, PaneSphere};
use crate::m1_core::m03_config::PvConfig;

#[cfg(feature = "evolution")]
use crate::m8_evolution::m36_ralph_engine::RalphEngine;
#[cfg(feature = "persistence")]
use crate::m5_bridges::m26_blackboard::Blackboard;
use crate::m4_intelligence::m15_coupling_network::CouplingNetwork;
#[cfg(feature = "intelligence")]
use crate::m4_intelligence::m21_circuit_breaker::{BreakerConfig, BreakerRegistry};

/// Maximum ghost traces retained (FIFO eviction beyond this).
const MAX_GHOSTS: usize = 20;

/// Current epoch time in milliseconds.
///
/// Falls back to 0 if the system clock is unavailable.
#[must_use]
pub fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| {
            u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
        })
}

/// Open the local blackboard database.
///
/// Uses `~/.local/share/orac/blackboard.db` (or `/tmp` fallback).
/// Returns `None` if the DB cannot be opened (non-fatal — hooks still work).
#[cfg(feature = "persistence")]
fn open_blackboard() -> Option<Mutex<Blackboard>> {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
    let orac_dir = dir.join("orac");
    let _ = std::fs::create_dir_all(&orac_dir);
    let path = orac_dir.join("blackboard.db");
    match Blackboard::open(&path.to_string_lossy()) {
        Ok(bb) => Some(Mutex::new(bb)),
        Err(e) => {
            tracing::warn!("blackboard open failed: {e}");
            None
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Consent types (FIX-018)
// ──────────────────────────────────────────────────────────────

/// Per-sphere consent declarations for ORAC bridge operations.
///
/// Controls which ORAC bridges may read/write data for this sphere.
/// Independent of PV2's coupling-level consent (Hebbian, modulation).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // consent fields are inherently boolean
pub struct OracConsent {
    /// Allow SYNTHEX bridge to write data for this sphere.
    pub synthex_write: bool,
    /// Allow POVM bridge to read data for this sphere.
    pub povm_read: bool,
    /// Allow POVM bridge to write data for this sphere.
    pub povm_write: bool,
    /// Allow session hydration from POVM + RM on `SessionStart`.
    pub hydration: bool,
    /// Epoch milliseconds when this consent was last updated.
    pub updated_ms: u64,
}

impl OracConsent {
    /// Create a fully-open consent (default for new spheres).
    #[must_use]
    pub fn fully_open() -> Self {
        Self {
            synthex_write: true,
            povm_read: true,
            povm_write: false,
            hydration: true,
            updated_ms: epoch_ms(),
        }
    }
}

/// Request body for `PUT /consent/{sphere_id}`.
#[derive(Debug, Clone, Deserialize)]
pub struct ConsentUpdateRequest {
    /// Update `synthex_write` consent.
    #[serde(default)]
    pub synthex_write: Option<bool>,
    /// Update `povm_read` consent.
    #[serde(default)]
    pub povm_read: Option<bool>,
    /// Update `povm_write` consent.
    #[serde(default)]
    pub povm_write: Option<bool>,
    /// Update `hydration` consent.
    #[serde(default)]
    pub hydration: Option<bool>,
}

// ──────────────────────────────────────────────────────────────
// Ghost types (FIX-019)
// ──────────────────────────────────────────────────────────────

/// Ghost trace of a deregistered sphere, tracked locally by ORAC.
///
/// Captured during `handle_stop` with timing data that PV2 doesn't expose
/// in its GET response (session duration, final phase).
#[derive(Debug, Clone, Serialize)]
pub struct OracGhost {
    /// Sphere ID at time of departure.
    pub sphere_id: String,
    /// Persona string (typically the working directory).
    pub persona: String,
    /// Epoch milliseconds when the sphere was deregistered.
    pub deregistered_ms: u64,
    /// Kuramoto phase at the moment of departure.
    pub final_phase: f64,
    /// Total number of tool calls during the session.
    pub total_tools: u64,
    /// Session wall-clock duration in milliseconds.
    pub session_duration_ms: u64,
}

// ──────────────────────────────────────────────────────────────
// Hook request types (from Claude Code)
// ──────────────────────────────────────────────────────────────

/// Generic hook event payload from Claude Code.
///
/// Claude Code sends different fields depending on the hook type.
/// All fields are optional — each handler extracts what it needs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookEvent {
    /// Session identifier (present in `SessionStart`, `Stop`).
    #[serde(default)]
    pub session_id: Option<String>,
    /// Tool name (present in `PostToolUse`, `PreToolUse`, `PermissionRequest`).
    #[serde(default)]
    pub tool_name: Option<String>,
    /// Tool input object (present in `PostToolUse`, `PreToolUse`, `PermissionRequest`).
    #[serde(default)]
    pub tool_input: Option<serde_json::Value>,
    /// Tool output string (present in `PostToolUse`).
    #[serde(default)]
    pub tool_output: Option<String>,
    /// User prompt text (present in `UserPromptSubmit`).
    #[serde(default)]
    pub prompt: Option<String>,
}

// ──────────────────────────────────────────────────────────────
// Hook response types (to Claude Code)
// ──────────────────────────────────────────────────────────────

/// Response returned to Claude Code from a hook endpoint.
///
/// The `systemMessage` field injects context into the conversation.
/// The `decision` field (for `PreToolUse`) can block tool execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookResponse {
    /// Optional message injected into the Claude Code conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,
    /// Decision for `PreToolUse` hooks: `"allow"` or `"block"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
    /// Reason for blocking (used with `decision: "block"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl HookResponse {
    /// Create an empty response (no context injection).
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Create a response with a system message.
    #[must_use]
    pub fn with_message(message: impl Into<String>) -> Self {
        Self {
            system_message: Some(message.into()),
            ..Self::default()
        }
    }

    /// Create a blocking response for `PreToolUse`.
    #[must_use]
    pub fn block(reason: impl Into<String>) -> Self {
        Self {
            decision: Some("block".into()),
            reason: Some(reason.into()),
            ..Self::default()
        }
    }

    /// Create a response that allows tool use with an optional message.
    #[must_use]
    pub fn allow(message: Option<String>) -> Self {
        Self {
            system_message: message,
            ..Self::default()
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Health response
// ──────────────────────────────────────────────────────────────

/// Health check response for the `/health` endpoint.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status.
    pub status: &'static str,
    /// Service name.
    pub service: &'static str,
    /// Listening port.
    pub port: u16,
    /// Number of tracked sessions.
    pub sessions: usize,
    /// Uptime tick counter.
    pub uptime_ticks: u64,
    /// RALPH evolution generation counter.
    #[cfg(feature = "evolution")]
    pub ralph_gen: u64,
    /// RALPH current phase name.
    #[cfg(feature = "evolution")]
    pub ralph_phase: String,
    /// RALPH current fitness score.
    #[cfg(feature = "evolution")]
    pub ralph_fitness: f64,
    /// IPC bus connection state (`"disconnected"`, `"connecting"`, `"connected"`).
    pub ipc_state: String,
    /// Circuit breaker states per bridge (service name -> state string).
    #[cfg(feature = "intelligence")]
    pub breakers: serde_json::Value,
    /// Total semantic routing dispatches.
    pub dispatch_total: u64,
}

// ──────────────────────────────────────────────────────────────
// Active session tracking
// ──────────────────────────────────────────────────────────────

/// Per-session tracking state for a Claude Code instance.
#[derive(Debug, Clone)]
pub struct SessionTracker {
    /// The sphere ID registered for this session.
    pub pane_id: PaneId,
    /// Active task ID (if any).
    pub active_task_id: Option<String>,
    /// Epoch ms when the active task was claimed (for duration tracking).
    pub active_task_claimed_ms: Option<u64>,
    /// Tool call counter for throttling.
    pub poll_counter: u64,
    /// Total tool calls in this session (for ghost enrichment).
    pub total_tool_calls: u64,
    /// Session start time (epoch ms) for duration calculation.
    pub started_ms: u64,
    /// Persona string from registration (for ghost trace).
    pub persona: String,
}

// ──────────────────────────────────────────────────────────────
// ORAC shared state
// ──────────────────────────────────────────────────────────────

/// Shared application state for all hook handlers.
///
/// Wrapped in `Arc` and passed to every Axum handler via `State`.
/// Interior mutability via `parking_lot::RwLock` for session tracking.
#[derive(Debug)]
pub struct OracState {
    /// ORAC configuration (immutable after startup).
    pub config: PvConfig,
    /// Cached field state from PV2 daemon.
    pub field_state: SharedState,
    /// PV2 daemon HTTP URL.
    pub pv2_url: String,
    /// SYNTHEX HTTP URL.
    pub synthex_url: String,
    /// POVM HTTP URL.
    pub povm_url: String,
    /// Reasoning Memory HTTP URL.
    pub rm_url: String,
    /// Per-session tracking (keyed by session ID).
    pub sessions: RwLock<HashMap<String, SessionTracker>>,
    /// Global tick counter for uptime.
    pub tick: AtomicU64,
    /// IPC bus connection state string (updated by IPC event loop).
    pub ipc_state: RwLock<String>,
    /// Ghost traces of deregistered spheres (FIFO, max 20).
    pub ghosts: RwLock<VecDeque<OracGhost>>,
    /// Per-sphere consent declarations (FIX-018).
    pub consents: RwLock<HashMap<String, OracConsent>>,
    /// `SQLite` blackboard for persistent fleet state (feature-gated).
    #[cfg(feature = "persistence")]
    pub blackboard: Option<Mutex<Blackboard>>,
    /// RALPH evolution engine (feature-gated).
    #[cfg(feature = "evolution")]
    pub ralph: RalphEngine,
    /// Hebbian coupling network for semantic routing.
    pub coupling: RwLock<CouplingNetwork>,
    /// Circuit breaker registry for external service calls.
    #[cfg(feature = "intelligence")]
    pub breakers: RwLock<BreakerRegistry>,
    /// Semantic routing dispatch counters (total + per-domain).
    pub dispatch_total: AtomicU64,
    /// Dispatch counter for `Read` domain tasks.
    pub dispatch_read: AtomicU64,
    /// Dispatch counter for `Write` domain tasks.
    pub dispatch_write: AtomicU64,
    /// Dispatch counter for `Execute` domain tasks.
    pub dispatch_execute: AtomicU64,
    /// Dispatch counter for `Communicate` domain tasks.
    pub dispatch_communicate: AtomicU64,
    /// In-process trace store for `OTel`-style span recording (feature-gated).
    #[cfg(feature = "monitoring")]
    pub trace_store: crate::m7_monitoring::m32_otel_traces::TraceStore,
    /// Kuramoto field dashboard for `/dashboard` endpoint (feature-gated).
    #[cfg(feature = "monitoring")]
    pub dashboard: crate::m7_monitoring::m34_field_dashboard::FieldDashboard,
    /// Token accounting registry for `/tokens` endpoint (feature-gated).
    #[cfg(feature = "monitoring")]
    pub token_accountant: crate::m7_monitoring::m35_token_accounting::TokenAccountant,
}

impl OracState {
    /// Create a new `OracState` from configuration.
    #[must_use]
    pub fn new(config: PvConfig) -> Self {
        Self {
            pv2_url: "http://127.0.0.1:8132".into(),
            synthex_url: "http://127.0.0.1:8090".into(),
            povm_url: "http://127.0.0.1:8125".into(),
            rm_url: "http://127.0.0.1:8130".into(),
            config,
            field_state: field_state::new_shared_state(),
            sessions: RwLock::new(HashMap::new()),
            tick: AtomicU64::new(0),
            ipc_state: RwLock::new("disconnected".into()),
            ghosts: RwLock::new(VecDeque::new()),
            consents: RwLock::new(HashMap::new()),
            #[cfg(feature = "persistence")]
            blackboard: open_blackboard(),
            #[cfg(feature = "evolution")]
            ralph: RalphEngine::new(),
            coupling: RwLock::new(CouplingNetwork::new()),
            #[cfg(feature = "intelligence")]
            breakers: RwLock::new(init_breaker_registry()),
            dispatch_total: AtomicU64::new(0),
            dispatch_read: AtomicU64::new(0),
            dispatch_write: AtomicU64::new(0),
            dispatch_execute: AtomicU64::new(0),
            dispatch_communicate: AtomicU64::new(0),
            #[cfg(feature = "monitoring")]
            trace_store: crate::m7_monitoring::m32_otel_traces::TraceStore::new(),
            #[cfg(feature = "monitoring")]
            dashboard: crate::m7_monitoring::m34_field_dashboard::FieldDashboard::new(),
            #[cfg(feature = "monitoring")]
            token_accountant: crate::m7_monitoring::m35_token_accounting::TokenAccountant::new(),
        }
    }

    /// Create `OracState` with custom service URLs (for testing).
    #[must_use]
    pub fn with_urls(
        config: PvConfig,
        pv2_url: String,
        synthex_url: String,
        povm_url: String,
        rm_url: String,
    ) -> Self {
        Self {
            config,
            field_state: field_state::new_shared_state(),
            pv2_url,
            synthex_url,
            povm_url,
            rm_url,
            sessions: RwLock::new(HashMap::new()),
            tick: AtomicU64::new(0),
            ipc_state: RwLock::new("disconnected".into()),
            ghosts: RwLock::new(VecDeque::new()),
            consents: RwLock::new(HashMap::new()),
            #[cfg(feature = "persistence")]
            blackboard: None,
            #[cfg(feature = "evolution")]
            ralph: RalphEngine::new(),
            coupling: RwLock::new(CouplingNetwork::new()),
            #[cfg(feature = "intelligence")]
            breakers: RwLock::new(init_breaker_registry()),
            dispatch_total: AtomicU64::new(0),
            dispatch_read: AtomicU64::new(0),
            dispatch_write: AtomicU64::new(0),
            dispatch_execute: AtomicU64::new(0),
            dispatch_communicate: AtomicU64::new(0),
            #[cfg(feature = "monitoring")]
            trace_store: crate::m7_monitoring::m32_otel_traces::TraceStore::new(),
            #[cfg(feature = "monitoring")]
            dashboard: crate::m7_monitoring::m34_field_dashboard::FieldDashboard::new(),
            #[cfg(feature = "monitoring")]
            token_accountant: crate::m7_monitoring::m35_token_accounting::TokenAccountant::new(),
        }
    }

    /// Register a new session.
    pub fn register_session(&self, session_id: String, pane_id: PaneId) {
        let tracker = SessionTracker {
            pane_id,
            active_task_id: None,
            active_task_claimed_ms: None,
            poll_counter: 0,
            total_tool_calls: 0,
            started_ms: epoch_ms(),
            persona: String::new(),
        };
        self.sessions.write().insert(session_id, tracker);
    }

    /// Remove a session.
    pub fn remove_session(&self, session_id: &str) -> Option<SessionTracker> {
        self.sessions.write().remove(session_id)
    }

    /// Get the number of active sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.read().len()
    }

    /// Increment and return the current tick.
    pub fn increment_tick(&self) -> u64 {
        self.tick.fetch_add(1, Ordering::Relaxed)
    }

    /// Update the IPC connection state string.
    pub fn set_ipc_state(&self, new_state: &str) {
        new_state.clone_into(&mut self.ipc_state.write());
    }

    /// Get the current IPC connection state.
    #[must_use]
    pub fn get_ipc_state(&self) -> String {
        self.ipc_state.read().clone()
    }

    /// Access the local `SQLite` blackboard (if persistence is enabled and DB opened).
    #[cfg(feature = "persistence")]
    #[must_use]
    pub fn blackboard(&self) -> Option<parking_lot::MutexGuard<'_, Blackboard>> {
        self.blackboard.as_ref().map(Mutex::lock)
    }

    /// Record a ghost trace for a deregistered sphere.
    ///
    /// Maintains a FIFO ring of the last 20 ghost entries in memory,
    /// and persists to the blackboard `SQLite` if available.
    pub fn add_ghost(&self, ghost: OracGhost) {
        // Persist to SQLite blackboard
        #[cfg(feature = "persistence")]
        if let Some(bb) = self.blackboard() {
            use crate::m5_bridges::m26_blackboard::GhostRecord;
            let record = GhostRecord {
                sphere_id: ghost.sphere_id.clone(),
                persona: ghost.persona.clone(),
                deregistered_ms: ghost.deregistered_ms,
                final_phase: ghost.final_phase,
                total_tools: ghost.total_tools,
                session_duration_ms: ghost.session_duration_ms,
            };
            if let Err(e) = bb.insert_ghost(&record) {
                tracing::debug!("blackboard insert_ghost failed: {e}");
            }
            // Keep SQLite bounded to 100 entries
            let _ = bb.prune_ghosts(100);
        }

        // In-memory ring buffer
        let mut ghosts = self.ghosts.write();
        if ghosts.len() >= MAX_GHOSTS {
            ghosts.pop_front();
        }
        ghosts.push_back(ghost);
    }

    /// Get all ghost traces (newest last).
    #[must_use]
    pub fn get_ghosts(&self) -> Vec<OracGhost> {
        self.ghosts.read().iter().cloned().collect()
    }

    /// Get consent for a sphere, creating fully-open default if absent.
    #[must_use]
    pub fn get_consent(&self, sphere_id: &str) -> OracConsent {
        self.consents
            .read()
            .get(sphere_id)
            .cloned()
            .unwrap_or_else(OracConsent::fully_open)
    }

    /// Check whether a specific bridge operation is consented for a sphere.
    ///
    /// Returns `true` if the operation is allowed (default for unknown spheres).
    /// Field names: `"synthex_write"`, `"povm_read"`, `"povm_write"`, `"hydration"`.
    #[must_use]
    pub fn consent_allows(&self, sphere_id: &str, field: &str) -> bool {
        let consent = self.get_consent(sphere_id);
        match field {
            "synthex_write" => consent.synthex_write,
            "povm_read" => consent.povm_read,
            "povm_write" => consent.povm_write,
            "hydration" => consent.hydration,
            _ => true, // unknown fields default to allowed
        }
    }

    /// Update consent fields for a sphere. Returns list of updated field names.
    ///
    /// Also persists changes to the blackboard consent audit trail.
    pub fn update_consent(
        &self,
        sphere_id: &str,
        update: &ConsentUpdateRequest,
    ) -> Vec<&'static str> {
        let mut updated = Vec::new();
        let mut audit_entries: Vec<(&'static str, bool, bool)> = Vec::new();
        let mut guard = self.consents.write();
        let consent = guard
            .entry(sphere_id.to_owned())
            .or_insert_with(OracConsent::fully_open);

        if let Some(v) = update.synthex_write {
            if consent.synthex_write != v {
                audit_entries.push(("synthex_write", consent.synthex_write, v));
            }
            consent.synthex_write = v;
            updated.push("synthex_write");
        }
        if let Some(v) = update.povm_read {
            if consent.povm_read != v {
                audit_entries.push(("povm_read", consent.povm_read, v));
            }
            consent.povm_read = v;
            updated.push("povm_read");
        }
        if let Some(v) = update.povm_write {
            if consent.povm_write != v {
                audit_entries.push(("povm_write", consent.povm_write, v));
            }
            consent.povm_write = v;
            updated.push("povm_write");
        }
        if let Some(v) = update.hydration {
            if consent.hydration != v {
                audit_entries.push(("hydration", consent.hydration, v));
            }
            consent.hydration = v;
            updated.push("hydration");
        }
        if !updated.is_empty() {
            consent.updated_ms = epoch_ms();
        }
        drop(guard);

        // Persist audit trail to blackboard
        #[cfg(feature = "persistence")]
        if !audit_entries.is_empty() {
            if let Some(bb) = self.blackboard() {
                use crate::m5_bridges::m26_blackboard::ConsentAuditEntry;
                let now = epoch_ms();
                for (field, old, new) in &audit_entries {
                    let _ = bb.insert_consent_audit(&ConsentAuditEntry {
                        sphere_id: sphere_id.to_owned(),
                        field_name: (*field).to_owned(),
                        old_value: *old,
                        new_value: *new,
                        changed_ms: now,
                    });
                }
            }
        }

        updated
    }

    /// Check whether a service's circuit breaker allows requests.
    #[cfg(feature = "intelligence")]
    #[must_use]
    pub fn breaker_allows(&self, service: &str) -> bool {
        self.breakers.read().allows_request(&PaneId::new(service))
    }

    /// Record a successful call to a service.
    #[cfg(feature = "intelligence")]
    pub fn breaker_success(&self, service: &str) {
        self.breakers.write().record_success(&PaneId::new(service));
    }

    /// Record a failed call to a service.
    #[cfg(feature = "intelligence")]
    pub fn breaker_failure(&self, service: &str) {
        let tick = self.tick.load(Ordering::Relaxed);
        self.breakers
            .write()
            .record_failure(&PaneId::new(service), tick);
    }

    /// Advance all breaker state machines by one tick.
    #[cfg(feature = "intelligence")]
    pub fn breaker_tick(&self) {
        let tick = self.tick.load(Ordering::Relaxed);
        self.breakers.write().tick_all(tick);
    }

    /// Get breaker state counts `(closed, open, half_open)`.
    #[cfg(feature = "intelligence")]
    #[must_use]
    pub fn breaker_state_counts(&self) -> (usize, usize, usize) {
        self.breakers.read().state_counts()
    }

    /// Record a semantic routing dispatch for the given domain.
    pub fn record_dispatch(
        &self,
        domain: &crate::m4_intelligence::m20_semantic_router::SemanticDomain,
    ) {
        use crate::m4_intelligence::m20_semantic_router::SemanticDomain;
        self.dispatch_total.fetch_add(1, Ordering::Relaxed);
        match domain {
            SemanticDomain::Read => {
                self.dispatch_read.fetch_add(1, Ordering::Relaxed);
            }
            SemanticDomain::Write => {
                self.dispatch_write.fetch_add(1, Ordering::Relaxed);
            }
            SemanticDomain::Execute => {
                self.dispatch_execute.fetch_add(1, Ordering::Relaxed);
            }
            SemanticDomain::Communicate => {
                self.dispatch_communicate.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Read dispatch counters as `(total, read, write, execute, communicate)`.
    #[must_use]
    pub fn dispatch_counts(&self) -> (u64, u64, u64, u64, u64) {
        (
            self.dispatch_total.load(Ordering::Relaxed),
            self.dispatch_read.load(Ordering::Relaxed),
            self.dispatch_write.load(Ordering::Relaxed),
            self.dispatch_execute.load(Ordering::Relaxed),
            self.dispatch_communicate.load(Ordering::Relaxed),
        )
    }

    /// Collect a fitness `TensorValues` from blackboard fleet metrics + field state.
    ///
    /// Populates dimensions that can be derived from local state:
    /// - D1 `field_coherence`: from cached r value
    /// - D3 `task_throughput`: completed tasks / total panes (normalized)
    /// - D4 `error_rate`: 1.0 - (failed / total tasks) (inverted)
    /// - D9 `fleet_utilization`: working panes / total panes
    /// - D11 `consent_compliance`: fraction of spheres with default-open consent
    ///
    /// Dimensions without local data are left at 0.5 (neutral).
    #[cfg(feature = "evolution")]
    #[must_use]
    pub fn collect_tensor(&self) -> crate::m8_evolution::m39_fitness_tensor::TensorValues {
        use crate::m8_evolution::m39_fitness_tensor::{FitnessDimension, TensorValues};

        let mut tensor = TensorValues::uniform(0.5); // neutral baseline

        // D1: field_coherence — from cached field state
        {
            let guard = self.field_state.read();
            let r = guard.field.order.r;
            tensor.set(FitnessDimension::FieldCoherence, r.clamp(0.0, 1.0));
        }

        // D9, D3, D4: from blackboard (if available)
        #[cfg(feature = "persistence")]
        if let Some(bb) = self.blackboard() {
            let panes = bb.list_panes().unwrap_or_default();
            let total_panes = panes.len();

            if total_panes > 0 {
                // D9: fleet_utilization — fraction of panes that are Working
                let working = panes
                    .iter()
                    .filter(|p| {
                        p.status == crate::m1_core::m01_core_types::PaneStatus::Working
                    })
                    .count();
                #[allow(clippy::cast_precision_loss)]
                let utilization = working as f64 / total_panes as f64;
                tensor.set(FitnessDimension::FleetUtilization, utilization);

                // D3: task_throughput — avg tasks_completed per pane (capped at 1.0 for 10+ tasks)
                #[allow(clippy::cast_precision_loss)]
                let avg_tasks = panes.iter().map(|p| p.tasks_completed).sum::<u64>() as f64
                    / total_panes as f64;
                tensor.set(FitnessDimension::TaskThroughput, (avg_tasks / 10.0).min(1.0));
            }

            // D4: error_rate (inverted) — 1.0 - (failed / total)
            let total_tasks = bb.task_count().unwrap_or(0);
            if total_tasks > 0 {
                // Count failed tasks by iterating recent tasks across all panes
                let mut failed = 0_u64;
                for pane in &panes {
                    if let Ok(tasks) = bb.recent_tasks(&pane.pane_id, 100) {
                        failed += tasks.iter().filter(|t| t.outcome == "failed").count() as u64;
                    }
                }
                #[allow(clippy::cast_precision_loss)]
                let fail_rate = failed as f64 / total_tasks as f64;
                tensor.set(FitnessDimension::ErrorRate, (1.0 - fail_rate).clamp(0.0, 1.0));
            }
        }

        // D11: consent_compliance — fraction of spheres with default-open consent
        {
            let consents = self.consents.read();
            let total = consents.len();
            if total > 0 {
                let compliant = consents
                    .values()
                    .filter(|c| c.hydration && c.povm_read)
                    .count();
                #[allow(clippy::cast_precision_loss)]
                let compliance = compliant as f64 / total as f64;
                tensor.set(FitnessDimension::ConsentCompliance, compliance);
            }
        }

        tensor
    }
}

/// Create and populate the default breaker registry with per-service configs.
#[cfg(feature = "intelligence")]
fn init_breaker_registry() -> BreakerRegistry {
    let mut reg = BreakerRegistry::new(BreakerConfig::default());
    // BUG-038 fix: aggressive() tripped too easily on transient failures.
    // PV2 is a local service — default config (threshold=5, timeout=30) is sufficient.
    reg.register(PaneId::new("pv2"), BreakerConfig::default());
    reg.register(PaneId::new("synthex"), BreakerConfig::default());
    reg.register(PaneId::new("me"), BreakerConfig::tolerant());
    reg.register(PaneId::new("povm"), BreakerConfig::default());
    reg.register(PaneId::new("rm"), BreakerConfig::default());
    reg
}

// ──────────────────────────────────────────────────────────────
// HTTP helpers (PV2 / service communication)
// ──────────────────────────────────────────────────────────────

/// Fire-and-forget HTTP POST to a service.
///
/// Spawns a background task that makes the request and logs any errors.
/// Does not block the caller.
pub fn fire_and_forget_post(url: String, body: String) {
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            ureq::post(&url)
                .timeout(Duration::from_millis(2000))
                .set("Content-Type", "application/json")
                .send_string(&body)
                .map_err(Box::new)
        })
        .await;
        match result {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => tracing::debug!("fire-and-forget POST failed: {e}"),
            Err(e) => tracing::debug!("fire-and-forget task panicked: {e}"),
        }
    });
}

/// Fire-and-forget HTTP POST that records breaker outcomes.
///
/// Checks the breaker before calling. Records success/failure after.
/// If the breaker is open, skips the call entirely.
#[cfg(feature = "intelligence")]
pub fn breaker_guarded_post(state: &Arc<OracState>, service: &str, url: String, body: String) {
    if !state.breaker_allows(service) {
        tracing::debug!("breaker open for {service}, skipping POST to {url}");
        return;
    }
    let service_name = service.to_owned();
    let state_clone = Arc::clone(state);
    tokio::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            ureq::post(&url)
                .timeout(Duration::from_millis(2000))
                .set("Content-Type", "application/json")
                .send_string(&body)
                .map_err(Box::new)
        })
        .await;
        match result {
            Ok(Ok(_)) => state_clone.breaker_success(&service_name),
            _ => state_clone.breaker_failure(&service_name),
        }
    });
}

/// Spawn a background task that polls PV2 `/health` every 5s
/// and updates `SharedState` with the latest field state.
///
/// Runs until the process shuts down. If PV2 is unreachable,
/// the cached state remains unchanged (graceful degradation).
#[allow(clippy::too_many_lines)] // BUG-038 fix added breaker integration lines
pub fn spawn_field_poller(state: Arc<OracState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let health_url = format!("{}/health", state.pv2_url);
            let spheres_url = format!("{}/spheres", state.pv2_url);

            // Fetch health and spheres in parallel
            let (health_resp, spheres_resp) = tokio::join!(
                http_get(&health_url, 2000),
                http_get(&spheres_url, 2000),
            );

            // BUG-038 fix: advance breaker ticks from poller (every 5s)
            // so Open→HalfOpen transitions happen reliably, not just in prompt hooks.
            #[cfg(feature = "intelligence")]
            state.breaker_tick();

            let Some(health_json) = health_resp else {
                state.field_state.write().record_poll_miss();
                // BUG-038 fix: record PV2 failure so breaker state is accurate
                #[cfg(feature = "intelligence")]
                state.breaker_failure("pv2");
                tracing::debug!("field poller: PV2 unreachable");
                continue;
            };

            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&health_json) else {
                continue;
            };

            let r = parsed.get("r").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
            let pv2_tick = parsed.get("tick").and_then(serde_json::Value::as_u64).unwrap_or(0);

            // Parse spheres into HashMap if available.
            // PV2 /spheres returns {"spheres": [...]} with compact fields:
            //   id, persona, status, phase, frequency, memories (count, not Vec),
            //   receptivity, total_steps. Missing fields like buoys, opt_out_hebbian
            //   would cause Vec<SphereMemory> deserialization failure (BUG-038).
            // Use a lightweight struct matching PV2's actual wire format.
            let sphere_map: HashMap<PaneId, PaneSphere> = spheres_resp
                .and_then(|s| {
                    #[derive(serde::Deserialize)]
                    struct PvSphereCompact {
                        id: String,
                        #[serde(default)]
                        persona: String,
                        #[serde(default)]
                        status: String,
                        #[serde(default)]
                        phase: f64,
                        #[serde(default)]
                        frequency: f64,
                        #[serde(default)]
                        receptivity: f64,
                        #[serde(default)]
                        total_steps: u64,
                    }

                    impl PvSphereCompact {
                        fn into_pane_sphere(self) -> PaneSphere {
                            use crate::m1_core::m01_core_types::PaneStatus;
                            let status = match self.status.as_str() {
                                "Working" | "working" => PaneStatus::Working,
                                "Blocked" | "blocked" => PaneStatus::Blocked,
                                "Complete" | "complete" => PaneStatus::Complete,
                                _ => PaneStatus::Idle,
                            };
                            PaneSphere {
                                id: PaneId::new(&self.id),
                                persona: self.persona,
                                status,
                                phase: self.phase,
                                frequency: self.frequency,
                                receptivity: self.receptivity.max(0.01),
                                total_steps: self.total_steps,
                                ..PaneSphere::default()
                            }
                        }
                    }

                    #[derive(serde::Deserialize)]
                    struct SpheresResponse {
                        spheres: Vec<PvSphereCompact>,
                    }

                    serde_json::from_str::<SpheresResponse>(&s)
                        .map(|resp| resp.spheres)
                        .or_else(|_| serde_json::from_str::<Vec<PvSphereCompact>>(&s))
                        .ok()
                })
                .map(|vec| {
                    vec.into_iter()
                        .map(|sp| {
                            let ps = sp.into_pane_sphere();
                            (ps.id.clone(), ps)
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Update SharedState — minimal lock scope
            {
                let mut guard = state.field_state.write();

                if sphere_map.is_empty() {
                    // No sphere data — update from health only
                    guard.field.order.r = r.clamp(0.0, 1.0);
                    guard.field.tick = pv2_tick;
                } else {
                    // Full recompute from sphere phases
                    guard.spheres = sphere_map;
                    guard.field = FieldState::compute(&guard.spheres, pv2_tick);
                }

                guard.tick = pv2_tick;
                guard.push_r(r);
                guard.tick_warmup();
                guard.record_poll_success();
            }

            // BUG-043 fix: Update dashboard with field state on each poll tick
            #[cfg(feature = "monitoring")]
            {
                let guard = state.field_state.read();
                let k_eff = {
                    let c = state.coupling.read();
                    c.k * c.k_modulation
                };
                state.dashboard.update_tick(
                    guard.field.tick,
                    &guard.field.order,
                    k_eff,
                );
            }

            // BUG-038 fix: record PV2 success against breaker.
            // The field poller proves PV2 is healthy — keep breaker Closed.
            #[cfg(feature = "intelligence")]
            state.breaker_success("pv2");

            tracing::trace!(r, pv2_tick, "field state updated from PV2");
        }
    });
}

/// Make an async HTTP GET and return the response body.
///
/// Returns `None` if the request fails or times out.
pub async fn http_get(url: &str, timeout_ms: u64) -> Option<String> {
    let url = url.to_owned();
    tokio::task::spawn_blocking(move || {
        ureq::get(&url)
            .timeout(Duration::from_millis(timeout_ms))
            .call()
            .ok()?
            .into_string()
            .ok()
    })
    .await
    .ok()
    .flatten()
}

/// Make an async HTTP POST with JSON body and return the response body.
///
/// Returns `None` if the request fails or times out.
pub async fn http_post(url: &str, body: &str, timeout_ms: u64) -> Option<String> {
    let url = url.to_owned();
    let body = body.to_owned();
    tokio::task::spawn_blocking(move || {
        ureq::post(&url)
            .timeout(Duration::from_millis(timeout_ms))
            .set("Content-Type", "application/json")
            .send_string(&body)
            .ok()?
            .into_string()
            .ok()
    })
    .await
    .ok()
    .flatten()
}

/// Generate a pane ID for this ORAC session.
///
/// Uses hostname and PID for uniqueness.
#[must_use]
pub fn generate_pane_id() -> PaneId {
    let hostname = std::env::var("HOSTNAME")
        .unwrap_or_else(|_| "orac".into());
    PaneId::new(format!("{hostname}:{}", std::process::id()))
}

// ──────────────────────────────────────────────────────────────
// Axum handlers
// ──────────────────────────────────────────────────────────────

/// Health check handler.
async fn health_handler(
    State(state): State<Arc<OracState>>,
) -> Json<HealthResponse> {
    #[cfg(feature = "evolution")]
    let ralph_state = state.ralph.state();

    Json(HealthResponse {
        status: "healthy",
        service: "orac-sidecar",
        port: state.config.server.port,
        sessions: state.session_count(),
        uptime_ticks: state.tick.load(Ordering::Relaxed),
        #[cfg(feature = "evolution")]
        ralph_gen: ralph_state.generation,
        #[cfg(feature = "evolution")]
        ralph_phase: ralph_state.phase.to_string(),
        #[cfg(feature = "evolution")]
        ralph_fitness: ralph_state.current_fitness,
        ipc_state: state.ipc_state.read().clone(),
        #[cfg(feature = "intelligence")]
        breakers: {
            let summaries = state.breakers.read().all_summaries();
            let map: std::collections::HashMap<String, String> = summaries
                .iter()
                .map(|(id, s)| (id.as_str().to_owned(), s.state.to_string()))
                .collect();
            serde_json::to_value(map).unwrap_or_default()
        },
        dispatch_total: state.dispatch_total.load(Ordering::Relaxed),
    })
}

/// Field state handler — returns cached field state, enriched with live PV2 `k`/`k_mod`.
///
/// Primary source: `SharedState` cache (populated by poller every 5s).
/// Enrichment: live PV2 `/health` for `k` and `k_mod` (not cached by poller).
/// Falls back to cache-only if PV2 is unreachable.
async fn field_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
    // Read cached field state (sub-microsecond)
    let (r, cached_tick, sphere_count, field_is_stale) = {
        let guard = state.field_state.read();
        (
            guard.field.order.r,
            guard.field.tick,
            guard.spheres.len(),
            guard.is_stale(),
        )
    };

    let mut result = serde_json::json!({
        "source": if field_is_stale { "cache_stale" } else { "cache" },
        "tick": state.tick.load(Ordering::Relaxed),
        "r": r,
        "sphere_count": sphere_count,
        "pv2_tick": cached_tick,
        "stale": field_is_stale,
    });

    // Enrich with k/k_mod from live PV2 (not cached by poller)
    let pv2_url = format!("{}/health", state.pv2_url);
    if let Some(h) = http_get(&pv2_url, 2000).await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&h) {
            result["k"] = v.get("k").cloned().unwrap_or_default();
            result["k_mod"] = v.get("k_mod").cloned().unwrap_or_default();
            result["source"] = serde_json::json!("cache_enriched");
        }
    }

    // Enrich with emergence data from RALPH EmergenceDetector
    #[cfg(feature = "evolution")]
    {
        let detector = state.ralph.emergence();
        let recent = detector.recent(5);
        let emergence_list: Vec<serde_json::Value> = recent
            .iter()
            .map(|e| {
                serde_json::json!({
                    "type": e.emergence_type.to_string(),
                    "severity": e.severity_class.to_string(),
                    "confidence": e.confidence,
                    "description": e.description,
                    "detected_at_tick": e.detected_at_tick,
                    "ttl": e.ttl,
                })
            })
            .collect();

        let em_stats = detector.stats();
        result["emergence"] = serde_json::json!({
            "total_detected": em_stats.total_detected,
            "active_monitors": detector.active_monitor_count(),
            "history_len": detector.history_len(),
            "by_type": em_stats.by_type,
            "recent": emergence_list,
        });
    }

    Json(result)
}

/// Query parameters for the `/blackboard` endpoint.
///
/// All fields are optional filters. When absent, all records are returned.
#[derive(Debug, Deserialize)]
struct BlackboardQuery {
    /// Filter panes by status (Idle, Working, Blocked, Complete).
    status: Option<String>,
    /// Filter panes updated after this Unix timestamp (seconds).
    since: Option<f64>,
    /// Limit number of recent tasks returned (default 20).
    task_limit: Option<usize>,
}

/// Blackboard handler — returns session tracking + persistent fleet state.
///
/// Merges in-memory session data with `SQLite` blackboard (pane status,
/// agent cards, task history). Supports query filters:
/// - `?status=Working` — only panes with this status
/// - `?since=1711100000` — only panes updated after this timestamp
/// - `?task_limit=50` — max recent tasks per pane (default 20)
#[allow(clippy::too_many_lines)] // Structured by section: sessions, panes, cards, tasks, RALPH
async fn blackboard_handler(
    State(state): State<Arc<OracState>>,
    Query(query): Query<BlackboardQuery>,
) -> Json<serde_json::Value> {
    // In-memory sessions
    let sessions = state.sessions.read();
    let session_list: Vec<serde_json::Value> = sessions
        .iter()
        .map(|(sid, tracker)| {
            serde_json::json!({
                "session_id": sid,
                "pane_id": tracker.pane_id.as_str(),
                "active_task": tracker.active_task_id,
                "poll_counter": tracker.poll_counter,
            })
        })
        .collect();
    drop(sessions);

    // BUG-L3-004 fix: Use SQLite pane_count as authoritative fleet_size (SSOT).
    // In-memory session_list.len() may diverge from SQLite if sessions leak.
    let fleet_size = {
        #[cfg(feature = "persistence")]
        {
            state
                .blackboard()
                .and_then(|bb| bb.pane_count().ok())
                .map_or(session_list.len(), |n| n)
        }
        #[cfg(not(feature = "persistence"))]
        {
            session_list.len()
        }
    };

    let mut result = serde_json::json!({
        "sessions": session_list,
        "fleet_size": fleet_size,
        "uptime_ticks": state.tick.load(Ordering::Relaxed),
    });

    // Persistent blackboard data (if available)
    #[cfg(feature = "persistence")]
    if let Some(bb) = state.blackboard() {
        // Pane status — apply filters
        let mut panes: Vec<serde_json::Value> = bb
            .list_panes()
            .unwrap_or_default()
            .into_iter()
            .filter(|p| {
                if let Some(ref s) = query.status {
                    if format!("{}", p.status) != *s {
                        return false;
                    }
                }
                if let Some(since) = query.since {
                    if p.updated_at < since {
                        return false;
                    }
                }
                true
            })
            .map(|p| {
                serde_json::json!({
                    "pane_id": p.pane_id.as_str(),
                    "status": format!("{}", p.status),
                    "persona": p.persona,
                    "tasks_completed": p.tasks_completed,
                    "updated_at": p.updated_at,
                    "phase": p.phase,
                })
            })
            .collect();
        panes.sort_by(|a, b| {
            let ta = a["updated_at"].as_f64().unwrap_or(0.0);
            let tb = b["updated_at"].as_f64().unwrap_or(0.0);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Agent cards
        let cards: Vec<serde_json::Value> = bb
            .list_cards()
            .unwrap_or_default()
            .into_iter()
            .map(|c| {
                serde_json::json!({
                    "pane_id": c.pane_id.as_str(),
                    "capabilities": c.capabilities,
                    "domain": c.domain,
                    "model": c.model,
                })
            })
            .collect();

        // Recent tasks — across all panes, limited
        let task_limit = query.task_limit.unwrap_or(20);
        let mut all_tasks: Vec<serde_json::Value> = Vec::new();
        for pane in bb.list_panes().unwrap_or_default() {
            if let Ok(tasks) = bb.recent_tasks(&pane.pane_id, task_limit) {
                for t in tasks {
                    all_tasks.push(serde_json::json!({
                        "task_id": t.task_id,
                        "pane_id": t.pane_id.as_str(),
                        "description": t.description,
                        "outcome": t.outcome,
                        "finished_at": t.finished_at,
                        "duration_secs": t.duration_secs,
                    }));
                }
            }
        }
        all_tasks.sort_by(|a, b| {
            let ta = a["finished_at"].as_f64().unwrap_or(0.0);
            let tb = b["finished_at"].as_f64().unwrap_or(0.0);
            tb.partial_cmp(&ta).unwrap_or(std::cmp::Ordering::Equal)
        });
        all_tasks.truncate(task_limit);

        result["pane_status"] = serde_json::json!(panes);
        result["pane_count"] = serde_json::json!(panes.len());
        result["agent_cards"] = serde_json::json!(cards);
        result["recent_tasks"] = serde_json::json!(all_tasks);
        result["task_count"] = serde_json::json!(bb.task_count().unwrap_or(0));
    }

    // RALPH evolution state (BUG-044: was missing from /blackboard)
    #[cfg(feature = "evolution")]
    {
        result["ralph"] = build_ralph_json(&state);
    }

    Json(result)
}

/// Build RALPH evolution state as JSON (BUG-044 fix).
///
/// Extracted from `blackboard_handler` to keep function length within clippy limits.
#[cfg(feature = "evolution")]
fn build_ralph_json(state: &OracState) -> serde_json::Value {
    let rs = state.ralph.state();
    let st = state.ralph.stats();
    serde_json::json!({
        "generation": rs.generation,
        "phase": rs.phase.to_string(),
        "fitness": rs.current_fitness,
        "peak_fitness": st.peak_fitness,
        "paused": rs.paused,
        "completed_cycles": rs.completed_cycles,
        "mutations": {
            "proposed": st.total_proposed,
            "accepted": st.total_accepted,
            "rolled_back": st.total_rolled_back,
            "skipped": st.total_skipped,
        }
    })
}

/// Metrics handler — Prometheus-compatible text format.
///
/// Reports sessions, uptime, and blackboard fleet metrics.
async fn metrics_handler(
    State(state): State<Arc<OracState>>,
) -> (axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], String) {
    let sessions = state.session_count();
    let uptime = state.tick.load(Ordering::Relaxed);

    let mut body = format!(
        "# HELP orac_sessions_active Active session count\n\
         # TYPE orac_sessions_active gauge\n\
         orac_sessions_active {sessions}\n\
         \n\
         # HELP orac_uptime_ticks Server uptime in ticks\n\
         # TYPE orac_uptime_ticks counter\n\
         orac_uptime_ticks {uptime}\n"
    );

    // Blackboard fleet metrics
    #[cfg(feature = "persistence")]
    if let Some(bb) = state.blackboard() {
        use crate::m1_core::m01_core_types::PaneStatus;
        use std::fmt::Write;

        let panes = bb.list_panes().unwrap_or_default();
        let (idle, working, blocked, complete) =
            panes
                .iter()
                .fold((0_u64, 0_u64, 0_u64, 0_u64), |(i, w, b, c), p| match p.status {
                    PaneStatus::Idle => (i + 1, w, b, c),
                    PaneStatus::Working => (i, w + 1, b, c),
                    PaneStatus::Blocked => (i, w, b + 1, c),
                    PaneStatus::Complete => (i, w, b, c + 1),
                });
        let tasks_completed: u64 = panes.iter().map(|p| p.tasks_completed).sum();
        let task_history = bb.task_count().unwrap_or(0);
        let agent_cards = bb.card_count().unwrap_or(0);
        let _ = write!(
            body,
            "\n# HELP orac_panes_by_status Number of panes per operational status\n\
             # TYPE orac_panes_by_status gauge\n\
             orac_panes_by_status{{status=\"Idle\"}} {idle}\n\
             orac_panes_by_status{{status=\"Working\"}} {working}\n\
             orac_panes_by_status{{status=\"Blocked\"}} {blocked}\n\
             orac_panes_by_status{{status=\"Complete\"}} {complete}\n\
             \n\
             # HELP orac_tasks_completed_total Tasks completed across all panes\n\
             # TYPE orac_tasks_completed_total counter\n\
             orac_tasks_completed_total {tasks_completed}\n\
             \n\
             # HELP orac_task_history_total Task records in audit history\n\
             # TYPE orac_task_history_total counter\n\
             orac_task_history_total {task_history}\n\
             \n\
             # HELP orac_agent_cards_registered Registered agent capability cards\n\
             # TYPE orac_agent_cards_registered gauge\n\
             orac_agent_cards_registered {agent_cards}\n"
        );
    }

    // Semantic routing dispatch counters
    {
        use std::fmt::Write;
        let (total, read, write, execute, communicate) = state.dispatch_counts();
        let _ = write!(
            body,
            "\n# HELP orac_dispatch_total Total semantic routing dispatches\n             # TYPE orac_dispatch_total counter\n             orac_dispatch_total {total}\n             \n             # HELP orac_dispatch_by_domain Dispatches per semantic domain\n             # TYPE orac_dispatch_by_domain counter\n             orac_dispatch_by_domain{{domain=\"Read\"}} {read}\n             orac_dispatch_by_domain{{domain=\"Write\"}} {write}\n             orac_dispatch_by_domain{{domain=\"Execute\"}} {execute}\n             orac_dispatch_by_domain{{domain=\"Communicate\"}} {communicate}\n"
        );
    }

    // Circuit breaker state per bridge
    #[cfg(feature = "intelligence")]
    {
        use std::fmt::Write;
        let summaries = state.breakers.read().all_summaries();
        let _ = write!(
            body,
            "\n# HELP orac_breaker_state Circuit breaker state per bridge (0=Closed, 1=Open, 2=HalfOpen)\n             # TYPE orac_breaker_state gauge\n"
        );
        for (pane_id, summary) in &summaries {
            let state_val = match summary.state {
                crate::m4_intelligence::m21_circuit_breaker::BreakerState::Closed => 0,
                crate::m4_intelligence::m21_circuit_breaker::BreakerState::Open => 1,
                crate::m4_intelligence::m21_circuit_breaker::BreakerState::HalfOpen => 2,
            };
            let _ = writeln!(
                body,
                "orac_breaker_state{{bridge=\"{}\"}} {state_val}",
                pane_id.as_str(),
            );
        }

        let _ = write!(
            body,
            "\n# HELP orac_breaker_failures_total Lifetime failure count per bridge\n             # TYPE orac_breaker_failures_total counter\n"
        );
        for (pane_id, summary) in &summaries {
            let _ = writeln!(
                body,
                "orac_breaker_failures_total{{bridge=\"{}\"}} {}",
                pane_id.as_str(), summary.total_failures,
            );
        }
    }

    // RALPH evolution metrics
    #[cfg(feature = "evolution")]
    append_ralph_metrics(&mut body, &state);

    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
}

/// Append RALPH evolution + emergence metrics in Prometheus text format.
#[cfg(feature = "evolution")]
fn append_ralph_metrics(body: &mut String, orac: &OracState) {
    use std::fmt::Write;
    let rs = orac.ralph.state();
    let st = orac.ralph.stats();
    let em = orac.ralph.emergence();

    let _ = write!(
        body,
        "\n# HELP orac_ralph_generation Current RALPH generation counter\n\
         # TYPE orac_ralph_generation counter\n\
         orac_ralph_generation {}\n\
         \n\
         # HELP orac_ralph_completed_cycles Completed RALPH 5-phase cycles\n\
         # TYPE orac_ralph_completed_cycles counter\n\
         orac_ralph_completed_cycles {}\n\
         \n\
         # HELP orac_ralph_fitness Current RALPH fitness score\n\
         # TYPE orac_ralph_fitness gauge\n\
         orac_ralph_fitness {:.4}\n\
         \n\
         # HELP orac_ralph_peak_fitness Peak fitness observed\n\
         # TYPE orac_ralph_peak_fitness gauge\n\
         orac_ralph_peak_fitness {:.4}\n\
         \n\
         # HELP orac_ralph_paused Whether RALPH is paused (0=running, 1=paused)\n\
         # TYPE orac_ralph_paused gauge\n\
         orac_ralph_paused {}\n\
         \n\
         # HELP orac_ralph_mutations_total RALPH mutation outcomes\n\
         # TYPE orac_ralph_mutations_total counter\n\
         orac_ralph_mutations_total{{outcome=\"proposed\"}} {}\n\
         orac_ralph_mutations_total{{outcome=\"accepted\"}} {}\n\
         orac_ralph_mutations_total{{outcome=\"rolled_back\"}} {}\n\
         orac_ralph_mutations_total{{outcome=\"skipped\"}} {}\n\
         \n\
         # HELP orac_emergence_total Total emergence events detected\n\
         # TYPE orac_emergence_total counter\n\
         orac_emergence_total {}\n\
         \n\
         # HELP orac_emergence_active_monitors Active emergence monitors\n\
         # TYPE orac_emergence_active_monitors gauge\n\
         orac_emergence_active_monitors {}\n",
        rs.generation,
        rs.completed_cycles,
        rs.current_fitness,
        st.peak_fitness,
        u8::from(rs.paused),
        st.total_proposed,
        st.total_accepted,
        st.total_rolled_back,
        st.total_skipped,
        em.stats().total_detected,
        em.active_monitor_count(),
    );
}

/// List ghost traces of deregistered spheres (FIX-019).
///
/// Returns ORAC-local ghost ring buffer (FIFO, max 20).
/// Enriched with session timing data not available from PV2's native `/ghosts`.
async fn field_ghosts_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
    let ghosts = state.get_ghosts();
    let ghost_list: Vec<serde_json::Value> = ghosts
        .iter()
        .map(|g| {
            serde_json::json!({
                "sphere_id": g.sphere_id,
                "persona": g.persona,
                "deregistered_ms": g.deregistered_ms,
                "final_phase": g.final_phase,
                "total_tools": g.total_tools,
                "session_duration_ms": g.session_duration_ms,
            })
        })
        .collect();

    Json(serde_json::json!({ "ghosts": ghost_list }))
}

/// Get consent declarations for a sphere (FIX-018).
///
/// Returns ORAC bridge-level consent (`synthex_write`, `povm_read`, `povm_write`, `hydration`).
/// Creates a fully-open default if no explicit consent has been declared.
async fn consent_get_handler(
    State(state): State<Arc<OracState>>,
    Path(sphere_id): Path<String>,
) -> Json<serde_json::Value> {
    let consent = state.get_consent(&sphere_id);
    Json(serde_json::json!({
        "sphere_id": sphere_id,
        "consents": {
            "synthex_write": consent.synthex_write,
            "povm_read": consent.povm_read,
            "povm_write": consent.povm_write,
            "hydration": consent.hydration,
        },
        "updated_ms": consent.updated_ms,
    }))
}

/// Update consent declarations for a sphere (FIX-018).
///
/// Accepts partial updates — only specified fields are changed.
/// Returns the list of fields that were updated.
async fn consent_put_handler(
    State(state): State<Arc<OracState>>,
    Path(sphere_id): Path<String>,
    Json(body): Json<ConsentUpdateRequest>,
) -> Json<serde_json::Value> {
    let updated = state.update_consent(&sphere_id, &body);

    // Fire-and-forget: notify PV2 of consent change
    let pv2_url = format!(
        "{}/sphere/{}/status",
        state.pv2_url, sphere_id
    );
    let pv2_body = serde_json::json!({
        "status": "consent_updated",
        "consent_fields": updated,
    })
    .to_string();
    fire_and_forget_post(pv2_url, pv2_body);

    Json(serde_json::json!({
        "status": "ok",
        "updated": updated,
    }))
}

// ──────────────────────────────────────────────────────────────
// Monitoring endpoint handlers (BUG-043 fix)
// ──────────────────────────────────────────────────────────────

/// Traces handler — returns `OTel`-style span summary and recent spans.
///
/// Reads from the in-process `TraceStore` ring buffer, returning
/// both aggregate statistics and the most recent 50 completed spans.
#[cfg(feature = "monitoring")]
async fn traces_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
    let summary = state.trace_store.summary();
    let recent = state.trace_store.recent(50);
    let spans: Vec<serde_json::Value> = recent.iter().map(|s| {
        serde_json::json!({
            "name": s.name,
            "status": format!("{:?}", s.status),
            "start_secs": s.start_secs,
            "duration_ms": s.duration_ms(),
        })
    }).collect();
    Json(serde_json::json!({
        "summary": {
            "buffered": summary.buffered,
            "capacity": summary.capacity,
            "total_recorded": summary.total_recorded,
            "total_errors": summary.total_errors,
            "total_dropped": summary.total_dropped,
        },
        "recent": spans,
    }))
}

/// Traces fallback handler when the `monitoring` feature is disabled.
#[cfg(not(feature = "monitoring"))]
async fn traces_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "monitoring feature not enabled",
    }))
}

/// Dashboard handler — returns Kuramoto field dashboard snapshot.
///
/// Includes global order parameter, effective coupling, sphere/cluster
/// counts, chimera detection, and `r` trend statistics.
#[cfg(feature = "monitoring")]
async fn dashboard_endpoint_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
    let snap = state.dashboard.snapshot();
    Json(serde_json::json!({
        "tick": snap.tick,
        "r": snap.order.r,
        "psi": snap.order.psi,
        "k_effective": snap.k_effective,
        "sphere_count": snap.sphere_count,
        "cluster_count": snap.clusters.len(),
        "chimera_detected": snap.chimera_detected,
        "r_mean": state.dashboard.r_mean(),
        "r_stddev": state.dashboard.r_stddev(),
        "r_trend": state.dashboard.r_trend(),
    }))
}

/// Dashboard fallback handler when the `monitoring` feature is disabled.
#[cfg(not(feature = "monitoring"))]
async fn dashboard_endpoint_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "monitoring feature not enabled",
    }))
}

/// Tokens handler — returns fleet token accounting summary.
///
/// Reports fleet-wide input/output token totals, pane count,
/// budget status, and remaining budget.
#[cfg(feature = "monitoring")]
async fn tokens_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
    let summary = state.token_accountant.summary();
    Json(serde_json::json!({
        "total_input": summary.fleet_total.input_tokens,
        "total_output": summary.fleet_total.output_tokens,
        "total_panes": summary.pane_count,
        "budget_remaining": summary.remaining_budget,
        "budget_status": format!("{:?}", summary.budget_status),
        "utilization": summary.utilization,
    }))
}

/// Tokens fallback handler when the `monitoring` feature is disabled.
#[cfg(not(feature = "monitoring"))]
async fn tokens_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "monitoring feature not enabled",
    }))
}

// ──────────────────────────────────────────────────────────────
// Router construction
// ──────────────────────────────────────────────────────────────

/// Build the Axum router with all hook endpoints.
///
/// Each hook endpoint delegates to its handler module (m11-m14).
/// Handlers receive `Arc<OracState>` via Axum's `State` extractor.
pub fn build_router(state: Arc<OracState>) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/field", get(field_handler))
        .route("/blackboard", get(blackboard_handler))
        .route("/metrics", get(metrics_handler))
        .route("/field/ghosts", get(field_ghosts_handler))
        .route("/traces", get(traces_handler))
        .route("/dashboard", get(dashboard_endpoint_handler))
        .route("/tokens", get(tokens_handler))
        .route(
            "/consent/{sphere_id}",
            get(consent_get_handler).put(consent_put_handler),
        )
        .route(
            "/hooks/SessionStart",
            post(super::m11_session_hooks::handle_session_start),
        )
        .route(
            "/hooks/Stop",
            post(super::m11_session_hooks::handle_stop),
        )
        .route(
            "/hooks/PostToolUse",
            post(super::m12_tool_hooks::handle_post_tool_use),
        )
        .route(
            "/hooks/PreToolUse",
            post(super::m12_tool_hooks::handle_pre_tool_use),
        )
        .route(
            "/hooks/UserPromptSubmit",
            post(super::m13_prompt_hooks::handle_user_prompt_submit),
        )
        .route(
            "/hooks/PermissionRequest",
            post(super::m14_permission_policy::handle_permission_request),
        )
        .with_state(state)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── HookEvent ──

    #[test]
    fn hook_event_default() {
        let ev = HookEvent::default();
        assert!(ev.session_id.is_none());
        assert!(ev.tool_name.is_none());
        assert!(ev.tool_input.is_none());
        assert!(ev.tool_output.is_none());
        assert!(ev.prompt.is_none());
    }

    #[test]
    fn hook_event_deserialize_session_start() {
        let json = r#"{"session_id": "sess-123"}"#;
        let ev: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.session_id.as_deref(), Some("sess-123"));
        assert!(ev.tool_name.is_none());
    }

    #[test]
    fn hook_event_deserialize_post_tool_use() {
        let json = r#"{"tool_name": "Read", "tool_input": {"file_path": "/tmp/x"}, "tool_output": "contents"}"#;
        let ev: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.tool_name.as_deref(), Some("Read"));
        assert!(ev.tool_input.is_some());
        assert_eq!(ev.tool_output.as_deref(), Some("contents"));
    }

    #[test]
    fn hook_event_deserialize_user_prompt() {
        let json = r#"{"prompt": "Fix the bug in main.rs"}"#;
        let ev: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.prompt.as_deref(), Some("Fix the bug in main.rs"));
    }

    #[test]
    fn hook_event_deserialize_empty_object() {
        let json = "{}";
        let ev: HookEvent = serde_json::from_str(json).unwrap();
        assert!(ev.session_id.is_none());
    }

    #[test]
    fn hook_event_ignores_unknown_fields() {
        let json = r#"{"unknown_field": 42, "tool_name": "Bash"}"#;
        let ev: HookEvent = serde_json::from_str(json).unwrap();
        assert_eq!(ev.tool_name.as_deref(), Some("Bash"));
    }

    #[test]
    fn hook_event_serialize_roundtrip() {
        let ev = HookEvent {
            session_id: Some("s1".into()),
            tool_name: Some("Edit".into()),
            tool_input: Some(serde_json::json!({"path": "/tmp/x"})),
            tool_output: Some("ok".into()),
            prompt: None,
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, ev.session_id);
        assert_eq!(back.tool_name, ev.tool_name);
    }

    // ── HookResponse ──

    #[test]
    fn hook_response_empty() {
        let resp = HookResponse::empty();
        assert!(resp.system_message.is_none());
        assert!(resp.decision.is_none());
        assert!(resp.reason.is_none());
    }

    #[test]
    fn hook_response_with_message() {
        let resp = HookResponse::with_message("[FIELD] r=0.99");
        assert_eq!(resp.system_message.as_deref(), Some("[FIELD] r=0.99"));
        assert!(resp.decision.is_none());
    }

    #[test]
    fn hook_response_block() {
        let resp = HookResponse::block("thermal too high");
        assert_eq!(resp.decision.as_deref(), Some("block"));
        assert_eq!(resp.reason.as_deref(), Some("thermal too high"));
        assert!(resp.system_message.is_none());
    }

    #[test]
    fn hook_response_allow() {
        let resp = HookResponse::allow(Some("allowed".into()));
        assert!(resp.decision.is_none());
        assert_eq!(resp.system_message.as_deref(), Some("allowed"));
    }

    #[test]
    fn hook_response_allow_no_message() {
        let resp = HookResponse::allow(None);
        assert!(resp.decision.is_none());
        assert!(resp.system_message.is_none());
    }

    #[test]
    fn hook_response_serializes_camel_case() {
        let resp = HookResponse::with_message("test");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("systemMessage"));
        assert!(!json.contains("system_message"));
    }

    #[test]
    fn hook_response_skips_none_fields() {
        let resp = HookResponse::empty();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn hook_response_block_serialization() {
        let resp = HookResponse::block("too hot");
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains(r#""decision":"block""#));
        assert!(json.contains(r#""reason":"too hot""#));
    }

    // ── OracState ──

    #[test]
    fn orac_state_new() {
        let config = PvConfig::default();
        let state = OracState::new(config);
        assert_eq!(state.pv2_url, "http://127.0.0.1:8132");
        assert_eq!(state.synthex_url, "http://127.0.0.1:8090");
        assert_eq!(state.session_count(), 0);
    }

    #[test]
    fn orac_state_with_urls() {
        let state = OracState::with_urls(
            PvConfig::default(),
            "http://pv2:8132".into(),
            "http://sx:8090".into(),
            "http://povm:8125".into(),
            "http://rm:8130".into(),
        );
        assert_eq!(state.pv2_url, "http://pv2:8132");
        assert_eq!(state.synthex_url, "http://sx:8090");
    }

    #[test]
    fn orac_state_register_session() {
        let state = OracState::new(PvConfig::default());
        state.register_session("sess-1".into(), PaneId::new("alpha"));
        assert_eq!(state.session_count(), 1);
    }

    #[test]
    fn orac_state_register_multiple_sessions() {
        let state = OracState::new(PvConfig::default());
        state.register_session("sess-1".into(), PaneId::new("alpha"));
        state.register_session("sess-2".into(), PaneId::new("beta"));
        assert_eq!(state.session_count(), 2);
    }

    #[test]
    fn orac_state_remove_session() {
        let state = OracState::new(PvConfig::default());
        state.register_session("sess-1".into(), PaneId::new("alpha"));
        let tracker = state.remove_session("sess-1");
        assert!(tracker.is_some());
        assert_eq!(tracker.unwrap().pane_id.as_str(), "alpha");
        assert_eq!(state.session_count(), 0);
    }

    #[test]
    fn orac_state_remove_nonexistent_session() {
        let state = OracState::new(PvConfig::default());
        assert!(state.remove_session("nope").is_none());
    }

    #[test]
    fn orac_state_increment_tick() {
        let state = OracState::new(PvConfig::default());
        assert_eq!(state.increment_tick(), 0);
        assert_eq!(state.increment_tick(), 1);
        assert_eq!(state.increment_tick(), 2);
    }

    // ── SessionTracker ──

    #[test]
    fn session_tracker_no_active_task() {
        let tracker = SessionTracker {
            pane_id: PaneId::new("test"),
            active_task_id: None,
            active_task_claimed_ms: None,
            poll_counter: 0,
            total_tool_calls: 0,
            started_ms: 0,
            persona: String::new(),
        };
        assert!(tracker.active_task_id.is_none());
        assert!(tracker.active_task_claimed_ms.is_none());
        assert_eq!(tracker.poll_counter, 0);
        assert_eq!(tracker.total_tool_calls, 0);
    }

    #[test]
    fn session_tracker_with_active_task() {
        let tracker = SessionTracker {
            pane_id: PaneId::new("test"),
            active_task_id: Some("task-42".into()),
            active_task_claimed_ms: Some(1_742_600_000_000),
            poll_counter: 5,
            total_tool_calls: 87,
            started_ms: 1_742_600_000_000,
            persona: "/home/user/project".into(),
        };
        assert_eq!(tracker.active_task_id.as_deref(), Some("task-42"));
        assert_eq!(tracker.active_task_claimed_ms, Some(1_742_600_000_000));
        assert_eq!(tracker.poll_counter, 5);
        assert_eq!(tracker.total_tool_calls, 87);
        assert_eq!(tracker.persona, "/home/user/project");
    }

    #[test]
    fn session_tracker_register_populates_started_ms() {
        let state = OracState::new(PvConfig::default());
        state.register_session("sess-t".into(), PaneId::new("test"));
        let sessions = state.sessions.read();
        let tracker = sessions.get("sess-t").unwrap();
        assert!(tracker.started_ms > 0);
        assert!(tracker.persona.is_empty());
    }

    // ── HealthResponse ──

    #[test]
    fn health_response_serializes() {
        let resp = HealthResponse {
            status: "healthy",
            service: "orac-sidecar",
            port: 8133,
            sessions: 3,
            uptime_ticks: 42,
            #[cfg(feature = "evolution")]
            ralph_gen: 7,
            #[cfg(feature = "evolution")]
            ralph_phase: "Recognize".into(),
            #[cfg(feature = "evolution")]
            ralph_fitness: 0.85,
            ipc_state: "disconnected".into(),
            #[cfg(feature = "intelligence")]
            breakers: serde_json::json!({}),
            dispatch_total: 0,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("orac-sidecar"));
        assert!(json.contains("8133"));
    }

    #[cfg(feature = "evolution")]
    #[test]
    fn health_response_includes_ralph_fields() {
        let resp = HealthResponse {
            status: "healthy",
            service: "orac-sidecar",
            port: 8133,
            sessions: 1,
            uptime_ticks: 100,
            ralph_gen: 42,
            ralph_phase: "Analyze".into(),
            ralph_fitness: 0.667,
            ipc_state: "connected".into(),
            #[cfg(feature = "intelligence")]
            breakers: serde_json::json!({"pv2": "Closed", "synthex": "Closed"}),
            dispatch_total: 5,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("ralph_gen"));
        assert!(json.contains("42"));
        assert!(json.contains("Analyze"));
        assert!(json.contains("0.667"));
    }

    // ── generate_pane_id ──

    #[test]
    fn generate_pane_id_contains_pid() {
        let id = generate_pane_id();
        let pid_str = format!("{}", std::process::id());
        assert!(id.as_str().contains(&pid_str));
    }

    #[test]
    fn generate_pane_id_not_empty() {
        let id = generate_pane_id();
        assert!(!id.as_str().is_empty());
    }

    // ── Router construction ──

    #[test]
    fn build_router_succeeds() {
        let state = Arc::new(OracState::new(PvConfig::default()));
        let _router = build_router(state);
        // If this compiles and runs, the router is valid
    }

    // ── HookResponse default ──

    #[test]
    fn hook_response_default_is_empty() {
        let resp = HookResponse::default();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "{}");
    }

    // ── Edge cases ──

    #[test]
    fn hook_event_all_fields_present() {
        let ev = HookEvent {
            session_id: Some("s".into()),
            tool_name: Some("t".into()),
            tool_input: Some(serde_json::json!({})),
            tool_output: Some("o".into()),
            prompt: Some("p".into()),
        };
        let json = serde_json::to_string(&ev).unwrap();
        let back: HookEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, ev.session_id);
        assert_eq!(back.prompt, ev.prompt);
    }

    #[test]
    fn hook_response_with_all_fields() {
        let resp = HookResponse {
            system_message: Some("msg".into()),
            decision: Some("allow".into()),
            reason: Some("reason".into()),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("systemMessage"));
        assert!(json.contains("decision"));
        assert!(json.contains("reason"));
    }

    #[test]
    fn orac_state_concurrent_session_access() {
        let state = Arc::new(OracState::new(PvConfig::default()));
        let s1 = Arc::clone(&state);
        let s2 = Arc::clone(&state);

        // Simulate concurrent access
        s1.register_session("a".into(), PaneId::new("alpha"));
        s2.register_session("b".into(), PaneId::new("beta"));
        assert_eq!(state.session_count(), 2);

        s1.remove_session("a");
        assert_eq!(state.session_count(), 1);
    }

    // ── Ghost traces (FIX-019) ──

    fn make_ghost(id: &str, tools: u64, duration_ms: u64) -> OracGhost {
        OracGhost {
            sphere_id: id.into(),
            persona: format!("/home/user/{id}"),
            deregistered_ms: epoch_ms(),
            final_phase: 1.57,
            total_tools: tools,
            session_duration_ms: duration_ms,
        }
    }

    #[test]
    fn add_ghost_basic() {
        let state = OracState::new(PvConfig::default());
        state.add_ghost(make_ghost("alpha", 10, 1000));
        assert_eq!(state.get_ghosts().len(), 1);
    }

    #[test]
    fn add_ghost_preserves_fields() {
        let state = OracState::new(PvConfig::default());
        state.add_ghost(make_ghost("alpha", 87, 1_800_000));
        let ghosts = state.get_ghosts();
        assert_eq!(ghosts[0].sphere_id, "alpha");
        assert_eq!(ghosts[0].total_tools, 87);
        assert_eq!(ghosts[0].session_duration_ms, 1_800_000);
        assert!((ghosts[0].final_phase - 1.57).abs() < f64::EPSILON);
    }

    #[test]
    fn add_ghost_fifo_eviction() {
        let state = OracState::new(PvConfig::default());
        for i in 0..25 {
            state.add_ghost(make_ghost(&format!("g{i}"), i as u64, 1000));
        }
        let ghosts = state.get_ghosts();
        assert_eq!(ghosts.len(), MAX_GHOSTS);
        assert_eq!(ghosts[0].sphere_id, "g5");
        assert_eq!(ghosts[MAX_GHOSTS - 1].sphere_id, "g24");
    }

    #[test]
    fn add_ghost_preserves_order() {
        let state = OracState::new(PvConfig::default());
        state.add_ghost(make_ghost("first", 1, 100));
        state.add_ghost(make_ghost("second", 2, 200));
        let ghosts = state.get_ghosts();
        assert_eq!(ghosts[0].sphere_id, "first");
        assert_eq!(ghosts[1].sphere_id, "second");
    }

    #[test]
    fn ghost_empty_initially() {
        let state = OracState::new(PvConfig::default());
        assert!(state.get_ghosts().is_empty());
    }

    #[test]
    fn ghost_serializes_correctly() {
        let g = make_ghost("ser-test", 42, 3_600_000);
        let json = serde_json::to_string(&g).unwrap();
        assert!(json.contains("sphere_id"));
        assert!(json.contains("final_phase"));
        assert!(json.contains("total_tools"));
        assert!(json.contains("session_duration_ms"));
    }

    #[test]
    fn ghost_concurrent_access() {
        let state = Arc::new(OracState::new(PvConfig::default()));
        let s1 = Arc::clone(&state);
        let s2 = Arc::clone(&state);
        s1.add_ghost(make_ghost("a", 10, 1000));
        s2.add_ghost(make_ghost("b", 20, 2000));
        assert_eq!(state.get_ghosts().len(), 2);
    }

    #[test]
    fn max_ghosts_is_20() {
        assert_eq!(MAX_GHOSTS, 20);
    }

    // ── Consent (FIX-018) ──

    #[test]
    fn consent_fully_open_defaults() {
        let c = OracConsent::fully_open();
        assert!(c.synthex_write);
        assert!(c.povm_read);
        assert!(!c.povm_write);
        assert!(c.hydration);
        assert!(c.updated_ms > 0);
    }

    #[test]
    fn consent_serializes() {
        let c = OracConsent::fully_open();
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains("synthex_write"));
        assert!(json.contains("povm_read"));
    }

    #[test]
    fn consent_deserializes() {
        let json = r#"{"synthex_write":false,"povm_read":true,"povm_write":true,"hydration":false,"updated_ms":1000}"#;
        let c: OracConsent = serde_json::from_str(json).unwrap();
        assert!(!c.synthex_write);
        assert!(c.povm_read);
        assert!(c.povm_write);
        assert!(!c.hydration);
        assert_eq!(c.updated_ms, 1000);
    }

    #[test]
    fn get_consent_default_fully_open() {
        let state = OracState::new(PvConfig::default());
        let c = state.get_consent("sphere-1");
        assert!(c.synthex_write);
        assert!(c.povm_read);
        assert!(!c.povm_write);
        assert!(c.hydration);
    }

    #[test]
    fn update_consent_single_field() {
        let state = OracState::new(PvConfig::default());
        let req = ConsentUpdateRequest {
            synthex_write: Some(false),
            povm_read: None,
            povm_write: None,
            hydration: None,
        };
        let updated = state.update_consent("sphere-1", &req);
        assert_eq!(updated, vec!["synthex_write"]);
        let c = state.get_consent("sphere-1");
        assert!(!c.synthex_write);
        assert!(c.povm_read);
    }

    #[test]
    fn update_consent_multiple_fields() {
        let state = OracState::new(PvConfig::default());
        let req = ConsentUpdateRequest {
            synthex_write: Some(false),
            povm_read: None,
            povm_write: Some(true),
            hydration: Some(false),
        };
        let updated = state.update_consent("sphere-1", &req);
        assert_eq!(updated.len(), 3);
        assert!(updated.contains(&"synthex_write"));
        assert!(updated.contains(&"povm_write"));
        assert!(updated.contains(&"hydration"));
    }

    #[test]
    fn update_consent_empty_no_changes() {
        let state = OracState::new(PvConfig::default());
        let req = ConsentUpdateRequest {
            synthex_write: None,
            povm_read: None,
            povm_write: None,
            hydration: None,
        };
        let updated = state.update_consent("sphere-1", &req);
        assert!(updated.is_empty());
    }

    #[test]
    fn update_consent_preserves_other_fields() {
        let state = OracState::new(PvConfig::default());
        let req1 = ConsentUpdateRequest {
            synthex_write: None,
            povm_read: None,
            povm_write: Some(true),
            hydration: None,
        };
        state.update_consent("sphere-1", &req1);
        let req2 = ConsentUpdateRequest {
            synthex_write: Some(false),
            povm_read: None,
            povm_write: None,
            hydration: None,
        };
        state.update_consent("sphere-1", &req2);
        let c = state.get_consent("sphere-1");
        assert!(!c.synthex_write);
        assert!(c.povm_write);
    }

    #[test]
    fn consent_per_sphere_isolation() {
        let state = OracState::new(PvConfig::default());
        let req = ConsentUpdateRequest {
            synthex_write: Some(false),
            povm_read: None,
            povm_write: None,
            hydration: None,
        };
        state.update_consent("sphere-a", &req);
        let c = state.get_consent("sphere-b");
        assert!(c.synthex_write);
    }

    #[test]
    fn consent_concurrent_access() {
        let state = Arc::new(OracState::new(PvConfig::default()));
        let s1 = Arc::clone(&state);
        let s2 = Arc::clone(&state);
        let req1 = ConsentUpdateRequest {
            synthex_write: Some(false),
            povm_read: None,
            povm_write: None,
            hydration: None,
        };
        let req2 = ConsentUpdateRequest {
            synthex_write: None,
            povm_read: None,
            povm_write: Some(true),
            hydration: None,
        };
        s1.update_consent("sphere-1", &req1);
        s2.update_consent("sphere-2", &req2);
        assert!(!state.get_consent("sphere-1").synthex_write);
        assert!(state.get_consent("sphere-2").povm_write);
    }

    #[test]
    fn consents_empty_initially() {
        let state = OracState::new(PvConfig::default());
        assert!(state.consents.read().is_empty());
    }

    // ── consent_allows ──

    #[test]
    fn consent_allows_default_all_open() {
        let state = OracState::new(PvConfig::default());
        assert!(state.consent_allows("unknown-sphere", "synthex_write"));
        assert!(state.consent_allows("unknown-sphere", "povm_read"));
        assert!(!state.consent_allows("unknown-sphere", "povm_write"));
        assert!(state.consent_allows("unknown-sphere", "hydration"));
    }

    #[test]
    fn consent_allows_respects_updates() {
        let state = OracState::new(PvConfig::default());
        let req = ConsentUpdateRequest {
            synthex_write: Some(false),
            povm_read: None,
            povm_write: None,
            hydration: Some(false),
        };
        state.update_consent("sphere-x", &req);
        assert!(!state.consent_allows("sphere-x", "synthex_write"));
        assert!(state.consent_allows("sphere-x", "povm_read"));
        assert!(!state.consent_allows("sphere-x", "hydration"));
    }

    #[test]
    fn consent_allows_unknown_field_defaults_true() {
        let state = OracState::new(PvConfig::default());
        assert!(state.consent_allows("any", "unknown_field"));
    }

    #[test]
    fn consent_allows_per_sphere_isolation() {
        let state = OracState::new(PvConfig::default());
        let req = ConsentUpdateRequest {
            synthex_write: None,
            povm_read: None,
            povm_write: None,
            hydration: Some(false),
        };
        state.update_consent("restricted", &req);
        assert!(!state.consent_allows("restricted", "hydration"));
        assert!(state.consent_allows("other-sphere", "hydration"));
    }

    // ── epoch_ms ──

    #[test]
    fn epoch_ms_is_reasonable() {
        let ms = epoch_ms();
        assert!(ms > 1_577_836_800_000);
        assert!(ms < 1_893_456_000_000);
    }

    // ── Response format validation ──

    #[test]
    fn consent_get_response_format() {
        let consent = OracConsent::fully_open();
        let response = serde_json::json!({
            "sphere_id": "test",
            "consents": {
                "synthex_write": consent.synthex_write,
                "povm_read": consent.povm_read,
                "povm_write": consent.povm_write,
                "hydration": consent.hydration,
            },
            "updated_ms": consent.updated_ms,
        });
        assert!(response["consents"]["synthex_write"].as_bool().unwrap());
        assert!(!response["consents"]["povm_write"].as_bool().unwrap());
    }

    #[test]
    fn consent_put_response_format() {
        let updated = vec!["synthex_write", "povm_write"];
        let response = serde_json::json!({
            "status": "ok",
            "updated": updated,
        });
        assert_eq!(response["status"], "ok");
        assert_eq!(response["updated"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn ghost_list_response_format() {
        let g = make_ghost("resp-test", 87, 1_800_000);
        let response = serde_json::json!({
            "ghosts": [{
                "sphere_id": g.sphere_id,
                "persona": g.persona,
                "deregistered_ms": g.deregistered_ms,
                "final_phase": g.final_phase,
                "total_tools": g.total_tools,
                "session_duration_ms": g.session_duration_ms,
            }]
        });
        let arr = response["ghosts"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["sphere_id"], "resp-test");
        assert_eq!(arr[0]["total_tools"], 87);
    }

    #[test]
    fn session_duration_calculation() {
        let started = 1_742_600_000_000_u64;
        let ended = 1_742_601_800_000_u64;
        assert_eq!(ended.saturating_sub(started), 1_800_000);
    }

    // ── IPC state ──

    #[test]
    fn ipc_state_defaults_disconnected() {
        let state = OracState::new(PvConfig::default());
        assert_eq!(state.get_ipc_state(), "disconnected");
    }

    #[test]
    fn set_ipc_state_updates() {
        let state = OracState::new(PvConfig::default());
        state.set_ipc_state("connected");
        assert_eq!(state.get_ipc_state(), "connected");
    }

    #[test]
    fn set_ipc_state_overwrites() {
        let state = OracState::new(PvConfig::default());
        state.set_ipc_state("connecting");
        state.set_ipc_state("connected");
        assert_eq!(state.get_ipc_state(), "connected");
    }

    // ── Emergence data via RALPH ──

    #[cfg(feature = "evolution")]
    #[test]
    fn emergence_detector_accessible_from_orac_state() {
        let state = OracState::new(PvConfig::default());
        let detector = state.ralph.emergence();
        assert_eq!(detector.history_len(), 0);
        assert_eq!(detector.active_monitor_count(), 0);
    }

    #[cfg(feature = "evolution")]
    #[test]
    fn emergence_stats_serializable() {
        let state = OracState::new(PvConfig::default());
        let detector = state.ralph.emergence();
        let stats = detector.stats();
        let json = serde_json::to_value(&serde_json::json!({
            "total_detected": stats.total_detected,
            "active_monitors": detector.active_monitor_count(),
            "history_len": detector.history_len(),
            "by_type": stats.by_type,
        }));
        assert!(json.is_ok());
        let v = json.unwrap_or_default();
        assert_eq!(v["total_detected"], 0);
        assert_eq!(v["history_len"], 0);
    }

    #[cfg(feature = "evolution")]
    #[test]
    fn emergence_recent_empty_on_fresh_engine() {
        let state = OracState::new(PvConfig::default());
        let recent = state.ralph.emergence().recent(5);
        assert!(recent.is_empty());
    }

    #[cfg(feature = "evolution")]
    #[test]
    fn emergence_record_serializes_for_field_response() {
        use crate::m8_evolution::m37_emergence_detector::{
            EmergenceRecord, EmergenceSeverity, EmergenceType,
        };
        let record = EmergenceRecord {
            id: 1,
            emergence_type: EmergenceType::BeneficialSync,
            confidence: 0.95,
            severity: 0.3,
            severity_class: EmergenceSeverity::Low,
            affected_panes: vec!["alpha".into()],
            description: "Fleet reached r=0.97".into(),
            detected_at_tick: 42,
            ttl: 500,
            recommended_action: None,
        };
        let json = serde_json::json!({
            "type": record.emergence_type.to_string(),
            "severity": record.severity_class.to_string(),
            "confidence": record.confidence,
            "description": record.description,
            "detected_at_tick": record.detected_at_tick,
            "ttl": record.ttl,
        });
        assert_eq!(json["type"], "beneficial_sync");
        assert_eq!(json["severity"], "low");
        assert_eq!(json["confidence"], 0.95);
        assert_eq!(json["detected_at_tick"], 42);
    }

    // ── RALPH Prometheus metrics ──

    #[cfg(feature = "evolution")]
    #[test]
    fn ralph_metrics_format_valid_prometheus() {
        use std::fmt::Write;
        let state = OracState::new(PvConfig::default());
        let ralph_state = state.ralph.state();
        let ralph_stats = state.ralph.stats();

        let mut body = String::new();
        let _ = write!(
            body,
            "orac_ralph_generation {}\norac_ralph_fitness {:.4}\n",
            ralph_state.generation, ralph_state.current_fitness,
        );
        // Prometheus lines: metric_name value\n
        assert!(body.contains("orac_ralph_generation 0"));
        assert!(body.contains("orac_ralph_fitness"));
        // No empty metric names or malformed lines
        for line in body.lines() {
            if !line.is_empty() && !line.starts_with('#') {
                assert!(
                    line.contains(' '),
                    "prometheus line must have space between name and value: {line}",
                );
            }
        }
        // Stats should be zero on fresh engine
        assert_eq!(ralph_stats.total_proposed, 0);
        assert_eq!(ralph_stats.total_accepted, 0);
        assert_eq!(ralph_stats.total_rolled_back, 0);
    }

    #[cfg(feature = "evolution")]
    #[test]
    fn ralph_metrics_mutation_counters_sum() {
        use crate::m8_evolution::m39_fitness_tensor::TensorValues;
        let engine = crate::m8_evolution::m36_ralph_engine::RalphEngine::new();
        let tensor = TensorValues::uniform(0.5);

        // Run through 10 ticks
        for tick in 0..10 {
            let _ = engine.tick(&tensor, tick);
        }

        let stats = engine.stats();
        // proposed = accepted + rolled_back (skipped are separate)
        let outcome_total = stats.total_accepted + stats.total_rolled_back;
        assert!(
            stats.total_proposed >= outcome_total,
            "proposed ({}) must >= accepted+rolled_back ({})",
            stats.total_proposed, outcome_total,
        );
    }
}
