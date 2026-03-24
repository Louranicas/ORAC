//! # M34: Kuramoto Field Dashboard
//!
//! Dashboard data model for Kuramoto field state visualization.
//! Computes per-cluster order parameters, phase gaps, and effective K.
//!
//! ## Layer: L7 (Monitoring)
//! ## Module: M34
//! ## Dependencies: `m01_core_types`, `m04_constants`
//! ## Feature: `monitoring`
//!
//! ## Dashboard Panels
//!
//! | Panel | Metric | Update |
//! |-------|--------|--------|
//! | Field `r` | Global order parameter | Per-tick |
//! | Phase map | Per-sphere phase (radians) | Per-tick |
//! | Cluster view | Per-cluster `r` + member count | Per-tick |
//! | K effective | Coupling strength | Per-tick |
//! | Chimera | Gap locations + sizes | On detection |
//! | History | `r` over last 60 ticks | Rolling |

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::{OrderParameter, PaneId};
use crate::m1_core::m04_constants;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Maximum `r` history samples.
const R_HISTORY_MAX: usize = m04_constants::R_HISTORY_MAX;

/// Maximum number of clusters tracked.
const MAX_CLUSTERS: usize = 32;

/// Maximum spheres in the phase map.
const MAX_SPHERES: usize = m04_constants::SPHERE_CAP;

/// Phase gap threshold for chimera detection.
const PHASE_GAP_THRESHOLD: f64 = m04_constants::PHASE_GAP_THRESHOLD;

// ──────────────────────────────────────────────────────────────
// Sphere phase entry
// ──────────────────────────────────────────────────────────────

/// Phase state for a single sphere in the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpherePhaseEntry {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// Current phase (radians, 0..2π).
    pub phase: f64,
    /// Natural frequency (Hz).
    pub frequency: f64,
    /// Status label (e.g. "working", "idle", "blocked").
    pub status: String,
    /// Cluster index (if assigned).
    pub cluster: Option<usize>,
}

// ──────────────────────────────────────────────────────────────
// Cluster summary
// ──────────────────────────────────────────────────────────────

/// Summary of a phase-coherent cluster.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterSummary {
    /// Cluster index.
    pub index: usize,
    /// Number of members.
    pub member_count: usize,
    /// Intra-cluster order parameter.
    pub r: f64,
    /// Mean phase of the cluster (radians).
    pub mean_phase: f64,
    /// Pane IDs in this cluster.
    pub members: Vec<PaneId>,
}

// ──────────────────────────────────────────────────────────────
// Phase gap
// ──────────────────────────────────────────────────────────────

/// A detected phase gap (potential chimera boundary).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseGap {
    /// Phase angle where the gap starts (radians).
    pub start_phase: f64,
    /// Phase angle where the gap ends (radians).
    pub end_phase: f64,
    /// Gap size (radians).
    pub size: f64,
    /// Whether this gap exceeds the chimera threshold.
    pub is_chimera: bool,
}

// ──────────────────────────────────────────────────────────────
// Dashboard snapshot
// ──────────────────────────────────────────────────────────────

/// Complete dashboard snapshot at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardSnapshot {
    /// Current tick number.
    pub tick: u64,
    /// Global order parameter.
    pub order: OrderParameter,
    /// Effective coupling strength.
    pub k_effective: f64,
    /// Number of active spheres.
    pub sphere_count: usize,
    /// Per-sphere phase entries.
    pub phases: Vec<SpherePhaseEntry>,
    /// Cluster summaries.
    pub clusters: Vec<ClusterSummary>,
    /// Detected phase gaps.
    pub gaps: Vec<PhaseGap>,
    /// Whether a chimera state is detected.
    pub chimera_detected: bool,
    /// `r` history (most recent last).
    pub r_history: Vec<f64>,
}

// ──────────────────────────────────────────────────────────────
// FieldDashboard
// ──────────────────────────────────────────────────────────────

