//! # M04: Constants
//!
//! All compile-time magic numbers as named constants.
//! Runtime-configurable values live in M03; these are fixed at compile time.
//!
//! ## Layer: L1 (Foundation)
//! ## Module: M04
//! ## Dependencies: None

use std::f64::consts::{FRAC_PI_3, TAU};

// ──────────────────────────────────────────────────────────────
// Tick timing
// ──────────────────────────────────────────────────────────────

/// Default tick interval (seconds). Configurable via `field.tick_interval_secs`.
pub const TICK_INTERVAL_SECS: u64 = 5;

/// Default coupling integration steps per tick. Configurable via config.
pub const COUPLING_STEPS_PER_TICK: usize = 15;

/// Kuramoto Euler integration timestep.
pub const KURAMOTO_DT: f64 = 0.01;

// ──────────────────────────────────────────────────────────────
// Hebbian learning
// ──────────────────────────────────────────────────────────────

/// Long-term potentiation rate (Hebbian weight increase).
pub const HEBBIAN_LTP: f64 = 0.01;

/// Long-term depression rate (Hebbian weight decrease).
pub const HEBBIAN_LTD: f64 = 0.002;

/// LTP multiplier during burst activity.
pub const HEBBIAN_BURST_MULTIPLIER: f64 = 3.0;

/// LTP multiplier for newcomer spheres (first `NEWCOMER_STEPS` ticks).
pub const HEBBIAN_NEWCOMER_MULTIPLIER: f64 = 2.0;

/// Minimum Hebbian coupling weight (prevents complete disconnection).
pub const HEBBIAN_WEIGHT_FLOOR: f64 = 0.15;

// ──────────────────────────────────────────────────────────────
// Coupling network
// ──────────────────────────────────────────────────────────────

/// Default connection weight for new sphere pairs.
pub const DEFAULT_WEIGHT: f64 = 0.18;

/// Weight exponent (fixed `w^2` scaling).
pub const WEIGHT_EXPONENT: f64 = 2.0;

// ──────────────────────────────────────────────────────────────
// Field thresholds
// ──────────────────────────────────────────────────────────────

/// Phase gap for chimera detection (π/3 radians).
pub const PHASE_GAP_THRESHOLD: f64 = FRAC_PI_3;

/// Order parameter above which the field is considered synchronized.
pub const SYNC_THRESHOLD: f64 = 0.5;

/// Angular distance (radians) below which buoys form a tunnel.
pub const TUNNEL_THRESHOLD: f64 = 0.8;

/// `r` above which the field is considered highly coherent.
pub const R_HIGH_THRESHOLD: f64 = 0.8;

/// `r` below which the field is considered incoherent.
pub const R_LOW_THRESHOLD: f64 = 0.3;

/// `r` trend: falling faster than this triggers `RTrend::Falling`.
pub const R_FALLING_THRESHOLD: f64 = -0.03;

/// `r` trend: rising faster than this triggers `RTrend::Rising`.
pub const R_RISING_THRESHOLD: f64 = 0.03;

/// Fraction of idle spheres above which `IdleFleet` action triggers.
pub const IDLE_RATIO_THRESHOLD: f64 = 0.6;

// ──────────────────────────────────────────────────────────────
// R target dynamics
// ──────────────────────────────────────────────────────────────

/// Base `r` target for small/medium fleets.
pub const R_TARGET_BASE: f64 = 0.93;

/// `r` target for large fleets (>50 spheres).
pub const R_TARGET_LARGE_FLEET: f64 = 0.85;

/// Sphere count (as `f64`) above which `R_TARGET_LARGE_FLEET` applies.
pub const LARGE_FLEET_THRESHOLD: f64 = 50.0;

// ──────────────────────────────────────────────────────────────
// Conductor (breathing controller)
// ──────────────────────────────────────────────────────────────

/// Proportional gain for the PI breathing controller.
pub const CONDUCTOR_GAIN: f64 = 0.15;

/// Fraction of emergent signal blended into conductor output.
pub const EMERGENT_BLEND: f64 = 0.3;

// ──────────────────────────────────────────────────────────────
// K modulation bounds
// ──────────────────────────────────────────────────────────────

/// Minimum `k` modulation value (absolute floor).
pub const K_MOD_MIN: f64 = -0.5;

/// Maximum `k` modulation value (absolute ceiling).
pub const K_MOD_MAX: f64 = 1.5;

/// Combined external influence floor (budget constraint).
/// Session 072: Lowered from 0.85 to 0.60 to break convergence trap.
/// With `K=1.5` and `N=6`, `K_MOD_BUDGET_MIN=0.85` gives floor 1.275 (supercritical).
/// At 0.60, floor is 0.90 — subcritical, allows phase diversity and STDP learning.
pub const K_MOD_BUDGET_MIN: f64 = 0.60;

/// Combined external influence ceiling (budget constraint).
pub const K_MOD_BUDGET_MAX: f64 = 1.15;

// ──────────────────────────────────────────────────────────────
// Sphere limits
// ──────────────────────────────────────────────────────────────

/// Maximum number of spheres (security: prevents O(N²) exhaustion).
pub const SPHERE_CAP: usize = 200;

/// Maximum memories per sphere.
pub const MEMORY_MAX_COUNT: usize = 500;

/// Maximum ghost traces retained.
pub const GHOST_MAX: usize = 20;

/// Maximum log entries in the message log.
pub const LOG_MAX: usize = 1000;

/// Maximum inbox messages per sphere.
pub const INBOX_MAX: usize = 50;

