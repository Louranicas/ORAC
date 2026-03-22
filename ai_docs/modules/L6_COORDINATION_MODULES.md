---
title: "Layer 6: Coordination — Module Documentation"
date: 2026-03-22
tags: [modules, coordination, L6, orac-sidecar]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L6
modules: [m27, m28, m29, m30, m31]
---

# Layer 6: Coordination (m27-m31)

> Orchestration substrate — conductor breathing, cascade handoffs, tick engine,
> WASM plugin bridge, memory aggregation.
> **Target LOC:** ~3,000 | **Target tests:** 60+
> **Source:** adapt from PV2 M31/M35, drop-in M33/M21, m30 new | **Phase:** 3

---

## Overview

Layer 6 is the coordination heartbeat of the ORAC sidecar. It contains the
conductor (PI controller for field breathing rhythm), cascade handoff protocol
(sphere mitosis for context transfer), tick orchestrator (the main loop body),
WASM plugin bridge (bidirectional IPC with swarm-orchestrator), and memory
manager (pruning and fleet-wide aggregation).

**Feature gate:** None (always on). Orchestrates whatever layers are compiled in.

**Critical invariants (non-negotiable):**

1. **Lock ordering: `AppState` before `BusState`** — deadlock prevention. If both locks are needed, acquire `AppState` first, extract values, drop the guard, then acquire `BusState`.
2. **`tick_once()` is the heartbeat** — called by the async tick timer, never by HTTP handlers. HTTP handlers read cached state.
3. **Snapshot persistence** — JSON every 60 ticks + on SIGTERM. Restores on startup.
4. **Memory pruning** — removes entries with activation < 0.05 every 200 ticks (m04_constants).
5. **Conductor is advisory** — ORAC observes PV2 field state; it does not own the authoritative `k_modulation`. Recommendations may be forwarded to PV2 via IPC.

**Design constraints:**
- Depends on: `m1_core`, `m2_wire`, `m4_intelligence` (optional, feature-gated), `m5_bridges` (optional)
- Conductor uses multiplicative bridge composition, not additive
- Cascade rate limit: max 10 per minute, max 50 pending
- WASM bridge protocol: FIFO inbound, ring-buffer outbound (1000 line cap)

---

## m27 — Conductor

**Source:** `src/m6_coordination/m27_conductor.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types` (`FieldAction`, `PaneId`), `m04_constants`, `field_state` (`AppState`, `FieldDecision`)
**Hot-Swap:** adapt from PV2 M31

### Design Decisions

- **PI controller** — proportional-integral control for field breathing rhythm. Gain and blend parameters are tunable. Integral accumulator prevents steady-state error.
- **Advisory only in ORAC** — the sidecar observes PV2 field state and computes suggestions. The authoritative `k_modulation` lives in PV2. Conductor recommendations are forwarded via IPC or used for local analytics.
- **Dynamic r_target** — priority ordering: (1) governance override `r_target_override`, (2) fleet-size-based: 0.93 for small/medium, 0.85 for large (>50 spheres). Fleet-negotiated `preferred_r` blending is deferred to PV2.
- **Multiplicative bridge composition** — bridge adjustments compose as `k * thermal_adj * fitness_adj`, not `k + thermal_adj + fitness_adj`. This preserves the scale invariant.
- **EMA-smoothed signals (Phase 2)** — divergence and coherence signals will be EMA-weighted in Phase 2 build. Currently using raw error.
- **No interior mutability** — `Conductor` is `Clone + Send + Sync` with no `RwLock`. The integral accumulator is mutated via `&mut self` (the tick loop owns it).

### Types to Implement

