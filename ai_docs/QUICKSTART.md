# ORAC Sidecar — Quick Start

> **Envoy-like proxy specialized for AI agent traffic**
> **Port:** 8133 | **Binary:** `orac-sidecar` | **DevEnv Batch:** 5 | **Rust 2021, MSRV 1.75**
> **GitLab:** `git@gitlab.com:lukeomahoney/orac-sidecar.git`
> **Status:** PHASE 1 COMPLETE — HTTP hook server live on :8133, 699 tests, binary deployed
> **Obsidian:** `[[Session 050 — ORAC Sidecar Architecture]]` | `[[Session 052 — Phase 1 Hooks Deployed]]`

---

## 1. What Is ORAC

ORAC replaces the V1 swarm-sidecar (546 LOC, non-functional 17h+ due to V1/V2 wire mismatch) with an intelligent fleet coordination proxy. It handles 10 gaps bash hooks cannot fill: real-time push via IPC, bidirectional event streaming, persistent socket multiplexing, sub-second coordination, cross-pane awareness, high-frequency Hebbian STDP, persistent fleet state, WASM plugin bridge, closed-loop thermal damping, and HTTP hook server.

**Validated by:** arxiv 2508.12314 (Kuramoto oscillators for AI agent coordination).

---

## 2. Build & Run

```bash
# Build
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release 2>&1 | tail -5

# Quality gate (MANDATORY before every commit/deploy)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30

# Deploy binary
\cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/orac-sidecar

# Run (manual)
PORT=8133 ~/.local/bin/orac-sidecar

# Run (via devenv — PREFERRED)
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
```

### Prerequisites

Start dependent services before ORAC:
```bash
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
# Verify:
for p in 8132 8090 8080 8125 8130; do
  echo "$p: $(curl -s -o /dev/null -w '%{http_code}' localhost:$p/health 2>/dev/null)"
done
```

### Health Check

```bash
# ORAC
curl -s localhost:8133/health | jq .

# PV2 (IPC bus provider)
curl -s localhost:8132/health | jq '{r,spheres,tick,status}'

# All 5 bridges
for svc in "8090:SYNTHEX:/api/health" "8080:ME:/api/health" "8125:POVM:/health" "8130:RM:/health" "8132:PV2:/health"; do
  IFS=: read -r port name path <<< "$svc"
  echo "$name:$port — $(curl -s -o /dev/null -w '%{http_code}' -m 2 localhost:$port$path 2>/dev/null)"
done
```

---

## 3. Three Binaries

| Binary | Source | Purpose |
|--------|--------|---------|
| `orac-sidecar` | [`src/bin/main.rs`](../src/bin/main.rs) | Main daemon — HTTP hooks + IPC + tick loop |
| `orac-client` | [`src/bin/client.rs`](../src/bin/client.rs) | CLI — status, field, spheres, hooks, bridges |
| `orac-probe` | [`src/bin/probe.rs`](../src/bin/probe.rs) | Diagnostics — health probes all 6 services |

---

## 4. Architecture — 8 Layers, 40 Modules, 4 Build Phases

```
         ┌─────────────────────────────────────────────────────────────┐
         │                    ORAC SIDECAR (:8133)                     │
         │                                                             │
Phase 1  │  L1 Core (m01-m06)         L2 Wire (m07-m09)              │
~8K LOC  │  Types, errors, config     IPC client → PV2 bus            │
         │                                                             │
         │  L3 Hooks (m10-m14)  ← KEYSTONE                           │
         │  6 HTTP endpoints replacing 8 bash scripts                  │
         │                                                             │
Phase 2  │  L4 Intelligence (m15-m21)                                 │
~4K LOC  │  Coupling, Hebbian, semantic router, circuit breaker        │
         │                                                             │
Phase 3  │  L5 Bridges (m22-m26)      L7 Monitoring (m32-m35)        │
~6K LOC  │  SYNTHEX, ME, POVM, RM    OTel, Prometheus, dashboard      │
         │  + SQLite blackboard       + token accounting               │
         │                                                             │
         │  L6 Coordination (m27-m31)                                 │
         │  Conductor, cascade, tick, WASM bridge, memory              │
         │                                                             │
Phase 4  │  L8 Evolution (m36-m40)                                    │
~6K LOC  │  RALPH 5-phase, emergence, correlation, fitness tensor     │
         │  Multi-parameter mutation (BUG-035 fix)                     │
         └─────────────────────────────────────────────────────────────┘
              │              │              │
         PV2 Daemon     Fleet Agents    WASM Plugin
         (IPC sock)     (HTTP hooks)    (FIFO/ring)
```

