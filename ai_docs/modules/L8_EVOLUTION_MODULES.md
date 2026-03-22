---
title: "Layer 8: Evolution — Module Documentation"
date: 2026-03-22
tags: [modules, evolution, L8, orac-sidecar, ralph]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L8
modules: [m36, m37, m38, m39, m40]
---

# Layer 8: Evolution (m36-m40)

> Self-improving coordination via 5-phase RALPH loop. Cloned from ME with CRITICAL fix:
> **multi-parameter mutation** (NOT mono-parameter like ME's BUG-035).
> **Target LOC:** ~3,500 | **Target tests:** 60+
> **Source:** ME clone + fix (ALL NEW for diversity enforcement) | **Phase:** 4
> **Feature gate:** `evolution` (implies `intelligence` + `monitoring`)

---

## Overview

L8 implements the RALPH evolution chamber -- a 5-phase self-modification loop that tunes ORAC's coordination parameters over time. The architecture is cloned from the Maintenance Engine but with a critical fix: **multi-parameter mutation selection** (pattern P20) that prevents the mono-parameter trap documented as BUG-035 / AP12. The emergence detector uses a capped ring buffer (AP19 lesson). All modules are feature-gated under `#[cfg(feature = "evolution")]`.

### BUG-035 Fix (CRITICAL)

ME's evolution chamber targeted `min_confidence` in 318/380 mutations (84%). This mono-parameter fixation is anti-pattern AP12. ORAC enforces diversity via m40:

- **Round-robin** across full parameter pool (not weighted toward one)
- **10-generation cooldown** per parameter between repeated targeting
- **Rejection gate:** reject proposal if >50% of last 20 mutations hit same parameter

See: `[[ORAC -- RALPH Multi-Parameter Mutation Fix]]` in Obsidian.

### Design Constraints

- RALPH loop (m36): max 30 iterations per cycle
- Convergence = delta < 0.001 for 3 consecutive steps
- Mutation engine (m40) must snapshot before mutation and support atomic rollback
- Emergence cap: 5,000 with TTL decay (AP19 -- cap alone leads to BUG-035 deadlock)
- Fitness threshold: only apply mutation if improvement >= 2%
- All tensor operations use FMA (pattern P01/P05)
- Feature-gated: `#[cfg(feature = "evolution")]`

### Dependencies

- **L1 Core** -- `OracError`, `Timestamp`, float utilities
- **L4 Intelligence** -- Hebbian weights, coupling parameters, decision engine
- **L5 Bridges** -- Reasoning Memory for persistence, SYNTHEX for cascade feedback
- **L7 Monitoring** -- metrics for convergence tracking, emergence scoring

### RALPH Phases

```
  Recognize ──> Analyze ──> Learn ──> Propose ──> Harvest
      │                                              │
      └──────── convergence check (delta < 0.001) ───┘
                  3 consecutive → STOP
                  max 30 iterations → STOP
```

---

## m36 -- RALPH Engine

**Source:** `src/m8_evolution/m36_ralph_engine.rs`
**LOC Target:** ~800
**Depends on:** `m01_core_types`, `m02_error_handling`, `m37_emergence_detector`, `m38_correlation_engine`, `m39_fitness_tensor`, `m40_mutation_selector`

### Design Decisions

- 5-phase loop: Recognize, Analyze, Learn, Propose, Harvest
- Max 30 iterations per RALPH cycle (hard limit, non-negotiable)
- Convergence = fitness delta < 0.001 for 3 consecutive steps
- Each phase is a distinct function returning `Result<PhaseOutput, OracError>`
- Phase transitions emit events via L7 monitoring for convergence tracking
- Snapshot taken before Propose phase; rollback if fitness regresses
- RALPH cycle can be triggered manually via API or automatically on emergence detection

### Types to Implement

```rust
/// The 5 phases of a RALPH cycle.
///
/// Recognize -> Analyze -> Learn -> Propose -> Harvest.
/// Each phase produces a `PhaseOutput` consumed by the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum RalphPhase {
    /// Detect patterns in recent field behaviour.
    Recognize,
    /// Analyze correlations and causal pathways.
    Analyze,
    /// Extract lessons from analysis.
    Learn,
    /// Propose parameter mutations (with snapshot).
    Propose,
    /// Evaluate fitness improvement and commit or rollback.
    Harvest,
}

/// Output from a single RALPH phase.
///
/// Carries phase-specific data forward to the next phase.
#[derive(Debug, Clone)]
pub struct PhaseOutput {
    /// Which phase produced this output.
    pub phase: RalphPhase,
    /// Emergences detected (Recognize) or correlations found (Analyze).
    pub signals: Vec<EvolutionSignal>,
    /// Lessons extracted (Learn phase).
    pub lessons: Vec<Lesson>,
    /// Proposed mutations (Propose phase).
    pub mutations: Vec<MutationProposal>,
    /// Fitness before and after (Harvest phase).
    pub fitness_delta: Option<f64>,
    /// Duration of this phase in microseconds.
    pub duration_us: u64,
}

/// A signal from the emergence detector or correlation engine.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EvolutionSignal {
    /// Signal identifier.
    pub id: u64,
    /// Signal type (emergence, correlation, anomaly).
    pub kind: SignalKind,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
    /// Parameters involved in this signal.
    pub parameters: Vec<String>,
    /// Timestamp when detected.
    pub detected_at: Timestamp,
}

/// Signal classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum SignalKind {
    /// Novel pattern not seen before.
    Emergence,
    /// Correlation between parameters.
    Correlation,
    /// Anomalous parameter value.
    Anomaly,
}

/// A lesson extracted during the Learn phase.
#[derive(Debug, Clone)]
pub struct Lesson {
    /// What was observed.
    pub observation: String,
    /// Which parameters are implicated.
    pub parameters: Vec<String>,
    /// Suggested direction of change.
    pub direction: MutationDirection,
    /// Confidence in the lesson.
    pub confidence: f64,
}

/// Direction of parameter mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MutationDirection {
    /// Increase the parameter value.
    Increase,
    /// Decrease the parameter value.
    Decrease,
    /// No change recommended.
    Hold,
}

/// The RALPH engine state machine.
///
/// Orchestrates the 5-phase loop with convergence checking
/// and iteration limits.
pub struct RalphEngine {
    /// Current phase.
    current_phase: RalphPhase,
    /// Iteration count within current cycle.
    iteration: u32,
    /// Maximum iterations per cycle (default: 30).
    max_iterations: u32,
    /// Convergence threshold (default: 0.001).
    convergence_threshold: f64,
    /// Number of consecutive steps with delta < threshold.
    convergence_streak: u32,
    /// Required streak length for convergence (default: 3).
    convergence_required: u32,
    /// Fitness history for convergence detection.
    fitness_history: VecDeque<f64>,
    /// Reference to emergence detector.
    emergence: Arc<EmergenceDetector>,
    /// Reference to correlation engine.
    correlator: Arc<CorrelationEngine>,
    /// Reference to fitness tensor.
    fitness: Arc<FitnessTensor>,
    /// Reference to mutation selector.
    selector: Arc<MutationSelector>,
    /// Snapshot for rollback.
    snapshot: Option<ParameterSnapshot>,
}

/// RALPH cycle summary, returned after a full cycle completes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CycleSummary {
    /// Total iterations executed.
    pub iterations: u32,
    /// Whether convergence was achieved.
    pub converged: bool,
    /// Final fitness value.
    pub final_fitness: f64,
    /// Number of mutations applied.
    pub mutations_applied: u32,
    /// Number of mutations rolled back.
    pub mutations_rolled_back: u32,
    /// Total duration in milliseconds.
    pub duration_ms: u64,
}
```

### Key Functions

- `RalphEngine::new(config: RalphConfig, emergence: Arc<EmergenceDetector>, correlator: Arc<CorrelationEngine>, fitness: Arc<FitnessTensor>, selector: Arc<MutationSelector>) -> Self` -- Construct engine with all dependencies.
- `run_cycle(&mut self, state: &SharedState) -> Result<CycleSummary, OracError>` -- Execute a full RALPH cycle (up to 30 iterations).
- `step_recognize(&self, state: &SharedState) -> Result<PhaseOutput, OracError>` -- Detect emergences and anomalies in recent field state.
- `step_analyze(&self, signals: &[EvolutionSignal]) -> Result<PhaseOutput, OracError>` -- Mine correlations from detected signals.
- `step_learn(&self, correlations: &[EvolutionSignal]) -> Result<PhaseOutput, OracError>` -- Extract actionable lessons.
- `step_propose(&mut self, lessons: &[Lesson], state: &SharedState) -> Result<PhaseOutput, OracError>` -- Propose mutations and snapshot state.
- `step_harvest(&mut self, mutations: &[MutationProposal], state: &SharedState) -> Result<PhaseOutput, OracError>` -- Evaluate fitness; commit if improvement >= 2%, rollback otherwise.
- `check_convergence(&mut self, delta: f64) -> bool` -- Returns true if delta < 0.001 for 3 consecutive steps.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_max_iterations_enforced` | unit | Cycle stops at 30 iterations |
| `test_convergence_three_consecutive` | unit | Converges after 3 steps with delta < 0.001 |
| `test_convergence_reset_on_large_delta` | unit | Streak resets when delta >= 0.001 |
| `test_rollback_on_regression` | unit | Fitness regression triggers rollback |
| `test_commit_on_improvement` | unit | >= 2% improvement commits mutation |
| `test_phase_ordering` | unit | Phases execute in R-A-L-P-H order |
| `test_empty_signals_short_circuit` | unit | No emergences -> no mutations proposed |
| `test_cycle_summary_fields` | integration | Summary has correct counts and duration |
| `test_snapshot_restore_integrity` | unit | Rolled-back state matches pre-mutation |

### Cross-References

- `m37_emergence_detector` -- Recognize phase queries emergence ring buffer
- `m38_correlation_engine` -- Analyze phase mines correlations
- `m39_fitness_tensor` -- Harvest phase evaluates 12D fitness
- `m40_mutation_selector` -- Propose phase selects parameters via diversity-enforced selector
- ORAC_PLAN.md Phase 4 Detail
- Obsidian: `[[ME RALPH Loop Specification]]`

---

## m37 -- Emergence Detector

**Source:** `src/m8_evolution/m37_emergence_detector.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types`, `m02_error_handling`

### Design Decisions

- Ring buffer with cap 5,000 entries (AP19: cap alone leads to exhaustion if no TTL decay)
- TTL decay: entries expire after configurable duration (default: 1 hour)
- Expired entries are lazily purged during insertion (amortised cost)
- Emergence threshold: pattern must appear >= 3 times within TTL to be classified as emergence
- Deduplication via content hash (FNV-1a) -- same emergence pattern is not double-counted
- Thread-safe via `parking_lot::RwLock` for read-heavy workload
- Property-based testing: buffer never exceeds 5,000 entries regardless of insertion rate

### Types to Implement

```rust
use std::collections::VecDeque;

/// Maximum entries in the emergence ring buffer (AP19).
pub const EMERGENCE_CAP: usize = 5_000;

/// Default TTL for emergence entries in seconds.
pub const EMERGENCE_TTL_SECS: u64 = 3_600;

/// Minimum occurrences within TTL to classify as emergence.
pub const EMERGENCE_THRESHOLD: u32 = 3;

/// An observed pattern that may constitute an emergence.
///
/// Stored in the ring buffer with TTL for automatic expiry.
#[derive(Debug, Clone)]
pub struct EmergenceEntry {
    /// Unique entry ID.
    pub id: u64,
    /// Content hash for deduplication (FNV-1a).
    pub content_hash: u64,
    /// The pattern description.
    pub pattern: String,
    /// Which parameters are involved.
    pub parameters: Vec<String>,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
    /// When this entry was recorded.
    pub recorded_at: Timestamp,
    /// When this entry expires (recorded_at + TTL).
    pub expires_at: Timestamp,
}

/// A confirmed emergence -- a pattern that has been observed
/// at least `EMERGENCE_THRESHOLD` times within the TTL window.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Emergence {
    /// Content hash identifying the pattern.
    pub content_hash: u64,
    /// The pattern description.
    pub pattern: String,
    /// Number of observations within TTL.
    pub occurrence_count: u32,
    /// Average confidence across observations.
    pub mean_confidence: f64,
    /// First observation timestamp.
    pub first_seen: Timestamp,
    /// Most recent observation timestamp.
    pub last_seen: Timestamp,
    /// Parameters implicated.
    pub parameters: Vec<String>,
}