```rust
/// PI breathing controller for the Kuramoto field.
///
/// In ORAC this is an advisory controller — the authoritative
/// `k_modulation` lives in the PV2 daemon.
///
/// # Thread Safety
/// `Conductor` is `Send + Sync` (no interior mutability).
#[derive(Debug, Clone)]
pub struct Conductor {
    /// Proportional gain for the PI controller.
    gain: f64,                    // Default: CONDUCTOR_GAIN (0.15)
    /// Fraction of emergent signal blended into output (0.0-1.0).
    breathing_blend: f64,         // Default: EMERGENT_BLEND (0.3)
    /// Integral accumulator for the PI controller.
    integral: f64,                // Default: 0.0
}

/// A field decision produced by the conductor.
/// Already defined in `field_state.rs` — used here, not re-defined.
/// Contains: action (FieldAction), k_delta (f64), reason (String).
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `new()` | `-> Self` | Default conductor from `m04_constants` |
| `with_params()` | `(gain: f64, breathing_blend: f64) -> Self` | Custom gain (clamped 0.01-1.0), blend (0.0-1.0) |
| `r_target()` | `(state: &AppState) -> f64` | Dynamic r target: governance > fleet-size-based |
| `decide()` | `(&self, state: &AppState) -> FieldDecision` | Main entry point: read state, produce advisory decision |
| `gain()` | `&self -> f64` | Read gain (`const fn`) |
| `breathing_blend()` | `&self -> f64` | Read blend (`const fn`) |
| `reset_integral()` | `(&mut self)` | Zero the integral accumulator |

**`decide()` algorithm:**
```text
1. If warming up -> FieldDecision::stable("warming up")
2. If spheres < MIN_SPHERES_FOR_BREATHING (3) -> stable("insufficient spheres")
3. Compute error = r_target - r
4. Classify error -> FieldAction:
   - divergence_cooldown > 0 -> Recovering
   - r > 0.99 -> OverSynchronized
   - error > R_RISING_THRESHOLD -> NeedsCoherence
   - error < R_FALLING_THRESHOLD -> NeedsDivergence
   - else -> Stable
5. k_delta = error * gain, clamped to K_MOD_BUDGET bounds
6. Return FieldDecision { action, k_delta, reason }
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `new_default_params` | unit | Gain = `CONDUCTOR_GAIN`, blend = `EMERGENT_BLEND` |
| `with_params_clamp` | unit | Out-of-range gain/blend are clamped |
| `r_target_governance_override` | unit | Override takes priority, clamped 0.5-0.99 |
| `r_target_small_fleet` | unit | < 50 spheres -> R_TARGET_BASE (0.93) |
| `r_target_large_fleet` | unit | > 50 spheres -> R_TARGET_LARGE_FLEET (0.85) |
| `decide_warmup_stable` | unit | During warmup, returns stable decision |
| `decide_few_spheres_stable` | unit | < 3 spheres returns stable |
| `decide_over_synchronized` | unit | r > 0.99 -> `OverSynchronized` action |
| `decide_needs_coherence` | unit | Low r -> `NeedsCoherence` |
| `decide_needs_divergence` | unit | High r -> `NeedsDivergence` |
| `decide_recovering` | unit | Cooldown > 0 -> `Recovering` |
| `k_delta_clamped` | unit | Output stays within K_MOD_BUDGET bounds |
| `reset_integral` | unit | `reset_integral()` zeros accumulator |
| `decide_integration` | integration | Multi-tick decision sequence with r_history |

### Cross-References

- `m04_constants::CONDUCTOR_GAIN` (0.15), `EMERGENT_BLEND` (0.3)
- `m04_constants::R_TARGET_BASE` (0.93), `R_TARGET_LARGE_FLEET` (0.85)
- `m04_constants::K_MOD_BUDGET_MIN` (0.85), `K_MOD_BUDGET_MAX` (1.15)
- `field_state::AppState` — the cached state the conductor reads from
- `field_state::FieldDecision` — the output type
- `m29_tick::tick_once()` — calls `conductor.decide()` each tick
- `m22_synthex_bridge` — thermal adjustment feeds bridge composition
- `[[Pane-Vortex — Fleet Coordination Daemon]]`

---

## m28 — Cascade

