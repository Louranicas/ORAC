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

use super::m02_error_handling::PvResult;

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
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // Test that traits are object-safe (can be used as trait objects)

    #[test]
    fn bridgeable_is_object_safe() {
        fn _accepts(_: &dyn Bridgeable) {}
    }
}
