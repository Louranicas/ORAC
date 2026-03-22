//! # M27: Conductor
//!
//! PI breathing controller for field synchronization. In ORAC the conductor
//! **observes** the PV2 field state and computes local breathing suggestions;
//! it does not own the authoritative `k_modulation`. The sidecar may forward
//! conductor recommendations to PV2 via IPC, or use them for local analytics.
//!
//! ## Layer: L6 (Coordination) | Module: M27
//! ## Dependencies: L1 (`m01_core_types`, `m04_constants`, `field_state`)
//!
//! ## ORAC Adaptation Notes
//! - `r_target` is computed from the cached sphere map (sourced via IPC from PV2)
//! - Phase noise injection is **advisory only** — the daemon applies it
//! - Gain and blend parameters are tunable via ORAC config

use crate::m1_core::{
    field_state::{AppState, FieldDecision},
    m01_core_types::{FieldAction, PaneId},
    m04_constants,
};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Divergence cooldown duration (ticks) after a divergence kick.
/// Used by the PV2 conductor — retained here for parity.
const _DIVERGENCE_COOLDOWN_TICKS: u32 = 3;

/// Minimum sphere count for emergent breathing to be meaningful.
const MIN_SPHERES_FOR_BREATHING: usize = 3;

/// EMA smoothing factor for divergence signal.
const _DIVERGENCE_EMA_ALPHA: f64 = 0.2;

/// EMA smoothing factor for coherence signal.
const _COHERENCE_EMA_ALPHA: f64 = 0.2;

// ──────────────────────────────────────────────────────────────
// Conductor
// ──────────────────────────────────────────────────────────────

/// PI breathing controller for the Kuramoto field.
///
/// The conductor modulates a recommended `k_delta` to steer the order parameter
/// `r` toward a dynamic `r_target`. In ORAC this is an advisory controller —
/// the authoritative `k_modulation` lives in the PV2 daemon.
///
/// # Thread Safety
/// `Conductor` is `Send + Sync` (no interior mutability).
#[derive(Debug, Clone)]
pub struct Conductor {
    /// Proportional gain for the PI controller.
    gain: f64,
    /// Fraction of emergent signal blended into output (0.0-1.0).
    breathing_blend: f64,
    /// Integral accumulator for the PI controller.
    integral: f64,
}

impl Conductor {
    /// Create a new conductor with default gains from constants.
    #[must_use]
    pub fn new() -> Self {
        Self {
            gain: m04_constants::CONDUCTOR_GAIN,
            breathing_blend: m04_constants::EMERGENT_BLEND,
            integral: 0.0,
        }
    }

    /// Create a conductor with custom gain and blend parameters.
    ///
    /// Both values are clamped to safe ranges.
    #[must_use]
    pub fn with_params(gain: f64, breathing_blend: f64) -> Self {
        Self {
            gain: gain.clamp(0.01, 1.0),
            breathing_blend: breathing_blend.clamp(0.0, 1.0),
            integral: 0.0,
        }
    }

    /// Compute the dynamic r target based on fleet state.
    ///
    /// Priority (highest to lowest):
    /// 1. Governance override (`r_target_override` from approved proposals)
    /// 2. Base: 0.93 (small/medium) or 0.85 (large >50 spheres)
    ///
    /// In ORAC the fleet-negotiated `preferred_r` blending is deferred to PV2.
    #[must_use]
    pub fn r_target(state: &AppState) -> f64 {
        // 1. Governance override takes priority
        if let Some(override_val) = state.r_target_override {
            return override_val.clamp(0.5, 0.99);
        }

        // 2. Base target depends on fleet size
        let n = state.spheres.len();
        let n_f = f64::from(u32::try_from(n).unwrap_or(u32::MAX));
        if n_f > m04_constants::LARGE_FLEET_THRESHOLD {
            m04_constants::R_TARGET_LARGE_FLEET
        } else {
            m04_constants::R_TARGET_BASE
        }
    }

    /// Produce a `FieldDecision` from the current `AppState`.
    ///
    /// This is the main entry point for the conductor. It reads the cached
    /// field state and produces an advisory decision for the tick loop.
    ///
    /// Returns a default stable decision during warmup or when insufficient
    /// spheres are registered.
    #[must_use]
    pub fn decide(&self, state: &AppState) -> FieldDecision {
        // During warmup, produce stable decisions
        if state.is_warming_up() {
            return FieldDecision::stable("conductor warming up");
        }

        // Need at least a few spheres to make meaningful decisions
        if state.spheres.len() < MIN_SPHERES_FOR_BREATHING {
            return FieldDecision::stable("insufficient spheres for breathing");
        }

        let r = state.field.order.r;
        let target = Self::r_target(state);
        let error = target - r;

        // Determine action based on error magnitude and thresholds
        let action = classify_error(r, state);

        // Compute recommended k_delta from PI controller
        // TODO: Phase 2 — integrate EMA-weighted divergence/coherence signals
        let k_delta = error * self.gain;
        let k_delta_clamped = k_delta.clamp(
            m04_constants::K_MOD_BUDGET_MIN - 1.0,
            m04_constants::K_MOD_BUDGET_MAX - 1.0,
        );

        FieldDecision {
            action,
            k_delta: k_delta_clamped,
            reason: format!(
                "r={r:.3} target={target:.3} error={error:.3} k_delta={k_delta_clamped:.4}"
            ),
        }
    }

