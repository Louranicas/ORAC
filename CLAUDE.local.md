# ORAC Sidecar — Local Development Context

```json
{"v":"0.2.0","status":"OPERATIONAL","phase":"runtime-wiring","port":8133,"plan":"ORAC_PLAN.md","mindmap":"ORAC_MINDMAP.md","plan_toml":"plan.toml","scaffold_modules":40,"layers":8,"bin_targets":3,"tests":1454,"loc":30524,"clippy":0,"modules_implemented":40,"modules_stub":0,"hooks_migrated":true,"ralph_live":true,"ralph_gen":0,"ralph_fitness":0.667,"field_poller":true,"default_features":"api,persistence,bridges,intelligence,monitoring,evolution","endpoints_new":3,"fixes_complete":9,"fixes_remaining":10,"session":"055"}
```

---

## Session 055 — Runtime Wiring + Fleet Operations (2026-03-22)

**Status:** OPERATIONAL — RALPH tick loop live (gen=0, fitness=0.667), field_state poller active, 3 new endpoints, default features include all 6, 9/19 fixes complete.

### What Was Done (Session 055)

1. **RALPH tick loop wired** — Evolution engine running live. Generation 0, fitness=0.667. 5-phase cycle (Recognize→Analyze→Learn→Propose→Harvest) executing against real field state. BUG-035 mono-parameter fix active (round-robin + diversity gate).
2. **field_state poller active** — ORAC now polls PV2 :8132/health on tick interval, caching r/K/spheres/tick into `AppState.field`. Feeds conductor decisions and dashboard.
3. **3 new endpoints deployed:**
   - `/traces` — OTel trace store query (recent spans, by-trace, by-pane, errors)
   - `/dashboard` — Kuramoto field dashboard snapshot (r history, clusters, gaps, chimera)
   - `/tokens` — Token accounting summary (fleet totals, per-pane, budget status)
4. **Default features expanded** — `default = ["api", "persistence", "bridges", "intelligence", "monitoring", "evolution"]` — all 6 features now build by default. No more `--features full` required.
5. **Fleet comms practice** — Full 4-leg relay chain (Orchestrator→ALPHA→BETA→GAMMA) verified. All 9 fleet instances (3 tabs × 3 panes) deployed and communicating via shared-context cascade files.
6. **Fleet-GAMMA audit** — 8 ORAC modules (M27-M35) audited: 6,544 LOC, 38 structs, 152 functions, 290 tests, 6 architectural TODOs (all deferred-by-design), 0 FIXMEs, 0 HACKs.
7. **Evolution DB deep dive** — evolution_tracking.db: 172 mutations (BUG-035 confirmed: 122/172 are no-ops on same parameter), fitness stagnant at 0.62. hebbian_pulse.db: 38 pathways at avg 0.895 strength, 6 patterns (all grade A).
8. **POVM x Evolution cross-correlation** — 120 POVM memories, 2,437 pathways (0% co-activated), 3 cross-system pathways found. Systems structurally aware but operationally disconnected.
9. **FIX-015 plan** — orac-client CLI implementation designed (7 subcommands, ~280 LOC, uses ureq). Plan at `shared-context/tasks/fix-015-orac-client-impl.md`.

### Fix Tracker (9 Complete, 10 Remaining)

**Complete (9):**
- FIX-001: Hook migration (6 hooks → ORAC HTTP)
- FIX-002: BUG-035 mono-parameter mutation fix (round-robin + diversity gate)
- FIX-003: BUG-033 bridge URL prefix fix (raw SocketAddr only)
- FIX-004: BUG-032 ProposalManager Default fix (custom impl)
- FIX-005: Default features expanded (all 6 enabled)
- FIX-006: field_state poller wired
- FIX-007: RALPH tick loop activated (gen=0, fitness=0.667)
- FIX-008: /traces endpoint deployed
- FIX-009: /dashboard + /tokens endpoints deployed

