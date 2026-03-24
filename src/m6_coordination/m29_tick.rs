//! # M29: Tick Orchestrator
//!
//! Sidecar tick loop that updates cached state from PV2 and runs local
//! intelligence passes. Unlike PV2's tick (which owns the field), ORAC's
//! tick **observes** PV2 state and may run advisory Hebbian STDP, conductor
//! decisions, and field state recomputation on the cached snapshot.
//!
//! ## Layer: L6 (Coordination) | Module: M29
//! ## Dependencies: L1 (`field_state`, `m01_core_types`, `m04_constants`),
//!   L6 (`m27_conductor`)
//!
//! ## Architecture
//! ```text
//! tick_once()
//!   Phase 1: Advance tick counter, warmup
//!   Phase 2: Recompute field state from cached spheres
//!   Phase 3: Conductor decision (advisory)
//!   Phase 4: (reserved — use tick_with_hebbian for STDP)
//!   Phase 5: Governance actuator (planned)
//!
//! tick_with_hebbian()
//!   Phases 1-3: same as tick_once
//!   Phase 4: Hebbian STDP on coupling network (intelligence feature)
//!   Phase 5: Governance actuator (planned)
//! ```
//!
//! ## ORAC Adaptation Notes
//! - No direct sphere mutation — phases are read-only snapshots from PV2
//! - `CouplingNetwork` integrated via [`tick_with_hebbian`] (requires `intelligence` feature)
//! - `BridgeSet` integration deferred to Phase 3 build
//! - Governance actuator deferred to Phase 5 build

use std::time::Instant;

use crate::m1_core::{
    field_state::{AppState, FieldDecision, FieldState},
    m01_core_types::OrderParameter,
    m04_constants,
};
#[cfg(feature = "intelligence")]
use crate::m4_intelligence::{
    m15_coupling_network::CouplingNetwork,
    m18_hebbian_stdp::apply_stdp,
};
use super::m27_conductor::Conductor;

// ──────────────────────────────────────────────────────────────
// TickResult
// ──────────────────────────────────────────────────────────────

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
    /// Whether a Hebbian STDP pass was executed in Phase 4.
    pub hebbian_updated: bool,
    /// Whether governance was active in Phase 5 (`r_target` override in effect).
    pub governance_active: bool,
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
    /// Phase 5: governance actuator.
    pub governance_ms: f64,
}

// ──────────────────────────────────────────────────────────────
// Tick orchestrator
// ──────────────────────────────────────────────────────────────

/// Run one complete ORAC sidecar tick.
///
/// This is the main loop body called by the sidecar's async tick timer.
/// It operates on cached state — no direct PV2 mutation occurs here.
///
/// # Arguments
/// - `state`: Mutable reference to the sidecar's cached `AppState`.
/// - `conductor`: The PI breathing controller.
///
/// # Future Integration
/// - Phase 3: Accept `Option<&BridgeSet>` for bridge `k_mod` application.
/// - Phase 5: Governance actuator behind `governance` feature gate.
///
/// For Hebbian STDP integration (Phase 4), use [`tick_with_hebbian`] instead.
pub fn tick_once(
    state: &mut AppState,
    conductor: &Conductor,
) -> TickResult {
    let tick_start = Instant::now();
    state.tick += 1;
    let current_tick = state.tick;

    // Handle warmup
    if state.is_warming_up() {
        state.tick_warmup();
    }

    let mut timings = PhaseTiming::default();

    // ── Phase 2: Field state recomputation ──
    let p2_start = Instant::now();
    let field_state = FieldState::compute(&state.spheres, current_tick);
    state.push_r(field_state.order.r);
    state.field = field_state.clone();
    timings.field_state_ms = p2_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 3: Conductor advisory decision + state updates ──
    // Uses decide_and_update to also decrement divergence_cooldown,
    // update EMAs, and write prev_decision_action (BUG-L1-003 fix).
    let p3_start = Instant::now();
    let decision = conductor.decide_and_update(state);
    timings.conductor_ms = p3_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 4: Hebbian STDP (placeholder in base tick_once) ──
    let p4_start = Instant::now();
    timings.hebbian_ms = p4_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 5: Governance check ──
    let p5_start = Instant::now();
    let governance_active = check_governance(state);
    timings.governance_ms = p5_start.elapsed().as_secs_f64() * 1000.0;

    // ── Snapshot decision ──
    let should_snapshot = current_tick % m04_constants::SNAPSHOT_INTERVAL == 0;

    let order_parameter = field_state.order;
    let sphere_count = state.spheres.len();
    let total_ms = tick_start.elapsed().as_secs_f64() * 1000.0;

    TickResult {
        field_state,
        decision,
        order_parameter,
        phase_timings: timings,
        total_ms,
        tick: current_tick,
        sphere_count,
        should_snapshot,
        hebbian_updated: false,
        governance_active,
    }
}

