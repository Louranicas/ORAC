# L7 Monitoring — Observability Layer

> OpenTelemetry integration, Prometheus metrics, Kuramoto field dashboard, token accounting.

## Feature Gate

`monitoring`

## Modules

| Module | File | Description | Test Kind |
|--------|------|-------------|-----------|
| m32_otel_traces | `src/m7_monitoring/m32_otel_traces.rs` | OpenTelemetry trace export for task lifecycle across panes, context propagation | integration |
| m33_metrics_export | `src/m7_monitoring/m33_metrics_export.rs` | Prometheus-compatible counters/gauges/histograms, `/metrics` endpoint | unit + integration |
| m34_field_dashboard | `src/m7_monitoring/m34_field_dashboard.rs` | Kuramoto field metrics: per-cluster r, phase gaps, K effective, chimera state | unit |
| m35_token_accounting | `src/m7_monitoring/m35_token_accounting.rs` | Per-task token cost tracking, per-agent usage, fleet budget management | unit + property |

## Metrics Exported

- `orac_hook_latency_ms` — per-hook response time histogram
- `orac_field_order_param` — Kuramoto r gauge
- `orac_k_effective` — effective coupling strength gauge
- `orac_pane_circuit_state` — per-pane circuit breaker state
- `orac_tokens_total` — cumulative token usage counter

## Dependencies

- **L1 Core** — `OracError`, `PaneId`, `Timestamp`
- **L2 Wire** — event subscription for metric updates
- **L5 Bridges** — bridge health status for dashboard

## Design Constraints

- Metrics endpoint at `/metrics` (Prometheus text format)
- Token counters are `AtomicU64` — no lock contention on the hot path
- Dashboard data (m34) is computed on-demand, not cached (avoids staleness)
- OTel trace context propagates through all L3 HTTP handlers
- Histogram buckets: [1, 5, 10, 25, 50, 100, 250, 500, 1000] ms
- Feature-gated optional deps: `opentelemetry`, `opentelemetry-otlp`

## Hot-Swap Source

- ALL NEW (no PV2 equivalent)

## Cross-References

- [[Session 045 Arena — 12-live-field-analysis]]
- [[ULTRAPLATE Metabolic Activation Plan 2026-03-07]]
- ORAC_PLAN.md §Phase 3 Detail (steps 2-5)
- ORAC_MINDMAP.md §Branch 5 (Monitoring / Observer)
