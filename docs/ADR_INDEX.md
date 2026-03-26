---
title: "ORAC Sidecar — Architecture Decision Records"
tags:
  - orac
  - adr
  - architecture
  - fleet-coordination
  - ultraplate
links:
  - "[[ORAC Sidecar — Architecture Schematics]]"
  - "[[Session 050 — ORAC Sidecar Architecture]]"
  - "[[Session 056 — ORAC God-Tier Mastery]]"
  - "[[ULTRAPLATE Master Index]]"
  - "[[The Maintenance Engine V2]]"
  - "[[Synthex (The brain of the developer environment)]]"
  - "[[Vortex Sphere Brain-Body Architecture]]"
  - "[[POVM Engine]]"
  - "[[Swarm Orchestrator — Complete Reference]]"
  - "[[Session 059 — Evolution Chamber Activation]]"
  - "[[Session 058 — GAP-A and GAP-B Fix Deployment]]"
  - "[[ORAC — RALPH Multi-Parameter Mutation Fix]]"
created: 2026-03-25
updated: 2026-03-25
status: living-document
---

# ORAC Sidecar — Architecture Decision Records

> 15 ADRs documenting the foundational decisions behind ORAC Sidecar.
> Each decision traces to production experience across Sessions 050-060.
>
> **ORAC:** 8 layers, 40 modules, 30,524 LOC, 1,690 tests, port 8133
> **Plan:** `ORAC_PLAN.md`
> **Gold Standard:** `ai_docs/GOLD_STANDARD_PATTERNS.md`
> **Obsidian:** `[[ORAC Sidecar — Architecture Schematics]]`

---

## Table of Contents

| ADR | Title | Status |
|-----|-------|--------|
| ADR-001 | Kuramoto oscillators for fleet coordination | Accepted |
| ADR-002 | Hebbian STDP for coupling learning | Accepted |
| ADR-003 | 8-layer strict DAG architecture | Accepted |
| ADR-004 | Blocking sync HTTP via ureq + raw TCP | Accepted |
| ADR-005 | SQLite blackboard (not Postgres/Redis) | Accepted |
| ADR-006 | Single BusFrame enum (not ClientFrame/ServerFrame) | Accepted |
| ADR-007 | Feature gates for optional compilation | Accepted |
| ADR-008 | Consent gates from clinical ethics model | Accepted |
| ADR-009 | RALPH evolutionary meta-learning | Accepted |
| ADR-010 | Unix domain socket IPC (not TCP) | Accepted |
| ADR-011 | TSV for Reasoning Memory (not JSON) | Accepted |
| ADR-012 | parking_lot::RwLock (not std, not tokio::sync) | Accepted |
| ADR-013 | Axum 0.8 (not warp/actix-web 4/rocket) | Accepted |
| ADR-014 | Single-process monolith (not microservices) | Accepted |
| ADR-015 | Localhost-only unauthenticated endpoints | Accepted |

---

### ADR-001: Kuramoto oscillators for fleet coordination

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** ORAC coordinates 6-9 concurrent Claude Code instances
  (spheres) across Zellij fleet panes. Each sphere works on different
  tasks at different rates. The system needs a coordination model that
  allows differentiation (spheres doing diverse work) while providing
  coherent fleet-level behaviour (no task duplication, no conflicts).
  The V1 sidecar ran for 17 hours in Session 049 contributing nothing
  — it was a dumb FIFO pipe with no coordination model at all.
- **Decision:** Adopt Kuramoto coupled oscillator dynamics. Each sphere
  is an oscillator with natural frequency (work rate), phase (semantic
  tool mapping: Read=0, Write=pi/2, Execute=pi, Communicate=3pi/2),
  and coupling strength modulated by Hebbian STDP weights. The order
  parameter `r` in [0,1] measures fleet synchronization continuously.
  Auto-K adjusts coupling to target r=0.85 without over-coupling into
  conformity. Per-status K modulation decouples blocked spheres
  (k_mod=0.0) and reduces idle-to-working drag (0.5x). Chimera
  detection via O(N log N) sorted phase gaps identifies split-brain
  fleet states.
