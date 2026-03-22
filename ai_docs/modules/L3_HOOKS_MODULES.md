---
title: "Layer 3: Hooks — Module Documentation"
date: 2026-03-22
tags: [modules, hooks, L3, orac-sidecar]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L3
modules: [m10, m11, m12, m13, m14]
---

# Layer 3: Hooks (m10-m14)

> THE KEYSTONE LAYER. Replaces all 8 bash hook scripts with sub-ms HTTP endpoints.
> Claude Code sends HTTP POST to `http://localhost:8133/hooks/{event}`.
> **Target LOC:** ~2,500 | **Target tests:** 50+
> **Source:** ALL NEW (no PV2 equivalent) | **Phase:** 1

---

## Overview

L3 is the first layer a Claude Code instance touches. Every tool call, every permission
request, every session lifecycle event flows through these 5 modules. The hook server
receives HTTP POST requests from Claude Code's native hook system and translates them
into PV2 sphere operations via the L2 Wire layer.

**Implementation order:** m10 (server skeleton) -> m14 (permission policy, standalone logic)
-> m11 (session lifecycle) -> m13 (prompt injection) -> m12 (tool hooks, needs L4).

**Feature gate:** `#[cfg(feature = "api")]`

**Design invariant:** All handlers return within 1ms. No external I/O in the hot path.
Local memory lookups only. The thermal gate (PreToolUse) queries a cached SYNTHEX
reading, never a live HTTP call.

### Hook Flow

```text
Claude Code  --POST-->  :8133/hooks/{event}  --handler-->  IPC to PV2  --response-->  Claude Code
                              |                                  |
                        m10 routing               m11/m12/m13/m14 logic
                        body parsing               sphere register/update
                        64KB limit                 field state query
```

### Response Schema

All endpoints return the same envelope:

```json
{
  "decision": "approve" | "deny" | "skip",
  "reason": "optional human-readable explanation",
  "inject": { "field_state": { "r": 0.987, "k": 2.41, "spheres": 6 } }
}
```

---

## m10 -- Hook Server

**Source:** `src/m3_hooks/m10_hook_server.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types` (`PaneId`, `Timestamp`), `m02_error_handling` (`PvError`), `m03_config` (`PvConfig`)
**Hot-Swap:** NEW (ORAC-specific, no PV2 equivalent)

### Design Decisions

- **Axum over Actix:** Axum integrates natively with `tower` middleware (rate limiting,
  tracing, timeout). Tower is also used by the circuit breaker (m21). One ecosystem.
- **Shared state via `Arc<SharedState>`:** Injected into all handlers via Axum's
  `Extension`. `SharedState` wraps `FieldState`, IPC client handle, and permission
  policy config behind `parking_lot::RwLock` (P02).
- **Body size limit:** 64KB hard cap via `tower_http::limit::RequestBodyLimitLayer`.
  Claude Code hook payloads are typically 2-8KB; 64KB provides margin for tool output
  in `PostToolUse` without allowing abuse.
- **Graceful shutdown:** `tokio::signal::ctrl_c()` + `axum::Server::with_graceful_shutdown`.
  On SIGTERM, the Stop handler fires for any active session before socket close.
- **Sub-1ms response (P14):** No I/O in handlers. All state is local. IPC sends are
  fire-and-forget (buffered channel). The response is computed from cached `FieldState`.

### Types to Implement

