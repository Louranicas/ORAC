---
title: "Layer 7: Monitoring — Module Documentation"
date: 2026-03-22
tags: [modules, monitoring, L7, orac-sidecar]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L7
modules: [m32, m33, m34, m35]
---

# Layer 7: Monitoring (m32-m35)

> Observability layer -- OpenTelemetry traces, Prometheus metrics, Kuramoto field dashboard, token accounting.
> **Target LOC:** ~1,500 | **Target tests:** 50+
> **Source:** ALL NEW (no PV2 equivalent) | **Phase:** 3
> **Feature gate:** `monitoring`

---

## Overview

L7 provides full observability for the ORAC sidecar: distributed tracing across panes (m32), Prometheus-compatible metric export (m33), a Kuramoto field dashboard computed on-demand (m34), and per-task token cost tracking (m35). All modules are feature-gated under `#[cfg(feature = "monitoring")]` with optional dependencies on `opentelemetry` and `opentelemetry-otlp`.

### Design Constraints

- Metrics endpoint at `/metrics` (Prometheus text format 0.0.4)
- Token counters are `AtomicU64` -- zero lock contention on the hot path
- Dashboard data (m34) is computed on-demand, not cached (avoids staleness; AP19 lesson)
- OTel trace context propagates through all L3 HTTP hook handlers
- Histogram buckets: `[1, 5, 10, 25, 50, 100, 250, 500, 1000]` ms
- Feature-gated optional deps: `opentelemetry`, `opentelemetry-otlp`
- All metric names prefixed `orac_` for namespace isolation
- Never block the tick loop with metric export (fire-and-forget pattern P05)

### Dependencies

- **L1 Core** -- `OracError`, `PaneId`, `Timestamp`
- **L2 Wire** -- event subscription for metric updates
- **L5 Bridges** -- bridge health status for dashboard

### Metrics Exported

| Metric | Type | Module | Description |
|--------|------|--------|-------------|
| `orac_hook_latency_ms` | Histogram | m33 | Per-hook response time |
| `orac_field_order_param` | Gauge | m33/m34 | Kuramoto order parameter r |
| `orac_k_effective` | Gauge | m33/m34 | Effective coupling strength K |
| `orac_pane_circuit_state` | Gauge | m33 | Per-pane circuit breaker state (0=Closed, 1=HalfOpen, 2=Open) |
| `orac_tokens_total` | Counter | m35 | Cumulative token usage |

---

## m32 -- OTel Traces

**Source:** `src/m7_monitoring/m32_otel_traces.rs`
**LOC Target:** ~350
**Depends on:** `m01_core_types`, `m02_error_handling`, `m10_hook_server`

### Design Decisions

- Uses `opentelemetry-otlp` for OTLP/gRPC export to any OTel collector
- Trace context propagated via `traceparent` HTTP header through hook handlers
- Each hook invocation creates a span with hook type, pane ID, and latency
- Task lifecycle spans: `task.create` -> `task.assign` -> `task.complete` across panes
- Graceful degradation: if no collector is configured, tracing is a no-op (not an error)
- Batch export with 5-second flush interval to avoid per-span overhead

### Types to Implement

```rust
use opentelemetry::trace::{Tracer, SpanKind};
use opentelemetry_sdk::trace::TracerProvider;

/// `OpenTelemetry` tracer configuration for ORAC.
///
/// Wraps the `TracerProvider` and manages lifecycle (init/shutdown).
/// Feature-gated: `#[cfg(feature = "monitoring")]`.
pub struct OracTracer {
    /// The underlying `OpenTelemetry` tracer provider.
    provider: Option<TracerProvider>,
    /// Service name reported to the collector.
    service_name: String,
    /// Collector endpoint (e.g., `http://localhost:4317`).
    endpoint: String,
}