**Source:** `src/m6_coordination/m28_cascade.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types` (`now_secs`, `PaneId`), `m02_error_handling` (`PvError`, `PvResult`)
**Hot-Swap:** drop-in from PV2 M33

### Design Decisions

- **Sphere mitosis (SYS-1)** — cascade handoff transfers phase + coupling weights to the target sphere. This is fork-and-continue, not amputation. The source retains its state; the target inherits context.
- **Rate limiting** — max 10 cascades per minute per tracker instance. Prevents cascade storms during fleet instability.
- **Depth tracking** — auto-summarizes brief at depth > 3. This compresses cascaded context to prevent unbounded growth.
- **Brief truncation** — max 4096 characters. Longer briefs are truncated with `[... truncated ...]` marker.
- **Markdown fallback** — non-bus-aware recipients receive a markdown-formatted brief with From/To/Depth metadata.
- **Rejection support (NA-P-7)** — targets can reject cascades with a reason string. Rejected cascades remain in the tracker for audit.

### Types to Implement

```rust
/// A cascade handoff between two fleet tabs.
#[derive(Debug, Clone)]
pub struct CascadeHandoff {
    /// Source sphere initiating the cascade.
    pub source: PaneId,
    /// Target sphere receiving the cascade.
    pub target: PaneId,
    /// Markdown brief describing the work context.
    pub brief: String,                    // Max 4096 chars, truncated on construction
    /// Unix timestamp when the cascade was dispatched.
    pub dispatched_at: f64,
    /// Cascade chain depth (1 = original, 2+ = re-cascade).
    pub depth: u32,
    /// Whether the target has acknowledged this cascade.
    pub acknowledged: bool,
    /// Whether the target has rejected this cascade.
    pub rejected: bool,
    /// Rejection reason (if rejected).
    pub rejection_reason: Option<String>,
}

/// Tracks cascade handoffs with rate limiting and depth management.
#[derive(Debug)]
pub struct CascadeTracker {
    /// Active cascades (pending + resolved).
    cascades: VecDeque<CascadeHandoff>,   // Bounded by MAX_PENDING_CASCADES (50)
    /// Cascade count in current rate window.
    window_count: u32,
    /// Start of current rate window (Unix timestamp).
    window_start: f64,
    /// Maximum cascade depth before auto-rejection.
    max_depth: u32,                       // Default: 10
}
```

**Constants:**
```rust
const MAX_CASCADES_PER_MINUTE: u32 = 10;
const RATE_WINDOW_SECS: f64 = 60.0;
const MAX_PENDING_CASCADES: usize = 50;
const AUTO_SUMMARIZE_DEPTH: u32 = 3;
const MAX_BRIEF_CHARS: usize = 4096;
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `CascadeHandoff::new()` | `(source, target, brief) -> Self` | Create handoff, truncate brief |
| `re_cascade()` | `(&self, new_target) -> Self` | Re-cascade with incremented depth, auto-summarize if deep |
| `acknowledge()` | `(&mut self)` | Mark as acknowledged |
| `reject()` | `(&mut self, reason: String)` | Mark as rejected with reason |
| `is_pending()` | `(&self) -> bool` | Neither ack'd nor rejected (`const fn`) |
| `elapsed_secs()` | `(&self) -> f64` | Time since dispatch |
| `needs_summarization()` | `(&self) -> bool` | Depth >= `AUTO_SUMMARIZE_DEPTH` (`const fn`) |
| `fallback_brief()` | `(&self) -> String` | Markdown brief for non-bus-aware recipients |
| `CascadeTracker::new()` | `-> Self` | Default tracker (max_depth=10) |
| `CascadeTracker::with_max_depth()` | `(u32) -> Self` | Custom max depth |
| `CascadeTracker::initiate()` | `(&mut self, source, target, brief) -> PvResult<()>` | Rate-limited cascade dispatch |

**Rate limit algorithm:**
```text
1. Check window expiry: if now - window_start >= 60s, reset window
2. If window_count >= MAX_CASCADES_PER_MINUTE -> PvError::CascadeRateLimit
3. If pending count >= MAX_PENDING_CASCADES -> PvError::BusProtocol
4. Create CascadeHandoff, push to deque, increment window_count
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `new_truncates_brief` | unit | Brief > 4096 chars is truncated |
| `new_preserves_short_brief` | unit | Brief < 4096 chars is unchanged |
| `re_cascade_increments_depth` | unit | Depth goes from N to N+1 |
| `re_cascade_auto_summarize` | unit | Brief is summarized at depth > 3 |
| `acknowledge_sets_flag` | unit | `acknowledged = true` after `acknowledge()` |
| `reject_sets_reason` | unit | `rejected = true`, reason stored |
| `is_pending_default` | unit | New cascade is pending |
| `is_pending_after_ack` | unit | Not pending after ack |
| `fallback_brief_format` | unit | Contains From/To/Depth in markdown |
| `rate_limit_blocks` | unit | 11th cascade in 60s returns error |
| `rate_limit_window_reset` | unit | After 60s, window resets |
| `max_pending_blocks` | unit | 51st pending cascade returns error |
| `with_max_depth_clamp` | unit | `max_depth.max(1)` prevents zero |

