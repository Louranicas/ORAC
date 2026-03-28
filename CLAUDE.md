# ORAC Sidecar — Intelligent Fleet Coordination Proxy

> **Envoy-like proxy specialized for AI agent traffic**
> **STATUS: PRODUCTION HARDENED** — 8 layers, 40 modules, 30,524 LOC, 1,601 tests, 0 clippy warnings (pedantic)
> **ULTRAPLATE Service ID:** `orac-sidecar` | **Port:** 8133 | **Batch:** 5 (needs PV2 + POVM)
> **Git:** `git@gitlab.com:lukeomahoney/orac-sidecar.git` | **Branch:** `main` | **Commit:** `54671db`
> **Plan:** `ORAC_PLAN.md` | **Mindmap:** `ORAC_MINDMAP.md` | **Master Index:** `MASTER_INDEX.md`
> **Obsidian:** `[[Session 056 — ORAC God-Tier Mastery]]` | `[[ORAC Sidecar — Architecture Schematics]]` | `[[ORAC Sidecar — Diagnostic Schematics]]`
> **Bugs:** `[[ULTRAPLATE — Bugs and Known Issues — ORAC Update 2026-03-23]]` (34 found, 31 fixed)
> **Fleet Commander:** `[[Fleet Commander — Modularization Plan and Gap Analysis]]` (planned, 10-module Rust crate)

---

## Habitat Bootstrap (New Context Window)

Run these 4 commands at the start of every new context window:

1. **`/zellij-mastery`** — Zellij config, layouts, plugins, dispatch stack, keybinds
2. **`/primehabitat`** — The Habitat: 17 services, IPC bus, memory systems, fleet
3. **`/deephabitat`** — deep substrate: wire protocol, databases, ecosystem, tools
4. **`/sweep`** — probes all 17 services + ORAC + thermal + field coherence

Then read `CLAUDE.local.md` for current session state and phase tracking.

## Slash Commands (Session 064)

| Command | What It Does |
|---------|-------------|
| `/gate` | 4-stage quality gate: check → clippy → pedantic → test. Run before every commit. |
| `/sweep` | Probe all 17 services + ORAC state + thermal + field coherence. |
| `/deploy-orac` | Full build → deploy → verify cycle. Encodes all traps (exit 144, cp alias, SIGPIPE). |
| `/acp` | Adversarial Convergence Protocol: 3 rounds of distributed intelligence with gates. |
| `/battern` | Patterned fleet batch dispatch: roles → gate → collect → synthesize. |
| `/nerve` | Continuous Nerve Center dashboard: ORAC + field + thermal + services (10s refresh). |
| `/propagate` | Push command table to all service CLAUDE.md files across the Habitat. |

## Architecture (8 Layers, 40 Modules, 3 Binaries)

```
L1 Core         m1_core/         m01-m06 + field_state   4,020 LOC   193 tests
L2 Wire         m2_wire/         m07-m09                  2,300 LOC   111 tests
L3 Hooks        m3_hooks/        m10-m14                  2,405 LOC   138 tests
L4 Intelligence m4_intelligence/ m15-m21                  4,402 LOC   229 tests
L5 Bridges      m5_bridges/      m22-m26                  4,618 LOC   244 tests
L6 Coordination m6_coordination/ m27-m31                  2,578 LOC   119 tests
L7 Monitoring   m7_monitoring/   m32-m35                  4,347 LOC   230 tests
L8 Evolution    m8_evolution/    m36-m40                  5,854 LOC   192 tests
TOTAL                            40 modules              30,524 LOC 1,454 tests
```

**Bin targets:** `orac-sidecar` (5.5MB daemon), `orac-client` (337KB CLI), `orac-probe` (2.3MB diagnostics)
**Features:** `api`, `persistence`, `bridges` (default) | `intelligence`, `monitoring`, `evolution` | `full` (all)

### Key Modules