/// Ring buffer for emergence detection with TTL decay.
///
/// Cap: 5,000 entries. Expired entries purged lazily on insert.
/// Thread-safe via `parking_lot::RwLock`.
///
/// # Anti-Pattern Reference
///
/// AP19: Emergence cap exhaustion. Cap alone is insufficient --
/// without TTL decay, the buffer fills with stale entries and
/// new emergences cannot be recorded, causing the evolution
/// chamber to stall (see BUG-035 deadlock chain).
pub struct EmergenceDetector {
    /// Ring buffer of entries (newest at back).
    buffer: parking_lot::RwLock<VecDeque<EmergenceEntry>>,
    /// Next entry ID (monotonically increasing).
    next_id: AtomicU64,
    /// TTL for entries in seconds.
    ttl_secs: u64,
    /// Occurrence count per content hash (for threshold detection).
    occurrence_counts: parking_lot::RwLock<HashMap<u64, u32>>,
}
```

### Key Functions

- `EmergenceDetector::new(ttl_secs: u64) -> Self` -- Construct with TTL (default: 3,600s).
- `record(&self, pattern: String, parameters: Vec<String>, confidence: f64, now: Timestamp) -> Result<u64, OracError>` -- Record a pattern observation. Purges expired entries first. Evicts oldest if at cap. Returns entry ID.
- `detect_emergences(&self, now: Timestamp) -> Vec<Emergence>` -- Scan buffer for patterns exceeding `EMERGENCE_THRESHOLD` within TTL. Returns confirmed emergences.
- `purge_expired(&self, now: Timestamp) -> usize` -- Remove entries past their TTL. Returns count of purged entries.
- `len(&self) -> usize` -- Current buffer size (always <= `EMERGENCE_CAP`).
- `content_hash(pattern: &str, parameters: &[String]) -> u64` -- FNV-1a hash for deduplication.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_buffer_cap_enforced` | unit | Buffer never exceeds 5,000 entries |
| `test_ttl_expiry_purge` | unit | Expired entries removed on purge |
| `test_lazy_purge_on_insert` | unit | Expired entries removed during record() |
| `test_emergence_threshold_met` | unit | 3 observations -> emergence detected |
| `test_emergence_threshold_not_met` | unit | 2 observations -> no emergence |
| `test_deduplication_by_hash` | unit | Same pattern hashes to same value |
| `test_different_patterns_different_hash` | unit | Distinct patterns produce distinct hashes |
| `test_mean_confidence_computation` | unit | Mean across 3 observations is correct |
| `test_oldest_evicted_at_cap` | unit | At 5,000, oldest entry is evicted on insert |
| `test_empty_buffer_detect` | unit | No emergences from empty buffer |
| `test_buffer_never_exceeds_cap` | property | 10,000 random inserts -> len <= 5,000 |
| `test_all_expired_yields_empty` | property | All entries past TTL -> detect returns empty |

