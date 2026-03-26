# D2: ORAC Sidecar -- Architectural Schematics

> **Version:** 0.6.0 | **Verified:** 2026-03-25 (Round 3 module name verification)
> **Scope:** 55 files, 40 modules, 8 layers, 4 binaries, 41,369 LOC, 1,690+ tests

---

## 1. Layer Dependency DAG (Corrected)

The ORAC codebase is organized into 8 layers with strict unidirectional dependencies enforced at compile time via Rust module visibility and feature gates. No layer may import from a higher-numbered layer.

```
L8 Evolution ─────────────────────────────────────────────────────
 │  m36_ralph_engine, m37_emergence_detector, m38_correlation_engine,
 │  m39_fitness_tensor, m40_mutation_selector
 │  Feature: "evolution"
 │  Depends on: L1, L4, L5, L7
 │
L7 Monitoring ────────────────────────────────────────────────────
 │  m32_otel_traces, m33_metrics_export, m34_field_dashboard,
 │  m35_token_accounting
 │  Feature: "monitoring"
 │  Depends on: L1, L2, L5
 │
L6 Coordination ──────────────────────────────────────────────────
 │  m27_conductor, m28_cascade, m29_tick, m30_wasm_bridge,
 │  m31_memory_manager
 │  Always compiled (no feature gate)
 │  Depends on: L1, L2, L4, L5
 │
L5 Bridges ───────────────────────────────────────────────────────
 │  m22_synthex_bridge, m23_me_bridge, m24_povm_bridge,
 │  m25_rm_bridge, m26_blackboard, http_helpers
 │  Feature: "bridges" (m26 also gated by "persistence")
 │  Depends on: L1
 │
L4 Intelligence ──────────────────────────────────────────────────
 │  m15_coupling_network, m16_auto_k, m17_topology,
 │  m18_hebbian_stdp, m19_buoy_network, m20_semantic_router,
 │  m21_circuit_breaker
 │  Feature: "intelligence"
 │  Depends on: L1, L2
 │
L3 Hooks (KEYSTONE) ──────────────────────────────────────────────
 │  m10_hook_server, m11_session_hooks, m12_tool_hooks,
 │  m13_prompt_hooks, m14_permission_policy
 │  Feature: "api"
 │  Depends on: L1, L2 (and cross-references L4, L5, L7, L8 via
 │  feature-gated conditional compilation)
 │
L2 Wire ──────────────────────────────────────────────────────────
 │  m07_ipc_client, m08_bus_types, m09_wire_protocol
 │  Always compiled (no feature gate)
 │  Depends on: L1
 │
L1 Core (Foundation) ─────────────────────────────────────────────
    m01_core_types, m02_error_handling, m03_config, m04_constants,
    m05_traits, m06_validation, field_state
    Always compiled (no feature gate)
    Depends on: nothing
```

### DAG Arrows (7 verified dependency edges)

| Arrow | Direction | Verification |
|-------|-----------|--------------|
| L2 -> L1 | Wire imports core types, errors | m07 uses `PaneId`, m08 uses `now_secs`/`TaskId`, m09 uses `PvError` |
| L3 -> L1, L2 | Hooks import types + IPC | m10 uses `PvConfig`/`SharedState`, m11-m14 use `PaneId` |
| L4 -> L1, L2 | Intelligence imports types + bus | m15-m21 use core types, m20 uses `PaneStatus` |
| L5 -> L1 | Bridges import types only | m22-m26 use `PvError`/`PvResult`, http_helpers is self-contained |
| L6 -> L1, L2, L4, L5 | Coordination imports broadly | m27/m29 use field_state+coupling, m28 uses core types, m31 uses memory types |
| L7 -> L1, L2, L5 | Monitoring imports types | m32-m35 use `PaneId`/`TaskId`/`now_secs`, m34 uses `OrderParameter` |
| L8 -> L1, L4, L5, L7 | Evolution imports intelligence | m36-m40 use core types; L4 coupling feeds m39 tensor |

