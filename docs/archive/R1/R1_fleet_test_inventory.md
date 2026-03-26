# ORAC Sidecar - R1 Fleet Test Inventory

> Generated: 2026-03-25 | Total: **1,748 tests** (1,696 unit + 52 integration)

---

## 1. Integration Tests

**Location:** `orac-sidecar/tests/` (13 files, 52 tests)

| File | Tests | Layer | Feature Gate | Status | Key Scenarios |
|------|------:|-------|-------------|--------|---------------|
| `api_endpoints.rs` | 26 | L3 Hooks / HTTP API | `api` | Implemented | health (3), field (5), blackboard (3), metrics (4), ghosts (2), consent (2), hook POST smoke (4), 404 (1) |
| `l8_evolution_integration.rs` | 15 | L8 Evolution / RALPH | `evolution` | Implemented | tick advancement, phase cycling (R-A-L-P-H), pause/resume, auto-pause at max_cycles, fitness tensor access, convergence stats |
| `l5_bridges_integration.rs` | 9 | L5 Bridges / Blackboard | `persistence` | Implemented | session lifecycle, multi-pane isolation, consent audit, ghost persistence, task history cross-pane, empty DB edge case, 200-ghost load test |
| `l1_core_integration.rs` | 1 | L1 Core (m01-m06) | none | Scaffold | placeholder only |
| `l2_wire_integration.rs` | 1 | L2 Wire / IPC | none | Scaffold | placeholder only |
| `l3_hooks_integration.rs` | 1 | L3 Hooks / HTTP | none | Scaffold | placeholder only (async) |
| `l4_intelligence_integration.rs` | 1 | L4 Intelligence | none | Scaffold | placeholder only |
| `l6_coordination_integration.rs` | 1 | L6 Coordination | none | Scaffold | placeholder only |
| `l7_monitoring_integration.rs` | 1 | L7 Monitoring | none | Scaffold | placeholder only |
| `cross_layer_workflows.rs` | 1 | Multi-layer | none | Scaffold | placeholder only |
| `stress_test.rs` | 1 | Load testing | none | Scaffold | placeholder only |
| `property_tests.rs` | 1 | Property-based | none | Scaffold | placeholder only |
| `common/mod.rs` | 1 | Test utilities | none | Scaffold | TestHarness struct (not implemented) |

### Integration Test Detail

#### `api_endpoints.rs` (26 tests, feature = `api`)

Async tests (`#[tokio::test]`, multi_thread, 2 workers) with ephemeral Axum server and mock PV2.

| Endpoint | Tests | Scenarios |
|----------|------:|-----------|
| `GET /health` | 3 | 200 status, required fields (status/service/port/sessions/uptime_ticks), session count = 2 |
| `GET /field` | 5 | 200 status, source + tick fields, JSON object, r + sphere_count from cache, k enrichment from live PV2, cache fallback when PV2 down |
| `GET /blackboard` | 3 | 200 status, sessions array + fleet_size + uptime_ticks, seeded pane IDs (alpha-left, beta-right) |
| `GET /metrics` | 4 | 200 status, Prometheus text format, orac_sessions_active = 2, all data lines have orac_ prefix |
| `GET /field/ghosts` | 2 | 200 status, empty array when no departures |
| `GET /consent/{id}` | 2 | 200 status, sphere_id in response |
| `POST /hooks/*` | 4 | SessionStart, PostToolUse, Stop, PermissionRequest (smoke tests) |
| `GET /nonexistent` | 1 | 404 for unknown route |

**Helpers:** `get_json()`, `get_text()`, `get_status()`, `post_status()`, `start_test_server()`, `start_mock_pv2()`, `start_test_server_with_pv2()`

#### `l8_evolution_integration.rs` (15 tests, feature = `evolution`)

Sync tests exercising RALPH engine lifecycle.

