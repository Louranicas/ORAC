---
title: "Layer 4: Intelligence — Module Documentation"
date: 2026-03-22
tags: [modules, intelligence, L4, orac-sidecar]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L4
modules: [m15, m16, m17, m18, m19, m20, m21]
---

# Layer 4: Intelligence (m15-m21)

> Hebbian learning, Kuramoto coupling dynamics, content-aware routing, per-pane health gating.
> m15-m19 are hot-swap from PV2. m20 (semantic router) and m21 (circuit breaker) are new.
> **Target LOC:** ~4,000 | **Target tests:** 80+
> **Source:** 5 drop-in, 2 new | **Phase:** 2

---

## Overview

L4 provides the intelligence substrate that makes ORAC more than a dumb proxy. Five
modules are dropped in directly from PV2 (proven in production with 1,527 tests and
6+ hours of continuous uptime), giving ORAC immediate access to Kuramoto phase dynamics
and Hebbian co-activation learning. Two new modules extend this foundation: a semantic
router that uses Hebbian weights for content-aware task dispatch, and a circuit breaker
FSM that gates per-pane health.

**Implementation order:** m15 (coupling network, foundation for all others) -> m16
(auto-K, depends on m15) -> m17 (topology, depends on m15) -> m18 (Hebbian STDP,
depends on m15) -> m19 (buoy network, standalone) -> m21 (circuit breaker, standalone
FSM) -> m20 (semantic router, depends on m15 + m18).

**Feature gate:** `#[cfg(feature = "intelligence")]`

**Mathematical foundation:** Kuramoto mean-field equation:

```
d theta_i / dt = omega_i + (K / N) * sum_j [ w_ij * sin(theta_j - theta_i) ]
```

Where `theta_i` is sphere phase, `omega_i` is natural frequency, `K` is global coupling
strength, `w_ij` is Hebbian weight, and `N` is sphere count. All phase arithmetic
uses `.rem_euclid(TAU)` after every operation (P01).

---

## m15 -- Coupling Network

**Source:** `src/m4_intelligence/m15_coupling_network.rs`
**LOC Target:** ~800
**Depends on:** `m01_core_types` (`PaneId`, `OrderParameter`, `phase_diff`), `m04_constants` (`KURAMOTO_DT`, `DEFAULT_WEIGHT`, `WEIGHT_EXPONENT`)
**Hot-Swap:** DROP-IN from PV2 M16

### Design Decisions

- **Adjacency-indexed for O(degree) step computation.** The naive Kuramoto step is
  O(N^2). The adjacency index (`HashMap<PaneId, Vec<usize>>`) precomputes which
  connections involve each sphere, reducing step complexity to O(N * degree). The
  index is rebuilt on register/deregister via `rebuild_index()`.
- **Jacobi integration with dt=0.01.** Small fixed timestep prevents phase jumps.
  Each tick runs `COUPLING_STEPS_PER_TICK` (15) Euler steps.
- **Coupling sum cap at 3.0.** Per-sphere coupling contribution is clamped to prevent
  supercritical runaway in dense networks. Without this cap, a sphere with many strong
  connections can jump its phase by more than pi/tick.
- **k_modulation is multiplicative.** `k_effective = k * k_modulation`. The modulation
  factor is externally set (by conductor, thermal gate, consent) and clamped to
  `[K_MOD_MIN, K_MOD_MAX]` = `[-0.5, 1.5]`. Negative modulation inverts coupling
  (repulsive), enabling deliberate desynchronization.
- **Phase wrapping (P01).** Every phase update ends with `.rem_euclid(TAU)`. This is
  non-negotiable. Without it, phases drift to large values and floating-point precision
  degrades. The `phase_diff()` helper also wraps to `[-pi, pi]`.

### Types to Implement