/// Dashboard state with interior mutability for live updates.
///
/// Fed by tick events from the coordination layer. Produces
/// snapshots for the floating Zellij pane or `/dashboard` endpoint.
#[derive(Debug)]
pub struct FieldDashboard {
    /// Interior-mutable state.
    state: RwLock<DashboardState>,
}

#[derive(Debug)]
struct DashboardState {
    /// Current tick.
    tick: u64,
    /// Global order parameter.
    order: OrderParameter,
    /// Effective coupling strength.
    k_effective: f64,
    /// Per-sphere phase entries.
    phases: Vec<SpherePhaseEntry>,
    /// Computed clusters.
    clusters: Vec<ClusterSummary>,
    /// Detected gaps.
    gaps: Vec<PhaseGap>,
    /// `r` history ring buffer.
    r_history: Vec<f64>,
    /// Whether chimera is currently detected.
    chimera_detected: bool,
}

impl FieldDashboard {
    /// Create a new empty dashboard.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(DashboardState {
                tick: 0,
                order: OrderParameter::new(0.0, 0.0),
                k_effective: 0.0,
                phases: Vec::new(),
                clusters: Vec::new(),
                gaps: Vec::new(),
                r_history: Vec::with_capacity(R_HISTORY_MAX),
                chimera_detected: false,
            }),
        }
    }

    /// Update the dashboard with a new tick.
    pub fn update_tick(&self, tick: u64, order: &OrderParameter, k_effective: f64) {
        let mut state = self.state.write();
        state.tick = tick;
        state.order = *order;
        state.k_effective = k_effective;

        // Append to r_history (ring buffer)
        if state.r_history.len() >= R_HISTORY_MAX {
            state.r_history.remove(0);
        }
        state.r_history.push(order.r);
    }

    /// Set the per-sphere phase entries.
    pub fn set_phases(&self, phases: Vec<SpherePhaseEntry>) {
        let truncated = if phases.len() > MAX_SPHERES {
            phases[..MAX_SPHERES].to_vec()
        } else {
            phases
        };
        self.state.write().phases = truncated;
    }

    /// Set the cluster summaries.
    pub fn set_clusters(&self, clusters: Vec<ClusterSummary>) {
        let truncated = if clusters.len() > MAX_CLUSTERS {
            clusters[..MAX_CLUSTERS].to_vec()
        } else {
            clusters
        };
        self.state.write().clusters = truncated;
    }

    /// Set the detected phase gaps.
    pub fn set_gaps(&self, gaps: Vec<PhaseGap>) {
        let mut state = self.state.write();
        state.chimera_detected = gaps.iter().any(|g| g.is_chimera);
        state.gaps = gaps;
    }

    /// Get a complete dashboard snapshot.
    #[must_use]
    pub fn snapshot(&self) -> DashboardSnapshot {
        let state = self.state.read();
        DashboardSnapshot {
            tick: state.tick,
            order: state.order,
            k_effective: state.k_effective,
            sphere_count: state.phases.len(),
            phases: state.phases.clone(),
            clusters: state.clusters.clone(),
            gaps: state.gaps.clone(),
            chimera_detected: state.chimera_detected,
            r_history: state.r_history.clone(),
        }
    }

    /// Current tick.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.state.read().tick
    }

    /// Current order parameter.
    #[must_use]
    pub fn order(&self) -> OrderParameter {
        self.state.read().order
    }

    /// Current `r` value.
    #[must_use]
    pub fn r(&self) -> f64 {
        self.state.read().order.r
    }

    /// Current effective K.
    #[must_use]
    pub fn k_effective(&self) -> f64 {
        self.state.read().k_effective
    }

    /// Number of tracked spheres.
    #[must_use]
    pub fn sphere_count(&self) -> usize {
        self.state.read().phases.len()
    }

    /// Number of clusters.
    #[must_use]
    pub fn cluster_count(&self) -> usize {
        self.state.read().clusters.len()
    }

    /// Whether chimera is currently detected.
    #[must_use]
    pub fn chimera_detected(&self) -> bool {
        self.state.read().chimera_detected
    }

    /// Length of `r` history.
    #[must_use]
    pub fn r_history_len(&self) -> usize {
        self.state.read().r_history.len()
    }

    /// Mean `r` over the history window.
    #[must_use]
    pub fn r_mean(&self) -> f64 {
        let state = self.state.read();
        if state.r_history.is_empty() {
            return 0.0;
        }
        let sum: f64 = state.r_history.iter().sum();
        sum / bounded_f64(state.r_history.len())
    }

    /// Standard deviation of `r` over the history window.
    #[must_use]
    pub fn r_stddev(&self) -> f64 {
        let state = self.state.read();
        if state.r_history.is_empty() {
            return 0.0;
        }
        let n = state.r_history.len();
        if n < 2 {
            return 0.0;
        }
        let n_f = bounded_f64(n);
        let mean = state.r_history.iter().sum::<f64>() / n_f;
        let variance = state
            .r_history
            .iter()
            .map(|&v| (v - mean).powi(2))
            .sum::<f64>()
            / (n_f - 1.0);
        variance.sqrt()
    }

    /// `r` trend: positive = rising, negative = falling.
    #[must_use]
    pub fn r_trend(&self) -> f64 {
        let state = self.state.read();
        let n = state.r_history.len();
        if n < 2 {
            return 0.0;
        }
        // Simple linear regression slope
        let count = bounded_f64(n);
        let mut sx = 0.0;
        let mut sy = 0.0;
        let mut sxy = 0.0;
        let mut sx2 = 0.0;
        for (i, &y) in state.r_history.iter().enumerate() {
            let x = bounded_f64(i);
            sx += x;
            sy += y;
            sxy += x * y;
            sx2 += x * x;
        }
        let denom = count.mul_add(sx2, -(sx * sx));
        if denom.abs() < f64::EPSILON {
            return 0.0;
        }
        count.mul_add(sxy, -(sx * sy)) / denom
    }

    /// Clear all state.
    pub fn clear(&self) {
        let mut state = self.state.write();
        state.tick = 0;
        state.order = OrderParameter::new(0.0, 0.0);
        state.k_effective = 0.0;
        state.phases.clear();
        state.clusters.clear();
        state.gaps.clear();
        state.r_history.clear();
        state.chimera_detected = false;
    }
}