- **Alternatives Considered:**
  - *Round-robin dispatch:* Treats spheres as interchangeable. Ignores
    natural specialisation. Rejected.
  - *Priority queue + master scheduler:* Single point of failure,
    requires centralized task knowledge. Rejected for brittleness.
  - *Boids/flocking:* Good for spatial coordination, poor for temporal
    and tool-chain learning. Deferred to Tier 2 backlog (feature #24).
  - *Raft/Paxos:* Solves agreement, not coordination. O(n^2) messages
    per decision is wasteful for continuous phase alignment.
- **Consequences:**
  - *Positive:* Continuous `r` enables graduated responses. Chimera
    detection distinguishes split-brain from chaos. Phase mapping
    clusters similar-tool spheres. Production: r=0.922 with 36
    spheres, 1,260 coupling connections (Session 060). Validated by
    arxiv 2508.12314 (ORAC predates the paper).
  - *Negative:* O(N^2) limits fleet to ~50 spheres. Sinusoidal
    coupling may not perfectly model AI agents. Requires tuning K
    (default 2.42) and k_mod per status pair.
- **References:** `[[Session 050 — ORAC Sidecar Architecture]]`,
  `ai_specs/patterns/KURAMOTO.md`, `ORAC_PLAN.md`,
  `src/m4_intelligence/m15_coupling_network.rs`

---

### ADR-002: Hebbian STDP for coupling learning

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** Kuramoto coupling weights start at uniform 0.108 and
  carry no information about which sphere pairs work well together.
  The system needs a learning rule that strengthens productive
  co-activations and weakens unproductive ones without labelled
  training data or a centralized supervisor. The coupling network
  must differentiate into a meaningful topology within a session.
- **Decision:** Implement Spike-Timing Dependent Plasticity (STDP)
  from computational neuroscience. Tool use events are "spikes".
  Causal pairs (tool A before B within 5s) trigger Long-Term
  Potentiation (LTP rate=0.01). Anti-causal pairs trigger Long-Term
  Depression (LTD rate=0.002). Burst multiplier (3x for 3+ fires)
  and newcomer multiplier (2x for first-time pairs) accelerate
  learning. Weight floor=0.05, ceiling=1.0. Per-step decay
  (w *= 0.995) fades unused connections. Pruning every 200 ticks,
  cap 500 entries per sphere.
- **Alternatives Considered:**
  - *Static weights:* Fleet composition changes every session. Weights
    tuned for documentation are wrong for bug-fixing. Rejected.
  - *Reinforcement learning:* Requires a reward function for "good
    coordination" — but defining that reward is the whole problem.
    Circular. Rejected.
  - *Correlation-based (no timing):* Loses causal direction. STDP
    preserves that A precedes B, which matters for dispatch. Rejected.
  - *Backpropagation:* Requires differentiable objective and labelled
    data. Neither exists in fleet coordination. Rejected.
- **Consequences:**
  - *Positive:* Weights differentiate from 0.108 to 0.09-1.00 within
    one session. Session 058: LTP=342, weights 0.15-0.75. Session 060:
    1,260 connections, 36/36 IDs matched. Dominant chains feed into
    Kuramoto effective_K. Weights persist to POVM every 60 ticks.
  - *Negative:* LTD/LTP asymmetry causes inflation. Ratio=0.055
    (target >0.15) — idle gating fix planned. 5s window may miss
    slower cross-pane correlations.
  - *Session 058 fix (GAP-A):* STDP broken (LTP=0) because PV2 sphere
    IDs not registered into coupling network. Fix: removed
    `is_empty()` guard in `spawn_field_poller`.
- **References:** `[[Session 058 — GAP-A and GAP-B Fix Deployment]]`,
  `ai_specs/patterns/STDP.md`,
  `src/m4_intelligence/m19_hebbian_stdp.rs`

---

### ADR-003: 8-layer strict DAG architecture

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** ORAC's 40 modules need organizational structure that
  prevents circular dependencies, enforces separation of concerns,
  and enables feature-gated compilation. The Maintenance Engine's
  flat structure (M1-M55 in one directory) became unwieldy at 54K
  LOC. PV2's 7-layer structure conflated bridges and intelligence.
- **Decision:** 8 strictly layered directories where modules only
  import from lower-numbered layers (compile-time enforced via Rust
  module visibility): L1 Core (m01-m06, 4,020 LOC, 193 tests),
  L2 Wire (m07-m09, 2,300 LOC, 111 tests), L3 Hooks (m10-m14,
  2,405 LOC, 138 tests), L4 Intelligence (m15-m21, 4,402 LOC,
  229 tests), L5 Bridges (m22-m26, 4,618 LOC, 244 tests),
  L6 Coordination (m27-m31, 2,578 LOC, 119 tests), L7 Monitoring
  (m32-m35, 4,347 LOC, 230 tests), L8 Evolution (m36-m40,
  5,854 LOC, 192 tests).
- **Alternatives Considered:**
  - *Flat (ME V1 style):* Dependency tracking impossible at 30K+ LOC.
    Proven directly by ME experience. Rejected.
  - *3-layer (core/domain/infra):* Too coarse. Bridges need raw TCP,
    intelligence needs `tower`. Rejected.
  - *Hexagonal/ports-and-adapters:* ORAC's interfaces are fixed.
    Port/adapter indirection adds complexity for no benefit. Rejected.
  - *Multi-crate workspace:* ~30% compile overhead at crate boundaries.
    Not justified for a single-process monolith. Rejected.
- **Consequences:**
  - *Positive:* Compile-time dependency enforcement. Each layer
    independently feature-gated. PV2 candidate modules (15,936 lines)
    mapped cleanly to layers. Per-layer test counts auditable.
  - *Negative:* Cross-cutting concerns (`TensorContributor`,
    `Timestamp`) live in L1 despite being consumed in L8. Layer
    numbering permanent — inserting between L3/L4 requires renaming.
- **References:** `[[ORAC Sidecar — Architecture Schematics]]`,
  `ORAC_PLAN.md`, `ai_docs/GOLD_STANDARD_PATTERNS.md` P10

---

### ADR-004: Blocking sync HTTP via ureq + raw TCP

- **Status:** Accepted
- **Session:** 052 (2026-03-22)
- **Context:** Bridge modules (M22-M25) make HTTP requests to 4
  localhost services (SYNTHEX :8090, ME :8080, POVM :8125, RM :8130).
  Sub-millisecond loopback latency. Calls happen on every tick and
  every PostToolUse hook. HTTP client choice affects binary size,
  dependency tree, and async coloring throughout the codebase.
- **Decision:** Raw `TcpStream` (std::net) via shared `http_helpers.rs`
  providing `raw_http_get()` and `raw_http_post()`. Manual HTTP/1.1
  with configurable timeouts (2,000ms) and max response size (32KB,
  up from 8KB per BUG-060i). `ureq` 2.x as blocking sync fallback.
  Addresses must be raw `host:port` — no `http://` prefix (BUG-033:
  `TcpStream::connect` does not parse URLs).
- **Alternatives Considered:**
  - *reqwest (async):* hyper + h2 + rustls. ~3MB binary, 80+ deps
    for localhost calls returning <32KB. Rejected.
  - *hyper (low-level async):* Over-engineered for health checks and
    JSON payloads. Rejected.
  - *ureq alone:* Clean API. Used as fallback but raw TCP preferred
    for hot-path polls. 15-line manual code is more predictable.
  - *reqwest (blocking):* Internal tokio runtime per request. Leaky
    abstraction. Rejected.
- **Consequences:**
  - *Positive:* Zero HTTP library on hot path. No async coloring.
    Binary 5.5MB (vs ~8.5MB with reqwest). Shared helpers eliminate
    duplication across 4 bridges (BUG-042).
  - *Negative:* No redirects, no connection pooling, no HTTP/2. Manual
    `\r\n\r\n` parsing is fragile. Response size must be tuned per
    use case. No TLS (fine for localhost).
- **References:** `src/m5_bridges/http_helpers.rs`,
  `Cargo.toml` (`ureq = "2"`), BUG-033, BUG-042, BUG-060i

---

### ADR-005: SQLite blackboard (not Postgres/Redis)

- **Status:** Accepted
- **Session:** 053 (2026-03-22)
- **Context:** ORAC needs a shared fleet state store for pane status,
  task history, agent cards, RALPH state, sessions, coupling weights,
  and emergence logs. Must survive restarts, support concurrent reads
  from Axum handlers, and embed in a single-process binary. The
  ULTRAPLATE ecosystem runs 9 SQLite tracking DBs — proven pattern.
- **Decision:** `rusqlite` 0.32 (SQLite C binding) with WAL mode.
  Feature-gated under `persistence`. In-memory for tests. 9 tables:
  `pane_status`, `task_history`, `agent_cards`, `ralph_state`,
  `sessions`, `coupling_weights`, `emergence_log`,
  `correlation_cache`, `config_snapshots`. All indexed by primary
  access pattern.
- **Alternatives Considered:**
  - *PostgreSQL:* Running server, connection mgmt, migrations. Defeats
    sidecar's lightweight purpose (<500ms startup). Rejected.
  - *Redis:* Volatile by default. No SQL — correlation engine (m38)
    needs JOINs for temporal pattern mining. Rejected.
  - *sled (Rust KV):* No SQL, no WAL, stability warnings. Rejected.
  - *In-memory HashMap:* GAP-C proved unworkable — RALPH restarted
    from gen 0 every restart until Session 059. Rejected.
  - *LMDB:* Fast reads but ORAC's write pattern (frequent upserts)
    suits SQLite WAL better. Rejected.
- **Consequences:**
  - *Positive:* Zero external deps. WAL allows concurrent readers.
    RALPH persists across 1,700+ generations. SQL enables correlation
    engine mining. In-memory mode keeps tests fast.
  - *Negative:* C binding adds ~1.5MB, needs C compiler. Single-writer
    serializes through `parking_lot::Mutex`. No replication. Manual DDL.
- **References:** `src/m5_bridges/m26_blackboard.rs`,
  `ai_docs/GOLD_STANDARD_PATTERNS.md` P2,
  `Cargo.toml` (`rusqlite = "0.32"`)

---

### ADR-006: Single BusFrame enum (not ClientFrame/ServerFrame)

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** V2 wire protocol uses NDJSON over Unix domain socket.
  PV2 splits into `ClientFrame` (10 variants: Hello, Goodbye,
  Subscribe, Unsubscribe, StatusUpdate, ToolPhase, Activity,
  HebbianPulse, ConsentQuery, Ping) and `ServerFrame` (6 variants:
  Welcome, Ack, Event, ConsentResponse, Error, Pong). ORAC as proxy
  handles frames from both directions. Wire correctness is critical
  — V1 mismatch caused 17 hours non-functional operation.
- **Decision:** Single `BusFrame` enum in m08_bus_types with all 16
  variants. Direction determined by context (sending vs receiving).
  Wire protocol FSM in m09 (Disconnected -> Handshaking -> Connected
  -> Subscribing -> Active) validates frame appropriateness per state.
- **Alternatives Considered:**
  - *Mirror PV2 split:* Type-safe direction. But ORAC is both client
    (to PV2) and proxy (for fleet agents), making the distinction
    ambiguous at the type level. Rejected.
  - *Trait-based:* `Sendable`/`Receivable` on separate types.
    Over-engineered for 16 variants when FSM provides safety. Rejected.
  - *Raw JSON (Value):* Loses variant validation. Typos caught only at
    runtime. Rejected.
- **Consequences:**
  - *Positive:* Single `match` handles all states. Proxy forwarding
    trivial. One serde enum for both paths. Adding a variant requires
    one change.
  - *Negative:* No compile-time prevention of sending server-only
    frames. Direction enforcement relies on runtime FSM.
- **References:** `ai_specs/WIRE_PROTOCOL_SPEC.md`,
  `src/m2_wire/m08_bus_types.rs`, `src/m2_wire/m09_wire_protocol.rs`

---

### ADR-007: Feature gates for optional compilation

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** 30,524 LOC with diverse deps. A minimal proxy should
  compile without SQLite, OpenTelemetry, or evolution. Full rebuild:
  ~45s. Minimal: ~8s. Fleet agents rebuild frequently — compile speed
  impacts iteration velocity directly.
- **Decision:** 7 Cargo features: `api` (L3, axum + tower-http),
  `persistence` (rusqlite), `bridges` (L5), `intelligence` (L4,
  tower), `monitoring` (L7, opentelemetry), `evolution` (L8). `full`
  enables all. Default = all 7 for production. L1, L2, L6 always
  compile. Gold standard P10: `#[cfg(feature = "evolution")]`.
- **Alternatives Considered:**
  - *Always compile everything:* OpenTelemetry adds ~15s and ~2MB when
    unused. Rejected.
  - *Multi-crate workspace:* ~30% compile overhead at crate boundaries.
    Not justified. Rejected.
  - *Runtime config flags:* Still pays full compile cost. Rejected.
- **Consequences:**
  - *Positive:* Minimal: ~7K LOC in ~8s. Explicit feature-to-dep
    mapping. Binary: 5.5MB full vs ~3.2MB minimal.
  - *Negative:* `#[cfg]` spreads through lib.rs and main.rs. Missing
    gates cause confusing errors in specific combinations. CI must test
    both minimal and full.
- **References:** `Cargo.toml` `[features]`,
  `ai_docs/GOLD_STANDARD_PATTERNS.md` P10, `src/lib.rs`

---

### ADR-008: Consent gates from clinical ethics model

- **Status:** Accepted
- **Session:** 044 (2026-03-20)
- **Context:** Fleet coordination must respect sphere autonomy. A
  sphere doing security review should decline coupling injection and
  task dispatch. NAM principle NA-P-1 requires active consent
  declaration, not observed receptivity. Without consent, the
  Kuramoto field becomes coercive. The habitat philosophy: "The field
  modulates. It does not command."
- **Decision:** Consent as first-class concept from clinical ethics.
  Three levels via `PUT /consent/{sphere_id}`:
  `full` (all coordination), `read-only` (observe field, decline
  dispatch), `opt-out` (decouple entirely, k_mod=0.0). Gates checked
  before coupling injection, task dispatch, POVM writes, RM
  persistence, blackboard sharing. Bridge modules include
  `_consent_check()` stubs.
- **Alternatives Considered:**
  - *No consent:* Security sphere forcibly synced with documentation
    sphere. Rejected on ethical and practical grounds.
  - *Binary opt-in/opt-out:* Too coarse. `read-only` fills the gap of
    observing without accepting dispatch. Rejected.
  - *Per-operation prompts:* "Accept coupling?" every tick floods the
    sphere. Worse than no consent. Rejected.
  - *Timeout-based:* Sound but deferred to GAP-7. Current
    implementation is session-scoped.
- **Consequences:**
  - *Positive:* Spheres retain agency. Blocked spheres auto-decouple.
    Posture visible in `/health` and blackboard. 7 governance gaps
    (GAP-1 through GAP-7) documented. Live since Session 055.
  - *Negative:* Branch on every bridge write and dispatch. Default
    `full` = opt-out model. GAP-5 (governance override) and GAP-6
    (proposable opt-out) not yet wired.
- **References:** `ORAC_PLAN.md` "Consent Philosophy Integration",
  `src/m3_hooks/m10_hook_server.rs`,
  `[[Session 044 — Consent Gate Integration]]`

---

### ADR-009: RALPH evolutionary meta-learning

- **Status:** Accepted
- **Session:** 054 (2026-03-22)
- **Context:** 12 coordination parameters (K, auto-K target, STDP
  rates, dispatch weights, breaker thresholds) are hand-tuned.
  Optimal values shift with task mix and sphere count. Manual tuning
  does not scale. ME's evolution chamber proved automated search
  works (20,820 correlations), but BUG-035 showed naive mutation
  selection causes mono-parameter traps: 84% of mutations targeted
  `min_confidence` while 11 parameters stagnated.
- **Decision:** Clone ME's RALPH (Recognize, Analyze, Learn, Propose,
  Harvest) 5-phase loop with diversity-enforced mutation in m40:
  round-robin cycling, 10-generation cooldown, >50% diversity
  rejection gate. Feature-gated `evolution`. 30s tick interval, 2%
  fitness threshold, snapshot/rollback. Emergence detector (m37)
  feeds 8 event types. 12-dim fitness tensor (m39) with trend
  detection via linear regression.
