//! # M39: Fitness Tensor
//!
//! 12-dimensional weighted fitness evaluation for the RALPH evolution chamber.
//! Evaluates fleet coordination health across named dimensions with weighted
//! scoring, trend detection, and stability assessment.
//!
//! ## Layer: L8 (Evolution)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//!
//! ## Dimension Layout (ORAC-specific)
//!
//! | D# | Name | Weight | Category |
//! |----|------|--------|----------|
//! | D0 | `coordination_quality` | 0.18 | Primary |
//! | D1 | `field_coherence` | 0.15 | Primary |
//! | D2 | `dispatch_accuracy` | 0.12 | Primary |
//! | D3 | `task_throughput` | 0.10 | Secondary |
//! | D4 | `error_rate` | 0.10 | Secondary |
//! | D5 | `latency` | 0.08 | Secondary |
//! | D6 | `hebbian_health` | 0.07 | Learning |
//! | D7 | `coupling_stability` | 0.06 | Learning |
//! | D8 | `thermal_balance` | 0.05 | Context |
//! | D9 | `fleet_utilization` | 0.04 | Context |
//! | D10 | `emergence_rate` | 0.03 | Context |
//! | D11 | `consent_compliance` | 0.02 | Context |

use std::collections::VecDeque;
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Number of fitness dimensions.
pub const DIMENSION_COUNT: usize = 12;

/// Dimension weights for ORAC fleet coordination fitness scoring.
/// Sum = 1.0.
pub const DIMENSION_WEIGHTS: [f64; DIMENSION_COUNT] = [
    0.18, // D0: coordination_quality (PRIMARY)
    0.15, // D1: field_coherence (PRIMARY)
    0.12, // D2: dispatch_accuracy (PRIMARY)
    0.10, // D3: task_throughput (SECONDARY)
    0.10, // D4: error_rate (SECONDARY, inverted: lower=better)
    0.08, // D5: latency (SECONDARY, inverted: lower=better)
    0.07, // D6: hebbian_health (LEARNING)
    0.06, // D7: coupling_stability (LEARNING)
    0.05, // D8: thermal_balance (CONTEXT)
    0.04, // D9: fleet_utilization (CONTEXT)
    0.03, // D10: emergence_rate (CONTEXT)
    0.02, // D11: consent_compliance (CONTEXT)
];

/// Dimension names for human-readable reports.
pub const DIMENSION_NAMES: [&str; DIMENSION_COUNT] = [
    "coordination_quality",
    "field_coherence",
    "dispatch_accuracy",
    "task_throughput",
    "error_rate",
    "latency",
    "hebbian_health",
    "coupling_stability",
    "thermal_balance",
    "fleet_utilization",
    "emergence_rate",
    "consent_compliance",
];

/// Default fitness history capacity.
const DEFAULT_HISTORY_CAPACITY: usize = 200;

/// Default trend window size.
const DEFAULT_TREND_WINDOW: usize = 10;

/// Standard deviation below which fitness is considered stable.
const DEFAULT_STABILITY_TOLERANCE: f64 = 0.02;

/// Standard deviation above which fitness is considered volatile.
const DEFAULT_VOLATILITY_THRESHOLD: f64 = 0.10;

/// Minimum fitness improvement threshold for RALPH to accept a mutation.
const DEFAULT_MIN_IMPROVEMENT: f64 = 0.02;

// ──────────────────────────────────────────────────────────────
// Enums
// ──────────────────────────────────────────────────────────────

/// Fitness trend direction over the trend window.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitnessTrend {
    /// System fitness is improving.
    Improving,
    /// System fitness is stable.
    Stable,
    /// System fitness is declining.
    Declining,
    /// Not enough data to determine trend.
    #[default]
    Unknown,
}

impl fmt::Display for FitnessTrend {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Improving => f.write_str("improving"),
            Self::Stable => f.write_str("stable"),
            Self::Declining => f.write_str("declining"),
            Self::Unknown => f.write_str("unknown"),
        }
    }
}

/// System state classification based on overall fitness score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemState {
    /// Fitness >= 0.9 — fleet is thriving.
    Optimal,
    /// 0.7 <= fitness < 0.9 — normal operation.
    Healthy,
    /// 0.5 <= fitness < 0.7 — some degradation.
    Degraded,
    /// 0.3 <= fitness < 0.5 — significant issues.
    Critical,
    /// Fitness < 0.3 — fleet failure.
    #[default]
    Failed,
}

impl SystemState {
    /// Classify system state from a fitness score.
    #[must_use]
    pub fn from_fitness(fitness: f64) -> Self {
        if fitness >= 0.9 {
            Self::Optimal
        } else if fitness >= 0.7 {
            Self::Healthy
        } else if fitness >= 0.5 {
            Self::Degraded
        } else if fitness >= 0.3 {
            Self::Critical
        } else {
            Self::Failed
        }
    }
}

impl fmt::Display for SystemState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Optimal => f.write_str("optimal"),
            Self::Healthy => f.write_str("healthy"),
            Self::Degraded => f.write_str("degraded"),
            Self::Critical => f.write_str("critical"),
            Self::Failed => f.write_str("failed"),
        }
    }
}

