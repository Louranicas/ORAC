# L1 Core — Foundation Layer

> Always compiled. Zero upward imports. All other layers depend on L1.

## Feature Gate

None (always on).

## Modules

| Module | File | Description | Test Kind |
|--------|------|-------------|-----------|
| m01_core_types | `src/m1_core/m01_core_types.rs` | `PaneId`, `TaskId`, `OrderParameter`, `FleetMode`, `Timestamp`, `DecisionRecord`, `PaneSphere` newtypes | unit |
| m02_error_handling | `src/m1_core/m02_error_handling.rs` | Unified `OracError` enum, `ErrorClassifier` trait (`Send + Sync + Debug`), `thiserror` derivation | unit |
| m03_config | `src/m1_core/m03_config.rs` | `PvConfig` with TOML + env overlay (port, bridges, hooks, evolution) | unit |
| m04_constants | `src/m1_core/m04_constants.rs` | Named constants: thresholds, intervals, budgets, limits, STDP parameters | unit |
| m05_traits | `src/m1_core/m05_traits.rs` | Core traits (`Oscillator`, `Learnable`, `Bridgeable`, `Persistable`, `FieldObserver`) with `Send + Sync` bounds | unit |
| m06_validation | `src/m1_core/m06_validation.rs` | Input validators (persona 256 chars, `tool_name` 128 chars, summary 4096 chars, body 65KB, phase, frequency) | unit |
| field_state | `src/m1_core/field_state.rs` | Sidecar-native `AppState`, `SharedState`, `FieldState`, `FieldDecision`, `Harmonics` — cached from PV2, not authoritative | unit |

## Dependencies

None. L1 is the dependency root.

## Design Constraints

- Every type is `Send + Sync`
- No `unsafe`, no panics, no I/O in type definitions
- `const fn` wherever compiler allows
- All constructors are `#[must_use]`
- `Timestamp` newtype replaces `chrono`/`SystemTime`
- `BridgeStaleness` uses `u8` bitfield (gold standard refactor)
- All validation collects errors, joins with "; "

## Hot-Swap Source

- m01-m06: DROP-IN from PV2 `m1_foundation/` (candidate-modules/drop-in/L1-foundation/)
- field_state: NEW (sidecar-native, not in PV2)

## Cross-References

- [[Session 050 — ORAC Sidecar Architecture]]
- [[Pane-Vortex — Fleet Coordination Daemon]]
- ORAC_PLAN.md §Hot-Swap Module Map → DROP-IN
