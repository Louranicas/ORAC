---
tags: [orac, executive-summary, architecture, session-062]
date: 2026-03-25
links:
  - "[[Session 062 — ORAC System Atlas (ACP)]]"
  - "[[ULTRAPLATE Master Index]]"
  - "[[D1_SYSTEM_ATLAS]]"
  - "[[D7_MODULE_PURPOSE_GUIDE]]"
  - "[[VERIFICATION_REPORT]]"
---

# ORAC Sidecar — Executive Summary & Module Guide

> **For anyone encountering this codebase for the first time.**
> **ACP-verified: every claim cross-checked by 2+ independent sources against live source.**

---

## Executive Summary

### What is ORAC?

ORAC is an **intelligent proxy for AI agent traffic** — think of it like Envoy or Nginx, but purpose-built to coordinate a fleet of Claude Code instances working in parallel across Zellij terminal panes.

It sits between Claude Code (the AI assistant) and a constellation of backend services, intercepting every significant event in a Claude session — when a session starts, when a tool is used, when a prompt is submitted, when a session ends — and enriching those events with fleet-wide intelligence: field coherence, coupling weights, thermal state, pending tasks, and evolutionary fitness.

### Why does it exist?

Before ORAC, fleet coordination relied on 8 bash scripts that ran synchronously, couldn't share state, and had no awareness of what other panes were doing. ORAC replaces those scripts with a persistent HTTP daemon that:

1. **Sees everything** — hooks into 6 Claude Code lifecycle events
2. **Remembers everything** — persists fleet state across sessions via SQLite
3. **Learns** — Hebbian STDP strengthens connections between panes that work well together
4. **Evolves** — the RALPH engine proposes and tests parameter mutations to improve fleet performance
5. **Coordinates** — the Kuramoto field model synchronises pane phases while detecting chimeras (healthy diversity)
6. **Bridges** — connects to 6 upstream services (SYNTHEX, ME, POVM, RM, VMS, PV2) for thermal regulation, fitness signals, memory persistence, and field state

### The numbers

| Metric | Value |
|--------|-------|
| Source files | 55 (.rs) |
| Lines of code | ~41,369 |
| Tests | ~1,748 (0 failures) |
| Clippy warnings | 0 (pedantic mode) |
| HTTP endpoints | 22 |
| Layers | 8 |
| Named modules | 40 (m01-m40) + field_state + http_helpers |
| Binaries | 4 (daemon, CLI, probe, benchmark) |
| Feature gates | 6 (all enabled by default) |
| Upstream bridges | 6 services |
| Port | 8133 |

### How it fits in ULTRAPLATE

ORAC is service #17 in the ULTRAPLATE ecosystem (Batch 5 — last to start, depends on PV2 + POVM). It runs alongside 16 other services:

```
Batch 1: DevOps Engine, CodeSynthor, POVM Engine
Batch 2: SYNTHEX, SAN-K7, Maintenance Engine, Architect, Prometheus Swarm
Batch 3: NAIS, Bash Engine, Tool Maker
Batch 4: Context Manager, Tool Library, Reasoning Memory
Batch 5: VMS, Pane-Vortex (PV2), ORAC Sidecar ← you are here
```

---

## The 8 Layers — What They Do and Why

ORAC is organised into 8 layers, each with a single responsibility. Lower layers know nothing about higher layers. This is enforced at compile time — if you try to import upward, the code won't build.

```
L8 Evolution        ← Self-improvement (RALPH engine, emergence detection)
L7 Monitoring       ← Observability (traces, metrics, dashboards, token budget)
L6 Coordination     ← Orchestration (conductor, cascades, tick loop, WASM bridge)
L5 Bridges          ← External communication (6 service bridges + SQLite)
L4 Intelligence     ← Learning & routing (Hebbian STDP, semantic router, circuit breaker)
L3 Hooks            ← HTTP API (22 endpoints, 6 hook handlers) — THE KEYSTONE
L2 Wire             ← IPC protocol (Unix socket to PV2 bus)
L1 Core             ← Foundation (types, errors, config, constants, validation)
```

---

## Every Module Explained

### Layer 1: Core — The Foundation