```rust
/// Hook event types supported by the ORAC sidecar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    /// New Claude Code session starting.
    SessionStart,
    /// Tool about to be invoked (pre-flight check).
    PreToolUse,
    /// Tool invocation completed.
    PostToolUse,
    /// User prompt submitted (before model processes it).
    UserPromptSubmit,
    /// Session ending.
    Stop,
    /// Permission requested for a tool/action.
    PermissionRequest,
}

/// Inbound hook request from Claude Code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRequest {
    /// Unique hook invocation ID (idempotency key, P16).
    pub hook_id: String,
    /// Which hook event triggered this request.
    pub event: HookEvent,
    /// Session identifier (maps to `PaneId`).
    pub session_id: String,
    /// Event-specific payload (tool name, prompt text, etc.).
    pub payload: serde_json::Value,
    /// Monotonic timestamp (millis since epoch).
    pub timestamp: u64,
}

/// Outbound hook response to Claude Code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookResponse {
    /// Decision: approve, deny, or skip.
    pub decision: HookDecision,
    /// Human-readable reason (for deny/skip).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional context to inject into the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inject: Option<serde_json::Value>,
}

/// Hook decision enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecision {
    /// Allow the action to proceed.
    Approve,
    /// Block the action.
    Deny,
    /// Neither approve nor deny -- let Claude Code decide.
    Skip,
}

/// Shared application state injected into all Axum handlers.
pub struct HookServerState {
    /// Cached field state from PV2 daemon.
    pub field: Arc<RwLock<FieldState>>,
    /// IPC client for PV2 communication.
    pub ipc: Arc<IpcClient>,
    /// Permission policy configuration.
    pub policy: Arc<RwLock<PermissionPolicy>>,
    /// Idempotency cache: hook_id -> response (bounded, LRU).
    pub idempotency: Arc<RwLock<HashMap<String, HookResponse>>>,
    /// Server configuration.
    pub config: Arc<PvConfig>,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `build_router` | `fn build_router(state: Arc<HookServerState>) -> Router` | Registers all 6 routes + middleware |
| `serve` | `async fn serve(config: &PvConfig, state: Arc<HookServerState>) -> PvResult<()>` | Binds `:8133`, graceful shutdown |
| `health_check` | `async fn health_check() -> impl IntoResponse` | `GET /health` -- returns 200 + uptime |
| `check_idempotency` | `fn check_idempotency(state: &HookServerState, hook_id: &str) -> Option<HookResponse>` | Returns cached response if hook_id seen (P16) |

### Tests

| Test | Validates |
|------|-----------|
| `test_router_registers_all_6_routes` | All endpoints respond (not 404) |
| `test_body_size_limit_64kb` | Payloads > 64KB rejected with 413 |
| `test_unknown_event_returns_skip` | Unrecognized event name -> skip decision |
| `test_idempotency_same_hook_id` | Same hook_id returns identical response (P16) |
| `test_malformed_json_returns_400` | Invalid JSON body -> 400 Bad Request |
| `test_health_endpoint_returns_200` | `/health` always returns 200 |
| `test_graceful_shutdown_drains` | In-flight requests complete before shutdown |
| `test_missing_session_id_returns_deny` | No session_id field -> deny |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] HTTP Hook Server
- [[Session 045 Arena -- 02-api-wiring-map]]
- `.claude/schemas/hook_request.json`, `.claude/schemas/hook_response.json`
- ORAC_PLAN.md Phase 1 Detail (steps 4, 11)
- ORAC_MINDMAP.md Branch 1 (HTTP Hook Server)
- **P14** (sub-1ms response), **P16** (idempotency), **AP18** (thermal fail-open)

---

## m11 -- Session Hooks

**Source:** `src/m3_hooks/m11_session_hooks.rs`
**LOC Target:** ~400
**Depends on:** `m10_hook_server` (`HookRequest`, `HookResponse`, `HookServerState`), `m07_ipc_client` (`IpcClient`), `m01_core_types` (`PaneId`, `PaneSphere`, `PaneStatus`)
**Hot-Swap:** NEW

### Design Decisions

- **SessionStart registers a sphere on PV2.** The sidecar translates the session_id
  into a `PaneId` and sends a `ClientFrame::Register` over IPC. If PV2 is unreachable,
  the handler still returns `approve` with a degraded flag -- sessions must never be
  blocked by PV2 downtime.
- **Stop performs a quality gate before deregistration.** Checks: (1) were any tasks
  left incomplete? (2) what was the session's STDP contribution? (3) log summary to
  reasoning memory. Then sends `ClientFrame::Deregister`.
- **Idempotent registration (P16):** If a `SessionStart` arrives for an already-registered
  sphere, return the cached field state without re-registering.

### Types to Implement

```rust
/// Session start hook payload (extracted from `HookRequest.payload`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartPayload {
    /// Persona name for the sphere (e.g. "architect", "fleet-alpha").
    pub persona: Option<String>,
    /// Initial consent posture.
    pub consent: Option<ConsentPosture>,
    /// Working directory of the Claude Code instance.
    pub cwd: Option<String>,
}