### Cross-References

- SYS-1: Cascade handoff as sphere mitosis (fork phase + coupling weights)
- NA-P-7: Cascade rejection support
- `m29_tick` — may trigger cascades based on field decisions
- `m08_bus_types::ClientFrame` — `CascadeHandoff` frame on the bus
- `[[Session 014 — Shared-Context Vault + Distributed Context Cascade]]`

---

## m29 — Tick Orchestrator

**Source:** `src/m6_coordination/m29_tick.rs`
**LOC Target:** ~600
**Depends on:** `field_state` (`AppState`, `FieldDecision`, `FieldState`), `m01_core_types` (`OrderParameter`), `m04_constants`, `m27_conductor::Conductor`
**Hot-Swap:** adapt from PV2 M35

### Design Decisions

- **Observer, not owner** — unlike PV2's tick which owns the field and mutates spheres, ORAC's tick **observes** cached state from PV2 and produces advisory results. No direct sphere mutation occurs.
- **Phased architecture** — 5 phases per tick, with Phase 4 (Hebbian STDP) and Phase 5 (governance actuator) deferred to later build phases.
- **60-tick snapshot cycle** — `should_snapshot` flag is true when `tick % SNAPSHOT_INTERVAL == 0`. The caller (main loop) acts on this flag.
- **Warmup handling** — first `WARMUP_TICKS` (5) ticks after restore have reduced dynamics. `state.is_warming_up()` controls this.
- **Per-phase timing** — each phase is instrumented with `Instant::now()` for performance monitoring. Reported in `TickResult`.
- **Hebbian Phase 2.5** — the tick orchestrator will accept an optional `CouplingNetwork` for local Hebbian STDP when the `intelligence` feature is enabled. Currently a placeholder.

### Types to Implement

```rust
/// Result of a single ORAC tick orchestration.
#[derive(Debug)]
pub struct TickResult {
    /// The recomputed field state snapshot.
    pub field_state: FieldState,
    /// The conductor's advisory decision.
    pub decision: FieldDecision,
    /// Current order parameter.
    pub order_parameter: OrderParameter,
    /// Per-phase timing breakdown.
    pub phase_timings: PhaseTiming,
    /// Total tick duration (milliseconds).
    pub total_ms: f64,
    /// Current tick number.
    pub tick: u64,
    /// Number of spheres in the cached field.
    pub sphere_count: usize,
    /// Whether a local snapshot should be taken.
    pub should_snapshot: bool,
}

/// Per-phase timing metrics (milliseconds).
#[derive(Debug, Default)]
pub struct PhaseTiming {
    /// Phase 2: field state recomputation.
    pub field_state_ms: f64,
    /// Phase 3: conductor decision.
    pub conductor_ms: f64,
    /// Phase 4: Hebbian STDP pass.
    pub hebbian_ms: f64,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `tick_once()` | `(state: &mut AppState, conductor: &Conductor) -> TickResult` | Main loop body |

**`tick_once()` algorithm:**
```text
Phase 1: Advance tick counter, handle warmup
  state.tick += 1
  if warming_up: state.tick_warmup()

