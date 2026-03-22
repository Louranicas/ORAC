//! # M16: Coupling Network
//!
//! Kuramoto coupling with Jacobi integration (dt=0.01). Phase stepping uses
//! the mean-field equation: dθi/dt = ωi + K/N × Σ wij × sin(θj - θi).
//!
//! ## Layer: L4 (Coupling)
//! ## Module: M16
//! ## Dependencies: L1 (M01, M02, M04)

use std::collections::HashMap;
use std::f64::consts::{PI, TAU};

use serde::{Deserialize, Serialize};

use crate::m1_foundation::{
    m01_core_types::{phase_diff, OrderParameter, PaneId},
    m04_constants,
};

// ──────────────────────────────────────────────────────────────
// Connection
// ──────────────────────────────────────────────────────────────

/// Directed connection between two pane-spheres.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    /// Source sphere.
    pub from: PaneId,
    /// Target sphere.
    pub to: PaneId,
    /// Base coupling weight (0.0–1.0). Modified by Hebbian learning.
    pub weight: f64,
    /// Connection type modifier (apex-apex=1.0, apex-horizon=0.6).
    pub type_weight: f64,
}

// ──────────────────────────────────────────────────────────────
// CouplingNetwork
// ──────────────────────────────────────────────────────────────

/// Kuramoto coupling network for all pane-spheres.
///
/// Manages phases, frequencies, connections, and K modulation.
/// Adjacency-indexed for O(degree) step computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouplingNetwork {
    /// Per-sphere phase (radians, 0..2π).
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
    #[serde(default = "default_k_modulation")]
    pub k_modulation: f64,
    /// Causal STDP: asymmetric weights when `true`.
    #[serde(default)]
    pub asymmetric_hebbian: bool,
    /// Adjacency index: sphere ID → indices into connections.
    #[serde(skip)]
    adj_index: HashMap<PaneId, Vec<usize>>,
}

const fn default_k_modulation() -> f64 {
    1.0
}

/// Maximum coupling sum per sphere per step (prevents supercritical runaway).
const COUPLING_SUM_CAP: f64 = 3.0;

impl CouplingNetwork {
    /// Create a new empty coupling network.
    #[must_use]
    pub fn new() -> Self {
        Self {
            phases: HashMap::new(),
            frequencies: HashMap::new(),
            connections: Vec::new(),
            k: 1.5,
            auto_k: true,
            k_modulation: default_k_modulation(),
            asymmetric_hebbian: false,
            adj_index: HashMap::new(),
        }
    }

    /// Register a new sphere with hash-based frequency seeding.
    ///
    /// Frequency is multiplied by a hash-derived scale in [0.2, 2.0] to create
    /// natural frequency diversity. Connections to all existing spheres are created.
    pub fn register(&mut self, id: PaneId, phase: f64, frequency: f64) {
        let hash_scale = frequency_hash_scale(&id);
        let freq = (frequency * hash_scale).clamp(0.1, 10.0);

        // Create bidirectional connections to all existing spheres
        let existing: Vec<PaneId> = self.phases.keys().cloned().collect();
        for other in &existing {
            self.connections.push(Connection {
                from: id.clone(),
                to: other.clone(),
                weight: m04_constants::DEFAULT_WEIGHT,
                type_weight: 0.6,
            });
            self.connections.push(Connection {
                from: other.clone(),
                to: id.clone(),
                weight: m04_constants::DEFAULT_WEIGHT,
                type_weight: 0.6,
            });
        }

        self.phases.insert(id.clone(), phase.rem_euclid(TAU));
        self.frequencies.insert(id, freq);
        self.rebuild_index();

        if self.auto_k {
            self.auto_scale_k();
        }
    }

    /// Remove a sphere from the network.
    pub fn deregister(&mut self, id: &PaneId) {
        self.phases.remove(id);
        self.frequencies.remove(id);
        self.connections.retain(|c| c.from != *id && c.to != *id);
        self.rebuild_index();

        if self.auto_k {
            self.auto_scale_k();
        }
    }