impl Default for FieldDashboard {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Gap detection helper
// ──────────────────────────────────────────────────────────────

/// Detect phase gaps from sorted phase values.
///
/// Returns a list of gaps with chimera flags set when
/// the gap exceeds `PHASE_GAP_THRESHOLD`.
#[must_use]
pub fn detect_gaps(sorted_phases: &[f64]) -> Vec<PhaseGap> {
    if sorted_phases.len() < 2 {
        return Vec::new();
    }
    let mut gaps = Vec::new();
    let n = sorted_phases.len();

    for i in 0..n {
        let current = sorted_phases[i];
        let next = sorted_phases[(i + 1) % n];
        let gap = if i == n - 1 {
            // Wrap-around gap
            (next + std::f64::consts::TAU) - current
        } else {
            next - current
        };

        if gap > PHASE_GAP_THRESHOLD * 0.5 {
            gaps.push(PhaseGap {
                start_phase: current,
                end_phase: next,
                size: gap,
                is_chimera: gap > PHASE_GAP_THRESHOLD,
            });
        }
    }
    gaps
}

/// Compute the order parameter for a subset of phases.
#[must_use]
pub fn cluster_order_param(phases: &[f64]) -> OrderParameter {
    if phases.is_empty() {
        return OrderParameter::new(0.0, 0.0);
    }
    let n = bounded_f64(phases.len());
    let sum_cos: f64 = phases.iter().map(|p| p.cos()).sum();
    let sum_sin: f64 = phases.iter().map(|p| p.sin()).sum();
    let r = (sum_cos / n).hypot(sum_sin / n);
    let psi = (sum_sin / n).atan2(sum_cos / n);
    OrderParameter::new(r, psi)
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Convert a bounded `usize` to `f64` without precision loss.
///
/// Uses `u32` as an intermediate type — `f64::from(u32)` is lossless.
/// Values above `u32::MAX` saturate (acceptable: our indices are bounded
/// by `SPHERE_CAP` = 200, `R_HISTORY_MAX` = 60, `MAX_CLUSTERS` = 32).
fn bounded_f64(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    // ── SpherePhaseEntry ──

    #[test]
    fn sphere_phase_entry_creation() {
        let entry = SpherePhaseEntry {
            pane_id: PaneId::new("fleet-alpha"),
            phase: 1.5,
            frequency: 0.2,
            status: "working".into(),
            cluster: Some(0),
        };
        assert_eq!(entry.pane_id.as_str(), "fleet-alpha");
        assert!((entry.phase - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn sphere_phase_entry_serializes() {
        let entry = SpherePhaseEntry {
            pane_id: PaneId::new("test"),
            phase: 0.0,
            frequency: 0.1,
            status: "idle".into(),
            cluster: None,
        };
        let json = serde_json::to_string(&entry);
        assert!(json.is_ok());
    }

    // ── ClusterSummary ──

    #[test]
    fn cluster_summary_creation() {
        let cs = ClusterSummary {
            index: 0,
            member_count: 3,
            r: 0.99,
            mean_phase: 1.0,
            members: vec![PaneId::new("a"), PaneId::new("b"), PaneId::new("c")],
        };
        assert_eq!(cs.member_count, 3);
        assert!((cs.r - 0.99).abs() < 1e-6);
    }

    #[test]
    fn cluster_summary_serializes() {
        let cs = ClusterSummary {
            index: 0,
            member_count: 1,
            r: 1.0,
            mean_phase: 0.0,
            members: vec![PaneId::new("a")],
        };
        let json = serde_json::to_string(&cs);
        assert!(json.is_ok());
    }

    // ── PhaseGap ──

    #[test]
    fn phase_gap_creation() {
        let gap = PhaseGap {
            start_phase: 0.5,
            end_phase: 2.0,
            size: 1.5,
            is_chimera: true,
        };
        assert!(gap.is_chimera);
        assert!((gap.size - 1.5).abs() < f64::EPSILON);
    }

    #[test]
    fn phase_gap_serializes() {
        let gap = PhaseGap {
            start_phase: 0.0,
            end_phase: 1.0,
            size: 1.0,
            is_chimera: false,
        };
        let json = serde_json::to_string(&gap);
        assert!(json.is_ok());
    }

    // ── FieldDashboard ──

    #[test]
    fn dashboard_new_is_empty() {
        let d = FieldDashboard::new();
        assert_eq!(d.tick(), 0);
        assert_eq!(d.sphere_count(), 0);
        assert_eq!(d.cluster_count(), 0);
        assert!(!d.chimera_detected());
    }

    #[test]
    fn dashboard_default_is_empty() {
        let d = FieldDashboard::default();
        assert_eq!(d.tick(), 0);
    }

    #[test]
    fn dashboard_update_tick() {
        let d = FieldDashboard::new();
        let order = OrderParameter::new(0.95, 1.0);
        d.update_tick(10, &order, 2.5);
        assert_eq!(d.tick(), 10);
        assert!((d.r() - 0.95).abs() < 1e-6);
        assert!((d.k_effective() - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn dashboard_r_history_accumulates() {
        let d = FieldDashboard::new();
        for i in 0..5_u64 {
            let r = 0.9 + f64::from(i as u32) * 0.01;
            d.update_tick(i, &OrderParameter::new(r, 0.0), 1.0);
        }
        assert_eq!(d.r_history_len(), 5);
    }

    #[test]
    fn dashboard_r_history_caps_at_max() {
        let d = FieldDashboard::new();
        for i in 0..u64::try_from(R_HISTORY_MAX + 10).unwrap_or(0) {
            d.update_tick(i, &OrderParameter::new(0.9, 0.0), 1.0);
        }
        assert_eq!(d.r_history_len(), R_HISTORY_MAX);
    }

    #[test]
    fn dashboard_set_phases() {
        let d = FieldDashboard::new();
        let phases = vec![
            SpherePhaseEntry {
                pane_id: PaneId::new("a"),
                phase: 0.5,
                frequency: 0.1,
                status: "working".into(),
                cluster: None,
            },
            SpherePhaseEntry {
                pane_id: PaneId::new("b"),
                phase: 1.0,
                frequency: 0.2,
                status: "idle".into(),
                cluster: None,
            },
        ];
        d.set_phases(phases);
        assert_eq!(d.sphere_count(), 2);
    }

    #[test]
    fn dashboard_set_clusters() {
        let d = FieldDashboard::new();
        let clusters = vec![ClusterSummary {
            index: 0,
            member_count: 3,
            r: 0.99,
            mean_phase: 1.0,
            members: vec![],
        }];
        d.set_clusters(clusters);
        assert_eq!(d.cluster_count(), 1);
    }

    #[test]
    fn dashboard_set_gaps_chimera() {
        let d = FieldDashboard::new();
        let gaps = vec![PhaseGap {
            start_phase: 0.5,
            end_phase: 2.5,
            size: 2.0,
            is_chimera: true,
        }];
        d.set_gaps(gaps);
        assert!(d.chimera_detected());
    }

    #[test]
    fn dashboard_set_gaps_no_chimera() {
        let d = FieldDashboard::new();
        let gaps = vec![PhaseGap {
            start_phase: 0.5,
            end_phase: 0.8,
            size: 0.3,
            is_chimera: false,
        }];
        d.set_gaps(gaps);
        assert!(!d.chimera_detected());
    }

    #[test]
    fn dashboard_snapshot() {
        let d = FieldDashboard::new();
        d.update_tick(5, &OrderParameter::new(0.93, 0.5), 2.0);
        let snap = d.snapshot();
        assert_eq!(snap.tick, 5);
        assert!((snap.order.r - 0.93).abs() < 1e-6);
        assert!((snap.k_effective - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dashboard_snapshot_serializes() {
        let d = FieldDashboard::new();
        d.update_tick(1, &OrderParameter::new(0.5, 0.0), 1.0);
        let snap = d.snapshot();
        let json = serde_json::to_string(&snap);
        assert!(json.is_ok());
    }

    #[test]
    fn dashboard_r_mean() {
        let d = FieldDashboard::new();
        d.update_tick(0, &OrderParameter::new(0.8, 0.0), 1.0);
        d.update_tick(1, &OrderParameter::new(1.0, 0.0), 1.0);
        let mean = d.r_mean();
        assert!((mean - 0.9).abs() < 1e-6);
    }

    #[test]
    fn dashboard_r_mean_empty() {
        let d = FieldDashboard::new();
        assert!((d.r_mean() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dashboard_r_stddev() {
        let d = FieldDashboard::new();
        d.update_tick(0, &OrderParameter::new(0.9, 0.0), 1.0);
        d.update_tick(1, &OrderParameter::new(0.9, 0.0), 1.0);
        d.update_tick(2, &OrderParameter::new(0.9, 0.0), 1.0);
        assert!(d.r_stddev() < 1e-10);
    }

    #[test]
    fn dashboard_r_stddev_single() {
        let d = FieldDashboard::new();
        d.update_tick(0, &OrderParameter::new(0.9, 0.0), 1.0);
        assert!((d.r_stddev() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dashboard_r_trend_rising() {
        let d = FieldDashboard::new();
        for i in 0..10_u64 {
            d.update_tick(i, &OrderParameter::new(0.5 + 0.05 * f64::from(i as u32), 0.0), 1.0);
        }
        assert!(d.r_trend() > 0.0);
    }

    #[test]
    fn dashboard_r_trend_falling() {
        let d = FieldDashboard::new();
        for i in 0..10_u64 {
            d.update_tick(i, &OrderParameter::new(1.0 - 0.05 * f64::from(i as u32), 0.0), 1.0);
        }
        assert!(d.r_trend() < 0.0);
    }

    #[test]
    fn dashboard_r_trend_flat() {
        let d = FieldDashboard::new();
        for i in 0..10 {
            d.update_tick(i, &OrderParameter::new(0.9, 0.0), 1.0);
        }
        assert!(d.r_trend().abs() < 1e-10);
    }

    #[test]
    fn dashboard_r_trend_empty() {
        let d = FieldDashboard::new();
        assert!((d.r_trend() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dashboard_clear() {
        let d = FieldDashboard::new();
        d.update_tick(10, &OrderParameter::new(0.9, 0.0), 2.0);
        d.set_phases(vec![SpherePhaseEntry {
            pane_id: PaneId::new("a"),
            phase: 0.5,
            frequency: 0.1,
            status: "working".into(),
            cluster: None,
        }]);
        d.clear();
        assert_eq!(d.tick(), 0);
        assert_eq!(d.sphere_count(), 0);
        assert_eq!(d.r_history_len(), 0);
    }

    // ── detect_gaps ──

    #[test]
    fn detect_gaps_empty() {
        assert!(detect_gaps(&[]).is_empty());
    }

    #[test]
    fn detect_gaps_single() {
        assert!(detect_gaps(&[1.0]).is_empty());
    }

    #[test]
    fn detect_gaps_uniform() {
        // 4 phases at 0, π/2, π, 3π/2 — gaps all equal π/2
        let phases = vec![0.0, FRAC_PI_2, PI, 3.0 * FRAC_PI_2];
        let gaps = detect_gaps(&phases);
        // π/2 > PHASE_GAP_THRESHOLD * 0.5 = π/6
        assert!(!gaps.is_empty());
        for gap in &gaps {
            assert!((gap.size - FRAC_PI_2).abs() < 1e-6);
        }
    }

    #[test]
    fn detect_gaps_chimera() {
        // Two clusters with a big gap
        let phases = vec![0.0, 0.1, 0.2, PI, PI + 0.1, PI + 0.2];
        let gaps = detect_gaps(&phases);
        let chimeras: Vec<_> = gaps.iter().filter(|g| g.is_chimera).collect();
        assert!(!chimeras.is_empty());
    }

    #[test]
    fn detect_gaps_two_phases() {
        let phases = vec![0.0, PI];
        let gaps = detect_gaps(&phases);
        assert!(!gaps.is_empty());
    }

    // ── cluster_order_param ──

    #[test]
    fn cluster_order_empty() {
        let o = cluster_order_param(&[]);
        assert!((o.r - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn cluster_order_single() {
        let o = cluster_order_param(&[1.5]);
        assert!((o.r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cluster_order_locked() {
        let o = cluster_order_param(&[0.5, 0.5, 0.5]);
        assert!((o.r - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cluster_order_opposed() {
        let o = cluster_order_param(&[0.0, PI]);
        assert!(o.r < 0.01);
    }

    #[test]
    fn cluster_order_three_equal_spacing() {
        let o = cluster_order_param(&[0.0, TAU / 3.0, 2.0 * TAU / 3.0]);
        assert!(o.r < 0.01);
    }

    // ── Thread safety ──

    #[test]
    fn dashboard_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<FieldDashboard>();
    }

    #[test]
    fn dashboard_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<FieldDashboard>();
    }

    // ── Constants ──

    #[test]
    fn r_history_max_matches_core() {
        assert_eq!(R_HISTORY_MAX, m04_constants::R_HISTORY_MAX);
    }

    #[test]
    fn max_spheres_matches_cap() {
        assert_eq!(MAX_SPHERES, m04_constants::SPHERE_CAP);
    }

    #[test]
    fn phase_gap_threshold_matches_core() {
        assert!((PHASE_GAP_THRESHOLD - m04_constants::PHASE_GAP_THRESHOLD).abs() < f64::EPSILON);
    }

    #[test]
    fn max_clusters_reasonable() {
        assert!(MAX_CLUSTERS >= 4);
        assert!(MAX_CLUSTERS <= 128);
    }

    // ── Order accessor ──

    #[test]
    fn dashboard_order_accessor() {
        let d = FieldDashboard::new();
        d.update_tick(1, &OrderParameter::new(0.85, 1.2), 1.5);
        let o = d.order();
        assert!((o.r - 0.85).abs() < 1e-6);
        assert!((o.psi - 1.2).abs() < 1e-6);
    }

    // ── DashboardSnapshot ──

    #[test]
    fn snapshot_sphere_count() {
        let d = FieldDashboard::new();
        d.set_phases(vec![
            SpherePhaseEntry {
                pane_id: PaneId::new("a"),
                phase: 0.5,
                frequency: 0.1,
                status: "working".into(),
                cluster: None,
            },
            SpherePhaseEntry {
                pane_id: PaneId::new("b"),
                phase: 1.0,
                frequency: 0.2,
                status: "idle".into(),
                cluster: None,
            },
        ]);
        let snap = d.snapshot();
        assert_eq!(snap.sphere_count, 2);
    }

    #[test]
    fn snapshot_r_history() {
        let d = FieldDashboard::new();
        d.update_tick(0, &OrderParameter::new(0.8, 0.0), 1.0);
        d.update_tick(1, &OrderParameter::new(0.9, 0.0), 1.0);
        let snap = d.snapshot();
        assert_eq!(snap.r_history.len(), 2);
        assert!((snap.r_history[0] - 0.8).abs() < 1e-6);
        assert!((snap.r_history[1] - 0.9).abs() < 1e-6);
    }

    /// BUG-Gen21: Verify `r_trend()` computes the correct slope for known
    /// linear ascending data. For r = 0.5, 0.6, 0.7, 0.8, 0.9
    /// (step +0.1 per tick), the slope should be exactly 0.1.
    #[test]
    fn r_trend_ascending_exact_slope() {
        let d = FieldDashboard::new();
        for i in 0..5_u64 {
            let r = 0.5 + 0.1 * f64::from(i as u32);
            d.update_tick(i, &OrderParameter::new(r, 0.0), 1.0);
        }
        let slope = d.r_trend();
        assert!(
            (slope - 0.1).abs() < 1e-10,
            "expected slope ~0.1, got {slope}"
        );
    }

    /// BUG-Gen21: Verify `r_trend()` computes the correct slope for known
    /// linear descending data. For r = 1.0, 0.8, 0.6, 0.4, 0.2
    /// (step -0.2 per tick), the slope should be exactly -0.2.
    #[test]
    fn r_trend_descending_exact_slope() {
        let d = FieldDashboard::new();
        for i in 0..5_u64 {
            let r = 1.0 - 0.2 * f64::from(i as u32);
            d.update_tick(i, &OrderParameter::new(r, 0.0), 1.0);
        }
        let slope = d.r_trend();
        assert!(
            (slope - (-0.2)).abs() < 1e-10,
            "expected slope ~-0.2, got {slope}"
        );
    }

    /// BUG-Gen21: Verify oldest-first ordering in the ring buffer after wrap.
    /// After filling past `R_HISTORY_MAX`, the oldest entry should be evicted
    /// and the trend should reflect only the retained window.
    #[test]
    fn r_trend_correct_after_ring_wrap() {
        let d = FieldDashboard::new();
        // Fill buffer with flat 0.5, then insert rising values
        let max = u64::try_from(R_HISTORY_MAX).unwrap_or(60);
        for i in 0..max {
            d.update_tick(i, &OrderParameter::new(0.5, 0.0), 1.0);
        }
        // Trend should be flat
        assert!(d.r_trend().abs() < 1e-10);

        // Now push 10 more values that are rising
        for i in 0..10_u64 {
            let r = 0.5 + 0.01 * f64::from(i as u32);
            d.update_tick(max + i, &OrderParameter::new(r, 0.0), 1.0);
        }
        // Buffer now has (R_HISTORY_MAX - 10) flat values at 0.5
        // followed by 10 rising values. Overall trend should be positive.
        assert!(d.r_trend() > 0.0);
    }
}
