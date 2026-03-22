# ORAC Sidecar — Master Index

> **Envoy-like proxy specialized for AI agent traffic**
> **Port:** 8133 | **8 layers, 40 modules, 3 binaries** | **19,454 LOC (2,405 hooks)**
> **GitLab:** `git@gitlab.com:lukeomahoney/orac-sidecar.git`
> **Status:** PHASE 1 COMPLETE — HTTP hook server live, 699 tests, binary deployed
> **Obsidian:** `[[Session 050 — ORAC Sidecar Architecture]]` | `[[Session 052 — Phase 1 Hooks Deployed]]`

---

## New Context Window — Bootstrap Sequence

**Execute in this order at the start of EVERY new context window:**

| # | Action | What It Loads |
|---|--------|---------------|
| 0 | **`/primehabitat`** | The Habitat: Zellij 6 tabs, 16 ULTRAPLATE services, IPC bus, 6 memory systems, tool chains |
| 1 | **`/deephabitat`** | Deep substrate: wire protocol, 166 DBs, 55+ binaries, devenv batches, vault navigation |
| 2 | **Read [`CLAUDE.local.md`](CLAUDE.local.md)** | Session state, critical path, next step, traps |
| 3 | **Read [`ai_docs/QUICKSTART.md`](ai_docs/QUICKSTART.md)** | Architecture, file map, build, deploy |
| 4 | **Read [`.claude/context.json`](.claude/context.json)** | Machine-readable project state |

After bootstrap: check phase status in CLAUDE.local.md and continue from where left off.

## Navigation

| Goal | Read This |
|------|-----------|
| **Full file inventory** | **This file** — `MASTER_INDEX.md` |
| **Understand the plan** | [`ORAC_PLAN.md`](ORAC_PLAN.md) — 4 phases, ~24.5K LOC, 33-feature backlog |
| **Explore the knowledge graph** | [`ORAC_MINDMAP.md`](ORAC_MINDMAP.md) — 248 Obsidian notes, 19 branches |
| **Know the rules** | [`CLAUDE.md`](CLAUDE.md) — gold standard, anti-patterns, quality gate |
| **Machine-readable state** | [`.claude/context.json`](.claude/context.json) — layers, bridges, hooks, bins |

---

## File Inventory (150+ files)

### Root — Planning & Context

| File | Lines | Purpose | Mindmap Branch |
|------|-------|---------|----------------|
| [`ORAC_PLAN.md`](ORAC_PLAN.md) | 429 | Architecture, 4 phases, module map, decisions | All 19 branches |
| [`ORAC_MINDMAP.md`](ORAC_MINDMAP.md) | 658 | 248 Obsidian notes, 19 branches, 3 vaults | IS the mindmap |
| [`CLAUDE.md`](CLAUDE.md) | ~180 | Gold standard rules, anti-patterns, build gate | §11 Scaffold, §17 Skills |
| [`CLAUDE.local.md`](CLAUDE.local.md) | ~200 | Session 050 state, bootstrap, critical path, traps | §11 Scaffold |
| [`plan.toml`](plan.toml) | 527 | Scaffold plan: layers, modules, features, consent | §11 Scaffold |
| [`Cargo.toml`](Cargo.toml) | 69 | Rust config: deps, features, lints, bin targets | — |
| [`bacon.toml`](bacon.toml) | — | Bacon task runner config | — |
| **`MASTER_INDEX.md`** | — | **THIS FILE** | — |

---

### Source Code — `src/` (56 files, 14,487 LOC)

#### Binary Targets

| Binary | File | LOC | Purpose |
|--------|------|-----|---------|
| `orac-sidecar` | [`src/bin/main.rs`](src/bin/main.rs) | 46 | Main daemon — HTTP hooks + IPC + tick loop |
| `orac-client` | [`src/bin/client.rs`](src/bin/client.rs) | 29 | CLI — status, field, spheres, hooks, bridges |
| `orac-probe` | [`src/bin/probe.rs`](src/bin/probe.rs) | 40 | Diagnostics — probes all 6 services (FUNCTIONAL) |

#### Library Entry Point

| File | LOC | Purpose |
|------|-----|---------|
| [`src/lib.rs`](src/lib.rs) | 48 | 8 layer declarations with feature gates |