/// Span context extracted from an inbound hook request.
///
/// Carries `trace_id` and `span_id` for distributed correlation.
#[derive(Debug, Clone)]
pub struct TraceContext {
    /// W3C `traceparent` trace ID (32 hex chars).
    pub trace_id: String,
    /// W3C `traceparent` span ID (16 hex chars).
    pub span_id: String,
    /// Trace flags (sampled, etc.).
    pub flags: u8,
}

/// A recorded span for a hook invocation.
#[derive(Debug, Clone)]
pub struct HookSpan {
    /// Which hook was invoked.
    pub hook_type: String,
    /// The pane that triggered the hook.
    pub pane_id: PaneId,
    /// Latency in microseconds.
    pub latency_us: u64,
    /// Whether the hook succeeded.
    pub success: bool,
    /// Parent context for distributed tracing.
    pub parent: Option<TraceContext>,
}
```

### Key Functions

- `init_tracer(config: &OracTracerConfig) -> Result<OracTracer, OracError>` -- Initialise the OTel tracer provider with OTLP exporter. Returns `OracError::TracerInit` on failure.
- `extract_context(headers: &HeaderMap) -> Option<TraceContext>` -- Extract W3C `traceparent` from HTTP headers.
- `start_hook_span(tracer: &OracTracer, hook_type: &str, pane_id: &PaneId, parent: Option<&TraceContext>) -> SpanGuard` -- Begin a span for a hook invocation.
- `record_task_span(tracer: &OracTracer, task_id: &TaskId, phase: &str, pane_id: &PaneId) -> Result<(), OracError>` -- Record a task lifecycle event (create, assign, complete).
- `shutdown_tracer(tracer: &mut OracTracer) -> Result<(), OracError>` -- Flush pending spans and shut down the provider.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_extract_valid_traceparent` | unit | Parse valid W3C traceparent header |
| `test_extract_missing_header` | unit | Returns `None` for missing header |
| `test_extract_malformed_traceparent` | unit | Returns `None` for invalid format |
| `test_hook_span_records_latency` | unit | Span captures latency in microseconds |
| `test_tracer_init_no_collector` | integration | Graceful no-op when collector unreachable |
| `test_task_span_lifecycle` | integration | Create -> assign -> complete chain |
| `test_shutdown_flushes_pending` | integration | Pending spans flushed on shutdown |

### Cross-References

- `m10_hook_server` -- spans created per hook invocation
- `m07_ipc_client` -- task lifecycle events trigger task spans
- ORAC_PLAN.md Phase 3 Detail (step 2)

---

## m33 -- Metrics Export

**Source:** `src/m7_monitoring/m33_metrics_export.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types`, `m02_error_handling`

### Design Decisions

- Prometheus text format 0.0.4 at `/metrics` endpoint
- All counters are `AtomicU64` -- zero lock contention on concurrent hook invocations
- Histograms use pre-defined bucket boundaries: `[1, 5, 10, 25, 50, 100, 250, 500, 1000]` ms
- Gauges for floating-point values (r, K) use `AtomicU64` with `f64::to_bits()`/`f64::from_bits()`
- Metric registration is static (all metrics known at compile time, no dynamic creation)
- Thread-safe: all types are `Send + Sync` for `Arc<dyn MetricsRegistry>`
- Export is synchronous text rendering -- no async overhead for the scrape path

### Types to Implement

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// Histogram bucket boundaries in milliseconds.
///
/// Fixed at `[1, 5, 10, 25, 50, 100, 250, 500, 1000]`.
pub const HISTOGRAM_BUCKETS: [u64; 9] = [1, 5, 10, 25, 50, 100, 250, 500, 1000];

/// A lock-free counter backed by `AtomicU64`.
///
/// Thread-safe increment with `Relaxed` ordering (sufficient for counters).
#[derive(Debug)]
pub struct AtomicCounter {
    /// The underlying atomic value.
    value: AtomicU64,
    /// Prometheus metric name (e.g., `orac_tokens_total`).
    name: &'static str,
    /// Prometheus HELP string.
    help: &'static str,
}