### Cross-References

- `m36_ralph_engine` -- Recognize phase queries `detect_emergences()`
- `m38_correlation_engine` -- uses confirmed emergences as correlation input
- AP19 (emergence cap exhaustion) -- ANTI_PATTERNS.md
- ORAC_PLAN.md Phase 4 (emergence cap: 5,000 with TTL decay)
- Obsidian: `[[ORAC -- RALPH Multi-Parameter Mutation Fix]]`

---

## m38 -- Correlation Engine

**Source:** `src/m8_evolution/m38_correlation_engine.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types`, `m02_error_handling`, `m37_emergence_detector`

### Design Decisions

- Discovers causal pathways between parameter changes and fitness outcomes
- Pearson correlation coefficient computed over sliding window (last 100 observations)
- Significant correlation: |r| >= 0.3 with p < 0.05
- Pathway graph stored as adjacency list (parameter -> parameter edges with correlation weight)
- Negative correlations are equally important (parameter A increases -> parameter B should decrease)
- All float arithmetic uses FMA (pattern P05)
- Correlation matrix is recomputed on each Analyze phase (not incrementally updated)

### Types to Implement

```rust
/// A correlation between two parameters.
///
/// Pearson r with significance test (p < 0.05).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Correlation {
    /// First parameter name.
    pub param_a: String,
    /// Second parameter name.
    pub param_b: String,
    /// Pearson correlation coefficient in [-1.0, 1.0].
    pub pearson_r: f64,
    /// Number of observations in the window.
    pub n_observations: u32,
    /// Whether correlation is statistically significant (p < 0.05).
    pub significant: bool,
    /// Direction: positive or negative correlation.
    pub direction: CorrelationDirection,
}

/// Direction of correlation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum CorrelationDirection {
    /// Both parameters move in the same direction.
    Positive,
    /// Parameters move in opposite directions.
    Negative,
}

/// A causal pathway discovered between parameters.
///
/// Chain of correlated parameters: A -> B -> C means
/// changes to A correlate with B, which correlates with C.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Pathway {
    /// Ordered chain of parameter names.
    pub chain: Vec<String>,
    /// Compound correlation (product of edge correlations).
    pub compound_r: f64,
    /// Number of edges in the chain.
    pub depth: u32,
}

/// Time-series observation for correlation computation.
///
/// Recorded at each tick or RALPH iteration.
#[derive(Debug, Clone)]
pub struct ParameterObservation {
    /// Parameter name.
    pub name: String,
    /// Observed value.
    pub value: f64,
    /// When observed.
    pub timestamp: Timestamp,
}

/// Correlation engine for pathway discovery.
///
/// Maintains a sliding window of parameter observations
/// and computes pairwise Pearson correlations.
pub struct CorrelationEngine {
    /// Sliding window of observations per parameter.
    /// Key: parameter name, Value: ring buffer of values.
    windows: parking_lot::RwLock<HashMap<String, VecDeque<f64>>>,
    /// Window size (default: 100 observations).
    window_size: usize,
    /// Minimum |r| for significance (default: 0.3).
    min_correlation: f64,
    /// Discovered pathways.
    pathways: parking_lot::RwLock<Vec<Pathway>>,
}
```