**Why it exists:** Every system needs shared types, error handling, and configuration. L1 provides the vocabulary that all other layers speak.

---

**m01_core_types** — *The shared vocabulary*

This is where `PaneId`, `TaskId`, `Point3D`, and `PaneSphere` live. Every pane in the fleet is represented as a `PaneSphere` — an oscillator on a unit sphere with a phase, frequency, status, memories, and buoys. The Kuramoto `OrderParameter { r, psi }` measures how synchronised the fleet is (r=1.0 means perfect lockstep, r=0.0 means chaos).

Think of it as: the data dictionary that defines what a "pane", a "task", a "sphere", and a "field" actually are.

---

**m02_error_handling** — *What goes wrong, and how to talk about it*

A single `PvError` enum with 15+ variants covering every failure mode: config errors, validation errors, bridge unreachable, bus protocol violations, persistence failures, governance rejections. Every function in the codebase returns `PvResult<T>` — there are zero `unwrap()` calls outside tests.

Think of it as: the error taxonomy that ensures no failure is silent or unhandled.

---

**m03_config** — *How to configure the system*

Uses `figment` to layer configuration from `config/default.toml`, then environment variables (`PV2_ADDR`, `SYNTHEX_ADDR`, etc.). Covers 10 sections: server, field, sphere, coupling, learning, bridges, conductor, IPC, persistence, governance.

Think of it as: the single source of truth for "what values does the system use at runtime?"

---

**m04_constants** — *The magic numbers, named and documented*

54 compile-time constants: `SPHERE_CAP=200`, `HEBBIAN_LTP=0.01`, `TICK_INTERVAL_SECS=5`, `R_TARGET_BASE=0.93`, `GHOST_MAX=20`. Every numeric literal in the codebase references a named constant here.

Think of it as: the tuning knobs, all in one place, all explained.

---

**m05_traits** — *The contracts between layers*

Defines the `Bridgeable` trait — any service bridge must implement `poll()`, `post()`, `health()`, and `is_stale()`. This means you can add a new bridge without touching any other code.

Think of it as: the interface definition that makes the system extensible.

---

**m06_validation** — *Input sanitisation*

Validators for every user-facing input: `validate_phase()` wraps to [0, 2pi), `validate_pane_id()` rejects invalid characters, `validate_weight()` clamps to [floor, 1.0]. All validators return `PvResult` with specific error variants.

Think of it as: the bouncer at the door — nothing invalid gets past L1.

---

**field_state** — *The cached picture of the fleet*

`AppState` holds the latest snapshot of the Kuramoto field: order parameter r, sphere phases, fleet mode, r trend, decision history. Updated every 5 seconds by the field poller. `SharedState` wraps it in `Arc<RwLock<>>` for thread-safe access.

Think of it as: the shared whiteboard that every layer reads to understand "what is the fleet doing right now?"

---

### Layer 2: Wire — The Communication Protocol

**Why it exists:** ORAC needs to talk to PV2 (the fleet coordination daemon) over a Unix socket using a custom binary protocol. L2 handles the connection, framing, and state machine.

---

**m07_ipc_client** — *The socket connection manager*

Manages the Unix socket connection to `/run/user/1000/pane-vortex-bus.sock`. Handles handshake, subscription, reconnection with exponential backoff (100ms → 5s cap, 10 attempts), and frame sending/receiving. Three states: Disconnected, Connecting, Connected.

Think of it as: the TCP client that keeps the line open to PV2, even when things go wrong.

---

**m08_bus_types** — *The message vocabulary*

Defines `BusFrame` — a single enum with 11 variants covering the entire IPC protocol: Handshake, Welcome, Subscribe, Subscribed, Submit, TaskSubmitted, Event, Cascade, CascadeAck, Disconnect, Error. Also defines `BusTask` (the task lifecycle) and `TaskStatus` (Pending → Claimed → Completed/Failed).

Think of it as: the dictionary of every message that can flow over the IPC bus.

---

**m09_wire_protocol** — *The state machine*

Implements the V2 NDJSON wire protocol state machine: Disconnected → Handshaking → Connected → Subscribing → Active → Closing. Validates that frames arrive in the right order, enforces max frame size (64KB), tracks send/receive statistics, and handles keepalives every 30 seconds.

