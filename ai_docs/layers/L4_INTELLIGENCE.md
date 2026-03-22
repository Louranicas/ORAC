# L4 Intelligence — Hebbian Learning, Coupling & Routing

> Smart dispatch using Hebbian weights, content-aware routing, per-pane health gating.
> Hot-swap M15-M19 from PV2, plus new semantic router (M20) and circuit breaker (M21).

## Feature Gate

`intelligence`

## Modules

| Module | File | Description | Source | Test Kind |
|--------|------|-------------|--------|-----------|
| m15_coupling_network | `src/m4_intelligence/m15_coupling_network.rs` | Kuramoto coupling matrix, phase dynamics, adjacency index | PV2 M16 (drop-in) | unit + property |
| m16_auto_k | `src/m4_intelligence/m16_auto_k.rs` | Adaptive coupling strength, consent-gated `k_adjustment`, EMA smoothing | PV2 M17 (drop-in) | unit |
| m17_topology | `src/m4_intelligence/m17_topology.rs` | Network topology analysis, `mean_coupling_weight` | PV2 M18 (drop-in) | unit |
| m18_hebbian_stdp | `src/m4_intelligence/m18_hebbian_stdp.rs` | STDP learning: LTP 0.01 (3x burst, 2x newcomer), LTD 0.002, weight floor 0.05 | PV2 M19 (drop-in) | unit + property |
| m19_buoy_network | `src/m4_intelligence/m19_buoy_network.rs` | Buoy health tracking, spatial recall, activation threshold 0.3, influence radius 0.50 | PV2 M20 (drop-in) | unit |
| m20_semantic_router | `src/m4_intelligence/m20_semantic_router.rs` | Content-aware dispatch using Hebbian weights, domain affinity scoring | NEW | unit |
| m21_circuit_breaker | `src/m4_intelligence/m21_circuit_breaker.rs` | Per-pane health gating: Closed/Open/`HalfOpen` FSM, tower-resilience pattern | NEW | unit + integration |

## STDP Parameters

- LTP: 0.01 (3x burst, 2x newcomer)
- LTD: 0.002
- Weight floor: 0.05
- Phase wrapping: `.rem_euclid(TAU)` after all arithmetic (P01)

## Design Constraints

- Hebbian weight updates are O(N^2 x degree) — cache the adjacency index
- `k_mod` range hard-clamped to [-0.5, 1.5]
- `auto_scale_k` multiplier: 0.5 (not 1.5). Weight exponent: fixed w^2
- Per-status K modulation: Working↔Working 1.2x, Idle↔Working 0.5x, Blocked 0.0x
- Circuit breaker transitions emit bus events
- All float operations use FMA (P01-P08)
- Semantic phase: tool→phase region (Read→0, Write→π/2, Execute→π, Communicate→3π/2)

## Hot-Swap Source

- m15-m19: DROP-IN from PV2 (candidate-modules/drop-in/L4-coupling/ + L4-learning/)
- m20-m21: NEW (ORAC-specific)

## Cross-References

- [[Session 045 Arena — 10-hebbian-operational-topology]]
- [[Vortex Sphere Brain-Body Architecture]]
- [[Executor and Nested Kuramoto Bridge — Session 028]]
- ORAC_PLAN.md §Phase 2 Detail
- ORAC_MINDMAP.md §Branch 3 (Intelligence Layer)
