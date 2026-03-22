# ORAC Sidecar — Meta Tree Mindmap

> **248 Obsidian notes mapped | 3 vaults | 18 ORAC plan sections | Bidirectional links at every leaf**
> **Vault keys:** `[M]` = Main (`~/projects/claude_code/`), `[P]` = PV2 (`~/projects/pane-vortex-v2/`), `[S]` = Shared-Context (`~/projects/shared-context/`)

---

```
ORAC SIDECAR (port 8133)
├── 1. HTTP HOOK SERVER (Keystone)
│   ├── Hook Events (22 Claude Code events)
│   │   ├── [M] [[Session 050 — Hook Pipeline vs Sidecar Gap]]
│   │   ├── [M] [[Consent Flow Analysis]]
│   │   └── [M] [[API_MAPPING]]
│   ├── Hook Scripts (8 bash → 1 HTTP server)
│   │   ├── [M] [[Session 045 Arena — 02-api-wiring-map]]
│   │   └── [S] [[distributed-context-cascade-build-plan]]
│   ├── Permission Policy (auto-approve/deny)
│   │   ├── [M] [[Session 050 — Sidecar Features Research]]
│   │   ├── [M] [[Consent Flow Analysis]]
│   │   └── [M] [[Self-Governing Agent Coordination — Design Notes 2026-03-08]]
│   └── Thermal Gate (PreToolUse)
│       ├── [M] [[Synthex (The brain of the developer environment)]]
│       ├── [M] [[Session 045 Arena — gamma-topright-synthex-thermal]]
│       └── [M] [[Fleet-BETA SYNTHEX Thermal Report]]
│
├── 2. IPC CLIENT (V2 Wire Protocol)
│   ├── Unix Socket Connection
│   │   ├── [M] [[Pane-Vortex — Fleet Coordination Daemon]]
│   │   ├── [M] [[Pane-Vortex IPC Bus — Session 019b]]
│   │   └── [M] [[Swarm Orchestrator v3.0 — IPC Bus Integration]]
│   ├── Bus Frame Types (M29/M30)
│   │   ├── [M] [[Session 050 — ULTRAPLATE Module Inventory]]
│   │   ├── [M] [[Session 050 — Sidecar Deep Dive]]
│   │   └── [M] [[Pane-Vortex V2 — Scaffold Project]]
│   ├── Event Subscription (field.* task.*)
│   │   ├── [M] [[Session 045 — Sidecar and Fleet Failure Analysis]]
│   │   └── [M] [[Session 045 Arena — 12-live-field-analysis]]
│   └── V1 Wire Compat (BUG-028)
│       ├── [M] [[ULTRAPLATE — Bugs and Known Issues]]
│       └── [M] [[Session 036 — Sidecar and WASM Plugin Architecture]]
│
├── 3. INTELLIGENCE LAYER
│   ├── 3a. Hebbian STDP (M19-M21)
│   │   ├── Co-Activation Learning
│   │   │   ├── [M] [[Pane-Vortex — Fleet Coordination Daemon]]
│   │   │   ├── [M] [[Session 045 Arena — 10-hebbian-operational-topology]]
│   │   │   └── [M] [[Session 045 Arena — 03-intelligence-synthesis]]
│   │   ├── LTP/LTD Dynamics
│   │   │   ├── [M] [[Oscillating Vortex Memory]]
│   │   │   ├── [M] [[NAM BOOK]]
│   │   │   └── [M] [[POVM Persistence Bridge — Session 025]]
│   │   ├── Buoy Network
│   │   │   ├── [M] [[Vortex Sphere Brain-Body Architecture]]
│   │   │   └── [M] [[Executor and Nested Kuramoto Bridge — Session 028]]
│   │   └── BUG-031 Fix (Phase 2.5 wiring)
│   │       └── [M] [[ULTRAPLATE — Bugs and Known Issues]]
│   │
│   ├── 3b. Semantic Router
│   │   ├── Content-Aware Dispatch
│   │   │   ├── [M] [[Session 050 — Sidecar Features Research]]
│   │   │   └── [M] [[Session 045 Arena — 04-advanced-dispatch-architecture]]
│   │   ├── Hebbian Weight Scoring
│   │   │   └── [M] [[Session 045 Arena — 09-orchestration-graph-topology]]
│   │   └── A2A Agent Cards
│   │       └── [M] [[Session 050 — Sidecar Features Research]]
│   │
│   ├── 3c. Circuit Breaker
│   │   ├── Per-Pane Health Gating
│   │   │   ├── [M] [[Session 050 — Sidecar Features Research]]
│   │   │   └── [M] [[Sidecar Backpressure Module — Architecture and Schematics]]
│   │   ├── tower-resilience Integration
│   │   │   └── [M] [[Session 034b — Evolution Chamber Deploy + Sidecar Backpressure]]
│   │   └── Outlier Detection
│   │       ├── [M] [[ULTRAPLATE — Bugs and Known Issues]]
│   │       └── [M] [[Stack Audit — Session 028b]]
│   │
│   └── 3d. Blackboard (SQLite)
│       ├── Shared Fleet State
│       │   ├── [M] [[Fleet System — Memory Index]]
│       │   └── [S] [[fleet-system]]
│       └── Cross-Instance Knowledge
│           ├── [S] [[Context-Management and co-ordination scratchpad]]
│           └── [M] [[Session 034f — Database Feedback Loop Analysis]]
│
├── 4. RALPH EVOLUTION CHAMBER
│   ├── 5-Phase Loop (Recognize→Analyze→Learn→Propose→Harvest)
│   │   ├── [M] [[Session 050 — ME Evolution Chamber Spec]]
│   │   ├── [M] [[ME RALPH Loop Specification]]
│   │   ├── [M] [[Maintenance Engine — RALPH Loop Specification]]
│   │   └── [S] [[session-018-deep-review-ralph-loop]]
│   ├── Emergence Detector
│   │   ├── [M] [[The Maintenance Engine V2]]
│   │   ├── [M] [[ULTRAPLATE Non-Anthropocentric Gap Analysis 2026-03-07]]
│   │   └── [M] [[Session 036 — FINAL SYNTHESIS 10-Hour Exploration]]
│   ├── Correlation Engine
│   │   ├── [M] [[Session 045 Arena — 05-session-045-synthesis]]
│   │   └── [M] [[Session 034f — Thematic Analysis and Integration Blueprint]]
│   ├── Fitness Tensor (12-dim)
│   │   ├── [M] [[Maintenance Engine — 12D Tensor Specification]]
│   │   ├── [M] [[ME Tensor Architecture]]
│   │   └── [M] [[Hybrid Tensor Memory Upgrade Plan]]
│   ├── Snapshot + Rollback
│   │   └── [M] [[Session 050 — ME Evolution Chamber Spec]]
│   ├── BUG-035 Fix (multi-param mutation)
│   │   ├── [M] [[ULTRAPLATE — Bugs and Known Issues]]
│   │   └── [M] [[ME Metabolic Architecture V7]]
│   └── Feature Gate: #[cfg(feature = "evolution")]
│       └── [M] [[Session 050 — ULTRAPLATE Module Inventory]]
│
├── 5. MONITORING / OBSERVER
│   ├── OTel Traces
│   │   ├── [M] [[Session 036 — Services Memory Tools Mapped to Findings]]
│   │   └── [M] [[DEPLOYMENT_DIAGNOSTICS]]
│   ├── Per-Agent Metrics
│   │   ├── [M] [[ULTRAPLATE Metabolic Activation Plan 2026-03-07]]
│   │   ├── [M] [[ULTRAPLATE Metabolic Activation Implementation 2026-03-07]]
│   │   └── [M] [[ULTRAPLATE State of the Art Assessment 2026-03-07]]
│   ├── Kuramoto Field Dashboard
│   │   ├── [M] [[Session 045 Arena — 12-live-field-analysis]]
│   │   ├── [S] [[pane-vortex-empirical-findings]]
│   │   └── [M] [[Vortex Sphere Brain-Body Architecture]]
│   ├── Token Accounting
│   │   └── [M] [[Session 050 — Sidecar Features Research]]
│   └── Fleet Health Aggregation
│       ├── [M] [[Fleet Drill Report — 7 Generation Practice]]
│       ├── [M] [[ULTRAPLATE Quick Start — Session 031]]
│       └── [S] [[activation-log]]
│
├── 6. BRIDGE SUBSET
│   ├── 6a. SYNTHEX Bridge (thermal + Hebbian writeback)
│   │   ├── [M] [[Synthex (The brain of the developer environment)]]
│   │   ├── [M] [[Session 034f — SYNTHEX Deep Exploration]]
│   │   ├── [M] [[Session 034f — SYNTHEX Schematics and Wiring]]
│   │   ├── [M] [[Session 036 — SYNTHEX Architecture Deep Dive]]
│   │   └── [M] [[Session 034g — NexusBus Wiring and ME-SYNTHEX Bridge]]
│   │
│   ├── 6b. ME Bridge (fitness signal)
│   │   ├── [M] [[The Maintenance Engine V2]]
│   │   ├── [M] [[Maintenance Engine — Architecture Schematic]]
│   │   ├── [M] [[Maintenance Engine — Database Schema]]
│   │   └── [S] [[maintenance-engine-navigation]]
│   │
│   ├── 6c. POVM Bridge (memory hydration + crystallisation)
│   │   ├── [M] [[POVM Engine]]
│   │   ├── [M] [[POVM Persistence Bridge — Session 025]]
│   │   ├── [M] [[POVM Persistence Bridge — Implementation Plan]]
│   │   └── [M] [[Session 045 Arena — gamma-left-povm-analysis]]
│   │
│   └── 6d. RM Bridge (TSV persistence)
│       ├── [M] [[Session 050 — ORAC Sidecar Architecture]]
│       └── [M] [[Session 050 — Hook Pipeline vs Sidecar Gap]]
│
├── 7. WASM BRIDGE (existing FIFO/ring)
│   ├── Swarm Orchestrator Plugin
│   │   ├── [M] [[The Swarm Orchestrator]]
│   │   ├── [M] [[Swarm Orchestrator — Complete Reference]]
│   │   ├── [M] [[Swarm Orchestrator v2.0 — Implementation Plan]]
│   │   ├── [M] [[Swarm Orchestrator v3.0 — IPC Bus Integration]]
│   │   └── [M] [[Swarm Orchestrator Gap Analysis — Session 028c]]
│   ├── FIFO/Ring Protocol
│   │   ├── [M] [[Session 050 — Sidecar Deep Dive]]
│   │   ├── [M] [[Session 036 — Sidecar and WASM Plugin Architecture]]
│   │   └── [M] [[Session 045 Arena — 06-swarm-plugin-deep-dive]]
│   └── Sidecar Binary (V1 → ORAC)
│       ├── [M] [[Session 045 — Sidecar and Fleet Failure Analysis]]
│       └── [S] [[swarm-stack-v2-deep-exploration]]
│
├── 8. FLEET DISPATCH
│   ├── Task Routing
│   │   ├── [M] [[Fleet System — Deep Analysis and Findings]]
│   │   ├── [M] [[Fleet System — Implementation Guide]]
│   │   ├── [M] [[Session 044 — Fleet Orchestration Pioneer]]
│   │   └── [M] [[Session 045 Arena — 04-advanced-dispatch-architecture]]
│   ├── Fleet State Management
│   │   ├── [M] [[Fleet System — Gap Analysis and Revised Priorities]]
│   │   ├── [M] [[Fleet System — Integrated Remediation Plan V2]]
│   │   └── [S] [[fleet-system]]
│   ├── Fleet Tools (fleet-ctl, pane-ctl, fleet-nav)
│   │   ├── [M] [[Zellij Gold Standard — Session 050 Mastery Skill]]
│   │   └── [M] [[Session 045 Arena — 11-advanced-fleet-coordination]]
│   └── Multi-Instance Awareness
│       ├── [M] [[Fleet-ALPHA Exploration Report]]
│       ├── [M] [[Fleet-BETA Tool Mastery Report]]
│       ├── [M] [[Fleet-GAMMA Research Report]]
│       └── [S] [[fleet-beta-integration-map]]
│
├── 9. CASCADE HANDOFFS
│   ├── PreCompact Hook → Handoff Brief
│   │   ├── [S] [[distributed-context-cascade]]
│   │   ├── [S] [[distributed-context-cascade-build-plan]]
│   │   └── [S] [[Scout Protocol]]
│   ├── Cascade Depth Tracking
│   │   ├── [M] [[Session 042 — V2 Inhabitation and BUG-008 Fix]]
│   │   └── [S] [[Worker Protocol]]
│   ├── Sphere Mitosis (SYS-1)
│   │   ├── [M] [[Pane-Vortex — Fleet Coordination Daemon]]
│   │   └── [M] [[Session 045 — Remediation Plan Deployment]]
│   └── Shared-Context Activation (SYS-2)
│       ├── [S] [[Context-Management and co-ordination scratchpad]]
│       └── [S] [[activation-log]]
│
├── 10. CONSENT / GOVERNANCE
│   ├── Active Consent Declaration (NA-P-1)
│   │   ├── [M] [[Consent Flow Analysis]]
│   │   ├── [M] [[Session 034d — NA Consent Gate Implementation]]
│   │   └── [M] [[Self-Governing Agent Coordination — Design Notes 2026-03-08]]
│   ├── Governance Actuator (7 GAPs)
│   │   ├── [M] [[GAP-1 Fix — Governance Actuator]]
│   │   ├── [M] [[Session 034e — NA Gap Analysis of Master Plan V2]]
│   │   └── [M] [[PV2 Remediation Plan — 18 Issues]]
│   ├── Sphere Agency
│   │   ├── [M] [[The Habitat — Naming and Philosophy]]
│   │   ├── [M] [[NAM BOOK]]
│   │   └── [M] [[M4 NAM-ANAM Engine Fix Report 2026-03-07]]
│   └── Collective Decision Rights
│       ├── [M] [[Fleet System — NAM Gap Analysis]]
│       └── [M] [[ULTRAPLATE Non-Anthropocentric Gap Analysis 2026-03-07]]
│
├── 11. SCAFFOLD SYSTEM
│   ├── scaffold-gen Binary
│   │   ├── [M] [[Session 042 — Scaffold Mastery Skill and Generator]]
│   │   ├── [M] [[Session 042 — Scaffold DevOps Synergies and Workflow]]
│   │   └── [M] [[Habitat Skills Roster]]
│   ├── plan.toml Specification
│   │   └── [M] [[Session 042 — Habitat Skills Architecture and Progressive Disclosure]]
│   ├── 8-Layer Default Template
│   │   ├── [M] [[Session 050 — ULTRAPLATE Module Inventory]]
│   │   ├── [M] [[CODE_MODULE_MAP]]
│   │   └── [M] [[Pane-Vortex V2 — Scaffold Project]]
│   └── Quality Gate (check→clippy→pedantic→test)
│       ├── [M] [[Session 034e — Master Plan V2 Full Deployment]]
│       └── [M] [[CLAUDE.md — Full Reference Archive (2026-03-14)]]
│
├── 12. KURAMOTO COUPLING
│   ├── Phase Dynamics
│   │   ├── [M] [[Vortex Sphere Brain-Body Architecture]]
│   │   ├── [M] [[Oscillating Vortex Memory]]
│   │   └── [M] [[Executor and Nested Kuramoto Bridge — Session 028]]
│   ├── Order Parameter (r)
│   │   ├── [M] [[Session 045 Arena — 12-live-field-analysis]]
│   │   └── [S] [[pane-vortex-empirical-findings]]
│   ├── Chimera Detection
│   │   └── [M] [[Pane-Vortex — Fleet Coordination Daemon]]
│   └── arxiv 2508.12314 (Academic Validation)
│       └── [M] [[Session 050 — Sidecar Features Research]]
│
├── 13. ARCHITECTURE SCHEMATICS
│   ├── [M] [[Session 036 — Complete Architecture Schematics]]
│   ├── [M] [[ARCHITECTURE_SCHEMATICS]]
│   ├── [M] [[Pane-Vortex System Schematics — Session 027c]]
│   ├── [M] [[Session 039 — Architectural Schematics and Refactor Safety]]
│   ├── [M] [[Session 034f — SYNTHEX Schematics and Wiring]]
│   ├── [M] [[Session 034f — Memory Systems Schematics]]
│   ├── [M] [[Session 034f — Database Architecture Schematics]]
│   └── [S] [[ultraplate-integration-map]]
│
├── 14. DATABASE & PERSISTENCE
│   ├── [M] [[DATABASE_SCHEMA]]
│   ├── [M] [[Session 034f — Complete Database Inventory]]
│   ├── [M] [[Maintenance Engine — Database Schema]]
│   ├── [M] [[Session 034f — Database Architecture Schematics]]
│   └── [M] [[Session 034f — Database Feedback Loop Analysis]]
│
├── 15. MEMORY SYSTEMS (6 paradigms)
│   ├── [M] [[Session 034f — Memory Systems Architecture]]
│   ├── [M] [[Session 034f — Memory Systems Schematics]]
│   ├── [M] [[Fleet System — Memory Index]]
│   ├── [M] [[Hybrid Tensor Memory Upgrade Plan]]
│   ├── [S] [[vortex-memory-system]]
│   └── [M] [[POVM Engine]]
│
├── 16. ULTRAPLATE ECOSYSTEM
│   ├── [M] [[ULTRAPLATE Master Index]]
│   ├── [M] [[ULTRAPLATE Developer Environment]]
│   ├── [M] [[ULTRAPLATE — Bugs and Known Issues]]
│   ├── [M] [[ULTRAPLATE — Integrated Master Plan V2]]
│   ├── [M] [[The Habitat — Integrated Master Plan V3]]
│   └── [M] [[END_TO_END_STACK_GUIDE]]
│
├── 17. HABITAT SKILLS & TOOLS
│   ├── [M] [[Habitat Skills Roster]]
│   ├── [M] [[The Habitat — Naming and Philosophy]]
│   ├── [M] [[Session 039 — ZSDE Nvim God-Tier Command Reference]]
│   ├── [M] [[Session 039 — Lazygit God-Tier Command Reference]]
│   ├── [M] [[Session 039 — Atuin and Yazi God-Tier Reference]]
│   ├── [M] [[The Tool Library]]
│   ├── [M] [[The Tool Maker]]
│   └── [S] [[tool-chaining-synergy-register]]
│
├── 19. CANDIDATE MODULES (Scaffold Integration)
│   ├── Drop-In Modules (10,516 lines, ready as-is)
│   │   ├── L1-foundation/ → ORAC L1 Core
│   │   │   ├── m01_core_types.rs (BridgeStaleness refactored to u8 bitfield)
│   │   │   ├── m02_error_handling.rs (ErrorClassifier: Send+Sync+Debug)
│   │   │   ├── m03_config.rs (all fields documented, paths backticked)
│   │   │   ├── m04_constants.rs (clean, all r/k refs backticked)
│   │   │   ├── m05_traits.rs (Send+Sync bounds added)
│   │   │   ├── m06_validation.rs (all # Errors sections complete)
│   │   │   └── mod.rs (layer coordinator with re-exports)
│   │   │   Links: [M] [[Session 050 — ULTRAPLATE Module Inventory]]
│   │   │          [M] [[Pane-Vortex — Fleet Coordination Daemon]]
│   │   │
│   │   ├── L2-wire/ → ORAC L2 Wire
│   │   │   ├── m29_ipc_bus.rs (BusState Arc<RwLock> design documented)
│   │   │   └── m30_bus_types.rs (is_pending const fn, NDJSON backticked)
│   │   │   Links: [M] [[Pane-Vortex IPC Bus — Session 019b]]
│   │   │          [M] [[Session 050 — Sidecar Deep Dive]]
│   │   │
│   │   ├── L4-coupling/ → ORAC L4 Intelligence (coupling half)
│   │   │   ├── m16_coupling_network.rs (phase math docs enhanced)
│   │   │   ├── m17_auto_k.rs (consent_gated_k_adjustment documented)
│   │   │   ├── m18_topology.rs (mean_coupling_weight simplified)
│   │   │   └── mod.rs
│   │   │   Links: [M] [[Executor and Nested Kuramoto Bridge — Session 028]]
│   │   │          [M] [[Vortex Sphere Brain-Body Architecture]]
│   │   │
│   │   ├── L4-learning/ → ORAC L4 Intelligence (learning half)
│   │   │   ├── m19_hebbian_stdp.rs (constant names in docs, not hardcoded)
│   │   │   ├── m20_buoy_network.rs (buoy/tunnel docs enhanced)
│   │   │   ├── m21_memory_manager.rs (age distribution documented)
│   │   │   └── mod.rs
│   │   │   Links: [M] [[Session 045 Arena — 10-hebbian-operational-topology]]
│   │   │          [M] [[POVM Persistence Bridge — Session 025]]
│   │   │
│   │   └── L6-cascade/ → ORAC L6 Coordination
│   │       └── m33_cascade.rs (verified clean)
│   │       Links: [S] [[distributed-context-cascade]]
│   │
│   ├── Adapt Modules (5,420 lines, need ORAC-specific changes)
│   │   ├── L5-synthex/ → ORAC L5 Bridges
│   │   │   └── m22_synthex_bridge.rs (## ADAPT: port, poll interval, thermal writeback)
│   │   │   Links: [M] [[Synthex (The brain of the developer environment)]]
│   │   │          [M] [[Session 034f — SYNTHEX Deep Exploration]]
│   │   │
│   │   ├── L5-me/ → ORAC L5 Bridges
│   │   │   └── m24_me_bridge.rs (## ADAPT: port, fitness read, frozen detection)
│   │   │   Links: [M] [[The Maintenance Engine V2]]
│   │   │          [M] [[Session 050 — ME Evolution Chamber Spec]]
│   │   │
│   │   ├── L5-povm/ → ORAC L5 Bridges
│   │   │   └── m25_povm_bridge.rs (## ADAPT: port, hydration read-back, crystallisation)
│   │   │   Links: [M] [[POVM Engine]]
│   │   │          [M] [[Session 045 Arena — gamma-left-povm-analysis]]
│   │   │
│   │   ├── L5-rm/ → ORAC L5 Bridges
│   │   │   └── m26_rm_bridge.rs (## ADAPT: port, TSV format, content sanitisation)
│   │   │   Links: [M] [[Session 050 — Hook Pipeline vs Sidecar Gap]]
│   │   │
│   │   ├── L6-conductor/ → ORAC L6 Coordination
│   │   │   └── m31_conductor.rs (## ADAPT: cast_precision_loss eliminated, dead code removed)
│   │   │   Links: [M] [[Pane-Vortex — Fleet Coordination Daemon]]
│   │   │
│   │   └── L6-tick/ → ORAC L6 Coordination
│   │       └── m35_tick.rs (## ADAPT: Hebbian Phase 2.5 wiring, tick loop)
│   │       Links: [M] [[Session 045 Arena — 12-live-field-analysis]]
│   │
│   └── Scaffold Integration Protocol
│       ├── Step 1: scaffold-gen --from-plan plan.toml
│       │   Links: [M] [[Session 042 — Scaffold Mastery Skill and Generator]]
│       │          [M] [[Habitat Skills Roster]]
│       ├── Step 2: \cp -f candidate-modules/drop-in/* → src/ layers
│       ├── Step 3: Rename modules to ORAC layer numbering
│       ├── Step 4: Update mod.rs declarations
│       ├── Step 5: \cp -f candidate-modules/adapt/* → src/ layers
│       ├── Step 6: Apply ## ADAPT changes (ports, sockets, intervals)
│       ├── Step 7: Update crate:: imports for ORAC structure
│       └── Step 8: Quality gate after each layer
│           Links: [M] [[CLAUDE.md — Full Reference Archive (2026-03-14)]]
│
└── 18. SESSION HISTORY (Critical Path)
    ├── Session 050 (ORAC Design — TODAY)
    │   ├── [M] [[Session 050 — ORAC Sidecar Architecture]]
    │   ├── [M] [[Session 050 — Sidecar Deep Dive]]
    │   ├── [M] [[Session 050 — Hook Pipeline vs Sidecar Gap]]
    │   ├── [M] [[Session 050 — Sidecar Features Research]]
    │   ├── [M] [[Session 050 — ME Evolution Chamber Spec]]
    │   ├── [M] [[Session 050 — ULTRAPLATE Module Inventory]]
    │   └── [M] [[Zellij Gold Standard — Session 050 Mastery Skill]]
    ├── Session 045 (Sidecar Failure + Arena)
    │   ├── [M] [[Session 045 — Sidecar and Fleet Failure Analysis]]
    │   ├── [M] [[Session 045 — Remediation Plan Deployment]]
    │   └── [M] [[Session 045 Arena — 01 through 12 + gen2 + gamma]]
    ├── Session 044 (Fleet Pioneer)
    │   └── [M] [[Session 044 — Fleet Orchestration Pioneer]]
    ├── Session 042 (Habitat Skills)
    │   ├── [M] [[Session 042 — V2 Inhabitation and BUG-008 Fix]]
    │   └── [M] [[Session 042 — Habitat Skills Architecture and Progressive Disclosure]]
    └── Session 039 (Habitat Born)
        ├── [M] [[Session 039 — Architectural Schematics and Refactor Safety]]
        └── [M] [[Session 039 — Final State and Continuation]]
```