**Note on L3 cross-references:** `m10_hook_server` (the keystone) holds `OracState` which contains fields from L4 (`CouplingNetwork`, `BreakerRegistry`), L5 (`Blackboard`, `SynthexBridge`, `MeBridge`, `RmBridge`), L7 (`TraceStore`, `FieldDashboard`, `TokenAccountant`), and L8 (`RalphEngine`). These are feature-gated at the field level using `#[cfg(feature = "...")]`, not at the import level. This is the architectural decision that makes L3 the keystone layer.

---

## 2. Module Inventory (55 Files)

### L1: Core (m1_core/) -- 4,071 LOC, 201 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m01_core_types.rs` | m01 | 1,128 | 52 | -- | `PaneId`, `TaskId`, `OrderParameter`, `FleetMode`, `PaneSphere`, `PaneStatus`, `Timestamp`, `now_secs()` |
| `m02_error_handling.rs` | m02 | 595 | 29 | -- | `PvError` unified error enum, `PvResult<T>` type alias, `ErrorClassifier` trait |
| `m03_config.rs` | m03 | 653 | 25 | -- | `PvConfig` TOML + env overlay (server, IPC, bridges, hooks, evolution) via figment |
| `m04_constants.rs` | m04 | 298 | 14 | -- | `HEBBIAN_WEIGHT_FLOOR`, intervals, budgets, thresholds, limits |
| `m05_traits.rs` | m05 | 65 | 1 | -- | `Bridgeable` trait (Send + Sync) |
| `m06_validation.rs` | m06 | 540 | 51 | -- | Input validators: persona, tool_name, summary, frequency, phase, body |
| `field_state.rs` | (support) | 754 | 29 | -- | `AppState`, `SharedState`, `FieldState`, `FieldDecision`, `new_shared_state()` |
| `mod.rs` | (coordinator) | 38 | 0 | -- | Layer re-exports and documentation |

### L2: Wire (m2_wire/) -- 3,028 LOC, 118 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m07_ipc_client.rs` | m07 | 980 | 11 | -- | Unix socket client, `connect_with_backoff()`, `subscribe()`, `recv_frame()`, reconnect FSM |
| `m08_bus_types.rs` | m08 | 1,035 | 67 | -- | `BusFrame`, `BusEvent`, task lifecycle types, NDJSON serialization |
| `m09_wire_protocol.rs` | m09 | 1,003 | 40 | -- | V2 wire state machine (Disconnected/Handshaking/Connected/Subscribing/Active), frame validation |
| `mod.rs` | (coordinator) | 10 | 0 | -- | Layer re-exports |

### L3: Hooks (m3_hooks/) -- 5,694 LOC, 206 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m10_hook_server.rs` | m10 | 3,295 | 76 | `api` | Axum HTTP router, `OracState` (32 fields), `build_router()`, `spawn_field_poller()`, 18 endpoints |
| `m11_session_hooks.rs` | m11 | 403 | 24 | `api` | `SessionStart` (register, POVM hydrate), `Stop` (deregister, quality gate) |
| `m12_tool_hooks.rs` | m12 | 1,123 | 56 | `api` | `PostToolUse` (STDP, task poll, blackboard write), `PreToolUse` (thermal gate) |
| `m13_prompt_hooks.rs` | m13 | 471 | 25 | `api` | `UserPromptSubmit` (inject r/tick/spheres/thermal + pending tasks) |
| `m14_permission_policy.rs` | m14 | 362 | 25 | `api` | `PermissionRequest` auto-approve/deny engine (read=allow, write=notice, deny list) |
| `mod.rs` | (coordinator) | 40 | 0 | `api` | Layer re-exports and hook flow documentation |

### L4: Intelligence (m4_intelligence/) -- 4,752 LOC, 237 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m15_coupling_network.rs` | m15 | 906 | 43 | `intelligence` | Kuramoto coupling matrix, phase dynamics, `CouplingConnection` management |
| `m16_auto_k.rs` | m16 | 365 | 23 | `intelligence` | Adaptive coupling strength, consent-gated K adjustment |
| `m17_topology.rs` | m17 | 457 | 28 | `intelligence` | Network topology analysis (degree distribution, clustering) |
| `m18_hebbian_stdp.rs` | m18 | 608 | 30 | `intelligence` | `apply_stdp()` LTP/LTD dynamics, tool co-activation, `StdpResult` |
| `m19_buoy_network.rs` | m19 | 449 | 23 | `intelligence` | Health tracking buoys, spatial recall, decay |
| `m20_semantic_router.rs` | m20 | 817 | 45 | `intelligence` | 4 semantic domains (Read/Write/Execute/Communicate), `classify_content()`, `route()` |
| `m21_circuit_breaker.rs` | m21 | 1,107 | 45 | `intelligence` | Per-pane Closed/Open/HalfOpen FSM, `BreakerRegistry`, `tick_all()` |
| `mod.rs` | (coordinator) | 43 | 0 | `intelligence` | Layer re-exports |