Think of it as: the protocol enforcer that ensures both sides are speaking the same language.

---

### Layer 3: Hooks — The HTTP API (Keystone Layer)

**Why it exists:** This is the layer that Claude Code actually talks to. Every hook event from Claude Code arrives here as an HTTP POST, gets processed, and returns enriched context. L3 is the keystone — it imports from every other layer to orchestrate the full system.

---

**m10_hook_server** — *The central nervous system*

The largest module (3,295 LOC). Contains `OracState` (32 fields covering every subsystem), `build_router()` (22 HTTP routes), the field poller (PV2 health every 5s), and 15 GET endpoints for monitoring. This is where the Axum server binds to port 8133.

Think of it as: the main switchboard — every request arrives here, every subsystem is wired through here.

---

**m11_session_hooks** — *Session lifecycle*

Handles `SessionStart` (register sphere on PV2, hydrate memories from POVM+RM, create coupling connections, insert blackboard records) and `Stop` (fail pending tasks, crystallise state to POVM+RM, capture ghost trace, deregister sphere). A session's entire birth-to-death lifecycle.

Think of it as: the check-in and check-out desk for fleet panes.

---

**m12_tool_hooks** — *Tool usage intelligence*

Handles `PostToolUse` (record memory, update status, classify tool domain, poll for tasks every 5th call, claim and route tasks via semantic scoring) and `PreToolUse` (thermal gate — warns if SYNTHEX temperature is >30% over target before write operations). The most complex hook handler.

Think of it as: the tool-use observer that learns from every action and routes work to the best pane.

---

**m13_prompt_hooks** — *Context injection*

Handles `UserPromptSubmit` by injecting field state (r, tick, spheres, thermal temperature) and pending fleet tasks into the conversation context. Skips short prompts (<20 chars). Runs blackboard garbage collection every 720 ticks (~1 hour).

Think of it as: the context enricher that gives Claude awareness of the fleet's current state.

---

**m14_permission_policy** — *Access control*

Handles `PermissionRequest` with a configurable policy engine: Read/Glob/Grep/LS always approved, Edit/Write/Bash approved with notice, custom deny lists. Simple but essential for fleet safety.

Think of it as: the permission guard that prevents fleet agents from doing things they shouldn't.

---

### Layer 4: Intelligence — Learning and Routing

**Why it exists:** ORAC doesn't just forward messages — it *learns* from tool usage patterns and *routes* work intelligently. L4 contains the Hebbian learning engine, semantic router, and circuit breakers.

---

**m15_coupling_network** — *The Kuramoto coupling matrix*

Maintains a directed, weighted graph of connections between all spheres. Implements Kuramoto phase integration (RK4), bidirectional weight management, and automatic K scaling based on frequency spread. When spheres synchronise, their coupling strengthens.

Think of it as: the social network of the fleet — who is coupled to whom, and how strongly.

---

**m16_auto_k** — *Adaptive coupling strength*

A P-controller that periodically (every 15 ticks) recalculates the global coupling constant K based on frequency variance across spheres. Consent-gated — won't adjust K without sphere agreement. Uses exponential smoothing to prevent oscillation.

Think of it as: the thermostat for coupling — automatically adjusts how tightly panes are bound together.

---

**m17_topology** — *Network analysis*

Provides topology queries: `neighbors()` (sorted by weight), `degree()`, `mean_coupling_weight()`, `strongest_neighbor()`, `most_coupled_pair()`, `topology_summary()`. Used by the semantic router and fitness tensor to assess network health.

Think of it as: the network analyst that answers "how connected is this pane?" and "where are the bottlenecks?"

---

**m18_hebbian_stdp** — *The learning engine*

Implements Spike-Timing Dependent Plasticity: when two panes are both Working (co-active), their coupling weight increases (LTP = +0.01, with 3x burst and 2x newcomer multipliers). When one is active and the other idle, weight decreases (LTD = -0.002). Weights clamped to [0.15, 0.85] to prevent saturation. Anti-saturation guard skips STDP when fewer than 2 panes are working.