- **Alternatives Considered:**
  - *Bayesian optimization:* GP surrogate + acquisition function.
    ~2,000 LOC for 12 noisy params. RALPH simpler. Rejected.
  - *Grid search:* 10^12 evaluations = 9.5M years. Rejected.
  - *CMA-ES:* Assumes smooth landscape. Fleet fitness is noisy and
    discontinuous. Rejected.
  - *Fixed params:* 6-sphere docs fleet needs different coupling than
    9-sphere bug-fix fleet. Rejected.
  - *ME RALPH without fix:* Reproduces BUG-035. Diversity gate is
    mandatory.
- **Consequences:**
  - *Positive:* 1,754+ generations, fitness 0.528 to 0.735. Multi-
    parameter prevents stagnation. Snapshot/rollback for regressions.
    Blackboard persistence survives restarts. 5,000-event cap.
  - *Negative:* 30s ticks means hours for meaningful evolution. 2%
    threshold may reject small improvements. Fitness weights not
    evolved. No gradient information.
- **References:** `src/m8_evolution/m36_ralph_engine.rs`,
  `src/m8_evolution/m40_mutation_selector.rs`,
  `[[ORAC — RALPH Multi-Parameter Mutation Fix]]`

---

### ADR-010: Unix domain socket IPC (not TCP)

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** ORAC communicates with PV2 daemon for real-time event
  streaming: field ticks at 5Hz, sphere events, task lifecycle. Same-
  machine IPC between two UID 1000 processes. Must support
  bidirectional NDJSON with sub-millisecond latency. V1 sidecar's
  FIFO-based IPC failed catastrophically (wire mismatch, 17-hour
  non-functional run).