#### L1 Core — Foundation (always compiled) — Mindmap §19 (Candidate Modules)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m01 | [`src/m1_core/m01_core_types.rs`](src/m1_core/m01_core_types.rs) | 1,358 | `PaneId`, `TaskId`, `OrderParameter`, `FleetMode`, `Timestamp` | PV2 drop-in |
| m02 | [`src/m1_core/m02_error_handling.rs`](src/m1_core/m02_error_handling.rs) | 595 | `OracError` enum, `ErrorClassifier` trait | PV2 drop-in |
| m03 | [`src/m1_core/m03_config.rs`](src/m1_core/m03_config.rs) | 644 | `PvConfig` TOML + env overlay | PV2 drop-in |
| m04 | [`src/m1_core/m04_constants.rs`](src/m1_core/m04_constants.rs) | 339 | Thresholds, intervals, STDP parameters | PV2 drop-in |
| m05 | [`src/m1_core/m05_traits.rs`](src/m1_core/m05_traits.rs) | 249 | `Oscillator`, `Learnable`, `Bridgeable` | PV2 drop-in |
| m06 | [`src/m1_core/m06_validation.rs`](src/m1_core/m06_validation.rs) | 540 | Input validators (persona, tool_name, body) | PV2 drop-in |
| — | [`src/m1_core/field_state.rs`](src/m1_core/field_state.rs) | 294 | Sidecar-native `AppState`, `SharedState` | NEW |
| — | [`src/m1_core/mod.rs`](src/m1_core/mod.rs) | 38 | Layer coordinator | — |

**Layer doc:** [`ai_docs/layers/L1_CORE.md`](ai_docs/layers/L1_CORE.md) | **Module doc:** [`ai_docs/modules/L1_CORE_MODULES.md`](ai_docs/modules/L1_CORE_MODULES.md)
**Obsidian:** `[[Pane-Vortex — Fleet Coordination Daemon]]` | `[[Session 050 — ULTRAPLATE Module Inventory]]`

#### L2 Wire — IPC Client (always compiled) — Mindmap §2 (IPC Client)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m07 | [`src/m2_wire/m07_ipc_client.rs`](src/m2_wire/m07_ipc_client.rs) | 410 | Unix socket client → PV2 bus | PV2 M29 drop-in |
| m08 | [`src/m2_wire/m08_bus_types.rs`](src/m2_wire/m08_bus_types.rs) | 978 | ClientFrame, ServerFrame, TaskStatus FSM | PV2 M30 drop-in |
| m09 | [`src/m2_wire/m09_wire_protocol.rs`](src/m2_wire/m09_wire_protocol.rs) | 12 | V2 wire format, handshake, V1 compat | NEW (stub) |

**Layer doc:** [`ai_docs/layers/L2_WIRE.md`](ai_docs/layers/L2_WIRE.md) | **Module doc:** [`ai_docs/modules/L2_WIRE_MODULES.md`](ai_docs/modules/L2_WIRE_MODULES.md)
**Spec:** [`ai_specs/WIRE_PROTOCOL_SPEC.md`](ai_specs/WIRE_PROTOCOL_SPEC.md) | **Schemas:** [`.claude/schemas/bus_*.json`](.claude/schemas/)
**Obsidian:** `[[Pane-Vortex IPC Bus — Session 019b]]` | `[[Session 050 — Sidecar Deep Dive]]`

#### L3 Hooks — HTTP Hook Server [KEYSTONE] (feature: `api`) — Mindmap §1 (HTTP Hook Server)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m10 | [`src/m3_hooks/m10_hook_server.rs`](src/m3_hooks/m10_hook_server.rs) | 12 | Axum router :8133, 6 endpoints | NEW (stub) |
| m11 | [`src/m3_hooks/m11_session_hooks.rs`](src/m3_hooks/m11_session_hooks.rs) | 12 | SessionStart + Stop handlers | NEW (stub) |
| m12 | [`src/m3_hooks/m12_tool_hooks.rs`](src/m3_hooks/m12_tool_hooks.rs) | 12 | PostToolUse + PreToolUse handlers | NEW (stub) |
| m13 | [`src/m3_hooks/m13_prompt_hooks.rs`](src/m3_hooks/m13_prompt_hooks.rs) | 12 | UserPromptSubmit handler | NEW (stub) |
| m14 | [`src/m3_hooks/m14_permission_policy.rs`](src/m3_hooks/m14_permission_policy.rs) | 12 | PermissionRequest auto-approve/deny | NEW (stub) |

**Layer doc:** [`ai_docs/layers/L3_HOOKS.md`](ai_docs/layers/L3_HOOKS.md) | **Module doc:** [`ai_docs/modules/L3_HOOKS_MODULES.md`](ai_docs/modules/L3_HOOKS_MODULES.md)
**Spec:** [`ai_specs/HOOKS_SPEC.md`](ai_specs/HOOKS_SPEC.md) | **Schemas:** [`.claude/schemas/hook_*.json`](.claude/schemas/)
**Obsidian:** `[[Session 050 — Hook Pipeline vs Sidecar Gap]]` | `[[Consent Flow Analysis]]` | `[[Session 045 Arena — 02-api-wiring-map]]`