### L5: Bridges (m5_bridges/) -- 7,074 LOC, 339 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `http_helpers.rs` | (support) | 650 | 29 | `bridges` | `raw_http_post()`, `raw_http_post_with_response()`, shared TCP helpers |
| `m22_synthex_bridge.rs` | m22 | 850 | 56 | `bridges` | SYNTHEX :8090 bridge -- thermal read, field state post, k_adjustment |
| `m23_me_bridge.rs` | m23 | 924 | 52 | `bridges` | ME :8080 bridge -- observer poll, fitness signal, frozen detection |
| `m24_povm_bridge.rs` | m24 | 1,127 | 60 | `bridges` | POVM :8125 bridge -- memory hydrate, pathway persist, serde alias fix |
| `m25_rm_bridge.rs` | m25 | 880 | 52 | `bridges` | RM :8130 bridge -- TSV-only persistence, `RmRecord`, search |
| `m26_blackboard.rs` | m26 | 2,603 | 90 | `persistence` | SQLite shared state: 9 tables, RALPH state, sessions, coupling weights, hebbian summary |
| `mod.rs` | (coordinator) | 40 | 0 | `bridges` | Layer re-exports and protocol rules |

### L6: Coordination (m6_coordination/) -- 2,968 LOC, 133 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m27_conductor.rs` | m27 | 511 | 25 | -- | PI controller for field breathing rhythm, `FieldDecision` |
| `m28_cascade.rs` | m28 | 856 | 46 | -- | Cascade handoff protocol with sphere mitosis (SYS-1) |
| `m29_tick.rs` | m29 | 452 | 13 | -- | `tick_once()` orchestrator, Hebbian Phase 2.5 wiring |
| `m30_wasm_bridge.rs` | m30 | 729 | 34 | -- | FIFO/ring bridge to Zellij WASM plugin, `EventRingBuffer` (1000-line cap) |
| `m31_memory_manager.rs` | m31 | 381 | 15 | -- | Memory aggregation, pruning (activation < 0.05 every 200 steps, 500/sphere cap) |
| `mod.rs` | (coordinator) | 39 | 0 | -- | Layer re-exports |

### L7: Monitoring (m7_monitoring/) -- 4,467 LOC, 236 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m32_otel_traces.rs` | m32 | 1,360 | 73 | `monitoring` | `TraceStore`, `SpanBuilder`, OTel-compatible span lifecycle |
| `m33_metrics_export.rs` | m33 | 1,130 | 60 | `monitoring` | Prometheus text format, 5 metric families |
| `m34_field_dashboard.rs` | m34 | 943 | 48 | `monitoring` | Kuramoto field dashboard: per-cluster r, phase gaps, K effective, chimera detection |
| `m35_token_accounting.rs` | m35 | 1,000 | 55 | `monitoring` | Per-task token cost, fleet budget, pane-level accounting |
| `mod.rs` | (coordinator) | 34 | 0 | `monitoring` | Layer re-exports |

### L8: Evolution (m8_evolution/) -- 7,524 LOC, 214 tests

