//! # M33: Prometheus Metrics Export
//!
//! Prometheus-compatible metrics in text exposition format.
//! Each metric uses the `orac_` prefix for namespace isolation.
//!
//! ## Layer: L7 (Monitoring)
//! ## Module: M33
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//! ## Feature: `monitoring`
//!
//! ## Exported Metrics
//!
//! | Metric | Type | Description |
//! |--------|------|-------------|
//! | `orac_hook_latency_ms` | Histogram | Per-hook response time |
//! | `orac_hook_total` | Counter | Total hook invocations |
//! | `orac_hook_errors_total` | Counter | Failed hook invocations |
//! | `orac_field_order_param` | Gauge | Kuramoto r value |
//! | `orac_k_effective` | Gauge | Effective coupling strength |
//! | `orac_pane_circuit_state` | Gauge | Per-pane circuit breaker state |
//! | `orac_tokens_total` | Counter | Cumulative token usage |
//! | `orac_bridge_poll_total` | Counter | Bridge poll attempts |
//! | `orac_bridge_errors_total` | Counter | Bridge poll failures |
//! | `orac_uptime_seconds` | Gauge | Seconds since start |

use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::now_secs;
use crate::m1_core::m02_error_handling::PvResult;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Maximum label key length.
const MAX_LABEL_KEY_LEN: usize = 64;

/// Maximum label value length.
const MAX_LABEL_VALUE_LEN: usize = 128;

/// Histogram bucket boundaries (milliseconds) for hook latency.
const LATENCY_BUCKETS: [f64; 10] = [0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0];

// ──────────────────────────────────────────────────────────────
// Metric types
// ──────────────────────────────────────────────────────────────

/// Prometheus metric type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MetricType {
    /// Monotonically increasing counter.
    Counter,
    /// Value that can go up or down.
    Gauge,
    /// Distribution of observations across buckets.
    Histogram,
}

impl MetricType {
    /// Prometheus type name for `# TYPE` lines.
    #[must_use]
    pub const fn type_name(&self) -> &'static str {
        match self {
            Self::Counter => "counter",
            Self::Gauge => "gauge",
            Self::Histogram => "histogram",
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Labels
// ──────────────────────────────────────────────────────────────

/// Ordered label set for a metric series.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Labels(BTreeMap<String, String>);

impl Labels {
    /// Empty label set.
    #[must_use]
    pub fn empty() -> Self {
        Self(BTreeMap::new())
    }

    /// Create a label set from a single key-value pair.
    #[must_use]
    pub fn single(key: impl Into<String>, value: impl Into<String>) -> Self {
        let mut map = BTreeMap::new();
        map.insert(key.into(), value.into());
        Self(map)
    }

    /// Add a label, returning `Self` for chaining.
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.0.insert(key.into(), value.into());
        self
    }

    /// Whether the label set is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Number of labels.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Format as Prometheus label string: `{key1="val1",key2="val2"}`.
    #[must_use]
    pub fn to_prom_string(&self) -> String {
        if self.0.is_empty() {
            return String::new();
        }
        let mut s = String::from("{");
        for (i, (k, v)) in self.0.iter().enumerate() {
            if i > 0 {
                s.push(',');
            }
            // Truncate long values for safety
            let key = &k[..k.len().min(MAX_LABEL_KEY_LEN)];
            let val = &v[..v.len().min(MAX_LABEL_VALUE_LEN)];
            let _ = write!(s, "{key}=\"{val}\"");
        }
        s.push('}');
        s
    }
}

impl Default for Labels {
    fn default() -> Self {
        Self::empty()
    }
}

// ──────────────────────────────────────────────────────────────
// Metric descriptor
// ──────────────────────────────────────────────────────────────

/// Metric metadata (name, type, help text).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricDescriptor {
    /// Metric name (e.g. `"orac_hook_latency_ms"`).
    pub name: String,
    /// Metric type.
    pub metric_type: MetricType,
    /// Help text for `# HELP` line.
    pub help: String,
}

// ──────────────────────────────────────────────────────────────
// Counter
// ──────────────────────────────────────────────────────────────

/// Monotonically increasing counter.
#[derive(Debug)]
pub struct Counter {
    /// Counter value per label set.
    values: RwLock<BTreeMap<Labels, f64>>,
}

