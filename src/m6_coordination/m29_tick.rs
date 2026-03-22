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
//!   Phase 4: Hebbian STDP on local coupling snapshot (TODO: Phase 2 build)
//!   Phase 5: Governance actuator (TODO: Phase 4 build)
//! ```
//!
//! ## ORAC Adaptation Notes
//! - No direct sphere mutation — phases are read-only snapshots from PV2
//! - `CouplingNetwork` integration deferred to Phase 2 (requires `intelligence` feature)
//! - `BridgeSet` integration deferred to Phase 3 build
//! - Governance actuator deferred to Phase 4

use std::time::Instant;

use crate::m1_core::{
    field_state::{AppState, FieldDecision, FieldState},
    m01_core_types::OrderParameter,
    m04_constants,
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
/// # Future Integration (TODO)
/// - Phase 2: Accept `Option<&mut CouplingNetwork>` for local Hebbian STDP.
/// - Phase 3: Accept `Option<&BridgeSet>` for bridge `k_mod` application.
/// - Phase 4: Governance actuator behind `governance` feature gate.
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

    // ── Phase 3: Conductor advisory decision ──
    let p3_start = Instant::now();
    let decision = conductor.decide(state);
    timings.conductor_ms = p3_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 4: Hebbian STDP (placeholder) ──
    // TODO: Phase 2 build — accept CouplingNetwork, call tick_hebbian()
    // Requires `intelligence` feature and m4_intelligence::m15_coupling_network
    let p4_start = Instant::now();
    timings.hebbian_ms = p4_start.elapsed().as_secs_f64() * 1000.0;

    // ── Phase 5: Governance actuator (placeholder) ──
    // TODO: Phase 4 build — process approved governance proposals

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
    }

    // ── PhaseTiming ──

    #[test]
    fn phase_timing_default_zero() {
        let t = PhaseTiming::default();
        assert!(t.field_state_ms.abs() < f64::EPSILON);
        assert!(t.conductor_ms.abs() < f64::EPSILON);
        assert!(t.hebbian_ms.abs() < f64::EPSILON);
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