| File | Module | LOC | Tests | Feature Gate | Purpose |
|------|--------|-----|-------|--------------|---------|
| `m36_ralph_engine.rs` | m36 | 1,233 | 29 | `evolution` | 5-phase RALPH: Recognize/Analyze/Learn/Propose/Harvest, snapshot/rollback, gen tracking |
| `m37_emergence_detector.rs` | m37 | 1,725 | 52 | `evolution` | 8 emergence types, ring buffer, TTL decay, 5000-event cap, monitor API |
| `m38_correlation_engine.rs` | m38 | 1,076 | 32 | `evolution` | Temporal/causal/recurring/fitness-linked correlation mining |
| `m39_fitness_tensor.rs` | m39 | 1,348 | 62 | `evolution` | 12D weighted fitness: `TensorValues`, `FitnessDimension`, trend/stability assessment |
| `m40_mutation_selector.rs` | m40 | 1,103 | 39 | `evolution` | BUG-035 fix: round-robin, 10-gen cooldown, >50% diversity rejection gate |
| `mod.rs` | (coordinator) | 39 | 0 | `evolution` | Layer re-exports |

### Binaries (src/bin/) -- 2,743 LOC

| File | Binary | LOC | Purpose |
|------|--------|-----|---------|
| `main.rs` | `orac-sidecar` | 1,779 | Daemon: config -> OracState -> hydrate -> spawn_field_poller -> spawn_ipc -> spawn_ralph -> axum |
| `client.rs` | `orac-client` | 804 | CLI: status, field, blackboard, metrics, hook-test, probe, watch, dispatch, fleet, completions |
| `probe.rs` | `orac-probe` | 40 | Diagnostics: connectivity check to ORAC + PV2 + SYNTHEX + ME + POVM + RM |
| `ralph_bench.rs` | `ralph-bench` | 120 | Benchmark: RALPH tick CPU cost across 1/8/16/32/64 sphere counts |

### Shared (src/) -- 48 LOC

| File | LOC | Purpose |
|------|-----|---------|
| `lib.rs` | 48 | 8 layer declarations, feature-gated `pub mod` |

### Summary

| Category | Files | LOC | Tests |
|----------|-------|-----|-------|
| L1 Core | 8 | 4,071 | 201 |
| L2 Wire | 4 | 3,028 | 118 |
| L3 Hooks | 6 | 5,694 | 206 |
| L4 Intelligence | 8 | 4,752 | 237 |
| L5 Bridges | 7 | 7,074 | 339 |
| L6 Coordination | 6 | 2,968 | 133 |
| L7 Monitoring | 5 | 4,467 | 236 |
| L8 Evolution | 6 | 7,524 | 214 |
| Binaries | 4 | 2,743 | 0 |
| lib.rs | 1 | 48 | 0 |
| **TOTAL** | **55** | **42,369** | **1,684** |

---

## 3. Cross-Layer Import Map

Verified by tracing all `use crate::` statements across the codebase.

| Importing Layer | Imports From | Key Types Used |
|-----------------|-------------|----------------|
| L2 Wire | L1 Core | `PaneId`, `TaskId`, `now_secs()`, `PvError`, `PvResult` |
| L3 Hooks | L1 Core | `PvConfig`, `SharedState`, `FieldState`, `PaneId`, `PaneStatus` |
| L3 Hooks | L2 Wire | (via `OracState` -- IPC state tracking) |
| L3 Hooks | L4 Intelligence | `CouplingNetwork`, `BreakerRegistry`, `BreakerConfig`, `SemanticDomain`, `classify_content()`, `route()` |
| L3 Hooks | L5 Bridges | `Blackboard`, `PaneRecord`, `TaskRecord`, `AgentCard`, `SynthexBridge`, `MeBridge`, `RmBridge`, `GhostRecord`, `ConsentAuditEntry` |
| L3 Hooks | L7 Monitoring | `TraceStore`, `SpanBuilder`, `FieldDashboard`, `TokenAccountant` |
| L3 Hooks | L8 Evolution | `RalphEngine`, `EmergenceDetector`, `TensorValues`, `FitnessDimension` |
| L4 Intelligence | L1 Core | `PaneId`, `PaneStatus`, `PaneSphere`, `OrderParameter`, constants |
| L4 Intelligence | L2 Wire | (bus event types for STDP trigger) |
| L5 Bridges | L1 Core | `PvError`, `PvResult` (m22-m25), rusqlite via `persistence` feature (m26) |
| L6 Coordination | L1 Core | `FieldState`, `PaneSphere`, `OrderParameter`, `SphereMemory`, `Point3D`, constants |
| L6 Coordination | L2 Wire | (bus frame types for cascade) |
| L6 Coordination | L4 Intelligence | `CouplingNetwork`, `auto_k`, `topology` (via m29_tick) |
| L6 Coordination | L5 Bridges | (blackboard types via m28_cascade) |
| L7 Monitoring | L1 Core | `PaneId`, `TaskId`, `now_secs()`, `PvResult`, `OrderParameter`, constants |
| L8 Evolution | L1 Core | Core types (via L4/L5 re-exports) |