| Category | Tests | Scenarios |
|----------|------:|-----------|
| Tick mechanics | 2 | 5-tick generation advance, 10-tick monotonic growth |
| Phase cycling | 1 | Full R-A-L-P-H phase rotation |
| Pause/Resume | 3 | Pause blocks advance, resume re-enables, auto-pause at max_cycles |
| Statistics | 2 | Proposal tracking, state accessor consistency |
| Fitness | 1 | Tensor accessible after Recognize+Analyze |
| Convergence | 4 | Default pauses at 1000 cycles, stats consistency at pause, fitness > 0 after 50 cycles, no ticks after pause, re-pause on resume |

**Helper:** `mock_tensor()` creates 12D TensorValues.

#### `l5_bridges_integration.rs` (9 tests, feature = `persistence`)

Sync tests for blackboard persistence and session management.

| Category | Tests | Scenarios |
|----------|------:|-----------|
| Session lifecycle | 1 | SessionStart -> PostToolUse -> TaskComplete -> Stop |
| Multi-pane | 2 | Alpha/Beta isolation, cross-pane task history |
| Consent | 1 | Audit trail, 2 entries, newest-first |
| Ghosts | 2 | Persistence across 5 records, 200-ghost load test with pruning to 100 |
| Edge cases | 1 | Empty DB: all query types return safely (10 assertions) |

**Helpers:** `bb()` (in-memory Blackboard), `pid()` (PaneId factory).

---

## 2. Per-Module Unit Tests

**Location:** `orac-sidecar/src/` (42 files with tests, 1,696 total)

### L1 Core (201 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| core_types | `m1_core/m01_core_types.rs` | 52 | Type definitions, conversions, Display impls |
| error_handling | `m1_core/m02_error_handling.rs` | 29 | Error variants, From impls, error messages |
| config | `m1_core/m03_config.rs` | 25 | Config loading, defaults, validation |
| constants | `m1_core/m04_constants.rs` | 14 | Constant values, boundary checks |
| traits | `m1_core/m05_traits.rs` | 1 | Trait definitions (minimal) |
| validation | `m1_core/m06_validation.rs` | 51 | Input validation, boundary enforcement |
| field_state | `m1_core/field_state.rs` | 29 | Field state management, transitions |

### L2 Wire (130 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| ipc_client | `m2_wire/m07_ipc_client.rs` | 23 | IPC connection, message send/recv |
| bus_types | `m2_wire/m08_bus_types.rs` | 67 | Bus message types, serialization |
| wire_protocol | `m2_wire/m09_wire_protocol.rs` | 40 | Wire format encoding/decoding |

### L3 Hooks (206 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| hook_server | `m3_hooks/m10_hook_server.rs` | 76 | HTTP server, routing, middleware |
| session_hooks | `m3_hooks/m11_session_hooks.rs` | 24 | Session start/stop lifecycle |
| tool_hooks | `m3_hooks/m12_tool_hooks.rs` | 56 | Pre/PostToolUse processing |
| prompt_hooks | `m3_hooks/m13_prompt_hooks.rs` | 25 | Prompt submission hooks |
| permission_policy | `m3_hooks/m14_permission_policy.rs` | 25 | Permission evaluation logic |

### L4 Intelligence (237 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| coupling_network | `m4_intelligence/m15_coupling_network.rs` | 43 | Network topology, weight updates |
| auto_k | `m4_intelligence/m16_auto_k.rs` | 23 | Automatic Kuramoto coupling |
| topology | `m4_intelligence/m17_topology.rs` | 28 | Graph structure, connectivity |
| hebbian_stdp | `m4_intelligence/m18_hebbian_stdp.rs` | 30 | Spike-timing dependent plasticity |
| buoy_network | `m4_intelligence/m19_buoy_network.rs` | 23 | Buoy placement, field sampling |
| semantic_router | `m4_intelligence/m20_semantic_router.rs` | 45 | Intent routing, cosine similarity |
| circuit_breaker | `m4_intelligence/m21_circuit_breaker.rs` | 45 | Failure detection, trip/reset |