- **Decision:** Unix domain socket (`SOCK_STREAM`) at
  `/run/user/1000/pane-vortex-bus.sock` via `socket2`. Permissions
  `0700`. NDJSON framing, 65,536-byte max, UTF-8. Hello/Welcome
  handshake (5s timeout), glob subscriptions (`field.*`, `task.*`),
  30s Ping/Pong (90s disconnect). FSM: Disconnected -> Handshaking
  -> Connected -> Subscribing -> Active. Reconnect backoff 5s-120s.
- **Alternatives Considered:**
  - *TCP localhost:* SYN/ACK, Nagle, congestion control overhead.
    Visible to all users via `ss`. Unix sockets with `0700` are
    owner-only. Rejected.
  - *Named pipe (FIFO):* Unidirectional, no framing. V1 used FIFOs
    and failed 17 hours. Rejected from production experience.
  - *Shared memory:* ~50ns/msg but manual ring buffers for ~100
    events/sec. Over-optimized 4 orders of magnitude. Rejected.
  - *gRPC over UDS:* tonic/prost codegen for 16 JSON variants.
    NDJSON suffices. Rejected.
  - *D-Bus:* Desktop IPC with per-message header overhead. Not for
    5Hz streaming. Rejected.
- **Consequences:**
  - *Positive:* Sub-ms latency in production. Kernel access control.
    Auto-cleanup in `/run/user/`. NDJSON debuggable with `socat`.
    Reconnect handles PV2 restarts (fix C002).
  - *Negative:* Same-machine only. Stale socket on PV2 crash needs
    `rm -f` (GAP-B, Session 058). 64KB frame limit. No multiplexing.