### Key Functions

- `CorrelationEngine::new(window_size: usize, min_correlation: f64) -> Self` -- Construct with sliding window configuration.
- `record_observation(&self, name: &str, value: f64)` -- Append a value to the parameter's sliding window.
- `compute_correlations(&self) -> Vec<Correlation>` -- Compute pairwise Pearson r across all parameters. Returns only significant correlations (|r| >= 0.3).
- `discover_pathways(&self, correlations: &[Correlation], max_depth: u32) -> Vec<Pathway>` -- BFS/DFS through correlation graph to find causal chains.
- `pearson_r(x: &[f64], y: &[f64]) -> f64` -- Compute Pearson correlation coefficient using FMA.
- `clear_windows(&self)` -- Reset all sliding windows (on RALPH cycle restart).

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_pearson_perfect_positive` | unit | r = 1.0 for identical series |
| `test_pearson_perfect_negative` | unit | r = -1.0 for negated series |
| `test_pearson_uncorrelated` | unit | r near 0.0 for random series |
| `test_pearson_uses_fma` | unit | FMA result matches expected precision |
| `test_significant_threshold` | unit | |r| < 0.3 filtered out |
| `test_sliding_window_cap` | unit | Window never exceeds configured size |
| `test_pathway_depth_limit` | unit | Pathways respect max_depth parameter |
| `test_pathway_compound_r` | unit | Compound r = product of edge r values |
| `test_empty_windows_no_correlations` | unit | No observations -> empty result |
| `test_single_parameter_no_correlations` | unit | Need >= 2 parameters for correlation |
| `test_record_and_compute_roundtrip` | integration | Record 100 observations, compute correlations |

### Cross-References

- `m36_ralph_engine` -- Analyze phase calls `compute_correlations()`
- `m37_emergence_detector` -- emergences feed into correlation analysis
- `m39_fitness_tensor` -- fitness dimensions correlate with parameter changes
- ORAC_PLAN.md Phase 4 Detail (step 3)

---

## m39 -- Fitness Tensor

**Source:** `src/m8_evolution/m39_fitness_tensor.rs`
**LOC Target:** ~450
**Depends on:** `m01_core_types`, `m02_error_handling`

### Design Decisions

- 12-dimensional fitness evaluation covering all ORAC coordination axes
- All tensor operations use FMA (pattern P01/P05) -- no bare `a * b + c`
- Dimension weights are configurable and sum to 1.0 (validated on construction)
- Fitness value in [0.0, 1.0] -- clamped after computation
- Improvement threshold: mutation applied only if fitness improves by >= 2%
- Thread-safe: `AtomicU64` for individual dimensions (stored as f64 bits)
- Each module in ORAC implements `TensorContributor` to report its fitness dimension

### Fitness Dimensions

| # | Dimension | Weight | Source |
|---|-----------|--------|--------|
| 0 | `order_parameter` | 0.15 | Kuramoto r from field state |
| 1 | `coupling_health` | 0.12 | Auto-K stability from m16 |
| 2 | `hebbian_coverage` | 0.10 | Fraction of sphere pairs with weight > 0.1 |
| 3 | `circuit_health` | 0.10 | Fraction of panes with circuit breaker Closed |
| 4 | `bridge_latency` | 0.08 | Mean bridge response time (inverse) |
| 5 | `token_efficiency` | 0.08 | Output tokens per tool call |
| 6 | `task_throughput` | 0.08 | Tasks completed per minute |
| 7 | `emergence_rate` | 0.07 | Confirmed emergences per RALPH cycle |
| 8 | `convergence_speed` | 0.06 | Iterations to convergence |
| 9 | `mutation_diversity` | 0.06 | Unique parameters targeted in last 20 mutations |
| 10 | `thermal_stability` | 0.05 | SYNTHEX temperature variance (inverse) |
| 11 | `fleet_utilisation` | 0.05 | Fraction of spheres in Working status |

### Types to Implement

```rust
/// Number of fitness dimensions.
pub const FITNESS_DIMS: usize = 12;

