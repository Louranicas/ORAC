//! # ORAC Sidecar
//!
//! Envoy-like proxy specialized for AI agent traffic.
//! Intelligent fleet coordination with HTTP hooks, Hebbian STDP, and RALPH evolution.
//!
//! ## Architecture (8 Layers, 40 Modules)
//!
//! | Layer | Dir | Purpose | Feature Gate |
//! |-------|-----|---------|--------------|
//! | L1 | `m1_core` | Types, errors, config, traits | — |
//! | L2 | `m2_wire` | IPC client, V2 wire protocol | — |
//! | L3 | `m3_hooks` | HTTP hook server (keystone) | `api` |
//! | L4 | `m4_intelligence` | Hebbian STDP, coupling, routing | `intelligence` |
//! | L5 | `m5_bridges` | Service bridges + `SQLite` blackboard | `bridges` |
//! | L6 | `m6_coordination` | Conductor, cascade, tick, WASM | — |
//! | L7 | `m7_monitoring` | `OpenTelemetry`, metrics, dashboard | `monitoring` |
//! | L8 | `m8_evolution` | RALPH evolution chamber | `evolution` |
//!
//! ## Port: 8133 | Binary: `orac-sidecar` | `DevEnv` Batch: 5

// L1: Foundation — no deps, always compiled
pub mod m1_core;

// L2: Wire — depends on L1
pub mod m2_wire;

// L3: Hooks (keystone) — depends on L1, L2
#[cfg(feature = "api")]
pub mod m3_hooks;

// L4: Intelligence — depends on L1, L2
#[cfg(feature = "intelligence")]
pub mod m4_intelligence;

// L5: Bridges — depends on L1
#[cfg(feature = "bridges")]
pub mod m5_bridges;

// L6: Coordination — depends on L1, L2, L4, L5
pub mod m6_coordination;

// L7: Monitoring — depends on L1, L2, L5
#[cfg(feature = "monitoring")]
pub mod m7_monitoring;

// L8: Evolution — depends on L1, L4, L5, L7
#[cfg(feature = "evolution")]
pub mod m8_evolution;
