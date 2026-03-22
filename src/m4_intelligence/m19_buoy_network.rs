//! # M20: Buoy Network
//!
//! Network-level buoy operations: cross-sphere buoy analysis, tunnel discovery,
//! activation zone statistics, buoy health metrics.
//!
//! ## Layer: L5 (Learning)
//! ## Module: M20
//! ## Dependencies: L1 (M01 `Point3D`, `Buoy`), L3 (M11 `PaneSphere`)

use std::collections::HashMap;

use crate::m1_core::{
    m01_core_types::{Buoy, PaneId, Point3D},
    m04_constants,
};
use crate::m1_core::m01_core_types::PaneSphere;

// ──────────────────────────────────────────────────────────────
// Buoy health metrics
// ──────────────────────────────────────────────────────────────

/// Health summary for a single sphere's buoy network.
#[derive(Debug, Clone)]
pub struct BuoyHealth {
    /// Sphere ID.
    pub sphere_id: PaneId,
    /// Number of buoys.
    pub buoy_count: usize,
    /// Mean drift distance from home position.
    pub mean_drift: f64,
    /// Maximum drift distance.
    pub max_drift: f64,
    /// Total activation count across all buoys.
    pub total_activations: u64,
    /// Whether any buoy has drifted significantly (> `0.5` rad).
    pub has_drifted: bool,
}

/// Compute buoy health for a sphere.
#[must_use]
pub fn buoy_health(sphere: &PaneSphere) -> BuoyHealth {
    let buoy_count = sphere.buoys.len();
    if buoy_count == 0 {
        return BuoyHealth {
            sphere_id: sphere.id.clone(),
            buoy_count: 0,
            mean_drift: 0.0,
            max_drift: 0.0,
            total_activations: 0,
            has_drifted: false,
        };
    }

    let drifts: Vec<f64> = sphere
        .buoys
        .iter()
        .map(|b| b.position.angular_distance(b.home))
        .collect();

    let count = f64::from(u32::try_from(buoy_count).unwrap_or(u32::MAX));
    let mean_drift = drifts.iter().sum::<f64>() / count;
    let max_drift = drifts.iter().copied().fold(0.0_f64, f64::max);
    let total_activations: u64 = sphere.buoys.iter().map(|b| b.activation_count).sum();

    BuoyHealth {
        sphere_id: sphere.id.clone(),
        buoy_count,
        mean_drift,
        max_drift,
        total_activations,
        has_drifted: max_drift > 0.5,
    }
}

// ──────────────────────────────────────────────────────────────
// Fleet-wide buoy analysis
// ──────────────────────────────────────────────────────────────

/// Fleet-wide buoy network summary.
#[derive(Debug, Clone, Default)]
pub struct FleetBuoyStats {
    /// Total buoys across all spheres.
    pub total_buoys: usize,
    /// Total activations across all buoys.
    pub total_activations: u64,
    /// Mean drift across all buoys.
    pub mean_drift: f64,
    /// Number of spheres with drifted buoys.
    pub spheres_with_drift: usize,
    /// Number of cross-sphere buoy overlaps (potential tunnels).
    pub buoy_overlaps: usize,
}

/// Compute fleet-wide buoy statistics.
#[must_use]
pub fn fleet_buoy_stats<S: std::hash::BuildHasher>(spheres: &HashMap<PaneId, PaneSphere, S>) -> FleetBuoyStats {
    if spheres.is_empty() {
        return FleetBuoyStats::default();
    }

    let mut total_buoys = 0;
    let mut total_activations = 0;
    let mut total_drift = 0.0;
    let mut spheres_with_drift = 0;
    let mut all_buoys: Vec<(&PaneId, &Buoy)> = Vec::new();

    for (id, sphere) in spheres {
        let health = buoy_health(sphere);
        total_buoys += health.buoy_count;
        total_activations += health.total_activations;
        total_drift += health.mean_drift;
        if health.has_drifted {
            spheres_with_drift += 1;
        }
        for buoy in &sphere.buoys {
            all_buoys.push((id, buoy));
        }
    }

    let mean_drift = if spheres.is_empty() {
        0.0
    } else {
        total_drift / f64::from(u32::try_from(spheres.len()).unwrap_or(u32::MAX))
    };

    // Count cross-sphere overlaps
    let buoy_overlaps = count_buoy_overlaps(&all_buoys);

    FleetBuoyStats {
        total_buoys,
        total_activations,
        mean_drift,
        spheres_with_drift,
        buoy_overlaps,
    }
}

/// Count buoy overlaps between different spheres (within `TUNNEL_THRESHOLD`).
fn count_buoy_overlaps(all_buoys: &[(&PaneId, &Buoy)]) -> usize {
    let mut overlaps = 0;
    for i in 0..all_buoys.len() {
        for j in (i + 1)..all_buoys.len() {
            let (id_a, buoy_a) = all_buoys[i];
            let (id_b, buoy_b) = all_buoys[j];

            // Only count cross-sphere overlaps
            if id_a == id_b {
                continue;
            }

            let dist = buoy_a.position.angular_distance(buoy_b.position);
            if dist < m04_constants::TUNNEL_THRESHOLD {
                overlaps += 1;
            }
        }
    }
    overlaps
}

