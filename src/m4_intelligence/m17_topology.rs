//! # M18: Coupling Topology
//!
//! Topology analysis and neighborhood queries for the coupling network.
//! Weight² amplification, neighbor discovery, coupling strength metrics.
//!
//! ## Layer: L4 (Coupling)
//! ## Module: M18
//! ## Dependencies: L1 (M01, M04), M16

use crate::m1_core::{
    m01_core_types::PaneId,
    m04_constants,
};
use super::m15_coupling_network::CouplingNetwork;

// ──────────────────────────────────────────────────────────────
// Neighbor info
// ──────────────────────────────────────────────────────────────

/// Information about a neighboring sphere in the coupling topology.
#[derive(Debug, Clone)]
pub struct NeighborInfo {
    /// Neighbor sphere ID.
    pub id: PaneId,
    /// Coupling weight (base × type).
    pub effective_weight: f64,
    /// Weight² amplified (for NA-25 topology-aware coupling).
    pub weight_squared: f64,
    /// Phase difference from the queried sphere.
    pub phase_diff: f64,
}

// ──────────────────────────────────────────────────────────────
// Topology queries
// ──────────────────────────────────────────────────────────────

/// Get neighbors of a sphere sorted by effective weight (descending).
#[must_use]
pub fn neighbors(network: &CouplingNetwork, sphere_id: &PaneId) -> Vec<NeighborInfo> {
    let my_phase = network.phases.get(sphere_id).copied().unwrap_or(0.0);
    let mut result: Vec<NeighborInfo> = network
        .connections
        .iter()
        .filter(|c| c.from == *sphere_id)
        .map(|c| {
            let ew = c.weight * c.type_weight;
            let other_phase = network.phases.get(&c.to).copied().unwrap_or(0.0);
            NeighborInfo {
                id: c.to.clone(),
                effective_weight: ew,
                weight_squared: ew * ew,
                phase_diff: crate::m1_core::m01_core_types::phase_diff(other_phase, my_phase),
            }
        })
        .collect();

    result.sort_by(|a, b| {
        b.effective_weight
            .partial_cmp(&a.effective_weight)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    result
}

/// Get the strongest neighbor of a sphere (highest effective weight).
#[must_use]
pub fn strongest_neighbor(network: &CouplingNetwork, sphere_id: &PaneId) -> Option<NeighborInfo> {
    neighbors(network, sphere_id).into_iter().next()
}

/// Mean effective coupling weight for a sphere.
#[must_use]
pub fn mean_coupling_weight(network: &CouplingNetwork, sphere_id: &PaneId) -> f64 {
    let nbrs = neighbors(network, sphere_id);
    if nbrs.is_empty() {
        return 0.0;
    }
    let count = f64::from(u32::try_from(nbrs.len()).unwrap_or(u32::MAX));
    nbrs.iter().map(|n| n.effective_weight).sum::<f64>() / count
}

/// Degree (number of connections) for a sphere.
#[must_use]
pub fn degree(network: &CouplingNetwork, sphere_id: &PaneId) -> usize {
    network
        .connections
        .iter()
        .filter(|c| c.from == *sphere_id)
        .count()
}

// ──────────────────────────────────────────────────────────────
// Network-wide topology metrics
// ──────────────────────────────────────────────────────────────

/// Network-wide topology summary.
#[derive(Debug, Clone)]
pub struct TopologySummary {
    /// Total number of connections.
    pub total_connections: usize,
    /// Mean weight across all connections.
    pub mean_weight: f64,
    /// Maximum weight in the network.
    pub max_weight: f64,
    /// Minimum weight in the network.
    pub min_weight: f64,
    /// Number of connections above the `HEBBIAN_WEIGHT_FLOOR`.
    pub active_connections: usize,
    /// Network density (actual connections / possible connections).
    pub density: f64,
}

/// Compute network-wide topology summary.
#[must_use]
pub fn topology_summary(network: &CouplingNetwork) -> TopologySummary {
    let total = network.connections.len();
    if total == 0 {
        return TopologySummary {
            total_connections: 0,
            mean_weight: 0.0,
            max_weight: 0.0,
            min_weight: 0.0,
            active_connections: 0,
            density: 0.0,
        };
    }

    let weights: Vec<f64> = network.connections.iter().map(|c| c.weight).collect();
    let sum: f64 = weights.iter().sum();
    let total_f = f64::from(u32::try_from(total).unwrap_or(u32::MAX));
    let mean = sum / total_f;
    let max = weights.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let min = weights.iter().copied().fold(f64::INFINITY, f64::min);
    let active = weights
        .iter()
        .filter(|&&w| w > m04_constants::HEBBIAN_WEIGHT_FLOOR + 0.01)
        .count();

    let n = network.sphere_count();
    let possible = if n > 1 {
        f64::from(u32::try_from(n * (n - 1)).unwrap_or(u32::MAX))
    } else {
        1.0
    };
    let density = total_f / possible;

    TopologySummary {
        total_connections: total,
        mean_weight: mean,
        max_weight: max,
        min_weight: min,
        active_connections: active,
        density,
    }
}

/// Find the most coupled pair (highest effective weight) in the network.
#[must_use]
pub fn most_coupled_pair(network: &CouplingNetwork) -> Option<(PaneId, PaneId, f64)> {
    network
        .connections
        .iter()
        .max_by(|a, b| {
            let ea = a.weight * a.type_weight;
            let eb = b.weight * b.type_weight;
            ea.partial_cmp(&eb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|c| (c.from.clone(), c.to.clone(), c.weight * c.type_weight))
}

/// Find the least coupled pair (lowest effective weight) in the network.
#[must_use]
pub fn least_coupled_pair(network: &CouplingNetwork) -> Option<(PaneId, PaneId, f64)> {
    network
        .connections
        .iter()
        .min_by(|a, b| {
            let ea = a.weight * a.type_weight;
            let eb = b.weight * b.type_weight;
            ea.partial_cmp(&eb).unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|c| (c.from.clone(), c.to.clone(), c.weight * c.type_weight))
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

    fn two_sphere_network() -> CouplingNetwork {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net
    }

    fn three_sphere_network() -> CouplingNetwork {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        net.register(pid("b"), 1.0, 0.1);
        net.register(pid("c"), 2.0, 0.1);
        net
    }

    // ── Neighbors ──

    #[test]
    fn neighbors_empty_network() {
        let net = CouplingNetwork::new();
        let nbrs = neighbors(&net, &pid("a"));
        assert!(nbrs.is_empty());
    }

    #[test]
    fn neighbors_single_sphere_no_neighbors() {
        let mut net = CouplingNetwork::new();
        net.register(pid("a"), 0.0, 0.1);
        let nbrs = neighbors(&net, &pid("a"));
        assert!(nbrs.is_empty());
    }

    #[test]
    fn neighbors_two_spheres() {
        let net = two_sphere_network();
        let nbrs = neighbors(&net, &pid("a"));
        assert_eq!(nbrs.len(), 1);
        assert_eq!(nbrs[0].id.as_str(), "b");
    }

    #[test]
    fn neighbors_sorted_by_weight() {
        let mut net = three_sphere_network();
        net.set_weight(&pid("a"), &pid("c"), 0.9);
        let nbrs = neighbors(&net, &pid("a"));
        assert_eq!(nbrs.len(), 2);
        assert!(nbrs[0].effective_weight >= nbrs[1].effective_weight);
    }

    #[test]
    fn neighbor_weight_squared() {
        let net = two_sphere_network();
        let nbrs = neighbors(&net, &pid("a"));
        let ew = nbrs[0].effective_weight;
        assert_relative_eq!(nbrs[0].weight_squared, ew * ew, epsilon = 1e-10);
    }

    #[test]
    fn neighbor_phase_diff_computed() {
        let net = two_sphere_network();
        let nbrs = neighbors(&net, &pid("a"));
        // a is at phase 0, b is at phase 1.0 (approximately)
        assert!(nbrs[0].phase_diff.abs() > 0.01);
    }

    // ── Strongest neighbor ──

    #[test]
    fn strongest_neighbor_exists() {
        let net = two_sphere_network();
        let sn = strongest_neighbor(&net, &pid("a"));
        assert!(sn.is_some());
    }

    #[test]
    fn strongest_neighbor_empty() {
        let net = CouplingNetwork::new();
        let sn = strongest_neighbor(&net, &pid("a"));
        assert!(sn.is_none());
    }

    #[test]
    fn strongest_neighbor_correct() {
        let mut net = three_sphere_network();
        net.set_weight(&pid("a"), &pid("c"), 0.9);
        let sn = strongest_neighbor(&net, &pid("a")).unwrap();
        assert_eq!(sn.id.as_str(), "c");
    }

    // ── Mean coupling weight ──

    #[test]
    fn mean_weight_no_neighbors() {
        let net = CouplingNetwork::new();
        assert_relative_eq!(mean_coupling_weight(&net, &pid("a")), 0.0);
    }

    #[test]
    fn mean_weight_uniform() {
        let net = three_sphere_network();
        let mw = mean_coupling_weight(&net, &pid("a"));
        assert!(mw > 0.0);
    }

    // ── Degree ──

    #[test]
    fn degree_empty() {
        let net = CouplingNetwork::new();
        assert_eq!(degree(&net, &pid("a")), 0);
    }

    #[test]
    fn degree_two_spheres() {
        let net = two_sphere_network();
        assert_eq!(degree(&net, &pid("a")), 1);
    }

    #[test]
    fn degree_three_spheres() {
        let net = three_sphere_network();
        assert_eq!(degree(&net, &pid("a")), 2);
    }

    // ── Topology summary ──

    #[test]
    fn summary_empty() {
        let net = CouplingNetwork::new();
        let summary = topology_summary(&net);
        assert_eq!(summary.total_connections, 0);
    }

    #[test]
    fn summary_two_spheres() {
        let net = two_sphere_network();
        let summary = topology_summary(&net);
        assert_eq!(summary.total_connections, 2);
        assert!(summary.mean_weight > 0.0);
        assert!(summary.density > 0.0);
    }

    #[test]
    fn summary_three_spheres() {
        let net = three_sphere_network();
        let summary = topology_summary(&net);
        assert_eq!(summary.total_connections, 6);
        assert_relative_eq!(summary.density, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn summary_weights_bounded() {
        let net = three_sphere_network();
        let summary = topology_summary(&net);
        assert!(summary.min_weight >= 0.0, "min_weight should be >= 0");
        assert!(summary.max_weight <= 1.0, "max_weight should be <= 1");
        // Allow small epsilon for floating point
        assert!(
            summary.mean_weight >= summary.min_weight - 1e-10,
            "mean ({}) >= min ({})",
            summary.mean_weight,
            summary.min_weight
        );
        assert!(
            summary.mean_weight <= summary.max_weight + 1e-10,
            "mean ({}) <= max ({})",
            summary.mean_weight,
            summary.max_weight
        );
    }

    // ── Most/least coupled pair ──

    #[test]
    fn most_coupled_empty() {
        let net = CouplingNetwork::new();
        assert!(most_coupled_pair(&net).is_none());
    }

    #[test]
    fn most_coupled_exists() {
        let net = two_sphere_network();
        let mcp = most_coupled_pair(&net);
        assert!(mcp.is_some());
    }

    #[test]
    fn most_coupled_correct() {
        let mut net = three_sphere_network();
        net.set_weight(&pid("a"), &pid("c"), 0.95);
        let (from, to, ew) = most_coupled_pair(&net).unwrap();
        assert_eq!(from.as_str(), "a");
        assert_eq!(to.as_str(), "c");
        assert!(ew > 0.5);
    }

    #[test]
    fn least_coupled_empty() {
        let net = CouplingNetwork::new();
        assert!(least_coupled_pair(&net).is_none());
    }

    #[test]
    fn least_coupled_exists() {
        let net = two_sphere_network();
        assert!(least_coupled_pair(&net).is_some());
    }

    // ── Weight squared amplification ──

    #[test]
    fn weight_squared_amplifies_strong() {
        let mut net = two_sphere_network();
        net.set_weight(&pid("a"), &pid("b"), 0.9);
        let nbrs = neighbors(&net, &pid("a"));
        // w²(0.9 * 0.6)² = 0.2916 < 0.54 = ew
        assert!(nbrs[0].weight_squared < nbrs[0].effective_weight);
    }

    #[test]
    fn weight_squared_weakens_weak() {
        let net = two_sphere_network();
        let nbrs = neighbors(&net, &pid("a"));
        // Default weight is 0.18 * 0.6 = 0.108, w² = 0.011664
        assert!(nbrs[0].weight_squared < nbrs[0].effective_weight);
    }

    // ── Integration ──

    #[test]
    fn topology_after_deregister() {
        let mut net = three_sphere_network();
        net.deregister(&pid("b"));
        let summary = topology_summary(&net);
        assert_eq!(summary.total_connections, 2); // a↔c
    }

    #[test]
    fn topology_after_weight_change() {
        let mut net = three_sphere_network();
        let before = topology_summary(&net).mean_weight;
        net.set_weight(&pid("a"), &pid("b"), 0.95);
        let after = topology_summary(&net).mean_weight;
        assert!(after > before);
    }

    // ── Stress test ──

    #[test]
    fn topology_50_spheres() {
        let mut net = CouplingNetwork::new();
        for i in 0..50 {
            let phase = (f64::from(i) / 50.0) * std::f64::consts::TAU;
            net.register(pid(&format!("s{i}")), phase, 0.1);
        }
        let summary = topology_summary(&net);
        assert_eq!(summary.total_connections, 50 * 49); // N(N-1) directed
        assert_relative_eq!(summary.density, 1.0, epsilon = 1e-10);
    }
}