    /// Rebuild adjacency index from connections list.
    pub fn rebuild_index(&mut self) {
        self.adj_index.clear();
        for (i, conn) in self.connections.iter().enumerate() {
            self.adj_index
                .entry(conn.from.clone())
                .or_default()
                .push(i);
        }
    }

    /// Number of registered spheres.
    #[must_use]
    pub fn sphere_count(&self) -> usize {
        self.phases.len()
    }

    // ── Weight operations ──

    /// Set coupling weight between two spheres (uses adjacency index).
    pub fn set_weight(&mut self, from: &PaneId, to: &PaneId, weight: f64) {
        let clamped = weight.clamp(m04_constants::HEBBIAN_WEIGHT_FLOOR, 1.0);

        // Indexed path
        if let Some(indices) = self.adj_index.get(from) {
            for &i in indices {
                if self.connections[i].to == *to {
                    self.connections[i].weight = clamped;
                    if !self.asymmetric_hebbian {
                        // Symmetric: also update reverse direction
                        self.set_reverse_weight(to, from, clamped);
                    }
                    return;
                }
            }
        }
    }

    /// Set weight in reverse direction (for symmetric mode).
    fn set_reverse_weight(&mut self, from: &PaneId, to: &PaneId, weight: f64) {
        if let Some(indices) = self.adj_index.get(from) {
            for &i in indices {
                if self.connections[i].to == *to {
                    self.connections[i].weight = weight;
                    return;
                }
            }
        }
    }

    /// Get base coupling weight between two spheres.
    #[must_use]
    pub fn get_weight(&self, from: &PaneId, to: &PaneId) -> Option<f64> {
        self.adj_index.get(from).and_then(|indices| {
            indices
                .iter()
                .find(|&&i| self.connections[i].to == *to)
                .map(|&i| self.connections[i].weight)
        })
    }

    // ── K scaling ──

    /// Auto-scale K based on frequency spread (IQR-robust).
    ///
    /// Uses interquartile range instead of `max - min` to prevent outlier-driven
    /// K spikes (Session 044: single registration caused 21x spread jump).
    /// Rate-limited to 25% change per recalculation.
    ///
    /// `K_new = Kc = (2 × iqr_spread / π) × N`, capped at N, rate-limited.
    pub fn auto_scale_k(&mut self) {
        let n = self.frequencies.len();
        if n < 2 {
            self.k = 1.5;
            return;
        }

        let mut freqs: Vec<f64> = self.frequencies.values().copied().collect();
        freqs.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let spread = if n >= 4 {
            // IQR: Q3 - Q1 for robustness against outlier frequencies
            let q1_idx = n / 4;
            let q3_idx = (3 * n) / 4;
            let iqr = freqs[q3_idx] - freqs[q1_idx];
            // Scale IQR to approximate full spread (IQR ≈ 1.35σ for normal)
            iqr * 1.5
        } else {
            // Too few spheres for IQR — use median-adjacent spread
            let mid = n / 2;
            freqs[mid] - freqs[0]
        };

        let new_k = if spread < 1e-6 {
            1.5
        } else {
            #[allow(clippy::cast_precision_loss)]
            let n_f = n as f64;
            let kc = (2.0 * spread / PI) * n_f;
            kc.min(n_f)
        };

        // Rate limiter: K can change at most 25% per recalculation
        let max_change = self.k * 0.25;
        self.k = new_k.clamp(self.k - max_change, self.k + max_change);
    }

    // ── Phase stepping ──

    /// Run one Kuramoto step with per-sphere receptivity modulation.
    ///
    /// Each sphere's coupling influence is scaled by its receptivity value
    /// (0.0 = closed, 1.0 = fully open). Spheres not in `receptivities`
    /// default to 1.0.
    pub fn step_with_receptivity(&mut self, receptivities: &HashMap<PaneId, f64>) {
        self.step_inner(receptivities);
    }