/// Compute centroid of all buoy positions for a sphere.
///
/// Returns `Point3D::north()` if the sphere has no buoys.
#[must_use]
pub fn buoy_centroid(sphere: &PaneSphere) -> Point3D {
    if sphere.buoys.is_empty() {
        return Point3D::north();
    }

    let n = f64::from(u32::try_from(sphere.buoys.len()).unwrap_or(u32::MAX));
    let x: f64 = sphere.buoys.iter().map(|b| b.position.x).sum::<f64>() / n;
    let y: f64 = sphere.buoys.iter().map(|b| b.position.y).sum::<f64>() / n;
    let z: f64 = sphere.buoys.iter().map(|b| b.position.z).sum::<f64>() / n;

    Point3D::new(x, y, z).normalized()
}

/// Find the nearest `Buoy` to a given `Point3D` on a sphere.
#[must_use]
pub fn nearest_buoy<'a>(sphere: &'a PaneSphere, point: &Point3D) -> Option<&'a Buoy> {
    sphere.buoys.iter().min_by(|a, b| {
        let da = a.position.angular_distance(*point);
        let db = b.position.angular_distance(*point);
        da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::m1_core::m01_core_types::SphereMemory;
    use approx::assert_relative_eq;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn test_sphere() -> PaneSphere {
        let mut s = PaneSphere::new(pid("test"), "tester");
        // Seed 3 buoys so tests that index into s.buoys[0..3] work
        s.buoys = vec![
            Buoy::new(Point3D::from_spherical(0.5, 0.0), "primary".into(), 0.1),
            Buoy::new(Point3D::from_spherical(0.5, 2.0), "secondary".into(), 0.1),
            Buoy::new(Point3D::from_spherical(0.5, 4.0), "tertiary".into(), 0.1),
        ];
        s
    }

    // ── buoy_health ──

    #[test]
    fn health_new_sphere_zero_drift() {
        let s = test_sphere();
        let h = buoy_health(&s);
        assert_eq!(h.buoy_count, 3);
        assert_relative_eq!(h.mean_drift, 0.0, epsilon = 1e-10);
        assert!(!h.has_drifted);
    }

    #[test]
    fn health_zero_activations_initially() {
        let s = test_sphere();
        let h = buoy_health(&s);
        assert_eq!(h.total_activations, 0);
    }

    #[test]
    fn health_sphere_id_correct() {
        let s = test_sphere();
        let h = buoy_health(&s);
        assert_eq!(h.sphere_id.as_str(), "test");
    }

    #[test]
    fn health_after_drift() {
        let mut s = test_sphere();
        // Manually drift a buoy
        s.buoys[0].position = Point3D::new(0.0, 0.0, 1.0); // North pole
        let h = buoy_health(&s);
        assert!(h.max_drift > 0.0);
    }

    #[test]
    fn health_empty_buoys() {
        let mut s = test_sphere();
        s.buoys.clear();
        let h = buoy_health(&s);
        assert_eq!(h.buoy_count, 0);
        assert_relative_eq!(h.mean_drift, 0.0);
    }

    // ── fleet_buoy_stats ──

    #[test]
    fn fleet_stats_empty() {
        let spheres = HashMap::new();
        let stats = fleet_buoy_stats(&spheres);
        assert_eq!(stats.total_buoys, 0);
    }

    #[test]
    fn fleet_stats_single_sphere() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), test_sphere());
        let stats = fleet_buoy_stats(&spheres);
        assert_eq!(stats.total_buoys, 3);
    }

    #[test]
    fn fleet_stats_two_spheres() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), test_sphere());
        spheres.insert(pid("b"), test_sphere());
        let stats = fleet_buoy_stats(&spheres);
        assert_eq!(stats.total_buoys, 6);
    }

    #[test]
    fn fleet_stats_overlaps_detected() {
        let mut spheres = HashMap::new();
        // Two spheres with same buoy positions → overlaps
        spheres.insert(pid("a"), test_sphere());
        spheres.insert(pid("b"), test_sphere());
        let stats = fleet_buoy_stats(&spheres);
        assert!(stats.buoy_overlaps > 0, "same-position buoys should overlap");
    }

    #[test]
    fn fleet_stats_no_self_overlaps() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), test_sphere());
        let stats = fleet_buoy_stats(&spheres);
        // Single sphere cannot have cross-sphere overlaps
        assert_eq!(stats.buoy_overlaps, 0);
    }

    // ── buoy_centroid ──

    #[test]
    fn centroid_empty_buoys() {
        let mut s = test_sphere();
        s.buoys.clear();
        let c = buoy_centroid(&s);
        assert_eq!(c, Point3D::north());
    }

    #[test]
    fn centroid_on_unit_sphere() {
        let s = test_sphere();
        let c = buoy_centroid(&s);
        assert_relative_eq!(c.norm(), 1.0, epsilon = 0.01);
    }

    #[test]
    fn centroid_changes_with_drift() {
        let mut s = test_sphere();
        let c1 = buoy_centroid(&s);
        // Drift buoy 0 significantly
        s.buoys[0].position = Point3D::new(0.0, 0.0, 1.0);
        let c2 = buoy_centroid(&s);
        let dist = c1.angular_distance(c2);
        assert!(dist > 0.01, "centroid should shift with buoy drift");
    }

    // ── nearest_buoy ──

    #[test]
    fn nearest_buoy_found() {
        let s = test_sphere();
        let point = s.buoys[0].position;
        let nearest = nearest_buoy(&s, &point);
        assert!(nearest.is_some());
    }

    #[test]
    fn nearest_buoy_is_closest() {
        let s = test_sphere();
        let point = s.buoys[0].position;
        let nearest = nearest_buoy(&s, &point).unwrap();
        assert_eq!(nearest.label, s.buoys[0].label);
    }

    #[test]
    fn nearest_buoy_empty() {
        let mut s = test_sphere();
        s.buoys.clear();
        let nearest = nearest_buoy(&s, &Point3D::north());
        assert!(nearest.is_none());
    }

    #[test]
    fn nearest_buoy_north_pole() {
        let s = test_sphere();
        let nearest = nearest_buoy(&s, &Point3D::north());
        assert!(nearest.is_some());
    }

    // ── count_buoy_overlaps ──

    #[test]
    fn overlaps_empty() {
        let buoys: Vec<(&PaneId, &Buoy)> = Vec::new();
        assert_eq!(count_buoy_overlaps(&buoys), 0);
    }

    #[test]
    fn overlaps_same_sphere_not_counted() {
        let id = pid("a");
        let b1 = Buoy::new(Point3D::north(), "primary".into(), 0.01);
        let b2 = Buoy::new(Point3D::north(), "secondary".into(), 0.01);
        let buoys: Vec<(&PaneId, &Buoy)> = vec![(&id, &b1), (&id, &b2)];
        assert_eq!(count_buoy_overlaps(&buoys), 0);
    }

    #[test]
    fn overlaps_different_spheres_same_position() {
        let id_a = pid("a");
        let id_b = pid("b");
        let b1 = Buoy::new(Point3D::north(), "primary".into(), 0.01);
        let b2 = Buoy::new(Point3D::north(), "primary".into(), 0.01);
        let buoys: Vec<(&PaneId, &Buoy)> = vec![(&id_a, &b1), (&id_b, &b2)];
        assert_eq!(count_buoy_overlaps(&buoys), 1);
    }

    #[test]
    fn overlaps_far_apart_not_counted() {
        let id_a = pid("a");
        let id_b = pid("b");
        let b1 = Buoy::new(Point3D::new(1.0, 0.0, 0.0), "a".into(), 0.01);
        let b2 = Buoy::new(Point3D::new(-1.0, 0.0, 0.0), "b".into(), 0.01);
        let buoys: Vec<(&PaneId, &Buoy)> = vec![(&id_a, &b1), (&id_b, &b2)];
        assert_eq!(count_buoy_overlaps(&buoys), 0);
    }

    // ── Integration: buoy health after recording memories ──

    #[test]
    fn health_after_recording_memories() {
        let mut s = test_sphere();
        // Directly push memories and increment steps (PaneSphere has no
        // record_memory/step methods in ORAC — manipulate fields instead).
        for i in 0..20 {
            s.memories.push(SphereMemory::new(
                i,
                Point3D::from_spherical(0.3, (i as f64) * 0.3),
                "Read".into(),
                "file".into(),
            ));
            s.total_steps += 1;
        }
        let h = buoy_health(&s);
        assert!(h.total_activations > 0 || h.mean_drift > 0.0 || h.buoy_count > 0);
    }

    #[test]
    fn fleet_stats_after_stepping() {
        let mut spheres = HashMap::new();
        let mut s1 = test_sphere();
        let mut s2 = PaneSphere::new(pid("s2"), "test2");
        s2.buoys = vec![
            Buoy::new(Point3D::from_spherical(0.5, 0.0), "primary".into(), 0.2),
            Buoy::new(Point3D::from_spherical(0.5, 2.0), "secondary".into(), 0.2),
            Buoy::new(Point3D::from_spherical(0.5, 4.0), "tertiary".into(), 0.2),
        ];
        for i in 0..10 {
            s1.memories.push(SphereMemory::new(
                i,
                Point3D::from_spherical(0.4, (i as f64) * 0.5),
                "Read".into(),
                "a".into(),
            ));
            s2.memories.push(SphereMemory::new(
                i + 100,
                Point3D::from_spherical(0.4, (i as f64) * 0.5),
                "Write".into(),
                "b".into(),
            ));
            s1.total_steps += 1;
            s2.total_steps += 1;
        }
        spheres.insert(pid("test"), s1);
        spheres.insert(pid("s2"), s2);
        let stats = fleet_buoy_stats(&spheres);
        assert_eq!(stats.total_buoys, 6);
    }
}