/// Consent posture declared at session start.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConsentPosture {
    /// Full coupling participation.
    Full,
    /// Read-only: observe field but do not contribute.
    ReadOnly,
    /// Opt out of all coupling.
    OptOut,
}

/// Session stop hook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStopPayload {
    /// Reason for stopping (user-initiated, timeout, error).
    pub reason: Option<String>,
    /// Final tool count for this session.
    pub tool_count: Option<u32>,
}

/// Quality gate result computed at session stop.
#[derive(Debug, Clone, Serialize)]
pub struct QualityGateResult {
    /// Number of incomplete tasks at session end.
    pub incomplete_tasks: usize,
    /// Total Hebbian weight contributed by this sphere.
    pub stdp_contribution: f64,
    /// Session duration in seconds.
    pub duration_secs: u64,
    /// Final order parameter when session ended.
    pub final_r: f64,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `handle_session_start` | `async fn handle_session_start(State(s): State<Arc<HookServerState>>, Json(req): Json<HookRequest>) -> impl IntoResponse` | Register sphere, return field state |
| `handle_stop` | `async fn handle_stop(State(s): State<Arc<HookServerState>>, Json(req): Json<HookRequest>) -> impl IntoResponse` | Quality gate + deregister |
| `quality_gate` | `fn quality_gate(state: &HookServerState, pane_id: &PaneId) -> QualityGateResult` | Compute session quality metrics |
| `extract_persona` | `fn extract_persona(payload: &serde_json::Value) -> String` | Extract or generate persona name |

### Tests

| Test | Validates |
|------|-----------|
| `test_session_start_registers_sphere` | IPC register message sent with correct `PaneId` |
| `test_session_start_returns_field_state` | Response includes current r, K, sphere count |
| `test_session_start_idempotent` | Duplicate session_id does not re-register |
| `test_session_start_with_persona` | Custom persona propagates to sphere |
| `test_session_start_pv2_down_still_approves` | Degraded mode: approve even if IPC fails |
| `test_stop_deregisters_sphere` | IPC deregister message sent |
| `test_stop_quality_gate_computes` | Quality metrics populated correctly |
| `test_stop_with_incomplete_tasks` | Incomplete tasks logged in quality gate |
| `test_consent_posture_propagates` | Consent declared at start reaches PV2 |
| `test_stop_logs_to_reasoning_memory` | Session summary written to RM (TSV!) |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] Session Lifecycle
- [[Consent Flow Analysis]]
- ORAC_PLAN.md Phase 1 Detail (steps 5, 7)
- **P15** (permission cascade), **P16** (idempotency)
- m07 IPC client for PV2 registration wire protocol

---

## m12 -- Tool Hooks

**Source:** `src/m3_hooks/m12_tool_hooks.rs`
**LOC Target:** ~600
**Depends on:** `m10_hook_server` (types), `m07_ipc_client`, `m18_hebbian_stdp` (STDP trigger), `m01_core_types` (`PaneId`, `PaneStatus`), `m04_constants` (STDP parameters)
**Hot-Swap:** NEW

### Design Decisions

- **PostToolUse is the STDP trigger point.** Every completed tool call fires Hebbian
  learning: the tool name updates the sphere's `WorkSignature`, and co-active sphere
  pairs strengthen coupling weights. This is the highest-frequency hook (dozens per
  minute during active work).
- **PreToolUse is the thermal gate.** Before a tool executes, ORAC checks the cached
  SYNTHEX thermal reading. If thermal load exceeds threshold, the tool is denied
  (soft throttling). **Critical: fails OPEN (AP18).** If the SYNTHEX cache is stale
  or unavailable, the tool is approved -- never block work due to monitoring failure.
- **Task polling on PostToolUse.** After STDP update, check if PV2 has dispatched
  any tasks for this sphere. Returned tasks are injected into the response.
- **Semantic phase tagging (P01).** Each tool name maps to a phase region:
  Read -> 0, Write -> pi/2, Execute -> pi, Communicate -> 3pi/2. This phase tag
  is used by the coupling network for phase-aware STDP.

### Types to Implement