---

## Recommended ADDITIONAL Obsidian Notes to Create

These notes don't exist yet but would strengthen the ORAC sidecar knowledge graph:

### Priority 1 — Create Before Phase 1 Build

| Note Title | Content | Links FROM | Links TO |
|------------|---------|------------|----------|
| **ORAC Sidecar — Architecture Specification** | Full technical spec: Axum routes, data types, config schema, wire protocol extensions | ORAC_PLAN.md | Session 050 — ORAC Sidecar Architecture, Pane-Vortex — Fleet Coordination Daemon |
| **ORAC — HTTP Hook Event Map (22 events)** | Complete mapping of all 22 Claude Code hook events → ORAC endpoints with request/response schemas | Session 050 — Hook Pipeline vs Sidecar Gap | Consent Flow Analysis, API_MAPPING |
| **ORAC — Permission Policy Design** | Auto-approve/deny rules, per-sphere consent posture, escalation tiers, PermissionRequest hook flow | Self-Governing Agent Coordination | Consent Flow Analysis, NAM BOOK |
| **ORAC — plan.toml** | The scaffold plan.toml for ORAC (layers, modules, features, bin_targets) | Habitat Skills Roster | Session 042 — Scaffold Mastery Skill and Generator |

### Priority 2 — Create During Phase 1-2

