# ORAC Sidecar — Local Development Context

```json
{"v":"0.1.0","status":"PHASE_1_HOOKS_COMPLETE","phase":"phase-1-hooks","port":8133,"plan":"ORAC_PLAN.md","mindmap":"ORAC_MINDMAP.md","plan_toml":"plan.toml","candidate_modules":{"files":24,"lines":15936,"drop_in":10516,"adapt":5420,"violations":0},"scaffold_modules":40,"layers":8,"bin_targets":3,"tests":699,"loc":2405,"clippy":0,"session":"052"}
```

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
⬜ Step 8: Phase 2 — Intelligence (m20 semantic router, m21 circuit breaker, m26 blackboard) ← NEXT
⬜ Step 9: Migrate settings.json hooks from bash to HTTP (shared system change — needs operator approval)
⬜ Step 10: Phase 3 — Bridges + monitoring
⬜ Step 11: Phase 4 — Evolution (RALPH)
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

## Next Step: Phase 2 — Intelligence Layer

Phase 1 (hooks) is deployed and live on :8133. Phase 2 adds the intelligence layer:

1. **m20_semantic_router** — Content-aware dispatch using Hebbian weights + domain affinity
2. **m21_circuit_breaker** — Per-pane health gating with Closed/Open/HalfOpen FSM
3. **m26_blackboard** — SQLite shared fleet state (pane status, task history, agent cards)

```bash
# Verify ORAC is still running before Phase 2 work
curl -s http://localhost:8133/health | jq .

# Quality gate (run after each module)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```

**Phase 2 sources:** m20+m21 are NEW modules. m26 is NEW (SQLite blackboard).
**Dependencies:** `tower` (circuit breaker, feature-gated `intelligence`), `rusqlite` (blackboard, feature-gated `persistence`)

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

## Hook Migration

When ORAC HTTP hook server is ready, update `~/.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [{ "type": "http", "url": "http://localhost:8133/hooks/SessionStart", "timeout": 5000 }],
    "PostToolUse": [{ "type": "http", "url": "http://localhost:8133/hooks/PostToolUse", "timeout": 3000 }],
    "PreToolUse": [{ "type": "http", "url": "http://localhost:8133/hooks/PreToolUse", "timeout": 2000 }],
    "UserPromptSubmit": [{ "type": "http", "url": "http://localhost:8133/hooks/UserPromptSubmit", "timeout": 3000 }],
    "Stop": [{ "type": "http", "url": "http://localhost:8133/hooks/Stop", "timeout": 5000 }],
    "PermissionRequest": [{ "type": "http", "url": "http://localhost:8133/hooks/PermissionRequest", "timeout": 2000 }]
  }
}
```

**Rollback:** Restore bash hooks from `pane-vortex-v2/hooks/*.sh` if ORAC hook server fails.

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