### Layer → Source → Docs Mapping

| Layer | Feature Gate | `src/` Directory | Modules | Layer Doc | Module Doc |
|-------|-------------|------------------|---------|-----------|------------|
| **L1 Core** | _(always)_ | [`src/m1_core/`](../src/m1_core/) | m01-m06 + `field_state` | [`layers/L1_CORE.md`](layers/L1_CORE.md) | [`modules/L1_CORE_MODULES.md`](modules/L1_CORE_MODULES.md) |
| **L2 Wire** | _(always)_ | [`src/m2_wire/`](../src/m2_wire/) | m07-m09 | [`layers/L2_WIRE.md`](layers/L2_WIRE.md) | [`modules/L2_WIRE_MODULES.md`](modules/L2_WIRE_MODULES.md) |
| **L3 Hooks** | `api` | [`src/m3_hooks/`](../src/m3_hooks/) | m10-m14 | [`layers/L3_HOOKS.md`](layers/L3_HOOKS.md) | [`modules/L3_HOOKS_MODULES.md`](modules/L3_HOOKS_MODULES.md) |
| **L4 Intelligence** | `intelligence` | [`src/m4_intelligence/`](../src/m4_intelligence/) | m15-m21 | [`layers/L4_INTELLIGENCE.md`](layers/L4_INTELLIGENCE.md) | [`modules/L4_INTELLIGENCE_MODULES.md`](modules/L4_INTELLIGENCE_MODULES.md) |
| **L5 Bridges** | `bridges` | [`src/m5_bridges/`](../src/m5_bridges/) | m22-m26 | [`layers/L5_BRIDGES.md`](layers/L5_BRIDGES.md) | [`modules/L5_BRIDGES_MODULES.md`](modules/L5_BRIDGES_MODULES.md) |
| **L6 Coordination** | _(always)_ | [`src/m6_coordination/`](../src/m6_coordination/) | m27-m31 | [`layers/L6_COORDINATION.md`](layers/L6_COORDINATION.md) | [`modules/L6_COORDINATION_MODULES.md`](modules/L6_COORDINATION_MODULES.md) |
| **L7 Monitoring** | `monitoring` | [`src/m7_monitoring/`](../src/m7_monitoring/) | m32-m35 | [`layers/L7_MONITORING.md`](layers/L7_MONITORING.md) | [`modules/L7_MONITORING_MODULES.md`](modules/L7_MONITORING_MODULES.md) |
| **L8 Evolution** | `evolution` | [`src/m8_evolution/`](../src/m8_evolution/) | m36-m40 | [`layers/L8_EVOLUTION.md`](layers/L8_EVOLUTION.md) | [`modules/L8_EVOLUTION_MODULES.md`](modules/L8_EVOLUTION_MODULES.md) |

### Module Index

Full 41-module inventory with files, purposes, and hot-swap sources: [`modules/INDEX.md`](modules/INDEX.md)

---

## 5. Hook Endpoints (The Keystone)

ORAC replaces all 8 bash hook scripts with 6 sub-1ms HTTP endpoints:

| Event | Endpoint | Action | Response |
|-------|----------|--------|----------|
| SessionStart | `POST /hooks/SessionStart` | Register sphere on PV2 | approve + field state |
| PreToolUse | `POST /hooks/PreToolUse` | Thermal gate (SYNTHEX) | approve/deny — **fails OPEN** |
| PostToolUse | `POST /hooks/PostToolUse` | Hebbian STDP + task poll | approve |
| UserPromptSubmit | `POST /hooks/UserPromptSubmit` | Inject field state | approve + context |
| Stop | `POST /hooks/Stop` | Quality gate + deregister | approve |
| PermissionRequest | `POST /hooks/PermissionRequest` | Auto-approve policy | approve/deny + reason |

**Schemas:** [`.claude/schemas/`](../.claude/schemas/) — `hook_request.json`, `hook_response.json`, `permission_policy.schema.json`
**Debug skill:** [`.claude/skills/hook-debug/SKILL.md`](../.claude/skills/hook-debug/SKILL.md)
**Spec:** [`ai_specs/HOOKS_SPEC.md`](../ai_specs/HOOKS_SPEC.md)

---

## 6. Service Topology