/// A lock-free gauge for floating-point values.
///
/// Stores `f64` as `u64` bits via `to_bits()`/`from_bits()`.
#[derive(Debug)]
pub struct AtomicGauge {
    /// f64 stored as u64 bits.
    bits: AtomicU64,
    /// Prometheus metric name.
    name: &'static str,
    /// Prometheus HELP string.
    help: &'static str,
}

/// A histogram with fixed bucket boundaries.
///
/// Each bucket is an `AtomicU64` counter. Thread-safe observation.
#[derive(Debug)]
pub struct AtomicHistogram {
    /// Per-bucket counters (len = `HISTOGRAM_BUCKETS.len()` + 1 for +Inf).
    buckets: Vec<AtomicU64>,
    /// Sum of all observed values (stored as f64 bits).
    sum_bits: AtomicU64,
    /// Total observation count.
    count: AtomicU64,
    /// Prometheus metric name.
    name: &'static str,
    /// Prometheus HELP string.
    help: &'static str,
}

/// Central metrics registry.
///
/// All ORAC metrics live here. Shared via `Arc<MetricsRegistry>`.
/// Thread-safe: all fields are atomic or immutable.
pub struct MetricsRegistry {
    /// Per-hook response time histogram.
    pub hook_latency: AtomicHistogram,
    /// Kuramoto order parameter r.
    pub field_order_param: AtomicGauge,
    /// Effective coupling strength K.
    pub k_effective: AtomicGauge,
    /// Per-pane circuit breaker state.
    /// Key: `PaneId` string, Value: state enum as u64.
    pub pane_circuit_states: parking_lot::RwLock<HashMap<String, AtomicU64>>,
    /// Cumulative token usage.
    pub tokens_total: AtomicCounter,
}
```

### Key Functions

- `MetricsRegistry::new() -> Self` -- Construct with all metrics initialized to zero.
- `AtomicCounter::inc(&self)` -- Increment counter by 1 (`Relaxed` ordering).
- `AtomicCounter::inc_by(&self, n: u64)` -- Increment counter by `n`.
- `AtomicGauge::set(&self, value: f64)` -- Store `f64` as `u64` bits.
- `AtomicGauge::get(&self) -> f64` -- Load and convert back to `f64`.
- `AtomicHistogram::observe(&self, value_ms: u64)` -- Increment the appropriate bucket and update sum/count.
- `render_prometheus(registry: &MetricsRegistry) -> String` -- Render all metrics as Prometheus text format 0.0.4.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_counter_inc` | unit | Counter increments atomically |
| `test_counter_inc_by` | unit | Counter increments by arbitrary amount |
| `test_gauge_set_get_roundtrip` | unit | f64 survives `to_bits`/`from_bits` |
| `test_gauge_nan_handling` | unit | NaN stored and retrieved correctly |
| `test_gauge_negative_values` | unit | Negative f64 stored correctly |
| `test_histogram_bucket_assignment` | unit | Values land in correct bucket |
| `test_histogram_overflow_bucket` | unit | Values above max bucket go to +Inf |
| `test_histogram_sum_and_count` | unit | Sum and count updated on observe |
| `test_render_prometheus_format` | unit | Output matches Prometheus text spec |
| `test_render_empty_registry` | unit | Empty registry renders valid output |
| `test_concurrent_counter_inc` | integration | 100 threads, 1000 increments each = 100,000 |
| `test_concurrent_histogram_observe` | integration | Multi-threaded observe produces correct count |

### Cross-References

- `m10_hook_server` -- `/metrics` route handler calls `render_prometheus()`
- `m34_field_dashboard` -- dashboard reads from same `MetricsRegistry`
- `m35_token_accounting` -- updates `tokens_total` counter
- ORAC_PLAN.md Phase 3 Detail (step 3)
- GOLD_STANDARD_PATTERNS.md P5 (FMA for float precision)

---

## m34 -- Field Dashboard

