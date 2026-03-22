//! # M19: Hebbian STDP Learning
//!
//! Spike-timing dependent plasticity adapted for Kuramoto oscillators.
//! Co-active spheres strengthen coupling weights; inactive pairs decay.
//!
//! ## Layer: L5 (Learning)
//! ## Module: M19
//! ## Dependencies: L1 (M01, M04), L3 (M11), L4 (M16)

use std::collections::HashMap;

use crate::m1_foundation::{
    m01_core_types::{PaneId, PaneStatus},
    m04_constants,
};
use crate::m3_field::m11_sphere::PaneSphere;
use crate::m4_coupling::m16_coupling_network::CouplingNetwork;

// ──────────────────────────────────────────────────────────────
// STDP update
// ──────────────────────────────────────────────────────────────

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

/// Apply one cycle of Hebbian STDP to the coupling network.
///
/// - Co-active spheres (both `Working`): LTP (`+HEBBIAN_LTP`)
/// - Burst detection (>3 recent memories): LTP *= `HEBBIAN_BURST_MULTIPLIER`
/// - Newcomer spheres (<`NEWCOMER_STEPS` steps): LTP *= `HEBBIAN_NEWCOMER_MULTIPLIER`
/// - Non-co-active pairs: LTD (`-HEBBIAN_LTD`)
/// - Weight floor enforced at `HEBBIAN_WEIGHT_FLOOR`
///
/// Respects `opt_out_hebbian` on both spheres.
#[allow(clippy::implicit_hasher)]
pub fn apply_stdp(
    network: &mut CouplingNetwork,
    spheres: &HashMap<PaneId, PaneSphere>,
) -> StdpResult {
    let mut result = StdpResult::default();

    // Identify working spheres (co-active candidates)
    let working: HashMap<&PaneId, &PaneSphere> = spheres
        .iter()
        .filter(|(_, s)| s.status == PaneStatus::Working)
        .collect();

    // Collect connection updates to avoid borrow conflicts
    let updates: Vec<(PaneId, PaneId, f64)> = network
        .connections
        .iter()
        .filter_map(|conn| {
            let from_sphere = spheres.get(&conn.from)?;
            let to_sphere = spheres.get(&conn.to)?;

            // Respect opt-out flags
            if from_sphere.opt_out_hebbian || to_sphere.opt_out_hebbian {
                return None;
            }

            let both_working =
                working.contains_key(&conn.from) && working.contains_key(&conn.to);

            let weight_delta = if both_working {
                // LTP: co-active pair
                let mut ltp = m04_constants::HEBBIAN_LTP;

                // Burst detection: boost if sphere has high short-term activity
                if from_sphere.activity_30s > 3 || to_sphere.activity_30s > 3 {
                    ltp *= m04_constants::HEBBIAN_BURST_MULTIPLIER;
                }

                // Newcomer boost
                if from_sphere.total_steps < m04_constants::NEWCOMER_STEPS
                    || to_sphere.total_steps < m04_constants::NEWCOMER_STEPS
                {
                    ltp *= m04_constants::HEBBIAN_NEWCOMER_MULTIPLIER;
                }

                ltp
            } else {
                // LTD: non-co-active
                -m04_constants::HEBBIAN_LTD
            };

            let new_weight = (conn.weight + weight_delta)
                .clamp(m04_constants::HEBBIAN_WEIGHT_FLOOR, 1.0);

            Some((conn.from.clone(), conn.to.clone(), new_weight))
        })
        .collect();

    // Apply updates
    for (from, to, new_weight) in &updates {
        let old_weight = network.get_weight(from, to).unwrap_or(0.0);
        network.set_weight(from, to, *new_weight);

        let delta = (new_weight - old_weight).abs();
        result.total_weight_change += delta;

        if *new_weight > old_weight {
            result.ltp_count += 1;
        } else if *new_weight < old_weight {
            result.ltd_count += 1;
        }

        if (*new_weight - m04_constants::HEBBIAN_WEIGHT_FLOOR).abs() < 1e-10 {
            result.at_floor_count += 1;
        }
    }

    result
}