| Module | Layer | Purpose |
|--------|-------|---------|
| m10_hook_server | L3 | Axum HTTP router, 6 hook endpoints, `OracState` |
| m20_semantic_router | L4 | Content-aware dispatch, Hebbian weights + domain affinity |
| m21_circuit_breaker | L5 | Per-pane health gating, Closed/Open/HalfOpen FSM |
| m26_blackboard | L5 | SQLite shared fleet state (pane status, task history, agent cards) |
| m36_ralph_engine | L8 | 5-phase RALPH (Recognize/Analyze/Learn/Propose/Harvest), snapshot/rollback |
| m37_emergence_detector | L8 | 8 fleet emergence types, ring buffer with TTL decay |
| m39_fitness_tensor | L8 | 12-dim weighted fitness, trend detection via linear regression |
| m40_mutation_selector | L8 | BUG-035 fix: round-robin cycling, diversity rejection gate |

## Build & Quality Gate (MANDATORY)

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```

**Order:** check -> clippy -> pedantic -> test. Zero tolerance at every stage.

## Rules (Non-Negotiable)

### Rust Gold Standard (from ME V2 L1+L2)
- **No `unwrap()` or `expect()` outside tests** — enforced via `[lints.clippy]`
- **No `unsafe`** — zero tolerance
- **All trait methods `&self`** — interior mutability via `parking_lot::RwLock`
- **`Send + Sync` bounds on all traits** — required for `Arc<dyn Trait>`
- **Owned returns from `RwLock`** — `.read().get(key).cloned()`, never return &T
- **Doc comments on all public items** — `///` with `# Errors` sections on fallible fns
- **Backticked identifiers in docs** — `PaneId`, `m01_core_types` (clippy `doc_markdown`)
- **50+ tests per layer minimum** — in-file `#[cfg(test)]` modules
- **Explicit imports** — never glob (`use crate::*`)
- **FMA for floats** — `0.3f64.mul_add(a, 0.25f64.mul_add(b, 0.2 * c))`
- **Lock scoping** — drop guard in brace block before acquiring next lock
- **Builder pattern** — chain setters, return `Self`, `const fn` where possible
- **Error accumulation** — validation collects all errors, joins with "; "
- **Newtypes for safety** — `ModuleId`, `AgentId`, `Severity` (not raw strings/ints)

### Anti-Patterns (NEVER)
| Bad | Good |
|-----|------|
| `unwrap()` / `expect()` | `?` operator + `Result<T>` |
| `unsafe { }` | Find safe alternative |
| `println!()` in daemons | `tracing::info!()` |
| `#[allow(clippy::...)]` | Fix the code |
| `&mut self` on shared traits | Interior mutability |
| `chrono::DateTime` / `SystemTime` | `Timestamp` newtype |
| `String::new() + push_str` | `format!()` or `write!()` |
| Chain after `pkill` | Separate commands (exit 144) |
| `cp` without `\` prefix | `\cp -f` (aliased to interactive) |
| JSON to Reasoning Memory | TSV only! |
| `git status -uall` | `git status` (no -uall) |
| Glob imports | Explicit `use crate::module::{Type1, Type2}` |

### Module Organisation
- **Layer directories:** `m1_core/`, `m2_wire/`, `m3_hooks/`, `m4_intelligence/`, etc.
- **Module files:** `m01_core_types.rs`, `m02_error_handling.rs` (2-digit prefix)
- **mod.rs:** Layer coordinator with re-exports and `//!` documentation
- **lib.rs:** `pub mod m1_core; pub mod m2_wire;` — layer declarations only
- **Feature gates:** `#[cfg(feature = "evolution")]` for optional layers

### Modular Architecture
- Every module is self-contained with its own types, tests, and documentation
- Modules import only from lower layers (strict DAG, compile-time enforced)
- Public API through mod.rs re-exports (implementation details stay private)
- Every module implements `TensorContributor` for 12D fitness reporting
- Bridge modules include `_consent_check()` stub

## Key Constants