### Cross-Layer Reference Matrix

```
             L1  L2  L3  L4  L5  L6  L7  L8
L1 Core       .   -   -   -   -   -   -   -
L2 Wire       X   .   -   -   -   -   -   -
L3 Hooks      X   X   .   X   X   -   X   X    <-- keystone: widest imports
L4 Intell     X   X   -   .   -   -   -   -
L5 Bridges    X   -   -   -   .   -   -   -
L6 Coord      X   X   -   X   X   .   -   -
L7 Monitor    X   X   -   -   X   -   .   -
L8 Evolve     X   -   -   X   X   -   X   .

X = imports from     - = no imports     . = self
```

---

## 4. Binary Entry Points

### `orac-sidecar` (src/bin/main.rs) -- 1,779 LOC -- Daemon

The primary daemon binary. Startup sequence:

1. Initialize `tracing_subscriber` with env filter
2. Load `PvConfig` from TOML + environment
3. Construct `Arc<OracState>` with all 32 fields
4. `hydrate_startup_state()` -- restore RALPH gen/fitness, sessions, coupling weights from blackboard, then POVM pathways as fallback
5. `spawn_field_poller()` -- background task polling PV2 :8132/health, updating `SharedState`
6. `spawn_ipc_listener()` -- background task connecting to PV2 Unix socket bus, subscribing to `field.*` + `sphere.*`, processing `BusEvent` stream
7. `spawn_ralph_loop()` -- 5s interval background task: breaker tick, conductor advisory, STDP pass, tensor build, RALPH tick, emergence detection, bridge posts (SYNTHEX every 6 ticks, VMS every 30 ticks, ME every 12 ticks, RM every 60 ticks, blackboard every 60 ticks, VMS consolidation every 300 ticks)
8. `build_router()` + `axum::serve()` on :8133 with graceful SIGINT shutdown

**Key functions defined in main.rs:**
- `hydrate_startup_state()` -- 4-step state restoration
- `spawn_ipc_listener()` -- IPC reconnect loop with escalating backoff (5s-30s)
- `process_bus_event()` -- field.tick, sphere.registered/deregistered/status handlers
- `feed_emergence_observations()` -- 8 emergence detectors sampled per tick
- `post_field_to_synthex()` -- 12 heat source fields to SYNTHEX /api/ingest
- `persist_stdp_to_povm()` -- top 10 coupling weights to POVM /pathways
- `post_state_to_vms()` -- RALPH state memory to VMS /mcp/tools/call
- `trigger_vms_consolidation()` -- VMS /v1/adaptation/trigger
- `query_vms_for_ralph_context()` -- VMS semantic query during Recognize phase
- `post_state_to_rm()` -- TSV record to RM /put
- `relay_emergence_to_rm()` -- emergence events to RM
- `build_tensor_from_state()` -- 12D fitness tensor from live state
- `spawn_ralph_loop()` -- main RALPH tick loop orchestrator

### `orac-client` (src/bin/client.rs) -- 804 LOC -- CLI

Subcommands: `status`, `field`, `blackboard`, `metrics`, `hook-test`, `probe`, `watch`, `dispatch`, `fleet`, `completions`, `help`. Connects to ORAC :8133 via HTTP. Supports `--json` flag for machine-readable output.

### `orac-probe` (src/bin/probe.rs) -- 40 LOC -- Diagnostics

Connectivity check hitting 6 endpoints: ORAC :8133, PV2 :8132, SYNTHEX :8090, ME :8080, POVM :8125, RM :8130. Returns pass/fail per endpoint with HTTP status codes.

### `ralph-bench` (src/bin/ralph_bench.rs) -- 120 LOC -- Benchmark