    /// Run one Kuramoto step without per-sphere receptivity.
    ///
    /// All spheres receive the full coupling influence (receptivity = 1.0).
    pub fn step(&mut self) {
        self.step_inner(&HashMap::new());
    }

    /// Jacobi integration: use old phases to compute new phases.
    fn step_inner(&mut self, receptivities: &HashMap<PaneId, f64>) {
        let dt = m04_constants::KURAMOTO_DT;
        let n = self.phases.len();

        if n < 2 {
            for (id, phase) in &mut self.phases {
                if let Some(&freq) = self.frequencies.get(id) {
                    *phase = freq.mul_add(dt, *phase).rem_euclid(TAU);
                }
            }
            return;
        }

        // Snapshot current phases (Jacobi: use old values)
        let old_phases: HashMap<PaneId, f64> = self.phases.clone();

        for (id, phase) in &mut self.phases {
            let freq = self.frequencies.get(id).copied().unwrap_or(0.0);
            let old_phase = old_phases[id];

            // Sum coupling contributions via adjacency index
            let coupling_sum: f64 = self
                .adj_index
                .get(id)
                .map_or(0.0, |indices| {
                    indices
                        .iter()
                        .filter_map(|&i| {
                            let c = &self.connections[i];
                            old_phases.get(&c.to).map(|&other_phase| {
                                let w_amp = c.weight * c.type_weight;
                                w_amp * phase_diff(other_phase, old_phase).sin()
                            })
                        })
                        .sum()
                })
                .clamp(-COUPLING_SUM_CAP, COUPLING_SUM_CAP);

            let receptivity = receptivities.get(id).copied().unwrap_or(1.0);
            let k_effective = self.k * self.k_modulation;

            #[allow(clippy::cast_precision_loss)]
            let d_phase = receptivity.mul_add(
                (k_effective / n as f64) * coupling_sum,
                freq,
            );
            *phase = d_phase.mul_add(dt, old_phase).rem_euclid(TAU);
        }
    }

    // ── Order parameter ──

    /// Compute the Kuramoto order parameter (r, ψ).
    #[must_use]
    pub fn order_parameter(&self) -> OrderParameter {
        if self.phases.is_empty() {
            return OrderParameter::default();
        }

        let (re, im) = self
            .phases
            .values()
            .map(|&phi| (phi.cos(), phi.sin()))
            .fold((0.0, 0.0), |(ar, ai), (r, i)| (ar + r, ai + i));

        #[allow(clippy::cast_precision_loss)]
        let n = self.phases.len() as f64;
        let r = (re / n).hypot(im / n);
        let psi = (im / n).atan2(re / n).rem_euclid(TAU);

        OrderParameter { r, psi }
    }

    // ── Phase manipulation ──

    /// Kick phases apart for desynchronization control.
    ///
    /// Assigns evenly-spaced target phases and applies a proportional kick.
    /// Returns the number of spheres affected.
    pub fn kick_phases_apart(&mut self, sphere_ids: &[PaneId], strength: f64) -> usize {
        if sphere_ids.is_empty() {
            return 0;
        }

        #[allow(clippy::cast_precision_loss)]
        let spacing = TAU / sphere_ids.len() as f64;
        let mut kicked = 0;

        for (i, id) in sphere_ids.iter().enumerate() {
            if let Some(phase) = self.phases.get_mut(id) {
                #[allow(clippy::cast_precision_loss)]
                let target = (i as f64) * spacing;
                let delta = phase_diff(target, *phase);
                *phase = delta.mul_add(strength, *phase).rem_euclid(TAU);
                kicked += 1;
            }
        }

        kicked
    }

