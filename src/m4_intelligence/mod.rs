//! # Layer 4: Intelligence
//!
//! Smart dispatch using Hebbian weights, content-aware routing, per-pane health gating.
//! Hot-swap M16-M21 from PV2, plus new semantic router and circuit breaker.
//!
//! ## Modules
//!
//! | Module | Name | Source | Purpose |
//! |--------|------|--------|---------|
//! | `m15` | Coupling Network | PV2 M16 | Kuramoto coupling matrix, phase dynamics |
//! | `m16` | Auto-K | PV2 M17 | Adaptive coupling strength, consent-gated |
//! | `m17` | Topology | PV2 M18 | Network topology analysis |
//! | `m18` | Hebbian STDP | PV2 M19 | LTP/LTD dynamics, tool co-activation |
//! | `m19` | Buoy Network | PV2 M20 | Health tracking, spatial recall |
//! | `m20` | Semantic Router | NEW | Content-aware dispatch, domain affinity |
//! | `m21` | Circuit Breaker | NEW | Per-pane health gating (Closed/Open/`HalfOpen`) |
//!
//! ## STDP Parameters
//!
//! - LTP: 0.01 (3x burst, 2x newcomer)
//! - LTD: 0.002
//! - Weight floor: 0.05
//! - Phase wrapping: `.rem_euclid(TAU)` after all arithmetic
//!
//! ## Design Invariants
//!
//! - Feature-gated: `#[cfg(feature = "intelligence")]`
//! - Depends on: `m1_core` (types, constants), `m2_wire` (bus events)

/// Kuramoto coupling matrix with phase dynamics (hot-swap M16)
pub mod m15_coupling_network;
/// Adaptive coupling strength with consent-gated adjustment (hot-swap M17)
pub mod m16_auto_k;
/// Network topology analysis (hot-swap M18)
pub mod m17_topology;
/// Hebbian STDP learning with LTP/LTD dynamics (hot-swap M19)
pub mod m18_hebbian_stdp;
/// Buoy health tracking and spatial recall (hot-swap M20)
pub mod m19_buoy_network;
/// Content-aware dispatch using Hebbian weights and domain affinity
pub mod m20_semantic_router;
/// Per-pane health gating with tower-resilience (Closed/Open/`HalfOpen` FSM)
pub mod m21_circuit_breaker;
