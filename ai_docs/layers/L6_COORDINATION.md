# L6 Coordination — Orchestration Layer

> Conductor logic, cascade management, tick engine, WASM plugin bridge, memory management.

## Feature Gate

None (always on). Orchestrates whatever layers are compiled in.

## Modules

| Module | File | Description | Source | Test Kind |
|--------|------|-------------|--------|-----------|
| m27_conductor | `src/m6_coordination/m27_conductor.rs` | PI controller for field breathing rhythm, EMA-smoothed decisions | PV2 M31 (adapt) | unit + integration |
| m28_cascade | `src/m6_coordination/m28_cascade.rs` | Cascade handoff protocol with sphere mitosis (SYS-1), phase + coupling weight transfer | PV2 M33 (drop-in) | unit |
| m29_tick | `src/m6_coordination/m29_tick.rs` | Tick orchestrator: 60-tick snapshot cycle, Hebbian Phase 2.5 wiring, activation decay | PV2 M35 (adapt) | unit + integration |
| m30_wasm_bridge | `src/m6_coordination/m30_wasm_bridge.rs` | FIFO/ring protocol bridge to Zellij swarm-orchestrator WASM plugin | NEW | integration |
| m31_memory_manager | `src/m6_coordination/m31_memory_manager.rs` | Memory aggregation: pruning (activation < 0.05 every 200 steps), cap 500/sphere | PV2 M21 (drop-in) | unit + property |

## WASM Bridge Protocol

```text
WASM plugin → /tmp/swarm-commands.pipe (FIFO)  → ORAC
ORAC        → /tmp/swarm-events.jsonl  (ring)  → WASM plugin
              (1000 line cap, oldest dropped — P22)
```

## Design Constraints

- `tick_once()` is the heartbeat — called by async tick loop, never by HTTP handlers
- Snapshot persistence: JSON every 60 ticks + on SIGTERM. Restores on startup
- Memory pruning runs every 200 ticks. Removes entries with activation < 0.05
- Lock ordering: `AppState` before `BusState` (deadlock prevention)
- Conductor uses multiplicative bridge composition, not additive

## Hot-Swap Source

- m27, m29: ADAPT from PV2 (candidate-modules/adapt/L6-conductor/, L6-tick/)
- m28, m31: DROP-IN from PV2 (candidate-modules/drop-in/L6-cascade/, L4-learning/m21)
- m30: NEW

## Cross-References

- [[Pane-Vortex — Fleet Coordination Daemon]]
- [[Session 045 Arena — 12-live-field-analysis]]
- [[Swarm Orchestrator — Complete Reference]]
- ORAC_PLAN.md §Phase 3 Detail
- ORAC_MINDMAP.md §Branch 7 (WASM Bridge), §Branch 9 (Cascade Handoffs)