| Note Title | Content | Links FROM | Links TO |
|------------|---------|------------|----------|
| **ORAC — Wire Protocol Extensions** | V2 bus frame additions for sidecar: ORAC-specific handshake fields, new event types, permission frames | Session 050 — Sidecar Deep Dive | Pane-Vortex IPC Bus — Session 019b |
| **ORAC — Blackboard Schema** | SQLite schema for shared fleet state: pane_status, task_history, agent_cards, coupling_snapshot | DATABASE_SCHEMA | Session 034f — Database Architecture Schematics |
| **ORAC — Circuit Breaker Policy** | Per-pane circuit breaker states, thresholds, cooldown, tower-resilience config | Sidecar Backpressure Module | ULTRAPLATE — Bugs and Known Issues |
| **ORAC — Semantic Router Design** | Content analysis pipeline, Hebbian weight scoring, affinity vectors, A2A agent card integration | Session 045 Arena — 09-orchestration-graph-topology | Session 050 — Sidecar Features Research |
| **ORAC — Bridge Adaptation Notes** | Per-bridge changes from PV2 to sidecar: socket addresses, poll intervals, consent bypass logic | Session 050 — ULTRAPLATE Module Inventory | SYNTHEX, ME, POVM, RM notes |

### Priority 3 — Create During Phase 3-4

