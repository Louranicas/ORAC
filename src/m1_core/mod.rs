//! # Layer 1: Core
//!
//! Foundation layer — types, errors, configuration, constants, traits, validation.
//! All other layers depend on L1. No upward imports permitted.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | `m01` | Core Types | `PaneId`, `TaskId`, `OrderParameter`, `FleetMode`, `Timestamp` |
//! | `m02` | Error Handling | Unified error enum, `ErrorClassifier` trait (`Send + Sync + Debug`) |
//! | `m03` | Config | TOML + env overlay (port, bridges, hooks, evolution) |
//! | `m04` | Constants | Thresholds, intervals, budgets, limits |
//! | `m05` | Traits | `Bridgeable` |
//! | `m06` | Validation | Input validators (persona, `tool_name`, summary, phase, body) |
//!
//! ## Design Invariants
//!
//! - Every type is `Send + Sync`
//! - No `unsafe`, no panics, no I/O in type definitions
//! - `const fn` wherever compiler allows
//! - All constructors are `#[must_use]`
//! - `Timestamp` newtype replaces `chrono`/`SystemTime`

/// Core types, identifiers, newtypes (`PaneId`, `TaskId`, `OrderParameter`, `FleetMode`)
pub mod m01_core_types;
/// Unified error enum with `ErrorClassifier` trait (`Send + Sync + Debug`)
pub mod m02_error_handling;
/// ORAC configuration from TOML + env overlay (port, bridges, hooks, evolution)
pub mod m03_config;
/// Named constants: thresholds, intervals, budgets, limits
pub mod m04_constants;
/// Core traits (`Bridgeable`)
pub mod m05_traits;
/// Input validators for persona, `tool_name`, summary, frequency, phase, body
pub mod m06_validation;
/// ORAC field state types (`AppState`, `SharedState`, `FieldState`, `FieldDecision`)
pub mod field_state;