/// Named fitness dimension for type-safe indexing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum FitnessDimension {
    /// D0: Quality of cross-pane coordination.
    CoordinationQuality = 0,
    /// D1: Kuramoto order parameter (r) coherence.
    FieldCoherence = 1,
    /// D2: Accuracy of semantic dispatch decisions.
    DispatchAccuracy = 2,
    /// D3: Task completion throughput.
    TaskThroughput = 3,
    /// D4: Error rate (inverted: 1.0 - raw rate).
    ErrorRate = 4,
    /// D5: Response latency (inverted: 1.0 - normalized latency).
    Latency = 5,
    /// D6: Health of Hebbian STDP learning.
    HebbianHealth = 6,
    /// D7: Stability of coupling weights.
    CouplingStability = 7,
    /// D8: Thermal regulation balance.
    ThermalBalance = 8,
    /// D9: Fleet pane utilization.
    FleetUtilization = 9,
    /// D10: Rate of beneficial emergences.
    EmergenceRate = 10,
    /// D11: Consent posture compliance.
    ConsentCompliance = 11,
}

impl FitnessDimension {
    /// Returns the zero-based index of this dimension.
    #[must_use]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Returns the human-readable name of this dimension.
    #[must_use]
    pub const fn name(self) -> &'static str {
        DIMENSION_NAMES[self as usize]
    }

    /// Returns the weight of this dimension.
    #[must_use]
    pub fn weight(self) -> f64 {
        DIMENSION_WEIGHTS[self as usize]
    }

    /// Attempt to create a `FitnessDimension` from an index.
    ///
    /// # Errors
    /// Returns [`PvError::OutOfRange`] if `idx` >= 12.
    pub fn from_index(idx: usize) -> PvResult<Self> {
        match idx {
            0 => Ok(Self::CoordinationQuality),
            1 => Ok(Self::FieldCoherence),
            2 => Ok(Self::DispatchAccuracy),
            3 => Ok(Self::TaskThroughput),
            4 => Ok(Self::ErrorRate),
            5 => Ok(Self::Latency),
            6 => Ok(Self::HebbianHealth),
            7 => Ok(Self::CouplingStability),
            8 => Ok(Self::ThermalBalance),
            9 => Ok(Self::FleetUtilization),
            10 => Ok(Self::EmergenceRate),
            11 => Ok(Self::ConsentCompliance),
            _ => {
                #[allow(clippy::cast_precision_loss)] // idx is small (display only)
                let idx_f = idx as f64;
                Err(PvError::OutOfRange {
                    field: "dimension_index",
                    value: idx_f,
                    min: 0.0,
                    max: 11.0,
                })
            }
        }
    }

    /// All dimensions in order.
    #[must_use]
    pub const fn all() -> [Self; DIMENSION_COUNT] {
        [
            Self::CoordinationQuality,
            Self::FieldCoherence,
            Self::DispatchAccuracy,
            Self::TaskThroughput,
            Self::ErrorRate,
            Self::Latency,
            Self::HebbianHealth,
            Self::CouplingStability,
            Self::ThermalBalance,
            Self::FleetUtilization,
            Self::EmergenceRate,
            Self::ConsentCompliance,
        ]
    }
}

impl fmt::Display for FitnessDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ──────────────────────────────────────────────────────────────
// Data structures
// ──────────────────────────────────────────────────────────────

/// Raw 12-dimensional tensor values.
///
/// All values are normalized to [0.0, 1.0]. Inverted dimensions
/// (`error_rate`, `latency`) should be pre-inverted before setting.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TensorValues {
    /// Raw dimension values.
    pub values: [f64; DIMENSION_COUNT],
}

impl TensorValues {
    /// Create a new tensor with all dimensions zeroed.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            values: [0.0; DIMENSION_COUNT],
        }
    }

    /// Create a new tensor with all dimensions set to the given value.
    #[must_use]
    pub const fn uniform(v: f64) -> Self {
        Self {
            values: [v; DIMENSION_COUNT],
        }
    }

    /// Get the value of a specific dimension.
    #[must_use]
    pub fn get(&self, dim: FitnessDimension) -> f64 {
        self.values[dim.index()]
    }

    /// Set the value of a specific dimension.
    pub fn set(&mut self, dim: FitnessDimension, value: f64) {
        self.values[dim.index()] = value.clamp(0.0, 1.0);
    }

    /// Validate that all values are finite and in [0.0, 1.0].
    ///
    /// # Errors
    /// Returns [`PvError::NonFinite`] or [`PvError::OutOfRange`] on invalid values.
    pub fn validate(&self) -> PvResult<()> {
        for (i, &v) in self.values.iter().enumerate() {
            if !v.is_finite() {
                return Err(PvError::NonFinite {
                    field: DIMENSION_NAMES[i],
                    value: v,
                });
            }
            if !(0.0..=1.0).contains(&v) {
                return Err(PvError::OutOfRange {
                    field: DIMENSION_NAMES[i],
                    value: v,
                    min: 0.0,
                    max: 1.0,
                });
            }
        }
        Ok(())
    }
}