```rust
/// Tool hook payload for `PostToolUse` and `PreToolUse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHookPayload {
    /// Tool name (e.g. "Read", "Bash", "Edit", "Grep").
    pub tool_name: String,
    /// Tool input summary (truncated to 4096 chars, m06 validation).
    pub input_summary: Option<String>,
    /// Tool output summary (truncated, PostToolUse only).
    pub output_summary: Option<String>,
    /// Whether the tool succeeded.
    pub success: Option<bool>,
    /// Duration of tool execution in milliseconds.
    pub duration_ms: Option<u64>,
}

/// Semantic phase region for tool classification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToolPhaseRegion {
    /// Read tools: phase 0.
    Read,
    /// Write tools: phase pi/2.
    Write,
    /// Execute tools: phase pi.
    Execute,
    /// Communicate tools: phase 3*pi/2.
    Communicate,
}

impl ToolPhaseRegion {
    /// Map a tool name to its semantic phase region.
    ///
    /// Unknown tools default to `Execute`.
    #[must_use]
    pub fn from_tool_name(name: &str) -> Self {
        match name {
            "Read" | "Grep" | "Glob" | "WebFetch" | "WebSearch" => Self::Read,
            "Write" | "Edit" | "NotebookEdit" => Self::Write,
            "Bash" | "TaskCreate" | "TaskUpdate" => Self::Execute,
            "Skill" | "mcp__memory__create_entities" => Self::Communicate,
            _ => Self::Execute,
        }
    }

    /// Phase value in radians (P01: always in [0, 2pi)).
    #[must_use]
    pub fn phase_value(self) -> f64 {
        match self {
            Self::Read => 0.0,
            Self::Write => std::f64::consts::FRAC_PI_2,
            Self::Execute => std::f64::consts::PI,
            Self::Communicate => 3.0 * std::f64::consts::FRAC_PI_2,
        }
    }
}

/// Result of PostToolUse processing.
#[derive(Debug, Clone, Serialize)]
pub struct PostToolResult {
    /// STDP update result (if Hebbian learning was triggered).
    pub stdp: Option<StdpResultSummary>,
    /// Tasks dispatched to this sphere by PV2.
    pub dispatched_tasks: Vec<TaskSummary>,
    /// Updated sphere status.
    pub status: PaneStatus,
}

/// Thermal gate result for PreToolUse.
#[derive(Debug, Clone, Serialize)]
pub struct ThermalGateResult {
    /// Whether the tool is allowed to proceed.
    pub allowed: bool,
    /// Current thermal load (0.0 = cold, 1.0 = critical).
    pub thermal_load: f64,
    /// Reason if denied.
    pub reason: Option<String>,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `handle_post_tool_use` | `async fn handle_post_tool_use(State(s): ..., Json(req): ...) -> impl IntoResponse` | STDP + task poll + status update |
| `handle_pre_tool_use` | `async fn handle_pre_tool_use(State(s): ..., Json(req): ...) -> impl IntoResponse` | Thermal gate check |
| `thermal_gate` | `fn thermal_gate(state: &HookServerState) -> ThermalGateResult` | Check cached SYNTHEX reading, fail OPEN (AP18) |
| `classify_tool` | `fn classify_tool(tool_name: &str) -> ToolPhaseRegion` | Map tool name to semantic phase (P01) |
| `trigger_stdp` | `fn trigger_stdp(state: &HookServerState, pane_id: &PaneId, tool: &str)` | Fire Hebbian update for co-active pairs |
| `poll_tasks` | `async fn poll_tasks(ipc: &IpcClient, pane_id: &PaneId) -> Vec<TaskSummary>` | Check for PV2-dispatched tasks |

### Tests

| Test | Validates |
|------|-----------|
| `test_post_tool_use_triggers_stdp` | Hebbian weight updated after tool call |
| `test_post_tool_use_polls_tasks` | Dispatched tasks included in response |
| `test_pre_tool_use_thermal_gate_allows` | Normal thermal load -> approve |
| `test_pre_tool_use_thermal_gate_denies` | High thermal load -> deny with reason |
| `test_thermal_gate_fails_open_ap18` | Stale/unavailable SYNTHEX cache -> approve |
| `test_tool_phase_classification` | Read/Write/Execute/Communicate mapping correct |
| `test_tool_phase_wrapping_p01` | Phase values in [0, TAU) |
| `test_unknown_tool_defaults_execute` | Unrecognized tool name -> Execute phase |
| `test_status_updates_to_working` | PostToolUse sets sphere status to Working |
| `test_input_summary_truncated` | Summary > 4096 chars truncated (m06 validation) |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] Tool Hooks
- [[Session 045 Arena -- 10-hebbian-operational-topology]]
- ORAC_PLAN.md Phase 1 Detail (steps 6, 10)
- **P01** (phase wrapping `.rem_euclid(TAU)`), **P14** (sub-1ms), **AP18** (thermal fail-open)
- m18 Hebbian STDP for weight update mechanics
- m22 SYNTHEX bridge for thermal reading source

