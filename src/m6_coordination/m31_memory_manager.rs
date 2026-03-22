//! # M31: Memory Manager
//!
//! Fleet-level memory aggregation, statistics, and pruning coordination.
//! Per-sphere memory operations live in `m01_core_types::PaneSphere`;
//! this module handles cross-sphere memory analysis and fleet-wide health.
//!
//! ## Layer: L6 (Coordination) | Module: M31
//! ## Dependencies: L1 (`m01_core_types`, `m04_constants`)
//!
//! ## ORAC Adaptation Notes
//! - Operates on the cached sphere map (sourced via IPC from PV2)
//! - Pruning recommendations are advisory — the daemon applies them
//! - Memory stats feed into the field dashboard and bridge health metrics

use std::collections::{HashMap, HashSet};

use crate::m1_core::m01_core_types::{
    ActivationZones, PaneId, PaneSphere,
};
use crate::m1_core::m04_constants;

// ──────────────────────────────────────────────────────────────
// Fleet memory statistics
// ──────────────────────────────────────────────────────────────

/// Fleet-wide memory statistics.
#[derive(Debug, Clone, Default)]
pub struct FleetMemoryStats {
    /// Total memories across all spheres.
    pub total_memories: usize,
    /// Total active memories (above activation threshold).
    pub active_memories: usize,
    /// Mean memories per sphere.
    pub mean_per_sphere: f64,
    /// Max memories in any single sphere.
    pub max_per_sphere: usize,
    /// Number of spheres at or near capacity.
    pub spheres_near_capacity: usize,
    /// Unique tool names across all memories.
    pub unique_tools: usize,
}

/// Compute fleet-wide memory statistics from a sphere map.
///
/// Returns `FleetMemoryStats::default()` for empty sphere maps.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn compute_stats(spheres: &HashMap<PaneId, PaneSphere>) -> FleetMemoryStats {
    if spheres.is_empty() {
        return FleetMemoryStats::default();
    }

    let mut total = 0_usize;
    let mut active = 0_usize;
    let mut max_per = 0_usize;
    let mut near_capacity = 0_usize;
    let mut all_tools: HashSet<String> = HashSet::new();

    let capacity = m04_constants::MEMORY_MAX_COUNT;
    let near_threshold = capacity.saturating_sub(50);

    for sphere in spheres.values() {
        let count = sphere.memories.len();
        total += count;
        max_per = max_per.max(count);

        if count >= near_threshold {
            near_capacity += 1;
        }

        for mem in &sphere.memories {
            if mem.activation > m04_constants::ACTIVATION_THRESHOLD {
                active += 1;
            }
            all_tools.insert(mem.tool_name.clone());
        }
    }

    #[allow(clippy::cast_precision_loss)]
    let mean = total as f64 / spheres.len() as f64;

    FleetMemoryStats {
        total_memories: total,
        active_memories: active,
        mean_per_sphere: mean,
        max_per_sphere: max_per,
        spheres_near_capacity: near_capacity,
        unique_tools: all_tools.len(),
    }
}

// ──────────────────────────────────────────────────────────────
// Memory pruning
// ──────────────────────────────────────────────────────────────

/// Result of a fleet-wide prune pass.
#[derive(Debug, Clone, Default)]
pub struct PruneResult {
    /// Total memories removed across all spheres.
    pub removed: usize,
    /// Number of spheres that had memories pruned.
    pub spheres_pruned: usize,
}

/// Prune low-activation memories from all spheres.
///
/// Removes memories with activation below `prune_threshold` and enforces
/// the per-sphere capacity limit.
///
/// # Arguments
/// - `spheres`: Mutable reference to the sphere map.
/// - `zones`: Activation zone configuration controlling thresholds.
///
/// # Returns
/// `PruneResult` with counts of removed memories and affected spheres.
#[allow(clippy::implicit_hasher)]
pub fn prune_memories(
    spheres: &mut HashMap<PaneId, PaneSphere>,
    zones: &ActivationZones,
) -> PruneResult {
    let mut total_removed = 0_usize;
    let mut spheres_pruned = 0_usize;

    for sphere in spheres.values_mut() {
        let before = sphere.memories.len();

        // Remove memories below prune threshold
        sphere.memories.retain(|mem| mem.activation >= zones.prune_threshold);

        // Enforce capacity limit by removing lowest-activation memories
        if sphere.memories.len() > zones.capacity {
            // Sort by activation descending, keep the top `capacity`
            sphere
                .memories
                .sort_by(|a, b| b.activation.partial_cmp(&a.activation).unwrap_or(std::cmp::Ordering::Equal));
            sphere.memories.truncate(zones.capacity);
        }

        let removed = before.saturating_sub(sphere.memories.len());
        if removed > 0 {
            total_removed += removed;
            spheres_pruned += 1;
        }
    }

    PruneResult {
        removed: total_removed,
        spheres_pruned,
    }
}

// ──────────────────────────────────────────────────────────────
// Tool frequency analysis
// ──────────────────────────────────────────────────────────────

/// Compute tool usage frequency across the fleet.
///
/// Returns a sorted vector of (tool name, count) pairs, descending by count.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn tool_frequency(spheres: &HashMap<PaneId, PaneSphere>) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for sphere in spheres.values() {
        for mem in &sphere.memories {
            *counts.entry(mem.tool_name.clone()).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted
}