Phase 2: Field state recomputation (timed)
  field_state = FieldState::compute(&state.spheres, tick)
  state.push_r(field_state.order.r)
  state.field = field_state.clone()

Phase 3: Conductor advisory decision (timed)
  decision = conductor.decide(state)

Phase 4: Hebbian STDP on local coupling snapshot (placeholder)
  TODO: Phase 2 build — accept Option<&mut CouplingNetwork>
  Requires `intelligence` feature and m15_coupling_network

Phase 5: Governance actuator (placeholder)
  TODO: Phase 4 build — process approved governance proposals

Snapshot decision:
  should_snapshot = tick % SNAPSHOT_INTERVAL == 0  // Every 60 ticks
```

**Future signature (Phase 2 build):**
```rust
/// Future tick_once with optional Hebbian and bridge integration.
pub fn tick_once(
    state: &mut AppState,
    conductor: &Conductor,
    coupling: Option<&mut CouplingNetwork>,  // Phase 2: intelligence feature
    bridges: Option<&BridgeSet>,             // Phase 3: bridges feature
) -> TickResult;
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `tick_result_fields_populated` | unit | All fields non-default after tick |
| `tick_increments_counter` | unit | `result.tick == 1` after first tick |
| `tick_sphere_count` | unit | `sphere_count` matches state |
| `phase_timing_default_zero` | unit | Default `PhaseTiming` is all zeros |
| `phase_timing_non_negative` | unit | All timing fields >= 0.0 |
| `total_ms_non_negative` | unit | `total_ms >= 0.0` |
| `should_snapshot_at_interval` | unit | True at tick 60, 120, 180 |
| `should_snapshot_not_between` | unit | False at ticks 1-59 |
| `warmup_ticks_reduce_dynamics` | unit | Warmup state produces stable decisions |
| `tick_pushes_r_history` | unit | `state.r_history` grows each tick |
| `field_state_updated` | unit | `state.field` is updated in-place |
| `empty_spheres_default_state` | unit | Zero spheres -> default field state |
| `multi_tick_sequence` | integration | 100 ticks produce valid r trajectory |

### Cross-References

- `m04_constants::SNAPSHOT_INTERVAL` (60) — snapshot trigger
- `m04_constants::WARMUP_TICKS` (5) — reduced dynamics window
- `m27_conductor::Conductor` — called each tick for advisory decision
- `field_state::FieldState::compute()` — Kuramoto order parameter calculation
- `m15_coupling_network` — Phase 2 integration (Hebbian STDP)
- `m5_bridges` — Phase 3 integration (bridge k_mod application)
- `[[Session 045 Arena — 12-live-field-analysis]]`

---

## m30 — WASM Bridge

**Source:** `src/m6_coordination/m30_wasm_bridge.rs`
**LOC Target:** ~600
**Depends on:** `m01_core_types`, `m08_bus_types`
**Hot-Swap:** NEW (no PV2 equivalent)

### Design Decisions

- **FIFO inbound, ring outbound** — the WASM plugin writes commands to a FIFO (`/tmp/swarm-commands.pipe`), ORAC reads them. ORAC writes events to a ring file (`/tmp/swarm-events.jsonl`), the WASM plugin reads them.
- **1000-line ring cap (P22)** — the events file is capped at 1000 lines. When full, oldest lines are dropped. This prevents unbounded growth in the shared tmpfs.
- **NDJSON protocol** — both FIFO commands and ring events use newline-delimited JSON. One JSON object per line, no framing.
- **Non-blocking FIFO reads** — `O_NONBLOCK` on the FIFO. `WouldBlock` is not an error — it means no commands pending.
- **Atomic ring writes** — write new events to a temp file, then `rename()` to the ring path. This prevents partial reads by the WASM plugin.
- **Zellij swarm-orchestrator integration** — the WASM plugin at `~/.config/zellij/plugins/swarm-orchestrator.wasm` uses this bridge for bidirectional coordination.

