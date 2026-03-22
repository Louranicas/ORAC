# ORAC Sidecar — Intelligent Fleet Coordination Proxy

> **Envoy-like proxy specialized for AI agent traffic**
> **STATUS: PRE-SCAFFOLD** — Plan + mindmap complete, scaffold next
> **ULTRAPLATE Service ID:** `orac-sidecar` | **Port:** 8133 | **Batch:** 5 (needs PV2 + POVM)
> **Plan:** `ORAC_PLAN.md` | **Mindmap:** `ORAC_MINDMAP.md`
> **Obsidian:** `[[Session 050 — ORAC Sidecar Architecture]]`

## Architecture (4 Build Phases, ~24,500 LOC)

```
Phase 1 — Wire + Hooks (~8K LOC):
  HTTP Hook Server (Axum, 6 endpoints)
  IPC Client (M29/M30 hot-swap, V2 wire protocol)
  WASM Bridge (FIFO/ring, existing protocol)

Phase 2 — Intelligence (~4K LOC):
  Hebbian STDP (M19-M21 hot-swap)
  Semantic Router (content-aware dispatch)
  Circuit Breaker (tower-resilience, per-pane health)
  Blackboard (SQLite, shared fleet state)

Phase 3 — Bridges + Monitoring (~6K LOC):
  SYNTHEX bridge (thermal + Hebbian writeback)
  ME bridge (fitness signal)
  POVM bridge (memory hydration + crystallisation)
  RM bridge (TSV persistence — NOT JSON)
  OTel traces + Prometheus metrics + field dashboard

Phase 4 — Evolution (~6K LOC):
  RALPH 5-phase loop (Recognize→Analyze→Learn→Propose→Harvest)
  Emergence detector, correlation engine, 12-dim fitness tensor
  Snapshot + rollback, multi-parameter mutation
  Feature-gated: #[cfg(feature = "evolution")]
```

## Build & Quality Gate (MANDATORY)

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```

**Order:** check → clippy → pedantic → test. Zero tolerance at every stage.

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
- **Layer directories:** `m1_core/`, `m2_hooks/`, `m3_intelligence/`, etc.
- **Module files:** `m01_types.rs`, `m02_errors.rs` (2-digit prefix)
- **mod.rs:** Layer coordinator with re-exports and `//!` documentation
- **lib.rs:** `pub mod m1_core; pub mod m2_hooks;` — layer declarations only
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
| FIFO | `/tmp/swarm-commands.pipe` | WASM → sidecar |
| Ring | `/tmp/swarm-events.jsonl` | Sidecar → WASM (1000 line cap) |

## Dependencies (planned)

```
Runtime: tokio, axum, tower-http, serde, serde_json, parking_lot, thiserror, tracing, ureq
IPC: socket2 (Unix domain sockets, SO_REUSEADDR)
DB: rusqlite (blackboard)
Optional: opentelemetry, opentelemetry-otlp (monitoring, feature-gated)
Hot-swap from PV2: M01-M06, M16-M21, M29-M30, M33 (10,170 LOC drop-in)
```

## Hot-Swap Strategy

Modules copied from PV2 (`~/claude-code-workspace/pane-vortex-v2/src/`):
- **DROP-IN:** M01-M06 (foundation), M16-M18 (coupling), M19-M21 (Hebbian), M29+M30 (IPC bus), M33 (cascade)
- **ADAPT:** M22 (SYNTHEX), M24 (ME), M25 (POVM), M26 (RM), M31 (conductor), M35 (tick)
- **SKIP:** M10 (API server — ORAC has own Axum), M28 (consent gate — daemon enforces)

## Related

- **PV2 Source:** `~/claude-code-workspace/pane-vortex-v2/` (31,859 LOC, 1,527 tests)
- **V1 Sidecar:** `~/claude-code-workspace/swarm-sidecar/` (753 LOC, 15 tests)
- **ME (RALPH source):** `~/claude-code-workspace/the_maintenance_engine/` (54K LOC, 2,288 tests)
- **ME V2 (Gold Standard):** `~/claude-code-workspace/the_maintenance_engine_v2/` (56K LOC)
- **Scaffold Binary:** `scaffold-gen --from-plan plan.toml`
- **Obsidian:** `[[Session 050 — ORAC Sidecar Architecture]]` + 6 supporting notes
- **Mindmap:** `ORAC_MINDMAP.md` (127 Obsidian notes mapped, 16 recommended additions)
