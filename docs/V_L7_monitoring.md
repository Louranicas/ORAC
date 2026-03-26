# V_L7: Layer 7 Monitoring — Verification Report

> **Generated:** 2026-03-25 | **Source:** m32-m35 actual source code vs D7_MODULE_PURPOSE_GUIDE.md
> **Verdict:** 1 naming discrepancy in D7 guide, all constants verified correct

---

## 1. Token Budget Defaults

**Status: VERIFIED**

| Constant | D7 Guide | Source (m35:41-44) | Match |
|----------|----------|-------------------|-------|
| `DEFAULT_SOFT_LIMIT` | 10 (USD) | `10.0` | YES |
| `DEFAULT_HARD_LIMIT` | 50 (USD) | `50.0` | YES |
| `DEFAULT_INPUT_COST` | — | `0.000_015` ($/token) | not in D7 |
| `DEFAULT_OUTPUT_COST` | — | `0.000_075` ($/token) | not in D7 |
| `MAX_TRACKED_PANES` | — | `256` | not in D7 |
| `MAX_TASK_RECORDS` | — | `5_000` | not in D7 |

**`BudgetConfig::new()` returns:** `{ soft: 10.0, hard: 50.0, input_cost: 0.000015, output_cost: 0.000075 }`

**`BudgetConfig::with_limits(soft, hard)`:** clamps `soft >= 0.0`, then `hard >= soft`.

### BudgetStatus Enum — DISCREPANCY FOUND

| D7 Guide | Actual Source (m35:129-136) |
|----------|---------------------------|
| `BudgetStatus::OverBudget` | `BudgetStatus::Exceeded` |

The D7 guide (line 709) says `BudgetStatus: Ok, Warning, OverBudget` but the actual variant
is **`Exceeded`**, not `OverBudget`. The `allows_work()` method returns `true` for `Ok` and
`Warning`, `false` for `Exceeded`.

---

## 2. R_HISTORY_MAX

**Status: VERIFIED**

| Location | Value |
|----------|-------|
| `m04_constants.rs:141` | `pub const R_HISTORY_MAX: usize = 60;` |
| `m34_field_dashboard.rs:33` | `const R_HISTORY_MAX: usize = m04_constants::R_HISTORY_MAX;` |
| Test `r_history_max_matches_core` (m34:819) | asserts equality |
| Test `dashboard_r_history_caps_at_max` (m34:558) | fills R_HISTORY_MAX+10, checks len == R_HISTORY_MAX |

The ring buffer uses `Vec<f64>` with manual eviction (`remove(0)`) when `len >= R_HISTORY_MAX`.
`Vec::with_capacity(R_HISTORY_MAX)` is used at construction (m34:172).

### Related Constants from m04_constants Used by m34

| Constant | Value | Usage in m34 |
|----------|-------|-------------|
| `R_HISTORY_MAX` | 60 | r history ring buffer cap |
| `SPHERE_CAP` | 200 | `MAX_SPHERES` alias (m34:39), phase entry cap |
| `PHASE_GAP_THRESHOLD` | π/3 (~1.047) | chimera detection threshold (m34:42) |

`MAX_CLUSTERS` = 32 is local to m34 (not from m04_constants).

---

## 3. Metric Names (m33_metrics_export)

**Status: VERIFIED** — all 10 metrics confirmed in source

### Counters

| Metric Name | Help Text | Label Key | Source Line |
|-------------|-----------|-----------|-------------|
| `orac_hook_total` | Total hook invocations | `event_type` | m33:512 |
| `orac_hook_errors_total` | Failed hook invocations | `event_type` | m33:513-518 |
| `orac_tokens_total` | Cumulative token usage | `agent` | m33:519-524 |
| `orac_bridge_poll_total` | Bridge poll attempts | `service` | m33:525-530 |
| `orac_bridge_errors_total` | Bridge poll failures | `service` | m33:531-536 |

### Gauges

| Metric Name | Help Text | Label Key | Source Line |
|-------------|-----------|-----------|-------------|
| `orac_field_order_param` | Kuramoto order parameter r | (none) | m33:539-544 |
| `orac_k_effective` | Effective coupling strength K | (none) | m33:545-550 |
| `orac_pane_circuit_state` | Per-pane circuit breaker state (0/1/2) | `pane_id` | m33:551-556 |
| `orac_uptime_seconds` | Seconds since ORAC start | (none) | m33:557-562 |

### Histograms

| Metric Name | Help Text | Buckets (ms) | Source Line |
|-------------|-----------|-------------|-------------|
| `orac_hook_latency_ms` | Per-hook response time in milliseconds | 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0 | m33:565-567 |

### MetricsRegistry Convenience Methods

| Method | Writes To | Labels |
|--------|-----------|--------|
| `record_hook(event_type, latency_ms, is_error)` | hook_total + hook_latency_ms + hook_errors_total | `{event_type="..."}` |
| `record_bridge_poll(service, is_error)` | bridge_poll_total + bridge_errors_total | `{service="..."}` |
| `set_field_r(r)` | field_order_param | `{}` |
| `set_k_effective(k)` | k_effective | `{}` |
| `set_circuit_state(pane_id, state_code)` | pane_circuit_state | `{pane_id="..."}` |
| `record_tokens(agent, count)` | tokens_total | `{agent="..."}` |

### D7 Guide vs Source — Metric Names

