//! # Layer 3: Hooks — THE KEYSTONE
//!
//! HTTP hook server replacing all 8 bash hook scripts with sub-ms endpoints.
//! Claude Code sends HTTP POST to `http://localhost:8133/hooks/{event}`.
//! ORAC processes the hook, interacts with PV2 via IPC, and returns a decision.
//!
//! ## Modules
//!
//! | Module | Name | Endpoint | Side Effects |
//! |--------|------|----------|--------------|
//! | `m10` | Hook Server | `:8133/hooks/*` | Axum router, state injection |
//! | `m11` | Session Hooks | `SessionStart`, `Stop` | Sphere register/deregister, quality gate |
//! | `m12` | Tool Hooks | `PostToolUse`, `PreToolUse` | Hebbian STDP, thermal gate |
//! | `m13` | Prompt Hooks | `UserPromptSubmit` | Field state injection into context |
//! | `m14` | Permission Policy | `PermissionRequest` | Auto-approve/deny (fleet + per-sphere) |
//!
//! ## Hook Flow
//!
//! ```text
//! Claude Code → POST :8133/hooks/{event} → ORAC handler
//!   → IPC to PV2 (register, update, query)
//!   → Return { decision, reason?, inject? }
//! ```
//!
//! ## Design Invariants
//!
//! - All handlers return within 1ms (local memory, no external I/O in hot path)
//! - Feature-gated: `#[cfg(feature = "api")]`
//! - Depends on: `m1_core` (types, errors, config), `m2_wire` (IPC client)

/// Axum HTTP server on `:8133` routing 6 hook endpoints
pub mod m10_hook_server;
/// `SessionStart` (register sphere) and `Stop` (quality gate + deregister) handlers
pub mod m11_session_hooks;
/// `PostToolUse` (Hebbian + task poll) and `PreToolUse` (thermal gate) handlers
pub mod m12_tool_hooks;
/// `UserPromptSubmit` (inject field state) handler
pub mod m13_prompt_hooks;
/// `PermissionRequest` auto-approve/deny policy engine (fleet-wide + per-sphere)
pub mod m14_permission_policy;
