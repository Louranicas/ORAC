---
title: "Layer 1: Core — Module Documentation"
date: 2026-03-22
tags: [modules, core, L1, orac-sidecar]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L1
modules: [m01, m02, m03, m04, m05, m06, field_state]
---

# Layer 1: Core (m01-m06 + field_state)

> Foundation layer. Always compiled. Zero upward imports. All other layers depend on L1.
> **Target LOC:** ~3,400 | **Target tests:** 80+
> **Source:** DROP-IN from PV2 M01-M06 (field_state is NEW) | **Phase:** 1

---

## Overview

L1 Core provides the type-safe foundation for the entire ORAC sidecar: newtypes that prevent
stringly-typed bugs at API boundaries, a unified error enum with classification for retry/alert
decisions, TOML+env configuration with validation, compile-time constants, dependency-inversion
traits, input validators, and sidecar-native field state. Every module in L1 is a leaf or
near-leaf in the dependency graph. Implementation order: m01 -> m02 -> m04 -> m05 -> m03 -> m06
-> field_state (m03 depends on m02; m05 depends on m01+m02; m06 depends on m02+m04; field_state
depends on m01).

### Design Invariants (All Modules)

- Every type is `Send + Sync`
- No `unsafe`, no panics, no I/O in type definitions (A1, A2, A3)
- `const fn` wherever compiler allows
- All constructors are `#[must_use]`
- `Timestamp` newtype replaces `chrono`/`SystemTime` (A5)
- FMA for all multi-step float arithmetic (P05)
- Explicit imports only, never glob (A7)

---

## m01 — Core Types

**Source:** `src/m1_core/m01_core_types.rs`
**LOC:** ~880
**Depends on:** None (leaf module)
**Hot-Swap:** DROP-IN from PV2 M01

### Design Decisions

- **Newtypes over raw strings**: `PaneId` and `TaskId` wrap `String` to prevent mix-ups at
  function boundaries. Both implement `Display`, `From<String>`, `From<&str>`.
- **Copy semantics for `Point3D`**: 24 bytes (3x f64) stays on the stack. Used pervasively
  for sphere memory placement and buoy positioning.
- **`OrderParameter` as value type**: Two-field struct (`r`, `psi`) with `const fn` constructors.
  Avoids the overhead of a newtype-around-tuple and keeps field access ergonomic.
- **`BridgeStaleness` as `u8` bitfield**: Replaces a 6-bool struct (P19). Each bit maps to one
  bridge (SYNTHEX=0, NEXUS=1, POVM=2, RM=3, VMS=4, ME=5). `const fn` accessors.
- **Semantic phase mapping**: `semantic_phase_region()` places known tool families at fixed
  octants on [0, 2pi), with FNV-1a hash fallback for unknown tools. Improves Hebbian buoy
  formation by clustering semantically related tool calls.

### Types to Implement