/// Minimum improvement required to commit a mutation (2%).
pub const IMPROVEMENT_THRESHOLD: f64 = 0.02;

/// A single fitness dimension with its current value and weight.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FitnessDimension {
    /// Dimension name (e.g., `order_parameter`).
    pub name: &'static str,
    /// Current value in [0.0, 1.0].
    pub value: f64,
    /// Weight for weighted sum (all weights sum to 1.0).
    pub weight: f64,
    /// Weighted contribution: `value * weight` (computed via FMA).
    pub contribution: f64,
}

/// 12-dimensional fitness tensor.
///
/// All operations use FMA. Weights validated to sum to 1.0.
/// Thread-safe: dimensions stored as `AtomicU64` (f64 bits).
pub struct FitnessTensor {
    /// Dimension values stored as f64 bits in `AtomicU64`.
    values: [AtomicU64; FITNESS_DIMS],
    /// Dimension weights (immutable after construction).
    weights: [f64; FITNESS_DIMS],
    /// Dimension names (immutable after construction).
    names: [&'static str; FITNESS_DIMS],
}

/// Snapshot of the fitness tensor at a point in time.
///
/// Used for before/after comparison in the Harvest phase.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FitnessSnapshot {
    /// Per-dimension breakdown.
    pub dimensions: Vec<FitnessDimension>,
    /// Weighted sum of all dimensions (scalar fitness).
    pub total: f64,
    /// When this snapshot was taken.
    pub timestamp: Timestamp,
}