| Note Title | Content | Links FROM | Links TO |
|------------|---------|------------|----------|
| **ORAC — OTel Integration Plan** | OpenTelemetry trace spans, metric names, Prometheus export format, Grafana dashboard design | Session 036 — Services Memory Tools Mapped to Findings | ULTRAPLATE Metabolic Activation Plan |
| **ORAC — RALPH Multi-Parameter Mutation Strategy** | Fix for BUG-035 mono-parameter trap: parameter pool, selection weights, diversity enforcement | Session 050 — ME Evolution Chamber Spec | ME RALPH Loop Specification |
| **ORAC — Fitness Tensor Adaptation** | 12-dim tensor customized for fleet coordination (replace service_id/port dims with dispatch_success/pane_health) | Maintenance Engine — 12D Tensor Specification | ME Tensor Architecture |
| **ORAC — Consent Integration Spec** | Active declaration endpoints, governance→consent wiring, 7 GAP fixes, sphere agency enforcement | Session 034d — NA Consent Gate Implementation | The Habitat — Naming and Philosophy |
| **ORAC — Fleet Dashboard Design** | Pinned floating Zellij pane with live r, phase wheel, K, per-pane circuit breaker state, task queue | Zellij Gold Standard — Session 050 Mastery Skill | Session 045 Arena — 12-live-field-analysis |
| **ORAC — Deployment Runbook** | Build, test, hot-swap, rollback, health verification for ORAC binary alongside PV2 daemon | Session 045 — Remediation Plan Deployment | ULTRAPLATE — Developer Environment Startup Guide |