```rust
/// Unique pane identifier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(String);

/// Unique task identifier (UUID v4).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

/// Point on the unit sphere (3D embedding). Copy semantics: 24 bytes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3D { pub x: f64, pub y: f64, pub z: f64 }

/// A memory placed on the sphere surface by a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SphereMemory {
    pub id: u64, pub position: Point3D, pub activation: f64,
    pub tool_name: String, pub summary: String,
    pub timestamp: f64, pub confidence: f64,
}

/// Hebbian buoy -- a learned cluster on the sphere surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buoy {
    pub position: Point3D, pub home: Point3D,
    pub activation_count: u64, pub influence_radius: f64,
    pub boost_multiplier: f64, pub learning_rate: f64, pub label: String,
}

/// Kuramoto order parameter: magnitude r and mean phase psi.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrderParameter { pub r: f64, pub psi: f64 }

/// Sphere operational status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneStatus { #[default] Idle, Working, Blocked, Complete }

/// Continuous work characterisation. All fields in [0.0, 1.0].
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkSignature {
    pub intensity: f64, pub rhythm: f64,
    pub diversity: f64, pub focus: f64,
}

/// Field context injected into a sphere's step().
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SphereFieldContext {
    pub global_r: f64, pub my_cluster_size: usize,
    pub is_synchronized: bool, pub my_coupling_strength: f64,
    pub tunnel_count: usize,
}

/// Lightweight trace of a deregistered sphere.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostTrace {
    pub id: PaneId, pub persona: String, pub deregistered_at: u64,
    pub total_steps_lived: u64, pub memory_count: usize,
    pub top_tools: Vec<String>, pub phase_at_departure: f64,
    pub receptivity: f64, pub work_signature: WorkSignature,
    pub strongest_neighbors: Vec<(String, f64)>,
}

/// Recommended action based on current field state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldAction {
    #[default] Stable, NeedsCoherence, NeedsDivergence,
    HasBlockedAgents, IdleFleet, FreshFleet, Recovering, OverSynchronized,
}

/// Trend of the order parameter over the rolling window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RTrend { Rising, Falling, #[default] Stable }

/// Bridge adjustment tracking.
#[derive(Debug, Clone, Default)]
pub struct BridgeAdjustments { /* synthex_adj, nexus_adj, me_adj, combined_effect, updated_at */ }

/// Per-bridge staleness flags packed into a u8 bitfield (P19).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeStaleness(u8);

/// Fleet operational mode reflecting sphere count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetMode { #[default] Solo, Pair, Small, Full }

/// A pane sphere's state as observed by the sidecar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSphere {
    pub id: PaneId, pub persona: String, pub status: PaneStatus,
    pub phase: f64, pub frequency: f64, pub buoys: Vec<Buoy>,
    pub memories: Vec<SphereMemory>, pub opt_out_hebbian: bool,
    pub activity_30s: usize, pub total_steps: u64,
    pub receptivity: f64, pub work_signature: WorkSignature,
    pub field_context: Option<SphereFieldContext>,
}

/// Decision record for conductor audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub tick: u64, pub action: FieldAction, pub r: f64,
    pub k_mod: f64, pub sphere_count: usize,
}

/// Inbox message waiting for sphere acknowledgement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxMessage {
    pub id: u64, pub from: String, pub content: String,
    pub received_at: f64, pub acknowledged: bool,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `PaneId::new` | `fn new(id: impl Into<String>) -> Self` | Create pane ID from any string |
| `TaskId::new` | `fn new() -> Self` | Generate random UUID v4 task ID |
| `Point3D::from_spherical` | `fn from_spherical(theta: f64, phi: f64) -> Self` | Construct from polar/azimuthal |
| `Point3D::dot` | `fn dot(self, other: Self) -> f64` | Dot product (FMA: P05) |
| `Point3D::angular_distance` | `fn angular_distance(self, other: Self) -> f64` | Great-circle distance on unit sphere |
| `Point3D::slerp` | `fn slerp(self, other: Self, t: f64) -> Self` | Spherical linear interpolation |
| `Buoy::boost_at` | `fn boost_at(&self, point: &Point3D) -> f64` | Activation boost at a given point |
| `Buoy::drift_toward` | `fn drift_toward(&mut self, centroid: &Point3D)` | Hebbian learning drift |
| `FleetMode::from_count` | `const fn from_count(n: usize) -> Self` | Determine mode from sphere count |
| `BridgeStaleness::is_stale` | `const fn is_stale(self, mask: u8) -> bool` | Check if a bridge is stale |
| `now_secs` | `fn now_secs() -> f64` | Current UNIX epoch seconds |
| `phase_diff` | `fn phase_diff(a: f64, b: f64) -> f64` | Wrapping phase difference in [-pi, pi] (P01) |
| `semantic_phase_region` | `fn semantic_phase_region(tool_name: &str) -> f64` | Map tool name to phase octant |

### Tests

| Test | Validates |
|------|-----------|
| `pane_id_roundtrip` | `PaneId::new` -> `as_str` identity |
| `task_id_uniqueness` | Two `TaskId::new()` calls produce different IDs |
| `point3d_north_is_unit` | `Point3D::north().norm()` == 1.0 |
| `point3d_angular_distance_self` | Distance to self == 0.0 |
| `point3d_angular_distance_opposite` | Antipodal points == pi |
| `point3d_slerp_endpoints` | `slerp(_, 0.0)` == self, `slerp(_, 1.0)` == other |
| `point3d_dot_fma` | FMA dot product matches manual computation |
| `order_parameter_incoherent` | `OrderParameter::incoherent()` has r=0, psi=0 |
| `fleet_mode_thresholds` | Solo(0,1), Pair(2), Small(3,4), Full(5+) |
| `bridge_staleness_bitfield` | Set/clear/query individual bridge bits |
| `bridge_staleness_count` | `stale_count()` matches number of set bits |
| `semantic_phase_read_zero` | `Read` tools map to phase 0.0 |
| `semantic_phase_write_octant` | `Write` tools map to TAU*0.125 |
| `semantic_phase_deterministic` | Same tool name always produces same phase |
| `phase_diff_wrapping` | Result always in [-pi, pi] (P01) |
| `ghost_trace_serialization` | `GhostTrace` round-trips through serde_json |
| `pane_sphere_default` | Default sphere has phase 0, frequency 1.0, receptivity 1.0 |
| `now_secs_positive` | `now_secs()` returns a positive value |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- type inventory
- ORAC_PLAN.md -- Hot-Swap Module Map, DROP-IN section
- `.claude/patterns.json` P01 (phase wrapping), P05 (FMA), P06 (NaN guard), P07 (saturating arithmetic), P19 (bridge staleness bitfield)
- `ai_docs/ANTI_PATTERNS.md` A5 (chrono/SystemTime), A7 (glob imports)
- PV2 source: `pane-vortex-v2/src/m1_foundation/m01_core_types.rs`

---

## m02 — Error Handling

**Source:** `src/m1_core/m02_error_handling.rs`
**LOC:** ~520
**Depends on:** None
**Hot-Swap:** DROP-IN from PV2 M02

### Design Decisions

- **Single unified error enum**: `PvError` with `thiserror` derivation. Every variant carries
  a structured error code in the PV-NNNN range, organized by category.
- **Error code ranges**: Config (1000-1099), Validation (1100-1199), Field (1200-1299),
  Bridge (1300-1399), Bus (1400-1499), Persistence (1500-1599), Governance (1600-1699),
  Generic (1900-1999).
- **`ErrorClassifier` trait**: Separates error *classification* (retryable? severity? code?)
  from error *content*. The conductor and bridge layers use this to decide retry, log, escalate,
  or drop. Trait requires `Send + Sync + Debug`.
- **`PvResult<T>` alias**: `Result<T, PvError>` used crate-wide. Every fallible function
  returns this type (P02: `?` operator, never panic).
- **Structured variants**: Bridge errors carry `service` + `url`/`status`/`reason`. Validation
  errors carry `field` + `value`. Never stringly-typed "something went wrong".

### Types to Implement

```rust
/// Convenience alias for Result<T, PvError>.
pub type PvResult<T> = Result<T, PvError>;