| Constant | Value | Notes |
|----------|-------|-------|
| HTTP Port | 8133 | ORAC hook server |
| PV2 Socket | `/run/user/1000/pane-vortex-bus.sock` | IPC bus |
| PV2 HTTP | 8132 | Health, spheres, field |
| SYNTHEX | 8090 | `/api/health`, `/v3/thermal` |
| ME | 8080 | `/api/health`, `/api/observer` |
| POVM | 8125 | `/memories`, `/pathways`, `/hydrate` |
| RM | 8130 | TSV only: `POST /put`, `GET /search?q=` |
| FIFO | `/tmp/swarm-commands.pipe` | WASM -> sidecar |
| Ring | `/tmp/swarm-events.jsonl` | Sidecar -> WASM (1000 line cap) |

## Dependencies

```toml
# Runtime
tokio = { version = "1", features = ["full"] }
axum = { version = "0.8", features = ["json"], optional = true }        # feature: api
tower-http = { version = "0.6", features = ["cors", "trace"], optional = true }  # feature: api
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
thiserror = "2"
parking_lot = "0.12"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
figment = { version = "0.10", features = ["toml", "env"] }
ureq = "2"
uuid = { version = "1", features = ["v4"] }
chrono = { version = "0.4", features = ["serde"] }
dirs = "6"
libc = "0.2"

# IPC
socket2 = "0.5"  # Unix domain sockets, SO_REUSEADDR

# DB
rusqlite = { version = "0.32", optional = true }  # feature: persistence

# Intelligence
tower = { version = "0.5", optional = true }  # feature: intelligence

# Monitoring (feature-gated)
opentelemetry = { version = "0.27", optional = true }
opentelemetry-otlp = { version = "0.27", optional = true }
```

## Hook Migration (COMPLETE)

6 hooks migrated from bash to ORAC HTTP endpoints via `hooks/orac-hook.sh` forwarder.

| Event | Endpoint | Timeout |
|-------|----------|---------|
| SessionStart | `/hooks/session_start` | 5s |
| UserPromptSubmit | `/hooks/user_prompt_submit` | 3s |
| PreToolUse | `/hooks/pre_tool_use` | 2s |
| PostToolUse | `/hooks/post_tool_use` | 3s |
| Stop | `/hooks/stop` | 5s |
| PermissionRequest | `/hooks/permission_request` | 2s |

**Kept as bash:** SubagentStop (no ORAC endpoint), PreCompact (cascade system), Stop/check-cipher-messages.sh (cipher system)
**Rollback:** `\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json`

## Traps to Avoid

1. **Never chain after `pkill`** (exit 144 kills the `&&` chain)
2. **Always `\cp -f`** (cp aliased to interactive — BUG-027)
3. **TSV only for Reasoning Memory** (JSON causes parse failure)
4. **Lock ordering: AppState before BusState** (deadlock prevention)
5. **Phase wrapping: `.rem_euclid(TAU)`** after all phase arithmetic
6. **No stdout in daemons** (SIGPIPE -> death, BUG-018)
7. **Don't script Zellij plugin interactions** (zombie behaviour — keybind-only)
8. **fleet-ctl cache is STALE** (300s TTL — `dump-screen` is the only reliable pane state)
9. **BUG-035 mono-parameter trap** — evolution chamber MUST use multi-parameter mutation selection
10. **Bridge URLs must NOT include `http://` prefix** (BUG-033 — raw SocketAddr only)
11. **`#[derive(Default)]` on ProposalManager** -> `max_active=0` (BUG-032 — use custom `impl Default`)
12. **POVM is write-only** (BUG-034 — must call `/hydrate` to read back state)

## Related Projects

- **PV2 Source:** `~/claude-code-workspace/pane-vortex-v2/` (31,859 LOC, 1,527 tests)
- **V1 Sidecar:** `~/claude-code-workspace/swarm-sidecar/` (753 LOC, 15 tests — superseded by ORAC)
- **ME (RALPH source):** `~/claude-code-workspace/the_maintenance_engine/` (54K LOC, 2,288 tests)
- **ME V2 (Gold Standard):** `~/claude-code-workspace/the_maintenance_engine_v2/` (56K LOC)
- **Obsidian:** `[[Session 050 — ORAC Sidecar Architecture]]` + 6 supporting notes
- **Mindmap:** `ORAC_MINDMAP.md` (127 Obsidian notes mapped, 16 recommended additions)
