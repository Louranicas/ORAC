# L8 Evolution — Self-Modification Layer (RALPH)

> Self-improving coordination via 5-phase RALPH loop. Cloned from ME with critical fix:
> **multi-parameter mutation** (NOT mono-parameter like ME's BUG-035).

## Feature Gate

`evolution` (implies `intelligence` + `monitoring`)

## Modules

| Module | File | Description | Test Kind |
|--------|------|-------------|-----------|
| m36_ralph_engine | `src/m8_evolution/m36_ralph_engine.rs` | 5-phase loop: Recognize→Analyze→Learn→Propose→Harvest, max 30 iterations, convergence check | unit + integration |
| m37_emergence_detector | `src/m8_evolution/m37_emergence_detector.rs` | Ring buffer with TTL decay, cap 5,000, emergence threshold detection | unit + property |
| m38_correlation_engine | `src/m8_evolution/m38_correlation_engine.rs` | Pathway discovery and correlation mining across agent interactions | unit |
| m39_fitness_tensor | `src/m8_evolution/m39_fitness_tensor.rs` | 12-dimensional weighted fitness evaluation, all FMA (P01) | unit + property |
| m40_mutation_selector | `src/m8_evolution/m40_mutation_selector.rs` | Diversity-enforced: round-robin, 10-gen cooldown, >50% rejection gate | unit + integration |

## BUG-035 Fix (CRITICAL)

ME's evolution chamber targeted `min_confidence` in 318/380 mutations (84%).
ORAC enforces:
- Round-robin across full parameter pool (not weighted toward one)
- 10-generation cooldown per parameter between repeated targeting
- Reject proposal if >50% of last 20 mutations hit same parameter
- See: `[[ORAC — RALPH Multi-Parameter Mutation Fix]]`

## Dependencies

- **L1 Core** — `OracError`, `Timestamp`, float utilities
- **L4 Intelligence** — Hebbian weights, coupling parameters, decision engine
- **L5 Bridges** — Reasoning Memory for persistence, SYNTHEX for cascade feedback
- **L7 Monitoring** — metrics for convergence tracking, emergence scoring

## Design Constraints

- RALPH loop (m36): max 30 iterations per cycle. Convergence = delta < 0.001 for 3 consecutive steps
- Mutation engine (m40) must snapshot before mutation and support atomic rollback
- Emergence cap: 5,000 with TTL decay (AP19 — cap alone → BUG-035 deadlock)
- Fitness threshold: only apply if improvement ≥ 2%
- All tensor operations use FMA
- Feature-gated: `#[cfg(feature = "evolution")]`

## Runtime Wiring (Session 055)

### Tick Loop (`src/bin/main.rs`)

RALPH runs as a background `tokio::spawn` task alongside the Axum HTTP server:

```
main()
  ├── tokio::spawn(spawn_ralph_loop)   ← 5-second interval
  │     ├── build_tensor_from_state()  ← 12D from live OracState
  │     └── state.ralph.tick(&tensor, tick)
  └── axum::serve(router)             ← HTTP on :8133
```

- **Interval:** 5 seconds (configurable via `RalphEngineConfig`)
- **Shutdown:** `tokio::sync::watch` channel shared with Axum graceful shutdown
- **Feature gate:** `#[cfg(feature = "evolution")]` — entire loop compiles out when disabled
- **Tensor source:** `build_tensor_from_state()` constructs 12D `TensorValues` from:
  - `vals[0]` coordination: `session_count / 9.0` (9 fleet panes max)
  - `vals[1]` field coherence: `field_state.order.r`
  - `vals[3]` bridge health: hardcoded 0.75 (TODO: wire to circuit breaker states)
  - `vals[11]` overall: mean of dims 0-10
  - Remaining dims: placeholders (0.5-1.0), wired incrementally as bridges mature

### OracState Integration (`m10_hook_server.rs`)

```rust
pub struct OracState {
    #[cfg(feature = "evolution")]
    pub ralph: RalphEngine,       // ← direct field, not Arc-wrapped
    // ...
}
```

- `RalphEngine` is `Send + Sync` (all fields behind `parking_lot::RwLock`)
- Shared via `Arc<OracState>` between Axum handlers and the RALPH tick loop
- No lock contention: RALPH reads `field_state` (fast RwLock::read) and writes to its own locks

### Health Endpoint (`GET /health`)

Three fields added to `HealthResponse`:

```json
{
  "ralph_gen": 42,
  "ralph_phase": "Analyze",
  "ralph_fitness": 0.667
}
```

Populated from `state.ralph.state()` on each health check.

### Field Endpoint (`GET /field`)

Emergence data appended when `evolution` feature is on:

```json
{
  "emergence": {
    "total_detected": 3,
    "active_monitors": 1,
    "history_len": 3,
    "by_type": {"beneficial_sync": 2, "coherence_lock": 1},
    "recent": [{"type": "beneficial_sync", "severity": "low", ...}]
  }
}
```

Source: `state.ralph.emergence().recent(5)` and `.stats()`.

### Metrics Endpoint (`GET /metrics`)

11 Prometheus metrics via `append_ralph_metrics()`:

| Metric | Type | Source |
|--------|------|--------|
| `orac_ralph_generation` | counter | `RalphState.generation` |
| `orac_ralph_completed_cycles` | counter | `RalphState.completed_cycles` |
| `orac_ralph_fitness` | gauge | `RalphState.current_fitness` |
| `orac_ralph_peak_fitness` | gauge | `RalphStats.peak_fitness` |
| `orac_ralph_paused` | gauge | `RalphState.paused` (0/1) |
| `orac_ralph_mutations_total{outcome=*}` | counter | proposed/accepted/rolled_back/skipped |
| `orac_emergence_total` | counter | `EmergenceStats.total_detected` |
| `orac_emergence_active_monitors` | gauge | `EmergenceDetector.active_monitor_count()` |

### Integration Tests (`tests/l8_evolution_integration.rs`)

15 tests covering:
- Phase cycling (Recognize→Analyze→Learn→Propose→Harvest)
- Generation monotonic advancement
- Pause/resume behavior
- Auto-pause at `max_cycles` (tested at 5, 50, 100, 1000)
- Stats invariants (`proposed + skipped >= completed_cycles`, `accepted + rolled_back <= proposed`)
- Fitness evaluation (non-zero after convergence)
- Frozen state after auto-pause (no advancement on further ticks)
- Resume-after-max-cycles (immediate re-pause)

## Hot-Swap Source

- Cloned from ME (`the_maintenance_engine/`) with multi-parameter mutation fix
- ALL NEW for ORAC (ME code restructured for diversity enforcement)

## Cross-References

- [[Session 050 — ME Evolution Chamber Spec]]
- [[ME RALPH Loop Specification]]
- [[ORAC — RALPH Multi-Parameter Mutation Fix]]
- [[ULTRAPLATE — Bugs and Known Issues]] (BUG-035)
- ORAC_PLAN.md §Phase 4 Detail
- ORAC_MINDMAP.md §Branch 4 (RALPH Evolution Chamber)
