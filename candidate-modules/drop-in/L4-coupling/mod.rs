//! # Layer 4: Coupling
//!
//! Kuramoto coupling network with Jacobi integration and auto-K scaling.
//! Depends on L1 (Foundation), L3 (Field).

pub mod m16_coupling_network;
pub mod m17_auto_k;
pub mod m18_topology;

// ── Ergonomic re-exports ──

pub use m16_coupling_network::{Connection, CouplingNetwork};
pub use m17_auto_k::{consent_gated_k_adjustment, AutoKController};
pub use m18_topology::{
    degree, least_coupled_pair, mean_coupling_weight, most_coupled_pair, neighbors,
    strongest_neighbor, topology_summary, NeighborInfo, TopologySummary,
};