| D7 Guide Lists | Source Confirms | Match |
|-----------------|----------------|-------|
| `orac_hook_latency_ms` | YES | YES |
| `orac_hook_total` | YES | YES |
| `orac_hook_errors_total` | YES | YES |
| `orac_field_order_param` | YES | YES |
| `orac_k_effective` | YES | YES |
| `orac_pane_circuit_state` | YES | YES |
| `orac_tokens_total` | YES | YES |
| `orac_bridge_poll_total` | YES | YES |
| `orac_bridge_errors_total` | YES | YES |
| `orac_uptime_seconds` | YES | YES |

All 10 metric names match exactly between D7 guide and source.

---

## 4. Span Names (m32_otel_traces)

**Status: VERIFIED**

### Documented Span Name Patterns (m32 module docs, lines 13-16)

| Pattern | Kind | Description |
|---------|------|-------------|
| `orac.hook.{event_type}` | Server | Per-hook processing (SessionStart, PostToolUse, etc.) |
| `orac.bridge.{service}` | Client | Per-bridge poll/post (synthex, me, povm, rm) |
| `orac.task.{task_id}` | Internal | Task lifecycle (claim → dispatch → complete) |
| `orac.tick` | Internal | Per-tick field integration |

### Span Attribute Keys

| Key | Method | Type |
|-----|--------|------|
| `orac.pane.id` | `set_pane(&PaneId)` | String |
| `orac.task.id` | `set_task(&TaskId)` | String |

`by_pane()` queries filter on `orac.pane.id` attribute (m32:670).

### SpanBuilder Constraints

| Constraint | Value | Source |
|------------|-------|--------|
| Max span name length | 256 bytes | `MAX_SPAN_NAME_LEN` (m32:45) |
| Max attributes per span | 32 | `MAX_ATTRIBUTES` (m32:42) |
| Max spans in TraceStore | 10,000 | `MAX_SPANS` (m32:39) |
| Default export batch | 100 | `DEFAULT_BATCH_SIZE` (m32:48) |

### SpanKind Enum

| D7 Guide | Source (m32:181-189) | Match |
|----------|---------------------|-------|
| Internal, Server, Client, Producer, Consumer | Internal, Server, Client | **DISCREPANCY** |

D7 guide lists 5 variants (`Producer`, `Consumer`). Source has only 3 (`Internal`, `Server`, `Client`).
`Producer` and `Consumer` are standard OTel kinds but are not implemented in ORAC's `SpanKind`.

### SpanStatus Enum

| D7 Guide | Source (m32:197-208) | Match |
|----------|---------------------|-------|
| Unset, Ok, Error | Unset, Ok, Error { message: String } | YES (Error has payload) |

---

## 5. D7 Guide Discrepancies Summary

| # | Location | D7 Says | Source Says | Severity |
|---|----------|---------|-------------|----------|
| 1 | m35 `BudgetStatus` | `OverBudget` | `Exceeded` | **Medium** — wrong variant name |
| 2 | m32 `SpanKind` | 5 variants (includes Producer, Consumer) | 3 variants (Internal, Server, Client) | **Low** — extra variants listed but not implemented |

---

## 6. Module Internals Summary

### m32_otel_traces (OTel Tracing)

- **Types:** TraceId([u8;16]), SpanId([u8;8]), SpanKind, SpanStatus, SpanAttribute, AttributeValue, SpanEvent, Span, SpanBuilder, TraceStore
- **TraceStore:** VecDeque ring buffer, FIFO eviction, tracks total_recorded/errors/dropped
- **Queries:** `recent(n)`, `by_trace(trace_id)`, `by_pane(pane_id)`, `errors()`
- **ID generation:** PRNG seeded from `SystemTime` — not cryptographic, used for correlation only
- **Thread safety:** `parking_lot::RwLock<TraceStoreState>`, Send+Sync

### m33_metrics_export (Prometheus Metrics)

- **Types:** MetricType, Labels, MetricDescriptor, Counter, Gauge, Histogram, MetricsRegistry
- **Counter:** monotonic, inc/inc_by with Labels, rejects non-finite/negative amounts
- **Gauge:** set/inc with Labels, rejects non-finite values
- **Histogram:** configurable bucket boundaries, cumulative bucket counts, Prometheus text format
- **Latency buckets:** [0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0] ms
- **Thread safety:** each metric uses `RwLock<BTreeMap<Labels, _>>`

### m34_field_dashboard (Kuramoto Dashboard)

- **Types:** SpherePhaseEntry, ClusterSummary, PhaseGap, DashboardSnapshot, FieldDashboard
- **Ring buffer:** r_history as Vec<f64>, cap R_HISTORY_MAX=60, oldest-first eviction
- **Statistics:** r_mean(), r_stddev() (sample std dev, n-1), r_trend() (linear regression slope)
- **Helpers:** `detect_gaps(sorted_phases)` — returns PhaseGap list with chimera flags, `cluster_order_param(phases)` — computes per-cluster r
- **Caps:** MAX_SPHERES=200, MAX_CLUSTERS=32
- **Thread safety:** `RwLock<DashboardState>`, Send+Sync

### m35_token_accounting (Budget Management)

- **Types:** TokenUsage, TaskTokenRecord, BudgetStatus, BudgetConfig, TokenAccountant, AccountingSummary
- **Token usage:** input/output counts, `total = saturating_add`, cost = input*rate + output*rate
- **Budget FSM:** cost < soft → Ok, soft ≤ cost < hard → Warning, cost ≥ hard → Exceeded
- **Pane tracking:** BTreeMap<PaneId, TokenUsage>, cap 256 panes
- **Task records:** Vec<TaskTokenRecord>, FIFO eviction at 5,000 records
- **Cost estimation:** chars/4 heuristic in PostToolUse handler (m12), wired to `record_pane_usage`
- **Thread safety:** `RwLock<AccountantState>`, Send+Sync