/// Find the most common tools for a specific sphere, up to `limit`.
#[must_use]
pub fn sphere_top_tools(sphere: &PaneSphere, limit: usize) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for mem in &sphere.memories {
        *counts.entry(mem.tool_name.as_str()).or_insert(0) += 1;
    }
    let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted
        .into_iter()
        .take(limit)
        .map(|(s, _)| s.to_owned())
        .collect()
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_core::m01_core_types::{Point3D, SphereMemory};

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn test_sphere() -> PaneSphere {
        PaneSphere::new(pid("test"), "tester")
    }

    fn sphere_with_memories(n: usize) -> PaneSphere {
        let mut s = test_sphere();
        for i in 0..n {
            s.memories.push(SphereMemory::new(
                i as u64,
                Point3D::north(),
                format!("Tool{}", i % 5),
                format!("summary {i}"),
            ));
        }
        s
    }

    fn sphere_with_decayed_memories(n: usize, activation: f64) -> PaneSphere {
        let mut s = test_sphere();
        for i in 0..n {
            let mut mem = SphereMemory::new(
                i as u64,
                Point3D::north(),
                format!("Tool{}", i % 3),
                format!("summary {i}"),
            );
            mem.activation = activation;
            s.memories.push(mem);
        }
        s
    }

    // ── compute_stats ──

    #[test]
    fn stats_empty_spheres() {
        let spheres = HashMap::new();
        let stats = compute_stats(&spheres);
        assert_eq!(stats.total_memories, 0);
        assert!(stats.mean_per_sphere.abs() < f64::EPSILON);
    }

    #[test]
    fn stats_single_empty_sphere() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), test_sphere());
        let stats = compute_stats(&spheres);
        assert_eq!(stats.total_memories, 0);
        assert_eq!(stats.unique_tools, 0);
    }

    #[test]
    fn stats_with_memories() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        let stats = compute_stats(&spheres);
        assert_eq!(stats.total_memories, 10);
        assert_eq!(stats.unique_tools, 5); // Tool0..Tool4
    }

    #[test]
    fn stats_multiple_spheres_mean() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        spheres.insert(pid("b"), sphere_with_memories(20));
        let stats = compute_stats(&spheres);
        assert_eq!(stats.total_memories, 30);
        assert_eq!(stats.max_per_sphere, 20);
        assert!((stats.mean_per_sphere - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn stats_active_memories() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        let stats = compute_stats(&spheres);
        // Fresh memories have activation 1.0 > threshold
        assert!(stats.active_memories > 0);
    }

    // ── prune_memories ──

    #[test]
    fn prune_empty_no_op() {
        let mut spheres: HashMap<PaneId, PaneSphere> = HashMap::new();
        let zones = ActivationZones::standard();
        let result = prune_memories(&mut spheres, &zones);
        assert_eq!(result.removed, 0);
        assert_eq!(result.spheres_pruned, 0);
    }

    #[test]
    fn prune_removes_low_activation() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_decayed_memories(10, 0.01));
        let zones = ActivationZones::standard();
        let result = prune_memories(&mut spheres, &zones);
        assert_eq!(result.removed, 10);
        assert_eq!(result.spheres_pruned, 1);
    }

    #[test]
    fn prune_keeps_high_activation() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10)); // activation=1.0
        let zones = ActivationZones::standard();
        let result = prune_memories(&mut spheres, &zones);
        assert_eq!(result.removed, 0);
    }

    #[test]
    fn prune_enforces_capacity() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(600));
        let zones = ActivationZones {
            active_threshold: 0.3,
            prune_threshold: 0.01, // Low threshold so activation-based prune doesn't fire
            capacity: 500,
        };
        let result = prune_memories(&mut spheres, &zones);
        assert_eq!(result.removed, 100);
        assert_eq!(spheres[&pid("a")].memories.len(), 500);
    }

    // ── tool_frequency ──

    #[test]
    fn tool_frequency_empty() {
        let spheres = HashMap::new();
        let freq = tool_frequency(&spheres);
        assert!(freq.is_empty());
    }

    #[test]
    fn tool_frequency_sorted_descending() {
        let mut spheres = HashMap::new();
        let mut s = test_sphere();
        s.memories.push(SphereMemory::new(0, Point3D::north(), "Read".into(), "a".into()));
        s.memories.push(SphereMemory::new(1, Point3D::north(), "Read".into(), "b".into()));
        s.memories.push(SphereMemory::new(2, Point3D::north(), "Write".into(), "c".into()));
        spheres.insert(pid("a"), s);
        let freq = tool_frequency(&spheres);
        assert_eq!(freq[0].0, "Read");
        assert_eq!(freq[0].1, 2);
    }

    // ── sphere_top_tools ──

    #[test]
    fn top_tools_empty() {
        let s = test_sphere();
        let top = sphere_top_tools(&s, 5);
        assert!(top.is_empty());
    }

    #[test]
    fn top_tools_limited() {
        let s = sphere_with_memories(100);
        let top = sphere_top_tools(&s, 3);
        assert_eq!(top.len(), 3);
    }

    // ── FleetMemoryStats ──

    #[test]
    fn fleet_stats_default() {
        let stats = FleetMemoryStats::default();
        assert_eq!(stats.total_memories, 0);
        assert!(stats.mean_per_sphere.abs() < f64::EPSILON);
    }

    // ── PruneResult ──

    #[test]
    fn prune_result_default() {
        let result = PruneResult::default();
        assert_eq!(result.removed, 0);
        assert_eq!(result.spheres_pruned, 0);
    }
}
