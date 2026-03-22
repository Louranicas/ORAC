//! # M05: Core Traits
//!
//! Dependency-inversion traits for cross-layer abstractions.
//! All trait methods use `&self` with interior mutability (C2).
//!
//! ## Layer: L1 (Foundation)
//! ## Module: M05
//! ## Dependencies: M01 (types), M02 (errors)
//!
//! ## Design Constraints
//! - C2: All methods `&self` — interior mutability via [`parking_lot::RwLock`]
//! - C7: Owned returns through [`RwLock`] (never return references)
//! - All traits require `Send + Sync + Debug`

use super::m01_core_types::OrderParameter;
use super::m02_error_handling::PvResult;

// ──────────────────────────────────────────────────────────────
// Oscillator trait
// ──────────────────────────────────────────────────────────────

/// A phase oscillator in the Kuramoto field.
///
/// Implemented by [`PaneSphere`] and any entity that participates in coupling.
/// All methods take `&self` — mutation uses interior mutability.
pub trait Oscillator: Send + Sync + std::fmt::Debug {
    /// Current phase in [0, 2π).
    fn phase(&self) -> f64;

    /// Natural frequency (Hz).
    fn frequency(&self) -> f64;

    /// Advance the oscillator by one integration step.
    ///
    /// `coupling_force` is the Kuramoto mean-field contribution.
    ///
    /// # Errors
    /// Returns [`PvError`] if the step produces invalid state (e.g. NaN phase).
    fn step(&self, coupling_force: f64) -> PvResult<()>;

    /// Reset phase and momentum to initial conditions.
    ///
    /// # Errors
    /// Returns [`PvError`] if the reset fails (e.g. lock contention).
    fn reset(&self) -> PvResult<()>;
}

// ──────────────────────────────────────────────────────────────
// Learnable trait
// ──────────────────────────────────────────────────────────────

/// A connection or entity that supports Hebbian learning (STDP).
///
/// Implemented by coupling connections and buoy networks.
pub trait Learnable: Send + Sync + std::fmt::Debug {
    /// Apply long-term potentiation (strengthen the connection).
    ///
    /// # Errors
    /// Returns [`PvError`] if the weight update produces invalid state.
    fn ltp(&self, amount: f64) -> PvResult<()>;

    /// Apply long-term depression (weaken the connection).
    ///
    /// # Errors
    /// Returns [`PvError`] if the weight update produces invalid state.
    fn ltd(&self, amount: f64) -> PvResult<()>;

    /// Current connection weight.
    fn weight(&self) -> f64;

    /// Apply time-based decay to the connection weight.
    ///
    /// # Errors
    /// Returns [`PvError`] if the decay produces invalid state.
    fn decay(&self, factor: f64) -> PvResult<()>;
}

// ──────────────────────────────────────────────────────────────
// Bridgeable trait
// ──────────────────────────────────────────────────────────────

/// An external service bridge (SYNTHEX, Nexus, ME, POVM, RM, VMS).
///
/// Bridges are `fire-and-forget` TCP HTTP (no hyper overhead).
/// All methods are fallible — external services may be down.
pub trait Bridgeable: Send + Sync + std::fmt::Debug {
    /// Service name (e.g. "synthex", "nexus", "me").
    fn service_name(&self) -> &str;

    /// Poll the service for its current state. Returns an adjustment factor.
    ///
    /// # Errors
    /// Returns [`PvError::BridgeUnreachable`] or [`PvError::BridgeParse`] on failure.
    fn poll(&self) -> PvResult<f64>;

    /// Post data to the service (`fire-and-forget` semantics).
    ///
    /// # Errors
    /// Returns [`PvError::BridgeUnreachable`] or [`PvError::BridgeError`] on failure.
    fn post(&self, payload: &[u8]) -> PvResult<()>;

    /// Check if the service is healthy.
    ///
    /// # Errors
    /// Returns [`PvError::BridgeUnreachable`] if the service cannot be reached.
    fn health(&self) -> PvResult<bool>;

    /// Whether the last poll result is stale (based on configured interval).
    fn is_stale(&self, current_tick: u64) -> bool;
}