/// Decay all connection weights by a factor (time-based LTD).
///
/// `decay_factor` should be in `[0.0, 1.0]`: `new_weight = old_weight * decay_factor`.
/// Weight floor (`HEBBIAN_WEIGHT_FLOOR`) is enforced.
pub fn decay_all_weights(network: &mut CouplingNetwork, decay_factor: f64) {
    let factor = decay_factor.clamp(0.0, 1.0);
    let updates: Vec<(PaneId, PaneId, f64)> = network
        .connections
        .iter()
        .map(|c| {
            let new_w = (c.weight * factor).max(m04_constants::HEBBIAN_WEIGHT_FLOOR);
            (c.from.clone(), c.to.clone(), new_w)
        })
        .collect();

    for (from, to, w) in &updates {
        network.set_weight(from, to, *w);
    }
}

/// Compute the Hebbian LTP rate for a specific sphere pair.
///
/// Takes into account burst detection and newcomer status.
#[must_use]
pub fn compute_ltp_rate(sphere_a: &PaneSphere, sphere_b: &PaneSphere) -> f64 {
    let mut ltp = m04_constants::HEBBIAN_LTP;

    // Burst detection
    if sphere_a.activity_30s > 3 || sphere_b.activity_30s > 3 {
        ltp *= m04_constants::HEBBIAN_BURST_MULTIPLIER;
    }

    // Newcomer boost
    if sphere_a.total_steps < m04_constants::NEWCOMER_STEPS
        || sphere_b.total_steps < m04_constants::NEWCOMER_STEPS
    {
        ltp *= m04_constants::HEBBIAN_NEWCOMER_MULTIPLIER;
    }

    ltp
}