impl Default for TensorValues {
    fn default() -> Self {
        Self::zero()
    }
}

/// A fitness evaluation report at a point in time.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessReport {
    /// Raw tensor values.
    pub tensor: TensorValues,
    /// Overall weighted fitness score [0.0, 1.0].
    pub overall_score: f64,
    /// Per-dimension weighted contributions.
    pub weighted_contributions: [f64; DIMENSION_COUNT],
    /// System state classification.
    pub system_state: SystemState,
    /// Current fitness trend.
    pub trend: FitnessTrend,
    /// Tick at which this report was generated.
    pub tick: u64,
}

/// A point-in-time fitness snapshot for history tracking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessSnapshot {
    /// Tick number.
    pub tick: u64,
    /// Overall fitness score.
    pub fitness: f64,
    /// Full tensor values.
    pub tensor: [f64; DIMENSION_COUNT],
    /// RALPH generation number, if during an active evolution cycle.
    pub generation: Option<u64>,
}

/// Configuration for the `FitnessTensor` evaluator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FitnessTensorConfig {
    /// Maximum snapshot history capacity.
    pub history_capacity: usize,
    /// Window size for trend calculation.
    pub trend_window: usize,
    /// Standard deviation below which fitness is "stable".
    pub stability_tolerance: f64,
    /// Standard deviation above which fitness is "volatile".
    pub volatility_threshold: f64,
    /// Minimum fitness improvement for RALPH to accept a mutation.
    pub min_improvement: f64,
    /// Custom dimension weights (overrides defaults if set).
    pub custom_weights: Option<[f64; DIMENSION_COUNT]>,
}

impl Default for FitnessTensorConfig {
    fn default() -> Self {
        Self {
            history_capacity: DEFAULT_HISTORY_CAPACITY,
            trend_window: DEFAULT_TREND_WINDOW,
            stability_tolerance: DEFAULT_STABILITY_TOLERANCE,
            volatility_threshold: DEFAULT_VOLATILITY_THRESHOLD,
            min_improvement: DEFAULT_MIN_IMPROVEMENT,
            custom_weights: None,
        }
    }
}

/// Aggregate statistics for the `FitnessTensor`.
#[derive(Clone, Debug, Default)]
pub struct FitnessTensorStats {
    /// Total evaluations performed.
    pub total_evaluations: u64,
    /// Peak fitness score observed.
    pub peak_fitness: f64,
    /// Lowest fitness score observed.
    pub trough_fitness: f64,
    /// Most recent fitness score.
    pub current_fitness: f64,
    /// Current trend.
    pub current_trend: FitnessTrend,
}

// ──────────────────────────────────────────────────────────────
// FitnessTensor evaluator
// ──────────────────────────────────────────────────────────────

/// 12-dimensional weighted fitness evaluator for ORAC fleet coordination.
///
/// Evaluates system health across named dimensions with configurable weights,
/// trend detection via linear regression over a sliding window, and stability
/// assessment.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct FitnessTensor {
    /// Fitness snapshot history (bounded ring buffer).
    history: RwLock<VecDeque<FitnessSnapshot>>,
    /// Active dimension weights (defaults or custom).
    weights: [f64; DIMENSION_COUNT],
    /// Configuration.
    config: FitnessTensorConfig,
    /// Aggregate statistics.
    stats: RwLock<FitnessTensorStats>,
}

