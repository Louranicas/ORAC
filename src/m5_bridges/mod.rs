//! # Layer 5: Bridges
//!
//! Direct service communication — thermal read, fitness signal, memory hydration, TSV persistence.
//! Adapt M22, M24-M26 from PV2. New blackboard for shared fleet state.
//!
//! ## Modules
//!
//! | Module | Name | Service | Port | Protocol |
//! |--------|------|---------|------|----------|
//! | `m22` | SYNTHEX Bridge | SYNTHEX | 8090 | HTTP (`/v3/thermal`) |
//! | `m23` | ME Bridge | Maintenance Engine | 8080 | HTTP (`/api/observer`) |
//! | `m24` | POVM Bridge | POVM Engine | 8125 | HTTP (`/hydrate`, `/memories`) |
//! | `m25` | RM Bridge | Reasoning Memory | 8130 | **TSV** (`POST /put`, `GET /search`) |
//! | `m26` | Blackboard | Local `SQLite` | — | `rusqlite` |
//!
//! ## Critical Rules
//!
//! - **RM is TSV only** — JSON causes parse failure (A12)
//! - **POVM is write-only** — must call `/hydrate` to read back (BUG-034)
//! - **Bridge URLs: raw `SocketAddr`** — no `http://` prefix (BUG-033)
//! - All bridges include `_consent_check()` stub
//!
//! ## Design Invariants
//!
//! - Feature-gated: `#[cfg(feature = "bridges")]`
//! - Depends on: `m1_core` (types, traits)
//! - Prerequisite: `devenv start` before bridge tests

/// Shared raw TCP HTTP helpers (BUG-042: extracted from M22-M25)
pub mod http_helpers;
/// SYNTHEX bridge — thermal read + Hebbian writeback (adapt M22)
pub mod m22_synthex_bridge;
/// ME bridge — fitness signal read + frozen detection (adapt M24)
pub mod m23_me_bridge;
/// POVM bridge — memory hydration + crystallisation (adapt M25)
pub mod m24_povm_bridge;
/// RM bridge — cross-session **TSV** persistence, NOT JSON (adapt M26)
pub mod m25_rm_bridge;
/// SQLite shared fleet state — pane status, task history, agent cards
pub mod m26_blackboard;
