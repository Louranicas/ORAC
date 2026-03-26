# ADR Source Sessions — Architectural Decision Traceability

> **Generated:** 2026-03-25 | **Scope:** 15 ORAC architectural decisions traced to earliest Obsidian note
> **Method:** Full-text search across `~/projects/shared-context/` and `~/projects/claude_code/`

---

## Decision Index

| # | Decision | Session | Obsidian Note |
|---|----------|---------|---------------|
| 1 | Kuramoto oscillators | 028 | `[[Executor and Nested Kuramoto Bridge — Session 028]]` |
| 2 | Hebbian STDP | 045 | `[[Session 045 Arena — 10-hebbian-operational-topology]]` |
| 3 | 8-layer architecture | 050 | `[[Session 050 — ORAC Sidecar Architecture]]` |
| 4 | ureq blocking HTTP | 056 | `[[Session 056 — ORAC God-Tier Mastery]]` |
| 5 | SQLite blackboard | 050 | `[[Session 050 — ORAC Sidecar Architecture]]` |
| 6 | BusFrame enum | 019b | `[[Pane-Vortex IPC Bus — Session 019b]]` |
| 7 | Feature gates | 050 | `[[Session 050 — ORAC Sidecar Architecture]]` |
| 8 | Consent gates | 034d | `[[Session 034d — NA Consent Gate Implementation]]` |
| 9 | RALPH loop | pre-050 | `[[ME RALPH Loop Specification]]` |
| 10 | Unix socket IPC | 019b | `[[Pane-Vortex IPC Bus — Session 019b]]` |
| 11 | TSV for RM | 050+ | `[[ORAC Sidecar — Executive Summary and Module Guide]]` |
| 12 | parking_lot | 040 | `[[PV2 Scaffolding Workflow]]` (patterns/) |
| 13 | Axum | 050 | `[[Session 050 — ORAC Sidecar Architecture]]` |
| 14 | Monolith design | 050 | `[[Session 050 — ORAC Sidecar Architecture]]` |
| 15 | Localhost security | 056+ | Deployment handoffs (implicit) |

---

## Decision Details

### 1. Kuramoto Oscillators for Fleet Coordination

**Session 028** (2026-03-15) — `Executor and Nested Kuramoto Bridge — Session 028.md`

Nested Kuramoto bridge synchronizes fleet pane phases via bidirectional field integration.
External NexusForge field modulates inner Pane-Vortex coupling via `k_modulation` multiplier
`[0.85, 1.1]` derived from outer coherence. Later validated by arxiv 2508.12314 (noted Session 050).

> NexusForge outer field → k_modulation → coupling → inner field r → feedback

### 2. Hebbian STDP for Learning

**Session 045** (2026-03-20) — `Session 045 Arena — 10-hebbian-operational-topology.md`
**Schematic:** Session 057 — `Session 057 — Hebbian Learning and Evolution Schematic.md`

Co-active panes (both Working) strengthen coupling (LTP = +0.01, 3x burst, 2x newcomer).
Active/idle pairs weaken coupling (LTD = -0.002). Weights clamped [0.15, 0.85].
Feeds semantic router at 35% of dispatch score. Persists to POVM every 60 ticks.

> "Panes that work together, couple together."

### 3. 8-Layer Architecture

**Session 050** (2026-03-22) — `Session 050 — ORAC Sidecar Architecture.md`

Emerged from hot-swap feasibility analysis of PV2's 41 modules. Each layer has single
responsibility. Lower layers cannot import from higher layers (compile-time enforced DAG).
Structure mirrors cognitive stack: foundation → learning → evolution.

> "If you try to import upward, the code won't build."

### 4. ureq Blocking HTTP

**Session 056** (2026-03-23) — `Session 056 — ORAC God-Tier Mastery.md`

Blocking HTTP via `ureq` in `spawn_blocking` chosen over async reqwest/hyper. Raw TCP sockets
for minimal overhead. Fire-and-forget bridges don't block the hook path. Sub-millisecond
hook response. Shared `http_helpers` module handles chunked encoding and growing buffers
(4KB initial, up to 2MB for POVM responses).

> All bridges fire-and-forget (ureq in spawn_blocking). Sub-millisecond hook response.

### 5. SQLite Blackboard

**Session 050** (2026-03-22) — `Session 050 — ORAC Sidecar Architecture.md`

SQLite for fleet shared state. Durable persistence across ORAC restarts. 10 tables
(pane_status, task_history, agent_cards, ghost_traces, consent_declarations, hebbian_summary,
ralph_state, sessions, coupling_weights, consent_audit). WAL mode for durability with
concurrent reads. 3-retry backoff (100/200/400ms). In-memory mode for tests.

> "The fleet's shared whiteboard — who is where, what happened, what was decided."

### 6. BusFrame Enum

**Session 019b** (2026-03-12) — `Pane-Vortex IPC Bus — Session 019b.md`

Single `BusFrame` enum with 11 variants covering the entire IPC protocol (Handshake, Welcome,
Subscribe, Subscribed, Submit, TaskSubmitted, Event, Cascade, CascadeAck, Disconnect, Error).
NDJSON format chosen for shell-debuggability (`socat`/`nc -U` can inspect). 65KB max line
matches existing body limit. Confirmed in Session 062 adversarial synthesis.

> "Shell-debuggable (`socat` or `nc -U` can inspect), serde already available."

### 7. Feature Gates

