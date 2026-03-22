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
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::m1_core::field_state::{self, SharedState};
use crate::m1_core::m01_core_types::PaneId;
use crate::m1_core::m03_config::PvConfig;

#[cfg(feature = "evolution")]
use crate::m8_evolution::m36_ralph_engine::RalphEngine;
#[cfg(feature = "persistence")]
use crate::m5_bridges::m26_blackboard::Blackboard;

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
            ghosts: RwLock::new(VecDeque::new()),
            consents: RwLock::new(HashMap::new()),
            #[cfg(feature = "persistence")]
            blackboard: None,
            #[cfg(feature = "evolution")]
            ralph: RalphEngine::new(),
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
            ghosts: RwLock::new(VecDeque::new()),
            consents: RwLock::new(HashMap::new()),
            #[cfg(feature = "persistence")]
            blackboard: None,
            #[cfg(feature = "evolution")]
            ralph: RalphEngine::new(),
        }
    }

    /// Register a new session.
    pub fn register_session(&self, session_id: String, pane_id: PaneId) {
        let tracker = SessionTracker {
            pane_id,
            active_task_id: None,
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

    /// Record a ghost trace for a deregistered sphere.
    ///
    /// Maintains a FIFO ring of the last 20 ghost entries.
    pub fn add_ghost(&self, ghost: OracGhost) {
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

    /// Update consent fields for a sphere. Returns list of updated field names.
    pub fn update_consent(
        &self,
        sphere_id: &str,
        update: &ConsentUpdateRequest,
    ) -> Vec<&'static str> {
        let mut updated = Vec::new();
        let mut guard = self.consents.write();
        let consent = guard
            .entry(sphere_id.to_owned())
            .or_insert_with(OracConsent::fully_open);

        if let Some(v) = update.synthex_write {
            consent.synthex_write = v;
            updated.push("synthex_write");
        }
        if let Some(v) = update.povm_read {
            consent.povm_read = v;
            updated.push("povm_read");
        }
        if let Some(v) = update.povm_write {
            consent.povm_write = v;
            updated.push("povm_write");
        }
        if let Some(v) = update.hydration {
            consent.hydration = v;
            updated.push("hydration");
        }
        if !updated.is_empty() {
            consent.updated_ms = epoch_ms();
        }
        updated
    }
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

/// Spawn a background task that polls PV2 `/health` every 5s
/// and updates `SharedState` with the latest field state.
///
/// Runs until the process shuts down. If PV2 is unreachable,
/// the cached state remains unchanged (graceful degradation).
pub fn spawn_field_poller(state: Arc<OracState>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            let health_url = format!("{}/health", state.pv2_url);
            let Some(health_json) = http_get(&health_url, 2000).await else {
                tracing::debug!("field poller: PV2 unreachable");
                continue;
            };

            let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&health_json) else {
                continue;
            };

            let r = parsed.get("r").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
            let pv2_tick = parsed.get("tick").and_then(serde_json::Value::as_u64).unwrap_or(0);

            // Update SharedState — minimal lock scope
            {
                let mut guard = state.field_state.write();
                guard.field.order.r = r.clamp(0.0, 1.0);
                guard.field.order_parameter.r = r.clamp(0.0, 1.0);
                guard.field.tick = pv2_tick;
                guard.tick = pv2_tick;
                guard.push_r(r);
                guard.tick_warmup();
            }

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
    Json(HealthResponse {
        status: "healthy",
        service: "orac-sidecar",
        port: state.config.server.port,
        sessions: state.session_count(),
        uptime_ticks: state.tick.load(Ordering::Relaxed),
    })
}

/// Field state handler — proxies current Kuramoto field from PV2.
///
/// Returns the live field state including `r`, `K`, spheres, and chimera detection.
/// Falls back to cached state if PV2 is unreachable.
async fn field_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
    let pv2_url = format!("{}/health", state.pv2_url);
    let spheres_url = format!("{}/spheres", state.pv2_url);

    let health = http_get(&pv2_url, 2000).await;
    let spheres = http_get(&spheres_url, 2000).await;

    let mut result = serde_json::json!({
        "source": "pv2_proxy",
        "tick": state.tick.load(Ordering::Relaxed),
    });

    if let Some(h) = health {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&h) {
            result["r"] = v.get("r").cloned().unwrap_or_default();
            result["k"] = v.get("k").cloned().unwrap_or_default();
            result["k_mod"] = v.get("k_mod").cloned().unwrap_or_default();
            result["sphere_count"] = v.get("spheres").cloned().unwrap_or_default();
            result["pv2_tick"] = v.get("tick").cloned().unwrap_or_default();
        }
    }

    if let Some(s) = spheres {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            result["spheres"] = v;
        }
    }

    Json(result)
}

/// Blackboard handler — returns current session and fleet state.
///
/// Provides a read-only view of ORAC's tracked sessions.
async fn blackboard_handler(
    State(state): State<Arc<OracState>>,
) -> Json<serde_json::Value> {
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

    Json(serde_json::json!({
        "sessions": session_list,
        "fleet_size": session_list.len(),
        "uptime_ticks": state.tick.load(Ordering::Relaxed),
    }))
}

/// Metrics handler — Prometheus-compatible text format.
///
/// Reports sessions, uptime, and bridge connectivity.
async fn metrics_handler(
    State(state): State<Arc<OracState>>,
) -> (axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], String) {
    let sessions = state.session_count();
    let uptime = state.tick.load(Ordering::Relaxed);

    let body = format!(
        "# HELP orac_sessions_active Active session count\n\
         # TYPE orac_sessions_active gauge\n\
         orac_sessions_active {sessions}\n\
         \n\
         # HELP orac_uptime_ticks Server uptime in ticks\n\
         # TYPE orac_uptime_ticks counter\n\
         orac_uptime_ticks {uptime}\n"
    );

    (
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
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

/// Start the ORAC HTTP hook server.
///
/// Binds to the configured address and port, then serves requests
/// until the shutdown signal is received.
///
/// # Errors
///
/// Returns an error if the server cannot bind to the configured address.
pub async fn start_server(state: Arc<OracState>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let addr = SocketAddr::new(
        state
            .config
            .server
            .bind_addr
            .parse()
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
        state.config.server.port,
    );

    let router = build_router(Arc::clone(&state));

    tracing::info!(%addr, "ORAC hook server starting");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("ORAC hook server shutting down");
        })
        .await?;

    Ok(())
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
            poll_counter: 0,
            total_tool_calls: 0,
            started_ms: 0,
            persona: String::new(),
        };
        assert!(tracker.active_task_id.is_none());
        assert_eq!(tracker.poll_counter, 0);
        assert_eq!(tracker.total_tool_calls, 0);
    }

    #[test]
    fn session_tracker_with_active_task() {
        let tracker = SessionTracker {
            pane_id: PaneId::new("test"),
            active_task_id: Some("task-42".into()),
            poll_counter: 5,
            total_tool_calls: 87,
            started_ms: 1_742_600_000_000,
            persona: "/home/user/project".into(),
        };
        assert_eq!(tracker.active_task_id.as_deref(), Some("task-42"));
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
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("orac-sidecar"));
        assert!(json.contains("8133"));
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
}