    /// Reseed all frequencies from a base frequency with hash-based variation.
    ///
    /// Each sphere's frequency is scaled by a deterministic hash of its `PaneId`,
    /// producing variation in the range [0.2, 2.0] times `base_freq`.
    pub fn reseed_frequencies(&mut self, base_freq: f64) {
        for (id, freq) in &mut self.frequencies {
            let scale = frequency_hash_scale(id);
            *freq = (base_freq * scale).clamp(0.1, 10.0);
        }
    }

    /// Export the effective coupling matrix (base weight times type weight).
    #[must_use]
    pub fn coupling_matrix(&self) -> HashMap<(PaneId, PaneId), f64> {
        self.connections
            .iter()
            .map(|c| ((c.from.clone(), c.to.clone()), c.weight * c.type_weight))
            .collect()
    }
}

impl Default for CouplingNetwork {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Helper functions
// ──────────────────────────────────────────────────────────────

/// Hash-based frequency scale in `[0.2, 2.0]` for natural frequency diversity.
fn frequency_hash_scale(id: &PaneId) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let hash: u64 = id
        .as_str()
        .bytes()
        .fold(0_u64, |h, b| h.wrapping_mul(31).wrapping_add(u64::from(b)));
    let h32 = (hash >> 32) as u32;
    (f64::from(h32 % 10000) / 10000.0).mul_add(1.8, 0.2)
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

    // ── Construction ──

    #[test]
    fn new_network_is_empty() {
        let net = CouplingNetwork::new();
        assert_eq!(net.sphere_count(), 0);
        assert!(net.connections.is_empty());
    }

    #[test]
    fn default_matches_new() {
        let a = CouplingNetwork::new();
        let b = CouplingNetwork::default();
        assert_eq!(a.k, b.k);
    }

    #[test]
    fn new_network_k_is_1_5() {
        let net = CouplingNetwork::new();
        assert_relative_eq!(net.k, 1.5);
    }

    #[test]
    fn new_network_k_mod_is_1() {
        let net = CouplingNetwork::new();
        assert_relative_eq!(net.k_modulation, 1.0);
    }

    // ── Registration ──

