//! # Layer 6: Coordination
//!
//! Coordination substrate — conductor breathing, cascade handoffs, tick loop,
//! WASM plugin bridge, memory aggregation.
//!
//! ## Modules
//!
//! | Module | Name | Source | Purpose |
//! |--------|------|--------|---------|
//! | `m27` | Conductor | PV2 M31 (adapt) | PI controller for field breathing rhythm |
//! | `m28` | Cascade | PV2 M33 (drop-in) | Cascade handoff with sphere mitosis (SYS-1) |
//! | `m29` | Tick | PV2 M35 (adapt) | Tick orchestrator, Hebbian Phase 2.5 wiring |
//! | `m30` | WASM Bridge | NEW | FIFO/ring protocol to swarm-orchestrator WASM |
//! | `m31` | Memory Manager | PV2 M21 (drop-in) | Memory aggregation and pruning |
//!
//! ## WASM Bridge Protocol
//!
//! ```text
//! WASM plugin → /tmp/swarm-commands.pipe (FIFO)  → ORAC
//! ORAC        → /tmp/swarm-events.jsonl  (ring)  → WASM plugin
//!              (1000 line cap, oldest dropped)
//! ```
//!
//! ## Design Invariants
//!
//! - Depends on: `m1_core`, `m2_wire`, `m4_intelligence`, `m5_bridges`
//! - Lock ordering: `AppState` before `BusState` (deadlock prevention)
//! - Memory pruning: activation < 0.05 every 200 steps, cap 500/sphere

/// PI controller for field breathing rhythm (adapt M31)
pub mod m27_conductor;
/// Cascade handoff protocol with sphere mitosis (hot-swap M33)
pub mod m28_cascade;
/// Tick orchestrator with Hebbian Phase 2.5 wiring (adapt M35)
pub mod m29_tick;
/// FIFO/ring protocol bridge to Zellij swarm-orchestrator WASM plugin
pub mod m30_wasm_bridge;
/// Memory aggregation and pruning (hot-swap M21)
pub mod m31_memory_manager;