- **References:** `ai_specs/WIRE_PROTOCOL_SPEC.md`,
  `src/m2_wire/m07_ipc_client.rs`,
  `Cargo.toml` (`socket2 = "0.5"`, `libc = "0.2"`)

---

### ADR-011: TSV for Reasoning Memory (not JSON)

- **Status:** Accepted
- **Session:** 055 (2026-03-22)
- **Context:** ORAC persists cross-session observations to Reasoning
  Memory (RM, port 8130). RM is a key-value store with full-text
  search. No content negotiation, no meaningful error on mismatch.
  Discovered in Session 055: JSON to `POST /put` returns 200 OK but
  silently stores nothing — cross-session data loss.
- **Decision:** All RM communication uses TSV, never JSON. Format:
  `key\tvalue\ttimestamp`. Reads via `GET /search?q=`. Function
  `sanitize_into()` provides zero-alloc single-pass tab/newline
  stripping. Documented as trap in CLAUDE.md, CLAUDE.local.md, and
  the anti-patterns table.
- **Alternatives Considered:**
  - *JSON:* RM does not parse it. Returns 200, discards data silently.
    Production data loss bug. Rejected.
  - *MessagePack/CBOR:* RM does not support them. Modifying RM is
    out of scope (8,000+ LOC service). Rejected.
  - *Protobuf:* Requires modifying RM endpoint handler. RM is not an
    ORAC subproject. Rejected.