impl fmt::Debug for FitnessTensor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FitnessTensor")
            .field("history_len", &self.history.read().len())
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl FitnessTensor {
    /// Creates a new `FitnessTensor` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(FitnessTensorConfig::default())
    }

    /// Creates a new `FitnessTensor` with the given configuration.
    #[must_use]
    pub fn with_config(config: FitnessTensorConfig) -> Self {
        let weights = config.custom_weights.unwrap_or(DIMENSION_WEIGHTS);
        Self {
            history: RwLock::new(VecDeque::with_capacity(
                config.history_capacity.min(1024),
            )),
            weights,
            config,
            stats: RwLock::new(FitnessTensorStats {
                trough_fitness: f64::MAX,
                ..FitnessTensorStats::default()
            }),
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if any parameter is invalid.
    pub fn validate_config(config: &FitnessTensorConfig) -> PvResult<()> {
        if config.history_capacity == 0 {
            return Err(PvError::ConfigValidation(
                "history_capacity must be > 0".into(),
            ));
        }
        if config.trend_window == 0 {
            return Err(PvError::ConfigValidation(
                "trend_window must be > 0".into(),
            ));
        }
        if config.stability_tolerance < 0.0 || !config.stability_tolerance.is_finite() {
            return Err(PvError::ConfigValidation(
                "stability_tolerance must be >= 0 and finite".into(),
            ));
        }
        if config.volatility_threshold < 0.0 || !config.volatility_threshold.is_finite() {
            return Err(PvError::ConfigValidation(
                "volatility_threshold must be >= 0 and finite".into(),
            ));
        }
        if let Some(ref w) = config.custom_weights {
            let sum: f64 = w.iter().sum();
            if (sum - 1.0).abs() > 0.01 {
                return Err(PvError::ConfigValidation(
                    format!("custom_weights must sum to 1.0, got {sum:.4}"),
                ));
            }
            for (i, &wv) in w.iter().enumerate() {
                if !wv.is_finite() || wv < 0.0 {
                    return Err(PvError::ConfigValidation(
                        format!("custom_weights[{i}] must be >= 0 and finite"),
                    ));
                }
            }
        }
        Ok(())
    }

    /// Evaluate a tensor and produce a fitness report.
    ///
    /// Records the evaluation in history and updates stats/trend.
    ///
    /// # Errors
    /// Returns [`PvError`] if the tensor values are invalid.
    pub fn evaluate(&self, tensor: &TensorValues, tick: u64, generation: Option<u64>) -> PvResult<FitnessReport> {
        tensor.validate()?;

        let mut weighted = [0.0_f64; DIMENSION_COUNT];
        let mut overall = 0.0_f64;
        for (i, w) in weighted.iter_mut().enumerate() {
            *w = tensor.values[i] * self.weights[i];
            overall += *w;
        }

        // Record snapshot
        let snapshot = FitnessSnapshot {
            tick,
            fitness: overall,
            tensor: tensor.values,
            generation,
        };

        {
            let mut hist = self.history.write();
            if hist.len() >= self.config.history_capacity {
                hist.pop_front();
            }
            hist.push_back(snapshot);
        }

        // Compute trend
        let trend = self.compute_trend();

        // Update stats
        {
            let mut stats = self.stats.write();
            stats.total_evaluations += 1;
            stats.current_fitness = overall;
            stats.current_trend = trend;
            if overall > stats.peak_fitness {
                stats.peak_fitness = overall;
            }
            if overall < stats.trough_fitness {
                stats.trough_fitness = overall;
            }
        }

        let system_state = SystemState::from_fitness(overall);

        Ok(FitnessReport {
            tensor: tensor.clone(),
            overall_score: overall,
            weighted_contributions: weighted,
            system_state,
            trend,
            tick,
        })
    }

    /// Compute the current fitness trend using linear regression over the trend window.
    #[must_use]
    pub fn compute_trend(&self) -> FitnessTrend {
        let hist = self.history.read();
        let window = self.config.trend_window;

        if hist.len() < 2 {
            return FitnessTrend::Unknown;
        }

        let n = hist.len().min(window);
        let start = hist.len() - n;
        let samples: Vec<f64> = hist.iter().skip(start).map(|s| s.fitness).collect();

        let slope = linear_regression_slope(&samples);

        if slope > self.config.stability_tolerance {
            FitnessTrend::Improving
        } else if slope < -self.config.stability_tolerance {
            FitnessTrend::Declining
        } else {
            FitnessTrend::Stable
        }
    }

    /// Compute the standard deviation of recent fitness scores.
    #[must_use]
    pub fn compute_volatility(&self) -> f64 {
        let hist = self.history.read();
        let window = self.config.trend_window;

        if hist.len() < 2 {
            return 0.0;
        }

        let n = hist.len().min(window);
        let start = hist.len() - n;
        let samples: Vec<f64> = hist.iter().skip(start).map(|s| s.fitness).collect();

        std_dev(&samples)
    }

    /// Returns whether the system is currently stable (low volatility).
    #[must_use]
    pub fn is_stable(&self) -> bool {
        self.compute_volatility() < self.config.stability_tolerance
    }

    /// Returns whether the system is currently volatile.
    #[must_use]
    pub fn is_volatile(&self) -> bool {
        self.compute_volatility() > self.config.volatility_threshold
    }

    /// Compute the fitness delta between two ticks.
    ///
    /// Returns `None` if either tick is not in the history.
    #[must_use]
    pub fn fitness_delta(&self, tick_before: u64, tick_after: u64) -> Option<f64> {
        let hist = self.history.read();
        let before = hist.iter().find(|s| s.tick == tick_before)?;
        let after = hist.iter().find(|s| s.tick == tick_after)?;
        Some(after.fitness - before.fitness)
    }

    /// Check whether a mutation produced sufficient improvement.
    #[must_use]
    pub fn is_improvement(&self, delta: f64) -> bool {
        delta >= self.config.min_improvement
    }

    /// Get the most recent fitness score, if any.
    #[must_use]
    pub fn current_fitness(&self) -> Option<f64> {
        self.history.read().back().map(|s| s.fitness)
    }

    /// Get the most recent `FitnessSnapshot`, if any.
    #[must_use]
    pub fn latest_snapshot(&self) -> Option<FitnessSnapshot> {
        self.history.read().back().cloned()
    }

    /// Get aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> FitnessTensorStats {
        self.stats.read().clone()
    }

    /// Get the number of snapshots in the history.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.read().len()
    }

    /// Get the dimension weights in use.
    #[must_use]
    pub const fn weights(&self) -> &[f64; DIMENSION_COUNT] {
        &self.weights
    }

    /// Get per-dimension analysis: identify the weakest and strongest dimensions.
    #[must_use]
    pub fn dimension_analysis(&self) -> Option<DimensionAnalysis> {
        let hist = self.history.read();
        let snapshot = hist.back()?;

        let mut weakest_idx = 0;
        let mut weakest_val = f64::MAX;
        let mut strongest_idx = 0;
        let mut strongest_val = f64::MIN;

        for (i, &v) in snapshot.tensor.iter().enumerate() {
            let weighted = v * self.weights[i];
            if weighted < weakest_val {
                weakest_val = weighted;
                weakest_idx = i;
            }
            if weighted > strongest_val {
                strongest_val = weighted;
                strongest_idx = i;
            }
        }

        // Safe: indices are always 0..12
        let weakest = FitnessDimension::from_index(weakest_idx).ok()?;
        let strongest = FitnessDimension::from_index(strongest_idx).ok()?;

        Some(DimensionAnalysis {
            weakest,
            weakest_weighted_score: weakest_val,
            strongest,
            strongest_weighted_score: strongest_val,
            raw_values: snapshot.tensor,
        })
    }

    /// Clear all history and reset stats.
    pub fn reset(&self) {
        self.history.write().clear();
        let mut stats = self.stats.write();
        *stats = FitnessTensorStats {
            trough_fitness: f64::MAX,
            ..FitnessTensorStats::default()
        };
    }
}