### L5 Bridges (339 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| http_helpers | `m5_bridges/http_helpers.rs` | 29 | HTTP client utilities |
| synthex_bridge | `m5_bridges/m22_synthex_bridge.rs` | 56 | SYNTHEX REST integration |
| me_bridge | `m5_bridges/m23_me_bridge.rs` | 52 | Maintenance Engine polling |
| povm_bridge | `m5_bridges/m24_povm_bridge.rs` | 60 | POVM memory read/write |
| rm_bridge | `m5_bridges/m25_rm_bridge.rs` | 52 | Reasoning Memory TSV interface |
| blackboard | `m5_bridges/m26_blackboard.rs` | 90 | Session state, ghosts, consent, persistence |

### L6 Coordination (133 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| conductor | `m6_coordination/m27_conductor.rs` | 25 | Orchestration, phase management |
| cascade | `m6_coordination/m28_cascade.rs` | 46 | Consent cascade logic |
| tick | `m6_coordination/m29_tick.rs` | 13 | Tick counter, timing |
| wasm_bridge | `m6_coordination/m30_wasm_bridge.rs` | 34 | WASM runtime interface |
| memory_manager | `m6_coordination/m31_memory_manager.rs` | 15 | Memory allocation, lifecycle |

### L7 Monitoring (236 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| otel_traces | `m7_monitoring/m32_otel_traces.rs` | 73 | OpenTelemetry span creation, export |
| metrics_export | `m7_monitoring/m33_metrics_export.rs` | 60 | Prometheus metric formatting |
| field_dashboard | `m7_monitoring/m34_field_dashboard.rs` | 48 | Dashboard data aggregation |
| token_accounting | `m7_monitoring/m35_token_accounting.rs` | 55 | Token usage tracking |

### L8 Evolution (214 tests)

| Module | File | Tests | Key Areas |
|--------|------|------:|-----------|
| ralph_engine | `m8_evolution/m36_ralph_engine.rs` | 29 | RALPH lifecycle, phase transitions |
| emergence_detector | `m8_evolution/m37_emergence_detector.rs` | 52 | Emergence event detection |
| correlation_engine | `m8_evolution/m38_correlation_engine.rs` | 32 | Cross-metric correlation |
| fitness_tensor | `m8_evolution/m39_fitness_tensor.rs` | 62 | 12D fitness evaluation |
| mutation_selector | `m8_evolution/m40_mutation_selector.rs` | 39 | Mutation strategy selection |

### Layer Summary

| Layer | Modules | Unit Tests | Integration Tests | Total |
|-------|--------:|----------:|-----------------:|------:|
| L1 Core | 7 | 201 | 1 (scaffold) | 202 |
| L2 Wire | 3 | 130 | 1 (scaffold) | 131 |
| L3 Hooks | 5 | 206 | 27 (26 API + 1 scaffold) | 233 |
| L4 Intelligence | 7 | 237 | 1 (scaffold) | 238 |
| L5 Bridges | 6 | 339 | 9 + 1 (scaffold) | 349 |
| L6 Coordination | 5 | 133 | 1 (scaffold) | 134 |
| L7 Monitoring | 4 | 236 | 1 (scaffold) | 237 |
| L8 Evolution | 5 | 214 | 16 (15 + 1 scaffold) | 230 |
| Cross-layer | - | - | 1 (scaffold) | 1 |
| Stress/Property | - | - | 2 (scaffold) | 2 |
| **Totals** | **42** | **1,696** | **52** | **1,748** |

---

## 3. Test Patterns Used

### Pattern Inventory