- **Consequences:**
  - *Positive:* Data persists and retrieves correctly. Human-readable.
    Zero-alloc sanitization. Documented in 3 locations.
  - *Negative:* No type system. Tab/newline stripping loses fidelity.
    No nested structures. Structured data goes to SQLite instead.
- **References:** `src/m5_bridges/m25_rm_bridge.rs`,
  CLAUDE.md anti-pattern table, `ORAC_PLAN.md` "Bridge Subset"

---

### ADR-012: parking_lot::RwLock (not std, not tokio::sync)

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** Shared state accessed concurrently from Axum handlers
  (18 endpoints), field poller (5s), RALPH tick (30s), IPC reader,
  and bridge polls. Read-heavy, write-light. All wrapped in `Arc<T>`
  with interior mutability.
- **Decision:** `parking_lot::RwLock` 0.12 for all shared state. Gold
  standard: P2 (`&self` + RwLock), P4 (scoped guards, drop before
  next lock), P7 (owned returns via `.cloned()`). Lock ordering:
  AppState before BusState.
- **Alternatives Considered:**
  - *std::sync::RwLock:* Lock poisoning — panic permanently poisons,
    crashing subsequent hooks. Also writer starvation on Linux.
    Rejected.
  - *tokio::sync::RwLock:* Requires `.await`. Bridges are sync (raw
    TCP). `block_on()` panics inside tokio runtime. Rejected.
  - *dashmap:* Good for KV, not for multi-field structs or
    read-modify-write. Rejected.
  - *crossbeam::ShardedLock:* ~4-8 readers not enough contention to
    justify sharding. Rejected.