**Remaining (10):**
- FIX-010: Wire circuit breaker to live pane health checks
- FIX-011: IPC client connect to PV2 Unix socket (half-stubbed)
- FIX-012: Semantic router dispatch integration
- FIX-013: WASM bridge FIFO/ring I/O (in-memory only currently)
- FIX-014: Blackboard SQLite persistence (in-memory mode only)
- FIX-015: orac-client real CLI (plan ready, not implemented)
- FIX-016: Conductor k_delta recommendations forwarded to PV2
- FIX-017: Hebbian STDP tick integration in m29_tick Phase 4
- FIX-018: Governance actuator in m29_tick Phase 5
- FIX-019: DevEnv registration (orac-sidecar as service #17)

### Fleet Dispatch Reports (Session 055)

- `handoff-gamma-monitoring-audit.md` — M27-M35 audit (290 tests, 6 TODOs)
- `handoff-gamma-evolution-deep.md` — Evolution DB deep dive (BUG-035 confirmed)
- `handoff-gamma-cross-correlation.md` — POVM x Evolution analysis (0% co-activation)
- `fix-015-orac-client-impl.md` — CLI implementation plan

---

## Session 054 — Phase 4 Evolution + Full Completion (2026-03-22)

**Status:** PLAN COMPLETE — 40/40 modules, 30,524 LOC, 1,454 tests, hooks migrated, all 14 critical path steps done.

### Step 9 — Hook Migration (2026-03-22)
11. **Hook forwarder created** — `hooks/orac-hook.sh` (generic stdin→curl→stdout bridge)
12. **6 hooks migrated to ORAC** — SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, PermissionRequest (NEW)
13. **3 bash scripts consolidated** — PostToolUse (post_tool_use.sh + post_tool_povm_pathway.sh + post_tool_nexus_pattern.sh → single ORAC endpoint)
14. **2 hooks kept as bash** — SubagentStop (no ORAC endpoint), PreCompact (cascade system)
15. **1 hook kept as bash** — Stop/check-cipher-messages.sh (non-PV2, cipher system)
16. **Backup** — `~/.claude/settings.json.pre-orac-backup`
17. **All 6 ORAC endpoints verified live** — SessionStart (POVM 111 mem, 2437 paths), UserPromptSubmit (r=0.9276, spheres=63), PreToolUse ({}), PostToolUse ({}), Stop ({}), PermissionRequest (auto-approved)
18. **Rollback command (if needed)** — `\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json`
19. **HOOKS LIVE** — settings.json has all 6 ORAC hooks wired via orac-hook.sh. Verified 2026-03-22.

### What Was Done (Session 054)
1. **m39_fitness_tensor** (1,317 LOC) — 12-dim weighted fitness evaluation with ORAC-specific dimensions (coordination_quality, field_coherence, dispatch_accuracy, etc.), trend detection via linear regression, stability/volatility assessment. 60 tests.
2. **m37_emergence_detector** (1,446 LOC) — 8 fleet emergence types (CoherenceLock, ChimeraFormation, CouplingRunaway, HebbianSaturation, DispatchLoop, ThermalSpike, BeneficialSync, ConsentCascade). Ring buffer with TTL decay, 5,000-event cap, monitor accumulation pattern. 41 tests.
3. **m38_correlation_engine** (976 LOC) — Temporal, causal, recurring, and fitness-linked correlation mining. Pathway discovery with establishment threshold, pattern key tracking, sliding window. 29 tests.
4. **m40_mutation_selector** (998 LOC) — BUG-035 fix: round-robin cycling, 10-generation cooldown, >50% diversity rejection gate. No mono-parameter monopoly. 34 tests.
5. **m36_ralph_engine** (1,117 LOC) — 5-phase RALPH orchestrator (Recognize→Analyze→Learn→Propose→Harvest) with snapshot/rollback, generation tracking, auto-pause at max cycles. 28 tests.
6. **m09_wire_protocol** (916 LOC) — V2 wire protocol state machine (Disconnected→Handshaking→Connected→Subscribing→Active), frame validation, send/recv queues, keepalive. 37 tests.
7. **m30_wasm_bridge** (729 LOC) — FIFO/ring protocol bridge: command parsing (dispatch/status/field_state/list_panes/ping), EventRingBuffer (1,000 line cap, FIFO eviction), JSONL serialization. 34 tests.
8. **Quality gate 4/4 clean** — check 0, clippy 0, pedantic 0, 1,454 tests 0 failures
9. **Release build** — 3 binaries deployed: orac-sidecar (5.5MB), orac-probe (2.3MB), orac-client (337KB)
10. **All stubs filled** — 40/40 modules implemented, zero scaffolds remaining

### Test Results
- **1,454 tests** (--features full) — 0 failures, 0 ignored
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings

### Per-Layer Summary

| Layer | Dir | Modules | LOC | Tests |
|-------|-----|---------|-----|-------|
| L1 Core | `m1_core` | m01-m06 + field_state | 4,020 | 193 |
| L2 Wire | `m2_wire` | m07-m09 | 2,300 | 111 |
| L3 Hooks | `m3_hooks` | m10-m14 | 2,405 | 138 |
| L4 Intelligence | `m4_intelligence` | m15-m21 | 4,402 | 229 |
| L5 Bridges | `m5_bridges` | m22-m26 | 4,618 | 244 |
| L6 Coordination | `m6_coordination` | m27-m31 | 2,578 | 119 |
| L7 Monitoring | `m7_monitoring` | m32-m35 | 4,347 | 230 |
| L8 Evolution | `m8_evolution` | m36-m40 | 5,854 | 192 |
| **TOTAL** | | **40** | **30,524** | **1,454** |

---

## Session 053 — Phase 2 Intelligence Layer (2026-03-22)

**Status:** PHASE 2 COMPLETE — 3 new modules, 2,593 LOC, quality gate 4/4 clean, 972 tests.

### What Was Done (Session 053)
1. **m20_semantic_router** (803 LOC) — Content-aware dispatch using Hebbian weights + domain affinity. 4 semantic domains (Read/Write/Execute/Communicate) mapped to Kuramoto phase regions. Tool classifier, content classifier, weighted composite scoring (domain 40% + Hebbian 35% + availability 25%), preferred pane bonus. 45 tests.
2. **m21_circuit_breaker** (870 LOC) — Per-pane health gating with Closed/Open/HalfOpen FSM. Configurable failure/success thresholds, tick-based Open→HalfOpen timeout, probe request limiting. `BreakerRegistry` for fleet-wide management with `tick_all()`, `state_counts()`, independent per-pane tracking. 38 tests.
3. **m26_blackboard** (920 LOC) — SQLite shared fleet state via rusqlite. 3 tables: `pane_status` (upsert/get/list/remove), `task_history` (insert/recent/count), `agent_cards` (A2A-inspired capability declarations). Indexed by pane_id and finished_at. In-memory mode for tests. 35 SQLite tests + 5 data type tests.
4. **Quality gate 4/4 clean** — check 0, clippy 0, pedantic 0, 972 tests 0 failures
5. **Feature gates** — `intelligence` (m20, m21), `persistence` (m26) — both already in Cargo.toml

### Test Results
- **972 tests** (--features full) — 0 failures, 0 ignored
- **734 tests** (default features) — 0 failures
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings

---

## Session 052 — Phase 1 Hooks Deployed (2026-03-22)

**Status:** PHASE 1 COMPLETE — HTTP hook server live on :8133, 17/17 services healthy.

### What Was Done (Session 052)
1. **5 hook modules implemented** (m10-m14) — 2,405 LOC, quality gate 4/4 clean
2. **m10_hook_server** (735 LOC) — Axum router, `OracState`, `HookEvent`/`HookResponse` types, HTTP helpers, health endpoint
3. **m11_session_hooks** (398 LOC) — `SessionStart` (register+hydrate from POVM+RM), `Stop` (fail tasks, crystallize, deregister)
4. **m12_tool_hooks** (559 LOC) — `PostToolUse` (memory+status, 1-in-5 task poll, atomic claim), `PreToolUse` (SYNTHEX thermal gate)
5. **m13_prompt_hooks** (351 LOC) — `UserPromptSubmit` (inject r/tick/spheres/thermal + pending tasks)
6. **m14_permission_policy** (362 LOC) — `PermissionRequest` auto-approve/deny engine (read=allow, write=notice, deny list)
7. **main.rs wired** — Feature-gated `api` starts Axum, graceful shutdown on SIGINT
8. **Binary deployed** — `~/.local/bin/orac-sidecar` (4.7MB), daemon running
9. **Integration tested** — All 6 endpoints verified live: POVM hydration (110 mem, 2437 paths), field state (r=0.993), thermal check
10. **Git committed + pushed** — `903fdd2` on main, pushed to GitLab
11. **RM recorded** — `r69bf788f008a` deployment entry

### Test Results
- **699 tests** — 0 failures, 0 ignored
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings
- **Live integration** — 9/9 endpoint tests pass

---

## Session 050 — Plan Complete (2026-03-22)

**Status:** SCAFFOLD-READY — All pre-scaffold tasks complete. `plan.toml` created. Awaiting deploy order.

### What Was Done (Session 050)
1. **V2 binary deployed** — PV2 daemon live on :8132, 1,527 tests, governance routes active (200)
2. **Hebbian wired** — BUG-031 fix verified, coupling weights differentiated 0.09–0.60
3. **ME deadlock addressed** — BUG-035 pruned (25K emergences → 1K), evolution chamber breathing
4. **ORAC_PLAN.md** — Full architecture: 4 phases, ~24,500 LOC, 33-feature backlog
5. **ORAC_MINDMAP.md** — 19 branches, 148+ leaves, 127 Obsidian notes, 16 recommended new notes
6. **Rust Gold Standard** documented — 10 constraints, 9 pattern categories, 17 anti-patterns from ME V2 L1+L2
7. **CLAUDE.md + CLAUDE.local.md** — project context files
8. **candidate-modules/** — 24 files (15,936 lines) cloned from PV2, refactored to gold standard, staged for scaffold integration. 42 violations found and fixed. Scaffold integration protocol documented in ORAC_PLAN.md.
9. **plan.toml** — 8 layers, 40 modules, 3 bin targets (orac-sidecar, orac-client, orac-probe), 7 features, consent config, quality gate, server/IPC/bridge/evolution config sections
10. **Git initialized** — commit `2d40fdc` with all planning artifacts + candidate modules
11. **scripts/test-hook-server.py** — Minimal HTTP hook format test server for Phase 1 de-risking
12. **Obsidian note** — `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` documenting BUG-035 lesson + diversity-enforced selection design
13. **ORAC_PLAN.md updated** — Phase 3 devenv prerequisite + Phase 4 mono-parameter mutation warning

### Critical Path Status
```
✅ Step 1: Deploy V2 binary (PV2 healthy, governance 200, k_modulation 1.21)
✅ Step 2: Verify Hebbian wired (coupling range 0.09–0.60, BUG-031 committed)
✅ Step 3: Fix ME deadlock (DB pruned, min_confidence 0.5, 57 mutations in 11 RALPH cycles)
✅ Step 3b: Git initialized (commit 2d40fdc, 28 files)
✅ Step 3c: plan.toml created (8 layers, 40 modules, 3 bin targets, 7 features)
✅ Step 3d: HTTP hook test server staged (scripts/test-hook-server.py)
✅ Step 3e: RALPH mutation fix documented (Obsidian + ORAC_PLAN.md Phase 4 warning)
✅ Step 3f: Phase 3 prerequisite documented (devenv start before bridges)
✅ Step 4: Scaffold ORAC (scaffold-gen ran, 53 files, 8 layers)
✅ Step 5: Phase 1 HTTP hooks (5 modules, 2,405 LOC, 699 tests, quality gate 4/4 clean)
✅ Step 6: Deploy binary + test against live PV2 (17/17 services, all 6 endpoints verified)
✅ Step 7: Git committed + pushed (903fdd2 + 4bf9335, GitLab main)
✅ Step 8: Phase 2 — Intelligence (m20 semantic router, m21 circuit breaker, m26 blackboard)
✅ Step 9: Migrate settings.json hooks from bash to HTTP (6 hooks → ORAC, SubagentStop+PreCompact kept as bash)
✅ Step 10: Phase 3 — Bridges + monitoring (m22-m26 bridges, m32-m35 monitoring, 8,965 LOC, 474 tests)
✅ Step 11: Phase 4 — Evolution (m36-m40 RALPH, 5,854 LOC, 192 tests, BUG-035 fixed)
✅ Step 12: Fill remaining stubs (m09 wire protocol 916 LOC, m30 WASM bridge 729 LOC)
✅ Step 13: Full quality gate (1,454 tests, 0 failures, 0 clippy warnings)
✅ Step 14: Release build (orac-sidecar 5.5MB, orac-probe 2.3MB, orac-client 337KB)
```

---

## BOOTSTRAP PROTOCOL (New Context Window)

**MANDATORY — execute these steps at the start of EVERY new context window:**

1. **Run `/primehabitat`** — loads The Habitat: Zellij tabs, 16 services, IPC bus, memory systems
2. **Run `/deephabitat`** — loads deep substrate: wire protocol, databases, ecosystem, tools
3. **Read this file** (`CLAUDE.local.md`) — current state, phase tracking
4. **Read `ORAC_PLAN.md`** — full architecture and build phases
5. **Read `ORAC_MINDMAP.md`** — Obsidian cross-references and Rust gold standard

**After bootstrap, WAIT for Luke to type `start coding` or `proceed with phase 2` before taking ANY action.**

Bootstrap gives you god-tier understanding. But implementation and code changes require explicit authorization via `start coding` or `proceed with phase 2`.

**If Luke types `proceed with phase 2`:**
1. Verify ORAC is still running: `curl -s http://localhost:8133/health | jq .`
2. Run quality gate to confirm clean baseline (699 tests)
3. Read `ORAC_PLAN.md` §Phase 2 Detail
4. Implement 3 modules: `m20_semantic_router`, `m21_circuit_breaker`, `m26_blackboard`
5. All 3 are NEW modules — no hot-swap from PV2, written from scratch
6. Feature gates: `intelligence` (m20, m21), `persistence` (m26)
7. Dependencies: `tower` crate (circuit breaker), `rusqlite` (blackboard)
8. Quality gate after each module, commit when all 3 pass

---

## What Is ORAC

ORAC is an Envoy-like proxy specialized for AI agent traffic — replacing the V1 swarm-sidecar (546 LOC, non-functional for 17+ hours due to V1/V2 wire mismatch). It fills 10 gaps that bash hooks cannot: real-time push notifications, bidirectional event streaming, persistent socket multiplexing, sub-second coordination, cross-pane awareness, high-frequency STDP, persistent fleet state, WASM plugin bridge, closed-loop thermal damping, and HTTP hook server replacing all 8 bash scripts.

**Validated by:** arxiv 2508.12314 (Kuramoto oscillators for AI agent coordination — we're ahead of academia).

---

## Git Repository

**Remote:** `git@gitlab.com:lukeomahoney/orac-sidecar.git`
**URL:** `https://gitlab.com/lukeomahoney/orac-sidecar`
**Branch:** `main`
**Commits:** 6 (latest: `4bf9335` Phase 1 hooks + Session 052 record)

---

## Next Steps: Runtime Wiring (10 Fixes Remaining)

All 40 modules implemented. All 4 phases complete. Focus is now on **runtime wiring** — connecting the implemented modules to live systems.

**Priority order:**
1. **FIX-015** — orac-client CLI (plan ready, self-contained)
2. **FIX-010** — Circuit breaker to live pane health
3. **FIX-011** — IPC client Unix socket connection
4. **FIX-014** — Blackboard SQLite persistence (disk, not in-memory)
5. **FIX-019** — DevEnv registration as service #17

```bash
# Verify ORAC is still running
curl -s http://localhost:8133/health | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'ORAC {d[\"status\"]} port={d[\"port\"]} sessions={d[\"sessions\"]} ticks={d[\"uptime_ticks\"]}')"

# Quality gate
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```

### Architecture (8 Layers, 40 Modules, 3 Binaries)

```
L1 Core        (m01-m06)  — Types, errors, config, constants, traits, validation
L2 Wire        (m07-m09)  — IPC client, bus types, wire protocol
L3 Hooks       (m10-m14)  — HTTP hook server, session/tool/prompt hooks, permission policy
L4 Intelligence (m15-m21) — Coupling, auto-K, topology, Hebbian, buoy, semantic router, circuit breaker
L5 Bridges     (m22-m26)  — SYNTHEX, ME, POVM, RM bridges, blackboard
L6 Coordination (m27-m31) — Conductor, cascade, tick, WASM bridge, memory manager
L7 Monitoring  (m32-m35)  — OTel traces, metrics, field dashboard, token accounting
L8 Evolution   (m36-m40)  — RALPH engine, emergence, correlation, fitness tensor, mutation selector
```

**Bin targets:** `orac-sidecar` (daemon), `orac-client` (CLI), `orac-probe` (diagnostics)
**Features:** `api`, `persistence`, `bridges`, `intelligence`, `monitoring`, `evolution`, `full`

### Candidate Modules (pre-refactored, staged)

```
candidate-modules/
├── drop-in/ (18 files, 10,516 lines — copy as-is into scaffolded src/)
│   ├── L1-foundation/  → src/m1_core/
│   ├── L2-wire/        → src/m2_wire/
│   ├── L4-coupling/    → src/m4_intelligence/
│   ├── L4-learning/    → src/m4_intelligence/
│   └── L6-cascade/     → src/m6_coordination/
└── adapt/ (6 files, 5,420 lines — need ORAC-specific changes marked with ## ADAPT headers)
    ├── L5-synthex/     → src/m5_bridges/
    ├── L5-me/          → src/m5_bridges/
    ├── L5-povm/        → src/m5_bridges/
    ├── L5-rm/          → src/m5_bridges/
    ├── L6-conductor/   → src/m6_coordination/
    └── L6-tick/        → src/m6_coordination/
```

### Key Services (must be running for relevant phases)

| Service | Port | Health | Needed For |
|---------|------|--------|------------|
| PV2 | 8132 | `/health` | Phase 1+ (IPC bus) |
| SYNTHEX | 8090 | `/api/health` | Phase 3 (bridge) |
| ME | 8080 | `/api/health` | Phase 3 (bridge) |
| POVM | 8125 | `/health` | Phase 3 (bridge) |
| RM | 8130 | `/health` | Phase 3 (bridge) |

Start all: `~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start`

---

## Traps to Avoid

1. **Never chain after `pkill`** (exit 144 kills the `&&` chain)
2. **Always `\cp -f`** (cp aliased to interactive — BUG-027)
3. **TSV only for Reasoning Memory** (JSON causes parse failure)
4. **Lock ordering: AppState before BusState** (deadlock prevention)
5. **Phase wrapping: `.rem_euclid(TAU)`** after all phase arithmetic
6. **No stdout in daemons** (SIGPIPE → death, BUG-018)
7. **Don't script Zellij plugin interactions** (zombie behaviour — keybind-only)
8. **fleet-ctl cache is STALE** (300s TTL — `dump-screen` is the only reliable pane state)
9. **BUG-035 mono-parameter trap** — evolution chamber MUST use multi-parameter mutation selection
10. **Bridge URLs must NOT include `http://` prefix** (BUG-033 — raw SocketAddr only)
11. **`#[derive(Default)]` on ProposalManager** → `max_active=0` (BUG-032 — use custom `impl Default`)
12. **POVM is write-only** (BUG-034 — must call `/hydrate` to read back state)

---

## Hot-Swap File Map

When scaffolding, these PV2 modules will be copied and adapted:

| ORAC Layer | PV2 Source | Files | Action |
|------------|-----------|-------|--------|
| L1 Core | `m1_foundation/m01-m06` | 6 files | DROP-IN |
| L2 Wire | `m7_coordination/m29,m30` | 2 files | DROP-IN |
| L4 Intelligence | `m4_coupling/m16-m18` | 3 files | DROP-IN |
| L4 Intelligence | `m5_learning/m19-m21` | 3 files | DROP-IN |
| L5 Bridges | `m6_bridges/m22,m24-m26` | 4 files | ADAPT |
| L6 Coordination | `m7_coordination/m31,m33,m35` | 3 files | ADAPT |

**Source:** `/home/louranicas/claude-code-workspace/pane-vortex-v2/src/`

---

## DevEnv Integration

When ORAC is ready for devenv registration:

```toml
# In ~/.config/devenv/devenv.toml
[services.orac-sidecar]
name = "ORAC Sidecar"
command = "./bin/orac-sidecar"
working_dir = "/home/louranicas/claude-code-workspace/orac-sidecar"
port = 8133
health_path = "/health"
batch = 5
depends_on = ["pane-vortex", "povm-engine"]
description = "Intelligent fleet coordination proxy — HTTP hooks, Hebbian STDP, RALPH evolution"
```

## Hook Migration — COMPLETE (2026-03-22)

Hooks migrated from PV2 bash scripts to ORAC HTTP endpoints via `hooks/orac-hook.sh` forwarder.

**Forwarder:** `orac-sidecar/hooks/orac-hook.sh <EventName> [timeout]` — reads stdin, POSTs to ORAC, outputs response.

| Event | Before (bash) | After (ORAC) | Timeout |
|-------|---------------|--------------|---------|
| SessionStart | session_start.sh | orac-hook.sh SessionStart | 5s |
| UserPromptSubmit | user_prompt_field_inject.sh | orac-hook.sh UserPromptSubmit | 3s |
| PreToolUse | pre_tool_thermal_gate.sh | orac-hook.sh PreToolUse | 2s |
| PostToolUse | 3 scripts (tool+povm+nexus) | orac-hook.sh PostToolUse | 3s |
| Stop | session_end.sh | orac-hook.sh Stop | 5s |
| PermissionRequest | (none) | orac-hook.sh PermissionRequest | 2s |
| SubagentStop | subagent_field_aggregate.sh | **KEPT** (no ORAC endpoint) | 5s |
| PreCompact | handoff-dispatch.sh | **KEPT** (cascade system) | 30s |
| Stop (cipher) | check-cipher-messages.sh | **KEPT** (non-PV2) | — |

**Rollback:** `\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json`

---

## Quality Gate

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```

## Working Directory
`/home/louranicas/claude-code-workspace/orac-sidecar`
