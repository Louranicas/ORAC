//! # Layer 7: Monitoring
//!
//! Observability layer — task lifecycle traces, per-agent metrics, field state dashboard.
//!
//! ## Modules
//!
//! | Module | Name | Purpose |
//! |--------|------|---------|
//! | `m32` | `OTel` Traces | `OpenTelemetry` trace export for task lifecycle across panes |
//! | `m33` | Metrics Export | Prometheus-compatible metrics (tokens, latency, error rate) |
//! | `m34` | Field Dashboard | Kuramoto field metrics: per-cluster r, phase gaps, K effective |
//! | `m35` | Token Accounting | Per-task token cost tracking and fleet budget management |
//!
//! ## Metrics Exported
//!
//! - `orac_hook_latency_ms` — per-hook response time histogram
//! - `orac_field_order_param` — Kuramoto r gauge
//! - `orac_k_effective` — effective coupling strength gauge
//! - `orac_pane_circuit_state` — per-pane circuit breaker state
//! - `orac_tokens_total` — cumulative token usage counter
//!
//! ## Design Invariants
//!
//! - Feature-gated: `#[cfg(feature = "monitoring")]`
//! - Depends on: `m1_core`, `m2_wire`, `m5_bridges`

/// `OpenTelemetry` trace export for task lifecycle across panes
pub mod m32_otel_traces;
/// Prometheus-compatible metrics (per-agent tokens, latency, error rate)
pub mod m33_metrics_export;
/// Kuramoto field metrics: per-cluster r, phase gaps, K effective
pub mod m34_field_dashboard;
/// Per-task token cost tracking and fleet budget management
pub mod m35_token_accounting;