impl Counter {
    /// Create a new counter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: RwLock::new(BTreeMap::new()),
        }
    }

    /// Increment by 1 with given labels.
    pub fn inc(&self, labels: &Labels) {
        let mut vals = self.values.write();
        *vals.entry(labels.clone()).or_insert(0.0) += 1.0;
    }

    /// Increment by a specific amount.
    pub fn inc_by(&self, labels: &Labels, amount: f64) {
        if amount <= 0.0 || !amount.is_finite() {
            return;
        }
        let mut vals = self.values.write();
        *vals.entry(labels.clone()).or_insert(0.0) += amount;
    }

    /// Get the current value for a label set.
    #[must_use]
    pub fn get(&self, labels: &Labels) -> f64 {
        self.values.read().get(labels).copied().unwrap_or(0.0)
    }

    /// Get all series.
    #[must_use]
    pub fn all(&self) -> BTreeMap<Labels, f64> {
        self.values.read().clone()
    }

    /// Reset all values.
    pub fn reset(&self) {
        self.values.write().clear();
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Gauge
// ──────────────────────────────────────────────────────────────

/// Gauge metric that can go up or down.
#[derive(Debug)]
pub struct Gauge {
    /// Gauge value per label set.
    values: RwLock<BTreeMap<Labels, f64>>,
}

impl Gauge {
    /// Create a new gauge.
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: RwLock::new(BTreeMap::new()),
        }
    }

    /// Set an absolute value.
    pub fn set(&self, labels: &Labels, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.values.write().insert(labels.clone(), value);
    }

    /// Increment by `delta`.
    pub fn inc(&self, labels: &Labels, delta: f64) {
        if !delta.is_finite() {
            return;
        }
        let mut vals = self.values.write();
        *vals.entry(labels.clone()).or_insert(0.0) += delta;
    }

    /// Get current value.
    #[must_use]
    pub fn get(&self, labels: &Labels) -> f64 {
        self.values.read().get(labels).copied().unwrap_or(0.0)
    }

    /// Get all series.
    #[must_use]
    pub fn all(&self) -> BTreeMap<Labels, f64> {
        self.values.read().clone()
    }

    /// Reset all values.
    pub fn reset(&self) {
        self.values.write().clear();
    }
}

impl Default for Gauge {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Histogram
// ──────────────────────────────────────────────────────────────

/// Histogram observation state for one label set.
#[derive(Debug, Clone)]
struct HistogramState {
    /// Bucket counts (same length as bucket boundaries + 1 for +Inf).
    buckets: Vec<u64>,
    /// Sum of all observed values.
    sum: f64,
    /// Total number of observations.
    count: u64,
}

/// Histogram metric for distribution tracking.
#[derive(Debug)]
pub struct Histogram {
    /// Bucket boundaries (sorted ascending).
    boundaries: Vec<f64>,
    /// State per label set.
    states: RwLock<BTreeMap<Labels, HistogramState>>,
}

impl Histogram {
    /// Create a histogram with the given bucket boundaries.
    #[must_use]
    pub fn new(boundaries: &[f64]) -> Self {
        let mut sorted: Vec<f64> = boundaries.iter().copied().filter(|v| v.is_finite()).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        sorted.dedup();
        Self {
            boundaries: sorted,
            states: RwLock::new(BTreeMap::new()),
        }
    }

    /// Create a histogram with default latency buckets.
    #[must_use]
    pub fn with_latency_buckets() -> Self {
        Self::new(&LATENCY_BUCKETS)
    }

    /// Observe a value.
    pub fn observe(&self, labels: &Labels, value: f64) {
        if !value.is_finite() {
            return;
        }
        let mut states = self.states.write();
        let state = states.entry(labels.clone()).or_insert_with(|| HistogramState {
            buckets: vec![0; self.boundaries.len() + 1],
            sum: 0.0,
            count: 0,
        });
        state.sum += value;
        state.count = state.count.saturating_add(1);
        for (i, &bound) in self.boundaries.iter().enumerate() {
            if value <= bound {
                state.buckets[i] = state.buckets[i].saturating_add(1);
            }
        }
        // +Inf bucket always gets incremented
        let inf_idx = self.boundaries.len();
        state.buckets[inf_idx] = state.buckets[inf_idx].saturating_add(1);
    }

