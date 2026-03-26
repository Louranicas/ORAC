# R1 Fleet Type Flow — Cross-Module Type Analysis

> **Generated:** 2026-03-25 | **Scope:** `orac-sidecar/src/` 8 layers, 40 modules
> **Method:** Static import analysis of all `use crate::` statements across layer boundaries

---

## 1. Per-Layer Re-exports

**No `pub use` re-exports exist in any `mod.rs`.** All types are accessed via fully qualified paths
(e.g., `crate::m1_core::m01_core_types::PaneId`). The `lib.rs` declares only `pub mod` for each layer.

| Layer | mod.rs | Re-exports |
|-------|--------|------------|
| L1 `m1_core` | 7 `pub mod` declarations (m01-m06 + field_state) | 0 |
| L2 `m2_wire` | 3 `pub mod` declarations (m07-m09) | 0 |
| L3 `m3_hooks` | 5 `pub mod` declarations (m10-m14) | 0 |
| L4 `m4_intelligence` | 7 `pub mod` declarations (m15-m21) | 0 |
| L5 `m5_bridges` | 6 `pub mod` declarations (m22-m26 + http_helpers) | 0 |
| L6 `m6_coordination` | 5 `pub mod` declarations (m27-m31) | 0 |
| L7 `m7_monitoring` | 4 `pub mod` declarations (m32-m35) | 0 |
| L8 `m8_evolution` | 5 `pub mod` declarations (m36-m40) | 0 |

---

## 2. Cross-Layer Type Usage Matrix

Rows = source layer defining the type. Columns = consuming layer.
`P` = production code import. `T` = test-only import. `-` = no import.

### L1 Types consumed by other layers

| Type | Origin Module | L2 | L3 | L4 | L5 | L6 | L7 | L8 | Layers |
|------|--------------|----|----|----|----|----|----|-----|--------|
| `PaneId` | m01_core_types | P | P | P | P | P | P | - | **6** |
| `PvError` | m02_error_handling | P | - | - | P | P | P | P | **5** |
| `PvResult` | m02_error_handling | P | - | - | P | P | P | P | **5** |
| `PaneStatus` | m01_core_types | - | P | P | P | - | - | - | **4** (1) |
| `m04_constants` | m04_constants | - | - | P | P | P | P | - | **4** |
| `PaneSphere` | m01_core_types | - | P | P | - | T | - | - | **3** |
| `OrderParameter` | m01_core_types | - | - | P | - | P | P | - | **3** |
| `now_secs` (fn) | m01_core_types | P | - | - | - | P | P | - | **3** |
| `TaskId` | m01_core_types | P | - | - | - | - | P | - | 2 |
| `FieldState` | field_state | - | P | - | - | P | - | - | 2 |
| `AppState` | field_state | - | - | - | - | P | - | - | 1 (2) |
| `SharedState` | field_state | - | P | - | - | - | - | - | 1 |
| `FieldDecision` | field_state | - | - | - | - | P | - | - | 1 |
| `FieldAction` | m01_core_types | - | - | - | - | P | - | - | 1 |
| `PvConfig` | m03_config | - | P | - | - | - | - | - | 1 |
| `Bridgeable` | m05_traits | - | - | - | P | - | - | - | 1 |
| `ActivationZones` | m01_core_types | - | - | - | - | P | - | - | 1 |
| `Point3D` | m01_core_types | - | - | P | - | T | - | - | 1-2 |
| `Buoy` | m01_core_types | - | - | P | - | - | - | - | 1 |
| `SphereMemory` | m01_core_types | - | - | T | - | T | - | - | 0 (T) |
| `Harmonics` | field_state | - | - | - | - | - | - | - | 0 |

(1) `PaneStatus` also used in m27 conductor (L6) via `FieldAction` decisions, but not directly imported.
(2) `AppState` used in L3 tests but not production L3 code (OracState wraps its own state).

### L2 Types consumed by other layers

