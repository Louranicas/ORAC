# ORAC Module Index

> 40 modules + 1 extra (field_state) across 8 layers, 3 bin targets
> Aligned with: ORAC_PLAN.md, ORAC_MINDMAP.md, src/ mod.rs declarations

## L1 Core (`src/m1_core/`)

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m01 | core_types | `m01_core_types.rs` | `PaneId`, `TaskId`, `OrderParameter`, `FleetMode`, `Timestamp`, `PaneSphere` | PV2 drop-in |
| m02 | error_handling | `m02_error_handling.rs` | `OracError` enum, `ErrorClassifier` trait | PV2 drop-in |
| m03 | config | `m03_config.rs` | `PvConfig` TOML + env overlay | PV2 drop-in |
| m04 | constants | `m04_constants.rs` | Thresholds, intervals, STDP parameters | PV2 drop-in |
| m05 | traits | `m05_traits.rs` | `Oscillator`, `Learnable`, `Bridgeable`, `Persistable` | PV2 drop-in |
| m06 | validation | `m06_validation.rs` | Input validators (persona, tool_name, summary, body) | PV2 drop-in |
| — | field_state | `field_state.rs` | Sidecar-native `AppState`, `SharedState`, `Harmonics` | NEW |

## L2 Wire (`src/m2_wire/`)

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m07 | ipc_client | `m07_ipc_client.rs` | Unix socket client → PV2 bus, backoff reconnect | PV2 drop-in (M29) |
| m08 | bus_types | `m08_bus_types.rs` | ClientFrame, ServerFrame, TaskTarget, TaskStatus | PV2 drop-in (M30) |
| m09 | wire_protocol | `m09_wire_protocol.rs` | V2 wire format, handshake, subscribe, V1 compat | NEW |

## L3 Hooks (`src/m3_hooks/`) — Phase 1

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m10 | hook_server | `m10_hook_server.rs` | Axum server :8133, 6 route endpoints | NEW |
| m11 | session_hooks | `m11_session_hooks.rs` | SessionStart + Stop handlers | NEW |
| m12 | tool_hooks | `m12_tool_hooks.rs` | PostToolUse + PreToolUse handlers | NEW |
| m13 | prompt_hooks | `m13_prompt_hooks.rs` | UserPromptSubmit handler | NEW |
| m14 | permission_policy | `m14_permission_policy.rs` | PermissionRequest auto-approve/deny | NEW |

## L4 Intelligence (`src/m4_intelligence/`) — Phase 2

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m15 | coupling_network | `m15_coupling_network.rs` | Kuramoto coupling matrix, phase dynamics | PV2 drop-in (M16) |
| m16 | auto_k | `m16_auto_k.rs` | Adaptive K, consent-gated adjustment | PV2 drop-in (M17) |
| m17 | topology | `m17_topology.rs` | Network topology analysis | PV2 drop-in (M18) |
| m18 | hebbian_stdp | `m18_hebbian_stdp.rs` | STDP learning (LTP/LTD), co-activation | PV2 drop-in (M19) |
| m19 | buoy_network | `m19_buoy_network.rs` | Buoy health tracking, spatial recall | PV2 drop-in (M20) |
| m20 | semantic_router | `m20_semantic_router.rs` | Content-aware dispatch, domain affinity | NEW |
| m21 | circuit_breaker | `m21_circuit_breaker.rs` | Per-pane health gating (Closed/Open/HalfOpen) | NEW |

## L5 Bridges (`src/m5_bridges/`) — Phase 3

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m22 | synthex_bridge | `m22_synthex_bridge.rs` | SYNTHEX :8090 — thermal + Hebbian writeback | PV2 adapt (M22) |
| m23 | me_bridge | `m23_me_bridge.rs` | ME :8080 — fitness signal, frozen detection | PV2 adapt (M24) |
| m24 | povm_bridge | `m24_povm_bridge.rs` | POVM :8125 — hydration + crystallisation | PV2 adapt (M25) |
| m25 | rm_bridge | `m25_rm_bridge.rs` | RM :8130 — TSV persistence (NOT JSON!) | PV2 adapt (M26) |
| m26 | blackboard | `m26_blackboard.rs` | SQLite shared fleet state | NEW |

## L6 Coordination (`src/m6_coordination/`) — Phase 3

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m27 | conductor | `m27_conductor.rs` | PI controller, breathing rhythm | PV2 adapt (M31) |
| m28 | cascade | `m28_cascade.rs` | Cascade handoff, sphere mitosis | PV2 drop-in (M33) |
| m29 | tick | `m29_tick.rs` | Tick orchestrator, Phase 2.5 Hebbian | PV2 adapt (M35) |
| m30 | wasm_bridge | `m30_wasm_bridge.rs` | FIFO/ring WASM plugin protocol | NEW |
| m31 | memory_manager | `m31_memory_manager.rs` | Memory aggregation + pruning | PV2 drop-in (M21) |

## L7 Monitoring (`src/m7_monitoring/`) — Phase 3

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m32 | otel_traces | `m32_otel_traces.rs` | OpenTelemetry trace export | NEW |
| m33 | metrics_export | `m33_metrics_export.rs` | Prometheus-compatible metrics | NEW |
| m34 | field_dashboard | `m34_field_dashboard.rs` | Kuramoto field metrics dashboard | NEW |
| m35 | token_accounting | `m35_token_accounting.rs` | Per-task token cost tracking | NEW |

## L8 Evolution (`src/m8_evolution/`) — Phase 4

| # | Module | File | Purpose | Source |
|---|--------|------|---------|--------|
| m36 | ralph_engine | `m36_ralph_engine.rs` | 5-phase RALPH loop | ME clone + fix |
| m37 | emergence_detector | `m37_emergence_detector.rs` | Ring buffer, TTL decay, cap 5000 | ME clone + fix |
| m38 | correlation_engine | `m38_correlation_engine.rs` | Pathway discovery, correlation mining | ME clone + fix |
| m39 | fitness_tensor | `m39_fitness_tensor.rs` | 12-dim weighted fitness evaluation | ME clone + fix |
| m40 | mutation_selector | `m40_mutation_selector.rs` | Diversity-enforced parameter selection | NEW (BUG-035 fix) |

## Bin Targets (`src/bin/`)

| Binary | File | Purpose |
|--------|------|---------|
| orac-sidecar | `main.rs` | Main daemon — hook server + IPC + tick loop |
| orac-client | `client.rs` | CLI client for ORAC API |
| orac-probe | `probe.rs` | Diagnostics tool for health + bridge probing |

## Summary

- **Total modules:** 41 (40 numbered + field_state)
- **Drop-in from PV2:** 14 modules (m01-m06, m07-m08, m15-m19, m28, m31)
- **Adapt from PV2:** 6 modules (m22-m25, m27, m29)
- **New for ORAC:** 21 modules (m09-m14, m20-m21, m26, m30, m32-m40, field_state)