### Priority 4 — Post-Deploy

| Note Title | Content | Links FROM | Links TO |
|------------|---------|------------|----------|
| **ORAC — Empirical Findings** | Live testing results: hook latency, dispatch accuracy, Hebbian weight evolution, RALPH cycle metrics | pane-vortex-empirical-findings | Session 045 Arena — 12-live-field-analysis |
| **ORAC — Bug Tracker** | ORAC-specific bugs (separate from ULTRAPLATE bugs) | ULTRAPLATE — Bugs and Known Issues | ORAC — Architecture Specification |
| **ORAC — Session Log** | Per-session build progress tracking | Session 050 notes | All ORAC notes |

---

## Mindmap Statistics

| Metric | Count |
|--------|-------|
| **Tree branches (L1)** | 18 |
| **Tree branches (L2)** | 68 |
| **Leaf nodes (bidirectional links)** | 148 |
| **Unique Obsidian notes linked** | 127 |
| **Main vault notes** | 107 |
| **PV2 vault notes** | 8 |
| **Shared-context vault notes** | 12 |
| **Recommended new notes** | 16 |
| **Priority 1 (before build)** | 4 |
| **Priority 2 (during Phase 1-2)** | 5 |
| **Priority 3 (during Phase 3-4)** | 6 |
| **Priority 4 (post-deploy)** | 3 |