**Source:** `src/m7_monitoring/m34_field_dashboard.rs`
**LOC Target:** ~400
**Depends on:** `m01_core_types`, `m15_coupling_network`, `m33_metrics_export`

### Design Decisions

- Dashboard data computed on-demand per request (not cached -- avoids staleness trap from AP19)
- Reads directly from `SharedState` and coupling network -- no intermediate cache layer
- Cluster detection via phase gap analysis (gap > pi/3 = cluster boundary)
- Chimera state detection: clusters with r disparity > 0.15 between any two clusters
- All float arithmetic uses FMA (pattern P05)
- JSON response format for programmatic consumption; no HTML rendering
- Integrates with `MetricsRegistry` to update gauges after each computation

### Types to Implement

```rust
use std::f64::consts::PI;

/// Kuramoto field dashboard snapshot.
///
/// Computed on-demand from live state. Never cached.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FieldDashboard {
    /// Global order parameter r in [0.0, 1.0].
    pub r_global: f64,
    /// Effective coupling strength K.
    pub k_effective: f64,
    /// Per-cluster breakdown.
    pub clusters: Vec<ClusterMetrics>,
    /// Whether a chimera state is detected.
    pub chimera_detected: bool,
    /// Phase distribution histogram (12 bins of pi/6 each).
    pub phase_histogram: [u32; 12],
    /// Total number of registered spheres.
    pub sphere_count: u32,
    /// Timestamp of this snapshot.
    pub computed_at: Timestamp,
}

/// Metrics for a single synchronization cluster.
///
/// A cluster is a group of spheres with phase gaps < pi/3.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClusterMetrics {
    /// Cluster index (0-based).
    pub id: u32,
    /// Order parameter within this cluster.
    pub r_local: f64,
    /// Mean phase of the cluster in radians.
    pub mean_phase: f64,
    /// Number of spheres in this cluster.
    pub sphere_count: u32,
    /// Sphere IDs in this cluster.
    pub members: Vec<PaneId>,
}

/// Phase gap between adjacent spheres (sorted by phase).
///
/// Used for cluster boundary detection.
#[derive(Debug, Clone)]
pub struct PhaseGap {
    /// The sphere before the gap.
    pub sphere_a: PaneId,
    /// The sphere after the gap.
    pub sphere_b: PaneId,
    /// Gap size in radians.
    pub gap_rad: f64,
}

/// Configuration for dashboard computation.
pub struct DashboardConfig {
    /// Phase gap threshold for cluster boundary (default: pi/3).
    pub cluster_gap_threshold: f64,
    /// Chimera detection threshold for r disparity (default: 0.15).
    pub chimera_r_disparity: f64,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            cluster_gap_threshold: PI / 3.0,
            chimera_r_disparity: 0.15,
        }
    }
}
```

### Key Functions