---

## m13 -- Prompt Hooks

**Source:** `src/m3_hooks/m13_prompt_hooks.rs`
**LOC Target:** ~300
**Depends on:** `m10_hook_server` (types), `field_state` (`FieldState`), `m01_core_types` (`OrderParameter`, `FleetMode`)
**Hot-Swap:** NEW

### Design Decisions

- **UserPromptSubmit injects field context into the prompt.** Before Claude processes
  a user prompt, ORAC adds current field state (r, K, sphere count, fleet mode, recent
  decisions) as structured context. This gives each Claude instance ambient awareness
  of the coordination field without requiring it to poll.
- **Injection is additive, never destructive.** The `inject` field in the response
  appends context; it never modifies the user's prompt text.
- **Lightweight handler.** This is the simplest hook: read cached `FieldState`, format
  as JSON, return. No IPC calls, no STDP, no policy checks.
- **Rate limiting.** If prompts arrive faster than 1/second (automated scripting),
  skip injection to avoid polluting context with stale repeated data.

### Types to Implement

```rust
/// Context injected into Claude Code on `UserPromptSubmit`.
#[derive(Debug, Clone, Serialize)]
pub struct FieldContextInjection {
    /// Current Kuramoto order parameter.
    pub r: f64,
    /// Mean phase angle (radians).
    pub psi: f64,
    /// Effective coupling strength.
    pub k_effective: f64,
    /// Number of active spheres.
    pub sphere_count: usize,
    /// Current fleet mode.
    pub fleet_mode: FleetMode,
    /// R-trend direction (Rising, Falling, Stable).
    pub r_trend: RTrend,
    /// Whether chimera state is detected.
    pub chimera_detected: bool,
    /// Current tick number.
    pub tick: u64,
}

impl FieldContextInjection {
    /// Build injection from cached field state.
    #[must_use]
    pub fn from_field_state(state: &FieldState, k_effective: f64) -> Self {
        Self {
            r: state.order.r,
            psi: state.order.psi,
            k_effective,
            sphere_count: 0, // filled by caller from sphere registry
            fleet_mode: state.fleet_mode,
            r_trend: state.r_trend,
            chimera_detected: state.harmonics.chimera_detected,
            tick: state.tick,
        }
    }
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `handle_user_prompt_submit` | `async fn handle_user_prompt_submit(State(s): ..., Json(req): ...) -> impl IntoResponse` | Inject field state context |
| `build_injection` | `fn build_injection(state: &HookServerState) -> FieldContextInjection` | Read cached field, format injection |
| `should_inject` | `fn should_inject(state: &HookServerState, session_id: &str) -> bool` | Rate limit check (1/sec max) |

### Tests

| Test | Validates |
|------|-----------|
| `test_prompt_hook_injects_field_state` | Response contains r, K, sphere_count |
| `test_prompt_hook_always_approves` | Decision is always "approve" |
| `test_injection_includes_chimera_flag` | Chimera detection state propagated |
| `test_injection_rate_limited` | Rapid prompts skip injection after first |
| `test_empty_field_returns_defaults` | No spheres -> r=0, K=0, count=0 |
| `test_fleet_mode_propagates` | Solo/Small/Medium/Large mode in response |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] Prompt Hooks
- ORAC_PLAN.md Phase 1 Detail (step 9)
- `field_state.rs` for `FieldState` source type
- **P07** (owned returns through `RwLock`)

---

## m14 -- Permission Policy

**Source:** `src/m3_hooks/m14_permission_policy.rs`
**LOC Target:** ~700
**Depends on:** `m01_core_types` (`PaneId`), `m03_config` (`PvConfig`), `m06_validation`
**Hot-Swap:** NEW

### Design Decisions

- **Permission cascade: sphere -> fleet -> default (P15).** When a `PermissionRequest`
  arrives, ORAC checks three policy levels in order:
  1. **Sphere-specific policy:** Does this exact sphere have an explicit rule for this
     tool/path combination? If yes, apply it.
  2. **Fleet-wide policy:** Is there a fleet-level rule? (e.g. "all spheres may write
     to `src/`" or "no sphere may run `rm -rf`").
  3. **Default policy:** Falls back to a configurable default (approve/deny/skip).
- **Glob matching for paths.** Tool paths use glob patterns: `src/**/*.rs` matches
  all Rust files under src. Uses the same `pattern_matches()` function from PV2's
  bus module (proven in production).
- **The Habitat philosophy: the field modulates, it does not command.** A sphere
  can decline coupling injection during sensitive work. Permission policy respects
  sphere agency -- if a sphere has declared `ConsentPosture::OptOut`, only critical
  safety rules (e.g. block `rm -rf /`) override the opt-out.
- **Fail-safe default.** If policy evaluation itself errors (malformed config, panic
  recovery), the default is `skip` -- never silently deny, never silently approve on
  error. Let Claude Code's own safety layer handle it.

### Types to Implement

```rust
/// Permission policy engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// Per-sphere rules (checked first in cascade).
    pub sphere_rules: HashMap<PaneId, Vec<PermissionRule>>,
    /// Fleet-wide rules (checked second).
    pub fleet_rules: Vec<PermissionRule>,
    /// Default decision when no rule matches.
    pub default_decision: HookDecision,
}