#### L4 Intelligence — Hebbian, Coupling, Routing (feature: `intelligence`) — Mindmap §3 (Intelligence Layer)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m15 | [`src/m4_intelligence/m15_coupling_network.rs`](src/m4_intelligence/m15_coupling_network.rs) | 912 | Kuramoto coupling matrix | PV2 M16 drop-in |
| m16 | [`src/m4_intelligence/m16_auto_k.rs`](src/m4_intelligence/m16_auto_k.rs) | 365 | Adaptive K, consent-gated | PV2 M17 drop-in |
| m17 | [`src/m4_intelligence/m17_topology.rs`](src/m4_intelligence/m17_topology.rs) | 456 | Network topology analysis | PV2 M18 drop-in |
| m18 | [`src/m4_intelligence/m18_hebbian_stdp.rs`](src/m4_intelligence/m18_hebbian_stdp.rs) | 554 | LTP/LTD, co-activation learning | PV2 M19 drop-in |
| m19 | [`src/m4_intelligence/m19_buoy_network.rs`](src/m4_intelligence/m19_buoy_network.rs) | 452 | Buoy health tracking, spatial recall | PV2 M20 drop-in |
| m20 | [`src/m4_intelligence/m20_semantic_router.rs`](src/m4_intelligence/m20_semantic_router.rs) | 12 | Content-aware dispatch | NEW (stub) |
| m21 | [`src/m4_intelligence/m21_circuit_breaker.rs`](src/m4_intelligence/m21_circuit_breaker.rs) | 12 | Per-pane health gating | NEW (stub) |

**Layer doc:** [`ai_docs/layers/L4_INTELLIGENCE.md`](ai_docs/layers/L4_INTELLIGENCE.md) | **Module doc:** [`ai_docs/modules/L4_INTELLIGENCE_MODULES.md`](ai_docs/modules/L4_INTELLIGENCE_MODULES.md)
**Spec:** [`ai_specs/patterns/STDP.md`](ai_specs/patterns/STDP.md) | [`ai_specs/patterns/KURAMOTO.md`](ai_specs/patterns/KURAMOTO.md)
**Obsidian:** `[[Session 045 Arena — 10-hebbian-operational-topology]]` | `[[Vortex Sphere Brain-Body Architecture]]`

#### L5 Bridges — Service Connectors (feature: `bridges`) — Mindmap §6 (Bridge Subset)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m22 | [`src/m5_bridges/m22_synthex_bridge.rs`](src/m5_bridges/m22_synthex_bridge.rs) | 930 | SYNTHEX :8090 thermal + Hebbian writeback | PV2 M22 adapt |
| m23 | [`src/m5_bridges/m23_me_bridge.rs`](src/m5_bridges/m23_me_bridge.rs) | 839 | ME :8080 fitness signal | PV2 M24 adapt |
| m24 | [`src/m5_bridges/m24_povm_bridge.rs`](src/m5_bridges/m24_povm_bridge.rs) | 952 | POVM :8125 hydration + crystallisation | PV2 M25 adapt |
| m25 | [`src/m5_bridges/m25_rm_bridge.rs`](src/m5_bridges/m25_rm_bridge.rs) | 983 | RM :8130 **TSV only** | PV2 M26 adapt |
| m26 | [`src/m5_bridges/m26_blackboard.rs`](src/m5_bridges/m26_blackboard.rs) | 12 | SQLite shared fleet state | NEW (stub) |

**Layer doc:** [`ai_docs/layers/L5_BRIDGES.md`](ai_docs/layers/L5_BRIDGES.md) | **Module doc:** [`ai_docs/modules/L5_BRIDGES_MODULES.md`](ai_docs/modules/L5_BRIDGES_MODULES.md)
**Spec:** [`ai_specs/BRIDGE_SPEC.md`](ai_specs/BRIDGE_SPEC.md)
**Obsidian:** `[[Synthex (The brain of the developer environment)]]` | `[[The Maintenance Engine V2]]` | `[[POVM Engine]]`

#### L6 Coordination — Orchestration (always compiled) — Mindmap §7, §9 (WASM Bridge, Cascade)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m27 | [`src/m6_coordination/m27_conductor.rs`](src/m6_coordination/m27_conductor.rs) | 470 | PI controller, breathing rhythm | PV2 M31 adapt |
| m28 | [`src/m6_coordination/m28_cascade.rs`](src/m6_coordination/m28_cascade.rs) | 711 | Cascade handoff, sphere mitosis | PV2 M33 drop-in |
| m29 | [`src/m6_coordination/m29_tick.rs`](src/m6_coordination/m29_tick.rs) | 287 | Tick orchestrator, Phase 2.5 Hebbian | PV2 M35 adapt |
| m30 | [`src/m6_coordination/m30_wasm_bridge.rs`](src/m6_coordination/m30_wasm_bridge.rs) | 12 | FIFO/ring WASM plugin protocol | NEW (stub) |
| m31 | [`src/m6_coordination/m31_memory_manager.rs`](src/m6_coordination/m31_memory_manager.rs) | 381 | Memory pruning, aggregation | PV2 M21 drop-in |