/// Trait for modules that contribute to the fitness tensor.
///
/// Every ORAC module implements this to report its fitness dimension.
pub trait TensorContributor: Send + Sync {
    /// Return the fitness dimension name this module contributes to.
    fn dimension_name(&self) -> &'static str;

    /// Compute the current fitness value for this dimension.
    ///
    /// # Errors
    ///
    /// Returns `OracError::FitnessComputation` if the value cannot be computed.
    fn compute_fitness(&self) -> Result<f64, OracError>;
}
```

### Key Functions

- `FitnessTensor::new(weights: [f64; FITNESS_DIMS]) -> Result<Self, OracError>` -- Construct and validate weights sum to 1.0 (tolerance 1e-9).
- `set_dimension(&self, index: usize, value: f64) -> Result<(), OracError>` -- Set a dimension value. Clamps to [0.0, 1.0].
- `compute_total(&self) -> f64` -- Weighted sum using FMA: `w[0].mul_add(v[0], w[1].mul_add(v[1], ...))`.
- `snapshot(&self) -> FitnessSnapshot` -- Capture current state as an immutable snapshot.
- `compare(before: &FitnessSnapshot, after: &FitnessSnapshot) -> f64` -- Compute fitness delta. Positive = improvement.
- `meets_threshold(before: &FitnessSnapshot, after: &FitnessSnapshot) -> bool` -- Returns true if improvement >= `IMPROVEMENT_THRESHOLD` (2%).
- `collect_from_contributors(&self, contributors: &[Arc<dyn TensorContributor>]) -> Result<(), OracError>` -- Poll all contributors and update dimensions.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_weights_must_sum_to_one` | unit | Construction fails if weights != 1.0 |
| `test_dimension_clamped_to_unit` | unit | Values outside [0,1] are clamped |
| `test_total_uses_fma` | unit | FMA result matches manual computation to 1e-12 |
| `test_total_all_zeros` | unit | All zeros -> total = 0.0 |
| `test_total_all_ones` | unit | All ones -> total = 1.0 (since weights sum to 1.0) |
| `test_snapshot_immutable` | unit | Snapshot is independent of subsequent mutations |
| `test_compare_improvement` | unit | Positive delta for improvement |
| `test_compare_regression` | unit | Negative delta for regression |
| `test_threshold_met` | unit | 2% improvement passes threshold |
| `test_threshold_not_met` | unit | 1.9% improvement does not pass |
| `test_fma_chain_precision` | property | FMA chain within 1e-15 of exact arithmetic |
| `test_concurrent_dimension_updates` | integration | 12 threads updating different dimensions |

### Cross-References

- `m36_ralph_engine` -- Harvest phase uses `compare()` and `meets_threshold()`
- `m40_mutation_selector` -- mutation diversity dimension feeds back into selector quality
- `m34_field_dashboard` -- order_parameter dimension reads from dashboard
- ORAC_PLAN.md Phase 4 Detail (step 4)
- GOLD_STANDARD_PATTERNS.md P05 (FMA for float precision)

---

## m40 -- Mutation Selector