```
                    ┌──────────────┐
          ┌────────→│  SYNTHEX     │ :8090  /api/health  /v3/thermal
          │         └──────────────┘
          │         ┌──────────────┐
          ├────────→│  ME          │ :8080  /api/health  /api/observer
          │         └──────────────┘
  ORAC    │         ┌──────────────┐
  :8133 ──┤────────→│  POVM        │ :8125  /health  /hydrate  /memories
          │         └──────────────┘
          │         ┌──────────────┐
          ├────────→│  RM          │ :8130  /health  POST /put (TSV!)
          │         └──────────────┘
          │         ┌──────────────┐
          └────────→│  PV2 Daemon  │ :8132  /health  /spheres  /field
                    │  IPC socket  │ /run/user/1000/pane-vortex-bus.sock
                    └──────────────┘
```

**Bridge probe skill:** [`.claude/skills/bridge-probe/SKILL.md`](../.claude/skills/bridge-probe/SKILL.md)
**Bridge spec:** [`ai_specs/BRIDGE_SPEC.md`](../ai_specs/BRIDGE_SPEC.md)

---

## 7. Hot-Swap Candidate Modules

24 files (15,936 lines) pre-refactored from PV2 and staged at [`candidate-modules/`](../candidate-modules/):

| Directory | Target Layer | Files | Lines | Action |
|-----------|-------------|-------|-------|--------|
| `drop-in/L1-foundation/` | `src/m1_core/` | 7 | ~3,373 | Copy as-is |
| `drop-in/L2-wire/` | `src/m2_wire/` | 2 | ~2,938 | Copy as-is |
| `drop-in/L4-coupling/` | `src/m4_intelligence/` | 4 | ~1,724 | Copy as-is |
| `drop-in/L4-learning/` | `src/m4_intelligence/` | 4 | ~1,424 | Copy as-is |
| `drop-in/L6-cascade/` | `src/m6_coordination/` | 1 | ~711 | Copy as-is |
| `adapt/L5-synthex/` | `src/m5_bridges/` | 1 | ~908 | Apply `## ADAPT` changes |
| `adapt/L5-me/` | `src/m5_bridges/` | 1 | ~814 | Apply `## ADAPT` changes |
| `adapt/L5-povm/` | `src/m5_bridges/` | 1 | ~924 | Apply `## ADAPT` changes |
| `adapt/L5-rm/` | `src/m5_bridges/` | 1 | ~956 | Apply `## ADAPT` changes |
| `adapt/L6-conductor/` | `src/m6_coordination/` | 1 | ~820 | Apply `## ADAPT` changes |
| `adapt/L6-tick/` | `src/m6_coordination/` | 1 | ~880 | Apply `## ADAPT` changes |

**Integration protocol:** See [ORAC_PLAN.md §Scaffold Integration Protocol](../ORAC_PLAN.md) (7-step copy→rename→wire→test process).

---

## 8. Configuration

```
config/
├── default.toml     # Defaults for all environments
├── dev.toml         # Development overrides (verbose logging)
├── prod.toml        # Production overrides (release tuning)
├── hooks.toml       # Hook endpoint config (timeouts, policy)
└── bridges.toml     # Bridge URLs, polling intervals, retry
```

**Feature flags** (in `Cargo.toml`):

| Feature | Default | Enables |
|---------|---------|---------|
| `api` | yes | Axum HTTP hook server (L3) |
| `persistence` | yes | SQLite blackboard (L5) |
| `bridges` | yes | Service bridge stubs (L5) |
| `intelligence` | no | Hebbian STDP, routing, breaker (L4) |
| `monitoring` | no | OTel, Prometheus, dashboard (L7) |
| `evolution` | no | RALPH engine (L8) |
| `full` | no | All features enabled |

```bash
# Build with all features
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full
```

---

## 9. Project File Map