Think of it as: the learning rule — "panes that work together, couple together."

---

**m19_buoy_network** — *Spatial health markers*

Tracks buoy health per sphere: drift distance, activation count, overlap with other spheres' buoys. Provides `buoy_centroid()` for spatial recall and `nearest_buoy()` for context-sensitive memory access.

Think of it as: the spatial memory system — landmarks on the sphere surface that mark where important work happened.

---

**m20_semantic_router** — *Intelligent task routing*

Classifies tools into 4 semantic domains (Read, Write, Execute, Communicate) mapped to Kuramoto phase regions (0, pi/2, pi, 3pi/2). Routes tasks using composite scoring: domain affinity (40%) + Hebbian coupling weight (35%) + availability (25%). The preferred pane gets a 15% bonus.

Think of it as: the dispatcher that knows "this task is a Read task, and pane-3 is the best reader."

---

**m21_circuit_breaker** — *Health gating*

Per-service FSM (Closed → Open → HalfOpen → Closed) that prevents cascading failures. When a service fails 5 times consecutively, the breaker opens and stops sending requests. After 30 ticks, it enters HalfOpen and allows a single probe. If the probe succeeds, the breaker closes. `BreakerRegistry` manages breakers for all 6 services.

Think of it as: the circuit breaker panel — if a service is down, stop hammering it and wait for recovery.

---

### Layer 5: Bridges — External Communication

**Why it exists:** ORAC connects to 6 external services. Each bridge handles connection management, data formatting, error recovery, and consent gating. All bridge HTTP calls are blocking synchronous (via `ureq` in the RALPH tick loop).

---

**http_helpers** — *Raw TCP HTTP utilities*

Shared functions for all bridges: `raw_http_get()`, `raw_http_post()`, `raw_http_post_tsv()`. Uses raw TCP sockets (not hyper) for minimal overhead. Handles chunked transfer encoding, growing buffers (4KB initial, up to 2MB), and IPv6-safe Host headers.

Think of it as: the HTTP toolkit that every bridge uses to talk to its service.

---

**m22_synthex_bridge** — *Thermal regulation (port 8090)*

Reads SYNTHEX temperature and PID output every 6 ticks. Computes a coupling adjustment factor: cold systems get boosted, hot systems get dampened. Posts field state (r, K, spheres, heat sources) back to SYNTHEX for bidirectional thermal regulation. Detects frozen responses (3 identical polls → neutral fallback).

Think of it as: the thermostat bridge — reads the temperature, adjusts the coupling.

---

**m23_me_bridge** — *Fitness signal (port 8080)*

Reads Maintenance Engine observer fitness every 12 ticks. Handles nested JSON response format (`last_report.current_fitness`). Detects frozen fitness (tolerance 0.003, threshold 3 polls) and falls back to neutral 1.0. The fitness signal feeds into the RALPH tensor's D3 dimension.

Think of it as: the fitness sensor — "how healthy is the overall system?"

---

**m24_povm_bridge** — *Persistent memory (port 8125)*

Reads pathway data from POVM on startup (hydration), writes coupling weights back every 60 ticks (crystallisation). Handles dual serde aliases (`source`/`pre_id`, `target`/`post_id`) for compatibility. Max response 2MB (raised from 512KB when production hit 1.3MB with 2,437 pathways).

Think of it as: the long-term memory bridge — learning survives across restarts.

---

**m25_rm_bridge** — *Cross-session knowledge (port 8130)*

Writes to Reasoning Memory in **TSV format only** (NEVER JSON — the parser rejects it). Format: `category\tagent\tconfidence\tttl\tcontent`. Reads via search queries. Agent name is `"orac-sidecar"`. Zero-alloc single-pass sanitisation removes tabs and newlines.

Think of it as: the note-taking bridge — writes observations for future sessions to find.

---

**m26_blackboard** — *Fleet state database (SQLite)*

The shared fleet state store: 10 SQLite tables (pane_status, task_history, agent_cards, ghost_traces, consent_declarations, consent_audit, hebbian_summary, ralph_state, sessions, coupling_weights). WAL mode for durability. Opens with 3-retry backoff (100/200/400ms). In-memory mode for tests.