Measures RALPH `tick()` CPU cost across sphere counts [1, 8, 16, 32, 64] with 500 timed iterations after 50 warmup ticks. Reports total/per-tick/min/max timings. Requires `--features evolution`.

---

## 5. OracState Structure (32 Fields)

Defined in `src/m3_hooks/m10_hook_server.rs` line 431. Wrapped in `Arc<OracState>` and shared across all Axum handlers and background tasks.

### Configuration and Topology (5 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `config` | `PvConfig` | -- | Immutable config (server, IPC, bridges, hooks, evolution) |
| `pv2_url` | `String` | -- | PV2 daemon HTTP URL (`http://127.0.0.1:8132`) |
| `synthex_url` | `String` | -- | SYNTHEX HTTP URL (`http://127.0.0.1:8090`) |
| `povm_url` | `String` | -- | POVM HTTP URL (`http://127.0.0.1:8125`) |
| `rm_url` | `String` | -- | Reasoning Memory HTTP URL (`http://127.0.0.1:8130`) |

### Core State (5 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `field_state` | `SharedState` (= `RwLock<AppState>`) | -- | Cached PV2 field: r, psi, tick, spheres, decisions |
| `sessions` | `RwLock<HashMap<String, SessionTracker>>` | -- | Per-session tracking keyed by session ID |
| `tick` | `AtomicU64` | -- | Global tick counter (5s interval, monotonic) |
| `ipc_state` | `RwLock<String>` | -- | IPC bus state: "disconnected"/"connected"/"subscribed"/"failed(N)" |
| `ghosts` | `RwLock<VecDeque<OracGhost>>` | -- | Ghost traces of deregistered spheres (FIFO, max 20) |

### Consent and Governance (1 field)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `consents` | `RwLock<HashMap<String, OracConsent>>` | -- | Per-sphere consent declarations (FIX-018), gates POVM writes |

### Persistence (1 field)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `blackboard` | `Option<Mutex<Blackboard>>` | `persistence` | SQLite shared fleet state: 9 tables (pane_status, task_history, agent_cards, ralph_state, sessions, coupling_weights, hebbian_summary, ghost_records, consent_audit) |

### Evolution (1 field)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `ralph` | `RalphEngine` | `evolution` | 5-phase RALPH engine with EmergenceDetector, CorrelationEngine, MutationSelector |

### Intelligence (2 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `coupling` | `RwLock<CouplingNetwork>` | -- | Hebbian coupling matrix: connections, K, k_modulation |
| `breakers` | `RwLock<BreakerRegistry>` | `intelligence` | Per-service circuit breakers: pv2, synthex, me, povm, rm, vms |

### Dispatch Counters (5 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `dispatch_total` | `AtomicU64` | -- | Total semantic routing dispatches |
| `dispatch_read` | `AtomicU64` | -- | Dispatches to Read domain |
| `dispatch_write` | `AtomicU64` | -- | Dispatches to Write domain |
| `dispatch_execute` | `AtomicU64` | -- | Dispatches to Execute domain |
| `dispatch_communicate` | `AtomicU64` | -- | Dispatches to Communicate domain |

### Hebbian Tracking (4 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `co_activations_total` | `AtomicU64` | -- | Total co-activation events (BUG-059 fix) |
| `hebbian_ltp_total` | `AtomicU64` | -- | Accumulated LTP event count (BUG-GEN13) |
| `hebbian_ltd_total` | `AtomicU64` | -- | Accumulated LTD event count (BUG-GEN13) |
| `hebbian_last_tick` | `AtomicU64` | -- | Last tick when STDP ran (BUG-GEN13) |

### Bridge Instances (3 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `me_bridge` | `MeBridge` | `bridges` | ME :8080 fitness polling, frozen detection, observer data |
| `rm_bridge` | `RmBridge` | `bridges` | RM :8130 TSV persistence, cross-session records |
| `synthex_bridge` | `SynthexBridge` | `bridges` | SYNTHEX :8090 thermal polling, field state posting |

### Tool Tracking (2 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `total_tool_calls` | `AtomicU64` | -- | Global tool call counter for cascade heat |
| `tool_calls_at_last_thermal` | `AtomicU64` | -- | Snapshot at last SYNTHEX post (for rate delta) |

