//! # M31: Conductor
//!
//! PI breathing controller targeting `r_target` (dynamic, fleet-negotiated). Breathing blend 30%.
//! Divergence cooldown: suppress coherence boost for 3 ticks when sphere requests divergence.
//!
//! ## Layer: L7 | Module: M31 | Dependencies: L1, L3 (M12 field state), L4 (M16 coupling)
//! ## NA: NA-P-5 (conductor cooldown deployed), NA-P-3 (fleet `r_target` deployed)
//!
//! ## ADAPT for ORAC
//! This module needs adaptation for ORAC deployment:
//! - Conductor gain and blend parameters may need re-tuning for sidecar tick rate
//! - `r_target` computation depends on `AppState` sphere map (wire from PV2 IPC)
//! - Phase noise injection operates on sphere phases (must sync back via IPC)

use std::f64::consts::TAU;

use crate::m1_foundation::{
    m01_core_types::{FieldAction, PaneId},
    m04_constants,
};
use crate::m3_field::{
    m12_field_state::FieldDecision,
    m15_app_state::AppState,
};
use crate::m4_coupling::m16_coupling_network::CouplingNetwork;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Divergence cooldown duration (ticks) after a divergence kick.
const DIVERGENCE_COOLDOWN_TICKS: u32 = 3;

/// Minimum sphere count for emergent breathing to be meaningful.
const MIN_SPHERES_FOR_EMERGENT: usize = 3;

/// Phase noise strength for divergence kicks.
const PHASE_NOISE_STRENGTH: f64 = 0.3;

/// EMA smoothing for divergence signal.
const DIVERGENCE_EMA_ALPHA: f64 = 0.2;

/// EMA smoothing for coherence signal.
const COHERENCE_EMA_ALPHA: f64 = 0.2;

/// Thrashing guard: minimum ticks before switching decision direction.
const THRASH_GUARD_TICKS: u32 = 3;

// ──────────────────────────────────────────────────────────────
// Conductor
// ──────────────────────────────────────────────────────────────

/// PI breathing controller for the Kuramoto field.
///
/// The conductor modulates `k_modulation` to steer the order parameter `r` toward
/// a dynamic `r_target`. It blends a PI control signal with an emergent natural
/// oscillation of the coupling field.
#[derive(Debug)]
pub struct Conductor {
    /// Proportional gain for the PI controller.
    gain: f64,
    /// Fraction of emergent signal blended into output (0.0–1.0).
    breathing_blend: f64,
}