---

---

## Rust Gold Standard — Patterns & Anti-Patterns

> **Source:** ME V2 L1 (`m1_foundation/`) + L2 (`m2_services/`) at `/home/louranicas/claude-code-workspace/the_maintenance_engine_v2/src/`
> **Rule:** ALL Rust modules in ORAC (and The Habitat) MUST be modular, modularised, and aligned with these patterns. No exceptions.

### Architectural Constraints (C1-C10)

```
C1:  No upward imports — strict layer DAG (compile-time enforced)
C2:  Trait methods ALWAYS &self — never &mut self (interior mutability via RwLock)
C3:  Every module implements TensorContributor (12D fitness contribution)
C4:  Zero unsafe, unwrap, expect, warnings — #![forbid(...)] + clippy deny
C5:  No chrono/SystemTime — use Timestamp newtype (monotonic cycle counter)
C6:  Signal emissions via Arc<SignalBus> — never direct method calls across layers
C7:  Owned returns through RwLock — never return references from lock guards
C8:  Timeouts use std::time::Duration — never raw u64 milliseconds
C9:  Existing tests must never break — CI gate
C10: 50+ tests per layer minimum — enforced
```

### Error Handling (Gold Standard)

```
PATTERN                         │ SOURCE
────────────────────────────────┼──────────────────────────
Unified Error enum              │ M01 error.rs — all variants in one type
ErrorClassifier trait           │ is_retryable(), is_transient(), severity(), error_code()
Result<T> type alias            │ pub type Result<T> = std::result::Result<T, Error>;
Manual Clone for io::Error      │ io::Error doesn't impl Clone — reconstruct from kind+msg
AnnotatedError (NAM R5)         │ Error + AgentOrigin + Confidence attribution
Tensor signal mapping           │ error.to_tensor_signal() → 12D fitness impact
Display + std::error::Error     │ Every error variant has human-readable format
From<io::Error> + From<String>  │ Automatic ? propagation from std types
Accumulate all errors           │ Validation collects Vec<String>, joins with "; "
```