| Type | Origin Module | L3 | L4 | L5 | L6 | L7 | L8 | Layers |
|------|--------------|----|----|----|----|----|----|--------|
| `BusFrame` | m08_bus_types | - | - | - | - | - | - | **0** (3) |
| `BusEvent` | m08_bus_types | - | - | - | - | - | - | 0 |
| `BusTask` | m08_bus_types | - | - | - | - | - | - | 0 |
| `IpcClient` | m07_ipc_client | - | - | - | - | - | - | 0 (4) |
| `ConnectionState` | m07_ipc_client | - | - | - | - | - | - | 0 |
| `WireProtocol` | m09_wire_protocol | - | - | - | - | - | - | 0 |

(3) L2 types are entirely self-contained within the wire layer. `BusFrame` and `BusEvent` are used in `main.rs` (binary), not library layers.
(4) `IpcClient` is consumed in `main.rs` binary only.

### L3+ Types consumed by higher layers (cross-layer)

| Type | Origin | Consumed In | Scope |
|------|--------|-------------|-------|
| `CouplingNetwork` | L4 m15 | L3 (m10, m12), L5 (m24 test), L6 (m29) | P+T |
| `apply_stdp` (fn) | L4 m18 | L6 (m29) | P |
| `SemanticDomain` | L4 m20 | L3 (m10, m12) | P+T |
| `classify_content` (fn) | L4 m20 | L3 (m12) | P+T |
| `classify_tool` (fn) | L4 m20 | L3 (m12) | P |
| `route` (fn) | L4 m20 | L3 (m12) | P+T |
| `RouteRequest` | L4 m20 | L3 (m12) | P+T |
| `BreakerConfig` | L4 m21 | L3 (m10) | P |
| `BreakerRegistry` | L4 m21 | L3 (m10) | P |
| `Blackboard` | L5 m26 | L3 (m10) | P |
| `PaneRecord` | L5 m26 | L3 (m10, m11, m12) | P+T |
| `TaskRecord` | L5 m26 | L3 (m11, m12) | P+T |
| `AgentCard` | L5 m26 | L3 (m11) | T |
| `GhostRecord` | L5 m26 | L3 (m10) | T |
| `ConsentAuditEntry` | L5 m26 | L3 (m10) | T |
| `SynthexBridge` | L5 m22 | L3 (m10) | P |
| `MeBridge` | L5 m23 | L3 (m10) | P |
| `RmBridge` | L5 m25 | L3 (m10) | P |
| `SpanBuilder` | L7 m32 | L3 (m12) | P |
| `RalphEngine` | L8 m36 | L3 (m10) | P |
| `FitnessDimension` | L8 m39 | L3 (m10) | P+T |
| `TensorValues` | L8 m39 | L3 (m10) | P+T |
| `EmergenceType` | L8 m37 | L3 (m10) | T |

**Observation:** L3 (hooks layer) is the convergence point. It imports from L4, L5, L7, and L8
because `OracState` in m10 holds the application's composite state and wires all subsystems together.

---

## 3. Most-Used Types (Ranked)

Types ranked by number of distinct layers they appear in (excluding their defining layer).

| Rank | Type | Origin | Layers Crossed | Consuming Layers |
|------|------|--------|---------------|-----------------|
| 1 | `PaneId` | L1 m01 | **6** | L2, L3, L4, L5, L6, L7 |
| 2 | `PvError` | L1 m02 | **5** | L2, L5, L6, L7, L8 |
| 2 | `PvResult` | L1 m02 | **5** | L2, L5, L6, L7, L8 |
| 4 | `PaneStatus` | L1 m01 | **4** | L3, L4, L5, L6 |
| 4 | `m04_constants` (module) | L1 m04 | **4** | L4, L5, L6, L7 |
| 6 | `PaneSphere` | L1 m01 | **3** | L3, L4, L6 |
| 6 | `OrderParameter` | L1 m01 | **3** | L4, L6, L7 |
| 6 | `now_secs` (fn) | L1 m01 | **3** | L2, L6, L7 |
| 9 | `CouplingNetwork` | L4 m15 | **3** | L3, L5(T), L6 |
| 10 | `TaskId` | L1 m01 | 2 | L2, L7 |
| 10 | `FieldState` | L1 field_state | 2 | L3, L6 |