Think of it as: the fleet's shared whiteboard — who is where, what happened, what was decided.

---

### Layer 6: Coordination — Orchestration

**Why it exists:** Someone has to drive the tick loop, manage cascading handoffs between panes, and bridge to the Zellij WASM plugin. L6 is the conductor.

---

**m27_conductor** — *Field breathing controller*

A P-controller that reads the Kuramoto order parameter r and recommends coupling adjustments. If r is too high (over-synchronised), reduce K. If r is too low (chaotic), boost K. Adapts r_target for fleet size (0.93 for small fleets, 0.85 for >50 spheres). Detects directional flips (thrashing) and applies cooldown.

Think of it as: the breathing regulator — keeps the fleet in the sweet spot between order and chaos.

---

**m28_cascade** — *Handoff protocol*

Manages cascading handoffs between fleet tabs. Rate-limited (10/minute), depth-tracked (max 10), with auto-summarisation at depth 3. Carries consent snapshots so downstream panes inherit upstream permissions. Tracks pending/acknowledged/rejected cascades.

Think of it as: the relay runner system — passing work between panes with audit trail and safety limits.

---

**m29_tick** — *The heartbeat*

Executes one ORAC tick: (1) advance counter, (2) recompute Kuramoto field state, (3) run conductor advisory, (4) run Hebbian STDP, (5) check governance overrides. Returns `TickResult` with timing breakdown. Called every 5 seconds from the RALPH loop.

Think of it as: the metronome — every 5 seconds, the entire system takes one coordinated step.

---

**m30_wasm_bridge** — *Plugin communication*

Bridges ORAC to the Zellij swarm-orchestrator WASM plugin via FIFO (commands in) and ring file (events out, 1,000 line cap). Parses 5 command types: dispatch, status, field_state, list_panes, ping. Validates frame sizes (8KB max). FIFO eviction when ring is full.

Think of it as: the plugin hotline — the Zellij UI plugin can ask ORAC for fleet state and dispatch tasks.

---

**m31_memory_manager** — *Fleet memory hygiene*

Computes fleet-wide memory statistics (total, active, mean per sphere, near-capacity count), prunes low-activation memories, enforces capacity limits (500 per sphere), and ranks tool frequency. Advisory — doesn't mutate state directly.

Think of it as: the librarian — keeps memory usage tidy and reports what tools the fleet uses most.

---

### Layer 7: Monitoring — Observability

**Why it exists:** You can't improve what you can't measure. L7 provides traces, metrics, dashboards, and token accounting.

---

**m32_otel_traces** — *Distributed tracing*

Records OpenTelemetry spans for every hook call, bridge poll, and task lifecycle event. In-process trace store with query methods (recent, by-trace, by-pane, errors). W3C Trace Context compatible IDs.

Think of it as: the flight recorder — every significant event gets a span with timing and context.

---

**m33_metrics_export** — *Prometheus metrics*

Exports Prometheus-compatible metrics via `/metrics`: `orac_hook_latency_ms` (histogram), `orac_field_order_param` (gauge), `orac_k_effective` (gauge), `orac_pane_circuit_state` (gauge), `orac_tokens_total` (counter), `orac_bridge_poll_total` (counter).

Think of it as: the dashboard data source — Prometheus scrapes these numbers for alerting and visualisation.

---

**m34_field_dashboard** — *Kuramoto field visualisation*

Maintains a live dashboard of the Kuramoto field: r history (60-sample ring buffer), phase clusters, phase gaps, chimera detection, K effective. Computes `r_trend()` via linear regression and `r_stddev()` for stability assessment.

Think of it as: the field monitor — shows the fleet's synchronisation state over time.

---

**m35_token_accounting** — *Cost tracking*

Tracks token usage per pane and per task. Budget system with soft limit ($10 — warning) and hard limit ($50 — blocks new work). Input cost $0.000015/token, output $0.000075/token. FIFO eviction at 5,000 task records. Exposes `AccountingSummary` for the `/tokens` endpoint.

Think of it as: the budget controller — tracks how much the fleet is spending on API calls.

---

### Layer 8: Evolution — Self-Improvement