| Pattern | Where Used | Description |
|---------|-----------|-------------|
| **Scaffold placeholder** | 10 integration files | `#[test] fn scaffold() { /* module doc */ }` — reserves test file with layer documentation |
| **Feature-gated tests** | 3 integration files | `#[cfg(feature = "X")]` gates: `api` (26), `persistence` (9), `evolution` (15) |
| **Async multi-thread** | `api_endpoints.rs` | `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` for HTTP server tests |
| **Ephemeral server** | `api_endpoints.rs` | `start_test_server()` binds port 0, returns URL for isolated API testing |
| **Mock service** | `api_endpoints.rs` | `start_mock_pv2()` simulates PV2 with /health and /spheres endpoints |
| **In-memory DB** | `l5_bridges_integration.rs` | `bb()` creates Blackboard with `:memory:` SQLite for isolation |
| **Factory helpers** | Multiple | `pid()`, `mock_tensor()` — small factories for test data construction |
| **Graceful degradation** | `api_endpoints.rs` | Tests cache fallback when upstream PV2 is unreachable |
| **Load testing** | `l5_bridges_integration.rs` | 200-ghost insert + prune-to-100 + verify-newest-preserved |
| **Edge case coverage** | `l5_bridges_integration.rs` | Empty DB: all query types return safely (10 assertions) |
| **Phase exhaustion** | `l8_evolution_integration.rs` | Full R-A-L-P-H cycle verification, convergence at max_cycles |
| **Pause/resume** | `l8_evolution_integration.rs` | State machine: running -> paused -> resumed -> re-paused |
| **Prometheus format** | `api_endpoints.rs` | Content-Type text/plain, HELP/TYPE headers, orac_ prefix enforcement |
| **Pane isolation** | `l5_bridges_integration.rs` | Multi-pane writes verified as independent state |

### Test Infrastructure

| Component | File | Purpose |
|-----------|------|---------|
| `TestHarness` | `common/mod.rs` | Defined but not yet implemented — intended as shared test context |
| `start_test_server()` | `api_endpoints.rs` | Ephemeral Axum with seeded sessions (alpha-left, beta-right) |
| `start_mock_pv2()` | `api_endpoints.rs` | Mock PV2 returning k=1.5, sphere_count=42 |
| `start_test_server_with_pv2()` | `api_endpoints.rs` | ORAC server configured with custom PV2 URL |
| `bb()` / `pid()` | `l5_bridges_integration.rs` | In-memory blackboard + PaneId factory |
| `mock_tensor()` | `l8_evolution_integration.rs` | 12D TensorValues with plausible field data |

---

## 4. Coverage Gaps

### Modules Below 50-Test Threshold

**Quality gate requires 50 tests per module.** The following 25 modules fall short:

| Module | File | Tests | Deficit | Priority |
|--------|------|------:|--------:|----------|
| m05_traits | `m1_core/m05_traits.rs` | 1 | -49 | CRITICAL |
| m29_tick | `m6_coordination/m29_tick.rs` | 13 | -37 | CRITICAL |
| m04_constants | `m1_core/m04_constants.rs` | 14 | -36 | CRITICAL |
| m31_memory_manager | `m6_coordination/m31_memory_manager.rs` | 15 | -35 | CRITICAL |
| m07_ipc_client | `m2_wire/m07_ipc_client.rs` | 23 | -27 | HIGH |
| m16_auto_k | `m4_intelligence/m16_auto_k.rs` | 23 | -27 | HIGH |
| m19_buoy_network | `m4_intelligence/m19_buoy_network.rs` | 23 | -27 | HIGH |
| m11_session_hooks | `m3_hooks/m11_session_hooks.rs` | 24 | -26 | HIGH |
| m03_config | `m1_core/m03_config.rs` | 25 | -25 | HIGH |
| m13_prompt_hooks | `m3_hooks/m13_prompt_hooks.rs` | 25 | -25 | HIGH |
| m14_permission_policy | `m3_hooks/m14_permission_policy.rs` | 25 | -25 | HIGH |
| m27_conductor | `m6_coordination/m27_conductor.rs` | 25 | -25 | HIGH |
| m17_topology | `m4_intelligence/m17_topology.rs` | 28 | -22 | MEDIUM |
| field_state | `m1_core/field_state.rs` | 29 | -21 | MEDIUM |
| m02_error_handling | `m1_core/m02_error_handling.rs` | 29 | -21 | MEDIUM |
| http_helpers | `m5_bridges/http_helpers.rs` | 29 | -21 | MEDIUM |
| m36_ralph_engine | `m8_evolution/m36_ralph_engine.rs` | 29 | -21 | MEDIUM |
| m18_hebbian_stdp | `m4_intelligence/m18_hebbian_stdp.rs` | 30 | -20 | MEDIUM |
| m38_correlation_engine | `m8_evolution/m38_correlation_engine.rs` | 32 | -18 | MEDIUM |
| m30_wasm_bridge | `m6_coordination/m30_wasm_bridge.rs` | 34 | -16 | MEDIUM |
| m40_mutation_selector | `m8_evolution/m40_mutation_selector.rs` | 39 | -11 | LOW |
| m15_coupling_network | `m4_intelligence/m15_coupling_network.rs` | 43 | -7 | LOW |
| m20_semantic_router | `m4_intelligence/m20_semantic_router.rs` | 45 | -5 | LOW |
| m21_circuit_breaker | `m4_intelligence/m21_circuit_breaker.rs` | 45 | -5 | LOW |
| m28_cascade | `m6_coordination/m28_cascade.rs` | 46 | -4 | LOW |