```
orac-sidecar/
│
├── ORAC_PLAN.md              ← ARCHITECTURE: 4 phases, ~24.5K LOC, 33-feature backlog
├── ORAC_MINDMAP.md           ← META TREE: 248 Obsidian notes, 19 branches, 3 vaults
├── CLAUDE.md                 ← PROJECT RULES: gold standard, anti-patterns, build gate
├── CLAUDE.local.md           ← SESSION STATE: bootstrap protocol, critical path, traps
├── plan.toml                 ← SCAFFOLD PLAN: 8 layers, 40 modules, 7 features
├── Cargo.toml                ← RUST CONFIG: deps, features, lints, profile
│
├── src/                      ← SOURCE CODE: 56 .rs files, 14,324 LOC
│   ├── lib.rs                   Layer declarations with feature gates
│   ├── bin/                     3 binary targets
│   ├── m1_core/                 L1: Types, errors, config, constants, traits, validation
│   ├── m2_wire/                 L2: IPC client, bus types, wire protocol
│   ├── m3_hooks/                L3: HTTP hook server (KEYSTONE — 6 endpoints)
│   ├── m4_intelligence/         L4: Coupling, Hebbian, routing, circuit breaker
│   ├── m5_bridges/              L5: SYNTHEX, ME, POVM, RM, blackboard
│   ├── m6_coordination/         L6: Conductor, cascade, tick, WASM, memory
│   ├── m7_monitoring/           L7: OTel, metrics, dashboard, tokens
│   └── m8_evolution/            L8: RALPH, emergence, correlation, tensor, mutation
│
├── candidate-modules/        ← HOT-SWAP: 24 files (15,936 LOC) from PV2, gold standard
│   ├── drop-in/                 18 files — copy as-is into src/
│   └── adapt/                   6 files — apply ## ADAPT headers
│
├── ai_docs/                  ← DOCUMENTATION
│   ├── QUICKSTART.md            THIS FILE — you are here
│   ├── INDEX.md                 Documentation navigation hub
│   ├── GOLD_STANDARD_PATTERNS.md  10 mandatory Rust patterns (P1-P10)
│   ├── ANTI_PATTERNS.md         17 banned patterns with severity
│   ├── layers/                  8 layer reference docs (L1-L8)
│   │   ├── L1_CORE.md           → src/m1_core/  → modules/L1_CORE_MODULES.md
│   │   ├── L2_WIRE.md           → src/m2_wire/   → modules/L2_WIRE_MODULES.md
│   │   ├── L3_HOOKS.md          → src/m3_hooks/  → modules/L3_HOOKS_MODULES.md
│   │   ├── L4_INTELLIGENCE.md   → src/m4_intelligence/ → modules/L4_INTELLIGENCE_MODULES.md
│   │   ├── L5_BRIDGES.md        → src/m5_bridges/ → modules/L5_BRIDGES_MODULES.md
│   │   ├── L6_COORDINATION.md   → src/m6_coordination/ → modules/L6_COORDINATION_MODULES.md
│   │   ├── L7_MONITORING.md     → src/m7_monitoring/ → modules/L7_MONITORING_MODULES.md
│   │   └── L8_EVOLUTION.md      → src/m8_evolution/ → modules/L8_EVOLUTION_MODULES.md
│   ├── modules/                 9 module docs (gold standard, YAML frontmatter)
│   │   ├── INDEX.md             41-module inventory with source paths
│   │   ├── L1_CORE_MODULES.md   m01-m06 + field_state: types, tests, design decisions
│   │   ├── L2_WIRE_MODULES.md   m07-m09: IPC, bus types, wire protocol
│   │   ├── L3_HOOKS_MODULES.md  m10-m14: hook server, session, tool, prompt, permission
│   │   ├── L4_INTELLIGENCE_MODULES.md  m15-m21: coupling, Hebbian, router, breaker
│   │   ├── L5_BRIDGES_MODULES.md  m22-m26: SYNTHEX, ME, POVM, RM, blackboard
│   │   ├── L6_COORDINATION_MODULES.md  m27-m31: conductor, cascade, tick, WASM, memory
│   │   ├── L7_MONITORING_MODULES.md  m32-m35: OTel, metrics, dashboard, tokens
│   │   └── L8_EVOLUTION_MODULES.md  m36-m40: RALPH, emergence, correlation, tensor, mutation
│   └── schematics/              4 Mermaid architecture diagrams
│       ├── layer_architecture.mmd
│       ├── hook_flow.mmd
│       ├── bridge_topology.mmd
│       └── field_dashboard.mmd
│
├── ai_specs/                 ← SPECIFICATIONS
│   ├── INDEX.md                 Specs navigation hub
│   ├── API_SPEC.md              REST endpoints, request/response schemas
│   ├── HOOKS_SPEC.md            6 hook events, payload structures
│   ├── BRIDGE_SPEC.md           5 bridges, polling intervals, TSV format
│   ├── WIRE_PROTOCOL_SPEC.md    V2 NDJSON, ClientFrame/ServerFrame, handshake
│   ├── EVOLUTION_SPEC.md        RALPH 5-phase, fitness tensor, mutation
│   └── patterns/                4 design pattern specs
│       ├── BUILDER.md           Typestate builder
│       ├── CIRCUIT_BREAKER.md   Closed/Open/HalfOpen FSM
│       ├── KURAMOTO.md          Phase oscillators, order parameter
│       └── STDP.md              Hebbian spike-timing plasticity
│
├── .claude/                  ← DEVELOPMENT CONTEXT (18 files)
│   ├── context.json             Machine-readable: layers, bridges, hooks, bins
│   ├── status.json              Build phase tracking
│   ├── patterns.json            22 patterns (P01-P22)
│   ├── anti_patterns.json       20 anti-patterns (AP01-AP20)
│   ├── ALIGNMENT_VERIFICATION.md  Cross-reference audit
│   ├── schemas/                 5 JSON schemas
│   │   ├── hook_request.json       6 hook events
│   │   ├── hook_response.json      approve/deny/skip
│   │   ├── permission_policy.schema.json  fleet + per-sphere rules
│   │   ├── bus_event.schema.json   24 IPC event types
│   │   └── bus_frame.schema.json   5 wire frame types
│   ├── queries/                 3 SQL query templates
│   │   ├── blackboard.sql          Shared fleet state
│   │   ├── hook_events.sql         Hook tracking + latency
│   │   └── fleet_state.sql         Field snapshots, Hebbian, routing
│   ├── hooks/                   2 development hooks
│   │   ├── quality-gate.md         4-step gate enforcement
│   │   └── pre-commit.md           Anti-pattern scan + conventions
│   └── skills/                  3 Claude Code skills
│       ├── orac-boot/SKILL.md      Bootstrap ORAC knowledge
│       ├── hook-debug/SKILL.md     Test all 6 hook endpoints
│       └── bridge-probe/SKILL.md   Diagnose bridge connectivity
│
├── config/                   5 TOML configuration files
├── tests/                    11 integration tests (per-layer + cross-layer + stress)
├── benches/                  3 criterion benchmarks (field, Hebbian, hook latency)
├── migrations/               1 SQL schema (blackboard tables)
└── scripts/                  test-hook-server.py (Phase 1 de-risking)
```