/// Maximum `r` history samples retained.
pub const R_HISTORY_MAX: usize = 60;

/// Maximum decision history records.
pub const DECISION_HISTORY_MAX: usize = 100;

// ──────────────────────────────────────────────────────────────
// Sphere dynamics
// ──────────────────────────────────────────────────────────────

/// Memory activation decay per tick step (multiplicative).
pub const DECAY_PER_STEP: f64 = 0.995;

/// Boost strength from sweep activation.
pub const SWEEP_BOOST: f64 = 0.05;

/// Activation threshold below which memories are prunable.
pub const ACTIVATION_THRESHOLD: f64 = 0.3;

/// Steps between memory prune checks.
pub const MEMORY_PRUNE_INTERVAL: u64 = 200;

/// Gentle semantic nudge strength (doesn't override coupling).
pub const SEMANTIC_NUDGE_STRENGTH: f64 = 0.02;

/// Steps during which a newcomer gets boosted LTP.
pub const NEWCOMER_STEPS: u64 = 50;

// ──────────────────────────────────────────────────────────────
// Persistence
// ──────────────────────────────────────────────────────────────

/// Ticks between field snapshots.
pub const SNAPSHOT_INTERVAL: u64 = 60;

/// Warmup ticks after snapshot restore (reduced dynamics).
pub const WARMUP_TICKS: u32 = 5;

// ──────────────────────────────────────────────────────────────
// Network / server
// ──────────────────────────────────────────────────────────────

/// Default HTTP server port.
pub const DEFAULT_PORT: u16 = 8132;

/// Maximum bind retry attempts.
pub const BIND_MAX_RETRIES: u32 = 5;

/// Initial delay between bind retries (milliseconds).
pub const BIND_INITIAL_DELAY_MS: u64 = 500;

// ──────────────────────────────────────────────────────────────
// Mathematical constants (re-exports for convenience)
// ──────────────────────────────────────────────────────────────

/// 2π — full circle in radians. Re-export for convenience.
pub const TWO_PI: f64 = TAU;

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_are_positive_where_expected() {
        assert!(TICK_INTERVAL_SECS > 0);
        assert!(COUPLING_STEPS_PER_TICK > 0);
        assert!(KURAMOTO_DT > 0.0);
        assert!(HEBBIAN_LTP > 0.0);
        assert!(HEBBIAN_LTD > 0.0);
        assert!(HEBBIAN_BURST_MULTIPLIER > 0.0);
        assert!(HEBBIAN_WEIGHT_FLOOR > 0.0);
        assert!(DEFAULT_WEIGHT > 0.0);
        assert!(PHASE_GAP_THRESHOLD > 0.0);
        assert!(SYNC_THRESHOLD > 0.0);
        assert!(TUNNEL_THRESHOLD > 0.0);
        assert!(R_HIGH_THRESHOLD > 0.0);
        assert!(R_TARGET_BASE > 0.0);
        assert!(CONDUCTOR_GAIN > 0.0);
        assert!(SPHERE_CAP > 0);
        assert!(MEMORY_MAX_COUNT > 0);
        assert!(GHOST_MAX > 0);
    }

    #[test]
    fn r_thresholds_ordered() {
        assert!(R_LOW_THRESHOLD < R_HIGH_THRESHOLD);
        assert!(R_HIGH_THRESHOLD < R_TARGET_BASE);
    }

    #[test]
    fn k_mod_bounds_ordered() {
        assert!(K_MOD_MIN < K_MOD_MAX);
        assert!(K_MOD_BUDGET_MIN < K_MOD_BUDGET_MAX);
        assert!(K_MOD_BUDGET_MIN > K_MOD_MIN);
        assert!(K_MOD_BUDGET_MAX < K_MOD_MAX);
    }

    #[test]
    fn phase_gap_threshold_is_pi_over_3() {
        assert!((PHASE_GAP_THRESHOLD - FRAC_PI_3).abs() < f64::EPSILON);
    }

    #[test]
    fn ltp_greater_than_ltd() {
        assert!(HEBBIAN_LTP > HEBBIAN_LTD);
    }

    #[test]
    fn decay_is_subunitary() {
        assert!(DECAY_PER_STEP > 0.0);
        assert!(DECAY_PER_STEP < 1.0);
    }

    #[test]
    fn activation_threshold_in_range() {
        assert!(ACTIVATION_THRESHOLD > 0.0);
        assert!(ACTIVATION_THRESHOLD < 1.0);
    }

    #[test]
    fn two_pi_equals_tau() {
        assert!((TWO_PI - TAU).abs() < f64::EPSILON);
    }

    #[test]
    fn r_target_large_fleet_smaller_than_base() {
        assert!(R_TARGET_LARGE_FLEET < R_TARGET_BASE);
    }

    #[test]
    fn budget_bounds_straddle_unity() {
        assert!(K_MOD_BUDGET_MIN < 1.0);
        assert!(K_MOD_BUDGET_MAX > 1.0);
    }

    #[test]
    fn sphere_cap_matches_claude_md() {
        assert_eq!(SPHERE_CAP, 200);
    }

    #[test]
    fn memory_max_matches_claude_md() {
        assert_eq!(MEMORY_MAX_COUNT, 500);
    }

    #[test]
    fn snapshot_interval_matches_claude_md() {
        assert_eq!(SNAPSHOT_INTERVAL, 60);
    }

    #[test]
    fn default_port_matches_ultraplate() {
        assert_eq!(DEFAULT_PORT, 8132);
    }
}