/// Unified error type for ORAC sidecar (thiserror-derived).
#[derive(Debug, thiserror::Error)]
pub enum PvError {
    // Config (1000-1099)
    #[error("[PV-1000] config load failed: {0}")]     ConfigLoad(String),
    #[error("[PV-1001] config validation: {0}")]       ConfigValidation(String),

    // Validation (1100-1199)
    #[error("[PV-1100] non-finite value: {field} = {value}")]
    NonFinite { field: &'static str, value: f64 },
    #[error("[PV-1101] out of range: {field} = {value} (expected {min}..{max})")]
    OutOfRange { field: &'static str, value: f64, min: f64, max: f64 },
    #[error("[PV-1102] empty string: {field}")]        EmptyString { field: &'static str },
    #[error("[PV-1103] string too long: {field} ({len} > {max})")]
    StringTooLong { field: &'static str, len: usize, max: usize },
    #[error("[PV-1104] invalid characters in {field}: {reason}")]
    InvalidChars { field: &'static str, reason: String },

    // Field (1200-1299)
    #[error("[PV-1200] sphere not found: {0}")]        SphereNotFound(String),
    #[error("[PV-1201] sphere already registered: {0}")] SphereAlreadyRegistered(String),
    #[error("[PV-1202] sphere cap reached ({0})")]     SphereCapReached(usize),
    #[error("[PV-1203] field computation error: {0}")] FieldComputation(String),

    // Bridge (1300-1399)
    #[error("[PV-1300] bridge unreachable: {service} at {url}")]
    BridgeUnreachable { service: String, url: String },
    #[error("[PV-1301] bridge error: {service} returned {status}")]
    BridgeError { service: String, status: u16 },
    #[error("[PV-1302] bridge parse error: {service}: {reason}")]
    BridgeParse { service: String, reason: String },
    #[error("[PV-1303] bridge consent denied: {service} for sphere {sphere}")]
    BridgeConsentDenied { service: String, sphere: String },

    // Bus (1400-1499)
    #[error("[PV-1400] bus socket error: {0}")]        BusSocket(String),
    #[error("[PV-1401] bus protocol error: {0}")]      BusProtocol(String),
    #[error("[PV-1402] bus task not found: {0}")]       BusTaskNotFound(String),
    #[error("[PV-1403] cascade rate limit exceeded: {per_minute} per minute")]
    CascadeRateLimit { per_minute: u32 },

    // Persistence (1500-1599)
    #[error("[PV-1500] database error: {0}")]          Database(String),
    #[error("[PV-1501] snapshot error: {0}")]           Snapshot(String),

    // Governance (1600-1699)
    #[error("[PV-1600] proposal not found: {0}")]      ProposalNotFound(String),
    #[error("[PV-1601] voting closed: {0}")]            VotingClosed(String),
    #[error("[PV-1602] quorum not reached: {votes}/{needed}")]
    QuorumNotReached { votes: usize, needed: usize },

    // Generic (1900-1999)
    #[error("[PV-1900] io error: {0}")]                Io(#[from] std::io::Error),
    #[error("[PV-1901] json error: {0}")]              Json(#[from] serde_json::Error),
    #[error("[PV-1999] internal error: {0}")]           Internal(String),
}

/// Error severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity { Info, Warning, Error, Critical }

/// Error classification for retry and alerting decisions.
pub trait ErrorClassifier: Send + Sync + std::fmt::Debug {
    fn is_retryable(&self) -> bool;
    fn severity(&self) -> ErrorSeverity;
    fn code(&self) -> u16;
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `ErrorClassifier::is_retryable` | `fn is_retryable(&self) -> bool` | True for transient errors (bridge, socket, DB, IO) |
| `ErrorClassifier::severity` | `fn severity(&self) -> ErrorSeverity` | Categorise for logging/alerting |
| `ErrorClassifier::code` | `fn code(&self) -> u16` | Numeric code in PV-NNNN range |

### Tests

| Test | Validates |
|------|-----------|
| `error_display_includes_code` | Display output starts with `[PV-NNNN]` |
| `bridge_unreachable_is_retryable` | `BridgeUnreachable` returns `is_retryable() == true` |
| `validation_not_retryable` | `NonFinite`, `EmptyString` return `is_retryable() == false` |
| `field_computation_is_critical` | `FieldComputation` has `ErrorSeverity::Critical` |
| `bridge_consent_is_warning` | `BridgeConsentDenied` has `ErrorSeverity::Warning` |
| `error_code_uniqueness` | All variants produce distinct codes |
| `io_error_from_conversion` | `std::io::Error` converts via `#[from]` |
| `json_error_from_conversion` | `serde_json::Error` converts via `#[from]` |
| `pv_result_type_alias` | `PvResult<u32>` compiles and works with `?` |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- error hierarchy
- `.claude/patterns.json` P02 (error propagation via `?`)
- `ai_docs/ANTI_PATTERNS.md` A1 (unwrap), A2 (expect)
- `ai_docs/GOLD_STANDARD_PATTERNS.md` P3 (Result everywhere)

---

## m03 — Configuration

**Source:** `src/m1_core/m03_config.rs`
**LOC:** ~540
**Depends on:** m02 (`PvError`, `PvResult`)
**Hot-Swap:** DROP-IN from PV2 M03

### Design Decisions

- **Figment-based loading**: `Figment` merges TOML files with environment variable overlay.
  Priority: `config/default.toml` -> `config/production.toml` -> `PV2_*` env vars.
- **`#[serde(default)]` on all sections**: Missing config keys get sensible defaults. No
  required fields beyond `server.port > 0`.
- **Validation-on-load**: `PvConfig::load()` calls `validate()` before returning. Invalid
  configs are caught at startup, not at first use.
- **10 config sections**: `ServerConfig`, `FieldConfig`, `SphereConfig`, `CouplingConfig`,
  `LearningConfig`, `BridgesConfig`, `ConductorConfig`, `IpcConfig`, `PersistenceConfig`,
  `GovernanceConfig`. Each has its own `Default` impl with PV2-validated production values.

### Types to Implement

```rust
/// Complete ORAC configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PvConfig {
    pub server: ServerConfig,       // port, bind_addr, body_limit_bytes
    pub field: FieldConfig,         // tick_interval, dt, thresholds, warmup
    pub sphere: SphereConfig,       // max_count, memory cap, decay, newcomer
    pub coupling: CouplingConfig,   // default_weight, exponent, auto_scale
    pub learning: LearningConfig,   // LTP, LTD, burst/newcomer multipliers
    pub bridges: BridgesConfig,     // k_mod bounds, poll intervals
    pub conductor: ConductorConfig, // PI gains, breathing blend
    pub ipc: IpcConfig,             // socket path, permissions, connections
    pub persistence: PersistenceConfig, // snapshot interval, WAL timeout
    pub governance: GovernanceConfig,   // voting window, quorum, max proposals
}

/// HTTP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,              // default: 8132
    pub bind_addr: String,      // default: "127.0.0.1"
    pub body_limit_bytes: usize, // default: 65536
}

/// IPC bus configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcConfig {
    pub socket_path: String,       // default: "/run/user/1000/pane-vortex-bus.sock"
    pub socket_permissions: u32,   // default: 0o700
    pub max_connections: usize,    // default: 50
    pub event_buffer_size: usize,  // default: 256
    pub task_ttl_secs: u64,        // default: 3600
    pub cascade_rate_limit: u32,   // default: 10
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `PvConfig::load` | `fn load() -> PvResult<Self>` | Load from default paths + env overlay |
| `PvConfig::from_path` | `fn from_path(path: &str) -> PvResult<Self>` | Load from specific TOML file |
| `PvConfig::validate` | `fn validate(&self) -> PvResult<()>` | Validate all invariants |

### Tests

| Test | Validates |
|------|-----------|
| `default_config_has_correct_port` | `ServerConfig::default().port == 8132` |
| `default_config_has_correct_tick_interval` | `FieldConfig::default().tick_interval_secs == 5` |
| `default_config_has_correct_sphere_cap` | `SphereConfig::default().max_count == 200` |
| `default_config_has_correct_dt` | `FieldConfig::default().kuramoto_dt == 0.01` |
| `validation_rejects_zero_port` | `port: 0` returns `ConfigValidation` |
| `validation_rejects_zero_tick_interval` | `tick_interval_secs: 0` returns error |
| `validation_rejects_negative_dt` | `kuramoto_dt: -1.0` returns error |
| `validation_rejects_inverted_k_mod_bounds` | `k_mod_min >= k_mod_max` returns error |
| `validation_rejects_quorum_out_of_range` | `quorum_threshold: 0.0` returns error |
| `env_overlay_overrides_toml` | `PV2_SERVER_PORT=9999` changes port |
| `ipc_default_socket_path` | Default is `/run/user/1000/pane-vortex-bus.sock` |
| `bridges_default_poll_intervals` | SYNTHEX=6, Nexus=12, ME=12 ticks |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- config sections
- `config/default.toml`, `config/dev.toml`, `config/prod.toml`
- `ai_docs/GOLD_STANDARD_PATTERNS.md` P3 (Result everywhere)

---

## m04 — Constants

**Source:** `src/m1_core/m04_constants.rs`
**LOC:** ~280
**Depends on:** None (leaf module, uses only `std::f64::consts`)
**Hot-Swap:** DROP-IN from PV2 M04

### Design Decisions

- **Compile-time only**: All values are `pub const`. Runtime-configurable values live in m03.
  This module is the single source of truth for magic numbers.
- **Category grouping**: Constants are grouped by subsystem: tick timing, Hebbian learning,
  coupling network, field thresholds, R target dynamics, conductor, K modulation bounds,
  sphere limits, sphere dynamics, ghost trace, persistence, network.
- **Mathematical constants re-exported**: `TWO_PI = TAU` for convenience.
- **Ordered thresholds**: Tests enforce `R_LOW < R_COHERENCE <= R_HIGH < R_TARGET_BASE` and
  `K_MOD_MIN < K_MOD_BUDGET_MIN < K_MOD_BUDGET_MAX < K_MOD_MAX`.

### Types to Implement

No types -- this module contains only `pub const` values.

### Key Constants

| Constant | Value | Category |
|----------|-------|----------|
| `TICK_INTERVAL_SECS` | 5 | Tick timing |
| `COUPLING_STEPS_PER_TICK` | 15 | Tick timing |
| `KURAMOTO_DT` | 0.01 | Tick timing |
| `HEBBIAN_LTP` | 0.01 | Learning |
| `HEBBIAN_LTD` | 0.002 | Learning |
| `HEBBIAN_BURST_MULTIPLIER` | 3.0 | Learning |
| `HEBBIAN_NEWCOMER_MULTIPLIER` | 2.0 | Learning |
| `HEBBIAN_WEIGHT_FLOOR` | 0.15 | Learning |
| `DEFAULT_WEIGHT` | 0.18 | Coupling |
| `WEIGHT_EXPONENT` | 2.0 | Coupling |
| `PHASE_GAP_THRESHOLD` | pi/3 | Field thresholds |
| `SYNC_THRESHOLD` | 0.5 | Field thresholds |
| `TUNNEL_THRESHOLD` | 0.8 | Field thresholds |
| `R_TARGET_BASE` | 0.93 | R dynamics |
| `CONDUCTOR_GAIN` | 0.15 | Conductor |
| `K_MOD_MIN` / `K_MOD_MAX` | -0.5 / 1.5 | K bounds |
| `SPHERE_CAP` | 200 | Limits |
| `MEMORY_MAX_COUNT` | 500 | Limits |
| `GHOST_MAX` | 20 | Limits |
| `DECAY_PER_STEP` | 0.995 | Sphere dynamics |
| `SNAPSHOT_INTERVAL` | 60 | Persistence |

### Tests

| Test | Validates |
|------|-----------|
| `constants_are_positive_where_expected` | All timing, learning, coupling, threshold consts > 0 |
| `r_thresholds_ordered` | `R_LOW < R_COHERENCE <= R_HIGH < R_TARGET_BASE` |
| `k_mod_bounds_ordered` | `K_MOD_MIN < K_MOD_BUDGET_MIN < K_MOD_BUDGET_MAX < K_MOD_MAX` |
| `phase_gap_thresholds_ordered` | `PHASE_GAP_FINE < PHASE_GAP_MINIMUM < PHASE_GAP_THRESHOLD` |
| `decay_per_step_in_unit` | `0.0 < DECAY_PER_STEP < 1.0` |
| `sphere_cap_positive` | `SPHERE_CAP > 0` |
| `two_pi_equals_tau` | `TWO_PI == std::f64::consts::TAU` |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- constant inventory
- Consumed by: m06 (validation clamp ranges), L4 (coupling/Hebbian), L6 (conductor/tick)
- `.claude/patterns.json` P06 (NaN guard uses these thresholds), P08 (frequency clamp range)
- PV2 source: `pane-vortex-v2/src/m1_foundation/m04_constants.rs`

---

## m05 — Core Traits

**Source:** `src/m1_core/m05_traits.rs`
**LOC:** ~250
**Depends on:** m01 (`OrderParameter`), m02 (`PvResult`)
**Hot-Swap:** DROP-IN from PV2 M05

### Design Decisions

- **Dependency inversion**: Traits defined in L1, implemented in higher layers. L4 implements
  `Oscillator` and `Learnable`. L5 implements `Bridgeable`. L6 implements `Persistable`.
  L7 implements `FieldObserver`. This ensures L1 has zero upward imports.
- **`&self` only (C2)**: All trait methods take `&self`. Interior mutability via
  `parking_lot::RwLock` is the implementor's responsibility. This enables `Arc<dyn Trait>`
  sharing across async tasks.
- **All traits `Send + Sync + Debug`**: Required for `Arc<dyn Trait>` usage in tokio tasks.
- **Object-safe**: Every trait is verified to be object-safe via `fn _accepts(_: &dyn Trait) {}`
  tests. No associated types, no `Self: Sized` bounds.
- **`Consentable` trait**: Encodes the Habitat philosophy -- the field modulates, it does not
  command. Every sphere can declare receptivity, opt out of modulation types, and set max K
  adjustment limits.

### Types to Implement

```rust
/// A phase oscillator in the Kuramoto field.
pub trait Oscillator: Send + Sync + std::fmt::Debug {
    fn phase(&self) -> f64;
    fn frequency(&self) -> f64;
    fn step(&self, coupling_force: f64) -> PvResult<()>;
    fn reset(&self) -> PvResult<()>;
}

/// A connection that supports Hebbian learning (STDP).
pub trait Learnable: Send + Sync + std::fmt::Debug {
    fn ltp(&self, amount: f64) -> PvResult<()>;
    fn ltd(&self, amount: f64) -> PvResult<()>;
    fn weight(&self) -> f64;
    fn decay(&self, factor: f64) -> PvResult<()>;
}

/// An external service bridge.
pub trait Bridgeable: Send + Sync + std::fmt::Debug {
    fn service_name(&self) -> &str;
    fn poll(&self) -> PvResult<f64>;
    fn post(&self, payload: &[u8]) -> PvResult<()>;
    fn health(&self) -> PvResult<bool>;
    fn is_stale(&self, current_tick: u64) -> bool;
}

/// An entity that can consent to or refuse external modulation.
pub trait Consentable: Send + Sync + std::fmt::Debug {
    fn receptivity(&self) -> f64;
    fn has_opted_out(&self, modulation: &str) -> bool;
    fn consent_posture(&self) -> ConsentPosture;
}

/// Summary of a sphere's consent state.
#[derive(Debug, Clone)]
pub struct ConsentPosture {
    pub receptivity: f64,
    pub opt_outs: Vec<String>,
    pub max_k_adj: Option<f64>,
}

/// An entity that can be snapshot'd and restored.
pub trait Persistable: Send + Sync + std::fmt::Debug {
    fn snapshot(&self) -> PvResult<Vec<u8>>;
    fn restore(&self, data: &[u8]) -> PvResult<()>;
    fn migrate(&self) -> PvResult<()>;
}

/// Observer that receives field state updates each tick.
pub trait FieldObserver: Send + Sync + std::fmt::Debug {
    fn on_tick(&self, tick: u64, order: &OrderParameter) -> PvResult<()>;
}
```

### Tests

| Test | Validates |
|------|-----------|
| `oscillator_is_object_safe` | `&dyn Oscillator` compiles |
| `learnable_is_object_safe` | `&dyn Learnable` compiles |
| `bridgeable_is_object_safe` | `&dyn Bridgeable` compiles |
| `consentable_is_object_safe` | `&dyn Consentable` compiles |
| `persistable_is_object_safe` | `&dyn Persistable` compiles |
| `field_observer_is_object_safe` | `&dyn FieldObserver` compiles |
| `consent_posture_creation` | Fields set correctly, opt_outs populated |
| `consent_posture_no_opt_outs` | Empty opt_outs, None max_k_adj |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- trait hierarchy
- `ai_docs/GOLD_STANDARD_PATTERNS.md` P2 (interior mutability), P7 (owned returns through RwLock)
- `ai_docs/ANTI_PATTERNS.md` A6 (&mut self on shared traits)
- `.claude/patterns.json` P21 (consent gate stub in every bridge)
- Implementors: L4 (Oscillator, Learnable), L5 (Bridgeable, Consentable), L6 (Persistable)

---

## m06 — Validation

**Source:** `src/m1_core/m06_validation.rs`
**LOC:** ~370
**Depends on:** m02 (`PvError`, `PvResult`), m04 (constants for clamp ranges)
**Hot-Swap:** DROP-IN from PV2 M06

### Design Decisions

- **Boundary validation**: Every external input is validated at the system boundary (API
  handlers, IPC frames, config loading). Interior code can assume validated inputs.
- **Phase wrapping via `rem_euclid(TAU)` (P01)**: Negative or large phase values are wrapped
  into [0, 2pi). NaN/infinity rejected outright.
- **Clamp, don't reject**: Numeric values that are out of range but finite are clamped to the
  valid range. Only non-finite values produce errors.
- **`chars().take()` for string truncation (P11)**: Never byte-slice. Multi-byte UTF-8
  characters are preserved.
- **Pane ID character allowlist**: ASCII alphanumeric plus `._:-`. Prevents injection and path
  traversal.
- **Distinct max lengths**: pane_id=128, persona=256, tool_name=128, summary=1024 characters.

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `validate_phase` | `fn validate_phase(phase: f64) -> PvResult<f64>` | Wrap to [0, 2pi), reject NaN/Inf (P01, P06) |
| `validate_frequency` | `fn validate_frequency(freq: f64) -> PvResult<f64>` | Clamp to [0.001, 10.0] (P08) |
| `validate_strength` | `fn validate_strength(s: f64) -> PvResult<f64>` | Clamp to [0.0, 2.0] |
| `validate_weight` | `fn validate_weight(w: f64) -> PvResult<f64>` | Clamp to [HEBBIAN_WEIGHT_FLOOR, 1.0] |
| `validate_receptivity` | `fn validate_receptivity(r: f64) -> PvResult<f64>` | Clamp to [0.0, 1.0] |
| `validate_k_mod` | `fn validate_k_mod(k: f64) -> PvResult<f64>` | Clamp to [K_MOD_MIN, K_MOD_MAX] |
| `validate_pane_id` | `fn validate_pane_id(id: &str) -> PvResult<()>` | 1-128 chars, ASCII+`._:-` |
| `validate_persona` | `fn validate_persona(p: &str) -> PvResult<()>` | 1-256 chars, UTF-8 |
| `validate_tool_name` | `fn validate_tool_name(n: &str) -> PvResult<()>` | 1-128 chars |
| `validate_summary` | `fn validate_summary(s: &str) -> PvResult<()>` | 0-1024 chars |
| `truncate_string` | `fn truncate_string(s: &str, max: usize) -> String` | UTF-8 safe truncation (P11) |

### Tests

| Test | Validates |
|------|-----------|
| `validate_phase_wraps_negative` | `-0.1` wraps to `TAU - 0.1` |
| `validate_phase_wraps_large` | `TAU + 1.0` wraps to `1.0` |
| `validate_phase_zero` | `0.0` passes through unchanged |
| `validate_phase_pi` | `pi` passes through unchanged |
| `validate_phase_rejects_nan` | `NaN` returns `NonFinite` error |
| `validate_phase_rejects_infinity` | `INFINITY` returns error |
| `validate_phase_rejects_neg_infinity` | `NEG_INFINITY` returns error |
| `validate_frequency_clamps_low` | `0.0` clamps to `0.001` |
| `validate_frequency_clamps_high` | `100.0` clamps to `10.0` |
| `validate_pane_id_empty` | Empty string returns `EmptyString` |
| `validate_pane_id_too_long` | 129 chars returns `StringTooLong` |
| `validate_pane_id_invalid_chars` | Spaces/unicode rejected |
| `validate_pane_id_valid` | `"fleet-alpha:left"` passes |
| `validate_persona_empty` | Empty returns `EmptyString` |
| `validate_persona_unicode` | Multi-byte chars counted correctly |
| `truncate_string_utf8_safe` | Multi-byte chars not split |
| `validate_k_mod_clamps_to_bounds` | Values clamped to [-0.5, 1.5] |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- input validation boundaries
- `.claude/patterns.json` P01 (phase wrapping), P06 (NaN guard), P08 (frequency clamp), P11 (chars not bytes)
- `ai_docs/ANTI_PATTERNS.md` A1 (no unwrap in validators)
- Consumed by: L3 (hook request validation), L2 (IPC frame validation)

---

## field_state — Sidecar-Native Field State

**Source:** `src/m1_core/field_state.rs`
**LOC:** ~300
**Depends on:** m01 (`PaneId`, `PaneSphere`, `OrderParameter`, `FleetMode`, `RTrend`, `FieldAction`, `DecisionRecord`)
**Hot-Swap:** NEW (sidecar-native, not in PV2)

### Design Decisions

- **Observer, not owner**: ORAC's `AppState` caches observed state from the PV2 daemon. It does
  not own the authoritative field -- PV2 does. This is a fundamental architectural distinction.
- **`FieldState::compute()`**: Calculates Kuramoto order parameter `r * e^(i*psi) = (1/N) * sum(e^(i*theta_j))`
  from cached sphere phases. Used for local diagnostics; the daemon's computation is authoritative.
- **`SharedState = Arc<RwLock<AppState>>`**: Thread-safe shared state with `parking_lot::RwLock`
  (P02). Lock ordering: always `AppState` before `BusState` (deadlock prevention from PV2).
- **Warmup phase**: First 10 ticks after startup use reduced dynamics. `is_warming_up()` gates
  conductor decisions.
- **R-history ring buffer**: `VecDeque<f64>` with capacity 60, used for trend analysis. Manually
  managed with `pop_front()` / `push_back()` (P10).
- **`Harmonics` for sub-cluster analysis**: Tracks per-cluster order parameters, chimera
  detection flag, and cluster count. Populated by field computation.

### Types to Implement

```rust
/// Harmonic decomposition of the field (per-cluster order parameters).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Harmonics {
    pub clusters: Vec<OrderParameter>,
    pub chimera_detected: bool,
    pub cluster_count: usize,
}

/// Cached field state snapshot, updated from PV2 daemon ticks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldState {
    pub order: OrderParameter,
    pub order_parameter: OrderParameter, // PV2 compat alias
    pub tick: u64,
    pub fleet_mode: FleetMode,
    pub r_trend: RTrend,
    pub recent_decisions: Vec<DecisionRecord>,
    pub harmonics: Harmonics,
}

/// A field-level decision produced by the conductor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDecision {
    pub action: FieldAction,
    pub k_delta: f64,
    pub reason: String,
}

/// ORAC sidecar application state (observer, not authoritative).
#[derive(Debug)]
pub struct AppState {
    pub spheres: HashMap<PaneId, PaneSphere>,
    pub field: FieldState,
    pub tick: u64,
    pub started_at: f64,
    pub r_target_override: Option<f64>,
    pub divergence_ema: f64,
    pub coherence_ema: f64,
    pub divergence_cooldown: u32,
    pub prev_decision_action: FieldAction,
    pub r_history: VecDeque<f64>,
    warmup_remaining: u32,
}

/// Thread-safe shared application state.
pub type SharedState = Arc<RwLock<AppState>>;
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `FieldState::compute` | `fn compute(spheres: &HashMap<PaneId, PaneSphere>, tick: u64) -> Self` | Kuramoto order parameter from sphere phases |
| `FieldDecision::recovering` | `fn recovering(reason: impl Into<String>) -> Self` | Create recovering decision |
| `FieldDecision::stable` | `fn stable(reason: impl Into<String>) -> Self` | Create stable decision |
| `AppState::is_warming_up` | `fn is_warming_up(&self) -> bool` | Whether in warmup phase |
| `AppState::tick_warmup` | `fn tick_warmup(&mut self)` | Advance warmup counter |
| `AppState::push_r` | `fn push_r(&mut self, r: f64)` | Push R into history ring buffer (P10) |
| `new_shared_state` | `fn new_shared_state() -> SharedState` | Create new `Arc<RwLock<AppState>>` |

### Tests

| Test | Validates |
|------|-----------|
| `field_state_default` | Default has tick=0, empty decisions |
| `field_state_compute_empty` | Empty sphere map produces correct tick, default order |
| `field_decision_default_is_stable` | Default action is `Stable`, k_delta is 0.0 |
| `field_decision_recovering` | `recovering()` sets action to `Recovering` |
| `app_state_default` | Empty spheres, tick=0, warming up |
| `app_state_warmup` | After `WARMUP_TICKS` calls to `tick_warmup()`, `is_warming_up()` returns false |
| `app_state_push_r` | Ring buffer caps at 60 entries |
| `shared_state_creation` | `new_shared_state()` produces usable `Arc<RwLock>` |
| `harmonics_default` | Default has empty clusters, no chimera |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] -- sidecar vs daemon state ownership
- `.claude/patterns.json` P10 (VecDeque for logs)
- `ai_docs/GOLD_STANDARD_PATTERNS.md` P2 (interior mutability), P4 (scoped lock guards)
- Consumed by: L2 (IPC event updates), L3 (hook handlers read field state), L6 (conductor decisions)
- Lock ordering invariant: AppState before BusState (see `ai_docs/layers/L2_WIRE.md`)
