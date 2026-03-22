//! # Layer 5: Learning
//!
//! Hebbian STDP, buoy network analysis, and fleet-level memory management.
//! Depends on L1 (Foundation), L3 (Field), L4 (Coupling).

pub mod m19_hebbian_stdp;
pub mod m20_buoy_network;
pub mod m21_memory_manager;

// ── Ergonomic re-exports ──

pub use m19_hebbian_stdp::{apply_stdp, are_coactive, compute_ltp_rate, decay_all_weights, StdpResult};
pub use m20_buoy_network::{buoy_centroid, buoy_health, fleet_buoy_stats, nearest_buoy, BuoyHealth, FleetBuoyStats};
pub use m21_memory_manager::{
    fleet_memory_stats, memory_age_distribution, shared_memories, sphere_top_tools,
    tool_frequency, FleetMemoryStats, MemoryAgeDistribution,
};