```rust
/// Directed connection between two pane-spheres.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Source sphere.
    pub from: PaneId,
    /// Target sphere.
    pub to: PaneId,
    /// Base coupling weight (0.0-1.0). Modified by Hebbian learning.
    pub weight: f64,
    /// Connection type modifier (apex-apex=1.0, apex-horizon=0.6).
    pub type_weight: f64,
}

/// Kuramoto coupling network for all pane-spheres.
///
/// Manages phases, frequencies, connections, and K modulation.
/// Adjacency-indexed for O(degree) step computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingNetwork {
    /// Per-sphere phase (radians, 0..2pi).
    pub phases: HashMap<PaneId, f64>,
    /// Per-sphere natural frequency.
    pub frequencies: HashMap<PaneId, f64>,
    /// Connection list (directed edges).
    pub connections: Vec<Connection>,
    /// Global coupling strength K.
    pub k: f64,
    /// Auto-scale K based on frequency spread.
    pub auto_k: bool,
    /// Multiplicative modulation factor for K (1.0 = no change).
    pub k_modulation: f64,
    /// Causal STDP: asymmetric weights when true.
    pub asymmetric_hebbian: bool,
    /// Adjacency index: sphere ID -> indices into `connections`.
    #[serde(skip)]
    adj_index: HashMap<PaneId, Vec<usize>>,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `new` | `pub fn new() -> Self` | Empty network, k=1.5, auto_k=true |
| `register` | `pub fn register(&mut self, id: PaneId, phase: f64, frequency: f64)` | Add sphere + connections to all existing |
| `deregister` | `pub fn deregister(&mut self, id: &PaneId)` | Remove sphere + its connections, rebuild index |
| `rebuild_index` | `pub fn rebuild_index(&mut self)` | Reconstruct adjacency index from connections |
| `step` | `pub fn step(&mut self)` | One Kuramoto Euler step (dt=0.01), phase wrapping (P01) |
| `step_with_receptivity` | `pub fn step_with_receptivity(&mut self, receptivities: &HashMap<PaneId, f64>)` | Step with per-sphere receptivity gating |
| `order_parameter` | `pub fn order_parameter(&self) -> OrderParameter` | Compute r and psi from current phases |
| `auto_scale_k` | `pub fn auto_scale_k(&mut self)` | K = multiplier * sqrt(N) * freq_spread, clamped |
| `set_weight` | `pub fn set_weight(&mut self, from: &PaneId, to: &PaneId, weight: f64)` | Update Hebbian weight for a connection |
| `get_weight` | `pub fn get_weight(&self, from: &PaneId, to: &PaneId) -> Option<f64>` | Read weight for a connection pair |
| `kick_phases_apart` | `pub fn kick_phases_apart(&mut self, sphere_ids: &[PaneId], strength: f64) -> usize` | Chimera intervention: spread phases by strength |
| `coupling_matrix` | `pub fn coupling_matrix(&self) -> HashMap<(PaneId, PaneId), f64>` | Full weight matrix snapshot |

### Kuramoto Step Detail (P01)

```rust
// Core step logic (simplified):
for id in self.phases.keys() {
    let theta_i = self.phases[id];
    let omega_i = self.frequencies[id];
    let k_eff = self.k * self.k_modulation;

    let coupling_sum: f64 = self.adj_index[id]
        .iter()
        .map(|&ci| {
            let conn = &self.connections[ci];
            let theta_j = self.phases[&conn.to];
            let w = conn.weight * conn.type_weight;
            // FMA: w^2 * sin(theta_j - theta_i) (P05)
            (w * w).mul_add(
                (theta_j - theta_i).rem_euclid(TAU).sin(),  // P01
                0.0,
            )
        })
        .sum::<f64>()
        .clamp(-COUPLING_SUM_CAP, COUPLING_SUM_CAP);

    // Euler step: theta_i += dt * (omega_i + k_eff/N * coupling_sum)
    let n = self.phases.len() as f64;
    let d_theta = KURAMOTO_DT.mul_add(
        omega_i + k_eff / n * coupling_sum,
        0.0,
    );
    new_phases.insert(id.clone(), (theta_i + d_theta).rem_euclid(TAU));  // P01
}
```

### Tests

| Test | Validates |
|------|-----------|
| `test_register_creates_connections` | N spheres -> N*(N-1) directed connections |
| `test_deregister_removes_connections` | Connections cleaned up, index rebuilt |
| `test_step_wraps_phase_p01` | Phase stays in [0, TAU) after step |
| `test_order_parameter_synchronized` | All same phase -> r = 1.0 |
| `test_order_parameter_uniform` | Uniformly distributed -> r near 0.0 |
| `test_coupling_sum_cap` | Coupling clamped to [-3.0, 3.0] |
| `test_k_modulation_negative` | k_modulation < 0 -> repulsive coupling |
| `test_k_modulation_clamped` | k_mod stays in [-0.5, 1.5] |
| `test_set_weight_updates_connection` | Weight change propagates to step |
| `test_auto_scale_k` | K scales with sqrt(N) * freq_spread |
| `test_adjacency_index_consistency` | Index matches connection list after mutations |
| `test_kick_phases_apart` | Phase spread increases by strength amount |
| `test_receptivity_zero_freezes_phase` | Sphere with receptivity=0 does not move |

### Cross-References

- [[Session 045 Arena -- 10-hebbian-operational-topology]]
- [[Vortex Sphere Brain-Body Architecture]]
- [[Executor and Nested Kuramoto Bridge -- Session 028]]
- ORAC_PLAN.md Phase 2 Detail (step 1)
- ORAC_MINDMAP.md Branch 3 (Intelligence Layer)
- **P01** (phase wrapping `.rem_euclid(TAU)`)
- **P05** (FMA for float precision)
- arxiv 2508.12314 (Kuramoto for AI agent coordination)

---

## m16 -- Auto-K

**Source:** `src/m4_intelligence/m16_auto_k.rs`
**LOC Target:** ~350
**Depends on:** `m15_coupling_network` (`CouplingNetwork`), `m04_constants` (`COUPLING_STEPS_PER_TICK`, `K_MOD_MIN`, `K_MOD_MAX`)
**Hot-Swap:** DROP-IN from PV2 M17

### Design Decisions

- **Auto-K multiplier is 0.5 (not 1.5).** Session 025 hardening reduced this from 1.5
  to prevent over-synchronization. At 1.5, the field pinned r=0.999 within 2 ticks,
  making differentiation impossible. At 0.5, r stabilizes around 0.985-0.997.
- **EMA smoothing on K transitions.** Raw K recalculation can jump by 50%+ when spheres
  register/deregister. EMA with alpha=0.3 prevents coupling shocks.
- **Consent-gated adjustment.** The `consent_gated_k_adjustment` function checks
  whether the sphere has explicitly consented to coupling modulation before applying
  K changes. Spheres with `ConsentPosture::OptOut` are not subject to K adjustments.
- **Per-status K modulation (NA-22).** Working<->Working pairs get 1.2x K. Idle<->Working
  pairs get 0.5x K. Blocked spheres get 0.0x K (complete decoupling).

### Types to Implement

```rust
/// Auto-K scaling controller state.
///
/// Tracks when to recalculate K and applies smoothing to prevent
/// sudden coupling strength changes.
#[derive(Debug, Clone)]
pub struct AutoKController {
    /// Ticks since last recalculation.
    ticks_since_recalc: u64,
    /// Recalculation period in ticks.
    period: u64,
    /// Previous K value for smoothing.
    previous_k: f64,
    /// Smoothing factor (0.0 = no smoothing, 1.0 = full smoothing).
    smoothing: f64,
}

