//! # Layer 8: Evolution
//!
//! Self-improving coordination via 5-phase RALPH loop. Cloned from ME with critical fix:
//! **multi-parameter mutation** (NOT mono-parameter like ME's BUG-035).
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | `m36` | RALPH Engine | 5-phase loop: Recognize→Analyze→Learn→Propose→Harvest |
//! | `m37` | Emergence Detector | Ring buffer, TTL decay, cap 5,000 |
//! | `m38` | Correlation Engine | Pathway discovery and correlation mining |
//! | `m39` | Fitness Tensor | 12-dimensional weighted fitness evaluation |
//! | `m40` | Mutation Selector | Diversity-enforced: round-robin, cooldown, rejection gate |
//!
//! ## BUG-035 Fix (CRITICAL)
//!
//! ME's evolution chamber targeted `min_confidence` in 318/380 mutations (84%).
//! ORAC enforces:
//! - Round-robin across full parameter pool
//! - 10-generation cooldown per parameter
//! - Reject if >50% of last 20 mutations hit same parameter
//!
//! ## Design Invariants
//!
//! - Feature-gated: `#[cfg(feature = "evolution")]`
//! - Depends on: `m1_core`, `m4_intelligence`, `m5_bridges`, `m7_monitoring`
//! - Snapshot + rollback: atomic state capture before each mutation

/// 5-phase RALPH loop: Recognize, Analyze, Learn, Propose, Harvest
pub mod m36_ralph_engine;
/// Emergence detection with ring buffer, TTL decay, cap 5000
pub mod m37_emergence_detector;
/// Pathway discovery and correlation mining
pub mod m38_correlation_engine;
/// 12-dimensional weighted fitness evaluation
pub mod m39_fitness_tensor;
/// Diversity-enforced parameter selection: round-robin, cooldown, rejection gate
pub mod m40_mutation_selector;
