# V: Layer 6 Coordination — Source Verification

> **Layer:** L6 (`m6_coordination/`) — Orchestration Substrate
> **Feature Gate:** None (always compiled)
> **Dependencies:** L1, L2, L4, L5
> **Modules:** 5 (m27–m31)
> **Date:** 2026-03-25
> **Verified against:** source files + `m04_constants.rs` + `D7_MODULE_PURPOSE_GUIDE.md`

---

## Verification Matrix

| Claim | Source | Verified | Value |
|-------|--------|----------|-------|
| Conductor gain = 0.15 | m04_constants.rs:100 | **PASS** | `CONDUCTOR_GAIN: f64 = 0.15` |
| Cascade rate limit = 10/min | m28_cascade.rs:21 | **PASS** | `MAX_CASCADES_PER_MINUTE: u32 = 10` |
| WASM 5 commands | m30_wasm_bridge.rs:69–90 | **PASS** | Dispatch, Status, FieldState, ListPanes, Ping |
| RING_LINE_CAP = 1000 | m30_wasm_bridge.rs:54 | **PASS** | `RING_LINE_CAP: usize = 1000` |

**All 4 verification targets: PASS**

---

## M27: Conductor (`m27_conductor.rs` — 511 LOC, 25 tests)

### Purpose

P(roportional) breathing controller for the Kuramoto field. Observes cached PV2 field state and computes advisory `k_delta` recommendations. Does NOT own the authoritative `k_modulation` — that lives in PV2.

### Key Types

| Type | Description |
|------|-------------|
| `Conductor` | Stateless P-controller. Two fields: `gain: f64`, `breathing_blend: f64`. `Send + Sync` (no interior mutability). |

### Constants (from `m04_constants.rs`)

| Constant | Value | Used In |
|----------|-------|---------|
| `CONDUCTOR_GAIN` | **0.15** | `Conductor::new()` default gain |
| `EMERGENT_BLEND` | 0.3 | `Conductor::new()` default breathing blend |
| `R_TARGET_BASE` | 0.93 | Target r for small/medium fleets |
| `R_TARGET_LARGE_FLEET` | 0.85 | Target r for >50 spheres |
| `LARGE_FLEET_THRESHOLD` | 50.0 | Sphere count cutover |
| `R_RISING_THRESHOLD` | 0.03 | Error > this → `NeedsCoherence` |
| `R_FALLING_THRESHOLD` | -0.03 | Error < this → `NeedsDivergence` |
| `K_MOD_BUDGET_MIN` | 0.85 | Clamp floor for k_delta |
| `K_MOD_BUDGET_MAX` | 1.15 | Clamp ceiling for k_delta |

### Public API

```rust
Conductor::new() -> Self                           // gain=0.15, blend=0.3
Conductor::with_params(gain, blend) -> Self        // both clamped to safe ranges
Conductor::r_target(state: &AppState) -> f64       // dynamic target (governance > fleet-size)
Conductor::decide(&self, state: &AppState) -> FieldDecision
Conductor::decide_and_update(&self, state: &mut AppState) -> FieldDecision
Conductor::gain(&self) -> f64                      // const fn
Conductor::breathing_blend(&self) -> f64           // const fn
is_direction_flip(prev, current) -> bool           // const fn, thrashing detection
deterministic_noise(id: &PaneId, tick: u64) -> f64 // hash-based, [-1.0, 1.0]
```

### Decision Logic

1. **Warmup guard:** Returns `Stable` during warmup period (first 10 ticks)
2. **Sphere minimum:** Returns `Stable` if `spheres.len() < 3` (`MIN_SPHERES_FOR_BREATHING`)
3. **Classification (`classify_error`):**
   - `divergence_cooldown > 0` → `Recovering`
   - `r > 0.99` → `OverSynchronized`
   - `error > 0.03` → `NeedsCoherence`
   - `error < -0.03` → `NeedsDivergence`
   - Otherwise → `Stable`
4. **k_delta computation:** `error * gain`, clamped to `[-0.15, 0.15]`
5. **`decide_and_update` extras:** Updates EMAs, decrements cooldown, records `prev_decision_action`