**Total deficit: 596 tests needed to reach 50/module across all 25 modules.**

### Modules Meeting Threshold (17 modules)

| Module | Tests | Status |
|--------|------:|--------|
| m26_blackboard | 90 | PASS |
| m10_hook_server | 76 | PASS |
| m32_otel_traces | 73 | PASS |
| m08_bus_types | 67 | PASS |
| m39_fitness_tensor | 62 | PASS |
| m33_metrics_export | 60 | PASS |
| m24_povm_bridge | 60 | PASS |
| m12_tool_hooks | 56 | PASS |
| m22_synthex_bridge | 56 | PASS |
| m35_token_accounting | 55 | PASS |
| m01_core_types | 52 | PASS |
| m37_emergence_detector | 52 | PASS |
| m23_me_bridge | 52 | PASS |
| m25_rm_bridge | 52 | PASS |
| m06_validation | 51 | PASS |
| m34_field_dashboard | 48 | NEAR (2 short) |
| m34_field_dashboard needs 2 more tests to pass threshold.

### Integration Test Gaps

| Gap | Severity | Notes |
|-----|----------|-------|
| L1 Core integration | HIGH | 7 modules, 201 unit tests, but no integration coverage |
| L2 Wire integration | HIGH | IPC + bus + wire protocol untested end-to-end |
| L4 Intelligence integration | HIGH | 7 modules, 237 unit tests, Hebbian/STDP/routing untested cross-module |
| L6 Coordination integration | MEDIUM | Conductor + cascade + tick untested as workflow |
| L7 Monitoring integration | MEDIUM | OTel + metrics + dashboard untested as pipeline |
| Cross-layer workflows | HIGH | No multi-layer integration tests implemented |
| Stress tests | MEDIUM | Scaffold only, no load/concurrency testing |
| Property tests | LOW | Scaffold only, no property-based testing |
| `TestHarness` | MEDIUM | Defined in `common/mod.rs` but not implemented |

### Files With Zero Tests (Expected)

| File | Reason |
|------|--------|
| `src/bin/main.rs` | Entry point |
| `src/bin/client.rs` | CLI binary |
| `src/bin/probe.rs` | Diagnostic binary |
| `src/bin/ralph_bench.rs` | Benchmark binary |
| `src/lib.rs` | Module re-exports |
| `src/m{1-8}_*/mod.rs` (8 files) | Module aggregators |

---

## 5. Quick Stats

```
Total tests:           1,748
  Unit tests:          1,696 (42 modules)
  Integration tests:      52 (13 files, 50 real + 10 scaffolds)

Feature-gated:            50 (api: 26, persistence: 9, evolution: 15)
Async tests:              27 (26 API + 1 hook scaffold)

Modules at threshold:     17 of 42 (40.5%)
Modules below threshold:  25 of 42 (59.5%)
Total deficit:           596 tests

Layers with real integration tests: 3 of 8 (L3, L5, L8)
Layers scaffold-only:               5 of 8 (L1, L2, L4, L6, L7)
```