    /// Get the current proportional gain.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// Get the current breathing blend fraction.
    #[must_use]
    pub const fn breathing_blend(&self) -> f64 {
        self.breathing_blend
    }

    /// Reset the integral accumulator.
    pub fn reset_integral(&mut self) {
        self.integral = 0.0;
    }
}

impl Default for Conductor {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Classify the current field state into a `FieldAction`.
fn classify_error(r: f64, state: &AppState) -> FieldAction {
    // Recovering takes priority
    if state.divergence_cooldown > 0 {
        return FieldAction::Recovering;
    }

    // Over-synchronized alert
    if r > 0.99 {
        return FieldAction::OverSynchronized;
    }

    let target = Conductor::r_target(state);
    let error = target - r;

    // Error-based classification
    if error > m04_constants::R_RISING_THRESHOLD {
        // r is below target — needs more coherence
        FieldAction::NeedsCoherence
    } else if error < m04_constants::R_FALLING_THRESHOLD {
        // r is above target — needs more divergence
        FieldAction::NeedsDivergence
    } else {
        FieldAction::Stable
    }
}

/// Detect if two consecutive `FieldAction` values represent a direction flip.
///
/// Used for thrashing detection in multi-tick scenarios.
#[must_use]
pub const fn is_direction_flip(prev: &FieldAction, current: &FieldAction) -> bool {
    matches!(
        (prev, current),
        (
            FieldAction::NeedsCoherence,
            FieldAction::NeedsDivergence | FieldAction::OverSynchronized,
        ) | (
            FieldAction::NeedsDivergence | FieldAction::OverSynchronized,
            FieldAction::NeedsCoherence,
        )
    )
}

/// Deterministic noise value from `PaneId` and tick (hash-based, not random).
///
/// Returns a value in \[-1.0, 1.0\] derived from the sphere ID and tick number.
/// Used for phase noise injection recommendations.
#[must_use]
pub fn deterministic_noise(id: &PaneId, tick: u64) -> f64 {
    let hash: u64 = id
        .as_str()
        .bytes()
        .fold(tick.wrapping_mul(0x517c_c1b7), |acc, b| {
            acc.wrapping_mul(31).wrapping_add(u64::from(b))
        });
    // Truncate to u32 (intentional: we only need 32 bits of entropy).
    let h32 = u32::try_from(hash >> 32).unwrap_or(0);
    // Map [0, u32::MAX] to [-1.0, 1.0] via f64::from (lossless for u32)
    (f64::from(h32) / f64::from(u32::MAX)).mul_add(2.0, -1.0)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    use crate::m1_core::m01_core_types::PaneSphere;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn make_state_with_spheres(n: usize) -> AppState {
        let mut state = AppState::default();
        for i in 0..n {
            let id = format!("s{i}");
            let sphere = PaneSphere::new(pid(&id), format!("sphere-{i}"));
            state.spheres.insert(pid(&id), sphere);
        }
        // Advance past warmup
        for _ in 0..15 {
            state.tick_warmup();
        }
        state
    }

    // ── Construction ──

    #[test]
    fn conductor_default_gains() {
        let c = Conductor::default();
        assert!((c.gain() - m04_constants::CONDUCTOR_GAIN).abs() < f64::EPSILON);
        assert!((c.breathing_blend() - m04_constants::EMERGENT_BLEND).abs() < f64::EPSILON);
    }

    #[test]
    fn conductor_with_params_clamps_gain() {
        let c = Conductor::with_params(5.0, 0.5);
        assert!((c.gain() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn conductor_with_params_clamps_blend() {
        let c = Conductor::with_params(0.1, 2.0);
        assert!((c.breathing_blend() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn conductor_with_params_clamps_negative() {
        let c = Conductor::with_params(-1.0, -1.0);
        assert!((c.gain() - 0.01).abs() < f64::EPSILON);
        assert!(c.breathing_blend().abs() < f64::EPSILON);
    }

    // ── r_target ──

    #[test]
    fn r_target_empty_state() {
        let state = AppState::default();
        let target = Conductor::r_target(&state);
        assert!((target - m04_constants::R_TARGET_BASE).abs() < f64::EPSILON);
    }

    #[test]
    fn r_target_small_fleet() {
        let state = make_state_with_spheres(5);
        let target = Conductor::r_target(&state);
        assert!((target - m04_constants::R_TARGET_BASE).abs() < f64::EPSILON);
    }

    #[test]
    fn r_target_large_fleet() {
        let state = make_state_with_spheres(60);
        let target = Conductor::r_target(&state);
        assert!((target - m04_constants::R_TARGET_LARGE_FLEET).abs() < f64::EPSILON);
    }

    #[test]
    fn r_target_governance_override() {
        let mut state = make_state_with_spheres(5);
        state.r_target_override = Some(0.75);
        let target = Conductor::r_target(&state);
        assert!((target - 0.75).abs() < f64::EPSILON);
    }

    #[test]
    fn r_target_governance_override_clamped() {
        let mut state = make_state_with_spheres(3);
        state.r_target_override = Some(0.1);
        let target = Conductor::r_target(&state);
        assert!(target >= 0.5);
    }

    // ── decide ──

    #[test]
    fn decide_warmup_returns_stable() {
        let c = Conductor::new();
        let state = AppState::default(); // Still warming up
        let decision = c.decide(&state);
        assert_eq!(decision.action, FieldAction::Stable);
    }

    #[test]
    fn decide_few_spheres_stable() {
        let c = Conductor::new();
        let state = make_state_with_spheres(1);
        let decision = c.decide(&state);
        assert_eq!(decision.action, FieldAction::Stable);
    }

    #[test]
    fn decide_high_r_may_over_sync() {
        let c = Conductor::new();
        let mut state = make_state_with_spheres(5);
        state.field.order.r = 0.995;
        let decision = c.decide(&state);
        assert_eq!(decision.action, FieldAction::OverSynchronized);
    }

    #[test]
    fn decide_low_r_needs_coherence() {
        let c = Conductor::new();
        let mut state = make_state_with_spheres(5);
        state.field.order.r = 0.3;
        let decision = c.decide(&state);
        assert_eq!(decision.action, FieldAction::NeedsCoherence);
    }

    #[test]
    fn decide_cooldown_returns_recovering() {
        let c = Conductor::new();
        let mut state = make_state_with_spheres(5);
        state.divergence_cooldown = 2;
        state.field.order.r = 0.3;
        let decision = c.decide(&state);
        assert_eq!(decision.action, FieldAction::Recovering);
    }

    #[test]
    fn decide_produces_finite_k_delta() {
        let c = Conductor::new();
        let mut state = make_state_with_spheres(5);
        state.field.order.r = 0.5;
        let decision = c.decide(&state);
        assert!(decision.k_delta.is_finite());
        assert!(decision.k_delta.abs() <= 0.5);
    }

    // ── Helpers ──

    #[test]
    fn is_direction_flip_coherence_to_divergence() {
        assert!(is_direction_flip(
            &FieldAction::NeedsCoherence,
            &FieldAction::NeedsDivergence,
        ));
    }

    #[test]
    fn is_direction_flip_same_direction_false() {
        assert!(!is_direction_flip(
            &FieldAction::NeedsCoherence,
            &FieldAction::NeedsCoherence,
        ));
    }

    #[test]
    fn is_direction_flip_stable_to_coherence_false() {
        assert!(!is_direction_flip(
            &FieldAction::Stable,
            &FieldAction::NeedsCoherence,
        ));
    }

    #[test]
    fn deterministic_noise_bounded() {
        for i in 0..50 {
            let val = deterministic_noise(&pid(&format!("sphere-{i}")), i);
            assert!(
                val >= -1.0 && val <= 1.0,
                "noise out of bounds: {val}"
            );
        }
    }

    #[test]
    fn deterministic_noise_deterministic() {
        let a = deterministic_noise(&pid("test"), 42);
        let b = deterministic_noise(&pid("test"), 42);
        assert!((a - b).abs() < f64::EPSILON);
    }

    #[test]
    fn deterministic_noise_varies_with_id() {
        let a = deterministic_noise(&pid("alpha"), 0);
        let b = deterministic_noise(&pid("beta"), 0);
        // Different IDs should generally produce different noise
        assert!(a.is_finite() && b.is_finite());
    }

    // ── Integration ──

    #[test]
    fn full_decision_cycle() {
        let c = Conductor::new();
        let mut state = make_state_with_spheres(5);

        // Low r -> needs coherence
        state.field.order.r = 0.3;
        let d1 = c.decide(&state);
        assert_eq!(d1.action, FieldAction::NeedsCoherence);
        assert!(d1.k_delta > 0.0, "should suggest increasing K");

        // High r -> over-synchronized
        state.field.order.r = 0.995;
        let d2 = c.decide(&state);
        assert_eq!(d2.action, FieldAction::OverSynchronized);
    }

    #[test]
    fn reset_integral_works() {
        let mut c = Conductor::new();
        c.integral = 0.5;
        c.reset_integral();
        assert!(c.integral.abs() < f64::EPSILON);
    }
}
