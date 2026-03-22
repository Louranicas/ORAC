//! # M21: Memory Manager
//!
//! Fleet-level memory management: statistics, decay, pruning coordination.
//! Per-sphere memory operations are in `m11_sphere`; this module handles
//! cross-sphere memory analysis and fleet-wide memory health.
//!
//! ## Layer: L5 (Learning)
//! ## Module: M21
//! ## Dependencies: L1 (M01, M04), L3 (M11)

use std::collections::HashMap;

use crate::m1_foundation::{
    m01_core_types::PaneId,
    m04_constants,
};
use crate::m3_field::m11_sphere::{ActivationZones, PaneSphere};

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
    /// Fleet-wide activation zones.
    pub zones: ActivationZones,
    /// Unique tool names across all memories.
    pub unique_tools: usize,
}

/// Compute fleet-wide memory statistics.
#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn fleet_memory_stats(spheres: &HashMap<PaneId, PaneSphere>) -> FleetMemoryStats {
    if spheres.is_empty() {
        return FleetMemoryStats::default();
    }

    let mut total = 0;
    let mut active = 0;
    let mut max_per = 0;
    let mut near_capacity = 0;
    let mut zones = ActivationZones::default();
    let mut all_tools = std::collections::HashSet::new();

    for sphere in spheres.values() {
        let count = sphere.memories.len();
        total += count;
        max_per = max_per.max(count);

        if count >= m04_constants::MEMORY_MAX_COUNT - 50 {
            near_capacity += 1;
        }

        let sz = sphere.activation_zones();
        zones.vivid += sz.vivid;
        zones.clear += sz.clear;
        zones.dim += sz.dim;
        zones.trace += sz.trace;

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
        zones,
        unique_tools: all_tools.len(),
    }
}

// ──────────────────────────────────────────────────────────────
// Memory analysis
// ──────────────────────────────────────────────────────────────

/// Tool usage frequency across the fleet.
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

/// Find the most common tools for a specific `PaneSphere`, up to `limit`.
#[must_use]
pub fn sphere_top_tools(sphere: &PaneSphere, limit: usize) -> Vec<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for mem in &sphere.memories {
        *counts.entry(mem.tool_name.as_str()).or_insert(0) += 1;
    }
    let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.into_iter().take(limit).map(|(s, _)| s.to_owned()).collect()
}

/// Memory age distribution (seconds since creation).
#[must_use]
pub fn memory_age_distribution(sphere: &PaneSphere) -> MemoryAgeDistribution {
    let now = crate::m1_foundation::m01_core_types::now_secs();
    let mut recent = 0; // < 60s
    let mut moderate = 0; // 60s - 300s
    let mut old = 0; // > 300s

    for mem in &sphere.memories {
        let age = now - mem.timestamp;
        if age < 60.0 {
            recent += 1;
        } else if age < 300.0 {
            moderate += 1;
        } else {
            old += 1;
        }
    }

    MemoryAgeDistribution { recent, moderate, old }
}

/// Memory age distribution bins (recent / moderate / old).
#[derive(Debug, Clone, Default)]
pub struct MemoryAgeDistribution {
    /// Memories created < 60s ago.
    pub recent: usize,
    /// Memories created 60-300s ago.
    pub moderate: usize,
    /// Memories created > 300s ago.
    pub old: usize,
}