**Layer doc:** [`ai_docs/layers/L6_COORDINATION.md`](ai_docs/layers/L6_COORDINATION.md) | **Module doc:** [`ai_docs/modules/L6_COORDINATION_MODULES.md`](ai_docs/modules/L6_COORDINATION_MODULES.md)
**Obsidian:** `[[Pane-Vortex — Fleet Coordination Daemon]]` | `[[Swarm Orchestrator — Complete Reference]]`

#### L7 Monitoring — Observability (feature: `monitoring`) — Mindmap §5 (Monitoring / Observer)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m32 | [`src/m7_monitoring/m32_otel_traces.rs`](src/m7_monitoring/m32_otel_traces.rs) | 12 | OpenTelemetry trace export | NEW (stub) |
| m33 | [`src/m7_monitoring/m33_metrics_export.rs`](src/m7_monitoring/m33_metrics_export.rs) | 12 | Prometheus-compatible metrics | NEW (stub) |
| m34 | [`src/m7_monitoring/m34_field_dashboard.rs`](src/m7_monitoring/m34_field_dashboard.rs) | 12 | Kuramoto field metrics | NEW (stub) |
| m35 | [`src/m7_monitoring/m35_token_accounting.rs`](src/m7_monitoring/m35_token_accounting.rs) | 12 | Per-task token cost tracking | NEW (stub) |

**Layer doc:** [`ai_docs/layers/L7_MONITORING.md`](ai_docs/layers/L7_MONITORING.md) | **Module doc:** [`ai_docs/modules/L7_MONITORING_MODULES.md`](ai_docs/modules/L7_MONITORING_MODULES.md)
**Obsidian:** `[[Session 045 Arena — 12-live-field-analysis]]` | `[[ULTRAPLATE Metabolic Activation Plan 2026-03-07]]`

#### L8 Evolution — RALPH (feature: `evolution`) — Mindmap §4 (RALPH Evolution Chamber)

| Module | File | LOC | Purpose | Hot-Swap |
|--------|------|-----|---------|----------|
| m36 | [`src/m8_evolution/m36_ralph_engine.rs`](src/m8_evolution/m36_ralph_engine.rs) | 12 | 5-phase RALPH loop | ME clone (stub) |
| m37 | [`src/m8_evolution/m37_emergence_detector.rs`](src/m8_evolution/m37_emergence_detector.rs) | 12 | Ring buffer, TTL decay, cap 5000 | ME clone (stub) |
| m38 | [`src/m8_evolution/m38_correlation_engine.rs`](src/m8_evolution/m38_correlation_engine.rs) | 12 | Pathway discovery | ME clone (stub) |
| m39 | [`src/m8_evolution/m39_fitness_tensor.rs`](src/m8_evolution/m39_fitness_tensor.rs) | 13 | 12-dim weighted fitness | ME clone (stub) |
| m40 | [`src/m8_evolution/m40_mutation_selector.rs`](src/m8_evolution/m40_mutation_selector.rs) | 13 | Multi-parameter (BUG-035 fix) | NEW |

**Layer doc:** [`ai_docs/layers/L8_EVOLUTION.md`](ai_docs/layers/L8_EVOLUTION.md) | **Module doc:** [`ai_docs/modules/L8_EVOLUTION_MODULES.md`](ai_docs/modules/L8_EVOLUTION_MODULES.md)
**Spec:** [`ai_specs/EVOLUTION_SPEC.md`](ai_specs/EVOLUTION_SPEC.md)
**Obsidian:** `[[Session 050 — ME Evolution Chamber Spec]]` | `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` | `[[ULTRAPLATE — Bugs and Known Issues]]`

---

### Candidate Modules — `candidate-modules/` (24 files, 15,936 LOC) — Mindmap §19

Pre-refactored from PV2, staged for integration. Zero violations. Gold standard compliant.

| Directory | Target | Files | LOC | Action |
|-----------|--------|-------|-----|--------|
| [`drop-in/L1-foundation/`](candidate-modules/drop-in/L1-foundation/) | `src/m1_core/` | 7 | 3,373 | Copy as-is |
| [`drop-in/L2-wire/`](candidate-modules/drop-in/L2-wire/) | `src/m2_wire/` | 2 | 2,938 | Copy as-is |
| [`drop-in/L4-coupling/`](candidate-modules/drop-in/L4-coupling/) | `src/m4_intelligence/` | 4 | 1,724 | Copy as-is |
| [`drop-in/L4-learning/`](candidate-modules/drop-in/L4-learning/) | `src/m4_intelligence/` | 4 | 1,424 | Copy as-is |
| [`drop-in/L6-cascade/`](candidate-modules/drop-in/L6-cascade/) | `src/m6_coordination/` | 1 | 711 | Copy as-is |
| [`adapt/L5-synthex/`](candidate-modules/adapt/L5-synthex/) | `src/m5_bridges/` | 1 | 908 | `## ADAPT` changes |
| [`adapt/L5-me/`](candidate-modules/adapt/L5-me/) | `src/m5_bridges/` | 1 | 814 | `## ADAPT` changes |
| [`adapt/L5-povm/`](candidate-modules/adapt/L5-povm/) | `src/m5_bridges/` | 1 | 924 | `## ADAPT` changes |
| [`adapt/L5-rm/`](candidate-modules/adapt/L5-rm/) | `src/m5_bridges/` | 1 | 956 | `## ADAPT` changes |
| [`adapt/L6-conductor/`](candidate-modules/adapt/L6-conductor/) | `src/m6_coordination/` | 1 | 820 | `## ADAPT` changes |
| [`adapt/L6-tick/`](candidate-modules/adapt/L6-tick/) | `src/m6_coordination/` | 1 | 880 | `## ADAPT` changes |