/// A single permission rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Rule identifier (for audit trail).
    pub id: String,
    /// Tool name pattern (glob, e.g. "Bash", "Edit", "*").
    pub tool_pattern: String,
    /// Path pattern (glob, e.g. "src/**/*.rs", "/tmp/*").
    #[serde(default)]
    pub path_pattern: Option<String>,
    /// Decision if this rule matches.
    pub decision: HookDecision,
    /// Human-readable reason for audit log.
    pub reason: String,
    /// Priority (higher = evaluated first within same cascade level).
    #[serde(default)]
    pub priority: i32,
}

/// `PermissionRequest` hook payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestPayload {
    /// Tool requesting permission.
    pub tool_name: String,
    /// File path (for file-access tools).
    pub path: Option<String>,
    /// Action (read, write, execute).
    pub action: Option<String>,
    /// Additional context from Claude Code.
    pub context: Option<serde_json::Value>,
}

/// Result of permission evaluation with audit trail.
#[derive(Debug, Clone, Serialize)]
pub struct PermissionEvaluation {
    /// Final decision.
    pub decision: HookDecision,
    /// Which rule matched (if any).
    pub matched_rule: Option<String>,
    /// Which cascade level matched.
    pub cascade_level: CascadeLevel,
    /// Reason string for Claude Code.
    pub reason: String,
}

/// Which level of the permission cascade matched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CascadeLevel {
    /// Sphere-specific rule matched.
    Sphere,
    /// Fleet-wide rule matched.
    Fleet,
    /// No rule matched -- using default.
    Default,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `handle_permission_request` | `async fn handle_permission_request(State(s): ..., Json(req): ...) -> impl IntoResponse` | Full cascade evaluation |
| `evaluate` | `fn evaluate(policy: &PermissionPolicy, pane_id: &PaneId, payload: &PermissionRequestPayload) -> PermissionEvaluation` | Core cascade logic (P15) |
| `matches_rule` | `fn matches_rule(rule: &PermissionRule, tool: &str, path: Option<&str>) -> bool` | Glob pattern matching |
| `load_policy` | `fn load_policy(config: &PvConfig) -> PvResult<PermissionPolicy>` | Load from `.claude/schemas/permission_policy.schema.json` |
| `merge_policies` | `fn merge_policies(base: PermissionPolicy, overlay: PermissionPolicy) -> PermissionPolicy` | Merge sphere-specific over fleet defaults |
| `audit_log` | `fn audit_log(eval: &PermissionEvaluation, pane_id: &PaneId, tool: &str)` | Structured tracing for permission decisions |

### Tests

