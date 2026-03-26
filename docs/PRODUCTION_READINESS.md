---
title: ORAC Sidecar — Production Readiness Assessment
date: 2026-03-25
status: ASSESSMENT
audience: Architecture reviewers, deployment leads
links:
  - "[[Session 062 — ORAC System Atlas (ACP)]]"
  - "[[ULTRAPLATE Master Index]]"
  - "[[ORAC Sidecar — Architecture Schematics]]"
  - "[[ULTRAPLATE — Bugs and Known Issues]]"
tags:
  - orac
  - production-readiness
  - security
  - architecture-review
---

# ORAC Sidecar — Production Readiness Assessment

> **30,524 LOC | 40 modules | 8 layers | 1,748 tests | 0 clippy warnings (pedantic)**
>
> Assessment date: 2026-03-25 | Source data: R1 Fleet Test Inventory, D6 Capacity Limits,
> D7 Verification Report, live runtime telemetry (Sessions 054-062)

---

## 1. What Works Reliably

These subsystems have been exercised under live fleet conditions across multiple sessions
with consistent, reproducible results.

### HTTP Hook Server (L3, m10-m14)

- 22 verified HTTP routes on `localhost:8133` serving 6 Claude Code hook events
- 1,748 total tests (1,696 unit + 52 integration), 0 clippy warnings at pedantic level
- `OracState` is `Arc`-wrapped with `parking_lot::RwLock`, shared safely across all handlers
- Fire-and-forget bridge calls via `tokio::spawn` keep hook latency sub-millisecond
- 6 production hooks migrated from bash: SessionStart, PostToolUse, PreToolUse,
  UserPromptSubmit, Stop, PermissionRequest
- Graceful degradation: cache fallback when upstream PV2 is unreachable (tested)

### Kuramoto Field (L1/L4)

- Sustained field coherence r=0.92 at 36-44 spheres across Sessions 058-060
- PI breathing controller (gain=0.15, blend=0.3) stabilizes r without overshoot
- Chimera detection at pi/3 gap threshold with r>0.95 auto-disable
- 8 FieldAction variants drive conductor decisions from real sphere data
- Phase wrapping via `.rem_euclid(TAU)` enforced at all arithmetic boundaries

### Hebbian STDP (L4, m18)

- LTP/LTD firing confirmed: LTP=6/tick, LTD=247/tick at 36 spheres (Session 060)
- Weight differentiation 0.15-1.00 (floor enforced, no disconnection)
- Burst multiplier (3.0x) and newcomer multiplier (2.0x, 50-tick window) operational
- Anti-saturation: HebbianSaturation emergence detector fires at 80% ceiling fraction

### RALPH Evolution (L8, m36-m40)

- 4,526+ generations completed with snapshot/rollback integrity
- 5-phase cycle (Recognize-Analyze-Learn-Propose-Harvest) verified via 15 integration tests
- Auto-pause at `max_cycles=1000`, pause/resume state machine tested
- BUG-035 fix active: round-robin parameter cycling + >50% diversity rejection gate
- Fitness trajectory: 0.432 -> 0.735 over Sessions 055-060

### SQLite Blackboard (L5, m26)

- 10 tables (pane_status, task_history, agent_cards, sessions, coupling_weights,
  consent_declarations, consent_audit, plus 3 index tables), WAL mode
- 90 unit tests (highest module count), session persistence verified across restarts
- In-memory mode for test isolation, FIFO ghost eviction at MAX_GHOSTS=20
- 200-ghost load test with prune-to-100 and newest-preserved verification

### Circuit Breakers (L4, m21)

- 6 breakers registered: pv2, synthex, me, povm, rm, vms
- Closed/Open/HalfOpen FSM with 5-failure trip, 2-success close, 30-tick timeout
- VMS uses tolerant config (10 fail / 3 success / 10 tick timeout)
- All 5 primary breakers at Closed state, 100% success rate at Session 060 end
- 45 unit tests covering full FSM transition matrix

---

## 2. What Is Scaffolded But Not Exercised

These subsystems are implemented and compile clean, but lack runtime validation or
have no active consumers.

### Integration Tests (8 of 13 files are scaffolds)

Only 3 of 8 layers have real integration tests (L3 Hooks: 26, L5 Bridges: 9,
L8 Evolution: 15). The remaining 5 layers plus cross-layer workflows, stress tests,
and property tests contain single placeholder functions. `TestHarness` in
`common/mod.rs` is defined but not implemented.