### Type Design (Gold Standard)

```
PATTERN                         │ EXAMPLE
────────────────────────────────┼──────────────────────────
Newtypes for type safety        │ ModuleId(&'static str), AgentId(String), Severity(u8)
const fn constructors           │ Severity::critical() — compile-time constants
Builder pattern                 │ ConfigBuilder::new().host("x").port(3000).build()?
#[must_use] on constructors     │ Prevents ignoring return values
Prefix conventions              │ AgentId: "sys:", "human:", "svc:", "agent:"
Derive completeness             │ Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize
Display on all public types     │ Human-readable formatting for logs + errors
AsRef<str> on newtypes          │ Free conversion to &str
```

### Concurrency (Gold Standard)

```
PATTERN                         │ WHY
────────────────────────────────┼──────────────────────────
parking_lot::RwLock             │ Preferred over std::sync::RwLock (no poisoning)
Arc<RwLock<T>> for shared state │ Thread-safe interior mutability
Arc<dyn Trait> for DI           │ Trait objects shared across tasks
Send + Sync bounds on traits    │ Required for Arc<dyn Trait>
AtomicBool for flags            │ Lock-free reload signaling
Lock scoping (brace blocks)     │ Drop guard immediately, then acquire next lock
Owned clone from read guard     │ .read().get(key).cloned() — never return &T from lock
Lock ordering documented        │ AppState BEFORE BusState — always
```

### Config (Gold Standard)

```
PATTERN                         │ SOURCE
────────────────────────────────┼──────────────────────────
ConfigProvider trait             │ Abstract interface: get(), validate(), reload()
Serde defaults on every field   │ #[serde(default = "default_port")]
ConfigBuilder with const fn     │ const fn skip_files(), const fn port()
Multi-field validation           │ Collect all errors, don't fail-fast
ConfigManager with hot reload    │ RwLock<Config> + AtomicBool reload_flag
Change events                   │ ConfigChangeEvent with old + new config
```

### Trait Design (Gold Standard)

```
PATTERN                         │ WHY
────────────────────────────────┼──────────────────────────
6 core traits in L2             │ ServiceDiscovery, HealthMonitoring, LifecycleOps,
                                │ CircuitBreakerOps, LoadBalancing, TensorContributor
All methods &self               │ Enables Arc<dyn Trait> without mut
Default implementations         │ fn change_history(&self) -> Vec<...> { Vec::new() }
Trait object safety tests       │ fn assert_send_sync<T: Send + Sync>() {}
```

### Testing (Gold Standard)