    /// Get the count for a label set.
    #[must_use]
    pub fn count(&self, labels: &Labels) -> u64 {
        self.states
            .read()
            .get(labels)
            .map_or(0, |s| s.count)
    }

    /// Get the sum for a label set.
    #[must_use]
    pub fn sum(&self, labels: &Labels) -> f64 {
        self.states
            .read()
            .get(labels)
            .map_or(0.0, |s| s.sum)
    }

    /// Number of bucket boundaries.
    #[must_use]
    pub fn num_buckets(&self) -> usize {
        self.boundaries.len()
    }

    /// Reset all observations.
    pub fn reset(&self) {
        self.states.write().clear();
    }

    /// Format histogram for Prometheus exposition.
    fn format_prom(&self, name: &str, output: &mut String) {
        let states = self.states.read();
        for (labels, state) in states.iter() {
            let label_str = labels.to_prom_string();
            let mut cumulative = 0u64;
            for (i, &bound) in self.boundaries.iter().enumerate() {
                cumulative = cumulative.saturating_add(state.buckets[i]);
                if label_str.is_empty() {
                    let _ = writeln!(output, "{name}_bucket{{le=\"{bound}\"}} {cumulative}");
                } else {
                    let inner = &label_str[1..label_str.len() - 1];
                    let _ = writeln!(
                        output,
                        "{name}_bucket{{{inner},le=\"{bound}\"}} {cumulative}"
                    );
                }
            }
            // +Inf bucket
            cumulative = cumulative.saturating_add(state.buckets[self.boundaries.len()]);
            if label_str.is_empty() {
                let _ = writeln!(output, "{name}_bucket{{le=\"+Inf\"}} {cumulative}");
            } else {
                let inner = &label_str[1..label_str.len() - 1];
                let _ = writeln!(output, "{name}_bucket{{{inner},le=\"+Inf\"}} {cumulative}");
            }
            let _ = writeln!(output, "{name}_sum{label_str} {}", state.sum);
            let _ = writeln!(output, "{name}_count{label_str} {}", state.count);
        }
    }
}

// ──────────────────────────────────────────────────────────────
// MetricsRegistry
// ──────────────────────────────────────────────────────────────

/// Central metrics registry.
///
/// Holds all ORAC metrics and produces Prometheus text exposition.
/// Thread-safe via interior mutability.
#[derive(Debug)]
pub struct MetricsRegistry {
    /// Hook invocation counter.
    pub hook_total: Counter,
    /// Hook error counter.
    pub hook_errors_total: Counter,
    /// Hook latency histogram (milliseconds).
    pub hook_latency_ms: Histogram,
    /// Kuramoto order parameter gauge.
    pub field_order_param: Gauge,
    /// Effective coupling strength gauge.
    pub k_effective: Gauge,
    /// Per-pane circuit breaker state (0=closed, 1=open, 2=half-open).
    pub pane_circuit_state: Gauge,
    /// Cumulative token usage counter.
    pub tokens_total: Counter,
    /// Bridge poll counter.
    pub bridge_poll_total: Counter,
    /// Bridge error counter.
    pub bridge_errors_total: Counter,
    /// Uptime gauge (seconds).
    pub uptime_seconds: Gauge,
    /// Start time for uptime calculation.
    start_time: f64,
}

impl MetricsRegistry {
    /// Create a new metrics registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            hook_total: Counter::new(),
            hook_errors_total: Counter::new(),
            hook_latency_ms: Histogram::with_latency_buckets(),
            field_order_param: Gauge::new(),
            k_effective: Gauge::new(),
            pane_circuit_state: Gauge::new(),
            tokens_total: Counter::new(),
            bridge_poll_total: Counter::new(),
            bridge_errors_total: Counter::new(),
            uptime_seconds: Gauge::new(),
            start_time: now_secs(),
        }
    }

    /// Record a hook invocation.
    pub fn record_hook(&self, event_type: &str, latency_ms: f64, is_error: bool) {
        let labels = Labels::single("event_type", event_type);
        self.hook_total.inc(&labels);
        self.hook_latency_ms.observe(&labels, latency_ms);
        if is_error {
            self.hook_errors_total.inc(&labels);
        }
    }

    /// Record a bridge poll.
    pub fn record_bridge_poll(&self, service: &str, is_error: bool) {
        let labels = Labels::single("service", service);
        self.bridge_poll_total.inc(&labels);
        if is_error {
            self.bridge_errors_total.inc(&labels);
        }
    }

    /// Update the field order parameter gauge.
    pub fn set_field_r(&self, r: f64) {
        self.field_order_param.set(&Labels::empty(), r);
    }

    /// Update the effective K gauge.
    pub fn set_k_effective(&self, k: f64) {
        self.k_effective.set(&Labels::empty(), k);
    }

    /// Set a pane's circuit breaker state (0=closed, 1=open, 2=half-open).
    pub fn set_circuit_state(&self, pane_id: &str, state_code: f64) {
        let labels = Labels::single("pane_id", pane_id);
        self.pane_circuit_state.set(&labels, state_code);
    }

    /// Record token usage.
    pub fn record_tokens(&self, agent: &str, count: f64) {
        let labels = Labels::single("agent", agent);
        self.tokens_total.inc_by(&labels, count);
    }

    /// Produce Prometheus text exposition format.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if formatting fails.
    pub fn exposition(&self) -> PvResult<String> {
        let mut out = String::with_capacity(4096);

        // Update uptime
        let uptime = now_secs() - self.start_time;
        self.uptime_seconds.set(&Labels::empty(), uptime);

        // Counters
        format_counter(&mut out, "orac_hook_total", "Total hook invocations", &self.hook_total);
        format_counter(
            &mut out,
            "orac_hook_errors_total",
            "Failed hook invocations",
            &self.hook_errors_total,
        );
        format_counter(
            &mut out,
            "orac_tokens_total",
            "Cumulative token usage",
            &self.tokens_total,
        );
        format_counter(
            &mut out,
            "orac_bridge_poll_total",
            "Bridge poll attempts",
            &self.bridge_poll_total,
        );
        format_counter(
            &mut out,
            "orac_bridge_errors_total",
            "Bridge poll failures",
            &self.bridge_errors_total,
        );

        // Gauges
        format_gauge(
            &mut out,
            "orac_field_order_param",
            "Kuramoto order parameter r",
            &self.field_order_param,
        );
        format_gauge(
            &mut out,
            "orac_k_effective",
            "Effective coupling strength K",
            &self.k_effective,
        );
        format_gauge(
            &mut out,
            "orac_pane_circuit_state",
            "Per-pane circuit breaker state (0=closed, 1=open, 2=half-open)",
            &self.pane_circuit_state,
        );
        format_gauge(
            &mut out,
            "orac_uptime_seconds",
            "Seconds since ORAC start",
            &self.uptime_seconds,
        );

        // Histogram
        let _ = writeln!(out, "# HELP orac_hook_latency_ms Per-hook response time in milliseconds");
        let _ = writeln!(out, "# TYPE orac_hook_latency_ms histogram");
        self.hook_latency_ms.format_prom("orac_hook_latency_ms", &mut out);

        Ok(out)
    }

    /// List all metric descriptors.
    #[must_use]
    pub fn descriptors(&self) -> Vec<MetricDescriptor> {
        vec![
            MetricDescriptor {
                name: "orac_hook_total".into(),
                metric_type: MetricType::Counter,
                help: "Total hook invocations".into(),
            },
            MetricDescriptor {
                name: "orac_hook_errors_total".into(),
                metric_type: MetricType::Counter,
                help: "Failed hook invocations".into(),
            },
            MetricDescriptor {
                name: "orac_hook_latency_ms".into(),
                metric_type: MetricType::Histogram,
                help: "Per-hook response time in milliseconds".into(),
            },
            MetricDescriptor {
                name: "orac_field_order_param".into(),
                metric_type: MetricType::Gauge,
                help: "Kuramoto order parameter r".into(),
            },
            MetricDescriptor {
                name: "orac_k_effective".into(),
                metric_type: MetricType::Gauge,
                help: "Effective coupling strength K".into(),
            },
            MetricDescriptor {
                name: "orac_pane_circuit_state".into(),
                metric_type: MetricType::Gauge,
                help: "Per-pane circuit breaker state".into(),
            },
            MetricDescriptor {
                name: "orac_tokens_total".into(),
                metric_type: MetricType::Counter,
                help: "Cumulative token usage".into(),
            },
            MetricDescriptor {
                name: "orac_bridge_poll_total".into(),
                metric_type: MetricType::Counter,
                help: "Bridge poll attempts".into(),
            },
            MetricDescriptor {
                name: "orac_bridge_errors_total".into(),
                metric_type: MetricType::Counter,
                help: "Bridge poll failures".into(),
            },
            MetricDescriptor {
                name: "orac_uptime_seconds".into(),
                metric_type: MetricType::Gauge,
                help: "Seconds since ORAC start".into(),
            },
        ]
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Formatting helpers
// ──────────────────────────────────────────────────────────────

fn format_counter(out: &mut String, name: &str, help: &str, counter: &Counter) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} counter");
    for (labels, value) in counter.all() {
        let label_str = labels.to_prom_string();
        let _ = writeln!(out, "{name}{label_str} {value}");
    }
}