### WASM Bridge (L6, m30)

- 729 LOC implementation: FIFO pipe (`/tmp/swarm-commands.pipe`), ring file
  (`/tmp/swarm-events.jsonl`), 5 command types, 1,000-line ring cap
- 34 unit tests pass
- Zero consumers in production: the Zellij swarm-orchestrator WASM plugin reads
  the ring file but no ORAC code path currently writes to it during normal operation

### Semantic Dispatch (L4, m20)

- 803 LOC semantic router: 4 domains, Hebbian weights, domain affinity,
  tool/content classifiers, composite scoring
- 45 unit tests pass
- `dispatch_total` metric is often 0 at runtime because task claiming is infrequent;
  the router fires only when PostToolUse encounters a claimable task (1-in-5 polls)

### Feature Gates

- 6 features: `api`, `persistence`, `bridges`, `intelligence`, `monitoring`, `evolution`
- All 6 are default-enabled; `--features full` is equivalent to default
- No stripped build has been tested — unknown whether the binary operates correctly
  with any feature disabled. Feature boundaries may have leaked through `use` chains.

### Load and Stress Tests

- `tests/stress_test.rs`: scaffold placeholder only
- `src/bin/ralph_bench.rs`: benchmark binary exists but contains no benchmarks
- No sustained-load test has been run beyond the 200-ghost blackboard test

### OTel Export (L7, m32)

- 73 unit tests for span creation, trace storage, and export formatting
- No OpenTelemetry collector has been configured or connected
- Traces accumulate in-memory only; no verification of OTLP export under real load

---

## 3. What Is a Facade or Inert (Cross-Service)

These findings come from 9-pane fleet exploration (Session 060) and affect ORAC's
upstream data quality even though ORAC itself is functioning correctly.

### DevOps Engine (:8081)

- Accepts any JSON input on any endpoint without schema validation
- Internal metrics frozen: 40 agents report load=0.0, 0 tasks dispatched
- ORAC's ME bridge polls this service, but receives stale data

### VMS Consolidation (:8120)

- `morphogenic_cycle=0` despite ORAC posting memories every 30 ticks
- The `/v1/adaptation/trigger` endpoint exists but has never advanced a cycle
- ORAC's VMS bridge writes succeed (HTTP 200) but produce no downstream effect

### SYNTHEX Heat Sources (:8090)

- All heat source values report 0.0 despite ORAC posting field state every 6 ticks
- Thermal PID controller operates on its own internal state only
- The `/api/ingest` handler may not call `thermal.update_heat_sources()` —
  suspected serialization gap

### Prometheus Swarm (:10001)

- 40 agents loaded, 0 tasks dispatched, load=0.0 across all agents
- Crashes with SIGABRT on `POST /api/tasks` (CRIT-01, confirmed Session 060)
- Pre-compiled binary; root cause requires rebuild or Python wrapper investigation

---

## 4. Security Posture

### Current State

| Aspect | Status | Detail |
|--------|--------|--------|
| Authentication | NONE | 22 HTTP endpoints accept any request |
| Authorization | NONE | No role/token/API-key checks |
| Rate limiting | NONE | No request throttling on any endpoint |
| TLS | NONE | Plaintext HTTP on localhost:8133 |
| Input validation | PARTIAL | Hook bodies parsed as JSON but no schema enforcement |
| Body size limit | 64 KB | `server.body_limit_bytes = 65,536` (configurable) |
| CORS | ENABLED | `tower-http` CORS middleware present |

### Specific Risks

- **Hook body injection:** POST `/hooks/*` accepts arbitrary JSON. Malformed payloads
  are deserialized into `serde_json::Value` with no schema gate. A crafted payload
  could populate `OracState` with unexpected data.
- **POVM unbounded weights:** The POVM bridge posts Hebbian weights without clamping
  on the receiver side. Weights outside [0.0, 1.0] are mathematically valid but
  operationally unexpected.
- **RM negative confidence:** Reasoning Memory accepts negative confidence values
  in TSV records. No validation on the RM bridge enforces a [0.0, 1.0] range.
- **SQLite injection:** Blackboard uses parameterized queries (safe). No raw SQL
  string concatenation found in 90 tests or source.
- **Socket permissions:** IPC socket at `/run/user/1000/pane-vortex-bus.sock` uses
  `0o700` permissions (owner-only). Adequate for single-user.

### Assessment