**Integration protocol:** [`ORAC_PLAN.md §Scaffold Integration Protocol`](ORAC_PLAN.md) (7 steps)

---

### Documentation — `ai_docs/` (25 files) — Mindmap §11, §13, §17

| File | Purpose | Mindmap Branch |
|------|---------|----------------|
| [`ai_docs/QUICKSTART.md`](ai_docs/QUICKSTART.md) | Build, deploy, architecture, file map, reading order | All |
| [`ai_docs/INDEX.md`](ai_docs/INDEX.md) | Documentation navigation hub | §11 |
| [`ai_docs/GOLD_STANDARD_PATTERNS.md`](ai_docs/GOLD_STANDARD_PATTERNS.md) | 10 mandatory Rust patterns | §11 |
| [`ai_docs/ANTI_PATTERNS.md`](ai_docs/ANTI_PATTERNS.md) | 17 banned patterns with severity | §11 |

#### Layer Docs (`ai_docs/layers/` — 8 files)

| File | Layer | Modules | Mindmap Branch |
|------|-------|---------|----------------|
| [`L1_CORE.md`](ai_docs/layers/L1_CORE.md) | Foundation | m01-m06 + field_state | §19 |
| [`L2_WIRE.md`](ai_docs/layers/L2_WIRE.md) | IPC Client | m07-m09 | §2 |
| [`L3_HOOKS.md`](ai_docs/layers/L3_HOOKS.md) | HTTP Hooks | m10-m14 | §1 |
| [`L4_INTELLIGENCE.md`](ai_docs/layers/L4_INTELLIGENCE.md) | Hebbian + Routing | m15-m21 | §3 |
| [`L5_BRIDGES.md`](ai_docs/layers/L5_BRIDGES.md) | Service Bridges | m22-m26 | §6 |
| [`L6_COORDINATION.md`](ai_docs/layers/L6_COORDINATION.md) | Orchestration | m27-m31 | §7, §9 |
| [`L7_MONITORING.md`](ai_docs/layers/L7_MONITORING.md) | Observability | m32-m35 | §5 |
| [`L8_EVOLUTION.md`](ai_docs/layers/L8_EVOLUTION.md) | RALPH | m36-m40 | §4 |

#### Module Docs (`ai_docs/modules/` — 9 files, 5,813 lines)

| File | Modules | Lines |
|------|---------|-------|
| [`INDEX.md`](ai_docs/modules/INDEX.md) | All 41 | 100 |
| [`L1_CORE_MODULES.md`](ai_docs/modules/L1_CORE_MODULES.md) | m01-m06 + field_state | 788 |
| [`L2_WIRE_MODULES.md`](ai_docs/modules/L2_WIRE_MODULES.md) | m07-m09 | 518 |
| [`L3_HOOKS_MODULES.md`](ai_docs/modules/L3_HOOKS_MODULES.md) | m10-m14 | 733 |
| [`L4_INTELLIGENCE_MODULES.md`](ai_docs/modules/L4_INTELLIGENCE_MODULES.md) | m15-m21 | 970 |
| [`L5_BRIDGES_MODULES.md`](ai_docs/modules/L5_BRIDGES_MODULES.md) | m22-m26 | 660 |
| [`L6_COORDINATION_MODULES.md`](ai_docs/modules/L6_COORDINATION_MODULES.md) | m27-m31 | 683 |
| [`L7_MONITORING_MODULES.md`](ai_docs/modules/L7_MONITORING_MODULES.md) | m32-m35 | 533 |
| [`L8_EVOLUTION_MODULES.md`](ai_docs/modules/L8_EVOLUTION_MODULES.md) | m36-m40 | 828 |

#### Schematics (`ai_docs/schematics/` — 4 Mermaid diagrams)

| File | Shows | Mindmap Branch |
|------|-------|----------------|
| [`layer_architecture.mmd`](ai_docs/schematics/layer_architecture.mmd) | 8 layers with dependency arrows | §13 |
| [`hook_flow.mmd`](ai_docs/schematics/hook_flow.mmd) | Claude Code → ORAC → PV2 | §1 |
| [`bridge_topology.mmd`](ai_docs/schematics/bridge_topology.mmd) | ORAC → 5 services | §6 |
| [`field_dashboard.mmd`](ai_docs/schematics/field_dashboard.mmd) | PV2 field → metrics → dashboard | §5, §12 |