### Types to Implement

```rust
/// A command received from the WASM plugin via FIFO.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WasmCommand {
    /// Request pane status update.
    StatusRequest { pane_id: String },
    /// Dispatch a task to a specific pane.
    TaskDispatch {
        pane_id: String,
        task: String,
        priority: Option<u32>,
    },
    /// Request field state summary.
    FieldQuery,
    /// Request cascade handoff between panes.
    CascadeRequest {
        source: String,
        target: String,
        brief: String,
    },
    /// Heartbeat from the WASM plugin.
    Heartbeat { plugin_version: String },
}

/// An event sent to the WASM plugin via ring file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WasmEvent {
    /// Field state update.
    FieldUpdate {
        r: f64,
        psi: f64,
        tick: u64,
        sphere_count: usize,
    },
    /// Pane status change.
    PaneStatusChange {
        pane_id: String,
        old_status: String,
        new_status: String,
    },
    /// Task completion notification.
    TaskComplete {
        pane_id: String,
        task_id: String,
        outcome: String,
    },
    /// Cascade event (dispatched, ack'd, rejected).
    CascadeEvent {
        cascade_type: String,
        source: String,
        target: String,
        depth: u32,
    },
    /// Bridge health report.
    BridgeHealth {
        service: String,
        healthy: bool,
        consecutive_failures: u32,
    },
}

/// WASM bridge handle for FIFO/ring protocol.
#[derive(Debug)]
pub struct WasmBridge {
    /// Path to the inbound FIFO.
    fifo_path: String,            // "/tmp/swarm-commands.pipe"
    /// Path to the outbound ring file.
    ring_path: String,            // "/tmp/swarm-events.jsonl"
    /// Maximum lines in the ring file.
    ring_cap: usize,              // 1000 (P22)
    /// Buffered events pending write.
    pending_events: parking_lot::Mutex<Vec<WasmEvent>>,
    /// Whether the FIFO has been created.
    fifo_ready: std::sync::atomic::AtomicBool,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `new()` | `-> Self` | Default paths and 1000-line cap |
| `with_paths()` | `(fifo_path, ring_path, ring_cap) -> Self` | Custom config |
| `ensure_fifo()` | `&self -> PvResult<()>` | Create FIFO if missing (`mkfifo`) |
| `read_commands()` | `&self -> PvResult<Vec<WasmCommand>>` | Non-blocking FIFO read, parse NDJSON |
| `emit_event()` | `&self, event: WasmEvent -> PvResult<()>` | Buffer an event for ring write |
| `flush_events()` | `&self -> PvResult<usize>` | Atomic ring write (temp + rename) |
| `ring_line_count()` | `&self -> PvResult<usize>` | Count lines in ring file |
| `truncate_ring()` | `&self -> PvResult<()>` | Drop oldest lines to stay within cap |

**Ring write algorithm:**
```text
1. Read existing ring file lines into Vec<String>
2. Append pending events as NDJSON lines
3. If total > ring_cap, drop oldest (keep last ring_cap lines)
4. Write to temp file: ring_path.tmp
5. rename(ring_path.tmp, ring_path)  // Atomic swap
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `wasm_command_deserialize` | unit | All `WasmCommand` variants parse from JSON |
| `wasm_event_serialize` | unit | All `WasmEvent` variants produce valid NDJSON |
| `ring_cap_enforced` | unit | > 1000 lines triggers truncation |
| `ring_atomic_write` | unit | Temp file + rename pattern |
| `fifo_nonblocking` | unit | `WouldBlock` returns empty vec, not error |
| `heartbeat_roundtrip` | unit | Serialize + deserialize `Heartbeat` |
| `empty_fifo_returns_empty` | unit | No data -> empty command vec |
| `ring_oldest_dropped` | unit | First lines removed, last lines kept |
| `ensure_fifo_idempotent` | integration | Calling twice doesn't error |
| `full_roundtrip` | integration | Write command to FIFO, read event from ring |