---

## 10. Reading Order (New Context Window)

### Step 0 — Habitat Bootstrap (MANDATORY FIRST)

These two skills load the full ULTRAPLATE ecosystem context. Run them **before** reading any ORAC files — they provide the service topology, tool chains, memory systems, and operational rules that ORAC depends on.

```
1. /primehabitat     — The Habitat: Zellij 6 tabs, 16 services, IPC bus, 6 memory systems, tool chains, NEVER list
2. /deephabitat      — Deep substrate: wire protocol, 166 DBs, 55+ custom binaries, devenv batches, vault nav
```

This is the same bootstrap protocol defined in [`CLAUDE.local.md`](../CLAUDE.local.md) §BOOTSTRAP PROTOCOL. After these two skills load, you have god-tier understanding of the environment ORAC operates within.

### Step 1 — ORAC Context (30s — get oriented)
3. **[`CLAUDE.local.md`](../CLAUDE.local.md)** — session state, critical path, next step
4. **This file** — `ai_docs/QUICKSTART.md`
5. **[`.claude/context.json`](../.claude/context.json)** — machine-readable project state

### Step 2 — Ready to Code (2min)
6. **[`CLAUDE.md`](../CLAUDE.md)** — rules, quality gate, anti-patterns
7. **Layer doc** for your target layer (e.g., [`ai_docs/layers/L3_HOOKS.md`](layers/L3_HOOKS.md))
8. **Module doc** for your target modules (e.g., [`ai_docs/modules/L3_HOOKS_MODULES.md`](modules/L3_HOOKS_MODULES.md))

### Step 3 — Deep Context (5min — full architecture)
9. **[`ORAC_PLAN.md`](../ORAC_PLAN.md)** — 4 phases, module map, build sequence, key decisions
10. **[`ORAC_MINDMAP.md`](../ORAC_MINDMAP.md)** — 248 Obsidian notes, 19 branches, knowledge graph
11. **[`ai_specs/HOOKS_SPEC.md`](../ai_specs/HOOKS_SPEC.md)** or relevant spec for your work
12. **[`.claude/patterns.json`](../.claude/patterns.json)** + **[`.claude/anti_patterns.json`](../.claude/anti_patterns.json)** — 22 patterns, 20 anti-patterns