---

### Specifications — `ai_specs/` (11 files) — Mindmap §11

| File | Description | Mindmap Branch |
|------|-------------|----------------|
| [`INDEX.md`](ai_specs/INDEX.md) | Specification navigation hub | §11 |
| [`API_SPEC.md`](ai_specs/API_SPEC.md) | REST endpoints, request/response schemas | §1, §8 |
| [`HOOKS_SPEC.md`](ai_specs/HOOKS_SPEC.md) | 6 hook events, payload structures | §1 |
| [`BRIDGE_SPEC.md`](ai_specs/BRIDGE_SPEC.md) | SYNTHEX, ME, POVM, RM integration | §6 |
| [`WIRE_PROTOCOL_SPEC.md`](ai_specs/WIRE_PROTOCOL_SPEC.md) | V2 NDJSON, frames, handshake | §2 |
| [`EVOLUTION_SPEC.md`](ai_specs/EVOLUTION_SPEC.md) | RALPH 5-phase, fitness tensor, mutation | §4 |
| [`patterns/BUILDER.md`](ai_specs/patterns/BUILDER.md) | Typestate builder pattern | §11 |
| [`patterns/CIRCUIT_BREAKER.md`](ai_specs/patterns/CIRCUIT_BREAKER.md) | Closed/Open/HalfOpen FSM | §3 |
| [`patterns/KURAMOTO.md`](ai_specs/patterns/KURAMOTO.md) | Phase oscillators, order parameter | §12 |
| [`patterns/STDP.md`](ai_specs/patterns/STDP.md) | Hebbian spike-timing plasticity | §3 |

---

### Development Context — `.claude/` (18 files)

| File | Purpose | Mindmap Branch |
|------|---------|----------------|
| [`context.json`](.claude/context.json) | Machine-readable: layers, bridges, hooks, bins | All |
| [`status.json`](.claude/status.json) | Build phase tracking, candidate counts | §11 |
| [`patterns.json`](.claude/patterns.json) | 22 patterns (P01-P22) | §11 |
| [`anti_patterns.json`](.claude/anti_patterns.json) | 20 anti-patterns (AP01-AP20) | §11 |
| [`ALIGNMENT_VERIFICATION.md`](.claude/ALIGNMENT_VERIFICATION.md) | Mindmap × plan × src audit | All |
| [`schemas/hook_request.json`](.claude/schemas/hook_request.json) | 6 hook events | §1 |
| [`schemas/hook_response.json`](.claude/schemas/hook_response.json) | approve/deny/skip | §1 |
| [`schemas/permission_policy.schema.json`](.claude/schemas/permission_policy.schema.json) | Fleet + per-sphere rules | §1, §10 |
| [`schemas/bus_event.schema.json`](.claude/schemas/bus_event.schema.json) | 24 IPC event types | §2 |
| [`schemas/bus_frame.schema.json`](.claude/schemas/bus_frame.schema.json) | 5 wire frame types | §2 |
| [`queries/blackboard.sql`](.claude/queries/blackboard.sql) | Shared fleet state queries | §8 |
| [`queries/hook_events.sql`](.claude/queries/hook_events.sql) | Hook tracking + latency | §1 |
| [`queries/fleet_state.sql`](.claude/queries/fleet_state.sql) | Field snapshots, Hebbian, routing | §5, §12 |
| [`hooks/quality-gate.md`](.claude/hooks/quality-gate.md) | 4-step gate enforcement | §11 |
| [`hooks/pre-commit.md`](.claude/hooks/pre-commit.md) | Anti-pattern scan | §11 |
| [`skills/orac-boot/SKILL.md`](.claude/skills/orac-boot/SKILL.md) | Bootstrap ORAC knowledge | §17 |
| [`skills/hook-debug/SKILL.md`](.claude/skills/hook-debug/SKILL.md) | Test all 6 hook endpoints | §1 |
| [`skills/bridge-probe/SKILL.md`](.claude/skills/bridge-probe/SKILL.md) | Bridge connectivity diagnostics | §6 |

---

### Configuration — `config/` (5 files)

| File | Purpose |
|------|---------|
| [`config/default.toml`](config/default.toml) | Default settings for all environments |
| [`config/dev.toml`](config/dev.toml) | Development overrides (verbose logging) |
| [`config/prod.toml`](config/prod.toml) | Production tuning |
| [`config/hooks.toml`](config/hooks.toml) | Hook endpoint timeouts, policy config |
| [`config/bridges.toml`](config/bridges.toml) | Bridge URLs, polling intervals, retry |

---

### Tests & Benchmarks

