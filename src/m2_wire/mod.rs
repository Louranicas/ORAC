//! Wire layer — IPC client, bus types, V2 wire protocol
//!
//! Depends on: `L1`

/// Unix socket client connecting to PV2 daemon bus (V2 wire protocol)
pub mod m07_ipc_client;
/// Bus frame types, task lifecycle, event subscription (NDJSON)
pub mod m08_bus_types;
/// V2 wire format: handshake, subscribe, frames, keepalive, V1 compat
pub mod m09_wire_protocol;