### D7 Guide Discrepancies

| D7 Claim | Source Reality | Status |
|----------|---------------|--------|
| "PI controller" in D7 title | P-only. Code comment at m27:127: "P-only (BUG-L1-009: I-term reserved)" | **D7 INACCURATE** — should say "P controller" |
| `decide_and_update` listed | Present and functional | MATCH |
| 25 tests | 25 tests confirmed (counted in `mod tests`) | MATCH |

### Thread Safety

`Conductor` is a plain struct with no interior mutability. `Send + Sync` by default. All methods take `&self` or `&AppState`/`&mut AppState` (caller controls the lock).

---

## M28: Cascade (`m28_cascade.rs` — 856 LOC, 46 tests)

### Purpose

Cascade handoff protocol for delegating work between fleet Claude Code instances. Supports depth tracking, consent propagation, rate limiting, auto-summarization, and markdown fallback briefs.

### Constants

| Constant | Value | Line | Purpose |
|----------|-------|------|---------|
| `MAX_CASCADES_PER_MINUTE` | **10** | :21 | Rate limit per sliding window |
| `RATE_WINDOW_SECS` | 60.0 | :24 | Sliding window duration |
| `MAX_PENDING_CASCADES` | 50 | :27 | Backpressure limit |
| `AUTO_SUMMARIZE_DEPTH` | 3 | :30 | Depth at which auto-summarization triggers |
| `MAX_BRIEF_CHARS` | 4096 | :33 | Brief truncation limit |

### Key Types

| Type | Fields | Description |
|------|--------|-------------|
| `CascadeHandoff` | source, target, brief, dispatched_at, depth, acknowledged, rejected, rejection_reason, consent_snapshot | Single handoff record |
| `CascadeTracker` | cascades (VecDeque), window_count, window_start, max_depth (default 10) | Manages lifecycle with rate limiting |

### Public API

```rust
// CascadeHandoff
CascadeHandoff::new(source, target, brief) -> Self       // depth=1, truncates at 4096 chars
CascadeHandoff::with_consent(consent_map) -> Self         // builder: attach consent snapshot
CascadeHandoff::consent_allows(field) -> bool             // default-open for unknown fields
CascadeHandoff::re_cascade(new_target) -> Self            // depth+1, auto-summarize at depth>=3
CascadeHandoff::acknowledge()                             // mark acknowledged
CascadeHandoff::reject(reason)                            // mark rejected with reason
CascadeHandoff::is_pending() -> bool                      // const fn
CascadeHandoff::elapsed_secs() -> f64
CascadeHandoff::needs_summarization() -> bool             // const fn
CascadeHandoff::fallback_brief() -> String                // markdown for non-bus-aware targets

// CascadeTracker
CascadeTracker::new() -> Self                             // max_depth=10
CascadeTracker::with_max_depth(max) -> Self               // min clamped to 1
CascadeTracker::initiate(source, target, brief) -> PvResult<usize>  // rate + pending checked
CascadeTracker::re_cascade(index, new_target) -> PvResult<usize>    // depth + rate checked
CascadeTracker::acknowledge(index) -> PvResult<()>
CascadeTracker::reject(index, reason) -> PvResult<()>
CascadeTracker::pending_cascades() -> Vec<(usize, &CascadeHandoff)>
CascadeTracker::total_count() -> usize
CascadeTracker::pending_count() -> usize
CascadeTracker::get(index) -> Option<&CascadeHandoff>
CascadeTracker::window_count() -> u32                     // const fn
CascadeTracker::prune(keep)                               // removes resolved from front
```

### Rate Limiting

- **Sliding window:** 60-second window, resets when expired
- **Per-minute cap:** 10 cascades per window (`CascadeRateLimit` error)
- **Pending cap:** 50 pending cascades max (`BusProtocol` error)
- **Depth cap:** Default 10 (configurable), checked on `re_cascade`

### Consent Propagation

Consent snapshots are `HashMap<String, bool>` attached via `.with_consent()`. They propagate through the full re-cascade chain. Unknown fields default to `true` (open-by-default). Tests confirm preservation through 3-hop chains.