### Per-module type export count (types consumed outside their layer)

| Module | Types Exported Cross-Layer | Key Exports |
|--------|---------------------------|-------------|
| L1 m01_core_types | 8 | PaneId, PaneStatus, PaneSphere, OrderParameter, TaskId, ... |
| L1 m02_error_handling | 2 | PvError, PvResult |
| L1 field_state | 4 | AppState, SharedState, FieldState, FieldDecision |
| L1 m04_constants | 1 (module) | 59+ named constants |
| L1 m03_config | 1 | PvConfig |
| L1 m05_traits | 1 | Bridgeable |
| L4 m15_coupling_network | 1 | CouplingNetwork |
| L4 m20_semantic_router | 3 | SemanticDomain, RouteRequest, classify_content/tool/route |
| L4 m21_circuit_breaker | 2 | BreakerConfig, BreakerRegistry |
| L5 m26_blackboard | 6 | Blackboard, PaneRecord, TaskRecord, AgentCard, GhostRecord, ... |
| L5 m22-m25 bridges | 3 | SynthexBridge, MeBridge, RmBridge |
| L7 m32_otel_traces | 1 | SpanBuilder |
| L8 m36_ralph_engine | 1 | RalphEngine |
| L8 m37_emergence_detector | 1 | EmergenceType |
| L8 m39_fitness_tensor | 2 | FitnessDimension, TensorValues |

---

## 4. Layer Boundary Types

Types that appear in public API surfaces crossing layer boundaries. These are the types that
define the contract between layers and would require coordinated changes if modified.

### Tier 1: Universal Foundation (6+ layers)

| Type | Kind | Defined In | Public API Surface |
|------|------|-----------|-------------------|
| `PaneId` | newtype struct (String) | L1 m01 | Parameter and return type in every layer. The universal identifier. Used as HashMap key, function parameter, struct field, and display value across 30+ modules. |

### Tier 2: Error Contract (5 layers)

| Type | Kind | Defined In | Public API Surface |
|------|------|-----------|-------------------|
| `PvError` | enum (12 variants) | L1 m02 | Error type for all fallible operations. Every bridge, coordination, monitoring, and evolution module returns `Result<T, PvError>`. |
| `PvResult<T>` | type alias | L1 m02 | Alias for `Result<T, PvError>`. The standard return type across 25+ public functions. |

### Tier 3: Domain Identity (3-4 layers)

| Type | Kind | Defined In | Public API Surface |
|------|------|-----------|-------------------|
| `PaneStatus` | enum (Idle/Working/Blocked/Complete) | L1 m01 | Used in Blackboard records (L5), STDP learning decisions (L4), hook state injection (L3), and conductor decisions (L6). |
| `PaneSphere` | struct | L1 m01 | ORAC-native sphere representation. Consumed in hook handlers (L3), coupling/routing (L4), and memory management (L6). |
| `OrderParameter` | struct (r, psi) | L1 m01 | Kuramoto order parameter. Used in coupling dynamics (L4), tick orchestration (L6), and field dashboard (L7). |
| `m04_constants` | module (59+ consts) | L1 m04 | Threshold values consumed by L4 (STDP rates), L5 (bridge intervals), L6 (conductor gains), L7 (dashboard thresholds). |

### Tier 4: Cross-Layer Intelligence (3 layers, non-L1)

| Type | Kind | Defined In | Public API Surface |
|------|------|-----------|-------------------|
| `CouplingNetwork` | struct | L4 m15 | Kuramoto coupling matrix. Created in L3 (OracState), mutated in L6 (tick/STDP), queried in L5 (POVM tests). The only non-L1 type crossing 3+ layer boundaries. |