// ──────────────────────────────────────────────────────────────
// Consentable trait
// ──────────────────────────────────────────────────────────────

/// An entity that can consent to or refuse external modulation.
///
/// Core to the Habitat philosophy: the field modulates, it does not command.
/// Every sphere has the right to self-determine coupling, refuse observation,
/// and maintain identity continuity.
pub trait Consentable: Send + Sync + std::fmt::Debug {
    /// Current receptivity to external coupling (0.0 = closed, 1.0 = fully open).
    fn receptivity(&self) -> f64;

    /// Whether this entity has opted out of a specific modulation type.
    fn has_opted_out(&self, modulation: &str) -> bool;

    /// Current consent posture as a human-readable summary.
    fn consent_posture(&self) -> ConsentPosture;
}

/// Summary of a sphere's consent state.
#[derive(Debug, Clone)]
pub struct ConsentPosture {
    /// Overall receptivity (0.0–1.0).
    pub receptivity: f64,
    /// Active opt-outs.
    pub opt_outs: Vec<String>,
    /// Maximum k adjustment this sphere consents to.
    pub max_k_adj: Option<f64>,
}

// ──────────────────────────────────────────────────────────────
// Persistable trait
// ──────────────────────────────────────────────────────────────

/// An entity that can be snapshot'd and restored from persistence.
///
/// Used for field state, bus state, and sphere snapshots.
pub trait Persistable: Send + Sync + std::fmt::Debug {
    /// Serialize current state to bytes.
    ///
    /// # Errors
    /// Returns [`PvError::Snapshot`] if serialization fails.
    fn snapshot(&self) -> PvResult<Vec<u8>>;

    /// Restore state from bytes.
    ///
    /// # Errors
    /// Returns [`PvError::Snapshot`] if deserialization fails.
    fn restore(&self, data: &[u8]) -> PvResult<()>;

    /// Apply any schema migrations needed after restore.
    ///
    /// # Errors
    /// Returns [`PvError::Snapshot`] if migration fails.
    fn migrate(&self) -> PvResult<()>;
}

// ──────────────────────────────────────────────────────────────
// FieldObserver trait
// ──────────────────────────────────────────────────────────────

/// Observer that receives field state updates each tick.
///
/// Used by bridges, persistence, and diagnostic endpoints.
pub trait FieldObserver: Send + Sync + std::fmt::Debug {
    /// Called after each tick with the updated order parameter.
    ///
    /// # Errors
    /// Returns [`PvError`] if the observer fails to process the update.
    fn on_tick(&self, tick: u64, order: &OrderParameter) -> PvResult<()>;
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Test that traits are object-safe (can be used as trait objects)

    #[test]
    fn oscillator_is_object_safe() {
        fn _accepts(_: &dyn Oscillator) {}
    }

    #[test]
    fn learnable_is_object_safe() {
        fn _accepts(_: &dyn Learnable) {}
    }

    #[test]
    fn bridgeable_is_object_safe() {
        fn _accepts(_: &dyn Bridgeable) {}
    }

    #[test]
    fn consentable_is_object_safe() {
        fn _accepts(_: &dyn Consentable) {}
    }

    #[test]
    fn persistable_is_object_safe() {
        fn _accepts(_: &dyn Persistable) {}
    }

    #[test]
    fn field_observer_is_object_safe() {
        fn _accepts(_: &dyn FieldObserver) {}
    }

    // Test ConsentPosture

    #[test]
    fn consent_posture_creation() {
        let posture = ConsentPosture {
            receptivity: 0.8,
            opt_outs: vec!["hebbian".into(), "observation".into()],
            max_k_adj: Some(0.1),
        };
        assert!((posture.receptivity - 0.8).abs() < f64::EPSILON);
        assert_eq!(posture.opt_outs.len(), 2);
        assert!(posture.max_k_adj.is_some());
    }

    #[test]
    fn consent_posture_no_opt_outs() {
        let posture = ConsentPosture {
            receptivity: 1.0,
            opt_outs: Vec::new(),
            max_k_adj: None,
        };
        assert!(posture.opt_outs.is_empty());
        assert!(posture.max_k_adj.is_none());
    }
}