### Auto-Summarization

At `depth >= 3`, `re_cascade` automatically summarizes the brief: keeps first 5 + last 5 lines, omits the middle. Brief truncated to 4096 chars on creation.

### D7 Guide Discrepancies

| D7 Claim | Source Reality | Status |
|----------|---------------|--------|
| "sphere mitosis" in title | No mitosis logic in source — cascades delegate, not split | **D7 MISLEADING** — conceptual label, not code |
| 46 tests | 46 tests confirmed | MATCH |
| Rate limit 10/min | `MAX_CASCADES_PER_MINUTE = 10` confirmed | MATCH |

### Thread Safety

No interior mutability. `CascadeTracker` requires `&mut self` for all mutations — caller must provide synchronization (e.g., behind a `RwLock` if shared). Currently not on `OracState` (no shared access).

---

## M29: Tick (`m29_tick.rs` — 452 LOC, 13 tests)

### Purpose

Tick orchestrator for the ORAC sidecar. Runs a 5-phase cycle on the cached `AppState`:

```
Phase 1: Advance tick counter + warmup
Phase 2: Recompute FieldState from cached spheres
Phase 3: Conductor advisory decision (decide_and_update)
Phase 4: Hebbian STDP pass (only in tick_with_hebbian)
Phase 5: Governance check (validate r_target_override)
```

### Key Types

| Type | Fields | Description |
|------|--------|-------------|
| `TickResult` | field_state, decision, order_parameter, phase_timings, total_ms, tick, sphere_count, should_snapshot, hebbian_updated, governance_active | Output of one tick |
| `PhaseTiming` | field_state_ms, conductor_ms, hebbian_ms, governance_ms | Per-phase timing (ms) |

### Public API

```rust
tick_once(state: &mut AppState, conductor: &Conductor) -> TickResult
tick_with_hebbian(state: &mut AppState, conductor: &Conductor, coupling: &mut CouplingNetwork) -> TickResult  // #[cfg(feature = "intelligence")]
```

### Governance Check (`check_governance`)

- Validates `r_target_override` is in `[0.5, 1.0]`
- Clears out-of-range overrides with warning log
- Returns `true` if governance is active (override in effect or cleared)

### Snapshot Interval

`should_snapshot = current_tick % SNAPSHOT_INTERVAL == 0` where `SNAPSHOT_INTERVAL = 60`.

### D7 Guide Discrepancies

| D7 Claim | Source Reality | Status |
|----------|---------------|--------|
| "Hebbian Phase 2.5 wiring" | Phase 4 in both code and doc comment | **D7 NAMING MISMATCH** — "2.5" is a PV2 legacy label |
| 13 tests | 13 tests confirmed | MATCH |
| Dependencies include m15, m18 | Only under `#[cfg(feature = "intelligence")]` — not always present | MATCH (feature-gated) |

### Thread Safety

Pure functions taking `&mut AppState`. No interior mutability. Caller (RALPH loop in `main.rs`) holds `field_state.write()` during `tick_once()`.

**Note:** In `main.rs`, `tick_once` is called within a `{ let mut app_state = state.field_state.write(); ... }` block. The `tick_with_hebbian` variant is NOT used in main.rs — the RALPH loop does STDP separately after dropping the field_state lock.

---

## M30: WASM Bridge (`m30_wasm_bridge.rs` — 729 LOC, 34 tests)

### Purpose

FIFO/ring protocol bridge between ORAC and the Zellij swarm-orchestrator WASM plugin. Commands flow in via named FIFO, events flow out via a ring-buffered JSONL file.

### Protocol

```
WASM plugin  → /tmp/swarm-commands.pipe (FIFO)   → ORAC reads commands
ORAC         → /tmp/swarm-events.jsonl  (ring)   → WASM plugin reads events
               (1000 line cap, oldest lines dropped)
```

### Constants