/// Per-status K modulation factors (NA-22).
pub struct StatusKModulation;

impl StatusKModulation {
    /// K multiplier when both spheres are `Working`.
    pub const WORKING_WORKING: f64 = 1.2;
    /// K multiplier when one is `Idle` and one is `Working`.
    pub const IDLE_WORKING: f64 = 0.5;
    /// K multiplier for `Blocked` spheres (complete decoupling).
    pub const BLOCKED: f64 = 0.0;
    /// K multiplier for same-status idle pairs.
    pub const IDLE_IDLE: f64 = 0.8;
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `new` | `pub const fn new() -> Self` | Default period=15, smoothing=0.3 |
| `with_params` | `pub const fn with_params(period: u64, smoothing: f64) -> Self` | Custom controller |
| `tick` | `pub fn tick(&mut self, network: &mut CouplingNetwork) -> bool` | Tick + maybe recalc. Returns true if K changed |
| `force_recalc` | `pub fn force_recalc(&mut self, network: &mut CouplingNetwork)` | Immediate K recalculation |
| `reset` | `pub fn reset(&mut self)` | Reset tick counter and previous_k |
| `consent_gated_k_adjustment` | `pub fn consent_gated_k_adjustment(network: &mut CouplingNetwork, sphere_id: &PaneId, delta: f64, consented: bool) -> bool` | Apply K delta only if consented |

### Tests

| Test | Validates |
|------|-----------|
| `test_tick_recalcs_at_period` | K updated after 15 ticks |
| `test_tick_no_recalc_before_period` | K unchanged at tick 14 |
| `test_ema_smoothing` | K change dampened by smoothing factor |
| `test_auto_k_disabled_skips` | `auto_k=false` -> no K change |
| `test_force_recalc_immediate` | K updated regardless of tick counter |
| `test_consent_gated_approved` | K delta applied when consented=true |
| `test_consent_gated_denied` | K delta rejected when consented=false |
| `test_k_mod_clamped` | K modulation stays in [-0.5, 1.5] |
| `test_multiplier_is_0_5` | Auto-scale uses 0.5 multiplier (not 1.5) |
| `test_status_k_modulation_working` | Working-Working pair gets 1.2x |
| `test_status_k_modulation_blocked` | Blocked sphere gets 0.0x |

### Cross-References

- [[Session 025 -- Hardening]] k_mod clamp unified [-0.5, 1.5]
- ORAC_PLAN.md Phase 2 Detail (step 1)
- MEMORY.md: "auto_scale_k multiplier: 0.5 (was 1.5)"
- **P05** (FMA for EMA smoothing)
- **AP17** (mono-parameter mutation -- K and weight_exponent are an invariant group)

---

## m17 -- Topology

**Source:** `src/m4_intelligence/m17_topology.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types` (`PaneId`, `phase_diff`), `m04_constants`, `m15_coupling_network` (`CouplingNetwork`)
**Hot-Swap:** DROP-IN from PV2 M18

### Design Decisions

- **Weight-squared amplification (NA-25).** Topology queries report both raw effective
  weight (`w * type_weight`) and `w^2` amplified weight. The squared term matches
  the exponent used in the Kuramoto step, ensuring topology analysis reflects actual
  coupling dynamics.
- **Free functions over methods.** Topology queries are free functions taking
  `&CouplingNetwork` rather than methods on the struct. This keeps the coupling
  network struct focused on phase dynamics and allows topology analysis to be
  used optionally (feature-gated consumers don't need to pull in analysis code).

### Types to Implement

```rust
/// Information about a neighboring sphere in the coupling topology.
#[derive(Debug, Clone)]
pub struct NeighborInfo {
    /// Neighbor sphere ID.
    pub id: PaneId,
    /// Coupling weight (base * type).
    pub effective_weight: f64,
    /// Weight-squared amplified (for NA-25 topology-aware coupling).
    pub weight_squared: f64,
    /// Phase difference from the queried sphere.
    pub phase_diff: f64,
}

/// Summary of the coupling topology.
#[derive(Debug, Clone)]
pub struct TopologySummary {
    /// Number of spheres.
    pub sphere_count: usize,
    /// Number of directed connections.
    pub connection_count: usize,
    /// Mean effective coupling weight.
    pub mean_weight: f64,
    /// Standard deviation of coupling weights.
    pub weight_std: f64,
    /// Maximum coupling weight in the network.
    pub max_weight: f64,
    /// Minimum coupling weight in the network.
    pub min_weight: f64,
    /// Mean degree (connections per sphere).
    pub mean_degree: f64,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `neighbors` | `pub fn neighbors(network: &CouplingNetwork, sphere_id: &PaneId) -> Vec<NeighborInfo>` | Sorted by effective weight descending |
| `strongest_neighbor` | `pub fn strongest_neighbor(network: &CouplingNetwork, sphere_id: &PaneId) -> Option<NeighborInfo>` | Highest effective weight |
| `mean_coupling_weight` | `pub fn mean_coupling_weight(network: &CouplingNetwork, sphere_id: &PaneId) -> f64` | Mean weight for one sphere |
| `degree` | `pub fn degree(network: &CouplingNetwork, sphere_id: &PaneId) -> usize` | Connection count for one sphere |
| `topology_summary` | `pub fn topology_summary(network: &CouplingNetwork) -> TopologySummary` | Full network statistics |
| `most_coupled_pair` | `pub fn most_coupled_pair(network: &CouplingNetwork) -> Option<(PaneId, PaneId, f64)>` | Strongest connection in network |
| `least_coupled_pair` | `pub fn least_coupled_pair(network: &CouplingNetwork) -> Option<(PaneId, PaneId, f64)>` | Weakest connection in network |

### Tests

| Test | Validates |
|------|-----------|
| `test_neighbors_sorted_by_weight` | Descending weight order |
| `test_strongest_neighbor` | Returns highest effective weight |
| `test_mean_coupling_weight` | Correct mean for known weights |
| `test_degree_counts_connections` | Correct count for known topology |
| `test_topology_summary_stats` | Mean, std, min, max computed correctly |
| `test_weight_squared_in_neighbor_info` | `weight_squared` = `effective_weight^2` |
| `test_empty_network_summary` | Zero spheres -> safe defaults |
| `test_most_coupled_pair` | Returns correct pair from known network |
| `test_phase_diff_in_neighbor_info` | Phase difference wrapped to [-pi, pi] |

### Cross-References

- [[Session 045 Arena -- 10-hebbian-operational-topology]]
- ORAC_PLAN.md Phase 2 Detail (step 1)
- m15 `CouplingNetwork` for source data structure

---

## m18 -- Hebbian STDP

**Source:** `src/m4_intelligence/m18_hebbian_stdp.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types` (`PaneId`, `PaneStatus`, `PaneSphere`), `m04_constants` (STDP parameters), `m15_coupling_network` (`CouplingNetwork`)
**Hot-Swap:** DROP-IN from PV2 M19

### Design Decisions

- **Spike-timing dependent plasticity adapted for Kuramoto oscillators.** In
  neuroscience, STDP strengthens synapses between neurons that fire together. Here,
  "co-active" means both spheres have `PaneStatus::Working` in the same tick.
  Co-active pairs get LTP (long-term potentiation); inactive pairs get LTD
  (long-term depression).
- **Three LTP multipliers:** Base LTP is 0.01 per co-activation. Burst detection
  (sphere has >3 recent tool calls in 30s window) applies 3x multiplier. Newcomer
  spheres (<`NEWCOMER_STEPS` ticks old) get 2x multiplier to accelerate integration
  into the coupling network.
- **Weight floor at 0.05.** Connections never drop below `HEBBIAN_WEIGHT_FLOOR`.
  This prevents complete disconnection -- even spheres that have been idle for hours
  retain minimal coupling to the field. A zero-weight connection is effectively
  invisible to the Kuramoto step.
- **Opt-out respected.** If either sphere in a connection has `opt_out_hebbian=true`,
  that connection is skipped entirely. No LTP, no LTD. This is the Habitat philosophy:
  the field does not impose learning on unwilling participants.
- **HashSet lookups for co-activation.** Working spheres are collected into a HashSet
  for O(1) lookup during the connection iteration. Previous implementation used
  linear search (O(N) per connection, O(N^3) total).

### STDP Parameters (from m04_constants)

| Parameter | Constant | Value | Notes |
|-----------|----------|-------|-------|
| LTP rate | `HEBBIAN_LTP` | 0.01 | Per co-activation event |
| LTD rate | `HEBBIAN_LTD` | 0.002 | Per non-co-activation event |
| Burst multiplier | `HEBBIAN_BURST_MULTIPLIER` | 3.0 | Applied when `activity_30s > 3` |
| Newcomer multiplier | `HEBBIAN_NEWCOMER_MULTIPLIER` | 2.0 | Applied when `total_steps < NEWCOMER_STEPS` |
| Weight floor | `HEBBIAN_WEIGHT_FLOOR` | 0.15 | Minimum connection weight |
| Default weight | `DEFAULT_WEIGHT` | 0.18 | Initial weight for new connections |
| Weight exponent | `WEIGHT_EXPONENT` | 2.0 | Fixed w^2 in Kuramoto step |

### Types to Implement

```rust
/// Result of a single Hebbian STDP update cycle.
#[derive(Debug, Clone, Default)]
pub struct StdpResult {
    /// Number of LTP (potentiation) updates applied.
    pub ltp_count: usize,
    /// Number of LTD (depression) updates applied.
    pub ltd_count: usize,
    /// Number of connections at weight floor.
    pub at_floor_count: usize,
    /// Total weight change (absolute sum).
    pub total_weight_change: f64,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `apply_stdp` | `pub fn apply_stdp(network: &mut CouplingNetwork, spheres: &HashMap<PaneId, PaneSphere>) -> StdpResult` | Full STDP cycle: LTP + LTD + floor |
| `decay_all_weights` | `pub fn decay_all_weights(network: &mut CouplingNetwork, decay_factor: f64)` | Uniform weight decay (e.g. 0.999 per tick) |
| `compute_ltp_rate` | `pub fn compute_ltp_rate(sphere_a: &PaneSphere, sphere_b: &PaneSphere) -> f64` | LTP with burst + newcomer multipliers |
| `are_coactive` | `pub fn are_coactive(sphere_a: &PaneSphere, sphere_b: &PaneSphere) -> bool` | Both `Working` and neither opted out |

### STDP Update Flow

```
apply_stdp(network, spheres):
  1. Collect working = { id | spheres[id].status == Working }
  2. For each connection (from, to) in network:
     a. If either sphere has opt_out_hebbian -> skip
     b. If both in working set:
        - ltp = HEBBIAN_LTP
        - If burst detected: ltp *= HEBBIAN_BURST_MULTIPLIER (3x)
        - If newcomer: ltp *= HEBBIAN_NEWCOMER_MULTIPLIER (2x)
        - new_weight = (old_weight + ltp).min(1.0)
        - record LTP
     c. Else:
        - new_weight = (old_weight - HEBBIAN_LTD).max(HEBBIAN_WEIGHT_FLOOR)
        - record LTD
     d. network.set_weight(from, to, new_weight)
  3. Return StdpResult { ltp_count, ltd_count, at_floor_count, total_weight_change }
```

### Tests

| Test | Validates |
|------|-----------|
| `test_coactive_spheres_get_ltp` | Weight increases for Working+Working pair |
| `test_inactive_pair_gets_ltd` | Weight decreases for non-co-active pair |
| `test_weight_floor_enforced` | Weight never drops below 0.15 |
| `test_weight_cap_at_1_0` | Weight never exceeds 1.0 |
| `test_burst_multiplier_3x` | High activity_30s triggers 3x LTP |
| `test_newcomer_multiplier_2x` | Low total_steps triggers 2x LTP |
| `test_opt_out_respected` | opt_out_hebbian skips connection entirely |
| `test_stdp_result_counts` | ltp_count + ltd_count + skipped = connection count |
| `test_total_weight_change_positive` | Absolute sum is always non-negative |
| `test_decay_all_weights` | Uniform decay reduces all weights proportionally |
| `test_decay_respects_floor` | Decay stops at weight floor |
| `test_compute_ltp_rate_base` | Base case returns 0.01 |
| `test_compute_ltp_rate_burst_newcomer` | Both multipliers stack: 0.01 * 3 * 2 = 0.06 |
| `test_asymmetric_hebbian_flag` | Causal STDP uses directional weights |
| `test_empty_spheres_no_crash` | Zero spheres -> empty StdpResult |

### Cross-References

- [[Session 045 Arena -- 10-hebbian-operational-topology]]
- [[Vortex Sphere Brain-Body Architecture]]
- ORAC_PLAN.md Phase 2 Detail (step 2)
- MEMORY.md: "Hebbian STDP: LTP 0.01 (3x burst, 2x newcomer), LTD 0.002, weight floor 0.05"
- **P05** (FMA for weight arithmetic)
- **P01** (no direct phase use, but weights feed into phase step)
- m12 `trigger_stdp` in L3 hooks invokes this on every PostToolUse

---

## m19 -- Buoy Network

**Source:** `src/m4_intelligence/m19_buoy_network.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types` (`PaneId`, `PaneSphere`, `Buoy`, `Point3D`), `m04_constants` (`ACTIVATION_THRESHOLD`, `TUNNEL_THRESHOLD`)
**Hot-Swap:** DROP-IN from PV2 M20

### Design Decisions

- **Network-level buoy operations, not per-sphere.** Individual buoy management lives
  in `PaneSphere` methods. This module provides cross-sphere analysis: fleet-wide
  health, tunnel discovery between spheres, centroid computation.
- **Activation threshold 0.3, influence radius 0.50.** Buoys below 0.3 activation
  are candidates for pruning. Buoys within 0.50 radians of a query point contribute
  to spatial recall.
- **Tunnel discovery.** Two spheres with buoys within `TUNNEL_THRESHOLD` (0.8 rad)
  angular distance are considered to have a "tunnel" -- a shared spatial memory region.
  Tunnels indicate thematic overlap and correlate with high Hebbian weights.

### Types to Implement

```rust
/// Health summary for a single sphere's buoy network.
#[derive(Debug, Clone)]
pub struct BuoyHealth {
    /// Sphere ID.
    pub sphere_id: PaneId,
    /// Number of buoys.
    pub buoy_count: usize,
    /// Mean drift distance from home position.
    pub mean_drift: f64,
    /// Maximum drift distance.
    pub max_drift: f64,
    /// Total activation count across all buoys.
    pub total_activations: u64,
    /// Whether any buoy has drifted significantly (> 0.5 rad).
    pub has_drifted: bool,
}

/// Fleet-wide buoy network summary.
#[derive(Debug, Clone, Default)]
pub struct FleetBuoyStats {
    /// Total buoys across all spheres.
    pub total_buoys: usize,
    /// Mean buoy count per sphere.
    pub mean_buoys_per_sphere: f64,
    /// Total activations fleet-wide.
    pub total_activations: u64,
    /// Number of spheres with drifted buoys.
    pub spheres_with_drift: usize,
    /// Number of inter-sphere tunnels detected.
    pub tunnel_count: usize,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `buoy_health` | `pub fn buoy_health(sphere: &PaneSphere) -> BuoyHealth` | Per-sphere buoy health metrics |
| `fleet_buoy_stats` | `pub fn fleet_buoy_stats(spheres: &HashMap<PaneId, PaneSphere>) -> FleetBuoyStats` | Fleet-wide buoy analysis |
| `buoy_centroid` | `pub fn buoy_centroid(sphere: &PaneSphere) -> Point3D` | Mean position of sphere's buoys |
| `nearest_buoy` | `pub fn nearest_buoy<'a>(sphere: &'a PaneSphere, point: &Point3D) -> Option<&'a Buoy>` | Closest buoy to a query point |

### Tests

| Test | Validates |
|------|-----------|
| `test_buoy_health_empty_sphere` | Zero buoys -> safe defaults |
| `test_buoy_health_drift_detection` | `has_drifted` true when max_drift > 0.5 |
| `test_fleet_buoy_stats` | Aggregate statistics correct |
| `test_buoy_centroid` | Mean position computed correctly |
| `test_nearest_buoy` | Returns closest by angular distance |
| `test_nearest_buoy_empty` | No buoys -> None |
| `test_tunnel_count` | Tunnels detected between close buoys |

### Cross-References

- [[Vortex Sphere Brain-Body Architecture]]
- MEMORY.md: "activation_threshold: 0.3, influence_radius: 0.50"
- m15 `CouplingNetwork` for phase context
- m18 STDP for tunnel-weight correlation

---

## m20 -- Semantic Router

**Source:** `src/m4_intelligence/m20_semantic_router.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types` (`PaneId`, `PaneSphere`), `m15_coupling_network` (`CouplingNetwork`), `m18_hebbian_stdp` (weight queries)
**Hot-Swap:** NEW (ORAC-specific)

### Design Decisions

- **Content-aware dispatch using Hebbian weights as domain affinity.** When a task
  needs dispatching to a fleet member, the semantic router scores candidates based
  on their Hebbian connection strength to the requesting sphere (historical
  co-activation), the tool phase alignment (a sphere that has been doing Read-heavy
  work is preferred for Read tasks), and explicit domain tags.
- **Domain affinity scoring.** Each sphere accumulates a domain profile from its tool
  usage: `{ "rust": 0.8, "docs": 0.3, "testing": 0.6 }`. Tasks carry a domain tag.
  The router matches task domain to sphere affinity for content-aware routing.
- **Hebbian weight as trust signal.** Spheres with high mutual Hebbian weight have
  demonstrated co-activation success. The router uses weight-squared (matching the
  Kuramoto step exponent) as a trust signal for dispatch preference.
- **Circuit breaker integration.** The router consults m21 before including a sphere
  in candidate scoring. Open-circuit spheres are excluded from dispatch.

### Types to Implement

```rust
/// Domain affinity profile for a sphere.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DomainProfile {
    /// Domain -> affinity score (0.0-1.0).
    pub affinities: HashMap<String, f64>,
    /// Total tool calls that contributed to this profile.
    pub sample_count: u64,
}

impl DomainProfile {
    /// Update affinity for a domain based on a tool call.
    ///
    /// Uses EMA with alpha=0.1 to smooth updates (P05).
    pub fn update(&mut self, domain: &str, weight: f64) {
        let current = self.affinities.get(domain).copied().unwrap_or(0.0);
        let alpha = 0.1;
        let updated = alpha.mul_add(weight - current, current);
        self.affinities.insert(domain.to_owned(), updated);
        self.sample_count += 1;
    }

    /// Get affinity for a specific domain.
    #[must_use]
    pub fn affinity(&self, domain: &str) -> f64 {
        self.affinities.get(domain).copied().unwrap_or(0.0)
    }
}

/// A task dispatch request to the semantic router.
#[derive(Debug, Clone)]
pub struct DispatchRequest {
    /// Source sphere requesting dispatch.
    pub from: PaneId,
    /// Task domain tag (e.g. "rust", "docs", "testing").
    pub domain: Option<String>,
    /// Tool type hint (maps to `ToolPhaseRegion`).
    pub tool_hint: Option<String>,
    /// Task description for context.
    pub description: String,
}

/// Scored dispatch candidate.
#[derive(Debug, Clone)]
pub struct DispatchCandidate {
    /// Candidate sphere ID.
    pub sphere_id: PaneId,
    /// Composite score (higher = better fit).
    pub score: f64,
    /// Hebbian weight component of score.
    pub hebbian_component: f64,
    /// Domain affinity component of score.
    pub domain_component: f64,
    /// Phase alignment component of score.
    pub phase_component: f64,
}

/// Semantic router configuration.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Weight of Hebbian component in composite score.
    pub hebbian_weight: f64,
    /// Weight of domain affinity component.
    pub domain_weight: f64,
    /// Weight of phase alignment component.
    pub phase_weight: f64,
    /// Minimum score to be considered a candidate.
    pub min_score: f64,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            hebbian_weight: 0.4,
            domain_weight: 0.35,
            phase_weight: 0.25,
            min_score: 0.1,
        }
    }
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `route` | `pub fn route(network: &CouplingNetwork, spheres: &HashMap<PaneId, PaneSphere>, profiles: &HashMap<PaneId, DomainProfile>, request: &DispatchRequest, config: &RouterConfig) -> Vec<DispatchCandidate>` | Score and rank all candidates |
| `score_candidate` | `fn score_candidate(network: &CouplingNetwork, from: &PaneId, to: &PaneId, profile: &DomainProfile, domain: Option<&str>, tool_hint: Option<&str>, config: &RouterConfig) -> f64` | Composite score for one candidate |
| `update_profile` | `pub fn update_profile(profiles: &mut HashMap<PaneId, DomainProfile>, sphere_id: &PaneId, tool_name: &str)` | Update domain profile from tool usage |
| `tool_to_domain` | `pub fn tool_to_domain(tool_name: &str) -> &'static str` | Map tool name to domain tag |

### Scoring Formula

```
score = config.hebbian_weight * w^2          // Hebbian trust (weight squared)
      + config.domain_weight  * affinity     // Domain match [0, 1]
      + config.phase_weight   * phase_align  // Phase alignment [0, 1]

where:
  w = network.get_weight(from, to)
  affinity = profile.affinity(domain)
  phase_align = 1.0 - |phase_diff(from, to)| / PI   // closer phase = higher align
```

### Tests

| Test | Validates |
|------|-----------|
| `test_route_returns_sorted_candidates` | Candidates sorted by score descending |
| `test_route_excludes_self` | Source sphere not in candidates |
| `test_hebbian_weight_influences_score` | Higher Hebbian weight -> higher score |
| `test_domain_affinity_influences_score` | Matching domain -> higher score |
| `test_phase_alignment_influences_score` | Closer phase -> higher score |
| `test_min_score_filter` | Candidates below min_score excluded |
| `test_update_profile_ema` | EMA smoothing on domain updates |
| `test_tool_to_domain_mapping` | Known tools map to correct domains |
| `test_empty_network_returns_empty` | No spheres -> no candidates |
| `test_config_weight_sum` | Component weights should sum to 1.0 for interpretability |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] Semantic Router
- ORAC_PLAN.md Phase 2 Detail (step 3), Feature Backlog #8
- m15 `CouplingNetwork` for Hebbian weights
- m18 STDP for weight update source
- m12 tool hooks for tool-to-domain signal
- **P05** (FMA in scoring), **P01** (phase_diff wrapping)

---

## m21 -- Circuit Breaker

**Source:** `src/m4_intelligence/m21_circuit_breaker.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types` (`PaneId`, `Timestamp`), `m04_constants`
**Hot-Swap:** NEW (ORAC-specific, tower-resilience pattern)

### Design Decisions

- **Per-pane circuit breaker FSM.** Each sphere in the fleet has an independent circuit
  breaker tracking its health. The FSM has three states: Closed (healthy, traffic flows),
  Open (unhealthy, traffic blocked), HalfOpen (probing, limited traffic allowed).
- **tower-resilience pattern.** Follows the standard circuit breaker pattern from
  distributed systems: consecutive failures trigger Open, a timeout triggers HalfOpen,
  a successful probe triggers Closed.
- **State transitions emit bus events (P09).** Every transition (Closed->Open,
  Open->HalfOpen, HalfOpen->Closed, HalfOpen->Open) emits an `OracEvent` for
  observability and conductor awareness.
- **Configurable thresholds.** Failure threshold (consecutive failures before opening),
  success threshold (consecutive successes in HalfOpen before closing), and recovery
  timeout (seconds before Open->HalfOpen) are all configurable.
- **Integration with semantic router.** m20 consults the circuit breaker before
  including a sphere in dispatch candidates. Open-circuit spheres are excluded.

### Circuit Breaker FSM

```
                    failure_count >= threshold
     ┌────────┐ ─────────────────────────────> ┌────────┐
     │ CLOSED │                                 │  OPEN  │
     └────────┘ <───────────────────────────── └────────┘
         ^        success in HalfOpen               │
         │                                          │ recovery_timeout elapsed
         │        ┌──────────┐                      │
         └─────── │ HALFOPEN │ <────────────────────┘
     success >=   └──────────┘
     threshold        │
                      │ failure in HalfOpen
                      └──────────────> OPEN
```

### Types to Implement

```rust
/// Circuit breaker state for a single pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakerState {
    /// Healthy. All traffic flows normally.
    Closed,
    /// Unhealthy. All traffic is blocked.
    Open,
    /// Probing. Limited traffic allowed to test recovery.
    HalfOpen,
}

/// Circuit breaker for a single pane-sphere.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Current FSM state.
    state: BreakerState,
    /// Consecutive failure count.
    failure_count: u32,
    /// Consecutive success count (in `HalfOpen`).
    success_count: u32,
    /// Timestamp when breaker entered Open state (for recovery timeout).
    opened_at: Option<Timestamp>,
    /// Configuration.
    config: BreakerConfig,
}

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct BreakerConfig {
    /// Consecutive failures before Closed -> Open.
    pub failure_threshold: u32,
    /// Consecutive successes in `HalfOpen` before -> Closed.
    pub success_threshold: u32,
    /// Seconds before Open -> `HalfOpen` (recovery probe window).
    pub recovery_timeout_secs: u64,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            recovery_timeout_secs: 30,
        }
    }
}

/// State transition event emitted on every breaker state change (P09).
#[derive(Debug, Clone)]
pub struct BreakerTransition {
    /// Sphere whose breaker transitioned.
    pub sphere_id: PaneId,
    /// Previous state.
    pub from: BreakerState,
    /// New state.
    pub to: BreakerState,
    /// When the transition occurred.
    pub timestamp: Timestamp,
    /// Failure count at transition time.
    pub failure_count: u32,
}

/// Fleet-wide circuit breaker registry.
#[derive(Debug, Clone, Default)]
pub struct BreakerRegistry {
    /// Per-sphere breakers.
    breakers: HashMap<PaneId, CircuitBreaker>,
    /// Configuration shared by all breakers.
    config: BreakerConfig,
}
```

### Key Functions

| Function | Signature | Notes |
|----------|-----------|-------|
| `CircuitBreaker::new` | `pub fn new(config: BreakerConfig) -> Self` | Initial state: Closed |
| `record_success` | `pub fn record_success(&mut self, now: Timestamp) -> Option<BreakerTransition>` | Record success, maybe transition HalfOpen->Closed |
| `record_failure` | `pub fn record_failure(&mut self, now: Timestamp) -> Option<BreakerTransition>` | Record failure, maybe transition Closed->Open or HalfOpen->Open |
| `check_recovery` | `pub fn check_recovery(&mut self, now: Timestamp) -> Option<BreakerTransition>` | Check if recovery timeout elapsed, Open->HalfOpen |
| `is_call_allowed` | `pub fn is_call_allowed(&self) -> bool` | Closed: true, Open: false, HalfOpen: true (probe) |
| `state` | `pub fn state(&self) -> BreakerState` | Current FSM state |
| `BreakerRegistry::register` | `pub fn register(&mut self, id: PaneId)` | Add breaker for new sphere |
| `BreakerRegistry::deregister` | `pub fn deregister(&mut self, id: &PaneId)` | Remove breaker for departing sphere |
| `BreakerRegistry::is_healthy` | `pub fn is_healthy(&self, id: &PaneId) -> bool` | True if Closed or HalfOpen |
| `BreakerRegistry::fleet_health` | `pub fn fleet_health(&self) -> FleetHealthSummary` | Count of Closed/Open/HalfOpen |

### Tests

| Test | Validates |
|------|-----------|
| `test_initial_state_closed` | New breaker starts Closed |
| `test_failures_trigger_open` | 5 consecutive failures -> Open |
| `test_open_blocks_calls` | `is_call_allowed()` returns false when Open |
| `test_recovery_timeout_triggers_halfopen` | After 30s, Open -> HalfOpen |
| `test_halfopen_success_closes` | 2 successes in HalfOpen -> Closed |
| `test_halfopen_failure_reopens` | 1 failure in HalfOpen -> Open |
| `test_closed_success_resets_count` | Success in Closed resets failure_count to 0 |
| `test_transition_emits_event` | State change returns `BreakerTransition` (P09) |
| `test_no_transition_returns_none` | Success in Closed -> no transition event |
| `test_registry_register_deregister` | Breakers added and removed correctly |
| `test_registry_fleet_health` | Correct counts per state |
| `test_config_custom_thresholds` | Non-default thresholds work correctly |
| `test_halfopen_allows_probe` | `is_call_allowed()` returns true in HalfOpen |

### Cross-References

- [[Session 050 -- ORAC Sidecar Architecture]] Circuit Breaker
- ORAC_PLAN.md Phase 2 Detail (step 4), Feature Backlog #3
- **P09** (state transitions emit events)
- **P02** (interior mutability for `BreakerRegistry` when shared)
- m20 semantic router consults breaker state before dispatch

---

## Layer-Wide Invariants

### Phase Wrapping (P01)

ALL phase arithmetic in L4 MUST end with `.rem_euclid(TAU)`. This includes:
- `CouplingNetwork::step()` -- phase updates
- `CouplingNetwork::kick_phases_apart()` -- phase perturbation
- `NeighborInfo::phase_diff` -- via `phase_diff()` helper
- `DispatchCandidate` phase alignment scoring in m20

Violation of P01 causes phase drift to large values where floating-point precision
degrades. At `theta > 1e6`, `sin(theta)` precision drops below 1e-10, making the
Kuramoto step unreliable.

### FMA Everywhere (P05)

Multi-step float arithmetic uses `f64::mul_add()`:
- EMA smoothing in auto-K: `alpha.mul_add(new - current, current)`
- Kuramoto step: `KURAMOTO_DT.mul_add(omega + coupling, 0.0)`
- Score composition in semantic router
- Weight update in STDP

### k_mod Invariant Group (AP17)

`k_modulation` and `WEIGHT_EXPONENT` form an invariant group. They must be updated
atomically (same function call, same lock scope). BUG-035 in the Maintenance Engine
demonstrated that mutating `k_modulation` alone while `WEIGHT_EXPONENT` is stale
creates transient inconsistency. In ORAC, `WEIGHT_EXPONENT` is a compile-time constant
(2.0), eliminating this risk -- but the principle applies to any future runtime-tunable
coupling parameters.

### Hot-Swap Compatibility

m15-m19 are DROP-IN from PV2. When updating these modules from upstream PV2 changes:
1. Verify `crate::` import paths match ORAC layout (PV2 uses `m1_foundation`, ORAC uses `m1_core`)
2. Run full quality gate after copy
3. Check that `PaneId`, `PaneSphere`, and `OrderParameter` types are source-compatible
4. Verify `m04_constants` values match (ORAC may diverge from PV2 constants over time)

---

## Implementation Dependencies

```
m15_coupling_network (foundation — no L4 deps)
  ├── m01_core_types (PaneId, OrderParameter, phase_diff)
  └── m04_constants (KURAMOTO_DT, DEFAULT_WEIGHT, WEIGHT_EXPONENT)

m16_auto_k
  ├── m15_coupling_network (CouplingNetwork)
  └── m04_constants (K_MOD_MIN, K_MOD_MAX, COUPLING_STEPS_PER_TICK)

m17_topology
  ├── m15_coupling_network (CouplingNetwork)
  ├── m01_core_types (PaneId, phase_diff)
  └── m04_constants

m18_hebbian_stdp
  ├── m15_coupling_network (CouplingNetwork, set_weight)
  ├── m01_core_types (PaneId, PaneSphere, PaneStatus)
  └── m04_constants (HEBBIAN_LTP, HEBBIAN_LTD, HEBBIAN_WEIGHT_FLOOR, ...)

m19_buoy_network
  ├── m01_core_types (PaneId, PaneSphere, Buoy, Point3D)
  └── m04_constants (ACTIVATION_THRESHOLD, TUNNEL_THRESHOLD)

m20_semantic_router (NEW)
  ├── m15_coupling_network (get_weight, phases)
  ├── m18_hebbian_stdp (weight queries)
  ├── m21_circuit_breaker (is_healthy check)
  └── m01_core_types (PaneId, PaneSphere)

m21_circuit_breaker (NEW)
  ├── m01_core_types (PaneId, Timestamp)
  └── m04_constants (if thresholds are centralized)
```