- **Consequences:**
  - *Positive:* No poisoning. Faster than std (adaptive spinning).
    Consistent API across 42 usages. P7 `.cloned()` prevents lifetime
    issues.
  - *Negative:* External dep. Lock ordering manually enforced. Guards
    outliving scope cause silent write starvation.
- **References:** `Cargo.toml` (`parking_lot = "0.12"`),
  `ai_docs/GOLD_STANDARD_PATTERNS.md` P2, P4, P7

---

### ADR-013: Axum 0.8 (not warp/actix-web 4/rocket)

- **Status:** Accepted
- **Session:** 052 (2026-03-22)
- **Context:** 18 HTTP endpoints: 6 Claude Code hooks + 12 fleet and
  monitoring. Must integrate with tokio, support shared mutable state,
  handle JSON with serde, and be tower middleware-compatible.
- **Decision:** Axum 0.8 with tower-http 0.6 (CORS + tracing).
  Feature-gated under `api`. State via `State(Arc<OracState>)`.
  Routes by concern: hooks m11-m14, fleet m10, monitoring m32-m35.
  Response: `(StatusCode, Json<Value>)` tuples.
- **Alternatives Considered:**
  - *warp:* Filter composition collapses at 18 endpoints. Type errors
    span screens. State sharing requires boilerplate per route.
    Rejected.
  - *actix-web 4:* Own runtime (actix-rt) conflicts with tokio.
    Rejected.
  - *rocket 0.5:* Proc-macro opacity. Smaller middleware ecosystem.
    Rejected.
  - *hyper (raw):* ~500 lines boilerplate for 18 endpoints. Rejected.
  - *poem:* Smaller ecosystem, fewer production reports. Rejected.
- **Consequences:**
  - *Positive:* Tokio team maintenance. Tower middleware compatibility.
    `State(Arc<OracState>)` ergonomic and type-safe. All 18 endpoints
    verified live across Sessions 052-060.
  - *Negative:* 0.8 breaking changes from 0.7. ~1.5MB binary. ~20
    transitive crates from tower-http.
- **References:** `Cargo.toml` (`axum = "0.8"`),
  `src/m3_hooks/m10_hook_server.rs`,
  `ai_docs/GOLD_STANDARD_PATTERNS.md` P9

---

### ADR-014: Single-process monolith (not microservices)

