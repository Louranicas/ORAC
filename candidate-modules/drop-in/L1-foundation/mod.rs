//! # Layer 1: Foundation
//!
//! Core types, error handling, and configuration. No dependencies on other layers.
//! This is the bedrock -- every other module in `Pane-Vortex` V2 depends on L1.
//!
//! ## Modules
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`m01_core_types`] | `PaneId`, `TaskId`, `Point3D`, `SphereMemory`, `Buoy`, `OrderParameter`, `FleetMode` |
//! | [`m02_error_handling`] | `PvError` enum, `PvResult` alias, `ErrorClassifier` trait |
//! | [`m03_config`] | Figment-based config loading with TOML + `PV2_*` env overlay |
//! | [`m04_constants`] | All compile-time magic numbers as named constants |
//! | [`m05_traits`] | Core traits: `Oscillator`, `Learnable`, `Bridgeable`, `Consentable`, `Persistable`, `FieldObserver` |
//! | [`m06_validation`] | Input validation: phase wrapping, frequency clamp, string limits |

pub mod m01_core_types;
pub mod m02_error_handling;
pub mod m03_config;
pub mod m04_constants;
pub mod m05_traits;
pub mod m06_validation;

// ── Ergonomic re-exports ──

// Types
pub use m01_core_types::{
    BridgeAdjustments, BridgeStaleness, Buoy, DecisionRecord, FieldAction, FleetMode, GhostTrace,
    InboxMessage, OrderParameter, PaneId, PaneStatus, Point3D, RTrend, SphereFieldContext,
    SphereMemory, TaskId, WorkSignature,
};

// Utilities
pub use m01_core_types::{now_secs, phase_diff, semantic_phase_region};

// Errors
pub use m02_error_handling::{ErrorClassifier, ErrorSeverity, PvError, PvResult};

// Config
pub use m03_config::PvConfig;

// Traits
pub use m05_traits::{
    Bridgeable, ConsentPosture, Consentable, FieldObserver, Learnable, Oscillator, Persistable,
};
