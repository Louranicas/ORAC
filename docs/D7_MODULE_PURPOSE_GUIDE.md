# D7: Module Purpose Guide

> **ORAC Sidecar** | 55 `.rs` files | 42 source modules | 8 layers | ~32K LOC | ~1690 tests
> **Deliverable 7 of 8** for the ORAC System Map (ACP-verified)
> **Obsidian:** `[[Session 061 — ORAC System Atlas]]` | `[[ULTRAPLATE Master Index]]`

---

## Table of Contents

1. [Layer 1: Core (m1_core)](#layer-1-core)
2. [Layer 2: Wire (m2_wire)](#layer-2-wire)
3. [Layer 3: Hooks (m3_hooks)](#layer-3-hooks)
4. [Layer 4: Intelligence (m4_intelligence)](#layer-4-intelligence)
5. [Layer 5: Bridges (m5_bridges)](#layer-5-bridges)
6. [Layer 6: Coordination (m6_coordination)](#layer-6-coordination)
7. [Layer 7: Monitoring (m7_monitoring)](#layer-7-monitoring)
8. [Layer 8: Evolution (m8_evolution)](#layer-8-evolution)
9. [Feature Gate Matrix](#feature-gate-matrix)
10. [Binary Entry Points](#binary-entry-points)
11. [Hot-Swap Sources](#hot-swap-sources)

---

## Layer 1: Core

**Directory:** `src/m1_core/` | **Feature Gate:** None (always compiled)
**Modules:** 6 named (m01-m06) + `field_state` | **Total Tests:** ~201

All other layers depend on L1. No upward imports permitted.

---

### m01_core_types

**Purpose:** Foundational newtypes, identifiers, sphere data structures, and field enums used across every layer.

**Key Types:**
- `PaneId(String)` -- newtype sphere identifier
- `TaskId(String)` -- UUID v4 task identifier
- `Point3D { x, y, z }` -- unit sphere coordinate (Copy, 24 bytes)
- `SphereMemory` -- tool-call memory with activation decay
- `Buoy` -- Hebbian cluster on sphere surface with drift
- `PaneSphere` -- full sphere state (phase, frequency, status, memories, buoys)
- `OrderParameter { r, psi }` -- Kuramoto synchronization measure
- `PaneStatus` -- enum: Idle, Working, Blocked, Complete
- `FleetMode` -- enum: Solo, Small, Medium, Large
- `FieldAction` -- enum: Stable, BoostCoupling, ReduceCoupling, InjectNoise, ForceDiverge, Recovering
- `RTrend` -- enum: Stable, Rising, Falling
- `DecisionRecord` -- audit trail entry for conductor decisions
- `WorkSignature` -- per-sphere domain work distribution
- `ActivationZones` -- hot/warm/cold memory zone counts

**Public API:**
- `fn now_secs() -> f64` -- epoch seconds (no chrono)
- `fn phase_diff(a: f64, b: f64) -> f64` -- shortest angular distance
- `fn semantic_phase_region(tool_name: &str) -> f64` -- tool-to-phase mapping

**Dependencies:** None (leaf module)
**Tests:** 52
**Feature Gate:** None

---

### m02_error_handling

**Purpose:** Unified error enum `PvError` with 25 variants, numeric codes (PV-1000 to PV-1999), and classification by retryability/severity.

**Key Types:**
- `PvError` -- thiserror enum (Config 1000-1099, Validation 1100-1199, Field 1200-1299, Bridge 1300-1399, Bus 1400-1499, Persistence 1500-1599, Governance 1600-1699, Generic 1900-1999)
- `PvResult<T>` -- alias for `Result<T, PvError>`
- `ErrorSeverity` -- enum: Info, Warning, Error, Critical
- `ErrorClassifier` -- trait: `is_retryable()`, `severity()`, `code()`

**Public API:**
- `fn is_retryable(&self) -> bool` -- true for network/IO/DB errors
- `fn severity(&self) -> ErrorSeverity` -- classification for alerting
- `fn code(&self) -> u16` -- numeric PV-NNNN code

**Dependencies:** None (uses thiserror, serde_json, figment, toml for From impls)
**Tests:** 29
**Feature Gate:** None

---

### m03_config

**Purpose:** Figment-based TOML configuration with environment variable overlay (`PV2_*` prefix) covering all subsystems.

**Key Types:**
- `PvConfig` -- top-level config: server, field, sphere, coupling, learning, bridges, conductor, ipc, persistence, governance
- `ServerConfig` -- port (default 8132), bind_addr, body_limit_bytes
- `FieldConfig` -- tick_interval_secs, kuramoto_dt, r_target, thresholds
- `LearningConfig` -- hebbian_ltp, hebbian_ltd, burst/newcomer multipliers, weight_floor
- `BridgesConfig` -- per-bridge poll intervals, k_mod budget bounds
- `IpcConfig` -- socket_path, max_connections, cascade_rate_limit
- `GovernanceConfig` -- quorum_threshold, proposal_voting_window_ticks

**Public API:**
- `fn PvConfig::load() -> PvResult<Self>` -- load from default paths + env
- `fn PvConfig::from_path(path: &str) -> PvResult<Self>` -- load from specific TOML

**Dependencies:** m02 (`PvError`)
**Tests:** 25
**Feature Gate:** None

---

### m04_constants

**Purpose:** All compile-time magic numbers as named constants. Runtime-configurable values live in m03.

**Key Types:**
- 40+ `pub const` values across categories: tick timing, Hebbian learning, coupling, field thresholds, R target dynamics, conductor, K modulation bounds, sphere limits, persistence, network

**Public API (selected):**
- `const TICK_INTERVAL_SECS: u64 = 5`
- `const HEBBIAN_LTP: f64 = 0.01` / `const HEBBIAN_LTD: f64 = 0.002`
- `const HEBBIAN_WEIGHT_FLOOR: f64 = 0.15`
- `const SPHERE_CAP: usize = 200`
- `const DEFAULT_PORT: u16 = 8132`
- `const R_TARGET_BASE: f64 = 0.93`

**Dependencies:** None
**Tests:** 14
**Feature Gate:** None

---

### m05_traits

**Purpose:** Dependency-inversion traits for cross-layer abstractions. All trait methods use `&self` with interior mutability.

**Key Types:**
- `Bridgeable` -- trait: `service_name()`, `poll() -> PvResult<f64>`, `post(payload)`, `health()`, `is_stale(tick)`. Requires `Send + Sync + Debug`.

**Public API:**
- `fn service_name(&self) -> &str`
- `fn poll(&self) -> PvResult<f64>` -- returns adjustment factor
- `fn post(&self, payload: &[u8]) -> PvResult<()>` -- fire-and-forget
- `fn health(&self) -> PvResult<bool>`

**Dependencies:** m02 (`PvResult`)
**Tests:** 1 (object safety check)
**Feature Gate:** None

---

### m06_validation

**Purpose:** Validates all external inputs at system boundary with wrapping, clamping, and string safety.

**Key Types:** (pure functions, no structs)

**Public API:**
- `fn validate_phase(phase: f64) -> PvResult<f64>` -- wraps to [0, 2pi)
- `fn validate_frequency(freq: f64) -> PvResult<f64>` -- clamps to [0.001, 10.0]
- `fn validate_strength(strength: f64) -> PvResult<f64>` -- clamps to [0.0, 2.0]
- `fn validate_weight(weight: f64) -> PvResult<f64>` -- clamps to [FLOOR, 1.0]
- `fn validate_persona(persona: &str) -> PvResult<String>` -- length + charset check
- `fn validate_tool_name(name: &str) -> PvResult<String>` -- length + charset check
- `fn validate_body(body: &str, max_len: usize) -> PvResult<String>` -- truncation via `chars().take()`

**Dependencies:** m02 (`PvError`), m04 (constants)
**Tests:** 51
**Feature Gate:** None

---

### field_state

**Purpose:** ORAC-native field state types replacing PV2's m3_field. The sidecar observes and caches field state; it does not own the authoritative field.

**Key Types:**
- `FieldState` -- cached snapshot: order (`OrderParameter`), tick, fleet_mode, r_trend, harmonics
- `Harmonics` -- sub-cluster analysis: clusters (`Vec<OrderParameter>`), chimera_detected, cluster_count
- `FieldDecision` -- conductor output: action, k_delta, reason
- `AppState` -- full sidecar state: spheres, field, tick, EMAs, r_history, warmup, staleness tracking (32 fields on `OracState`)
- `SharedState` -- `Arc<RwLock<AppState>>`

**Public API:**
- `fn FieldState::compute(spheres, tick) -> Self` -- Kuramoto order parameter + harmonics
- `fn AppState::push_r(r)` -- ring buffer R history (cap 60)
- `fn AppState::update_emas(divergence, coherence)` -- EMA smoothing (alpha=0.2)
- `fn AppState::is_stale() -> bool` -- true after 3+ consecutive missed polls
- `fn new_shared_state() -> SharedState`

**Dependencies:** m01 (`PaneId`, `PaneSphere`, etc.), m04 (constants)
**Tests:** 29
**Feature Gate:** None

---

## Layer 2: Wire

**Directory:** `src/m2_wire/` | **Feature Gate:** None (always compiled)
**Modules:** 3 named (m07-m09) | **Total Tests:** ~118

---

### m07_ipc_client

**Purpose:** Async tokio-based Unix socket client connecting to PV2 daemon's IPC bus for event subscription and task submission.

**Key Types:**
- `IpcClient` -- persistent socket connection with handshake, subscribe, send/recv capabilities
- `IpcState` -- enum: Disconnected, Connecting, Connected, Subscribed

**Public API:**
- `async fn IpcClient::connect(path) -> PvResult<Self>` -- connect + handshake
- `async fn IpcClient::subscribe(patterns) -> PvResult<usize>` -- subscribe to event patterns
- `async fn IpcClient::send_frame(frame: &BusFrame) -> PvResult<()>` -- NDJSON write
- `async fn IpcClient::recv_frame() -> PvResult<BusFrame>` -- NDJSON read with timeout
- `fn IpcClient::state() -> IpcState`

**Dependencies:** m01 (`PaneId`), m02 (`PvError`), m08 (`BusFrame`), m09 (`MAX_FRAME_SIZE`)
**Tests:** 11
**Feature Gate:** None

---

### m08_bus_types

**Purpose:** Bus frame types, task lifecycle, and event subscription protocol using serde internally-tagged NDJSON enums.

**Key Types:**
- `BusFrame` -- 11-variant enum: Handshake, Welcome, Subscribe, Subscribed, Event, Submit, TaskSubmitted, Claim, Complete, Fail, Disconnect
- `BusTask` -- id, target, summary, status, timestamps
- `TaskTarget` -- enum: Specific(`PaneId`), AnyIdle, FieldDriven, Willing
- `TaskStatus` -- enum: Pending, Claimed, Completed, Failed
- `BusEvent` -- typed events: FieldTick, SphereRegistered, SphereDeregistered, TaskDispatched, CascadeHandoff, Custom

**Public API:**
- `fn BusTask::new(target, summary) -> Self`
- `fn BusFrame::is_server_frame() -> bool`
- `fn BusFrame::is_client_frame() -> bool`

**Dependencies:** m01 (`PaneId`, `TaskId`, `now_secs`)
**Tests:** 67
**Feature Gate:** None

---

### m09_wire_protocol

**Purpose:** V2 wire protocol state machine, frame validation, send/receive queues, and keepalive management.

**Key Types:**
- `ProtocolState` -- enum: Disconnected, Handshaking, Connected, Subscribing, Active, Closing
- `WireProtocol` -- state machine with send_queue (`VecDeque`, cap 1000), recv_buffer (cap 500), keepalive tracking
- `FrameValidation` -- result of validating a frame against current protocol state

**Public API:**
- `fn WireProtocol::new(pane_id) -> Self`
- `fn WireProtocol::advance(&self, frame) -> PvResult<ProtocolState>` -- FSM transition
- `fn WireProtocol::validate_outbound(&self, frame) -> PvResult<()>`
- `fn WireProtocol::enqueue_send(&self, frame) -> PvResult<()>`
- `const MAX_FRAME_SIZE: usize = 65_536`

**Dependencies:** m01 (`PaneId`), m02 (`PvError`), m08 (`BusFrame`)
**Tests:** 40
**Feature Gate:** None

---

## Layer 3: Hooks

**Directory:** `src/m3_hooks/` | **Feature Gate:** `api` (axum, tower-http)
**Modules:** 5 named (m10-m14) | **Total Tests:** ~206

THE KEYSTONE: replaces all bash hook scripts with sub-ms HTTP endpoints.

---

### m10_hook_server

**Purpose:** Axum HTTP server on `:8133` with 6 hook endpoints, `OracState` shared state, health check, field poller, and bridge integration.

**Key Types:**
- `OracState` -- 32 fields: config, shared_state, tick (`AtomicU64`), pv2_url, synthex/me/povm/rm URLs, coupling_network, ralph_engine, blackboard, breaker_registry, consent map, sessions, ghosts, token_accounting, trace_store, metrics_registry, field_dashboard, ipc_connected, me_bridge, rm_bridge, hooks counter
- `HookEvent` -- deserialized hook payload: session_id, tool_name, tool_input, tool_output, prompt, permission
- `HookResponse` -- response with optional decision, `systemMessage`, `outputPrefix`

**Public API:**
- `fn build_router(state: Arc<OracState>) -> Router` -- creates Axum router with all endpoints
- `fn spawn_field_poller(state: Arc<OracState>)` -- background PV2 health poller
- `fn fire_and_forget_post(url, body)` -- async non-blocking HTTP POST
- `fn breaker_guarded_post(state, service, url, body)` -- POST through circuit breaker
- `fn http_get(url, timeout_ms) -> Option<String>` / `fn http_post(url, body, timeout_ms) -> Option<String>`
- `fn epoch_ms() -> u64` -- current epoch milliseconds

**Dependencies:** L1 (field_state, m01, m03), L4 (m15, m21), L5 (m22-m26), L8 (m36)
**Tests:** 76
**Feature Gate:** `api`

---

### m11_session_hooks

**Purpose:** Handlers for `SessionStart` (register sphere, hydrate from POVM+RM) and `Stop` (fail tasks, crystallize, deregister) lifecycle events.

**Key Types:** (uses `OracState`, `HookEvent`, `HookResponse` from m10)

**Public API:**
- `async fn handle_session_start(State, Json<HookEvent>) -> Json<HookResponse>` -- register + hydrate
- `async fn handle_stop(State, Json<HookEvent>) -> Json<HookResponse>` -- crystallize + deregister

**Dependencies:** m10 (`OracState`, `HookEvent`, `HookResponse`, helpers)
**Tests:** 24
**Feature Gate:** `api`

---

### m12_tool_hooks

**Purpose:** Handlers for `PostToolUse` (memory update, Hebbian STDP, task polling, blackboard writes, token accounting) and `PreToolUse` (SYNTHEX thermal gate).

**Key Types:** (uses `OracState`, `HookEvent`, `HookResponse` from m10)

**Public API:**
- `async fn handle_post_tool_use(State, Json<HookEvent>) -> Json<HookResponse>` -- memory + status + task poll
- `async fn handle_pre_tool_use(State, Json<HookEvent>) -> Json<HookResponse>` -- thermal gate check

**Dependencies:** m10, m01 (`PaneId`, `PaneStatus`), m20 (`semantic_router`), m26 (`blackboard`)
**Tests:** 56
**Feature Gate:** `api`

---

### m13_prompt_hooks

**Purpose:** Handler for `UserPromptSubmit` that injects field state (r, tick, spheres, thermal) and pending tasks into user prompts. Skips short prompts (<20 chars).

**Key Types:** (uses `OracState`, `HookEvent`, `HookResponse` from m10)

**Public API:**
- `async fn handle_user_prompt_submit(State, Json<HookEvent>) -> Json<HookResponse>` -- field state injection

**Dependencies:** m10 (`OracState`, helpers)
**Tests:** 25
**Feature Gate:** `api`

---

### m14_permission_policy

**Purpose:** Auto-approve/deny policy engine for fleet agent `PermissionRequest` events. Eliminates permission dialog spam across the fleet.

**Key Types:**
- `Decision` -- enum: Allow, AllowWithNotice, Deny
- `PermissionPolicy` -- configurable rule set: always_approve (Read, Glob, Grep), approve_with_notice (Edit, Write, Bash), always_deny (configurable), default_approve

**Public API:**
- `async fn handle_permission_request(State, Json<HookEvent>) -> Json<HookResponse>`
- `fn PermissionPolicy::evaluate(tool_name: &str) -> Decision`
- `fn PermissionPolicy::default() -> Self` -- permissive fleet defaults

**Dependencies:** m10 (`OracState`, `HookEvent`, `HookResponse`)
**Tests:** 25
**Feature Gate:** `api`

---

## Layer 4: Intelligence

**Directory:** `src/m4_intelligence/` | **Feature Gate:** `intelligence` (tower)
**Modules:** 7 named (m15-m21) | **Total Tests:** ~237

---

### m15_coupling_network

**Purpose:** Kuramoto coupling matrix with Jacobi integration. Phase stepping uses mean-field equation: `dtheta_i/dt = omega_i + K/N * sum(w_ij * sin(theta_j - theta_i))`.

**Key Types:**
- `CouplingNetwork` -- phases, frequencies, connections, K, k_modulation, adj_index
- `Connection` -- directed edge: from, to, weight, type_weight

**Public API:**
- `fn CouplingNetwork::new() -> Self`
- `fn CouplingNetwork::add_sphere(id, phase, frequency)`
- `fn CouplingNetwork::step(dt)` -- Euler integration step
- `fn CouplingNetwork::order_parameter() -> OrderParameter`
- `fn CouplingNetwork::rebuild_index()` -- adjacency index for O(degree) lookup

**Dependencies:** m01 (`PaneId`, `OrderParameter`, `phase_diff`), m04 (constants)
**Tests:** 43
**Feature Gate:** `intelligence`

---

### m16_auto_k

**Purpose:** Adaptive coupling strength K based on frequency spread and fleet size. Prevents over-synchronization with smoothed recalculation.

**Key Types:**
- `AutoKController` -- ticks_since_recalc, period, previous_k, smoothing factor

**Public API:**
- `fn AutoKController::new() -> Self` -- default period from constants
- `fn AutoKController::with_params(period, smoothing) -> Self`
- `fn AutoKController::tick(network: &mut CouplingNetwork) -> bool` -- returns true if K recalculated

**Dependencies:** m04 (constants), m15 (`CouplingNetwork`)
**Tests:** 23
**Feature Gate:** `intelligence`

---

### m17_topology

**Purpose:** Network topology analysis: neighbor discovery, weight-squared amplification, coupling strength metrics.

**Key Types:**
- `NeighborInfo` -- id, effective_weight, weight_squared, phase_diff

**Public API:**
- `fn neighbors(network, sphere_id) -> Vec<NeighborInfo>` -- sorted by effective weight (descending)
- `fn mean_coupling_strength(network) -> f64`
- `fn max_coupling_strength(network) -> f64`
- `fn degree(network, sphere_id) -> usize` -- connection count

**Dependencies:** m01 (`PaneId`), m04 (constants), m15 (`CouplingNetwork`)
**Tests:** 28
**Feature Gate:** `intelligence`

---

### m18_hebbian_stdp

**Purpose:** Spike-timing dependent plasticity adapted for Kuramoto oscillators. Co-active spheres strengthen weights (LTP); inactive pairs decay (LTD).

**Key Types:**
- `StdpResult` -- ltp_count, ltd_count, at_floor_count, total_weight_change

**Public API:**
- `fn apply_stdp(network: &mut CouplingNetwork, spheres: &HashMap<PaneId, PaneSphere>) -> StdpResult`
  - Co-active (both Working): LTP (+0.01, 3x burst, 2x newcomer)
  - Non-co-active: LTD (-0.002)
  - Weight floor: 0.15, soft ceiling: 0.85
  - G1 guard: skip when working_count < 2 (anti-saturation)

**Dependencies:** m01 (`PaneId`, `PaneStatus`, `PaneSphere`), m04 (constants), m15 (`CouplingNetwork`)
**Tests:** 30
**Feature Gate:** `intelligence`

---

### m19_buoy_network

**Purpose:** Cross-sphere buoy analysis, tunnel discovery, activation zone statistics, and buoy health metrics.

**Key Types:**
- `BuoyHealth` -- sphere_id, buoy_count, mean_drift, max_drift, total_activations, has_drifted
- `TunnelInfo` -- pair of sphere IDs, distance, buoy count in tunnel
- `FleetBuoyStats` -- total buoys, mean per sphere, drifted fraction

**Public API:**
- `fn buoy_health(sphere: &PaneSphere) -> BuoyHealth`
- `fn find_tunnels(spheres, threshold) -> Vec<TunnelInfo>` -- cross-sphere buoy proximity
- `fn fleet_buoy_stats(spheres) -> FleetBuoyStats`

**Dependencies:** m01 (`PaneId`, `Point3D`, `Buoy`, `PaneSphere`), m04 (constants)
**Tests:** 23
**Feature Gate:** `intelligence`

---

### m20_semantic_router

**Purpose:** Content-aware dispatch using Hebbian weights and domain affinity. Routes tasks to best-suited panes using weighted composite scoring.

**Key Types:**
- `SemanticDomain` -- enum: Read (phase 0), Write (phase pi/2), Execute (phase pi), Communicate (phase 3pi/2)
- `RouteRequest` -- tool_name, content, preferred_pane
- `RouteResult` -- target `PaneId`, score, domain, reasoning

**Public API:**
- `fn classify_tool(tool_name: &str) -> SemanticDomain`
- `fn classify_content(content: &str) -> SemanticDomain`
- `fn route(request: RouteRequest, spheres, network) -> Option<RouteResult>` -- weighted composite: domain 40% + Hebbian 35% + availability 25%

**Dependencies:** m01 (`PaneId`, `PaneStatus`, `PaneSphere`), m15 (`CouplingNetwork`)
**Tests:** 45
**Feature Gate:** `intelligence`

---

### m21_circuit_breaker

**Purpose:** Per-pane health gating with Closed/Open/HalfOpen FSM. Prevents dispatch to failing panes and auto-recovers via probe requests.

**Key Types:**
- `BreakerState` -- enum: Closed, Open, HalfOpen
- `BreakerConfig` -- failure_threshold, success_threshold, open_timeout_ticks, half_open_max_requests
- `CircuitBreaker` -- per-pane state machine with counters and timing
- `BreakerRegistry` -- fleet-wide registry: `HashMap<PaneId, CircuitBreaker>`

**Public API:**
- `fn CircuitBreaker::record_success(&self)` / `fn record_failure(&self)`
- `fn CircuitBreaker::state() -> BreakerState`
- `fn BreakerRegistry::get_or_create(pane_id) -> &CircuitBreaker`
- `fn BreakerRegistry::tick_all()` -- advance all breaker timeouts
- `fn BreakerRegistry::state_counts() -> (closed, open, half_open)`

**Dependencies:** m01 (`PaneId`)
**Tests:** 45
**Feature Gate:** `intelligence`

---

## Layer 5: Bridges

**Directory:** `src/m5_bridges/` | **Feature Gate:** `bridges`
**Modules:** 5 named (m22-m26) + `http_helpers` | **Total Tests:** ~339

---

### http_helpers

**Purpose:** Shared raw TCP HTTP helpers extracted from M22-M25 to eliminate duplication (BUG-042). All bridges use raw `TcpStream` for minimal overhead.

**Key Types:** (pure functions)

**Public API:**
- `fn raw_http_get(addr, path, service) -> PvResult<String>` -- GET with 2s timeout, 32KB limit
- `fn raw_http_get_with_limit(addr, path, service, max_size) -> PvResult<String>` -- custom limit
- `fn raw_http_post(addr, path, body, service) -> PvResult<String>` -- POST JSON
- `fn raw_http_post_tsv(addr, path, body, service) -> PvResult<String>` -- POST TSV (for RM)
- `fn extract_body(response: &str) -> &str` -- extract body after `\r\n\r\n`

**Dependencies:** m02 (`PvError`)
**Tests:** 29
**Feature Gate:** `bridges`

---

### m22_synthex_bridge

**Purpose:** Bidirectional REST bridge to SYNTHEX at `127.0.0.1:8090`. Polls `/v3/thermal` for k_adjustment, posts field state to `/api/ingest`.

**Key Types:**
- `SynthexBridge` -- implements `Bridgeable`. Stores base_url, last_adjustment (`RwLock`), last_poll_tick, poll_interval.
- `ThermalResponse` -- temperature, target, k_adjustment, overall_health

**Public API:**
- `fn SynthexBridge::new() -> Self` / `fn with_config(url, interval) -> Self`
- `fn poll(&self) -> PvResult<f64>` -- returns thermal k_adjustment
- `fn post(&self, payload: &[u8]) -> PvResult<()>` -- POST to /api/ingest
- `fn parse_thermal(body: &str) -> f64` -- extracts k_adjustment (NaN/INF safe, default 1.0)

**Dependencies:** m02, m04, m05 (`Bridgeable`), http_helpers
**Tests:** 56
**Feature Gate:** `bridges`

---

### m23_me_bridge

**Purpose:** Polls Maintenance Engine at `127.0.0.1:8080/api/observer` for fitness signal. Handles BUG-008 frozen detection (3 identical polls trigger neutral fallback).

**Key Types:**
- `MeBridge` -- implements `Bridgeable`. Stores base_url, last_fitness (`RwLock`), recent_fitness (`VecDeque` for frozen detection), successful_polls counter.
- `ObserverResponse` -- `last_report.current_fitness` parse chain

**Public API:**
- `fn MeBridge::new() -> Self` / `fn with_config(url, interval) -> Self`
- `fn poll(&self) -> PvResult<f64>` -- returns fitness-based adjustment
- `fn is_frozen(&self) -> bool` -- 3 identical polls within tolerance (0.003)
- `fn successful_polls(&self) -> u64`

**Dependencies:** m02, m04, m05 (`Bridgeable`), http_helpers
**Tests:** 52
**Feature Gate:** `bridges`

---

### m24_povm_bridge

**Purpose:** Snapshots sphere data to POVM Engine at `127.0.0.1:8125` every 12 ticks. Reads Hebbian pathways every 60 ticks. Startup hydration via `/hydrate`.

**Key Types:**
- `PovmBridge` -- implements `Bridgeable`. Stores base_url, write/read intervals, last values.
- `PovmPathway` -- pre_id, post_id, weight (with serde aliases for compatibility)
- `PovmMemory` -- id, content, tags, strength

**Public API:**
- `fn PovmBridge::new() -> Self` / `fn with_config(url, write_interval, read_interval) -> Self`
- `fn poll(&self) -> PvResult<f64>` -- reads pathway health signal
- `fn post(&self, payload: &[u8]) -> PvResult<()>` -- POST memory snapshot
- `fn hydrate_pathways(&self) -> PvResult<Vec<PovmPathway>>` -- startup pathway load (2MB limit)
- `fn hydrate_summary(&self) -> PvResult<String>` -- summary for session context

**Dependencies:** m02, m05 (`Bridgeable`), http_helpers
**Tests:** 60
**Feature Gate:** `bridges`

---

### m25_rm_bridge

**Purpose:** TSV POST to Reasoning Memory at `127.0.0.1:8130`. **NEVER JSON** -- TSV only (AP05). Format: `category\tagent\tconfidence\tttl\tcontent`.

**Key Types:**
- `RmBridge` -- implements `Bridgeable`. Stores base_url, agent name ("orac-sidecar"), poll_interval.
- `TsvRecord` -- category, agent, confidence, ttl, content

**Public API:**
- `fn RmBridge::new() -> Self` / `fn with_config(url, agent, interval) -> Self`
- `fn poll(&self) -> PvResult<f64>` -- searches RM for ORAC entries
- `fn post(&self, payload: &[u8]) -> PvResult<()>` -- POST TSV (Content-Type: text/tab-separated-values)
- `fn post_field_state(r, tick, spheres) -> PvResult<()>` -- format + POST field state TSV
- `fn sanitize_tsv(input: &str) -> String` -- single-pass tab/newline sanitization

**Dependencies:** m02, m05 (`Bridgeable`), http_helpers
**Tests:** 52
**Feature Gate:** `bridges`

---

### m26_blackboard

**Purpose:** SQLite-backed shared fleet state for cross-pane coordination. 9 tables: `pane_status`, `task_history`, `agent_cards`, `sessions`, `ralph_state`, `coupling_weights`, `ghosts`, `emergence_log`, `field_snapshots`.

**Key Types:**
- `Blackboard` -- rusqlite `Connection` wrapper with all CRUD methods
- `PaneRecord` -- pane_id, status, persona, updated_at, phase, tasks_completed
- `TaskRecord` -- task_id, pane_id, description, outcome, finished_at, duration_secs
- `AgentCard` -- pane_id, capabilities, domain, model, registered_at
- `GhostRecord` -- deregistered sphere trace

**Public API:**
- `fn Blackboard::open(path) -> PvResult<Self>` / `fn in_memory() -> PvResult<Self>`
- `fn upsert_pane(record) -> PvResult<()>` / `fn get_pane(id) -> PvResult<Option<PaneRecord>>`
- `fn insert_task(record) -> PvResult<()>` / `fn recent_tasks(limit) -> PvResult<Vec<TaskRecord>>`
- `fn save_ralph_state(json) -> PvResult<()>` / `fn load_ralph_state() -> PvResult<Option<String>>`
- `fn save_session(id, data) -> PvResult<()>` / `fn load_all_sessions() -> PvResult<Vec<(String, String)>>`
- `fn prune_complete_panes(before_secs) -> PvResult<usize>`
- `fn prune_old_tasks(before_secs) -> PvResult<usize>`

**Dependencies:** m01 (`PaneId`, `PaneStatus`), m02 (`PvError`)
**Tests:** 90
**Feature Gate:** `persistence` (rusqlite)

---

## Layer 6: Coordination

**Directory:** `src/m6_coordination/` | **Feature Gate:** None (always compiled)
**Modules:** 5 named (m27-m31) | **Total Tests:** ~133

---

### m27_conductor

**Purpose:** PI breathing controller for field synchronization. In ORAC the conductor is advisory -- it observes PV2 state and computes local breathing suggestions.

**Key Types:**
- `Conductor` -- gain, breathing_blend (both f64, no interior mutability)

**Public API:**
- `fn Conductor::new() -> Self` -- default gains from constants
- `fn Conductor::with_params(gain, blend) -> Self`
- `fn Conductor::decide(state: &AppState) -> FieldDecision` -- PI controller output
- `fn Conductor::r_target(sphere_count) -> f64` -- dynamic target (0.93 base, 0.85 for >50 spheres)

**Dependencies:** m01 (`FieldAction`, `PaneId`), m04 (constants), field_state (`AppState`, `FieldDecision`)
**Tests:** 25
**Feature Gate:** None

---

### m28_cascade

**Purpose:** Cascade handoff system with rate limiting (max 10/minute), depth tracking (auto-summarize at >3), and markdown fallback briefs.

**Key Types:**
- `CascadeHandoff` -- source, target, brief, dispatched_at, depth, acknowledged, rejected
- `CascadeManager` -- pending queue (`VecDeque`, cap 50), rate limiter (timestamps), history

**Public API:**
- `fn CascadeManager::new() -> Self`
- `fn CascadeManager::dispatch(source, target, brief) -> PvResult<String>` -- rate-limited dispatch
- `fn CascadeManager::acknowledge(cascade_id) -> PvResult<()>`
- `fn CascadeManager::reject(cascade_id, reason) -> PvResult<()>`
- `fn CascadeManager::pending_count() -> usize`

**Dependencies:** m01 (`PaneId`, `now_secs`), m02 (`PvError`)
**Tests:** 46
**Feature Gate:** None

---

### m29_tick

**Purpose:** Sidecar tick loop that updates cached state from PV2 and runs local intelligence passes (advisory Hebbian STDP, conductor decisions).

**Key Types:**
- `TickResult` -- field_state, decision, order_parameter, phase_timings
- `PhaseTiming` -- per-phase duration breakdown (Phase 1-5)

**Public API:**
- `fn tick_once(state: &mut AppState) -> TickResult` -- phases 1-3 (advance, recompute, conductor)
- `fn tick_with_hebbian(state, conductor, network, spheres) -> TickResult` -- phases 1-4 (+ STDP)

**Dependencies:** field_state, m01, m04, m15 (`CouplingNetwork`), m18 (`apply_stdp`), m27 (`Conductor`)
**Tests:** 13
**Feature Gate:** None (STDP integration requires `intelligence`)

---

### m30_wasm_bridge

**Purpose:** FIFO/ring protocol bridge between ORAC and Zellij swarm-orchestrator WASM plugin. Reads commands from FIFO pipe, writes events to ring-buffered JSONL file.

**Key Types:**
- `WasmCommand` -- enum: Dispatch(pane, task), Status, FieldState, ListPanes, Ping
- `WasmEvent` -- enum: Tick, TaskCompleted, TaskFailed, FieldUpdate, Pong
- `EventRingBuffer` -- `VecDeque` with 1000-line cap and FIFO eviction
- `WasmBridge` -- fifo_path, ring_path, event_buffer (`RwLock`)

**Public API:**
- `fn WasmBridge::new() -> Self` -- default FIFO/ring paths
- `fn WasmBridge::parse_command(line: &str) -> PvResult<WasmCommand>`
- `fn WasmBridge::push_event(event: WasmEvent) -> PvResult<()>`
- `fn WasmBridge::flush_ring() -> PvResult<()>` -- write buffer to ring file
- `const DEFAULT_FIFO_PATH: &str = "/tmp/swarm-commands.pipe"`
- `const DEFAULT_RING_PATH: &str = "/tmp/swarm-events.jsonl"`

**Dependencies:** m02 (`PvError`)
**Tests:** 34
**Feature Gate:** None

---

### m31_memory_manager

**Purpose:** Fleet-level memory aggregation, statistics, and pruning coordination across all spheres.

**Key Types:**
- `FleetMemoryStats` -- total_memories, active_memories, mean_per_sphere, max_per_sphere, spheres_near_capacity, unique_tools
- `PruneResult` -- memories_pruned, spheres_touched

**Public API:**
- `fn compute_stats(spheres) -> FleetMemoryStats`
- `fn prune_candidates(spheres) -> Vec<(PaneId, Vec<u64>)>` -- memories below activation threshold
- `fn fleet_activation_zones(spheres) -> ActivationZones` -- aggregate hot/warm/cold zones

**Dependencies:** m01 (`PaneId`, `PaneSphere`, `ActivationZones`), m04 (constants)
**Tests:** 15
**Feature Gate:** None

---

## Layer 7: Monitoring

**Directory:** `src/m7_monitoring/` | **Feature Gate:** `monitoring` (opentelemetry, opentelemetry-otlp)
**Modules:** 4 named (m32-m35) | **Total Tests:** ~236

---

### m32_otel_traces

**Purpose:** In-process OpenTelemetry trace store for task lifecycle spans across panes. Ring buffer with 10,000-span cap.

**Key Types:**
- `TraceId([u8; 16])` -- 128-bit trace identifier
- `SpanId([u8; 8])` -- 64-bit span identifier
- `OtelSpan` -- trace_id, span_id, parent_span_id, name, start_time, end_time, status, attributes
- `TraceStore` -- ring buffer (`VecDeque`, cap 10,000) with query methods (`RwLock`)

**Public API:**
- `fn TraceStore::new() -> Self`
- `fn TraceStore::start_span(name, trace_id, parent) -> SpanId`
- `fn TraceStore::end_span(span_id, status)`
- `fn TraceStore::recent(limit) -> Vec<OtelSpan>`
- `fn TraceStore::by_trace(trace_id) -> Vec<OtelSpan>`
- `fn TraceStore::by_pane(pane_id) -> Vec<OtelSpan>`
- `fn TraceStore::errors(limit) -> Vec<OtelSpan>`

**Dependencies:** m01 (`PaneId`, `TaskId`, `now_secs`), m02 (`PvError`)
**Tests:** 73
**Feature Gate:** `monitoring`

---

### m33_metrics_export

**Purpose:** Prometheus-compatible metrics in text exposition format with `orac_` namespace prefix. Supports Counter, Gauge, Histogram types.

**Key Types:**
- `MetricType` -- enum: Counter, Gauge, Histogram
- `MetricDefinition` -- name, metric_type, help, labels
- `MetricsRegistry` -- `BTreeMap` of metric definitions + values (`RwLock`)

**Public API:**
- `fn MetricsRegistry::new() -> Self`
- `fn MetricsRegistry::register(name, metric_type, help)`
- `fn MetricsRegistry::increment(name, labels, delta)`
- `fn MetricsRegistry::set_gauge(name, labels, value)`
- `fn MetricsRegistry::observe_histogram(name, labels, value)`
- `fn MetricsRegistry::export() -> String` -- Prometheus text format

**Dependencies:** m01 (`now_secs`), m02 (`PvResult`)
**Tests:** 60
**Feature Gate:** `monitoring`

---

### m34_field_dashboard

**Purpose:** Kuramoto field dashboard data model: per-cluster r, phase maps, chimera detection, K effective, R history over last 60 ticks.

**Key Types:**
- `SpherePhaseEntry` -- pane_id, phase, frequency, status, cluster
- `ClusterInfo` -- order_parameter, member_count, centroid_phase
- `FieldDashboard` -- r_history (`VecDeque`, cap 60), phase_map, clusters, chimera_gaps, k_effective (`RwLock`)
- `DashboardSnapshot` -- serializable snapshot of all dashboard state

**Public API:**
- `fn FieldDashboard::new() -> Self`
- `fn FieldDashboard::update(field_state, spheres, k_effective)` -- refresh all panels
- `fn FieldDashboard::snapshot() -> DashboardSnapshot` -- read-only export
- `fn FieldDashboard::r_history() -> Vec<f64>` -- last 60 R values

**Dependencies:** m01 (`OrderParameter`, `PaneId`), m04 (constants)
**Tests:** 48
**Feature Gate:** `monitoring`

---

### m35_token_accounting

**Purpose:** Per-task token cost tracking and fleet budget management with soft/hard limits.

**Key Types:**
- `TokenUsage` -- input_tokens, output_tokens, total_tokens, estimated_cost_usd
- `TokenAccountant` -- per-pane usage (`BTreeMap`), per-task records (`VecDeque`, cap 5000), fleet totals, budget config (`RwLock`)
- `BudgetStatus` -- enum: WithinBudget, SoftLimitReached, HardLimitReached

**Public API:**
- `fn TokenAccountant::new() -> Self`
- `fn TokenAccountant::record_pane_usage(pane_id, input, output)`
- `fn TokenAccountant::record_task_usage(task_id, pane_id, input, output)`
- `fn TokenAccountant::fleet_total() -> TokenUsage`
- `fn TokenAccountant::pane_usage(pane_id) -> Option<TokenUsage>`
- `fn TokenAccountant::budget_status() -> BudgetStatus`
- `fn TokenAccountant::summary() -> String` -- human-readable fleet summary

**Dependencies:** m01 (`PaneId`, `TaskId`), m02 (`PvError`)
**Tests:** 55
**Feature Gate:** `monitoring`

---

## Layer 8: Evolution

**Directory:** `src/m8_evolution/` | **Feature Gate:** `evolution`
**Modules:** 5 named (m36-m40) | **Total Tests:** ~214

Critical BUG-035 fix: multi-parameter mutation (NOT mono-parameter like ME).

---

### m36_ralph_engine

**Purpose:** 5-phase RALPH meta-learning loop orchestrator: Recognize, Analyze, Learn, Propose, Harvest. Drives system parameter evolution with atomic snapshot/rollback.

**Key Types:**
- `RalphPhase` -- enum: Recognize, Analyze, Learn, Propose, Harvest
- `RalphEngine` -- phase, generation, fitness_tensor, emergence_detector, correlation_engine, mutation_selector, snapshots (`VecDeque`), config (`RwLock`)
- `RalphEngineConfig` -- accept_threshold (0.02), rollback_threshold (-0.01), verification_ticks (10), max_cycles (1000), snapshot_capacity (50)
- `RalphSnapshot` -- generation, parameter values, fitness, timestamp

**Public API:**
- `fn RalphEngine::new() -> Self` / `fn with_config(config) -> Self`
- `fn RalphEngine::tick(tensor_values: &TensorValues, tick: u64) -> PvResult<RalphPhase>`
- `fn RalphEngine::generation() -> u64`
- `fn RalphEngine::current_phase() -> RalphPhase`
- `fn RalphEngine::fitness() -> f64`
- `fn RalphEngine::snapshot() -> RalphSnapshot` / `fn rollback(snapshot)`
- `fn RalphEngine::to_json() -> String` / `fn from_json(json) -> PvResult<Self>`

**Dependencies:** m02, m37 (`EmergenceDetector`), m38 (`CorrelationEngine`), m39 (`FitnessTensor`), m40 (`MutationSelector`)
**Tests:** 29
**Feature Gate:** `evolution`

---

### m37_emergence_detector

**Purpose:** Detects 8 types of emergent fleet coordination behaviors using a ring buffer (cap 5000) with TTL-based decay and configurable monitors.

**Key Types:**
- `EmergenceType` -- enum: CoherenceLock, ChimeraFormation, CouplingRunaway, HebbianSaturation, DispatchLoop, ThermalSpike, BeneficialSync, ConsentCascade
- `EmergenceRecord` -- emergence_type, confidence, tick, metadata
- `EmergenceDetector` -- history (`VecDeque`, cap 5000), monitors, config (`RwLock`)
- `EmergenceMonitor` -- per-type detection state and thresholds

**Public API:**
- `fn EmergenceDetector::new() -> Self`
- `fn EmergenceDetector::observe(emergence_type, confidence, tick, metadata) -> PvResult<bool>`
- `fn EmergenceDetector::check_coherence_lock(r, tick) -> Option<EmergenceRecord>`
- `fn EmergenceDetector::check_beneficial_sync(r, prev_r, tick) -> Option<EmergenceRecord>`
- `fn EmergenceDetector::check_hebbian_saturation(floor_ratio, tick) -> Option<EmergenceRecord>`
- `fn EmergenceDetector::recent(limit) -> Vec<EmergenceRecord>`
- `fn EmergenceDetector::count_by_type() -> HashMap<EmergenceType, usize>`
- `fn EmergenceDetector::tick_decay(current_tick)` -- TTL-based pruning

**Dependencies:** m02 (`PvError`)
**Tests:** 52
**Feature Gate:** `evolution`

---

### m38_correlation_engine

**Purpose:** Discovers correlations between emergence events, parameter changes, and fitness outcomes. Mines temporal, causal, recurring, and fitness-linked pathways.

**Key Types:**
- `CorrelationType` -- enum: Temporal, Causal, Recurring, FitnessLinked
- `CorrelationRecord` -- correlation_type, events, confidence, tick, pattern_key
- `Pathway` -- pattern_key, correlation_type, confidence, occurrence_count, established
- `CorrelationEngine` -- event_buffer, pathways (`HashMap`), history (`VecDeque`, cap 1000), config (`RwLock`)

**Public API:**
- `fn CorrelationEngine::new() -> Self`
- `fn CorrelationEngine::observe(emergence_type, tick, fitness_delta) -> PvResult<Vec<CorrelationRecord>>`
- `fn CorrelationEngine::established_pathways() -> Vec<&Pathway>`
- `fn CorrelationEngine::pathway_count() -> usize`
- `fn CorrelationEngine::recent_correlations(limit) -> Vec<CorrelationRecord>`

**Dependencies:** m02 (`PvError`), m37 (`EmergenceType`)
**Tests:** 32
**Feature Gate:** `evolution`

---

### m39_fitness_tensor

**Purpose:** 12-dimensional weighted fitness evaluation for RALPH. Evaluates fleet coordination health with trend detection via linear regression and stability assessment.

**Key Types:**
- `FitnessDimension` -- enum with 12 variants: CoordinationQuality (D0, weight 0.18), FieldCoherence (D1, 0.15), DispatchAccuracy (D2, 0.12), TaskThroughput (D3, 0.10), ErrorRate (D4, 0.10), Latency (D5, 0.08), HebbianHealth (D6, 0.07), CouplingStability (D7, 0.06), ThermalBalance (D8, 0.05), FleetUtilization (D9, 0.04), EmergenceRate (D10, 0.03), ConsentCompliance (D11, 0.02)
- `TensorValues` -- `[f64; 12]` wrapper
- `FitnessTensor` -- current values, history (`VecDeque`, cap 200), weights, trend_window (`RwLock`)
- `FitnessTrend` -- enum: Improving, Stable, Declining, Volatile
- `SystemState` -- fitness, trend, dimension_scores, stability

**Public API:**
- `fn FitnessTensor::new() -> Self`
- `fn FitnessTensor::evaluate(values: &TensorValues) -> f64` -- weighted dot product
- `fn FitnessTensor::update(values: &TensorValues)` -- push to history + recompute
- `fn FitnessTensor::trend() -> FitnessTrend` -- linear regression on history
- `fn FitnessTensor::system_state() -> SystemState` -- full state summary
- `const DIMENSION_WEIGHTS: [f64; 12]` -- sum = 1.0
- `const DIMENSION_NAMES: [&str; 12]`

**Dependencies:** m02 (`PvError`)
**Tests:** 62
**Feature Gate:** `evolution`

---

### m40_mutation_selector

**Purpose:** Diversity-enforced parameter selection for RALPH. BUG-035 fix: round-robin cycling, 10-generation cooldown, >50% diversity rejection gate.

**Key Types:**
- `MutableParameter` -- name, current_value, min/max_value, target_value, description
- `MutationProposal` -- parameter_name, old_value, new_value, delta, direction, reason
- `MutationSelector` -- parameter pool, cooldown tracker, diversity window (`VecDeque`, cap 20), round-robin index, history (`VecDeque`, cap 1000), config (`RwLock`)

**Public API:**
- `fn MutationSelector::new() -> Self`
- `fn MutationSelector::register(param: MutableParameter)` -- add to parameter pool
- `fn MutationSelector::propose(fitness_trend) -> PvResult<Option<MutationProposal>>`
  - Round-robin across full pool
  - 10-generation cooldown per parameter
  - Reject if >50% of last 20 mutations hit same parameter
- `fn MutationSelector::accept(proposal)` / `fn reject(proposal)`
- `fn MutationSelector::diversity_score() -> f64` -- 0.0 = mono-parameter, 1.0 = perfect spread

**Dependencies:** m02 (`PvError`)
**Tests:** 39
**Feature Gate:** `evolution`

---

## Feature Gate Matrix

| Feature | Cargo.toml | Layers | Modules | External Deps |
|---------|-----------|--------|---------|---------------|
| `api` | default | L3 | m10-m14 | axum 0.8, tower-http 0.6 |
| `persistence` | default | L5 (m26 only) | m26 | rusqlite 0.32 |
| `bridges` | default | L5 | m22-m25, http_helpers | (none -- raw TCP) |
| `intelligence` | default | L4 | m15-m21 | tower 0.5 |
| `monitoring` | default | L7 | m32-m35 | opentelemetry 0.27, opentelemetry-otlp 0.27 |
| `evolution` | default | L8 | m36-m40 | (none) |
| `full` | alias | All | All | All above |

**Default features:** `api`, `persistence`, `bridges`, `intelligence`, `monitoring`, `evolution` -- all six enabled by default since Session 055.

**Compile-time gating in lib.rs:**

```rust
pub mod m1_core;                          // always
pub mod m2_wire;                          // always
#[cfg(feature = "api")]        pub mod m3_hooks;
#[cfg(feature = "intelligence")] pub mod m4_intelligence;
#[cfg(feature = "bridges")]    pub mod m5_bridges;
pub mod m6_coordination;                  // always
#[cfg(feature = "monitoring")] pub mod m7_monitoring;
#[cfg(feature = "evolution")]  pub mod m8_evolution;
```

---

## Binary Entry Points

### orac-sidecar (main daemon)

**File:** `src/bin/main.rs` | **Size:** ~5.5 MB (release)
**Entry:** `#[tokio::main] async fn main()`

Orchestrates 8 subsystems:
1. Loads `PvConfig` (TOML + env overlay)
2. Creates `OracState` with all bridges, registries, and engines
3. Hydrates persisted state (RALPH snapshots, sessions, coupling weights from blackboard)
4. Spawns field state poller (PV2 health every 5s)
5. Spawns IPC client connection to PV2 bus
6. Spawns RALPH evolution loop (30s tick interval, 5-phase cycle)
7. Spawns STDP update loop (applies Hebbian learning on tick)
8. Spawns emergence observation feed (feeds detector from field/coupling state)
9. Binds Axum HTTP server on `:8133` with graceful shutdown

**Feature gates:** Extensive `#[cfg(feature = "...")]` blocks for optional subsystems.

### orac-client (CLI)

**File:** `src/bin/client.rs` | **Size:** ~337 KB (release)
**Entry:** `fn main() -> ExitCode`

10 subcommands:
- `status` -- sidecar health and session info
- `field` -- Kuramoto field state (r, K, spheres)
- `blackboard` -- fleet blackboard state
- `metrics` -- Prometheus text format dump
- `hook-test <event>` -- send test payload to hook endpoint
- `probe` -- connectivity checks (6 services)
- `watch` -- live dashboard polling every 2s
- `dispatch <desc>` -- submit task to fleet via PV2 bus
- `fleet` -- list registered spheres with status
- `completions <shell>` -- bash/zsh/fish completions

Uses `ureq` for synchronous HTTP to `127.0.0.1:8133`. Supports `--json` flag for machine-readable output.

### orac-probe (diagnostics)

**File:** `src/bin/probe.rs` | **Size:** ~2.3 MB (release)
**Entry:** `fn main() -> ExitCode`

Quick connectivity check against 6 endpoints:
- ORAC HTTP (`127.0.0.1:8133/health`)
- PV2 daemon (`127.0.0.1:8132/health`)
- SYNTHEX (`127.0.0.1:8090/api/health`)
- ME (`127.0.0.1:8080/api/health`)
- POVM (`127.0.0.1:8125/health`)
- RM (`127.0.0.1:8130/health`)

Returns exit code 0 if all reachable, 1 otherwise. Timeout: 2s per endpoint.

### ralph-bench (benchmark)

**File:** `src/bin/ralph_bench.rs` | **Feature:** requires `evolution`
**Entry:** `fn main()`

Benchmarks RALPH tick CPU cost across sphere counts (1, 8, 16, 32, 64). Runs 500 ticks per configuration with 50-tick warmup. Uses `std::time::Instant` for timing. Target: <100us per tick for 64 spheres (5s budget).

---

## Hot-Swap Sources

Modules originated from pane-vortex-v2 (`~/claude-code-workspace/pane-vortex-v2/src/`), categorized by integration method:

### DROP-IN (copied with minimal changes)

| ORAC Module | PV2 Source Module | Files | Changes |
|-------------|-------------------|-------|---------|
| m01_core_types | m1_foundation/m01 | 1 | Namespace rename |
| m02_error_handling | m1_foundation/m02 | 1 | Namespace rename |
| m03_config | m1_foundation/m03 | 1 | Namespace rename |
| m04_constants | m1_foundation/m04 | 1 | Namespace rename |
| m05_traits | m1_foundation/m05 | 1 | Namespace rename |
| m06_validation | m1_foundation/m06 | 1 | Namespace rename |
| m07_ipc_client | m7_coordination/m29 | 1 | Client role (was server) |
| m08_bus_types | m7_coordination/m30 | 1 | Namespace rename |
| m15_coupling_network | m4_coupling/m16 | 1 | Module number shift |
| m16_auto_k | m4_coupling/m17 | 1 | Module number shift |
| m17_topology | m4_coupling/m18 | 1 | Module number shift |
| m18_hebbian_stdp | m5_learning/m19 | 1 | Added G1 idle guard |
| m19_buoy_network | m5_learning/m20 | 1 | Module number shift |
| m28_cascade | m7_coordination/m33 | 1 | Namespace rename |
| m31_memory_manager | m5_learning/m21 | 1 | Module number shift |

### ADAPT (significant ORAC-specific changes)

| ORAC Module | PV2 Source Module | Key Changes |
|-------------|-------------------|-------------|
| field_state | m3_field (multiple) | Observer model (read-only cache vs authoritative), staleness tracking, chimera detection |
| m22_synthex_bridge | m6_bridges/m22 | Raw TCP (BUG-033), configurable URL, consent stub |
| m23_me_bridge | m6_bridges/m24 | BUG-008 frozen detection, 3-poll neutral fallback |
| m24_povm_bridge | m6_bridges/m25 | 2MB response limit, serde aliases, hydrate methods |
| m25_rm_bridge | m6_bridges/m26 | TSV-only (AP05), agent name "orac-sidecar" |
| m27_conductor | m7_coordination/m31 | Advisory-only (no authoritative k_modulation) |
| m29_tick | m7_coordination/m35 | Observer model, optional STDP, no direct sphere mutation |

### NEW (written for ORAC, no PV2 origin)

| ORAC Module | Purpose |
|-------------|---------|
| m09_wire_protocol | V2 wire protocol state machine |
| m10_hook_server | Axum HTTP server, `OracState` |
| m11_session_hooks | SessionStart/Stop handlers |
| m12_tool_hooks | PostToolUse/PreToolUse handlers |
| m13_prompt_hooks | UserPromptSubmit handler |
| m14_permission_policy | PermissionRequest policy engine |
| m20_semantic_router | Content-aware dispatch |
| m21_circuit_breaker | Per-pane health FSM |
| m26_blackboard | SQLite shared fleet state |
| m30_wasm_bridge | FIFO/ring WASM protocol |
| http_helpers | Shared raw TCP HTTP utilities |
| m32_otel_traces | OpenTelemetry trace store |
| m33_metrics_export | Prometheus metrics |
| m34_field_dashboard | Kuramoto dashboard |
| m35_token_accounting | Token cost tracking |
| m36_ralph_engine | 5-phase RALPH loop |
| m37_emergence_detector | 8-type emergence detection |
| m38_correlation_engine | Pathway discovery |
| m39_fitness_tensor | 12D weighted fitness |
| m40_mutation_selector | Diversity-enforced mutation (BUG-035 fix) |

---

## Cross-References

- **D1:** `docs/D1_SYSTEM_ATLAS.md` -- system overview and layer map
- **D2:** `docs/D2_ARCHITECTURAL_SCHEMATICS.md` -- Mermaid diagrams
- **D3:** `docs/D3_ENDPOINT_PROTOCOL_CATALOG.md` -- all 22 HTTP routes
- **D4:** `docs/D4_BRIDGE_PIPELINE_WIRING.md` -- bridge data flow
- **D5:** `docs/D5_CODE_FLOW_MAPS.md` -- request/tick lifecycle flows
- **D6:** `docs/D6_CAPACITY_LIMITS_REFERENCE.md` -- constants and limits
- **D8:** `docs/D8_MERMAID_DIAGRAMS.md` -- visual diagrams
- **Obsidian:** `[[Session 061 — ORAC System Atlas]]`
- **Obsidian:** `[[ORAC Sidecar — Architecture Schematics]]`
- **Obsidian:** `[[ULTRAPLATE Master Index]]`

---

*Generated from ACP-verified source analysis. 55 `.rs` files read. 42 source modules documented. 1684 tests counted across all modules.*