### Cross-References

- FIFO: `/tmp/swarm-commands.pipe` (WASM -> ORAC)
- Ring: `/tmp/swarm-events.jsonl` (ORAC -> WASM, 1000 line cap, P22)
- `~/.config/zellij/plugins/swarm-orchestrator.wasm` — the plugin that reads/writes these files
- `m26_blackboard` — pane status data fed into `WasmEvent::PaneStatusChange`
- `m28_cascade` — cascade events fed into `WasmEvent::CascadeEvent`
- `[[Swarm Orchestrator — Complete Reference]]`
- ORAC_MINDMAP.md Branch 7 (WASM Bridge)

---

## m31 — Memory Manager

**Source:** `src/m6_coordination/m31_memory_manager.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types` (`ActivationZones`, `PaneId`, `PaneSphere`, `SphereMemory`, `Point3D`), `m04_constants`
**Hot-Swap:** drop-in from PV2 M21

### Design Decisions

- **Pruning at activation < 0.05** — `TRACE_PRUNE_THRESHOLD` from m04_constants. This is the death threshold for memories that have decayed beyond usefulness.
- **200-tick prune interval** — `MEMORY_PRUNE_INTERVAL` from m04_constants. Pruning is expensive (O(N*M) where N=spheres, M=memories), so it runs infrequently.
- **500/sphere capacity cap** — `MEMORY_MAX_COUNT` from m04_constants. When a sphere exceeds capacity, lowest-activation memories are dropped.
- **Sort + truncate for capacity enforcement** — sort by activation descending, keep top 500. This is O(M log M) per sphere, acceptable at 200-tick intervals.
- **Advisory in ORAC** — operates on the cached sphere map from PV2. Pruning recommendations are advisory; the daemon applies them.
- **Tool frequency analysis** — secondary function: computes fleet-wide tool usage patterns for semantic routing and field dashboard.

### Types to Implement

```rust
/// Fleet-wide memory statistics.
#[derive(Debug, Clone, Default)]
pub struct FleetMemoryStats {
    /// Total memories across all spheres.
    pub total_memories: usize,
    /// Total active memories (above activation threshold).
    pub active_memories: usize,
    /// Mean memories per sphere.
    pub mean_per_sphere: f64,
    /// Max memories in any single sphere.
    pub max_per_sphere: usize,
    /// Number of spheres at or near capacity.
    pub spheres_near_capacity: usize,
    /// Unique tool names across all memories.
    pub unique_tools: usize,
}

/// Result of a fleet-wide prune pass.
#[derive(Debug, Clone, Default)]
pub struct PruneResult {
    /// Total memories removed across all spheres.
    pub removed: usize,
    /// Number of spheres that had memories pruned.
    pub spheres_pruned: usize,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `compute_stats()` | `(&HashMap<PaneId, PaneSphere>) -> FleetMemoryStats` | Fleet-wide memory statistics |
| `prune_memories()` | `(&mut HashMap<PaneId, PaneSphere>, &ActivationZones) -> PruneResult` | Remove low-activation memories + enforce capacity |
| `tool_frequency()` | `(&HashMap<PaneId, PaneSphere>) -> Vec<(String, usize)>` | Fleet-wide tool usage, sorted descending |
| `sphere_top_tools()` | `(&PaneSphere, limit: usize) -> Vec<String>` | Top-N tools for a single sphere |

**`prune_memories()` algorithm:**
```text
For each sphere:
  1. Retain only memories with activation >= zones.prune_threshold (0.05)
  2. If still over zones.capacity (500):
     a. Sort by activation descending
     b. Truncate to capacity
  3. Count removed = before - after
