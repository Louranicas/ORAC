//! # M17: Auto-Scale K
//!
//! Periodically adjusts coupling strength K based on frequency spread and fleet size.
//! Prevents over-synchronization while maintaining coupling dynamics.
//!
//! ## Layer: L4 (Coupling)
//! ## Module: M17
//! ## Dependencies: L1 (M04), M16

use crate::m1_core::m04_constants;
use super::m15_coupling_network::CouplingNetwork;

// ──────────────────────────────────────────────────────────────
// Auto-K controller
// ──────────────────────────────────────────────────────────────

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

impl AutoKController {
    /// Create a new auto-K controller with default period.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ticks_since_recalc: 0,
            period: m04_constants::COUPLING_STEPS_PER_TICK as u64, // 15 ticks default
            previous_k: 1.5,
            smoothing: 0.3,
        }
    }

    /// Create with custom period and smoothing.
    #[must_use]
    pub const fn with_params(period: u64, smoothing: f64) -> Self {
        Self {
            ticks_since_recalc: 0,
            period,
            previous_k: 1.5,
            smoothing,
        }
    }

    /// Tick the controller. Returns true if K was recalculated.
    pub fn tick(&mut self, network: &mut CouplingNetwork) -> bool {
        self.ticks_since_recalc += 1;

        if self.ticks_since_recalc < self.period {
            return false;
        }

        self.ticks_since_recalc = 0;

        if !network.auto_k {
            return false;
        }

        network.auto_scale_k();

        // Apply smoothing
        let raw_k = network.k;
        let smoothed = self.smoothing.mul_add(self.previous_k, (1.0 - self.smoothing) * raw_k);
        network.k = smoothed;
        self.previous_k = smoothed;

        true
    }

    /// Force immediate recalculation.
    pub fn force_recalc(&mut self, network: &mut CouplingNetwork) {
        self.ticks_since_recalc = 0;
        if network.auto_k {
            network.auto_scale_k();
            self.previous_k = network.k;
        }
    }

    /// Reset the controller state.
    pub fn reset(&mut self) {
        self.ticks_since_recalc = 0;
        self.previous_k = 1.5;
    }

    /// Ticks until next recalculation.
    #[must_use]
    pub const fn ticks_remaining(&self) -> u64 {
        self.period.saturating_sub(self.ticks_since_recalc)
    }
}

impl Default for AutoKController {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Consent-gated K adjustment
// ──────────────────────────────────────────────────────────────

/// Apply a K modulation adjustment with consent gating.
///
/// Returns the new `k_mod` value after applying `proposed_adjustment` to
/// `current_k_mod`. The result is clamped to
/// [`K_MOD_BUDGET_MIN`](crate::m1_core::m04_constants::K_MOD_BUDGET_MIN)..=[`K_MOD_BUDGET_MAX`](crate::m1_core::m04_constants::K_MOD_BUDGET_MAX).
#[must_use]
pub fn consent_gated_k_adjustment(
    current_k_mod: f64,
    proposed_adjustment: f64,
) -> f64 {
    let new_k_mod = current_k_mod + proposed_adjustment;
    new_k_mod.clamp(
        m04_constants::K_MOD_BUDGET_MIN,
        m04_constants::K_MOD_BUDGET_MAX,
    )
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_core::m01_core_types::PaneId;
    use approx::assert_relative_eq;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    // ── AutoKController construction ──

    #[test]
    fn new_controller_default() {
        let ctrl = AutoKController::new();
        assert_eq!(ctrl.ticks_since_recalc, 0);
        assert_relative_eq!(ctrl.previous_k, 1.5);
    }

    #[test]
    fn default_matches_new() {
        let a = AutoKController::new();
        let b = AutoKController::default();
        assert_eq!(a.period, b.period);
    }

    #[test]
    fn with_params_custom() {
        let ctrl = AutoKController::with_params(10, 0.5);
        assert_eq!(ctrl.period, 10);
        assert_relative_eq!(ctrl.smoothing, 0.5);
    }

    // ── Tick behavior ──

    #[test]
    fn tick_increments_counter() {
        let mut ctrl = AutoKController::with_params(5, 0.0);
        let mut net = CouplingNetwork::new();
        ctrl.tick(&mut net);
        assert_eq!(ctrl.ticks_since_recalc, 1);
    }

    #[test]
    fn tick_does_not_recalc_before_period() {
        let mut ctrl = AutoKController::with_params(5, 0.0);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.2);
        let recalced = ctrl.tick(&mut net);
        assert!(!recalced);
    }

    #[test]
    fn tick_recalcs_at_period() {
        let mut ctrl = AutoKController::with_params(3, 0.0);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 5.0);