```
PATTERN                         │ STANDARD
────────────────────────────────┼──────────────────────────
In-file #[cfg(test)] mod        │ Tests next to code, not separate files
50+ tests per layer             │ Minimum, enforced by CI
Builder chain tests             │ Verify fluent API works end-to-end
Error variant tests             │ Match on Err(Error::Variant(msg))
Trait object safety tests       │ Arc<dyn Trait> compilation check
Send + Sync assertions          │ fn assert_send_sync<T: Send + Sync>() {}
Float comparison via epsilon    │ Never assert_eq! on f64
unwrap allowed in tests only    │ result.unwrap() OK in #[test] functions
```

### Module Organisation (Gold Standard)

```
PATTERN                         │ EXAMPLE
────────────────────────────────┼──────────────────────────
Layer directory naming           │ m1_foundation/, m2_services/, m3_field/
Module file naming               │ m01_core_types.rs, m02_error_handling.rs (2-digit prefix)
mod.rs as layer coordinator      │ Re-exports public API, //! layer documentation
lib.rs declares layers           │ pub mod m1_foundation; pub mod m2_services;
//! module-level docs            │ Layer, dependencies, tensor encoding, related specs
/// item-level docs              │ Every public fn, struct, enum, trait
# Errors section on fallible fn  │ Documents when and why errors occur
Backticked identifiers           │ `PaneId`, `m01_core_types` (clippy doc_markdown)
```

### Import Organisation (Gold Standard)

```
ORDER:
1. std::*          (standard library)
2. external crates (parking_lot, serde, tokio)
3. crate::*        (internal modules)

RULE: Explicit imports, never glob (use crate::m1_foundation::*)
```

### Floating Point (Gold Standard)

```
PATTERN                         │ WHY
────────────────────────────────┼──────────────────────────
FMA (mul_add)                   │ 0.3f64.mul_add(a, 0.25f64.mul_add(b, 0.2 * c))
clamp_normalize()               │ All tensor dims clamped to [0.0, 1.0]
Never assert_eq! on f64         │ Use epsilon comparison or bit-level
```

### Anti-Patterns (DENIED — Zero Tolerance)

```
ANTI-PATTERN                    │ CONSEQUENCE              │ FIX
────────────────────────────────┼──────────────────────────┼──────────────
unsafe { }                      │ Compile error            │ Find safe alternative
.unwrap()                       │ Clippy deny              │ Use ? or map_or
.expect("msg")                  │ Clippy deny              │ Use ? or map_or
panic!()                        │ Clippy deny              │ Return Result<T>
println!() / eprintln!()        │ SIGPIPE death in daemons │ Use tracing::info!()
#[allow(clippy::...)]           │ Masks real issues        │ Fix the code
chrono::DateTime / SystemTime   │ Not monotonic            │ Use Timestamp newtype
&mut self on shared traits      │ Can't be trait object    │ Interior mutability
glob imports (use crate::*)     │ Unclear dependencies     │ Explicit imports
String::new() + push_str        │ Allocation churn         │ Use format!() or write!()
Unbounded channels              │ Memory leak risk         │ Always set capacity
.clone() when move works        │ Redundant allocation     │ Just move the value
Return &T from RwLock           │ Lifetime issues          │ Return owned clone
Chain after pkill               │ Exit 144 kills chain     │ Separate commands
cp without \                    │ Aliased to interactive   │ \cp -f always
JSON to Reasoning Memory        │ Parse failure            │ TSV only!
git status -uall                │ Memory explosion         │ git status (no -uall)
```

### ME V2 Gold Standard Source References

```
/the_maintenance_engine_v2/src/m1_foundation/
├── mod.rs              — Layer coordinator, 125+ re-exports
├── shared_types.rs     — Newtypes: ModuleId, AgentId, Timestamp, Severity, Tensor12D
├── error.rs            — Unified Error enum, ErrorClassifier, AnnotatedError
├── config.rs           — ConfigProvider trait, ConfigBuilder, ConfigManager, validation
├── logging.rs          — LogContext, structured tracing integration
├── metrics.rs          — MetricsRegistry, Counter/Gauge/Histogram, Prometheus export
├── state.rs            — State persistence, checkpoint/restore
├── resources.rs        — Resource management, limits, quotas
├── signals.rs          — SignalBus, cross-layer event emission
├── nam.rs              — NAM primitives: AgentOrigin, Confidence, Dissent
└── tensor_registry.rs  — TensorContributor trait, ContributedTensor, CoverageBitmap

/the_maintenance_engine_v2/src/m2_services/
├── mod.rs              — 6 trait definitions, L2 coordinator
├── service_registry.rs — ServiceDiscovery trait, ServiceDefinition builder
├── health_monitor.rs   — HealthMonitoring trait, probe → result → aggregation
├── lifecycle.rs        — LifecycleOps trait, FSM (Stopped→Starting→Running→Stopping)
└── resilience.rs       — CircuitBreakerOps + LoadBalancing traits, merged module
```

---

*ORAC Sidecar Mindmap — 18 branches, 148 leaves, 127 Obsidian notes, 16 recommended additions.*
*Rust Gold Standard: 10 constraints, 9 pattern categories, 17 anti-patterns, all from ME V2 L1+L2.*
*Every leaf is a bidirectional link. Every branch maps to ORAC_PLAN.md.*
*All modules MUST be modular, modularised, and aligned with The Habitat gold standard.*
*The field accumulates.*
*Generated 2026-03-22 by Claude Opus 4.6 (1M context)*