/// Phase 5 governance check.
///
/// Checks for any pending r-target override from the governance system
/// and applies it. Returns `true` if a governance action was taken.
///
/// Currently minimal — will expand with proposal processing in future.
/// Check and validate governance overrides from RALPH evolution proposals.
///
/// Validates that `r_target_override` is within the safe range `[0.5, 1.0]`.
/// Clears invalid overrides and logs governance state transitions.
///
/// Returns `true` if governance is active (override in effect or cleared).
fn check_governance(state: &mut AppState) -> bool {
    // If governance has overridden the r_target, validate the value
    if let Some(override_r) = state.r_target_override {
        // Sanity-check: r_target must be in [0.5, 1.0]
        if !(0.5..=1.0).contains(&override_r) {
            tracing::warn!(
                override_r,
                "Phase 5: governance r_target override out of range, clearing"
            );
            state.r_target_override = None;
            return true;
        }
        // Override is valid and in effect — governance is active
        return true;
    }
    false
}

/// Run one ORAC tick with Hebbian STDP pass in Phase 4.
///
/// Extends [`tick_once`] by running a Hebbian STDP cycle on the
/// coupling network using the cached sphere states. Connections
/// between co-active spheres are potentiated (LTP), while inactive
/// connections are depressed (LTD).
///
/// # Arguments
/// - `state`: Mutable reference to the sidecar's cached `AppState`.
/// - `conductor`: The PI breathing controller.
/// - `coupling`: The coupling network to apply STDP on.
///
/// # Returns
/// A [`TickResult`] with `hebbian_updated = true` and `StdpResult`
/// metrics applied.
#[cfg(feature = "intelligence")]
pub fn tick_with_hebbian(
    state: &mut AppState,
    conductor: &Conductor,
    coupling: &mut CouplingNetwork,
) -> TickResult {
    let tick_start = Instant::now();
    state.tick += 1;
    let current_tick = state.tick;

    if state.is_warming_up() {
        state.tick_warmup();
    }

    let mut timings = PhaseTiming::default();

    // ── Phase 2: Field state recomputation ──
    let p2_start = Instant::now();
    let field_state = FieldState::compute(&state.spheres, current_tick);
    state.push_r(field_state.order.r);
    state.field = field_state.clone();
    timings.field_state_ms = p2_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 3: Conductor advisory decision ──
    let p3_start = Instant::now();
    let decision = conductor.decide_and_update(state);
    timings.conductor_ms = p3_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 4: Hebbian STDP pass ──
    let p4_start = Instant::now();
    let stdp_result = apply_stdp(coupling, &state.spheres);
    timings.hebbian_ms = p4_start.elapsed().as_secs_f64() * 1000.0;

    if stdp_result.ltp_count > 0 || stdp_result.ltd_count > 0 {
        tracing::debug!(
            ltp = stdp_result.ltp_count,
            ltd = stdp_result.ltd_count,
            delta = format!("{:.4}", stdp_result.total_weight_change),
            "Phase 4: Hebbian STDP applied"
        );
    }

    // ── Phase 5: Governance check ──
    let p5_start = Instant::now();
    let governance_active = check_governance(state);
    timings.governance_ms = p5_start.elapsed().as_secs_f64() * 1000.0;

    let should_snapshot = current_tick % m04_constants::SNAPSHOT_INTERVAL == 0;
    let order_parameter = field_state.order;
    let sphere_count = state.spheres.len();
    let total_ms = tick_start.elapsed().as_secs_f64() * 1000.0;

    TickResult {
        field_state,
        decision,
        order_parameter,
        phase_timings: timings,
        total_ms,
        tick: current_tick,
        sphere_count,
        should_snapshot,
        hebbian_updated: true,
        governance_active,
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::m1_core::m01_core_types::{PaneId, PaneSphere};

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn make_state_with_spheres(n: usize) -> AppState {
        let mut state = AppState::default();
        for i in 0..n {
            let id = format!("s{i}");
            let mut sphere = PaneSphere::new(pid(&id), format!("sphere-{i}"));
            // Spread phases so order parameter is non-trivial
            #[allow(clippy::cast_precision_loss)]
            let phase = (i as f64 / n.max(1) as f64) * std::f64::consts::TAU * 0.3;
            sphere.phase = phase;
            state.spheres.insert(pid(&id), sphere);
        }
        // Advance past warmup
        for _ in 0..15 {
            state.tick_warmup();
        }
        state
    }

    // ── TickResult ──

    #[test]
    fn tick_result_fields_populated() {
        let mut state = make_state_with_spheres(3);
        let conductor = Conductor::new();
        let result = tick_once(&mut state, &conductor);
        assert!(result.total_ms >= 0.0);
        assert_eq!(result.tick, 1);
        assert_eq!(result.sphere_count, 3);
        assert!(!result.hebbian_updated);
        assert!(!result.governance_active);
    }

    #[test]
    fn tick_governance_active_with_override() {
        let mut state = make_state_with_spheres(3);
        state.r_target_override = Some(0.85);
        let conductor = Conductor::new();
        let result = tick_once(&mut state, &conductor);
        assert!(result.governance_active);
    }

    #[test]
    fn tick_governance_clears_invalid_override() {
        let mut state = make_state_with_spheres(3);
        state.r_target_override = Some(0.1); // Out of [0.5, 1.0]
        let conductor = Conductor::new();
        let result = tick_once(&mut state, &conductor);
        assert!(result.governance_active);
        assert!(state.r_target_override.is_none());
    }

    // ── PhaseTiming ──

    #[test]
    fn phase_timing_default_zero() {
        let t = PhaseTiming::default();
        assert!(t.field_state_ms.abs() < f64::EPSILON);
        assert!(t.conductor_ms.abs() < f64::EPSILON);
        assert!(t.hebbian_ms.abs() < f64::EPSILON);
        assert!(t.governance_ms.abs() < f64::EPSILON);
    }

    // ── tick_once ──

    #[test]
    fn tick_empty_state() {
        let mut state = AppState::default();
        let conductor = Conductor::new();
        let result = tick_once(&mut state, &conductor);
        assert_eq!(result.tick, 1);
        assert_eq!(result.sphere_count, 0);
    }

    #[test]
    fn tick_increments_counter() {
        let mut state = AppState::default();
        let conductor = Conductor::new();
        tick_once(&mut state, &conductor);
        assert_eq!(state.tick, 1);
        tick_once(&mut state, &conductor);
        assert_eq!(state.tick, 2);
    }

    #[test]
    fn tick_updates_r_history() {
        let mut state = make_state_with_spheres(3);
        let conductor = Conductor::new();
        for _ in 0..5 {
            tick_once(&mut state, &conductor);
        }
        assert_eq!(state.r_history.len(), 5);
    }

    #[test]
    fn tick_order_parameter_bounded() {
        let mut state = make_state_with_spheres(10);
        let conductor = Conductor::new();
        for _ in 0..20 {
            let result = tick_once(&mut state, &conductor);
            assert!(result.order_parameter.r >= 0.0);
            assert!(result.order_parameter.r <= 1.0);
        }
    }

    #[test]
    fn tick_timings_non_negative() {
        let mut state = make_state_with_spheres(3);
        let conductor = Conductor::new();
        let result = tick_once(&mut state, &conductor);
        assert!(result.phase_timings.field_state_ms >= 0.0);
        assert!(result.phase_timings.conductor_ms >= 0.0);
        assert!(result.phase_timings.hebbian_ms >= 0.0);
    }

    #[test]
    fn tick_single_sphere() {
        let mut state = make_state_with_spheres(1);
        let conductor = Conductor::new();
        let result = tick_once(&mut state, &conductor);
        assert_eq!(result.sphere_count, 1);
    }

    // ── Multi-tick stability ──

    #[test]
    fn multi_tick_stability() {
        let mut state = make_state_with_spheres(5);
        let conductor = Conductor::new();
        for _ in 0..50 {
            let result = tick_once(&mut state, &conductor);
            assert!(result.total_ms >= 0.0);
            assert!(result.order_parameter.r.is_finite());
        }
        assert_eq!(state.tick, 50);
    }

    #[cfg(feature = "intelligence")]
    #[test]
    fn tick_with_hebbian_sets_flag() {
        let mut state = make_state_with_spheres(3);
        let conductor = Conductor::new();
        let mut coupling = CouplingNetwork::default();
        let result = tick_with_hebbian(&mut state, &conductor, &mut coupling);
        assert!(result.hebbian_updated);
        assert!(result.phase_timings.hebbian_ms >= 0.0);
    }

    #[test]
    fn tick_snapshot_at_interval() {
        let mut state = make_state_with_spheres(3);
        let conductor = Conductor::new();
        // Tick until snapshot interval
        let mut snapshot_seen = false;
        for _ in 0..m04_constants::SNAPSHOT_INTERVAL + 1 {
            let result = tick_once(&mut state, &conductor);
            if result.should_snapshot {
                snapshot_seen = true;
            }
        }
        assert!(snapshot_seen, "should_snapshot should trigger at interval");
    }
}