impl Default for FitnessTensor {
    fn default() -> Self {
        Self::new()
    }
}

/// Per-dimension analysis result.
#[derive(Clone, Debug)]
pub struct DimensionAnalysis {
    /// The weakest dimension (lowest weighted contribution).
    pub weakest: FitnessDimension,
    /// Weighted score of the weakest dimension.
    pub weakest_weighted_score: f64,
    /// The strongest dimension (highest weighted contribution).
    pub strongest: FitnessDimension,
    /// Weighted score of the strongest dimension.
    pub strongest_weighted_score: f64,
    /// Raw tensor values.
    pub raw_values: [f64; DIMENSION_COUNT],
}

// ──────────────────────────────────────────────────────────────
// Math helpers
// ──────────────────────────────────────────────────────────────

/// Compute the slope of a simple linear regression over evenly-spaced samples.
#[allow(clippy::cast_precision_loss)] // indices and sample counts are small
fn linear_regression_slope(samples: &[f64]) -> f64 {
    let n = samples.len();
    if n < 2 {
        return 0.0;
    }

    let nf = n as f64;
    let mut sum_x = 0.0_f64;
    let mut cross_sum = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut sum_x2 = 0.0_f64;

    for (i, &y) in samples.iter().enumerate() {
        let x = i as f64;
        sum_x += x;
        sum_y += y;
        cross_sum += x * y;
        sum_x2 += x * x;
    }

    let denom = nf.mul_add(sum_x2, -(sum_x * sum_x));
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }

    nf.mul_add(cross_sum, -(sum_x * sum_y)) / denom
}