**Why it exists:** ORAC doesn't just run — it evolves. The RALPH engine continuously proposes, tests, and accepts or rejects parameter mutations to improve fleet fitness. L8 is the meta-learning layer.

---

**m36_ralph_engine** — *The 5-phase evolution loop*

**R**ecognize → **A**nalyze → **L**earn → **P**ropose → **H**arvest. Each phase runs for multiple ticks:
- **Recognize:** Survey the field, identify drifting parameters, query VMS for semantic context
- **Analyze:** Compute 12-dimensional fitness tensor, detect trends via linear regression
- **Learn:** Mine correlations (temporal, causal, recurring, fitness-linked) from mutation history
- **Propose:** Generate diverse mutations via round-robin parameter selection with cooldown
- **Harvest:** Accept (fitness +0.02), rollback (fitness -0.01), or continue observing

Snapshots state before mutations for atomic rollback. Generation counter tracks evolutionary progress.

Think of it as: the evolution chamber — the system that makes ORAC get better over time.

---

**m37_emergence_detector** — *Fleet behaviour recognition*

Detects 8 types of emergent fleet behaviour:
1. **CoherenceLock** — fleet over-synchronised (r > 0.92 for 10 ticks)
2. **ChimeraFormation** — healthy phase clusters with large gaps
3. **CouplingRunaway** — K increasing without r improvement
4. **HebbianSaturation** — >80% of weights at floor or ceiling
5. **DispatchLoop** — same task routed to same pane repeatedly
6. **ThermalSpike** — SYNTHEX temperature exceeds damping capacity
7. **BeneficialSync** — spontaneous synchronisation (r > 0.78, delta > 0.005)
8. **ConsentCascade** — multiple spheres opting out simultaneously

Ring buffer (5,000 events, 600-tick TTL). 50 concurrent monitors.

Think of it as: the pattern detector — recognises when the fleet is doing something interesting (or dangerous).

---

**m38_correlation_engine** — *Pattern mining*

Mines correlations from RALPH mutation history: temporal (consecutive fitness improvements), causal (parameter change → fitness delta), recurring (same mutation succeeds multiple times), fitness-linked (high/low fitness episodes). Feeds discovered patterns into RALPH's Learn phase.

Think of it as: the data scientist — finds hidden patterns in the fleet's evolutionary history.

---

**m39_fitness_tensor** — *12-dimensional evaluation*

Evaluates fleet fitness across 12 weighted dimensions:

| Dim | Name | Weight | What it measures |
|-----|------|--------|------------------|
| D0 | CoordinationQuality | 18% | Coupling network density and health |
| D1 | FieldCoherence | 15% | Kuramoto order parameter r |
| D2 | DispatchAccuracy | 12% | Semantic routing success rate |
| D3 | TaskThroughput | 10% | ME fitness signal |
| D4 | ErrorRate | 10% | Inverse of bridge/hook errors |
| D5 | Latency | 8% | SYNTHEX thermal convergence speed |
| D6 | HebbianHealth | 7% | Weight distribution (not all at floor) |
| D7 | CouplingStability | 6% | Circuit breaker closed fraction |
| D8 | ThermalBalance | 5% | Temperature vs target |
| D9 | FleetUtilization | 4% | Working / total spheres |
| D10 | EmergenceRate | 3% | Detected emergence events |
| D11 | ConsentCompliance | 2% | Consent gate compliance |

Weights sum to 1.0. Trend detection via 10-generation sliding window linear regression.

Think of it as: the report card — a single number (0.0 to 1.0) that says "how well is the fleet performing?"

---

**m40_mutation_selector** — *Diversity-enforced parameter selection*

Selects which parameter to mutate next, with 3 mechanisms to prevent getting stuck (BUG-035 fix):
1. **Round-robin cycling** — cycles through the full parameter pool, not random selection
2. **Per-parameter cooldown** — 10-generation minimum between targeting the same parameter
3. **Diversity rejection gate** — rejects if >50% of last 20 mutations hit the same parameter

10 mutable parameters: K_modulation, r_target, thermal_setpoint, dispatch_timeout, ltp_alpha, ltd_alpha, breaker thresholds (failure + success), session_ttl, emergence_confidence_min.

