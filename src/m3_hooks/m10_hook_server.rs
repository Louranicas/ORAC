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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::field_state::{self, SharedState};
use crate::m1_core::m01_core_types::PaneId;
use crate::m1_core::m03_config::PvConfig;

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
        }
    }

    /// Register a new session.
    pub fn register_session(&self, session_id: String, pane_id: PaneId) {
        let tracker = SessionTracker {
            pane_id,
            active_task_id: None,
            poll_counter: 0,
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
        };
        assert!(tracker.active_task_id.is_none());
        assert_eq!(tracker.poll_counter, 0);
    }

    #[test]
    fn session_tracker_with_active_task() {
        let tracker = SessionTracker {
            pane_id: PaneId::new("test"),
            active_task_id: Some("task-42".into()),
            poll_counter: 5,
        };
        assert_eq!(tracker.active_task_id.as_deref(), Some("task-42"));
        assert_eq!(tracker.poll_counter, 5);
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
}