### Tier 5: Convergence Hub Types (L3 as sink)

L3 `m10_hook_server::OracState` is the convergence struct that holds references to types from
L4, L5, L7, and L8. These types don't cross between each other but all flow into L3:

| Type | From | Role in OracState |
|------|------|-------------------|
| `CouplingNetwork` | L4 m15 | Kuramoto coupling matrix |
| `BreakerRegistry` | L4 m21 | Per-service circuit breakers |
| `Blackboard` | L5 m26 | SQLite shared fleet state |
| `SynthexBridge` | L5 m22 | Thermal read bridge |
| `MeBridge` | L5 m23 | Fitness signal bridge |
| `RmBridge` | L5 m25 | TSV persistence bridge |
| `RalphEngine` | L8 m36 | Evolution chamber |

---

## 5. Dependency Graph Summary

```
L1 Core ──────────────────────────────────────────────────────────────────
  │  PaneId, PvError/PvResult, PaneStatus, OrderParameter, constants
  │
  ├──► L2 Wire (PaneId, PvError, TaskId, now_secs)
  │      │  [self-contained: BusFrame, IpcClient stay in L2 + binary]
  │      │
  ├──► L4 Intelligence (PaneId, PaneStatus, PaneSphere, OrderParameter, constants)
  │      │  [exports: CouplingNetwork → L3, L6]
  │      │  [exports: SemanticDomain, RouteRequest → L3]
  │      │  [exports: BreakerRegistry → L3]
  │      │
  ├──► L5 Bridges (PaneId, PaneStatus, PvError/PvResult, Bridgeable, constants)
  │      │  [exports: Blackboard, *Record types → L3]
  │      │  [exports: SynthexBridge, MeBridge, RmBridge → L3]
  │      │
  ├──► L6 Coordination (PaneId, PvError, AppState, FieldState, FieldDecision, constants)
  │      │  [imports: CouplingNetwork, apply_stdp ← L4]
  │      │
  ├──► L7 Monitoring (PaneId, TaskId, PvError/PvResult, OrderParameter, constants, now_secs)
  │      │  [exports: SpanBuilder → L3]
  │      │
  ├──► L8 Evolution (PvError/PvResult)
  │      │  [exports: RalphEngine, FitnessDimension, TensorValues, EmergenceType → L3]
  │      │
  └──► L3 Hooks ◄── CONVERGENCE POINT
         [imports from: L1, L4, L5, L7, L8]
         [exports: OracState, build_router → binary only]
```

### Key Architectural Observations

1. **L1 is the true foundation** — 8 types cross 3+ layers. `PaneId` is universal (6 layers).

2. **L2 is hermetically sealed** — No L2 type escapes to L3+. Wire protocol types stay
   internal; only `main.rs` (binary) touches `IpcClient`/`BusFrame`.

3. **L3 is a convergence sink, not a source** — Hooks layer imports from 5 other layers
   (L1, L4, L5, L7, L8) but exports nothing to library layers. Its types (`OracState`,
   `HookEvent`, `HookResponse`) flow only to the binary.

4. **L4 → L3 and L4 → L6 are the only non-L1 cross-layer flows** — `CouplingNetwork` is
   the sole non-L1 type crossing 3+ boundaries (L3, L5-test, L6).

5. **L8 → L3 is a notable upward dependency** — Evolution types (`RalphEngine`,
   `FitnessDimension`) flow into the hooks layer, making L3 depend on L8. This is
   architecturally justified: `OracState` is the application's composite state.

6. **Error types (`PvError`/`PvResult`) skip L3 and L4** — L3 uses `axum::Json` error
   responses instead. L4 intelligence modules use `Option` semantics rather than `Result`.

7. **Constants module acts as a configuration broadcast** — `m04_constants` is imported as a
   module (not individual items) in 4 layers, providing compile-time configuration to
   STDP rates, conductor gains, dashboard thresholds, and bridge intervals.