        for _ in 0..2 {
            ctrl.tick(&mut net);
        }
        let recalced = ctrl.tick(&mut net);
        assert!(recalced);
    }

    #[test]
    fn tick_resets_counter_after_recalc() {
        let mut ctrl = AutoKController::with_params(2, 0.0);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.2);

        ctrl.tick(&mut net);
        ctrl.tick(&mut net);
        assert_eq!(ctrl.ticks_since_recalc, 0);
    }

    #[test]
    fn tick_respects_auto_k_flag() {
        let mut ctrl = AutoKController::with_params(1, 0.0);
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        net.register(pid("a"), 0.0, 0.1);
        let recalced = ctrl.tick(&mut net);
        assert!(!recalced);
    }

    // ── Smoothing ──

    #[test]
    fn smoothing_zero_uses_raw_k() {
        let mut ctrl = AutoKController::with_params(1, 0.0);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 5.0);
        ctrl.tick(&mut net);
        // With 0 smoothing, K should be raw auto-scaled value
        assert!(net.k > 0.0);
    }

    #[test]
    fn smoothing_one_preserves_previous() {
        let mut ctrl = AutoKController::with_params(1, 1.0);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 5.0);
        ctrl.tick(&mut net);
        // With full smoothing, K should stay at previous (1.5)
        assert_relative_eq!(net.k, 1.5, epsilon = 1e-10);
    }

    // ── Force recalc ──

    #[test]
    fn force_recalc_resets_counter() {
        let mut ctrl = AutoKController::with_params(10, 0.0);
        let mut net = CouplingNetwork::new();
        ctrl.tick(&mut net);
        ctrl.tick(&mut net);
        ctrl.force_recalc(&mut net);
        assert_eq!(ctrl.ticks_since_recalc, 0);
    }

    #[test]
    fn force_recalc_updates_k() {
        let mut ctrl = AutoKController::with_params(100, 0.0);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 5.0);
        ctrl.force_recalc(&mut net);
        assert!(net.k != 1.5 || net.frequencies.len() < 2);
    }

    // ── Reset ──

    #[test]
    fn reset_clears_state() {
        let mut ctrl = AutoKController::with_params(5, 0.0);
        let mut net = CouplingNetwork::new();
        for _ in 0..3 {
            ctrl.tick(&mut net);
        }
        ctrl.reset();
        assert_eq!(ctrl.ticks_since_recalc, 0);
        assert_relative_eq!(ctrl.previous_k, 1.5);
    }

    // ── Ticks remaining ──

    #[test]
    fn ticks_remaining_full() {
        let ctrl = AutoKController::with_params(10, 0.0);
        assert_eq!(ctrl.ticks_remaining(), 10);
    }

    #[test]
    fn ticks_remaining_after_ticks() {
        let mut ctrl = AutoKController::with_params(10, 0.0);
        let mut net = CouplingNetwork::new();
        ctrl.tick(&mut net);
        ctrl.tick(&mut net);
        assert_eq!(ctrl.ticks_remaining(), 8);
    }

    // ── Consent-gated adjustment ──

    #[test]
    fn consent_gated_normal() {
        let result = consent_gated_k_adjustment(1.0, 0.05);
        assert_relative_eq!(result, 1.05, epsilon = 1e-10);
    }

    #[test]
    fn consent_gated_clamps_high() {
        let result = consent_gated_k_adjustment(1.0, 1.0);
        assert_relative_eq!(result, m04_constants::K_MOD_BUDGET_MAX, epsilon = 1e-10);
    }

    #[test]
    fn consent_gated_clamps_low() {
        let result = consent_gated_k_adjustment(1.0, -0.5);
        assert!(result >= m04_constants::K_MOD_BUDGET_MIN);
    }

    #[test]
    fn consent_gated_at_budget_min() {
        let result = consent_gated_k_adjustment(m04_constants::K_MOD_BUDGET_MIN, -0.1);
        assert_relative_eq!(result, m04_constants::K_MOD_BUDGET_MIN, epsilon = 1e-10);
    }

    #[test]
    fn consent_gated_at_budget_max() {
        let result = consent_gated_k_adjustment(m04_constants::K_MOD_BUDGET_MAX, 0.1);
        assert_relative_eq!(result, m04_constants::K_MOD_BUDGET_MAX, epsilon = 1e-10);
    }

    #[test]
    fn consent_gated_zero_adjustment() {
        let result = consent_gated_k_adjustment(1.0, 0.0);
        assert_relative_eq!(result, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn consent_gated_small_adjustment() {
        // 1.0 + 0.05 = 1.05, within budget
        let result = consent_gated_k_adjustment(1.0, 0.05);
        assert_relative_eq!(result, 1.05, epsilon = 1e-10);
    }

    // ── Integration tests ──

    #[test]
    fn auto_k_controller_full_cycle() {
        let mut ctrl = AutoKController::with_params(3, 0.2);
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 1.0);
        net.register(pid("c"), 2.0, 5.0);

        let mut recalc_count = 0;
        for _ in 0..30 {
            if ctrl.tick(&mut net) {
                recalc_count += 1;
            }
            net.step();
        }

        assert_eq!(recalc_count, 10); // 30/3 = 10 recalculations
    }
}