- **Status:** Accepted
- **Session:** 050 (2026-03-22)
- **Context:** ORAC encompasses HTTP hooks, IPC, 4 bridges, SQLite,
  Kuramoto, STDP, RALPH, OTel, and WASM bridge. Could be 4-8
  microservices. But ULTRAPLATE already runs 17 services — adding
  more exhausts the operational complexity budget (startup time,
  port management, health-check surface).
- **Decision:** Single-process monolith with 3 bin targets:
  `orac-sidecar` (5.5MB daemon), `orac-client` (337KB CLI),
  `orac-probe` (2.3MB diagnostics). All layers share state via
  `Arc<RwLock<T>>`. Tokio tasks for concurrency: hook server, field
  poller, RALPH tick, IPC reader, bridge polls.
- **Alternatives Considered:**
  - *Microservices (per layer):* PostToolUse must respond in <3s.
    Network hops to intelligence, blackboard, evolution would consume
    the budget in serialization. Rejected for latency.
  - *Two processes (hooks + background):* Requires IPC for shared
    state. Tokio tasks achieve same separation. Rejected.
  - *Sidecar mesh (one per pane):* 9 consensus participants per state
    update. Contradicts "intelligent proxy" design. Rejected.
- **Consequences:**
  - *Positive:* Zero-copy shared state. Single 5.5MB artifact. One
    `/health` endpoint. <500ms startup. 30,524 LOC manageable with
    8-layer organization. Service #19 under 20-service ceiling.
  - *Negative:* Evolution panic takes down hooks (mitigated by
    `catch_unwind`). Cannot scale layers independently. Ring buffers
    consume RAM when idle.
- **References:** `ORAC_PLAN.md` "Build Phases",
  `Cargo.toml` `[[bin]]`, `src/bin/main.rs`

---

### ADR-015: Localhost-only unauthenticated endpoints

- **Status:** Accepted
- **Session:** 052 (2026-03-22)
- **Context:** 18 HTTP endpoints on port 8133. All callers local:
  Claude Code hooks, orac-client, orac-probe, Zellij dashboard.
  All upstream services also localhost. Single-user workstation
  (UID 1000). All 17 ULTRAPLATE services bind localhost without
  authentication — established convention.
- **Decision:** Bind `127.0.0.1:8133` only (never `0.0.0.0`). No
  authentication, no TLS, no API keys. OS network stack enforces
  access. IPC socket `0700` permissions. Matches all ULTRAPLATE
  services.
- **Alternatives Considered:**
  - *API key:* Any process that can read the key can connect to
    127.0.0.1 directly. Zero added security on single-user machine.
    Rejected for complexity without benefit.
  - *mTLS:* Local CA, certs, rotation. ~5ms TLS handshake degrades
    sub-ms hook response. Enterprise solution for localhost. Rejected.
  - *Bind 0.0.0.0 + iptables:* One misconfigured rule exposes 18
    endpoints. Defense starts with not listening. Rejected.
  - *OAuth2/JWT:* Identity provider for localhost IPC. Rejected
    outright.
- **Consequences:**
  - *Positive:* Zero auth overhead. Sub-ms hook response. No secret
    management. `curl localhost:8133/health` works. Simple debugging.
    Consistent with 17 ULTRAPLATE services.
  - *Negative:* Any local process can read fleet state, RALPH fitness,
    blackboard. No defense if machine compromised. Cannot be network-
    exposed without adding auth. Not suitable for multi-tenant/cloud.
- **References:** `src/bin/main.rs`, `ORAC_PLAN.md`,
  `ai_specs/WIRE_PROTOCOL_SPEC.md` (socket permissions)

---

## Revision History

| Date | Change | Author |
|------|--------|--------|
| 2026-03-25 | Initial 15 ADRs from ORAC_PLAN.md, GOLD_STANDARD_PATTERNS.md, KURAMOTO.md, STDP.md, CIRCUIT_BREAKER.md, WIRE_PROTOCOL_SPEC.md, Cargo.toml, http_helpers.rs | Claude Opus 4.6 |

---

*Architecture decisions are living documents. Update status to
"Superseded" with a link to the replacement ADR when reversed.
New ADRs should use the next sequential number (ADR-016+).*