| Test | Validates |
|------|-----------|
| `test_cascade_sphere_first` | Sphere-specific rule overrides fleet rule |
| `test_cascade_fleet_when_no_sphere` | Fleet rule used when no sphere match |
| `test_cascade_default_when_no_match` | Default decision when nothing matches |
| `test_glob_matching_tool_name` | `Bash` matches `Bash`, `*` matches all |
| `test_glob_matching_path` | `src/**/*.rs` matches `src/m3_hooks/m10.rs` |
| `test_priority_ordering` | Higher priority rules evaluated first |
| `test_safety_override_ignores_optout` | `rm -rf /` blocked even for opt-out spheres |
| `test_eval_error_returns_skip` | Malformed policy -> skip (never silent deny) |
| `test_audit_trail_populated` | `matched_rule` and `cascade_level` in result |
| `test_empty_policy_uses_default` | No rules at all -> default decision |
| `test_path_none_matches_pathless_rule` | Tool-only rule matches when no path given |
| `test_deny_reason_propagates` | Reason string passed through to response |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] Permission Policy
- [[Consent Flow Analysis]]
- `.claude/schemas/permission_policy.schema.json`
- ORAC_PLAN.md Phase 1 Detail (step 8), Consent Philosophy Integration
- ORAC_MINDMAP.md Branch 1 (Hook Server) leaf: Permission cascade
- **P15** (permission cascade sphere->fleet->default)
- **P16** (idempotency -- same permission request returns same result)
- **AP18** (fail-open on error -- but permissions fail to `skip`, not `approve`)

---

## Layer-Wide Invariants

### Performance Contract (P14)

All 6 hook handlers MUST return within 1ms measured at the Axum handler boundary.
This means:

- No synchronous HTTP calls to external services
- No disk I/O (SQLite queries, file reads)
- No unbounded iteration over sphere collections
- IPC sends are fire-and-forget via bounded channel (never `await` the response)
- Field state is read from a cached `Arc<RwLock<FieldState>>`, never computed on-demand

### Idempotency (P16)

Every hook request carries a `hook_id`. If the same `hook_id` is received twice,
the handler MUST return the same response without re-executing side effects.
The idempotency cache is bounded (1000 entries, LRU eviction).

### Thermal Gate Fail-Open (AP18)

The `PreToolUse` thermal gate MUST fail OPEN. If the SYNTHEX bridge is down, the
thermal cache is stale, or any error occurs during thermal evaluation, the tool
is APPROVED. Rationale: blocking agent work due to a monitoring subsystem failure
is worse than allowing work during unmeasured thermal conditions. The thermal gate
is advisory, not a safety mechanism.

### Permission Cascade (P15)

Permission evaluation follows strict cascade ordering:
1. Sphere-specific rules (most specific)
2. Fleet-wide rules (organizational)
3. Default decision (configurable)

A match at any level short-circuits: sphere rules are never consulted if a fleet
rule has already matched. Within a cascade level, rules are evaluated in priority
order (highest first), then insertion order for equal priority.

---

## Implementation Dependencies

```
m10_hook_server
  ├── m01_core_types (PaneId, Timestamp)
  ├── m02_error_handling (PvError, PvResult)
  ├── m03_config (PvConfig — port, timeouts)
  └── field_state (FieldState — cached field snapshot)

m11_session_hooks
  ├── m10_hook_server (HookRequest, HookResponse, HookServerState)
  ├── m07_ipc_client (IpcClient — sphere register/deregister)
  └── m01_core_types (PaneId, PaneSphere, PaneStatus)

m12_tool_hooks
  ├── m10_hook_server (HookRequest, HookResponse, HookServerState)
  ├── m07_ipc_client (IpcClient — task polling)
  ├── m18_hebbian_stdp (apply_stdp — co-activation learning)
  ├── m04_constants (STDP parameters)
  └── m01_core_types (PaneId, PaneStatus, WorkSignature)

m13_prompt_hooks
  ├── m10_hook_server (HookRequest, HookResponse, HookServerState)
  ├── field_state (FieldState)
  └── m01_core_types (OrderParameter, FleetMode, RTrend)

m14_permission_policy
  ├── m10_hook_server (HookDecision)
  ├── m01_core_types (PaneId)
  ├── m03_config (PvConfig — policy file path)
  └── m06_validation (input bounds checking)
```