- `compute_dashboard(state: &SharedState, config: &DashboardConfig) -> Result<FieldDashboard, OracError>` -- Compute full dashboard from live state. On-demand, never cached.
- `detect_clusters(phases: &[(PaneId, f64)], gap_threshold: f64) -> Vec<ClusterMetrics>` -- Sort phases, find gaps > threshold, group into clusters.
- `compute_phase_gaps(phases: &[(PaneId, f64)]) -> Vec<PhaseGap>` -- Compute pairwise phase gaps between adjacent sorted spheres.
- `detect_chimera(clusters: &[ClusterMetrics], disparity: f64) -> bool` -- Returns true if any two clusters have r disparity exceeding threshold.
- `compute_phase_histogram(phases: &[f64]) -> [u32; 12]` -- Bin phases into 12 sectors of pi/6 each.
- `update_dashboard_gauges(registry: &MetricsRegistry, dashboard: &FieldDashboard)` -- Push r and K to Prometheus gauges after dashboard computation.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_single_sphere_dashboard` | unit | One sphere: r=1.0, one cluster |
| `test_two_synced_spheres` | unit | Phases within pi/6: one cluster, r near 1.0 |
| `test_two_opposed_spheres` | unit | Phases pi apart: two clusters, chimera detected |
| `test_cluster_detection_gap_threshold` | unit | Gap > pi/3 creates cluster boundary |
| `test_no_chimera_uniform_clusters` | unit | Equal-r clusters: chimera=false |
| `test_chimera_r_disparity` | unit | r disparity > 0.15: chimera=true |
| `test_phase_histogram_uniform` | unit | 12 uniformly spaced phases: 1 per bin |
| `test_phase_histogram_concentrated` | unit | All phases near 0: first bin gets all |
| `test_empty_field_dashboard` | unit | Zero spheres: r=0, no clusters |
| `test_dashboard_gauge_update` | unit | Gauges reflect computed r and K |
| `test_cluster_members_correct` | unit | Each sphere appears in exactly one cluster |
| `test_fma_precision` | unit | FMA computation matches expected to 1e-12 |

### Cross-References

- `m15_coupling_network` -- reads coupling matrix for K effective
- `m33_metrics_export` -- updates gauges via `update_dashboard_gauges()`
- `m10_hook_server` -- `/dashboard` route handler
- ORAC_MINDMAP.md Branch 5 (Monitoring / Observer)
- Pane-vortex v1 Session 017: "Synchronization without differentiation = conformity"

---

## m35 -- Token Accounting

**Source:** `src/m7_monitoring/m35_token_accounting.rs`
**LOC Target:** ~350
**Depends on:** `m01_core_types`, `m02_error_handling`, `m33_metrics_export`

### Design Decisions

- Token counts are `AtomicU64` -- no lock contention on the PostToolUse hot path
- Per-agent tracking uses `parking_lot::RwLock<HashMap<PaneId, AgentTokens>>` -- read-heavy workload
- Fleet budget is a soft cap with warning threshold (80%) and hard cap (100%)
- Budget enforcement is advisory only -- ORAC logs warnings but does not block tool use
- Token counts are cumulative within a session (reset on `SessionStart`)
- Property-based testing: token invariant = sum of per-task tokens equals per-agent total

### Types to Implement

```rust
use std::sync::atomic::{AtomicU64, Ordering};

/// Per-agent token usage tracking.
///
/// Tracks input tokens, output tokens, and cost estimate per agent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AgentTokens {
    /// Agent (pane) identifier.
    pub pane_id: PaneId,
    /// Cumulative input tokens this session.
    pub input_tokens: u64,
    /// Cumulative output tokens this session.
    pub output_tokens: u64,
    /// Number of tool invocations.
    pub tool_calls: u64,
    /// Estimated cost in millicents (USD * 100,000).
    pub cost_millicents: u64,
    /// Session start timestamp.
    pub session_start: Timestamp,
}

/// Per-task token record.
///
/// Recorded on each `PostToolUse` hook invocation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TaskTokenRecord {
    /// The task that consumed tokens.
    pub task_id: TaskId,
    /// The agent that executed the task.
    pub pane_id: PaneId,
    /// Input tokens for this invocation.
    pub input_tokens: u64,
    /// Output tokens for this invocation.
    pub output_tokens: u64,
    /// Tool name that was invoked.
    pub tool_name: String,
    /// Timestamp of the invocation.
    pub timestamp: Timestamp,
}

/// Fleet-wide token budget.
///
/// Soft cap at 80%, hard cap at 100%. Advisory only (does not block).
#[derive(Debug, Clone, serde::Serialize)]
pub struct FleetBudget {
    /// Maximum tokens across all agents (0 = unlimited).
    pub max_tokens: u64,
    /// Current total across all agents.
    pub current_total: u64,
    /// Warning threshold as fraction (default: 0.8).
    pub warning_threshold: f64,
    /// Whether the warning threshold has been breached.
    pub warning_triggered: bool,
    /// Whether the hard cap has been breached.
    pub hard_cap_breached: bool,
}