/// Check if two spheres are co-active (both `Working`).
#[must_use]
pub fn are_coactive(sphere_a: &PaneSphere, sphere_b: &PaneSphere) -> bool {
    sphere_a.status == PaneStatus::Working && sphere_b.status == PaneStatus::Working
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn working_sphere(id: &str) -> PaneSphere {
        let mut s = PaneSphere::new(pid(id), "test".into(), 0.1).unwrap();
        s.status = PaneStatus::Working;
        s.has_worked = true;
        s
    }

    fn idle_sphere(id: &str) -> PaneSphere {
        PaneSphere::new(pid(id), "test".into(), 0.1).unwrap()
    }

    fn setup_two_working() -> (CouplingNetwork, HashMap<PaneId, PaneSphere>) {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), working_sphere("a"));
        spheres.insert(pid("b"), working_sphere("b"));
        (net, spheres)
    }

    fn setup_mixed() -> (CouplingNetwork, HashMap<PaneId, PaneSphere>) {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), working_sphere("a"));
        spheres.insert(pid("b"), idle_sphere("b"));
        (net, spheres)
    }

    // ── apply_stdp: co-active LTP ──

    #[test]
    fn stdp_coactive_increases_weight() {
        let (mut net, spheres) = setup_two_working();
        let before = net.get_weight(&pid("a"), &pid("b")).unwrap();
        let result = apply_stdp(&mut net, &spheres);
        let after = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(after > before, "LTP should increase weight");
        assert!(result.ltp_count > 0);
    }

    #[test]
    fn stdp_coactive_ltp_count() {
        let (mut net, spheres) = setup_two_working();
        let result = apply_stdp(&mut net, &spheres);
        // 2 directed connections, both co-active
        // Symmetric mode: first set_weight(a,b) also sets b→a, so second
        // iteration sees same weight → delta ~0 → counted differently.
        // At minimum 1 LTP update.
        assert!(result.ltp_count >= 1, "should have at least 1 LTP update");
    }

    #[test]
    fn stdp_coactive_ltd_count_zero() {
        let (mut net, spheres) = setup_two_working();
        let result = apply_stdp(&mut net, &spheres);
        assert_eq!(result.ltd_count, 0);
    }

    // ── apply_stdp: non-co-active LTD ──

    #[test]
    fn stdp_non_coactive_decreases_weight() {
        let (mut net, spheres) = setup_mixed();
        let before = net.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net, &spheres);
        let after = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(after <= before, "LTD should decrease or maintain weight");
    }

    #[test]
    fn stdp_non_coactive_ltd_count() {
        let (mut net, spheres) = setup_mixed();
        let result = apply_stdp(&mut net, &spheres);
        assert!(result.ltd_count > 0);
    }

    // ── Weight floor ──

    #[test]
    fn stdp_respects_weight_floor() {
        let (mut net, spheres) = setup_mixed();
        // Apply LTD many times
        for _ in 0..1000 {
            apply_stdp(&mut net, &spheres);
        }
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(
            w >= m04_constants::HEBBIAN_WEIGHT_FLOOR - 1e-10,
            "weight should not go below floor"
        );
    }

    #[test]
    fn stdp_weight_capped_at_one() {
        let (mut net, spheres) = setup_two_working();
        for _ in 0..1000 {
            apply_stdp(&mut net, &spheres);
        }
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(w <= 1.0 + 1e-10, "weight should not exceed 1.0");
    }

    // ── Burst detection ──

    #[test]
    fn stdp_burst_detection_boosts_ltp() {
        let (mut net, mut spheres) = setup_two_working();
        // Set high activity for burst detection
        spheres.get_mut(&pid("a")).unwrap().activity_30s = 5;
        let before = net.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net, &spheres);
        let after = net.get_weight(&pid("a"), &pid("b")).unwrap();
        let delta_burst = after - before;

        // Compare with non-burst
        let (mut net2, spheres2) = setup_two_working();
        let before2 = net2.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net2, &spheres2);
        let after2 = net2.get_weight(&pid("a"), &pid("b")).unwrap();
        let delta_normal = after2 - before2;

        assert!(delta_burst > delta_normal, "burst should boost LTP");
    }

    // ── Newcomer boost ──

    #[test]
    fn stdp_newcomer_boosts_ltp() {
        let (mut net, spheres) = setup_two_working();
        // Sphere a is a newcomer (0 steps)
        assert_eq!(spheres[&pid("a")].total_steps, 0);
        let before = net.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net, &spheres);
        let after = net.get_weight(&pid("a"), &pid("b")).unwrap();
        let delta_newcomer = after - before;

        // Compare with established sphere
        let (mut net2, mut spheres2) = setup_two_working();
        spheres2.get_mut(&pid("a")).unwrap().total_steps = 100;
        spheres2.get_mut(&pid("b")).unwrap().total_steps = 100;
        let before2 = net2.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net2, &spheres2);
        let after2 = net2.get_weight(&pid("a"), &pid("b")).unwrap();
        let delta_established = after2 - before2;

        assert!(delta_newcomer > delta_established, "newcomer should have boosted LTP");
    }

    // ── Opt-out ──

    #[test]
    fn stdp_respects_opt_out() {
        let (mut net, mut spheres) = setup_two_working();
        spheres.get_mut(&pid("a")).unwrap().opt_out_hebbian = true;
        let before = net.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net, &spheres);
        let after = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert_relative_eq!(before, after, epsilon = 1e-10);
    }

    #[test]
    fn stdp_opt_out_target() {
        let (mut net, mut spheres) = setup_two_working();
        spheres.get_mut(&pid("b")).unwrap().opt_out_hebbian = true;
        let before = net.get_weight(&pid("a"), &pid("b")).unwrap();
        apply_stdp(&mut net, &spheres);
        let after = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert_relative_eq!(before, after, epsilon = 1e-10);
    }

    // ── Empty cases ──

    #[test]
    fn stdp_empty_network() {
        let mut net = CouplingNetwork::new();
        let spheres = HashMap::new();
        let result = apply_stdp(&mut net, &spheres);
        assert_eq!(result.ltp_count, 0);
        assert_eq!(result.ltd_count, 0);
    }

    #[test]
    fn stdp_single_sphere() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), working_sphere("a"));
        let result = apply_stdp(&mut net, &spheres);
        assert_eq!(result.ltp_count, 0);
    }

    // ── StdpResult ──

    #[test]
    fn stdp_result_default() {
        let r = StdpResult::default();
        assert_eq!(r.ltp_count, 0);
        assert_eq!(r.ltd_count, 0);
        assert_relative_eq!(r.total_weight_change, 0.0);
    }

    #[test]
    fn stdp_result_total_change_positive() {
        let (mut net, spheres) = setup_two_working();
        let result = apply_stdp(&mut net, &spheres);
        assert!(result.total_weight_change > 0.0);
    }

    // ── decay_all_weights ──

    #[test]
    fn decay_all_reduces_weights() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.set_weight(&pid("a"), &pid("b"), 0.8);
        decay_all_weights(&mut net, 0.95);
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(w < 0.8);
    }

    #[test]
    fn decay_all_respects_floor() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        decay_all_weights(&mut net, 0.0);
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(w >= m04_constants::HEBBIAN_WEIGHT_FLOOR - 1e-10);
    }

    #[test]
    fn decay_all_factor_one_no_change() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.set_weight(&pid("a"), &pid("b"), 0.7);
        decay_all_weights(&mut net, 1.0);
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert_relative_eq!(w, 0.7, epsilon = 1e-10);
    }

    // ── compute_ltp_rate ──

    #[test]
    fn ltp_rate_includes_newcomer_by_default() {
        // New spheres (0 steps) are newcomers → LTP boosted
        let a = idle_sphere("a");
        let b = idle_sphere("b");
        let rate = compute_ltp_rate(&a, &b);
        assert!(rate > m04_constants::HEBBIAN_LTP);
    }

    #[test]
    fn ltp_rate_newcomer_boost() {
        let a = idle_sphere("a"); // 0 steps = newcomer
        let mut b = idle_sphere("b");
        b.total_steps = 100;
        let rate = compute_ltp_rate(&a, &b);
        assert!(rate > m04_constants::HEBBIAN_LTP);
    }

    #[test]
    fn ltp_rate_burst_boost() {
        let mut a = idle_sphere("a");
        a.total_steps = 100;
        a.activity_30s = 5;
        let mut b = idle_sphere("b");
        b.total_steps = 100;
        let rate = compute_ltp_rate(&a, &b);
        assert_relative_eq!(
            rate,
            m04_constants::HEBBIAN_LTP * m04_constants::HEBBIAN_BURST_MULTIPLIER,
            epsilon = 1e-10
        );
    }

    #[test]
    fn ltp_rate_both_boosts() {
        let mut a = idle_sphere("a"); // newcomer
        a.activity_30s = 5;
        let b = idle_sphere("b"); // also newcomer
        let rate = compute_ltp_rate(&a, &b);
        let expected = m04_constants::HEBBIAN_LTP
            * m04_constants::HEBBIAN_BURST_MULTIPLIER
            * m04_constants::HEBBIAN_NEWCOMER_MULTIPLIER;
        assert_relative_eq!(rate, expected, epsilon = 1e-10);
    }

    #[test]
    fn ltp_rate_established_no_burst() {
        let mut a = idle_sphere("a");
        a.total_steps = 200;
        let mut b = idle_sphere("b");
        b.total_steps = 200;
        let rate = compute_ltp_rate(&a, &b);
        assert_relative_eq!(rate, m04_constants::HEBBIAN_LTP, epsilon = 1e-10);
    }

    // ── are_coactive ──

    #[test]
    fn coactive_both_working() {
        let a = working_sphere("a");
        let b = working_sphere("b");
        assert!(are_coactive(&a, &b));
    }

    #[test]
    fn not_coactive_one_idle() {
        let a = working_sphere("a");
        let b = idle_sphere("b");
        assert!(!are_coactive(&a, &b));
    }

    #[test]
    fn not_coactive_both_idle() {
        let a = idle_sphere("a");
        let b = idle_sphere("b");
        assert!(!are_coactive(&a, &b));
    }

    // ── Three-sphere network ──

    #[test]
    fn stdp_three_spheres_mixed_states() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.register(pid("c"), 2.0, 0.1);

        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), working_sphere("a"));
        spheres.insert(pid("b"), working_sphere("b"));
        spheres.insert(pid("c"), idle_sphere("c"));

        let result = apply_stdp(&mut net, &spheres);
        // Symmetric mode: set_weight(a,b) also sets (b,a), so the second
        // iteration for b→a sees the already-updated weight. The total
        // update count depends on set_weight's symmetric behavior.
        assert!(result.ltp_count > 0, "should have some LTP");
        assert!(result.ltd_count > 0 || result.ltp_count > 0, "should have some updates");
        assert!(result.total_weight_change > 0.0, "should have changed weights");
    }

    // ── Multiple cycles ──

    #[test]
    fn stdp_multiple_cycles_converge() {
        let (mut net, spheres) = setup_two_working();
        for _ in 0..100 {
            apply_stdp(&mut net, &spheres);
        }
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert!(w > 0.5, "co-active pair should converge to high weight");
    }

    #[test]
    fn stdp_multiple_cycles_ltd_converge_to_floor() {
        let (mut net, spheres) = setup_mixed();
        for _ in 0..10000 {
            apply_stdp(&mut net, &spheres);
        }
        let w = net.get_weight(&pid("a"), &pid("b")).unwrap();
        assert_relative_eq!(w, m04_constants::HEBBIAN_WEIGHT_FLOOR, epsilon = 0.01);
    }
}