### Monitoring Stores (3 fields)

| Field | Type | Feature Gate | Purpose |
|-------|------|--------------|---------|
| `trace_store` | `TraceStore` | `monitoring` | In-process OTel-style span recording |
| `dashboard` | `FieldDashboard` | `monitoring` | Kuramoto field metrics for /dashboard endpoint |
| `token_accountant` | `TokenAccountant` | `monitoring` | Per-task token cost tracking for /tokens endpoint |

---

## 6. Feature Gate Matrix

### Feature Definitions (Cargo.toml)

| Feature | Dependencies | Crate Dependencies Enabled |
|---------|-------------|---------------------------|
| `api` | -- | `axum`, `tower-http` |
| `persistence` | -- | `rusqlite` |
| `bridges` | -- | (none -- uses `ureq` from main deps) |
| `intelligence` | -- | `tower` |
| `monitoring` | -- | `opentelemetry`, `opentelemetry-otlp` |
| `evolution` | -- | (none) |
| `default` | all 6 above | All optional crates |
| `full` | all 6 above | Same as default (explicit alias) |

### Feature-to-Layer Mapping

| Feature | Layers Gated | Modules Gated |
|---------|-------------|---------------|
| `api` | L3 Hooks | m10-m14, m3_hooks/mod.rs |
| `intelligence` | L4 Intelligence | m15-m21, m4_intelligence/mod.rs |
| `bridges` | L5 Bridges (m22-m25, http_helpers) | m22, m23, m24, m25, http_helpers |
| `persistence` | L5 Bridges (m26) | m26_blackboard |
| `monitoring` | L7 Monitoring | m32-m35, m7_monitoring/mod.rs |
| `evolution` | L8 Evolution | m36-m40, m8_evolution/mod.rs |
| (none) | L1, L2, L6 | Always compiled |

### Feature-to-OracState Field Mapping

| Feature | OracState Fields Gated |
|---------|----------------------|
| `persistence` | `blackboard` |
| `evolution` | `ralph` |
| `intelligence` | `breakers` |
| `bridges` | `me_bridge`, `rm_bridge`, `synthex_bridge` |
| `monitoring` | `trace_store`, `dashboard`, `token_accountant` |

### Feature Combinations in main.rs

Several functions in the daemon binary require compound feature gates:

| Function | Features Required |
|----------|-----------------|
| `post_field_to_synthex()` | `bridges` + `evolution` |
| `persist_stdp_to_povm()` | `bridges` + `intelligence` |
| `post_state_to_vms()` | `bridges` + `evolution` |
| `query_vms_for_ralph_context()` | `bridges` + `evolution` |
| `post_state_to_rm()` | `bridges` + `evolution` |
| `relay_emergence_to_rm()` | `bridges` + `evolution` |
| `build_tensor_from_state()` | `evolution` (with conditional `bridges` and `intelligence` blocks) |
| RALPH state persistence | `persistence` + `evolution` |
| STDP to POVM persistence | `bridges` + `intelligence` |
| Session persistence | `persistence` |
| Coupling weight persistence | `persistence` + `intelligence` |
| Hebbian summary to blackboard | `persistence` + `intelligence` |

---

## Appendix: Previous Module Name Errors (Corrected)

The following 14 module labels were incorrect in prior schematic versions, plus 2 modules were missing entirely. All corrected in this document.

| Module | Old (WRONG) Label | Correct Label |
|--------|-------------------|---------------|
| m04 | timestamp | constants |
| m05 | event | traits |
| m06 | util | validation |
| m07 | bus | ipc_client |
| m08 | ipc | bus_types |
| m09 | codec | wire_protocol |
| m11 | pre_tool | session_hooks |
| m12 | post_tool | tool_hooks |
| m13 | notification | prompt_hooks |
| m14 | middleware | permission_policy |
| m15 | hebbian | coupling_network |
| m16 | coupling | auto_k |
| m18 | breaker | hebbian_stdp |
| m19 | phase | buoy_network |
| m20 | chimera | semantic_router |
| m21 | decision | circuit_breaker |
| field_state | MISSING | L1 support module |
| http_helpers | MISSING | L5 support module |
