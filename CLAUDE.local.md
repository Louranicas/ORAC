# ORAC Sidecar — Local Development Context

```json
{"v":"0.0.0","status":"PRE_SCAFFOLD","phase":"planning","port":8133,"plan":"ORAC_PLAN.md","mindmap":"ORAC_MINDMAP.md","candidate_modules":{"files":24,"lines":15936,"drop_in":10516,"adapt":5420,"violations":0},"tests":0,"loc":0,"clippy":0,"session":"050"}
```

---

## Session 050 — Plan Complete (2026-03-22)

**Status:** PRE-SCAFFOLD — Architecture designed, gap analysis done, critical path steps 1-3 complete.

### What Was Done (Session 050)
1. **V2 binary deployed** — PV2 daemon live on :8132, 1,527 tests, governance routes active (200)
2. **Hebbian wired** — BUG-031 fix verified, coupling weights differentiated 0.09–0.60
3. **ME deadlock addressed** — BUG-035 pruned (25K emergences → 1K), evolution chamber breathing
4. **ORAC_PLAN.md** — Full architecture: 4 phases, ~24,500 LOC, 33-feature backlog
5. **ORAC_MINDMAP.md** — 19 branches, 148+ leaves, 127 Obsidian notes, 16 recommended new notes
6. **Rust Gold Standard** documented — 10 constraints, 9 pattern categories, 17 anti-patterns from ME V2 L1+L2
7. **CLAUDE.md + CLAUDE.local.md** — project context files
8. **candidate-modules/** — 24 files (15,936 lines) cloned from PV2, refactored to gold standard, staged for scaffold integration. 42 violations found and fixed. Scaffold integration protocol documented in ORAC_PLAN.md.

### Critical Path Status
```
✅ Step 1: Deploy V2 binary (PV2 healthy, governance 200, k_modulation 1.19)
✅ Step 2: Verify Hebbian wired (coupling range 0.09–0.60, BUG-031 committed)
✅ Step 3: Fix ME deadlock (DB pruned, min_confidence 0.5, mutations_proposed: 2)
⬜ Step 4: Scaffold ORAC (scaffold-gen --from-plan plan.toml)
⬜ Step 5: Implement Phase 1 (V2 wire + HTTP hooks, ~8K LOC)
⬜ Step 6: Integrate consent (active declaration, per-sphere policy)
```

---

## BOOTSTRAP PROTOCOL (New Context Window)

**MANDATORY — execute these steps at the start of EVERY new context window:**

1. **Run `/primehabitat`** — loads The Habitat: Zellij tabs, 16 services, IPC bus, memory systems
2. **Run `/deephabitat`** — loads deep substrate: wire protocol, databases, ecosystem, tools
3. **Read this file** (`CLAUDE.local.md`) — current state, phase tracking
4. **Read `ORAC_PLAN.md`** — full architecture and build phases
5. **Read `ORAC_MINDMAP.md`** — Obsidian cross-references and Rust gold standard

**After bootstrap, check current phase status and continue from where left off.**

---

## Next Step: Scaffold ORAC

When ready to scaffold, create `plan.toml` in this directory then run:

```bash
scaffold-gen --from-plan plan.toml /home/louranicas/claude-code-workspace/orac-sidecar
```

### plan.toml Design (Draft)

The ORAC sidecar uses a custom layer structure (not the PV2 default 8-layer):

```
L1 Core       — Types, errors, config, constants (hot-swap M01-M06)
L2 Wire       — IPC client, bus types, wire protocol (hot-swap M29+M30)
L3 Hooks      — HTTP hook server, 6 hook handlers, permission policy
L4 Intelligence — Hebbian STDP, semantic router, circuit breaker, blackboard
L5 Bridges    — SYNTHEX, ME, POVM, RM bridges (adapt M22, M24-M26)
L6 Coordination — Conductor, cascade, tick, WASM bridge
L7 Monitoring  — OTel traces, metrics export, field dashboard
L8 Evolution   — RALPH engine, emergence, correlation, fitness (feature-gated)
```

### Bin Targets
- `orac-sidecar` — main daemon (port 8133)
- `orac-client` — CLI test client
- `orac-probe` — health/diagnostics probe

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