**Source:** `src/m8_evolution/m40_mutation_selector.rs`
**LOC Target:** ~600
**Depends on:** `m01_core_types`, `m02_error_handling`, `m36_ralph_engine`

### Design Decisions

- **P20 (multi-parameter mutation):** This module is the CRITICAL fix for BUG-035 / AP12
- **Round-robin:** parameters are targeted in cyclic order, not by weight or fitness gradient
- **10-generation cooldown:** a parameter cannot be targeted again for 10 RALPH generations after mutation
- **>50% rejection gate:** reject any proposal if >50% of the last 20 mutations targeted the same parameter
- **Snapshot before mutation:** atomic state capture; rollback if fitness regresses
- **Parameter pool:** all tunable coordination parameters registered at startup
- Mutation magnitude: proportional to fitness distance from target (larger distance -> larger mutation)
- Mutation direction: informed by correlation engine (m38) lessons

### Types to Implement

```rust
/// Size of the diversity window for rejection gate.
pub const DIVERSITY_WINDOW: usize = 20;

/// Maximum fraction of window that can target same parameter (50%).
pub const DIVERSITY_THRESHOLD: f64 = 0.5;

/// Minimum generations between repeated targeting of same parameter.
pub const COOLDOWN_GENERATIONS: u32 = 10;

/// A tunable coordination parameter.
///
/// Registered at startup. The mutation selector cycles through these.
#[derive(Debug, Clone)]
pub struct TunableParameter {
    /// Parameter name (e.g., `k_mod`, `ltp_rate`, `burst_multiplier`).
    pub name: String,
    /// Current value.
    pub current_value: f64,
    /// Minimum allowed value.
    pub min_value: f64,
    /// Maximum allowed value.
    pub max_value: f64,
    /// Default value (for rollback).
    pub default_value: f64,
    /// Generation when this parameter was last mutated.
    pub last_mutated_gen: Option<u32>,
}

/// A proposed mutation to a parameter.
///
/// Created by the selector, evaluated by the fitness tensor.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MutationProposal {
    /// Which parameter to mutate.
    pub parameter_name: String,
    /// Current value before mutation.
    pub old_value: f64,
    /// Proposed new value.
    pub new_value: f64,
    /// Mutation magnitude (absolute delta).
    pub magnitude: f64,
    /// Direction of mutation.
    pub direction: MutationDirection,
    /// Which generation proposed this.
    pub generation: u32,
    /// Reason for this mutation (from lessons).
    pub rationale: String,
}

/// Atomic snapshot of all parameter values.
///
/// Taken before Propose phase. Restored on rollback.
#[derive(Debug, Clone)]
pub struct ParameterSnapshot {
    /// Parameter name -> value at snapshot time.
    pub values: HashMap<String, f64>,
    /// Generation number at snapshot time.
    pub generation: u32,
    /// Timestamp of snapshot.
    pub timestamp: Timestamp,
}

/// Rejection reason when a mutation proposal is rejected.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum RejectionReason {
    /// Parameter is in cooldown (last mutated < 10 generations ago).
    Cooldown {
        /// Parameter name.
        parameter: String,
        /// Generations remaining in cooldown.
        remaining: u32,
    },
    /// Diversity gate triggered (>50% of last 20 mutations hit same parameter).
    DiversityGate {
        /// The over-targeted parameter.
        parameter: String,
        /// Fraction of window targeting this parameter.
        fraction: f64,
    },
    /// Fitness did not improve by >= 2%.
    InsufficientImprovement {
        /// Actual improvement percentage.
        actual_pct: f64,
    },
}

/// Diversity-enforced mutation selector.
///
/// Prevents mono-parameter fixation (BUG-035 / AP12).
/// Uses round-robin, 10-gen cooldown, and >50% rejection gate (P20).
pub struct MutationSelector {
    /// Registered tunable parameters.
    parameters: parking_lot::RwLock<Vec<TunableParameter>>,
    /// Round-robin index.
    round_robin_index: AtomicU64,
    /// Current generation counter.
    generation: AtomicU64,
    /// Sliding window of recent mutation targets (parameter names).
    mutation_history: parking_lot::RwLock<VecDeque<String>>,
    /// Snapshot storage for rollback.
    snapshots: parking_lot::RwLock<Option<ParameterSnapshot>>,
}
```

### Key Functions