**Acceptable for single-machine development** where all 17 ULTRAPLATE services run
under the same UID on localhost. The absence of auth, rate limiting, and TLS makes
ORAC **unsuitable for any network-exposed deployment** without adding:
1. Bearer token or mTLS on all endpoints
2. Per-IP rate limiting (suggest 100 req/s soft, 500 req/s hard)
3. JSON Schema validation on hook POST bodies
4. TLS termination (reverse proxy or native rustls)

---

## 5. Test Maturity

### Summary

| Category | Count | Grade | Notes |
|----------|------:|-------|-------|
| Unit tests | 1,696 | STRONG | 42 modules, 17 meet 50-test threshold |
| Integration (real) | 50 | MODERATE | 3 files: api_endpoints (26), l8_evolution (15), l5_bridges (9) |
| Integration (scaffold) | 10 | WEAK | 8 layer scaffolds + stress + property placeholders |
| Property/fuzz | 0 | ABSENT | Scaffold file exists, no implementation |
| Load/stress | 0 | ABSENT | Scaffold file exists, no implementation |
| End-to-end | 0 | ABSENT | No multi-service integration tests |

### Unit Test Coverage by Layer

| Layer | Tests | Modules at 50+ | Modules Below 50 |
|-------|------:|:--------------:|:----------------:|
| L1 Core | 201 | 2 (m01, m06) | 5 |
| L2 Wire | 130 | 1 (m08) | 2 |
| L3 Hooks | 206 | 2 (m10, m12) | 3 |
| L4 Intelligence | 237 | 3 (m20, m21, m24) | 4 |
| L5 Bridges | 339 | 5 (m22-m26) | 1 |
| L6 Coordination | 133 | 0 | 5 |
| L7 Monitoring | 236 | 3 (m32, m33, m35) | 1 |
| L8 Evolution | 214 | 2 (m37, m39) | 3 |
| **Total** | **1,696** | **17** | **25** |

### Deficit

25 of 42 modules fall below the 50-test quality gate threshold. Total deficit:
596 tests needed. Worst offenders: m05_traits (1 test, -49), m29_tick (13, -37),
m04_constants (14, -36), m31_memory_manager (15, -35).

### Integration Test Gaps

- L1 Core, L2 Wire, L4 Intelligence: 0 real integration tests (scaffold only)
- Cross-layer workflows: scaffold only
- No test exercises the full hook->intelligence->bridge->evolution pipeline

---

## 6. Known Hard Limits

All values traced to source with file:line references in D6 Capacity Limits Reference.

| Limit | Value | Source | Impact |
|-------|-------|--------|--------|
| `SPHERE_CAP` | 200 | `m04_constants.rs:126` | O(N^2) coupling matrix; 200 spheres = 39,800 pairs |
| Production max spheres | 66 | Session 056 peak | Highest observed; 200-sphere behavior untested |
| `MAX_FRAME_SIZE` | 64 KB | `m09_wire_protocol.rs:46` | IPC frames exceeding this are rejected |
| `GHOST_MAX` | 20 | `m04_constants.rs:132` | FIFO eviction; oldest ghosts lost silently |
| `MAX_SEND_QUEUE` | 1,000 frames | `m09_wire_protocol.rs:49` | Outbound backpressure; no overflow handling tested |
| `MAX_MONITORS` | 50 | `m37_emergence_detector.rs:45` | Emergence monitors cap; excess silently dropped |
| `EMERGENCE_HISTORY_CAP` | 5,000 events | `m37_emergence_detector.rs:36` | Ring buffer; oldest events evicted |
| `DEFAULT_MAX_CYCLES` | 1,000 | `m36_ralph_engine.rs:53` | RALPH auto-pauses; requires manual resume |
| `DEFAULT_SNAPSHOT_CAPACITY` | 50 | `m36_ralph_engine.rs:56` | Oldest snapshots evicted; rollback depth bounded |
| `RING_LINE_CAP` | 1,000 lines | `m30_wasm_bridge.rs:54` | WASM event ring overflow evicts oldest |
| `DEFAULT_TCP_TIMEOUT_MS` | 2,000 ms | `http_helpers.rs:15` | Bridge calls fail-fast at 2s; no retry built-in |
| `body_limit_bytes` | 64 KB | `m03_config.rs:141` | HTTP request body rejection threshold |

### Scaling Concerns

- **STDP at SPHERE_CAP:** Hebbian updates are O(N^2) per tick. At 200 spheres,
  each tick processes up to 39,800 weight updates. No profiling data exists beyond
  66 spheres. The 5-second tick interval may be insufficient at scale.