| File | Purpose |
|------|---------|
| [`tests/common/mod.rs`](tests/common/mod.rs) | Shared test utilities |
| [`tests/l1_core_integration.rs`](tests/l1_core_integration.rs) | L1 integration tests |
| [`tests/l2_wire_integration.rs`](tests/l2_wire_integration.rs) | L2 integration tests |
| [`tests/l3_hooks_integration.rs`](tests/l3_hooks_integration.rs) | L3 integration tests |
| [`tests/l4_intelligence_integration.rs`](tests/l4_intelligence_integration.rs) | L4 integration tests |
| [`tests/l5_bridges_integration.rs`](tests/l5_bridges_integration.rs) | L5 integration tests |
| [`tests/l6_coordination_integration.rs`](tests/l6_coordination_integration.rs) | L6 integration tests |
| [`tests/l7_monitoring_integration.rs`](tests/l7_monitoring_integration.rs) | L7 integration tests |
| [`tests/l8_evolution_integration.rs`](tests/l8_evolution_integration.rs) | L8 integration tests |
| [`tests/cross_layer_workflows.rs`](tests/cross_layer_workflows.rs) | Cross-layer workflow tests |
| [`tests/property_tests.rs`](tests/property_tests.rs) | Property-based tests |
| [`tests/stress_test.rs`](tests/stress_test.rs) | Stress tests |
| [`benches/hook_latency.rs`](benches/hook_latency.rs) | Hook response time benchmark |
| [`benches/field_computation.rs`](benches/field_computation.rs) | Field computation benchmark |
| [`benches/hebbian_update.rs`](benches/hebbian_update.rs) | Hebbian update benchmark |

---

### Infrastructure

| File | Purpose |
|------|---------|
| [`migrations/001_blackboard.sql`](migrations/001_blackboard.sql) | SQLite schema: field_state, hook_events, fleet_state |
| [`scripts/test-hook-server.py`](scripts/test-hook-server.py) | Minimal HTTP hook test server (Phase 1 de-risk) |

---

## Mindmap Branch Coverage

Every file in this index maps to at least one of the 19 mindmap branches in [`ORAC_MINDMAP.md`](ORAC_MINDMAP.md):