/// Token accounting service.
///
/// Thread-safe: `AtomicU64` for global counter, `RwLock` for per-agent map.
pub struct TokenAccounting {
    /// Global cumulative token counter (input + output).
    global_total: AtomicU64,
    /// Per-agent token tracking.
    agents: parking_lot::RwLock<HashMap<PaneId, AgentTokens>>,
    /// Per-task token records (ring buffer, cap 10,000).
    records: parking_lot::RwLock<VecDeque<TaskTokenRecord>>,
    /// Fleet budget configuration.
    budget: parking_lot::RwLock<FleetBudget>,
    /// Reference to metrics registry for `orac_tokens_total`.
    metrics: Arc<MetricsRegistry>,
}
```

### Key Functions

- `TokenAccounting::new(metrics: Arc<MetricsRegistry>, max_tokens: u64) -> Self` -- Construct with budget and metrics reference.
- `record_usage(&self, record: TaskTokenRecord) -> Result<(), OracError>` -- Record a tool invocation's token usage. Updates per-agent, global counter, metrics, and budget state.
- `get_agent_tokens(&self, pane_id: &PaneId) -> Option<AgentTokens>` -- Get per-agent token summary (owned clone, P07).
- `get_fleet_summary(&self) -> FleetBudget` -- Get current fleet budget state.
- `reset_agent(&self, pane_id: &PaneId)` -- Reset agent counters on `SessionStart`.
- `check_budget(&self) -> BudgetStatus` -- Check if warning or hard cap is breached.
- `top_consumers(&self, n: usize) -> Vec<AgentTokens>` -- Return top N agents by total tokens.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_record_updates_global_counter` | unit | Global counter increments on record |
| `test_record_updates_agent_tokens` | unit | Per-agent input/output tracked correctly |
| `test_record_updates_metrics` | unit | `orac_tokens_total` gauge updated |
| `test_budget_warning_threshold` | unit | Warning triggers at 80% of max |
| `test_budget_hard_cap` | unit | Hard cap flag set at 100% |
| `test_budget_unlimited` | unit | max_tokens=0 never triggers budget |
| `test_reset_agent_clears_counts` | unit | SessionStart resets to zero |
| `test_top_consumers_ordering` | unit | Sorted by total tokens descending |
| `test_record_ring_buffer_cap` | unit | Records capped at 10,000 |
| `test_concurrent_recording` | integration | 10 agents, 100 records each = correct total |
| `test_token_invariant` | property | Sum of per-task tokens == per-agent total |
| `test_get_agent_returns_owned` | unit | Returned `AgentTokens` is a clone (P07) |

### Cross-References

- `m12_tool_hooks` -- `PostToolUse` handler calls `record_usage()`
- `m11_session_hooks` -- `SessionStart` handler calls `reset_agent()`
- `m33_metrics_export` -- updates `orac_tokens_total` counter
- `m25_rm_bridge` -- token summaries can be persisted to Reasoning Memory (TSV)
- ORAC_PLAN.md Tier 2 Feature 17 (Token accounting)

---

## Cross-References (Layer-Wide)

- [layers/L7_MONITORING.md](../layers/L7_MONITORING.md) -- Layer overview
- [modules/INDEX.md](INDEX.md) -- Module index (m32-m35)
- [GOLD_STANDARD_PATTERNS.md](../GOLD_STANDARD_PATTERNS.md) -- P05 (FMA), P07 (owned returns), P10 (feature gates)
- [ANTI_PATTERNS.md](../ANTI_PATTERNS.md) -- A04 (no println in daemons), A15 (bounded channels)
- ORAC_PLAN.md Phase 3 Detail (steps 2-5)
- ORAC_MINDMAP.md Branch 5 (Monitoring / Observer)
- Obsidian: `[[Session 045 Arena -- 12-live-field-analysis]]`
- Obsidian: `[[ULTRAPLATE Metabolic Activation Plan 2026-03-07]]`