Think of it as: the mutation engine — picks *what* to change next, ensuring the search explores broadly.

---

## The 4 Binaries

| Binary | Purpose | Key Detail |
|--------|---------|------------|
| **orac-sidecar** | Main daemon (1,779 LOC) | HTTP server on :8133, RALPH loop, field poller, IPC listener, STDP, bridge polling, emergence detection, persistence |
| **orac-client** | CLI tool (804 LOC) | 10 subcommands: health, field, thermal, blackboard, metrics, traces, tokens, coupling, hebbian, consent |
| **orac-probe** | Diagnostics (40 LOC) | Single-shot health check for scripts |
| **ralph-bench** | Benchmark (120 LOC) | Performance testing for RALPH fitness evaluation + mutation selection |

---

## How Data Flows

```
Claude Code
    │
    ├─ SessionStart ──→ ORAC ──→ register sphere on PV2
    │                         ──→ hydrate from POVM + RM
    │                         ──→ create coupling connections
    │
    ├─ UserPromptSubmit ──→ ORAC ──→ inject field state (r, tick, spheres, thermal)
    │                             ──→ inject pending fleet tasks
    │
    ├─ PreToolUse ──→ ORAC ──→ check SYNTHEX thermal gate
    │                        ──→ allow/warn based on temperature
    │
    ├─ PostToolUse ──→ ORAC ──→ record memory + update status
    │                         ──→ classify tool domain (Read/Write/Execute/Communicate)
    │                         ──→ poll for tasks (every 5th call)
    │                         ──→ claim + route task via semantic scoring
    │
    ├─ Stop ──→ ORAC ──→ fail pending tasks
    │                  ──→ crystallise state to POVM + RM
    │                  ──→ deregister sphere, capture ghost trace
    │
    └─ PermissionRequest ──→ ORAC ──→ auto-approve/deny per policy

Meanwhile, every 5 seconds:
    ORAC tick loop ──→ Kuramoto field computation
                   ──→ Conductor advisory (k_delta)
                   ──→ Hebbian STDP (LTP/LTD weight updates)
                   ──→ Emergence detection (8 detectors)
                   ──→ RALPH evolution (5-phase cycle)
                   ──→ Bridge polling (SYNTHEX/ME/POVM/RM/VMS/PV2)
                   ──→ State persistence (blackboard, POVM, RM)
```

---

## Reading Paths by Audience

**Rust Developer (new to project):**
1. [[CONTRIBUTING]] — build, test, style guide (no AI references)
2. [[GLOSSARY]] — understand domain terminology (100 terms + reverse index)
3. [[D7_MODULE_PURPOSE_GUIDE]] — what each module does
4. [[ADR_INDEX]] — why it was built this way

**DevOps / Operator (something is broken):**
1. [[OPERATIONS]] — self-contained: ports, restart order, 25 SYM entries, debug workflows
2. [[D1_SYSTEM_ATLAS]] — ports, deploy, verify

**Architecture Reviewer (production readiness):**
1. This document (EXECUTIVE_SUMMARY)
2. [[PRODUCTION_READINESS]] — what works, what's scaffold, what's facade, security posture
3. [[ADR_INDEX]] — 15 architectural decisions with trade-offs
4. [[D6_CAPACITY_LIMITS_REFERENCE]] — scaling analysis + capacity limits

**AI Agent (Claude Code, new session):**
1. `CLAUDE.md` — bootstrap protocol, architecture, rules
2. `CLAUDE.local.md` — current session state
3. `/primehabitat` → `/deephabitat` — load environment

---

## Key Insight

ORAC is not just a proxy. It is a **living system** that observes, learns, and evolves. The Kuramoto field provides coordination without command. The Hebbian weights encode experience. The RALPH engine searches for better configurations. The consent gates ensure no sphere is forced.

Built by a social worker who put clinical ethics into Rust. Consent gates are informed consent. Opt-out is self-determination. Ghost traces remember those who leave. The field modulates. It does not command.

---

*[[Session 062 — ORAC System Atlas (ACP)]] | [[ULTRAPLATE Master Index]] | [[D7_MODULE_PURPOSE_GUIDE]]*