### ORAC Skills (context-free alternatives)
- **`/orac-boot`** — architecture, hooks, candidates, quality gate, traps
- **`/hook-debug`** — test all 6 hook endpoints with curl
- **`/bridge-probe`** — diagnose bridge connectivity

---

## 11. Traps (Memorize These)

| # | Trap | Fix |
|---|------|-----|
| 1 | `pkill` exit 144 kills `&&` chains | Separate commands — never chain after pkill |
| 2 | `cp` aliased to interactive | Always `\cp -f` |
| 3 | JSON to Reasoning Memory | TSV only! `printf 'cat\tagent\tconf\tttl\tcontent'` |
| 4 | BUG-035: mono-parameter mutation | Multi-parameter: round-robin + 10-gen cooldown + 50% cap |
| 5 | BUG-033: `http://` in bridge URL | Raw `SocketAddr` only |
| 6 | BUG-032: `#[derive(Default)]` on ProposalManager | Custom `impl Default` (max_active=0 trap) |
| 7 | BUG-034: POVM write-only | Must call `/hydrate` to read back |
| 8 | Blocking in hook handler | Async only, sub-1ms response (AP16) |
| 9 | Tight IPC reconnect loop | Exponential backoff 100ms→5s (AP17) |
| 10 | PreToolUse fails closed on SYNTHEX down | Must fail OPEN (AP18) |
| 11 | `stdout` in daemon | SIGPIPE death — use `tracing` only |
| 12 | `git status -uall` | Memory explosion on large repos |

---

## 12. Cross-References

### Within This Project
| Document | Purpose |
|----------|---------|
| [`ORAC_PLAN.md`](../ORAC_PLAN.md) | Architecture, 4 phases, module map, decisions, backlog |
| [`ORAC_MINDMAP.md`](../ORAC_MINDMAP.md) | 248 Obsidian notes across 3 vaults, 19 branches |
| [`ai_docs/INDEX.md`](INDEX.md) | Documentation hub — layers, schematics, patterns |
| [`ai_specs/INDEX.md`](../ai_specs/INDEX.md) | Specification hub — API, hooks, wire, bridges, evolution |
| [`ai_docs/modules/INDEX.md`](modules/INDEX.md) | 41-module inventory with source paths and hot-swap status |
| [`.claude/ALIGNMENT_VERIFICATION.md`](../.claude/ALIGNMENT_VERIFICATION.md) | Mindmap × plan × src × docs cross-reference audit |

### External Projects
| Project | Location | Relationship |
|---------|----------|-------------|
| PV2 (source) | `~/claude-code-workspace/pane-vortex-v2/` | IPC bus provider, hot-swap module source |
| PV v1 | `~/claude-code-workspace/pane-vortex/` | Legacy daemon (running binary) |
| ME v2 (gold standard) | `~/claude-code-workspace/the_maintenance_engine_v2/` | RALPH evolution source |
| V1 Sidecar | `~/claude-code-workspace/swarm-sidecar/` | Predecessor (replaced by ORAC) |
| Swarm Orchestrator | `~/claude-code-workspace/swarm-orchestrator/` | WASM plugin (FIFO/ring bridge) |

### Obsidian Vault
| Note | Content |
|------|---------|
| `[[Session 050 — ORAC Sidecar Architecture]]` | Plan creation session |
| `[[Session 051 — ORAC Sidecar .claude Scaffolding]]` | .claude folder scaffolding |
| `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` | BUG-035 lesson + diversity design |
| `[[Pane-Vortex — Fleet Coordination Daemon]]` | PV2 daemon reference |
| `[[The Habitat — Naming and Philosophy]]` | "The field modulates. It does not command." |

### Memory Systems
| System | Access |
|--------|--------|
| Auto-Memory | `~/.claude/projects/-home-louranicas/memory/MEMORY.md` |
| Reasoning Memory | `localhost:8130` — TSV format |
| POVM | `localhost:8125` — 3 memories from Session 051 |
| MCP Knowledge Graph | `mcp__memory__*` tools |
| Obsidian (main) | `~/projects/claude_code/` |
| Obsidian (shared) | `~/projects/shared-context/` |

---

*ORAC Sidecar — from dumb pipe to intelligent fleet coordination proxy.*
*The field accumulates. ORAC observes, amplifies, and coordinates.*