Return PruneResult { total_removed, spheres_affected }
```

**`compute_stats()` algorithm:**
```text
For each sphere:
  1. Count total memories, active memories (activation > ACTIVATION_THRESHOLD)
  2. Track max_per_sphere
  3. Collect unique tool names
  4. Check if near capacity (within 50 of MEMORY_MAX_COUNT)
Compute mean = total / sphere_count
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `stats_empty_map` | unit | Empty sphere map -> `FleetMemoryStats::default()` |
| `stats_single_sphere` | unit | Correct counts for one sphere |
| `stats_multi_sphere` | unit | Aggregation across multiple spheres |
| `stats_near_capacity` | unit | `spheres_near_capacity` counts correctly |
| `stats_unique_tools` | unit | Deduplication across spheres |
| `prune_removes_low_activation` | unit | Memories below 0.05 are removed |
| `prune_enforces_capacity` | unit | Sphere with 600 memories -> 500 after prune |
| `prune_keeps_highest` | unit | After capacity truncation, remaining are highest-activation |
| `prune_no_change` | unit | All above threshold -> 0 removed |
| `prune_result_counts` | unit | `PruneResult` fields are accurate |
| `tool_frequency_sorted` | unit | Descending by count |
| `tool_frequency_empty` | unit | Empty map -> empty vec |
| `sphere_top_tools_limit` | unit | Respects limit parameter |
| `sphere_top_tools_sorted` | unit | Most frequent first |
| `prune_property` | property | After prune, no memory has activation < threshold |
| `capacity_property` | property | After prune, no sphere has > capacity memories |

### Cross-References

- `m04_constants::MEMORY_PRUNE_INTERVAL` (200) — tick interval for prune passes
- `m04_constants::TRACE_PRUNE_THRESHOLD` (0.05) — activation death threshold
- `m04_constants::MEMORY_MAX_COUNT` (500) — per-sphere capacity cap
- `m04_constants::ACTIVATION_THRESHOLD` (0.3) — active vs. inactive boundary
- `m04_constants::DECAY_PER_STEP` (0.995) — multiplicative decay per tick
- `m29_tick::tick_once()` — triggers pruning at `tick % MEMORY_PRUNE_INTERVAL == 0`
- `m01_core_types::ActivationZones` — configurable prune threshold and capacity
- `m34_field_dashboard` — consumes `FleetMemoryStats` for display
- `[[Pane-Vortex — Fleet Coordination Daemon]]`

---

## Lock Ordering Protocol

When multiple locks are needed (common in the tick loop and cascade paths):

```text
ALWAYS acquire in this order:
  1. AppState (field state, spheres, r_history)
  2. BusState (tasks, subscriptions, IPC clients)

NEVER:
  BusState -> AppState  // DEADLOCK

Pattern:
  {
      let values = state.read().extract_needed_values();
  }  // <-- AppState guard dropped here
  {
      let mut bus = bus_state.write();
      bus.apply(values);
  }  // <-- BusState guard dropped here
```

This ordering is enforced by code review and the `tick_once()` function which
extracts all needed values from `AppState` before touching `BusState`.

---

## Patterns and Anti-Patterns

**Patterns to follow:**
- P2: All trait methods `&self` with interior mutability
- P6: `Timestamp` newtype for temporal values
- P7: Owned returns from `RwLock` (clone, never reference)
- P22: Ring buffer cap (1000 lines for WASM events)

**Anti-patterns to avoid:**
- A1/A2: No `unwrap()` or `expect()` outside tests
- A4: No `println!()` in daemon code — use `tracing`
- A6: No `&mut self` on shared traits
- A17: No mono-parameter mutation (BUG-035)
- Lock inversion: **NEVER** acquire `BusState` before `AppState`
- Blocking the tick loop: **NEVER** call synchronous bridge I/O from `tick_once()`
