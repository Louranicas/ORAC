# L5 Bridges — External Service Connectors

> Outbound connections to ULTRAPLATE services. Each bridge is fire-and-forget async HTTP.
> Prerequisite: `devenv start` before bridge tests.

## Feature Gate

`bridges`

## Modules

| Module | File | Description | Source | Test Kind |
|--------|------|-------------|--------|-----------|
| m22_synthex_bridge | `src/m5_bridges/m22_synthex_bridge.rs` | SYNTHEX bridge (:8090) — thermal read + Hebbian writeback, cascade amplification | PV2 M22 (adapt) | unit + integration |
| m23_me_bridge | `src/m5_bridges/m23_me_bridge.rs` | ME bridge (:8080) — fitness signal read, frozen detection, observer API | PV2 M24 (adapt) | unit |
| m24_povm_bridge | `src/m5_bridges/m24_povm_bridge.rs` | POVM bridge (:8125) — memory hydration + crystallisation. **Write-only (BUG-034)** — must call `/hydrate` to read | PV2 M25 (adapt) | unit |
| m25_rm_bridge | `src/m5_bridges/m25_rm_bridge.rs` | RM bridge (:8130) — cross-session **TSV** persistence (NOT JSON!), content sanitisation | PV2 M26 (adapt) | unit + integration |
| m26_blackboard | `src/m5_bridges/m26_blackboard.rs` | SQLite shared fleet state — pane status, task history, agent cards | NEW | unit |

## Service Endpoints

| Bridge | Port | Health | Key Endpoints |
|--------|------|--------|---------------|
| SYNTHEX | 8090 | `/api/health` | `/v3/thermal`, `/v3/diagnostics`, `POST /api/ingest` |
| ME | 8080 | `/api/health` | `/api/observer` (fitness, correlations) |
| POVM | 8125 | `/health` | `/memories`, `/pathways`, `/hydrate`, `/consolidate` |
| RM | 8130 | `/health` | `POST /put` (TSV!), `GET /search?q=`, `GET /entries` |

## Critical Rules

- **RM is TSV only** — JSON causes parse failure (AP05)
- **POVM is write-only** — must call `/hydrate` to read back (BUG-034)
- **Bridge URLs: raw `SocketAddr`** — no `http://` prefix (BUG-033)
- All bridges include `_consent_check()` stub (P21)
- SYNTHEX cascade amplification: use `amp` directly, not `1.0/amp` (Session 012)

## Design Constraints

- Fire-and-forget: `tokio::spawn`, raw TCP HTTP, no hyper overhead
- Timeout per bridge request: 5s (configurable)
- Bridge failures emit events — do not retry silently
- Retry policy: exponential backoff, max 3 attempts, jitter
- Blackboard uses `PRAGMA journal_mode=WAL` (P13)

## Hot-Swap Source

- m22-m25: ADAPT from PV2 (candidate-modules/adapt/L5-*/) — see `## ADAPT for ORAC` headers
- m26: NEW

## Cross-References

- [[Synthex (The brain of the developer environment)]]
- [[The Maintenance Engine V2]]
- [[POVM Engine]]
- [[ULTRAPLATE — Bugs and Known Issues]] (BUG-033, BUG-034)
- `.claude/queries/blackboard.sql`
- `.claude/skills/bridge-probe/SKILL.md`
- ORAC_PLAN.md §Phase 3 Detail
- ORAC_MINDMAP.md §Branch 6 (Bridge Subset)