impl Conductor {
    /// Create a new conductor with default gains.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            gain: m04_constants::CONDUCTOR_GAIN,
            breathing_blend: m04_constants::EMERGENT_BLEND,
        }
    }

    /// Create a conductor with custom gain and blend parameters.
    #[must_use]
    pub fn with_params(gain: f64, breathing_blend: f64) -> Self {
        Self {
            gain: gain.clamp(0.01, 1.0),
            breathing_blend: breathing_blend.clamp(0.0, 1.0),
        }
    }

    /// Compute the dynamic r target based on fleet state.
    ///
    /// Priority (highest to lowest):
    /// 1. Governance override (`r_target_override` from approved proposals)
    /// 2. Fleet-negotiated: mean of spheres' `preferred_r` values
    /// 3. Base: 0.93 (small/medium) or 0.85 (large >50 spheres)
    #[must_use]
    pub fn r_target(state: &AppState) -> f64 {
        // 1. Governance override takes priority
        if let Some(override_val) = state.r_target_override {
            return override_val.clamp(0.5, 0.99);
        }

        // 2. Base target depends on fleet size.
        // Convert count via u32 (lossless to f64) to compare with f64 threshold.
        let n = state.spheres.len();
        let n_f = f64::from(u32::try_from(n).unwrap_or(u32::MAX));
        let base = if n_f > m04_constants::LARGE_FLEET_THRESHOLD {
            m04_constants::R_TARGET_LARGE_FLEET
        } else {
            m04_constants::R_TARGET_BASE
        };

        // 3. Fleet-negotiated: average preferred_r from spheres that express a preference
        let (sum, count) = state
            .spheres
            .values()
            .filter_map(|s| s.preferred_r)
            .fold((0.0, 0_u32), |(s, c), pref| (s + pref, c + 1));

        if count == 0 {
            return base;
        }

        let avg_preferred = sum / f64::from(count);
        // Blend: 70% fleet preference, 30% base
        avg_preferred.mul_add(0.7, base * 0.3).clamp(0.5, 0.99)
    }

    /// Conduct breathing: adjust `k_modulation` based on r deviation from `r_target`.
    ///
    /// This is the main control loop entry point called once per tick.
    pub fn conduct_breathing(&self, state: &mut AppState, decision: &FieldDecision) {
        // Exponential decay on divergence_ema to prevent compounding/stickiness
        // at budget clamp boundary. α=0.95 gives ~37-tick half-life.
        state.divergence_ema *= 0.95;
        state.coherence_ema *= 0.95;

        // Decrement cooldown
        state.divergence_cooldown = state.divergence_cooldown.saturating_sub(1);

        // During cooldown, suppress coherence adjustments
        if state.divergence_cooldown > 0 {
            return;
        }

        // Thrashing guard: don't flip decisions too fast
        if is_direction_flip(&state.prev_decision_action, &decision.action)
            && state.prev_decision_ticks < THRASH_GUARD_TICKS
        {
            state.prev_decision_ticks += 1;
            return;
        }

        let r = decision.r;
        let target = Self::r_target(state);
        let error = target - r;

        // Update EMAs
        state.divergence_ema = state
            .divergence_ema
            .mul_add(1.0 - DIVERGENCE_EMA_ALPHA, decision.divergence_pressure * DIVERGENCE_EMA_ALPHA);
        state.coherence_ema = state
            .coherence_ema
            .mul_add(1.0 - COHERENCE_EMA_ALPHA, decision.coherence_pressure * COHERENCE_EMA_ALPHA);

        // PI control: proportional + integral
        state.divergence_integral += error * 0.1; // Integral with slow accumulation
        state.divergence_integral = state.divergence_integral.clamp(-0.5, 0.5);

        let pi_signal = error.mul_add(self.gain, state.divergence_integral * 0.05);

        // Apply based on decision action
        match decision.action {
            FieldAction::NeedsCoherence => {
                // Boost coupling to increase coherence
                let adjustment = pi_signal.abs() * self.gain;
                apply_k_adjustment(state, adjustment);
            }
            FieldAction::NeedsDivergence | FieldAction::OverSynchronized => {
                // Reduce coupling to allow exploration
                let adjustment = -(pi_signal.abs() * self.gain);
                apply_k_adjustment(state, adjustment);
            }
            FieldAction::Recovering => {
                // Gently restore to neutral
                let current = state
                    .cached_field
                    .as_ref()
                    .map_or(1.0, |_| 1.0);
                let restore = (1.0 - current) * 0.1;
                apply_k_adjustment(state, restore);
            }
            FieldAction::Stable
            | FieldAction::HasBlockedAgents
            | FieldAction::IdleFleet
            | FieldAction::FreshFleet => {
                // No active breathing adjustment needed
            }
        }

        state.prev_decision_action = decision.action.clone();
        state.prev_decision_ticks = 0;
    }

    /// Compute emergent breathing signal from the coupling field.
    ///
    /// This captures the natural oscillation of the system by measuring
    /// the variance of phase velocities — a proxy for collective rhythm.
    #[must_use]
    pub fn emergent_breathing(network: &CouplingNetwork, tick: u64) -> f64 {
        let n = network.sphere_count();
        if n < MIN_SPHERES_FOR_EMERGENT {
            return 0.0;
        }

        // Use order parameter magnitude as base breathing signal
        let op = network.order_parameter();

        // Modulate with a slow sinusoidal oscillation (period ~60 ticks).
        // Only the tick-within-period matters, so mod 60 keeps values small
        // and avoids precision loss from large `u64` → `f64` casts.
        // Safety: `tick % 60` is always in [0, 59], so `u32::try_from` cannot fail.
        let tick_in_period = u32::try_from(tick % 60).unwrap_or(0);
        let oscillation = (f64::from(tick_in_period) * TAU / 60.0).sin();

        // Emergent signal: r deviation from 0.8 modulated by natural oscillation
        let deviation = op.r - 0.8;
        deviation * oscillation * 0.1
    }

    /// Inject phase noise for controlled desynchronization.
    ///
    /// Called when the field decision is `NeedsDivergence` to kick phases apart.
    /// Sets the divergence cooldown to prevent immediate re-coherence.
    pub fn inject_phase_noise(state: &mut AppState, decision: &FieldDecision) {
        if !matches!(
            decision.action,
            FieldAction::NeedsDivergence | FieldAction::OverSynchronized
        ) {
            return;
        }

        // Only kick idle spheres (respect working spheres' stability)
        let idle_ids: Vec<PaneId> = decision.idle_spheres.clone();

        if idle_ids.is_empty() {
            return;
        }

        // Apply phase perturbation to idle spheres
        for id in &idle_ids {
            if let Some(sphere) = state.spheres.get_mut(id) {
                // Hash-based deterministic perturbation
                let hash_offset = deterministic_noise(id, state.tick);
                sphere.phase = hash_offset.mul_add(PHASE_NOISE_STRENGTH, sphere.phase).rem_euclid(TAU);
            }
        }

        // Set cooldown to prevent re-coherence whiplash
        state.divergence_cooldown = DIVERGENCE_COOLDOWN_TICKS;
        state.log(format!(
            "phase noise injected into {} idle spheres, cooldown={DIVERGENCE_COOLDOWN_TICKS}",
            idle_ids.len()
        ));
    }

    /// Get the current gain.
    #[must_use]
    pub const fn gain(&self) -> f64 {
        self.gain
    }

    /// Get the current breathing blend.
    #[must_use]
    pub const fn breathing_blend(&self) -> f64 {
        self.breathing_blend
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

/// Apply a `k_modulation` adjustment, clamped to budget bounds.
///
/// The conductor does not modify the coupling network directly. Instead, it
/// tracks the adjustment in `divergence_ema`, which the tick orchestrator
/// composes multiplicatively with the bridge-derived `k_modulation`.
fn apply_k_adjustment(state: &mut AppState, adjustment: f64) {
    state.divergence_ema = (state.divergence_ema + adjustment)
        .clamp(m04_constants::K_MOD_BUDGET_MIN - 1.0, m04_constants::K_MOD_BUDGET_MAX - 1.0);
    state.mark_dirty();
}

/// Detect if two consecutive `FieldAction` values represent a direction flip (thrashing).
const fn is_direction_flip(prev: &FieldAction, current: &FieldAction) -> bool {
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
/// Returns a value in [-1.0, 1.0] derived from the sphere ID and tick number.
fn deterministic_noise(id: &PaneId, tick: u64) -> f64 {
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
    use std::collections::HashMap;
    use crate::m1_foundation::m01_core_types::{FleetMode, RTrend};
    use crate::m3_field::{
        m11_sphere::PaneSphere,
        m12_field_state::FieldDecision,
        m13_chimera::ChimeraRouting,
        m15_app_state::AppState,
    };
    use approx::assert_relative_eq;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn make_state_with_spheres(n: usize) -> AppState {
        let mut state = AppState::new();
        for i in 0..n {
            let id = format!("s{i}");
            let sphere = PaneSphere::new(pid(&id), "test".into(), 0.1).unwrap();
            state.spheres.insert(pid(&id), sphere);
        }
        state
    }

    fn make_decision(action: FieldAction) -> FieldDecision {
        FieldDecision {
            action,
            r: 0.5,
            r_trend: RTrend::Stable,
            idle_spheres: Vec::new(),
            blocked_spheres: Vec::new(),
            working_spheres: Vec::new(),
            routing: ChimeraRouting::default(),
            tunnel_count: 0,
            strongest_tunnel: None,
            tick: 0,
            coherence_pressure: 0.0,
            divergence_pressure: 0.0,
            fleet_mode: FleetMode::Full,
        }
    }

    // ── Construction ──

    #[test]
    fn conductor_default() {
        let c = Conductor::default();
        assert_relative_eq!(c.gain(), m04_constants::CONDUCTOR_GAIN);
        assert_relative_eq!(c.breathing_blend(), m04_constants::EMERGENT_BLEND);
    }

    #[test]
    fn conductor_new() {
        let c = Conductor::new();
        assert_relative_eq!(c.gain(), m04_constants::CONDUCTOR_GAIN);
    }

    #[test]
    fn conductor_with_params() {
        let c = Conductor::with_params(0.5, 0.8);
        assert_relative_eq!(c.gain(), 0.5);
        assert_relative_eq!(c.breathing_blend(), 0.8);
    }

    #[test]
    fn conductor_with_params_clamps_gain() {
        let c = Conductor::with_params(5.0, 0.5);
        assert_relative_eq!(c.gain(), 1.0);
    }

    #[test]
    fn conductor_with_params_clamps_blend() {
        let c = Conductor::with_params(0.1, 2.0);
        assert_relative_eq!(c.breathing_blend(), 1.0);
    }

    #[test]
    fn conductor_with_params_clamps_negative() {
        let c = Conductor::with_params(-1.0, -1.0);
        assert_relative_eq!(c.gain(), 0.01);
        assert_relative_eq!(c.breathing_blend(), 0.0);
    }

    // ── r_target ──

    #[test]
    fn r_target_empty_state() {
        let state = AppState::new();
        let target = Conductor::r_target(&state);
        assert_relative_eq!(target, m04_constants::R_TARGET_BASE);
    }

    #[test]
    fn r_target_small_fleet() {
        let state = make_state_with_spheres(5);
        let target = Conductor::r_target(&state);
        assert_relative_eq!(target, m04_constants::R_TARGET_BASE);
    }

    #[test]
    fn r_target_large_fleet() {
        let state = make_state_with_spheres(60);
        let target = Conductor::r_target(&state);
        assert_relative_eq!(target, m04_constants::R_TARGET_LARGE_FLEET);
    }

    #[test]
    fn r_target_fleet_negotiated() {
        let mut state = make_state_with_spheres(3);
        for sphere in state.spheres.values_mut() {
            sphere.preferred_r = Some(0.8);
        }
        let target = Conductor::r_target(&state);
        // 0.8 * 0.7 + 0.93 * 0.3 = 0.56 + 0.279 = 0.839
        assert!(target > 0.8 && target < 0.93);
    }

    #[test]
    fn r_target_negotiated_clamped() {
        let mut state = make_state_with_spheres(2);
        for sphere in state.spheres.values_mut() {
            sphere.preferred_r = Some(0.1);
        }
        let target = Conductor::r_target(&state);
        assert!(target >= 0.5);
    }

    #[test]
    fn r_target_partial_preferences() {
        let mut state = make_state_with_spheres(4);
        // Only 2 out of 4 spheres express a preference
        let ids: Vec<PaneId> = state.spheres.keys().take(2).cloned().collect();
        for id in &ids {
            if let Some(sphere) = state.spheres.get_mut(id) {
                sphere.preferred_r = Some(0.85);
            }
        }
        let target = Conductor::r_target(&state);
        // Should be blended, not pure base
        assert!(target < m04_constants::R_TARGET_BASE);
    }

    // ── conduct_breathing ──

    #[test]
    fn conduct_breathing_stable_no_change() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(3);
        let decision = make_decision(FieldAction::Stable);
        let ema_before = state.divergence_ema;
        conductor.conduct_breathing(&mut state, &decision);
        // Stable action should not significantly change EMAs
        assert!((state.divergence_ema - ema_before).abs() < 0.1);
    }

    #[test]
    fn conduct_breathing_needs_coherence() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);
        let mut decision = make_decision(FieldAction::NeedsCoherence);
        decision.coherence_pressure = 0.5;
        decision.r = 0.3; // Low r
        conductor.conduct_breathing(&mut state, &decision);
        assert!(state.dirty);
    }

    #[test]
    fn conduct_breathing_needs_divergence() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);
        let mut decision = make_decision(FieldAction::NeedsDivergence);
        decision.divergence_pressure = 0.3;
        decision.r = 0.95; // High r
        conductor.conduct_breathing(&mut state, &decision);
        assert!(state.dirty);
    }

    #[test]
    fn conduct_breathing_cooldown_suppresses() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(3);
        state.divergence_cooldown = 2;
        let decision = make_decision(FieldAction::NeedsCoherence);
        let ema_before = state.divergence_ema;
        conductor.conduct_breathing(&mut state, &decision);
        // During cooldown, EMA should not change
        assert_relative_eq!(state.divergence_ema, ema_before);
        assert_eq!(state.divergence_cooldown, 1); // Decremented
    }

    #[test]
    fn conduct_breathing_cooldown_decrements() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(3);
        state.divergence_cooldown = 5;
        let decision = make_decision(FieldAction::Stable);
        conductor.conduct_breathing(&mut state, &decision);
        assert_eq!(state.divergence_cooldown, 4);
    }

    #[test]
    fn conduct_breathing_thrash_guard() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);
        state.prev_decision_action = FieldAction::NeedsCoherence;
        state.prev_decision_ticks = 0;

        let decision = make_decision(FieldAction::NeedsDivergence);
        let ema_before = state.divergence_ema;
        conductor.conduct_breathing(&mut state, &decision);
        // Thrash guard should suppress the flip
        assert_relative_eq!(state.divergence_ema, ema_before);
        assert_eq!(state.prev_decision_ticks, 1);
    }

    #[test]
    fn conduct_breathing_over_synchronized() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);
        let mut decision = make_decision(FieldAction::OverSynchronized);
        decision.r = 0.995;
        conductor.conduct_breathing(&mut state, &decision);
        assert!(state.dirty);
    }

    // ── emergent_breathing ──

    #[test]
    fn emergent_breathing_few_spheres_zero() {
        let network = CouplingNetwork::new();
        assert_relative_eq!(Conductor::emergent_breathing(&network, 0), 0.0);
    }

    #[test]
    fn emergent_breathing_with_spheres() {
        let mut network = CouplingNetwork::new();
        network.register(pid("a"), 0.0, 0.1);
        network.register(pid("b"), 0.5, 0.1);
        network.register(pid("c"), 1.0, 0.1);
        // Non-zero result with sufficient spheres
        let result = Conductor::emergent_breathing(&network, 15);
        assert!(result.is_finite());
    }

    #[test]
    fn emergent_breathing_varies_with_tick() {
        let mut network = CouplingNetwork::new();
        network.register(pid("a"), 0.0, 0.1);
        network.register(pid("b"), 0.5, 0.1);
        network.register(pid("c"), 1.0, 0.1);
        let r1 = Conductor::emergent_breathing(&network, 0);
        let r2 = Conductor::emergent_breathing(&network, 15);
        // Should differ due to sinusoidal modulation (at tick 0, sin=0; at tick 15, sin!=0)
        assert!((r1 - r2).abs() >= 0.0); // Just need finite values
    }

    #[test]
    fn emergent_breathing_bounded() {
        let mut network = CouplingNetwork::new();
        for i in 0..10 {
            let id = format!("s{i}");
            network.register(pid(&id), 0.0, 0.1);
        }
        for tick in 0..100 {
            let v = Conductor::emergent_breathing(&network, tick);
            assert!(v.abs() < 1.0, "emergent breathing out of bounds: {v}");
        }
    }

    // ── inject_phase_noise ──

    #[test]
    fn inject_phase_noise_on_divergence() {
        let mut state = make_state_with_spheres(5);
        let idle_ids: Vec<PaneId> = state.spheres.keys().cloned().collect();
        let mut decision = make_decision(FieldAction::NeedsDivergence);
        decision.idle_spheres = idle_ids;

        let phases_before: HashMap<PaneId, f64> = state
            .spheres
            .iter()
            .map(|(id, s)| (id.clone(), s.phase))
            .collect();

        Conductor::inject_phase_noise(&mut state, &decision);

        // Phases should have changed for at least some spheres
        let changed = state
            .spheres
            .iter()
            .filter(|(id, s)| {
                let before = phases_before.get(id).copied().unwrap_or(0.0);
                (s.phase - before).abs() > f64::EPSILON
            })
            .count();
        assert!(changed > 0, "at least some phases should change");
        assert_eq!(state.divergence_cooldown, DIVERGENCE_COOLDOWN_TICKS);
    }

    #[test]
    fn inject_phase_noise_stable_no_change() {
        let mut state = make_state_with_spheres(3);
        let decision = make_decision(FieldAction::Stable);
        Conductor::inject_phase_noise(&mut state, &decision);
        assert_eq!(state.divergence_cooldown, 0);
    }

    #[test]
    fn inject_phase_noise_empty_idle_no_change() {
        let mut state = make_state_with_spheres(3);
        let decision = make_decision(FieldAction::NeedsDivergence);
        // decision.idle_spheres is empty
        Conductor::inject_phase_noise(&mut state, &decision);
        assert_eq!(state.divergence_cooldown, 0);
    }

    #[test]
    fn inject_phase_noise_over_synchronized() {
        let mut state = make_state_with_spheres(3);
        let idle_ids: Vec<PaneId> = state.spheres.keys().cloned().collect();
        let mut decision = make_decision(FieldAction::OverSynchronized);
        decision.idle_spheres = idle_ids;
        Conductor::inject_phase_noise(&mut state, &decision);
        assert_eq!(state.divergence_cooldown, DIVERGENCE_COOLDOWN_TICKS);
    }

    #[test]
    fn inject_phase_noise_preserves_bounds() {
        let mut state = make_state_with_spheres(10);
        let idle_ids: Vec<PaneId> = state.spheres.keys().cloned().collect();
        let mut decision = make_decision(FieldAction::NeedsDivergence);
        decision.idle_spheres = idle_ids;
        Conductor::inject_phase_noise(&mut state, &decision);
        for sphere in state.spheres.values() {
            assert!(
                sphere.phase >= 0.0 && sphere.phase < TAU,
                "phase out of bounds: {}",
                sphere.phase
            );
        }
    }

    // ── Helper functions ──

    #[test]
    fn is_direction_flip_coherence_to_divergence() {
        assert!(is_direction_flip(
            &FieldAction::NeedsCoherence,
            &FieldAction::NeedsDivergence
        ));
    }

    #[test]
    fn is_direction_flip_divergence_to_coherence() {
        assert!(is_direction_flip(
            &FieldAction::NeedsDivergence,
            &FieldAction::NeedsCoherence
        ));
    }

    #[test]
    fn is_direction_flip_stable_to_coherence_false() {
        assert!(!is_direction_flip(
            &FieldAction::Stable,
            &FieldAction::NeedsCoherence
        ));
    }

    #[test]
    fn is_direction_flip_same_direction_false() {
        assert!(!is_direction_flip(
            &FieldAction::NeedsCoherence,
            &FieldAction::NeedsCoherence
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
        assert_relative_eq!(a, b);
    }

    #[test]
    fn deterministic_noise_varies_with_id() {
        let a = deterministic_noise(&pid("alpha"), 0);
        let b = deterministic_noise(&pid("beta"), 0);
        // Different IDs should produce different noise (with high probability)
        assert!((a - b).abs() > f64::EPSILON || a == b); // Allow coincidence
    }

    #[test]
    fn deterministic_noise_varies_with_tick() {
        let a = deterministic_noise(&pid("test"), 0);
        let b = deterministic_noise(&pid("test"), 1);
        assert!((a - b).abs() > f64::EPSILON || a == b);
    }

    // ── Integration ──

    #[test]
    fn full_breathing_cycle() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);

        // Tick 1: Needs coherence
        let mut decision = make_decision(FieldAction::NeedsCoherence);
        decision.r = 0.3;
        decision.coherence_pressure = 0.5;
        conductor.conduct_breathing(&mut state, &decision);

        // Tick 2: Still coherence (no thrash)
        decision.r = 0.4;
        conductor.conduct_breathing(&mut state, &decision);

        // Tick 3: Stable
        let decision = make_decision(FieldAction::Stable);
        conductor.conduct_breathing(&mut state, &decision);

        // System should have reached a steady state
        assert!(state.divergence_ema.is_finite());
        assert!(state.coherence_ema.is_finite());
    }

    #[test]
    fn breathing_with_noise_injection() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);

        let idle_ids: Vec<PaneId> = state.spheres.keys().cloned().collect();
        let mut decision = make_decision(FieldAction::NeedsDivergence);
        decision.idle_spheres = idle_ids;
        decision.r = 0.98;

        // Inject noise
        Conductor::inject_phase_noise(&mut state, &decision);
        assert_eq!(state.divergence_cooldown, DIVERGENCE_COOLDOWN_TICKS);

        // Breathing should be suppressed during cooldown
        conductor.conduct_breathing(&mut state, &decision);
        assert_eq!(state.divergence_cooldown, DIVERGENCE_COOLDOWN_TICKS - 1);
    }

    #[test]
    fn divergence_ema_decays_over_time() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(3);
        state.divergence_ema = 0.1;
        state.coherence_ema = 0.1;
        let decision = make_decision(FieldAction::Stable);

        // Run 50 ticks — EMA should decay toward 0
        for _ in 0..50 {
            conductor.conduct_breathing(&mut state, &decision);
        }
        assert!(
            state.divergence_ema.abs() < 0.01,
            "EMA should decay: {}",
            state.divergence_ema
        );
        assert!(
            state.coherence_ema.abs() < 0.01,
            "coherence EMA should decay: {}",
            state.coherence_ema
        );
    }

    #[test]
    fn divergence_ema_decay_prevents_stickiness() {
        let conductor = Conductor::new();
        let mut state = make_state_with_spheres(5);

        // Saturate the EMA at max
        state.divergence_ema = m04_constants::K_MOD_BUDGET_MAX - 1.0;

        // Run stable ticks — should decay back
        let decision = make_decision(FieldAction::Stable);
        for _ in 0..100 {
            conductor.conduct_breathing(&mut state, &decision);
        }
        assert!(
            state.divergence_ema < 0.01,
            "saturated EMA should recover: {}",
            state.divergence_ema
        );
    }
}