| Constant | Value | Line | Purpose |
|----------|-------|------|---------|
| `DEFAULT_FIFO_PATH` | `/tmp/swarm-commands.pipe` | :48 | Incoming command FIFO |
| `DEFAULT_RING_PATH` | `/tmp/swarm-events.jsonl` | :51 | Outgoing event ring file |
| `RING_LINE_CAP` | **1000** | :54 | Maximum lines in ring buffer |
| `MAX_COMMAND_LEN` | 8192 | :57 | Command size limit (bytes) |
| `MAX_EVENT_LEN` | 8192 | :60 | Event size limit (bytes) |

### Commands (5 variants)

| Command | Tag | Fields | Description |
|---------|-----|--------|-------------|
| `Dispatch` | `"dispatch"` | `pane: String, task: String` | Dispatch a task to a specific pane |
| `Status` | `"status"` | — | Request fleet status |
| `FieldState` | `"field_state"` | — | Request Kuramoto field state (r, K, phases) |
| `ListPanes` | `"list_panes"` | — | Request pane list |
| `Ping` | `"ping"` | — | Keepalive |

**Total: 5 commands — VERIFIED**

### Event Types

| Factory | Event Tag | Data |
|---------|-----------|------|
| `WasmEvent::tick_event(tick, r, k)` | `"tick"` | `{"r": f64, "k": f64}` |
| `WasmEvent::task_completed(tick, task_id, pane)` | `"task_completed"` | `{"task_id": str, "pane": str}` |
| `WasmEvent::pong(tick)` | `"pong"` | `null` |
| `WasmEvent::new(event, tick, data)` | custom | custom `Value` |

### Key Types

| Type | Description |
|------|-------------|
| `WasmCommand` | Tagged enum (5 variants), serde `#[serde(tag = "cmd")]` |
| `WasmEvent` | Struct: `event: String`, `tick: u64`, `data: Value` |
| `EventRingBuffer` | `VecDeque<String>` with cap, FIFO eviction, total_written counter |
| `WasmBridge` | Thread-safe bridge: 3 `RwLock` fields (ring, command_queue, stats) |
| `WasmBridgeStats` | Counters: commands_received, events_written, parse_errors, oversized_rejected |

### Ring Buffer Behavior

- `new(cap)` — capacity clamped to `min(cap, RING_LINE_CAP)`
- `write_event` — serializes to JSONL, rejects if > 8192 bytes, FIFO evicts oldest when full
- `to_file_content()` — renders all lines with newline separators for disk flush
- `total_written` — monotonically increasing, survives eviction

### WasmBridge Thread Safety

All mutable state behind `parking_lot::RwLock`:
- `ring: RwLock<EventRingBuffer>` — outbound events
- `command_queue: RwLock<VecDeque<WasmCommand>>` — parsed inbound commands
- `stats: RwLock<WasmBridgeStats>` — counters

### D7 Guide Discrepancies

| D7 Claim | Source Reality | Status |
|----------|---------------|--------|
| 5 commands listed | 5 enum variants confirmed | MATCH |
| `RING_LINE_CAP(1000)` | `pub const RING_LINE_CAP: usize = 1000` | MATCH |
| 34 tests | 34 tests confirmed | MATCH |
| No D7 mention of `WasmBridge` struct | Source has full `WasmBridge` with 3 RwLocks | **D7 INCOMPLETE** — omits bridge struct |

---

## M31: Memory Manager (`m31_memory_manager.rs` — 381 LOC, 15 tests)

### Purpose

Fleet-level memory aggregation, statistics, and pruning. Per-sphere memory operations live in `PaneSphere`; this module handles cross-sphere analysis.

### Key Types

| Type | Fields | Description |
|------|--------|-------------|
| `FleetMemoryStats` | total_memories, active_memories, mean_per_sphere, max_per_sphere, spheres_near_capacity, unique_tools | Fleet-wide aggregation |
| `PruneResult` | removed, spheres_pruned | Per-pass pruning outcome |

### Public API