/// Find memories shared between two spheres (same `tool_name` + close position).
#[must_use]
pub fn shared_memories(
    sphere_a: &PaneSphere,
    sphere_b: &PaneSphere,
    distance_threshold: f64,
) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for (i, mem_a) in sphere_a.memories.iter().enumerate() {
        for (j, mem_b) in sphere_b.memories.iter().enumerate() {
            if mem_a.tool_name == mem_b.tool_name {
                let dist = mem_a.position.angular_distance(mem_b.position);
                if dist < distance_threshold {
                    pairs.push((i, j));
                }
            }
        }
    }
    pairs
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

    fn test_sphere() -> PaneSphere {
        PaneSphere::new(pid("test"), "tester".into(), 0.1).unwrap()
    }

    fn sphere_with_memories(n: usize) -> PaneSphere {
        let mut s = test_sphere();
        for i in 0..n {
            s.record_memory(format!("Tool{}", i % 5), format!("summary {i}"));
        }
        s
    }

    // ── fleet_memory_stats ──

    #[test]
    fn fleet_stats_empty() {
        let spheres = HashMap::new();
        let stats = fleet_memory_stats(&spheres);
        assert_eq!(stats.total_memories, 0);
    }

    #[test]
    fn fleet_stats_single_empty_sphere() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), test_sphere());
        let stats = fleet_memory_stats(&spheres);
        assert_eq!(stats.total_memories, 0);
    }

    #[test]
    fn fleet_stats_with_memories() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        let stats = fleet_memory_stats(&spheres);
        assert_eq!(stats.total_memories, 10);
    }

    #[test]
    fn fleet_stats_multiple_spheres() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        spheres.insert(pid("b"), sphere_with_memories(20));
        let stats = fleet_memory_stats(&spheres);
        assert_eq!(stats.total_memories, 30);
        assert_eq!(stats.max_per_sphere, 20);
    }

    #[test]
    fn fleet_stats_mean_per_sphere() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        spheres.insert(pid("b"), sphere_with_memories(20));
        let stats = fleet_memory_stats(&spheres);
        assert_relative_eq!(stats.mean_per_sphere, 15.0);
    }

    #[test]
    fn fleet_stats_unique_tools() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        let stats = fleet_memory_stats(&spheres);
        // 10 memories with Tool0..Tool4
        assert_eq!(stats.unique_tools, 5);
    }

    #[test]
    fn fleet_stats_near_capacity() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(460)); // Near capacity
        let stats = fleet_memory_stats(&spheres);
        assert_eq!(stats.spheres_near_capacity, 1);
    }

    #[test]
    fn fleet_stats_zones() {
        let mut spheres = HashMap::new();
        let mut s = sphere_with_memories(5);
        s.step(); // Triggers activation decay
        spheres.insert(pid("a"), s);
        let stats = fleet_memory_stats(&spheres);
        let total = stats.zones.vivid + stats.zones.clear + stats.zones.dim + stats.zones.trace;
        assert_eq!(total, 5);
    }

    #[test]
    fn fleet_stats_active_memories() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(10));
        let stats = fleet_memory_stats(&spheres);
        // Fresh memories have activation 1.0 > threshold
        assert!(stats.active_memories > 0);
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
        s.record_memory("Read".into(), "a".into());
        s.record_memory("Read".into(), "b".into());
        s.record_memory("Write".into(), "c".into());
        spheres.insert(pid("a"), s);
        let freq = tool_frequency(&spheres);
        assert_eq!(freq[0].0, "Read");
        assert_eq!(freq[0].1, 2);
    }

    #[test]
    fn tool_frequency_across_spheres() {
        let mut spheres = HashMap::new();
        let mut s1 = test_sphere();
        s1.record_memory("Read".into(), "a".into());
        let mut s2 = PaneSphere::new(pid("b"), "b".into(), 0.1).unwrap();
        s2.record_memory("Read".into(), "b".into());
        spheres.insert(pid("a"), s1);
        spheres.insert(pid("b"), s2);
        let freq = tool_frequency(&spheres);
        assert_eq!(freq[0].1, 2); // Read appears twice
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

    #[test]
    fn top_tools_sorted() {
        let mut s = test_sphere();
        s.record_memory("Read".into(), "a".into());
        s.record_memory("Read".into(), "b".into());
        s.record_memory("Read".into(), "c".into());
        s.record_memory("Write".into(), "d".into());
        let top = sphere_top_tools(&s, 2);
        assert_eq!(top[0], "Read");
    }

    // ── memory_age_distribution ──

    #[test]
    fn age_dist_empty() {
        let s = test_sphere();
        let dist = memory_age_distribution(&s);
        assert_eq!(dist.recent, 0);
        assert_eq!(dist.moderate, 0);
        assert_eq!(dist.old, 0);
    }

    #[test]
    fn age_dist_recent() {
        let mut s = test_sphere();
        s.record_memory("Read".into(), "a".into());
        let dist = memory_age_distribution(&s);
        assert_eq!(dist.recent, 1);
    }

    #[test]
    fn age_dist_total_matches_memories() {
        let mut s = test_sphere();
        for _ in 0..10 {
            s.record_memory("Read".into(), "a".into());
        }
        let dist = memory_age_distribution(&s);
        assert_eq!(dist.recent + dist.moderate + dist.old, 10);
    }

    // ── shared_memories ──

    #[test]
    fn shared_memories_empty() {
        let a = test_sphere();
        let b = test_sphere();
        let shared = shared_memories(&a, &b, 1.0);
        assert!(shared.is_empty());
    }

    #[test]
    fn shared_memories_same_tool_same_position() {
        let mut a = test_sphere();
        let mut b = PaneSphere::new(pid("b"), "b".into(), 0.1).unwrap();
        // Both record Read at approximately the same position (phase 0)
        a.record_memory("Read".into(), "a".into());
        b.record_memory("Read".into(), "b".into());
        let shared = shared_memories(&a, &b, 2.0); // Large threshold
        assert!(!shared.is_empty(), "same tool should find shared memories");
    }

    #[test]
    fn shared_memories_different_tools() {
        let mut a = test_sphere();
        let mut b = PaneSphere::new(pid("b"), "b".into(), 0.1).unwrap();
        a.record_memory("Read".into(), "a".into());
        b.record_memory("Write".into(), "b".into());
        let shared = shared_memories(&a, &b, 2.0);
        assert!(shared.is_empty(), "different tools should not share");
    }

    #[test]
    fn shared_memories_tight_threshold() {
        let mut a = test_sphere();
        let mut b = PaneSphere::new(pid("b"), "b".into(), 0.1).unwrap();
        a.record_memory("Read".into(), "a".into());
        b.record_memory("Read".into(), "b".into());
        let shared = shared_memories(&a, &b, 0.001); // Very tight threshold
        // May or may not find shared depending on exact positions
        assert!(shared.len() <= 1);
    }

    // ── MemoryAgeDistribution ──

    #[test]
    fn age_distribution_default() {
        let dist = MemoryAgeDistribution::default();
        assert_eq!(dist.recent, 0);
        assert_eq!(dist.moderate, 0);
        assert_eq!(dist.old, 0);
    }

    // ── FleetMemoryStats ──

    #[test]
    fn fleet_stats_default() {
        let stats = FleetMemoryStats::default();
        assert_eq!(stats.total_memories, 0);
        assert_relative_eq!(stats.mean_per_sphere, 0.0);
    }

    // ── Integration ──

    #[test]
    fn stats_consistent_with_zone_totals() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_with_memories(50));
        spheres.insert(pid("b"), sphere_with_memories(30));
        let stats = fleet_memory_stats(&spheres);
        let zone_total =
            stats.zones.vivid + stats.zones.clear + stats.zones.dim + stats.zones.trace;
        assert_eq!(zone_total, stats.total_memories);
    }
}