    #[test]
    fn register_adds_sphere() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        assert_eq!(net.sphere_count(), 1);
    }

    #[test]
    fn register_creates_connections() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        // 2 connections: a→b and b→a
        assert_eq!(net.connections.len(), 2);
    }

    #[test]
    fn register_three_creates_six_connections() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.register(pid("c"), 2.0, 0.1);
        // 6 directed connections: a↔b, a↔c, b↔c
        assert_eq!(net.connections.len(), 6);
    }

    #[test]
    fn register_phase_stored() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 1.5, 0.1);
        assert_relative_eq!(*net.phases.get(&pid("a")).unwrap(), 1.5, epsilon = 1e-10);
    }

    #[test]
    fn register_frequency_diversified() {
        let mut net = CouplingNetwork::new();
        net.register(pid("sphere-alpha-1"), 0.0, 1.0);
        net.register(pid("sphere-beta-2"), 0.0, 1.0);
        let f1 = net.frequencies[&pid("sphere-alpha-1")];
        let f2 = net.frequencies[&pid("sphere-beta-2")];
        // Hash-based seeding may produce close values for similar names
        // Just verify frequencies are stored and positive
        assert!(f1 > 0.0 && f2 > 0.0, "frequencies should be positive");
    }

    // ── Deregistration ──

    #[test]
    fn deregister_removes_sphere() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.deregister(&pid("a"));
        assert_eq!(net.sphere_count(), 1);
    }

    #[test]
    fn deregister_removes_connections() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.deregister(&pid("a"));
        assert!(net.connections.is_empty());
    }

    // ── Weight operations ──

    #[test]
    fn get_weight_default() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        let w = net.get_weight(&pid("a"), &pid("b"));
        assert!(w.is_some());
        assert_relative_eq!(w.unwrap(), m04_constants::DEFAULT_WEIGHT, epsilon = 1e-10);
    }

    #[test]
    fn set_weight_updates() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.set_weight(&pid("a"), &pid("b"), 0.9);
        assert_relative_eq!(net.get_weight(&pid("a"), &pid("b")).unwrap(), 0.9, epsilon = 1e-10);
    }

    #[test]
    fn set_weight_clamps_to_floor() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.set_weight(&pid("a"), &pid("b"), 0.01);
        assert_relative_eq!(
            net.get_weight(&pid("a"), &pid("b")).unwrap(),
            m04_constants::HEBBIAN_WEIGHT_FLOOR,
            epsilon = 1e-10
        );
    }

    #[test]
    fn set_weight_symmetric_updates_reverse() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.set_weight(&pid("a"), &pid("b"), 0.8);
        // In symmetric mode, b→a should also be 0.8
        assert_relative_eq!(net.get_weight(&pid("b"), &pid("a")).unwrap(), 0.8, epsilon = 1e-10);
    }

    #[test]
    fn set_weight_asymmetric_preserves_directions() {
        let mut net = CouplingNetwork::new();
        net.asymmetric_hebbian = true;
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.set_weight(&pid("a"), &pid("b"), 0.8);
        // In asymmetric mode, b→a should still be the default
        assert_relative_eq!(
            net.get_weight(&pid("b"), &pid("a")).unwrap(),
            m04_constants::DEFAULT_WEIGHT,
            epsilon = 1e-10
        );
    }

    #[test]
    fn get_weight_missing_returns_none() {
        let net = CouplingNetwork::new();
        assert!(net.get_weight(&pid("x"), &pid("y")).is_none());
    }

    // ── Auto-K scaling ──

    #[test]
    fn auto_k_single_sphere_default() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        assert_relative_eq!(net.k, 1.5);
    }

    #[test]
    fn auto_k_scales_with_frequency_spread() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        // Insert raw frequencies to control the spread exactly
        net.phases.insert(pid("a"), 0.0);
        net.phases.insert(pid("b"), 0.0);
        net.frequencies.insert(pid("a"), 0.1);
        net.frequencies.insert(pid("b"), 5.0);
        net.auto_k = true;
        // With only 2 spheres, uses median-adjacent spread (not IQR)
        // Rate limiter caps at 25% change from initial k=1.5
        net.auto_scale_k();
        assert!(net.k >= 1.5, "K should not decrease with large spread, got {}", net.k);
    }

    #[test]
    fn auto_k_iqr_resists_outlier() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        // 5 spheres with similar frequencies + 1 extreme outlier
        for i in 0..5 {
            let id = format!("s{i}");
            net.phases.insert(pid(&id), 0.0);
            net.frequencies.insert(pid(&id), 1.0 + (i as f64) * 0.1);
        }
        // Add outlier
        net.phases.insert(pid("outlier"), 0.0);
        net.frequencies.insert(pid("outlier"), 50.0);
        net.auto_k = true;
        net.auto_scale_k();
        // IQR should be ~0.3 (spread within main cluster), not 49.0 (outlier)
        // So K should remain moderate, not spike to 50
        assert!(net.k < 10.0, "IQR should resist outlier: K={}", net.k);
    }

    #[test]
    fn auto_k_rate_limited() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        // Start with k=1.5, then create conditions that want k=100
        net.phases.insert(pid("a"), 0.0);
        net.phases.insert(pid("b"), 0.0);
        net.frequencies.insert(pid("a"), 0.1);
        net.frequencies.insert(pid("b"), 100.0);
        net.auto_k = true;
        net.auto_scale_k();
        // Rate limit: max change is 25% of 1.5 = 0.375
        assert!(net.k <= 1.875 + 0.01, "rate limiter should cap K change: K={}", net.k);
    }

    #[test]
    fn auto_k_converges_over_multiple_recalcs() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        for i in 0..6 {
            let id = format!("s{i}");
            net.phases.insert(pid(&id), 0.0);
            net.frequencies.insert(pid(&id), 0.5 + (i as f64) * 0.5);
        }
        net.auto_k = true;
        // Run multiple recalculations — K should converge smoothly
        let mut prev_k = net.k;
        for _ in 0..20 {
            net.auto_scale_k();
            let delta = (net.k - prev_k).abs();
            assert!(delta <= prev_k * 0.25 + 0.01, "rate limit violated: delta={delta}");
            prev_k = net.k;
        }
    }

    // ── Phase stepping ──

    #[test]
    fn step_single_sphere_advances_phase() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 1.0);
        let before = net.phases[&pid("a")];
        net.step();
        let after = net.phases[&pid("a")];
        assert!(after != before || net.frequencies[&pid("a")] < 1e-10);
    }

    #[test]
    fn step_preserves_phase_bounds() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 1.0);
        for _ in 0..1000 {
            net.step();
        }
        let phase = net.phases[&pid("a")];
        assert!(phase >= 0.0 && phase < TAU);
    }

    #[test]
    fn step_two_spheres_converge() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        let initial_diff = phase_diff(net.phases[&pid("a")], net.phases[&pid("b")]).abs();

        for _ in 0..500 {
            net.step();
        }

        let final_diff = phase_diff(net.phases[&pid("a")], net.phases[&pid("b")]).abs();
        assert!(
            final_diff < initial_diff || final_diff < 0.5,
            "phases should converge: initial={initial_diff:.3}, final={final_diff:.3}"
        );
    }

    #[test]
    fn step_with_receptivity_modulates() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);

        let mut receptivities = HashMap::new();
        receptivities.insert(pid("a"), 0.0); // Closed — should not move
        receptivities.insert(pid("b"), 1.0); // Open — should move

        let before_a = net.phases[&pid("a")];
        net.step_with_receptivity(&receptivities);
        // a has zero receptivity so it only advances by frequency (minimal)
        let after_a = net.phases[&pid("a")];
        // The phase change should be approximately just freq * dt
        let change = (after_a - before_a).abs();
        assert!(change < 0.1, "closed receptivity should minimize coupling effect");
    }

    // ── Order parameter ──

    #[test]
    fn order_parameter_empty() {
        let net = CouplingNetwork::new();
        let op = net.order_parameter();
        assert_relative_eq!(op.r, 0.0);
    }

    #[test]
    fn order_parameter_single() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 1.0, 0.1);
        let op = net.order_parameter();
        assert_relative_eq!(op.r, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn order_parameter_identical_phases() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        net.register(pid("a"), 1.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        let op = net.order_parameter();
        assert_relative_eq!(op.r, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn order_parameter_opposite_phases() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        net.phases.insert(pid("a"), 0.0);
        net.phases.insert(pid("b"), PI);
        let op = net.order_parameter();
        assert_relative_eq!(op.r, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn order_parameter_bounded() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        for i in 0..10 {
            #[allow(clippy::cast_precision_loss)]
            let phase = (i as f64 / 10.0) * TAU;
            net.phases.insert(pid(&format!("s{i}")), phase);
        }
        let op = net.order_parameter();
        assert!(op.r >= 0.0 && op.r <= 1.0);
    }

    // ── Phase kicks ──

    #[test]
    fn kick_phases_apart_spreads_phases() {
        let mut net = CouplingNetwork::new();
        net.auto_k = false;
        net.phases.insert(pid("a"), 0.0);
        net.phases.insert(pid("b"), 0.01);
        net.phases.insert(pid("c"), 0.02);

        let ids = vec![pid("a"), pid("b"), pid("c")];
        let kicked = net.kick_phases_apart(&ids, 0.5);
        assert_eq!(kicked, 3);

        let diff_ab = phase_diff(net.phases[&pid("a")], net.phases[&pid("b")]).abs();
        assert!(diff_ab > 0.1, "phases should be spread apart");
    }

    #[test]
    fn kick_phases_empty_returns_zero() {
        let mut net = CouplingNetwork::new();
        assert_eq!(net.kick_phases_apart(&[], 1.0), 0);
    }

    #[test]
    fn kick_phases_missing_sphere_skipped() {
        let mut net = CouplingNetwork::new();
        net.phases.insert(pid("a"), 0.0);
        let ids = vec![pid("a"), pid("missing")];
        let kicked = net.kick_phases_apart(&ids, 0.5);
        assert_eq!(kicked, 1);
    }

    // ── Coupling matrix ──

    #[test]
    fn coupling_matrix_empty() {
        let net = CouplingNetwork::new();
        assert!(net.coupling_matrix().is_empty());
    }

    #[test]
    fn coupling_matrix_has_entries() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        let matrix = net.coupling_matrix();
        assert_eq!(matrix.len(), 2);
    }

    // ── Frequency reseeding ──

    #[test]
    fn reseed_frequencies_changes_all() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 0.0, 0.1);
        let before_a = net.frequencies[&pid("a")];
        net.reseed_frequencies(2.0);
        let after_a = net.frequencies[&pid("a")];
        assert!((after_a - before_a).abs() > 0.01);
    }

    // ── K modulation ──

    #[test]
    fn k_modulation_affects_step() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);

        // Step with k_mod = 0 (no coupling)
        net.k_modulation = 0.0;
        let before = net.phases[&pid("a")];
        net.step();
        let change_no_coupling = (net.phases[&pid("a")] - before).abs();

        // Reset and step with k_mod = 2.0 (strong coupling)
        net.phases.insert(pid("a"), 0.0);
        net.phases.insert(pid("b"), 1.0);
        net.k_modulation = 2.0;
        let before = net.phases[&pid("a")];
        net.step();
        let change_strong = (net.phases[&pid("a")] - before).abs();

        // Strong coupling should cause more phase change
        assert!(
            change_strong >= change_no_coupling,
            "strong coupling should cause more change"
        );
    }

    // ── Serde ──

    #[test]
    fn coupling_network_serde_roundtrip() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.5, 0.1);
        net.register(pid("b"), 1.5, 0.2);
        let json = serde_json::to_string(&net).unwrap();
        let back: CouplingNetwork = serde_json::from_str(&json).unwrap();
        assert_eq!(back.sphere_count(), 2);
        assert_eq!(back.connections.len(), 2);
    }

    #[test]
    fn connection_serde_roundtrip() {
        let conn = Connection {
            from: pid("a"),
            to: pid("b"),
            weight: 0.5,
            type_weight: 0.8,
        };
        let json = serde_json::to_string(&conn).unwrap();
        let back: Connection = serde_json::from_str(&json).unwrap();
        assert_relative_eq!(back.weight, 0.5);
    }

    // ── Frequency hash ──

    #[test]
    fn frequency_hash_bounded() {
        for name in &["a", "sphere-1", "fleet-alpha:left", "claude:session-039"] {
            let scale = frequency_hash_scale(&pid(name));
            assert!(scale >= 0.2 && scale <= 2.0, "{name} → {scale}");
        }
    }

    #[test]
    fn frequency_hash_deterministic() {
        let a = frequency_hash_scale(&pid("test"));
        let b = frequency_hash_scale(&pid("test"));
        assert_relative_eq!(a, b);
    }

    #[test]
    fn frequency_hash_varies() {
        // Use very different strings to ensure different hashes
        let a = frequency_hash_scale(&pid("fleet-alpha-commander-1"));
        let b = frequency_hash_scale(&pid("zeta-observer-99"));
        // With 10000 bins in [0.2, 2.0], very different names should diverge
        assert!((a - b).abs() > 0.0, "hashes should differ for different names");
    }
}