/// Compute the standard deviation of a slice.
#[allow(clippy::cast_precision_loss)] // sample counts are small (trend window)
fn std_dev(samples: &[f64]) -> f64 {
    let n = samples.len();
    if n < 2 {
        return 0.0;
    }

    let nf = n as f64;
    let mean = samples.iter().sum::<f64>() / nf;
    let variance = samples.iter().map(|&x| (x - mean) * (x - mean)).sum::<f64>() / nf;
    variance.sqrt()
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_one() {
        let sum: f64 = DIMENSION_WEIGHTS.iter().sum();
        assert!((sum - 1.0).abs() < 0.001, "weights sum to {sum}");
    }

    #[test]
    fn dimension_count_matches() {
        assert_eq!(DIMENSION_WEIGHTS.len(), DIMENSION_COUNT);
        assert_eq!(DIMENSION_NAMES.len(), DIMENSION_COUNT);
    }

    #[test]
    fn all_weights_positive() {
        for (i, &w) in DIMENSION_WEIGHTS.iter().enumerate() {
            assert!(w > 0.0, "weight[{i}] = {w} should be positive");
        }
    }

    #[test]
    fn tensor_values_zero() {
        let t = TensorValues::zero();
        for &v in &t.values {
            assert!((v - 0.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn tensor_values_uniform() {
        let t = TensorValues::uniform(0.5);
        for &v in &t.values {
            assert!((v - 0.5).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn tensor_get_set() {
        let mut t = TensorValues::zero();
        t.set(FitnessDimension::FieldCoherence, 0.95);
        assert!((t.get(FitnessDimension::FieldCoherence) - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn tensor_set_clamps() {
        let mut t = TensorValues::zero();
        t.set(FitnessDimension::ErrorRate, 1.5);
        assert!((t.get(FitnessDimension::ErrorRate) - 1.0).abs() < f64::EPSILON);

        t.set(FitnessDimension::Latency, -0.3);
        assert!((t.get(FitnessDimension::Latency) - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn tensor_validate_ok() {
        let t = TensorValues::uniform(0.5);
        assert!(t.validate().is_ok());
    }

    #[test]
    fn tensor_validate_nan() {
        let mut t = TensorValues::uniform(0.5);
        t.values[3] = f64::NAN;
        assert!(t.validate().is_err());
    }

    #[test]
    fn tensor_validate_out_of_range() {
        let mut t = TensorValues::uniform(0.5);
        t.values[5] = 1.5;
        assert!(t.validate().is_err());
    }

    #[test]
    fn dimension_from_index_valid() {
        for i in 0..12 {
            assert!(FitnessDimension::from_index(i).is_ok());
        }
    }

    #[test]
    fn dimension_from_index_invalid() {
        assert!(FitnessDimension::from_index(12).is_err());
        assert!(FitnessDimension::from_index(100).is_err());
    }

    #[test]
    fn dimension_roundtrip() {
        for dim in FitnessDimension::all() {
            let idx = dim.index();
            let recovered = FitnessDimension::from_index(idx).unwrap();
            assert_eq!(dim, recovered);
        }
    }

    #[test]
    fn dimension_names_non_empty() {
        for name in DIMENSION_NAMES {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn system_state_boundaries() {
        assert_eq!(SystemState::from_fitness(1.0), SystemState::Optimal);
        assert_eq!(SystemState::from_fitness(0.9), SystemState::Optimal);
        assert_eq!(SystemState::from_fitness(0.89), SystemState::Healthy);
        assert_eq!(SystemState::from_fitness(0.7), SystemState::Healthy);
        assert_eq!(SystemState::from_fitness(0.69), SystemState::Degraded);
        assert_eq!(SystemState::from_fitness(0.5), SystemState::Degraded);
        assert_eq!(SystemState::from_fitness(0.49), SystemState::Critical);
        assert_eq!(SystemState::from_fitness(0.3), SystemState::Critical);
        assert_eq!(SystemState::from_fitness(0.29), SystemState::Failed);
        assert_eq!(SystemState::from_fitness(0.0), SystemState::Failed);
    }

    #[test]
    fn evaluate_uniform_tensor() {
        let ft = FitnessTensor::new();
        let t = TensorValues::uniform(0.8);
        let report = ft.evaluate(&t, 1, None).unwrap();
        assert!((report.overall_score - 0.8).abs() < 0.001);
        assert_eq!(report.system_state, SystemState::Healthy);
    }

    #[test]
    fn evaluate_zero_tensor() {
        let ft = FitnessTensor::new();
        let t = TensorValues::zero();
        let report = ft.evaluate(&t, 1, None).unwrap();
        assert!((report.overall_score - 0.0).abs() < f64::EPSILON);
        assert_eq!(report.system_state, SystemState::Failed);
    }

    #[test]
    fn evaluate_perfect_tensor() {
        let ft = FitnessTensor::new();
        let t = TensorValues::uniform(1.0);
        let report = ft.evaluate(&t, 1, None).unwrap();
        assert!((report.overall_score - 1.0).abs() < 0.001);
        assert_eq!(report.system_state, SystemState::Optimal);
    }

    #[test]
    fn evaluate_increments_history() {
        let ft = FitnessTensor::new();
        let t = TensorValues::uniform(0.5);
        assert_eq!(ft.history_len(), 0);

        ft.evaluate(&t, 1, None).unwrap();
        assert_eq!(ft.history_len(), 1);

        ft.evaluate(&t, 2, None).unwrap();
        assert_eq!(ft.history_len(), 2);
    }

    #[test]
    fn history_bounded() {
        let config = FitnessTensorConfig {
            history_capacity: 5,
            ..Default::default()
        };
        let ft = FitnessTensor::with_config(config);
        let t = TensorValues::uniform(0.5);

        for tick in 0..10 {
            ft.evaluate(&t, tick, None).unwrap();
        }
        assert_eq!(ft.history_len(), 5);
    }

    #[test]
    fn trend_unknown_with_no_data() {
        let ft = FitnessTensor::new();
        assert_eq!(ft.compute_trend(), FitnessTrend::Unknown);
    }

    #[test]
    fn trend_unknown_with_one_sample() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.5), 1, None).unwrap();
        assert_eq!(ft.compute_trend(), FitnessTrend::Unknown);
    }

    #[test]
    fn trend_improving() {
        let ft = FitnessTensor::new();
        for i in 0..10 {
            let val = 0.3 + (i as f64) * 0.05;
            ft.evaluate(&TensorValues::uniform(val), i as u64, None).unwrap();
        }
        assert_eq!(ft.compute_trend(), FitnessTrend::Improving);
    }

    #[test]
    fn trend_declining() {
        let ft = FitnessTensor::new();
        for i in 0..10 {
            let val = 0.9 - (i as f64) * 0.05;
            ft.evaluate(&TensorValues::uniform(val), i as u64, None).unwrap();
        }
        assert_eq!(ft.compute_trend(), FitnessTrend::Declining);
    }

    #[test]
    fn trend_stable() {
        let ft = FitnessTensor::new();
        for i in 0..10 {
            ft.evaluate(&TensorValues::uniform(0.7), i as u64, None).unwrap();
        }
        assert_eq!(ft.compute_trend(), FitnessTrend::Stable);
    }

    #[test]
    fn volatility_zero_for_constant() {
        let ft = FitnessTensor::new();
        for i in 0..5 {
            ft.evaluate(&TensorValues::uniform(0.5), i, None).unwrap();
        }
        assert!(ft.compute_volatility() < f64::EPSILON);
    }

    #[test]
    fn volatility_positive_for_varying() {
        let ft = FitnessTensor::new();
        let vals = [0.3, 0.8, 0.4, 0.9, 0.2];
        for (i, &v) in vals.iter().enumerate() {
            ft.evaluate(&TensorValues::uniform(v), i as u64, None).unwrap();
        }
        assert!(ft.compute_volatility() > 0.1);
    }

    #[test]
    fn fitness_delta_existing_ticks() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.5), 10, None).unwrap();
        ft.evaluate(&TensorValues::uniform(0.8), 20, None).unwrap();

        let delta = ft.fitness_delta(10, 20).unwrap();
        assert!((delta - 0.3).abs() < 0.001);
    }

    #[test]
    fn fitness_delta_missing_tick() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.5), 10, None).unwrap();
        assert!(ft.fitness_delta(10, 99).is_none());
    }

    #[test]
    fn is_improvement_above_threshold() {
        let ft = FitnessTensor::new();
        assert!(ft.is_improvement(0.05));
        assert!(ft.is_improvement(0.02));
    }

    #[test]
    fn is_improvement_below_threshold() {
        let ft = FitnessTensor::new();
        assert!(!ft.is_improvement(0.01));
        assert!(!ft.is_improvement(-0.05));
    }

    #[test]
    fn current_fitness_none_when_empty() {
        let ft = FitnessTensor::new();
        assert!(ft.current_fitness().is_none());
    }

    #[test]
    fn current_fitness_returns_latest() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.5), 1, None).unwrap();
        ft.evaluate(&TensorValues::uniform(0.7), 2, None).unwrap();
        let fitness = ft.current_fitness().unwrap();
        assert!((fitness - 0.7).abs() < 0.001);
    }

    #[test]
    fn stats_updated() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.3), 1, None).unwrap();
        ft.evaluate(&TensorValues::uniform(0.9), 2, None).unwrap();
        ft.evaluate(&TensorValues::uniform(0.6), 3, None).unwrap();

        let stats = ft.stats();
        assert_eq!(stats.total_evaluations, 3);
        assert!((stats.peak_fitness - 0.9).abs() < 0.001);
        assert!((stats.trough_fitness - 0.3).abs() < 0.001);
        assert!((stats.current_fitness - 0.6).abs() < 0.001);
    }

    #[test]
    fn dimension_analysis_identifies_extremes() {
        let ft = FitnessTensor::new();
        let mut t = TensorValues::uniform(0.5);
        // D0: 0.0 × 0.18 = 0.0 (weakest)
        t.set(FitnessDimension::CoordinationQuality, 0.0);
        // D1: 1.0 × 0.15 = 0.15 (strongest)
        t.set(FitnessDimension::FieldCoherence, 1.0);
        ft.evaluate(&t, 1, None).unwrap();

        let analysis = ft.dimension_analysis().unwrap();
        assert_eq!(analysis.weakest, FitnessDimension::CoordinationQuality);
        assert_eq!(analysis.strongest, FitnessDimension::FieldCoherence);
    }

    #[test]
    fn reset_clears_state() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.5), 1, None).unwrap();
        assert_eq!(ft.history_len(), 1);

        ft.reset();
        assert_eq!(ft.history_len(), 0);
        assert!(ft.current_fitness().is_none());
    }

    #[test]
    fn config_validation_ok() {
        assert!(FitnessTensor::validate_config(&FitnessTensorConfig::default()).is_ok());
    }

    #[test]
    fn config_validation_zero_history() {
        let config = FitnessTensorConfig {
            history_capacity: 0,
            ..Default::default()
        };
        assert!(FitnessTensor::validate_config(&config).is_err());
    }

    #[test]
    fn config_validation_zero_trend_window() {
        let config = FitnessTensorConfig {
            trend_window: 0,
            ..Default::default()
        };
        assert!(FitnessTensor::validate_config(&config).is_err());
    }

    #[test]
    fn config_validation_bad_weights_sum() {
        let config = FitnessTensorConfig {
            custom_weights: Some([0.1; 12]),
            ..Default::default()
        };
        assert!(FitnessTensor::validate_config(&config).is_err());
    }

    #[test]
    fn config_validation_negative_weight() {
        let mut weights = DIMENSION_WEIGHTS;
        weights[0] = -0.1;
        weights[1] = 0.28; // rebalance
        let config = FitnessTensorConfig {
            custom_weights: Some(weights),
            ..Default::default()
        };
        assert!(FitnessTensor::validate_config(&config).is_err());
    }

    #[test]
    fn custom_weights_used() {
        let mut weights = [0.0; 12];
        weights[0] = 1.0; // All weight on D0
        let config = FitnessTensorConfig {
            custom_weights: Some(weights),
            ..Default::default()
        };
        let ft = FitnessTensor::with_config(config);
        let mut t = TensorValues::zero();
        t.set(FitnessDimension::CoordinationQuality, 0.8);
        let report = ft.evaluate(&t, 1, None).unwrap();
        assert!((report.overall_score - 0.8).abs() < 0.001);
    }

    #[test]
    fn weighted_contributions_correct() {
        let ft = FitnessTensor::new();
        let t = TensorValues::uniform(1.0);
        let report = ft.evaluate(&t, 1, None).unwrap();
        for i in 0..DIMENSION_COUNT {
            assert!(
                (report.weighted_contributions[i] - DIMENSION_WEIGHTS[i]).abs() < 0.001,
                "contribution[{i}] mismatch"
            );
        }
    }

    #[test]
    fn linear_regression_slope_flat() {
        let slope = linear_regression_slope(&[5.0, 5.0, 5.0, 5.0]);
        assert!(slope.abs() < f64::EPSILON);
    }

    #[test]
    fn linear_regression_slope_positive() {
        let slope = linear_regression_slope(&[1.0, 2.0, 3.0, 4.0, 5.0]);
        assert!((slope - 1.0).abs() < 0.001);
    }

    #[test]
    fn linear_regression_slope_negative() {
        let slope = linear_regression_slope(&[5.0, 4.0, 3.0, 2.0, 1.0]);
        assert!((slope - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn linear_regression_slope_empty() {
        assert!(linear_regression_slope(&[]).abs() < f64::EPSILON);
    }

    #[test]
    fn linear_regression_slope_single() {
        assert!(linear_regression_slope(&[42.0]).abs() < f64::EPSILON);
    }

    #[test]
    fn std_dev_zero_for_constant() {
        assert!(std_dev(&[5.0, 5.0, 5.0]).abs() < f64::EPSILON);
    }

    #[test]
    fn std_dev_correct() {
        let sd = std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert!((sd - 2.0).abs() < 0.01);
    }

    #[test]
    fn std_dev_empty() {
        assert!(std_dev(&[]).abs() < f64::EPSILON);
    }

    #[test]
    fn fitness_dimension_display() {
        assert_eq!(
            FitnessDimension::CoordinationQuality.to_string(),
            "coordination_quality"
        );
        assert_eq!(
            FitnessDimension::ConsentCompliance.to_string(),
            "consent_compliance"
        );
    }

    #[test]
    fn system_state_display() {
        assert_eq!(SystemState::Optimal.to_string(), "optimal");
        assert_eq!(SystemState::Failed.to_string(), "failed");
    }

    #[test]
    fn fitness_trend_display() {
        assert_eq!(FitnessTrend::Improving.to_string(), "improving");
        assert_eq!(FitnessTrend::Unknown.to_string(), "unknown");
    }

    #[test]
    fn latest_snapshot_returns_last() {
        let ft = FitnessTensor::new();
        ft.evaluate(&TensorValues::uniform(0.3), 10, Some(1)).unwrap();
        ft.evaluate(&TensorValues::uniform(0.7), 20, Some(2)).unwrap();

        let snap = ft.latest_snapshot().unwrap();
        assert_eq!(snap.tick, 20);
        assert_eq!(snap.generation, Some(2));
        assert!((snap.fitness - 0.7).abs() < 0.001);
    }

    #[test]
    fn dimension_all_has_twelve() {
        assert_eq!(FitnessDimension::all().len(), 12);
    }

    #[test]
    fn evaluate_rejects_nan() {
        let ft = FitnessTensor::new();
        let mut t = TensorValues::uniform(0.5);
        t.values[7] = f64::NAN;
        assert!(ft.evaluate(&t, 1, None).is_err());
    }

    #[test]
    fn evaluate_with_generation() {
        let ft = FitnessTensor::new();
        let t = TensorValues::uniform(0.6);
        let report = ft.evaluate(&t, 100, Some(42)).unwrap();
        let snap = ft.latest_snapshot().unwrap();
        assert_eq!(snap.generation, Some(42));
        assert_eq!(report.tick, 100);
    }

    #[test]
    fn is_stable_with_constant_data() {
        let ft = FitnessTensor::new();
        for i in 0..10 {
            ft.evaluate(&TensorValues::uniform(0.5), i, None).unwrap();
        }
        assert!(ft.is_stable());
        assert!(!ft.is_volatile());
    }

    #[test]
    fn is_volatile_with_wild_data() {
        let ft = FitnessTensor::new();
        let vals = [0.1, 0.9, 0.2, 0.8, 0.1, 0.9, 0.2, 0.8, 0.1, 0.9];
        for (i, &v) in vals.iter().enumerate() {
            ft.evaluate(&TensorValues::uniform(v), i as u64, None).unwrap();
        }
        assert!(ft.is_volatile());
        assert!(!ft.is_stable());
    }
}