- `MutationSelector::new() -> Self` -- Construct with empty parameter pool.
- `register_parameter(&self, param: TunableParameter) -> Result<(), OracError>` -- Register a tunable parameter. Duplicate names are rejected.
- `select_target(&self) -> Result<String, OracError>` -- Select the next parameter to mutate using round-robin. Skips parameters in cooldown. Returns `OracError::AllParametersInCooldown` if none eligible.
- `check_diversity_gate(&self, parameter: &str) -> Result<(), RejectionReason>` -- Check if targeting this parameter would breach the 50% diversity threshold.
- `propose_mutation(&self, parameter: &str, direction: MutationDirection, magnitude: f64) -> Result<MutationProposal, OracError>` -- Create a mutation proposal. Validates bounds, checks cooldown, checks diversity gate.
- `snapshot(&self) -> ParameterSnapshot` -- Capture current values of all parameters.
- `rollback(&self, snapshot: &ParameterSnapshot) -> Result<(), OracError>` -- Restore all parameter values from snapshot.
- `commit(&self, proposal: &MutationProposal) -> Result<(), OracError>` -- Apply mutation, update cooldown tracker, push to history window.
- `diversity_score(&self) -> f64` -- Fraction of unique parameters in the last 20 mutations. 1.0 = perfect diversity.

### Tests

| Test | Kind | Description |
|------|------|-------------|
| `test_round_robin_cycles` | unit | Parameters targeted in cyclic order |
| `test_cooldown_enforced` | unit | Parameter skipped within 10 generations |
| `test_cooldown_expires` | unit | Parameter eligible after 10 generations |
| `test_diversity_gate_rejects` | unit | >50% window targeting same param -> rejected |
| `test_diversity_gate_passes` | unit | <=50% window targeting same param -> passes |
| `test_snapshot_captures_all` | unit | All registered parameters in snapshot |
| `test_rollback_restores_values` | unit | Values match pre-mutation state after rollback |
| `test_commit_updates_cooldown` | unit | Last-mutated generation updated on commit |
| `test_commit_pushes_to_history` | unit | Mutation target added to sliding window |
| `test_history_window_cap` | unit | Window never exceeds 20 entries |
| `test_register_duplicate_rejected` | unit | Duplicate parameter name returns error |
| `test_select_all_in_cooldown` | unit | Returns error when no parameter eligible |
| `test_bounds_enforced` | unit | Mutation clamped to [min, max] |
| `test_diversity_score_all_unique` | unit | 20 unique params -> score = 1.0 |
| `test_diversity_score_all_same` | unit | 20 identical params -> score = 0.05 |
| `test_bug035_scenario_prevented` | integration | Simulate 380 mutations: no parameter exceeds 50% |

### Cross-References

- `m36_ralph_engine` -- Propose phase calls `select_target()` and `propose_mutation()`
- `m39_fitness_tensor` -- `mutation_diversity` dimension tracks selector quality
- `m38_correlation_engine` -- lessons inform mutation direction
- P20 (multi-parameter mutation) -- GOLD_STANDARD_PATTERNS.md
- AP12 (mono-parameter BUG-035) -- ANTI_PATTERNS.md
- AP19 (emergence cap exhaustion) -- ANTI_PATTERNS.md
- ORAC_PLAN.md Phase 4 Detail (critical warning block)
- Obsidian: `[[ORAC -- RALPH Multi-Parameter Mutation Fix]]`
- Obsidian: `[[ULTRAPLATE -- Bugs and Known Issues]]` (BUG-035)

---

## Cross-References (Layer-Wide)

- [layers/L8_EVOLUTION.md](../layers/L8_EVOLUTION.md) -- Layer overview
- [modules/INDEX.md](INDEX.md) -- Module index (m36-m40)
- [GOLD_STANDARD_PATTERNS.md](../GOLD_STANDARD_PATTERNS.md) -- P01/P05 (FMA), P07 (owned returns), P10 (feature gates), P20 (multi-parameter mutation)
- [ANTI_PATTERNS.md](../ANTI_PATTERNS.md) -- AP12 (mono-parameter BUG-035), AP19 (emergence cap exhaustion), A17 (invariant group mutation)
- ORAC_PLAN.md Phase 4 Detail
- ORAC_MINDMAP.md Branch 4 (RALPH Evolution Chamber)
- ME source: `~/claude-code-workspace/the_maintenance_engine/` (original RALPH, 54K LOC)
- ME V2 source: `~/claude-code-workspace/the_maintenance_engine_v2/` (gold standard, 56K LOC)
- Obsidian: `[[Session 050 -- ME Evolution Chamber Spec]]`
- Obsidian: `[[ME RALPH Loop Specification]]`
- Obsidian: `[[ORAC -- RALPH Multi-Parameter Mutation Fix]]`
- Obsidian: `[[ULTRAPLATE -- Bugs and Known Issues]]` (BUG-035)