- **Coupling pruning:** Stale entries are removed each poll cycle, but pruning
  itself iterates the full coupling map. At 200 spheres this is ~40K entries.
- **SQLite WAL contention:** All bridge writes and blackboard queries share a single
  WAL-mode connection pool. Under high sphere counts with 11 bridge poll intervals
  firing, write contention has not been characterized.

---

## 7. Recommendations

### Priority 1: Immediate (before any scale-up)

1. **Move bridge polls to `spawn_blocking`** -- All 6 bridge poll functions
   (`synthex_poll`, `me_poll`, `povm_snapshot`, `povm_weights`, `rm_post`,
   `vms_post`) currently use synchronous `ureq` calls. At higher sphere counts
   or degraded upstream latency, these block the tokio runtime. Wrap each in
   `tokio::task::spawn_blocking` to prevent tick starvation.

2. **Profile STDP at 200 spheres** -- The Hebbian update loop in
   `m18_hebbian_stdp.rs` is O(N^2). Run a synthetic benchmark at SPHERE_CAP
   to determine whether the 5-second tick budget is sufficient. If not, consider
   sparse update (only active pairs) or batched processing.

3. **Add auth middleware for any network deployment** -- A single `tower` layer
   checking a bearer token or shared secret would gate all 22 endpoints. This is
   a hard requirement before exposing port 8133 beyond localhost.

### Priority 2: Test hardening

4. **Fill integration test scaffolds** -- 8 of 13 integration test files are
   single-placeholder functions. Priority: L4 Intelligence (Hebbian+routing
   cross-module), L2 Wire (IPC end-to-end), cross-layer workflows (hook ->
   intelligence -> bridge round-trip).

5. **Implement property tests** -- The `tests/property_tests.rs` scaffold should
   cover: phase arithmetic wraps correctly for arbitrary f64, coupling weights
   stay within [HEBBIAN_WEIGHT_FLOOR, 1.0] after any STDP sequence, RALPH
   snapshot/rollback preserves generation monotonicity.

6. **Run an endurance test** -- ORAC has been observed running for 17+ hours
   but never under instrumented conditions. Run a 24-hour endurance test with
   synthetic hook traffic at 10 req/s, monitoring memory growth, SQLite WAL
   size, emergence ring fill rate, and coupling map size.

### Priority 3: Operational maturity

7. **Connect OTel collector** -- The 73-test OTel module (m32) has never exported
   to a real collector. Configure Jaeger or similar to validate trace export
   under production load.

8. **Test feature-stripped builds** -- Build and smoke-test with individual
   features disabled (e.g., `--no-default-features --features api,persistence`)
   to verify feature gate isolation. This enables lighter deployments where
   evolution or monitoring are not needed.

9. **Validate upstream facades** -- ORAC's data quality depends on SYNTHEX
   heat sources, VMS consolidation, and Prometheus Swarm functioning correctly.
   These upstream services are currently inert or crashing. Fix or stub them
   to provide meaningful data to ORAC's intelligence layer.

10. **Add JSON Schema validation on hook bodies** -- Define schemas for each
    of the 6 hook event types and reject non-conforming payloads at the router
    level. This prevents malformed data from propagating into OracState.

---

## Verdict

**ORAC is production-ready for its current deployment context:** a single-machine
ULTRAPLATE developer environment running 17 services under one UID on localhost.

The core loop (hooks -> field -> STDP -> RALPH -> bridges -> persistence) is
exercised, tested, and has run reliably across 8 sessions spanning 100+ hours
of cumulative runtime. Unit test coverage is strong. The Rust quality bar
(0 clippy pedantic warnings, no `unwrap()` outside tests, no `unsafe`) exceeds
typical production standards.

**ORAC is NOT ready for:**
- Network-exposed deployment (no auth, no TLS, no rate limiting)
- Scale beyond ~66 spheres (untested; O(N^2) STDP is the bottleneck)
- High-availability (single-instance, no replication, SQLite-only)
- Formal compliance (no audit logging beyond consent_audit table)

The path from current state to network-ready is estimated at ~500 LOC
(auth middleware, rate limiter, TLS config) plus ~2,000 LOC of integration
and property tests to close the test maturity gap.

---

*Cross-referenced: D6 Capacity Limits, R1 Fleet Test Inventory, D7 Verification Report*
*Obsidian: `[[Session 062 — ORAC System Atlas (ACP)]]`, `[[ULTRAPLATE Master Index]]`*
