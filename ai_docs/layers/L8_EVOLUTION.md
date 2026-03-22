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