**Session 050** (2026-03-22) — `Session 050 — ORAC Sidecar Architecture.md`
**Clarification:** `shared-context/tasks/fix-017-default-features.md`

Feature gates originally for phased build order (Phase 4 couldn't compile until Phases 1-3
existed). `intelligence` gates Hebbian/router/breaker (tiny `tower` dep). `evolution` gates
RALPH (zero external deps — pure cfg gate). `monitoring` gates OTel (genuinely heavy/optional).
Fix-017 changed defaults to include all 6 features after build completion.

> "The rationale was about build order during development, not runtime optionality."

### 8. Consent Gates

**Session 034d** (2026-03-15) — `Session 034d — NA Consent Gate Implementation.md`

External `k_modulation` from SYNTHEX/Nexus bridges now passes through fleet-consent filter.
Prevents external override of focused/divergent fleets. Fleet-wide receptivity scaling,
per-sphere opt-out flag, newcomer damping (80% reduction at 100% newcomers), divergence
exemption (receptivity < 0.15 suppresses positive boosts).

> "Before: raw multiplier applied uniformly. After: consent-gated, respecting sphere autonomy."

### 9. RALPH Evolution Loop

**Pre-Session 050** — `ME RALPH Loop Specification.md` (Obsidian vault, undated)
**Adopted for ORAC:** Session 050 (2026-03-22)

5-phase cyclic meta-learning: Recognize (compare tensor vs targets), Analyze (rank mutation
candidates), Learn (extract patterns from accept/rollback history), Propose (bounded mutations,
delta ≤ 0.20, max 3 concurrent), Harvest (evaluate, accept beneficial, rollback harmful).
Atomic snapshot/rollback. BUG-035 fix: diversity-enforced multi-parameter selection.

> "Autonomous parameter tuning — the system observes, proposes, tests, and keeps improvements."

### 10. Unix Socket IPC

**Session 019b** (2026-03-12) — `Pane-Vortex IPC Bus — Session 019b.md`

All Claude Code instances run on same machine (Zellij panes) — Unix socket is faster than TCP.
Replaces fire-and-forget text injection (`zellij action write-chars`) with structured NDJSON
messaging. Bus socket separate from HTTP for persistent bidirectional connections. Server can
broadcast to all subscribers simultaneously.

> "Addresses the core limitation: fire-and-forget text injection provides no structured
> response mechanism."

### 11. TSV for Reasoning Memory

**Session 050+** — `ORAC Sidecar — Executive Summary and Module Guide.md`

RM service parser rejects JSON — TSV is the only accepted format. Format:
`category\tagent\tconfidence\tttl\tcontent`. Zero-alloc single-pass sanitisation removes
tabs and newlines. Agent name is `"orac-sidecar"`. Simple line-delimited records avoid
JSON overhead.

> "Reasoning Memory is TSV, not JSON — the parser rejects it."

### 12. parking_lot RwLock

**Session 040** (PV2 scaffolding era) — `PV2 Scaffolding Workflow.md` (patterns/)

Gold-standard pattern adopted from Maintenance Engine V2. parking_lot avoids lock poisoning
where std::sync::RwLock becomes permanently poisoned after a panic. More robust for
long-running daemons. All shared state uses `parking_lot::RwLock` with interior mutability.

> "Read ME v2 gold standard — extract: `parking_lot::RwLock`, trait-first design, builder pattern."

### 13. Axum HTTP Framework

**Session 050** (2026-03-22) — `Session 050 — ORAC Sidecar Architecture.md`

Tokio-based async HTTP framework. Minimal overhead for sub-millisecond hook response times
required by Claude Code's hook pipeline. Feature-gated under `api`. Version 0.8 with JSON
support. tower-http for CORS and trace middleware.

### 14. Monolith Design

**Session 050** (2026-03-22) — `Session 050 — ORAC Sidecar Architecture.md`

Single binary (port 8133) rather than distributed microservices. V1 swarm-sidecar (546 LOC)
ran for 17 hours contributing nothing due to V1/V2 wire mismatch. Decision driven by need
for unified state management (coupling network, STDP, field state), sub-millisecond hook
responses (centralized memory), atomic RALPH evolution cycles, and centralized permission
policy.

> "An Envoy-like proxy specialized for AI agent traffic — not a proxy and not an orchestrator.
> A coordination substrate that learns."

### 15. Localhost Security

**Session 056+** — Deployment handoffs (implicit in operational practice)

ORAC binds to `127.0.0.1` only. All services (SYNTHEX :8090, ME :8080, POVM :8125, RM :8130,
PV2 :8132) are also localhost-bound. No authentication layer required — trusted local traffic
model. Single-machine developer environment with no network isolation boundaries.

No explicit rationale note exists — this is an inherited convention from the ULTRAPLATE
developer environment where all 17 services bind to localhost.

---

## Provenance Map

```
Session 019b ──── BusFrame, Unix socket IPC
Session 028  ──── Kuramoto oscillators
Session 034d ──── Consent gates
Session 040  ──── parking_lot RwLock
Session 045  ──── Hebbian STDP
Session 050  ──── 8-layer, SQLite blackboard, feature gates, Axum, monolith, RALPH (adopted)
Session 056  ──── ureq blocking, localhost security (implicit)
Pre-050      ──── RALPH (ME specification), TSV for RM (RM service constraint)
```

5 of 15 decisions originate in Session 050 (ORAC's founding architecture session).
2 originate in Session 019b (PV2 IPC bus design, inherited by ORAC).
The remaining 8 trace to individual sessions spanning 028–056.