| # | Branch | Files Covering It |
|---|--------|-------------------|
| 1 | HTTP Hook Server | L3 layer/module docs, HOOKS_SPEC, hook schemas, hook-debug skill, hook_events.sql |
| 2 | IPC Client | L2 layer/module docs, WIRE_PROTOCOL_SPEC, bus schemas |
| 3 | Intelligence Layer | L4 layer/module docs, STDP + CIRCUIT_BREAKER patterns |
| 4 | RALPH Evolution | L8 layer/module docs, EVOLUTION_SPEC |
| 5 | Monitoring | L7 layer/module docs, fleet_state.sql, field_dashboard.mmd |
| 6 | Bridge Subset | L5 layer/module docs, BRIDGE_SPEC, bridge-probe skill |
| 7 | WASM Bridge | L6 layer/module docs (m30) |
| 8 | Fleet Dispatch | API_SPEC, blackboard.sql, fleet_state.sql |
| 9 | Cascade Handoffs | L6 layer/module docs (m28), bus_frame.schema.json |
| 10 | Consent / Governance | permission_policy.schema.json, patterns P21, anti_patterns AP20 |
| 11 | Scaffold System | QUICKSTART, INDEX, GOLD_STANDARD_PATTERNS, ANTI_PATTERNS |
| 12 | Kuramoto Coupling | KURAMOTO pattern, field_dashboard.mmd, fleet_state.sql |
| 13 | Architecture Schematics | schematics/*.mmd |
| 14 | Database & Persistence | migrations/, blackboard.sql, config/bridges.toml |
| 15 | Memory Systems | L5 bridges (POVM, RM), queries/ |
| 16 | ULTRAPLATE Ecosystem | context.json (devenv_batch, service_id) |
| 17 | Habitat Skills | skills/ (orac-boot, hook-debug, bridge-probe) |
| 19 | Candidate Modules | candidate-modules/, ORAC_PLAN.md §Integration Protocol |

---

## External Cross-References

### Related Projects

| Project | Location | Relationship |
|---------|----------|-------------|
| **PV2** (source) | `~/claude-code-workspace/pane-vortex-v2/` | IPC bus provider, hot-swap module source (31,859 LOC) |
| **PV v1** | `~/claude-code-workspace/pane-vortex/` | Legacy daemon, running binary |
| **ME v2** (gold standard) | `~/claude-code-workspace/the_maintenance_engine_v2/` | RALPH evolution source (56K LOC) |
| **V1 Sidecar** | `~/claude-code-workspace/swarm-sidecar/` | Predecessor replaced by ORAC |
| **Swarm Orchestrator** | `~/claude-code-workspace/swarm-orchestrator/` | WASM plugin (FIFO/ring bridge) |

### Obsidian Vault — Bidirectional Links

| Obsidian Note | Links To |
|---------------|----------|
| `[[Session 050 — ORAC Sidecar Architecture]]` | ORAC_PLAN.md, all layer docs |
| `[[Session 051 — ORAC Sidecar .claude Scaffolding]]` | .claude/, module docs, this index |
| `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` | L8 evolution, BUG-035 |
| `[[Session 050 — Hook Pipeline vs Sidecar Gap]]` | L3 hooks, HOOKS_SPEC |
| `[[Session 050 — Sidecar Deep Dive]]` | L2 wire, WIRE_PROTOCOL_SPEC |
| `[[Session 050 — ME Evolution Chamber Spec]]` | L8 evolution, EVOLUTION_SPEC |
| `[[Session 050 — Sidecar Features Research]]` | L4 intelligence, L7 monitoring |
| `[[Session 050 — ULTRAPLATE Module Inventory]]` | L1 core, candidate-modules |
| `[[Pane-Vortex — Fleet Coordination Daemon]]` | L1, L2, L6 (source of hot-swap) |
| `[[Pane-Vortex IPC Bus — Session 019b]]` | L2 wire protocol |
| `[[Synthex (The brain of the developer environment)]]` | L5 m22 SYNTHEX bridge |
| `[[The Maintenance Engine V2]]` | L5 m23 ME bridge, L8 evolution |
| `[[POVM Engine]]` | L5 m24 POVM bridge |
| `[[ULTRAPLATE — Bugs and Known Issues]]` | BUG-032 through BUG-037 |
| `[[Consent Flow Analysis]]` | L3 m14 permission policy, §10 consent |
| `[[Self-Governing Agent Coordination — Design Notes 2026-03-08]]` | §10 consent |
| `[[The Habitat — Naming and Philosophy]]` | Philosophy: "the field modulates" |
| `[[Vortex Sphere Brain-Body Architecture]]` | L4 coupling, Kuramoto dynamics |
| `[[Session 045 Arena — 02-api-wiring-map]]` | L3 hooks (bash→HTTP migration) |
| `[[Session 045 Arena — 10-hebbian-operational-topology]]` | L4 Hebbian STDP |
| `[[Session 045 Arena — 12-live-field-analysis]]` | L7 monitoring, field dashboard |
| `[[Fleet System — Memory Index]]` | L5 blackboard, fleet state |
| `[[Swarm Orchestrator — Complete Reference]]` | L6 WASM bridge |
| `[[Habitat Skills Roster]]` | .claude/skills/ |

### Memory Systems

| System | Access | Format |
|--------|--------|--------|
| Auto-Memory | `~/.claude/projects/-home-louranicas/memory/MEMORY.md` | Markdown (auto-loaded) |
| Reasoning Memory | `localhost:8130` | **TSV only** |
| POVM | `localhost:8125` | JSON (3 Session 051 memories) |
| MCP Knowledge Graph | `mcp__memory__*` tools | Graph nodes |
| Obsidian (main) | `~/projects/claude_code/` | 215+ notes |
| Obsidian (shared) | `~/projects/shared-context/` | Multi-instance scratchpad |

---

## Statistics

| Metric | Value |
|--------|-------|
| Source files (src/) | 56 |
| Rust LOC | 14,487 |
| Layers | 8 |
| Numbered modules | 40 |
| Extra modules (field_state) | 1 |
| Binary targets | 3 |
| Candidate modules (staged) | 24 (15,936 LOC) |
| Documentation files | 25 (ai_docs/) |
| Specification files | 11 (ai_specs/) |
| Development context files | 18 (.claude/) |
| Configuration files | 5 (config/) |
| Integration tests | 12 |
| Benchmarks | 3 |
| Feature flags | 7 |
| Obsidian notes mapped | 248 (via mindmap) |
| Git commits | 4+ |

---

## Inconsistencies Resolved (Session 051)

| # | Issue | Resolution |
|---|-------|-----------|
| 1 | CLAUDE.md said `m2_hooks/` instead of `m2_wire/` | Fixed — now `m1_core/`, `m2_wire/`, `m3_hooks/` |
| 2 | ai_docs/layers/ had wrong module names vs src/ | Fixed — all 8 layer docs now match src/ exactly |
| 3 | ai_docs/modules/ was empty | Fixed — 9 files, 5,813 lines, gold standard format |
| 4 | .claude/ folder was bare (gitkeeps only) | Fixed — 18 files: context, patterns, schemas, queries, skills |
| 5 | hook_request.json had 4 events | Fixed — now covers all 6 ORAC hook events |

---

*ORAC Sidecar — from dumb pipe to intelligent fleet coordination proxy.*
*150+ files. 14,487 LOC scaffolded. 15,936 LOC staged. 248 Obsidian notes mapped.*
*The field accumulates. ORAC observes, amplifies, and coordinates.*