```rust
compute_stats(spheres: &HashMap<PaneId, PaneSphere>) -> FleetMemoryStats
prune_memories(spheres: &mut HashMap<PaneId, PaneSphere>, zones: &ActivationZones) -> PruneResult
tool_frequency(spheres: &HashMap<PaneId, PaneSphere>) -> Vec<(String, usize)>  // sorted desc
sphere_top_tools(sphere: &PaneSphere, limit: usize) -> Vec<String>
```

### Pruning Logic

1. **Threshold prune:** Remove memories with `activation < zones.prune_threshold`
2. **Capacity enforce:** If still over `zones.capacity`, sort by activation descending, truncate
3. **Near-capacity detection:** `MEMORY_MAX_COUNT (500) - 50 = 450` threshold for warning

### Constants Used (from `m04_constants.rs`)

| Constant | Value | Purpose |
|----------|-------|---------|
| `MEMORY_MAX_COUNT` | 500 | Per-sphere capacity |
| `ACTIVATION_THRESHOLD` | 0.3 | Active memory threshold |

### D7 Guide Discrepancies

| D7 Claim | Source Reality | Status |
|----------|---------------|--------|
| `prune_memories(spheres, threshold, cap)` signature | Actual: `prune_memories(spheres, zones: &ActivationZones)` | **D7 INACCURATE** — API uses `ActivationZones` struct, not separate threshold/cap args |
| 15 tests | 15 tests confirmed | MATCH |

### Thread Safety

All functions are pure — take `&HashMap` or `&mut HashMap`. Caller controls synchronization. Advisory only (no side effects beyond the passed-in map).

---

## Layer-Level Summary

### Module Statistics

| Module | LOC | Tests | Hot-Swap Source | Feature Gate |
|--------|-----|-------|-----------------|--------------|
| m27_conductor | 511 | 25 | PV2 m31 (adapt) | None |
| m28_cascade | 856 | 46 | PV2 m33 (drop-in) | None |
| m29_tick | 452 | 13 | PV2 m35 (adapt) | `intelligence` (Phase 4 only) |
| m30_wasm_bridge | 729 | 34 | NEW (ORAC-only) | None |
| m31_memory_manager | 381 | 15 | PV2 m21 (drop-in) | None |
| **Total** | **2,929** | **133** | | |

### D7 Guide Accuracy

| Check | D7 Claim | Source | Verdict |
|-------|----------|--------|---------|
| Layer LOC | ~2,929 | 2,929 (511+856+452+729+381) | MATCH |
| Layer tests | 133 | 133 (25+46+13+34+15) | MATCH |
| Feature gate | None | None (intelligence only for tick Phase 4) | MATCH |
| Dependencies | L1, L2, L4, L5 | L1 (all), L4 (m29 only, feature-gated), L6 internal (m29→m27) | **D7 OVERSTATES** — L2 and L5 not directly imported by any L6 module |

### D7 Discrepancies Found (6)

| # | Module | Issue | Severity |
|---|--------|-------|----------|
| 1 | m27 | D7 says "PI controller" — source is P-only (I-term reserved per BUG-L1-009) | Medium |
| 2 | m28 | D7 says "sphere mitosis" — no mitosis code, just delegation | Low (conceptual label) |
| 3 | m29 | D7 says "Hebbian Phase 2.5" — source uses Phase 4, "2.5" is PV2 legacy | Low |
| 4 | m30 | D7 omits `WasmBridge` struct (3 RwLocks, full bridge lifecycle) | Medium |
| 5 | m31 | D7 lists `prune_memories(spheres, threshold, cap)` — actual uses `ActivationZones` struct | Medium |
| 6 | Layer | D7 lists L2+L5 as dependencies — no L6 module imports from L2 or L5 | Low |

### Cross-Module Integration Points

```
main.rs RALPH loop
  └── tick_once(state, conductor)          → m29 calls m27
  └── apply_stdp(coupling, spheres)        → m29's tick_with_hebbian (NOT used in main.rs)

m10_hook_server.rs
  └── spawn_field_poller → updates field_state → tick_once reads it

WasmBridge (m30) — standalone, not yet wired into main.rs runtime
CascadeTracker (m28) — standalone, not yet on OracState
MemoryManager (m31) — standalone, advisory functions
```