fn format_gauge(out: &mut String, name: &str, help: &str, gauge: &Gauge) {
    let _ = writeln!(out, "# HELP {name} {help}");
    let _ = writeln!(out, "# TYPE {name} gauge");
    for (labels, value) in gauge.all() {
        let label_str = labels.to_prom_string();
        let _ = writeln!(out, "{name}{label_str} {value}");
    }
}

// now_secs() imported from m01_core_types (centralized)

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── MetricType ──

    #[test]
    fn counter_type_name() {
        assert_eq!(MetricType::Counter.type_name(), "counter");
    }

    #[test]
    fn gauge_type_name() {
        assert_eq!(MetricType::Gauge.type_name(), "gauge");
    }

    #[test]
    fn histogram_type_name() {
        assert_eq!(MetricType::Histogram.type_name(), "histogram");
    }

    // ── Labels ──

    #[test]
    fn labels_empty() {
        let l = Labels::empty();
        assert!(l.is_empty());
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn labels_single() {
        let l = Labels::single("service", "synthex");
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn labels_with_chaining() {
        let l = Labels::single("a", "1").with("b", "2");
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn labels_prom_string_empty() {
        let l = Labels::empty();
        assert_eq!(l.to_prom_string(), "");
    }

    #[test]
    fn labels_prom_string_single() {
        let l = Labels::single("service", "synthex");
        assert_eq!(l.to_prom_string(), "{service=\"synthex\"}");
    }

    #[test]
    fn labels_prom_string_sorted() {
        let l = Labels::single("z", "1").with("a", "2");
        let s = l.to_prom_string();
        assert!(s.starts_with("{a=\"2\""));
    }

    #[test]
    fn labels_default_is_empty() {
        let l = Labels::default();
        assert!(l.is_empty());
    }

    // ── Counter ──

    #[test]
    fn counter_new_zero() {
        let c = Counter::new();
        assert!((c.get(&Labels::empty()) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_default_zero() {
        let c = Counter::default();
        assert!((c.get(&Labels::empty()) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_inc() {
        let c = Counter::new();
        let l = Labels::empty();
        c.inc(&l);
        c.inc(&l);
        assert!((c.get(&l) - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_inc_by() {
        let c = Counter::new();
        let l = Labels::empty();
        c.inc_by(&l, 5.0);
        assert!((c.get(&l) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_inc_by_negative_ignored() {
        let c = Counter::new();
        let l = Labels::empty();
        c.inc_by(&l, -1.0);
        assert!((c.get(&l) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_inc_by_nan_ignored() {
        let c = Counter::new();
        let l = Labels::empty();
        c.inc_by(&l, f64::NAN);
        assert!((c.get(&l) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_per_label_set() {
        let c = Counter::new();
        let l1 = Labels::single("a", "1");
        let l2 = Labels::single("a", "2");
        c.inc(&l1);
        c.inc(&l1);
        c.inc(&l2);
        assert!((c.get(&l1) - 2.0).abs() < f64::EPSILON);
        assert!((c.get(&l2) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn counter_all() {
        let c = Counter::new();
        let l = Labels::single("x", "y");
        c.inc(&l);
        let all = c.all();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn counter_reset() {
        let c = Counter::new();
        c.inc(&Labels::empty());
        c.reset();
        assert!((c.get(&Labels::empty()) - 0.0).abs() < f64::EPSILON);
    }

    // ── Gauge ──

    #[test]
    fn gauge_new_zero() {
        let g = Gauge::new();
        assert!((g.get(&Labels::empty()) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_default_zero() {
        let g = Gauge::default();
        assert!((g.get(&Labels::empty()) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_set() {
        let g = Gauge::new();
        let l = Labels::empty();
        g.set(&l, 42.0);
        assert!((g.get(&l) - 42.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_set_overwrites() {
        let g = Gauge::new();
        let l = Labels::empty();
        g.set(&l, 10.0);
        g.set(&l, 20.0);
        assert!((g.get(&l) - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_set_nan_ignored() {
        let g = Gauge::new();
        let l = Labels::empty();
        g.set(&l, 5.0);
        g.set(&l, f64::NAN);
        assert!((g.get(&l) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_inc() {
        let g = Gauge::new();
        let l = Labels::empty();
        g.set(&l, 10.0);
        g.inc(&l, 5.0);
        assert!((g.get(&l) - 15.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_inc_nan_ignored() {
        let g = Gauge::new();
        let l = Labels::empty();
        g.set(&l, 10.0);
        g.inc(&l, f64::NAN);
        assert!((g.get(&l) - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn gauge_all() {
        let g = Gauge::new();
        g.set(&Labels::single("x", "1"), 1.0);
        g.set(&Labels::single("x", "2"), 2.0);
        assert_eq!(g.all().len(), 2);
    }

    #[test]
    fn gauge_reset() {
        let g = Gauge::new();
        g.set(&Labels::empty(), 99.0);
        g.reset();
        assert!((g.get(&Labels::empty()) - 0.0).abs() < f64::EPSILON);
    }

    // ── Histogram ──

    #[test]
    fn histogram_new() {
        let h = Histogram::new(&[1.0, 5.0, 10.0]);
        assert_eq!(h.num_buckets(), 3);
    }

    #[test]
    fn histogram_with_latency_buckets() {
        let h = Histogram::with_latency_buckets();
        assert_eq!(h.num_buckets(), LATENCY_BUCKETS.len());
    }

    #[test]
    fn histogram_observe() {
        let h = Histogram::new(&[1.0, 5.0, 10.0]);
        let l = Labels::empty();
        h.observe(&l, 3.0);
        assert_eq!(h.count(&l), 1);
        assert!((h.sum(&l) - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_multiple_observations() {
        let h = Histogram::new(&[1.0, 5.0, 10.0]);
        let l = Labels::empty();
        h.observe(&l, 0.5);
        h.observe(&l, 3.0);
        h.observe(&l, 7.0);
        assert_eq!(h.count(&l), 3);
        assert!((h.sum(&l) - 10.5).abs() < f64::EPSILON);
    }

    #[test]
    fn histogram_nan_ignored() {
        let h = Histogram::new(&[1.0, 5.0]);
        let l = Labels::empty();
        h.observe(&l, f64::NAN);
        assert_eq!(h.count(&l), 0);
    }

    #[test]
    fn histogram_format_prom() {
        let h = Histogram::new(&[1.0, 5.0]);
        let l = Labels::empty();
        h.observe(&l, 3.0);
        let mut out = String::new();
        h.format_prom("test_metric", &mut out);
        assert!(out.contains("test_metric_bucket{le=\"1\"} 0"));
        assert!(out.contains("test_metric_bucket{le=\"5\"} 1"));
        // +Inf is cumulative: le=5 bucket (1) + inf bucket (1) = 2
        assert!(out.contains("test_metric_bucket{le=\"+Inf\"} 2"));
        assert!(out.contains("test_metric_count 1"));
    }

    #[test]
    fn histogram_reset() {
        let h = Histogram::new(&[1.0]);
        let l = Labels::empty();
        h.observe(&l, 0.5);
        h.reset();
        assert_eq!(h.count(&l), 0);
    }

    #[test]
    fn histogram_deduplicates_boundaries() {
        let h = Histogram::new(&[5.0, 1.0, 5.0, 1.0, 10.0]);
        assert_eq!(h.num_buckets(), 3);
    }

    #[test]
    fn histogram_sorts_boundaries() {
        let h = Histogram::new(&[10.0, 1.0, 5.0]);
        assert_eq!(h.boundaries, vec![1.0, 5.0, 10.0]);
    }

    // ── MetricsRegistry ──

    #[test]
    fn registry_new() {
        let r = MetricsRegistry::new();
        assert_eq!(r.descriptors().len(), 10);
    }

    #[test]
    fn registry_default() {
        let r = MetricsRegistry::default();
        assert!(!r.descriptors().is_empty());
    }

    #[test]
    fn registry_record_hook() {
        let r = MetricsRegistry::new();
        r.record_hook("PostToolUse", 1.5, false);
        let l = Labels::single("event_type", "PostToolUse");
        assert!((r.hook_total.get(&l) - 1.0).abs() < f64::EPSILON);
        assert!((r.hook_errors_total.get(&l) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_record_hook_error() {
        let r = MetricsRegistry::new();
        r.record_hook("Stop", 5.0, true);
        let l = Labels::single("event_type", "Stop");
        assert!((r.hook_errors_total.get(&l) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_record_bridge_poll() {
        let r = MetricsRegistry::new();
        r.record_bridge_poll("synthex", false);
        let l = Labels::single("service", "synthex");
        assert!((r.bridge_poll_total.get(&l) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_record_bridge_error() {
        let r = MetricsRegistry::new();
        r.record_bridge_poll("me", true);
        let l = Labels::single("service", "me");
        assert!((r.bridge_errors_total.get(&l) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_set_field_r() {
        let r = MetricsRegistry::new();
        r.set_field_r(0.993);
        assert!((r.field_order_param.get(&Labels::empty()) - 0.993).abs() < 1e-6);
    }

    #[test]
    fn registry_set_k_effective() {
        let r = MetricsRegistry::new();
        r.set_k_effective(2.42);
        assert!((r.k_effective.get(&Labels::empty()) - 2.42).abs() < 1e-6);
    }

    #[test]
    fn registry_set_circuit_state() {
        let r = MetricsRegistry::new();
        r.set_circuit_state("fleet-alpha", 0.0);
        let l = Labels::single("pane_id", "fleet-alpha");
        assert!((r.pane_circuit_state.get(&l) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_record_tokens() {
        let r = MetricsRegistry::new();
        r.record_tokens("fleet-alpha", 1500.0);
        let l = Labels::single("agent", "fleet-alpha");
        assert!((r.tokens_total.get(&l) - 1500.0).abs() < f64::EPSILON);
    }

    #[test]
    fn registry_exposition_contains_help() {
        let r = MetricsRegistry::new();
        r.set_field_r(0.99);
        let text = r.exposition().unwrap();
        assert!(text.contains("# HELP orac_field_order_param"));
        assert!(text.contains("# TYPE orac_field_order_param gauge"));
    }

    #[test]
    fn registry_exposition_contains_uptime() {
        let r = MetricsRegistry::new();
        let text = r.exposition().unwrap();
        assert!(text.contains("orac_uptime_seconds"));
    }

    #[test]
    fn registry_exposition_contains_counters() {
        let r = MetricsRegistry::new();
        r.record_hook("SessionStart", 2.0, false);
        let text = r.exposition().unwrap();
        assert!(text.contains("orac_hook_total"));
    }

    // ── Thread safety ──

    #[test]
    fn counter_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Counter>();
    }

    #[test]
    fn counter_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Counter>();
    }

    #[test]
    fn gauge_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Gauge>();
    }

    #[test]
    fn gauge_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Gauge>();
    }

    #[test]
    fn histogram_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<Histogram>();
    }

    #[test]
    fn histogram_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<Histogram>();
    }

    #[test]
    fn registry_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MetricsRegistry>();
    }

    #[test]
    fn registry_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<MetricsRegistry>();
    }

    // ── Constants ──

    #[test]
    fn latency_buckets_sorted() {
        for w in LATENCY_BUCKETS.windows(2) {
            assert!(w[0] < w[1]);
        }
    }

    // ── MetricDescriptor ──

    #[test]
    fn descriptor_serializes() {
        let d = MetricDescriptor {
            name: "test".into(),
            metric_type: MetricType::Counter,
            help: "a counter".into(),
        };
        let json = serde_json::to_string(&d);
        assert!(json.is_ok());
    }
}
