//! # M40: Mutation Selector
//!
//! Diversity-enforced parameter selection for RALPH evolution.
//! **Critical BUG-035 fix:** prevents mono-parameter targeting that caused
//! ME's evolution chamber to target `min_confidence` in 84% of mutations.
//!
//! ## Layer: L8 (Evolution)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//!
//! ## Diversity Enforcement (3 mechanisms)
//!
//! 1. **Round-robin**: Cycles through the full parameter pool, not weighted selection
//! 2. **Cooldown**: 10-generation minimum between repeated targeting of same parameter
//! 3. **Rejection gate**: Reject if >50% of last 20 mutations hit same parameter
//!
//! ## Mutation Strategy
//!
//! - **Bounded delta**: `|delta| <= max_delta` (default 0.20)
//! - **Direction**: Guided by fitness trend (increase if below target, decrease if above)
//! - **Snap to bounds**: Mutated value clamped to parameter's valid range

use std::collections::{HashMap, VecDeque};
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default per-parameter cooldown (generations).
const DEFAULT_COOLDOWN_GENERATIONS: u64 = 10;

/// Default diversity window (last N mutations checked).
const DEFAULT_DIVERSITY_WINDOW: usize = 20;

/// Default diversity rejection threshold (reject if ratio exceeds this).
const DEFAULT_DIVERSITY_THRESHOLD: f64 = 0.5;

/// Default maximum mutation delta magnitude.
const DEFAULT_MAX_DELTA: f64 = 0.20;

/// Default minimum mutation delta magnitude (avoid no-op mutations).
const DEFAULT_MIN_DELTA: f64 = 0.001;

/// Maximum mutation history retained.
const DEFAULT_HISTORY_CAPACITY: usize = 1000;

// ──────────────────────────────────────────────────────────────
// Data structures
// ──────────────────────────────────────────────────────────────

/// A mutable parameter registered in the parameter pool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutableParameter {
    /// Parameter name (unique key).
    pub name: String,
    /// Current value.
    pub current_value: f64,
    /// Minimum valid value.
    pub min_value: f64,
    /// Maximum valid value.
    pub max_value: f64,
    /// Target value (what RALPH aims for).
    pub target_value: f64,
    /// Description of this parameter.
    pub description: String,
}

impl MutableParameter {
    /// Create a new mutable parameter.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        current_value: f64,
        min_value: f64,
        max_value: f64,
        target_value: f64,
        description: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            current_value,
            min_value,
            max_value,
            target_value,
            description: description.into(),
        }
    }

    /// How far this parameter is from its target (absolute delta).
    #[must_use]
    pub fn drift(&self) -> f64 {
        (self.current_value - self.target_value).abs()
    }

    /// Whether this parameter is within 1% of its target.
    #[must_use]
    pub fn is_on_target(&self) -> bool {
        let range = self.max_value - self.min_value;
        if range <= 0.0 {
            return true;
        }
        self.drift() / range < 0.01
    }
}

/// A proposed mutation to a parameter.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationProposal {
    /// Target parameter name.
    pub parameter: String,
    /// Current value before mutation.
    pub current_value: f64,
    /// Proposed new value.
    pub proposed_value: f64,
    /// Signed delta.
    pub delta: f64,
    /// RALPH generation number.
    pub generation: u64,
    /// Reason for this mutation.
    pub reason: String,
}

impl fmt::Display for MutationProposal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} → {} (Δ{:+.4})",
            self.parameter, self.current_value, self.proposed_value, self.delta
        )
    }
}

/// A record of a mutation that was selected (for diversity tracking).
#[derive(Clone, Debug)]
struct SelectionRecord {
    /// Parameter name.
    parameter: String,
}

/// Reason a mutation was rejected by diversity enforcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RejectionReason {
    /// Parameter is on cooldown.
    Cooldown {
        /// Parameter name.
        parameter: String,
        /// Generations remaining.
        remaining: u64,
    },
    /// Diversity gate failed.
    DiversityThreshold {
        /// Parameter name.
        parameter: String,
        /// Current ratio.
        ratio: u64,
        /// Window size.
        window: usize,
    },
    /// No parameters available.
    NoParameters,
    /// All parameters are on target.
    AllOnTarget,
    /// All candidates are blocked by cooldown or diversity, but NOT on target.
    AllBlocked,
}

impl fmt::Display for RejectionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cooldown { parameter, remaining } => {
                write!(f, "{parameter} on cooldown ({remaining} generations remaining)")
            }
            Self::DiversityThreshold { parameter, ratio, window } => {
                write!(f, "{parameter} hit {ratio}/{window} in diversity window")
            }
            Self::NoParameters => f.write_str("no parameters registered"),
            Self::AllOnTarget => f.write_str("all parameters are on target"),
            Self::AllBlocked => f.write_str("all candidates blocked by cooldown or diversity"),
        }
    }
}

/// Configuration for the `MutationSelector`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationSelectorConfig {
    /// Cooldown per parameter (generations).
    pub cooldown_generations: u64,
    /// Window of recent mutations checked for diversity.
    pub diversity_window: usize,
    /// Rejection threshold: reject if same param > this fraction of window.
    pub diversity_threshold: f64,
    /// Maximum absolute mutation delta.
    pub max_delta: f64,
    /// Minimum absolute mutation delta.
    pub min_delta: f64,
    /// Maximum selection history retained.
    pub history_capacity: usize,
}

impl Default for MutationSelectorConfig {
    fn default() -> Self {
        Self {
            cooldown_generations: DEFAULT_COOLDOWN_GENERATIONS,
            diversity_window: DEFAULT_DIVERSITY_WINDOW,
            diversity_threshold: DEFAULT_DIVERSITY_THRESHOLD,
            max_delta: DEFAULT_MAX_DELTA,
            min_delta: DEFAULT_MIN_DELTA,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
        }
    }
}

/// Aggregate statistics for the `MutationSelector`.
#[derive(Clone, Debug, Default)]
pub struct MutationSelectorStats {
    /// Total selection attempts.
    pub total_attempts: u64,
    /// Total successful selections.
    pub total_selections: u64,
    /// Total rejections (any reason).
    pub total_rejections: u64,
    /// Rejections by reason category.
    pub rejection_counts: HashMap<String, u64>,
    /// Per-parameter selection counts.
    pub per_parameter: HashMap<String, u64>,
}

// ──────────────────────────────────────────────────────────────
// MutationSelector
// ──────────────────────────────────────────────────────────────

/// Diversity-enforced mutation selector for RALPH evolution.
///
/// Implements the BUG-035 fix with three diversity mechanisms:
/// round-robin cycling, per-parameter cooldown, and a sliding-window
/// rejection gate.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct MutationSelector {
    /// Registered mutable parameters.
    parameters: RwLock<Vec<MutableParameter>>,
    /// Round-robin index (cycles through parameter pool).
    round_robin_idx: RwLock<usize>,
    /// Per-parameter last-selected generation (cooldown tracking).
    last_selected: RwLock<HashMap<String, u64>>,
    /// Selection history (sliding window for diversity gate).
    selection_history: RwLock<VecDeque<SelectionRecord>>,
    /// Configuration.
    config: MutationSelectorConfig,
    /// Aggregate statistics.
    stats: RwLock<MutationSelectorStats>,
}

impl fmt::Debug for MutationSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MutationSelector")
            .field("parameters", &self.parameters.read().len())
            .field("round_robin_idx", &*self.round_robin_idx.read())
            .finish_non_exhaustive()
    }
}

impl MutationSelector {
    /// Creates a new `MutationSelector` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(MutationSelectorConfig::default())
    }

    /// Creates a new `MutationSelector` with the given configuration.
    #[must_use]
    pub fn with_config(config: MutationSelectorConfig) -> Self {
        Self {
            parameters: RwLock::new(Vec::new()),
            round_robin_idx: RwLock::new(0),
            last_selected: RwLock::new(HashMap::new()),
            selection_history: RwLock::new(VecDeque::with_capacity(
                config.history_capacity.min(1000),
            )),
            config,
            stats: RwLock::new(MutationSelectorStats::default()),
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if any parameter is invalid.
    pub fn validate_config(config: &MutationSelectorConfig) -> PvResult<()> {
        if config.diversity_window == 0 {
            return Err(PvError::ConfigValidation(
                "diversity_window must be > 0".into(),
            ));
        }
        if config.diversity_threshold <= 0.0 || config.diversity_threshold > 1.0 {
            return Err(PvError::ConfigValidation(
                "diversity_threshold must be in (0.0, 1.0]".into(),
            ));
        }
        if config.max_delta <= 0.0 || !config.max_delta.is_finite() {
            return Err(PvError::ConfigValidation(
                "max_delta must be positive and finite".into(),
            ));
        }
        if config.min_delta < 0.0 || config.min_delta >= config.max_delta {
            return Err(PvError::ConfigValidation(
                "min_delta must be in [0, max_delta)".into(),
            ));
        }
        Ok(())
    }

    /// Register a mutable parameter.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if min > max or name is empty.
    pub fn register_parameter(&self, param: MutableParameter) -> PvResult<()> {
        if param.name.is_empty() {
            return Err(PvError::EmptyString {
                field: "parameter_name",
            });
        }
        if param.min_value > param.max_value {
            return Err(PvError::ConfigValidation(
                format!("min_value ({}) > max_value ({}) for {}", param.min_value, param.max_value, param.name),
            ));
        }

        let mut params = self.parameters.write();
        // Prevent duplicates — update if exists
        if let Some(existing) = params.iter_mut().find(|p| p.name == param.name) {
            *existing = param;
        } else {
            params.push(param);
        }
        Ok(())
    }

    /// Update a parameter's current value (e.g. after mutation applied).
    ///
    /// # Errors
    /// Returns [`PvError::Internal`] if the parameter is not registered.
    pub fn update_value(&self, name: &str, value: f64) -> PvResult<()> {
        let mut params = self.parameters.write();
        let param = params
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or_else(|| PvError::Internal(format!("parameter '{name}' not registered")))?;
        param.current_value = value.clamp(param.min_value, param.max_value);
        Ok(())
    }

    /// Select the next parameter for mutation using diversity-enforced round-robin.
    ///
    /// Returns the mutation proposal, or a rejection reason.
    ///
    /// # Errors
    /// Returns [`RejectionReason::NoParameters`] if no parameters are registered,
    /// [`RejectionReason::AllOnTarget`] if all parameters are within target tolerance,
    /// or [`RejectionReason::AllBlocked`] if all off-target candidates are blocked
    /// by cooldown or diversity enforcement.
    pub fn select(&self, generation: u64) -> Result<MutationProposal, RejectionReason> {
        self.stats.write().total_attempts += 1;

        let params = self.parameters.read();
        if params.is_empty() {
            self.stats.write().total_rejections += 1;
            return Err(RejectionReason::NoParameters);
        }

        let param_count = params.len();
        let idx = *self.round_robin_idx.read();

        // Track whether any candidate was skipped due to cooldown/diversity
        // (as opposed to being on-target) so we can distinguish the rejection reason.
        let mut any_blocked = false;

        // Try each parameter starting from round-robin position
        for attempt in 0..param_count {
            let candidate_idx = (idx + attempt) % param_count;
            let candidate = &params[candidate_idx];

            // Skip if on target
            if candidate.is_on_target() {
                continue;
            }

            // Check cooldown
            if self.check_cooldown(&candidate.name, generation).is_some() {
                any_blocked = true;
                continue;
            }

            // Check diversity gate
            if self.check_diversity(&candidate.name).is_some() {
                any_blocked = true;
                continue;
            }

            // This parameter passes all gates — select it
            let proposal = self.create_proposal(candidate, generation);

            // Record selection
            drop(params); // Release read lock before write
            self.record_selection(&proposal.parameter, generation);
            *self.round_robin_idx.write() = (candidate_idx + 1) % param_count;

            let mut stats = self.stats.write();
            stats.total_selections += 1;
            *stats.per_parameter.entry(proposal.parameter.clone()).or_insert(0) += 1;

            return Ok(proposal);
        }

        // All parameters exhausted
        drop(params);
        self.stats.write().total_rejections += 1;

        // Advance round-robin even on failure
        *self.round_robin_idx.write() = (idx + 1) % param_count;

        if any_blocked {
            Err(RejectionReason::AllBlocked)
        } else {
            Err(RejectionReason::AllOnTarget)
        }
    }

    /// Check if a parameter is on cooldown.
    fn check_cooldown(&self, name: &str, generation: u64) -> Option<RejectionReason> {
        let last = self.last_selected.read();
        if let Some(&last_gen) = last.get(name) {
            let elapsed = generation.saturating_sub(last_gen);
            if elapsed < self.config.cooldown_generations {
                return Some(RejectionReason::Cooldown {
                    parameter: name.to_owned(),
                    remaining: self.config.cooldown_generations - elapsed,
                });
            }
        }
        None
    }

    /// Check the diversity gate for a parameter.
    fn check_diversity(&self, name: &str) -> Option<RejectionReason> {
        let history = self.selection_history.read();
        let window = self.config.diversity_window;
        let recent: Vec<&SelectionRecord> = history.iter().rev().take(window).collect();

        if recent.is_empty() {
            return None;
        }

        let count = recent.iter().filter(|r| r.parameter == name).count();
        #[allow(clippy::cast_precision_loss)] // count and window are small (diversity_window ~20)
        let ratio = count as f64 / recent.len().min(window) as f64;

        if ratio > self.config.diversity_threshold {
            return Some(RejectionReason::DiversityThreshold {
                parameter: name.to_owned(),
                ratio: count as u64,
                window: recent.len(),
            });
        }

        None
    }

    /// Create a mutation proposal for a parameter.
    fn create_proposal(&self, param: &MutableParameter, generation: u64) -> MutationProposal {
        let drift = param.current_value - param.target_value;
        let range = param.max_value - param.min_value;

        // Direction: move toward target
        let direction = if drift > 0.0 { -1.0 } else { 1.0 };

        // Delta magnitude: proportional to drift, bounded
        let magnitude = (drift.abs() * 0.3)
            .max(self.config.min_delta)
            .min(self.config.max_delta)
            .min(range * 0.1); // Never more than 10% of range

        let delta = direction * magnitude;
        let proposed = (param.current_value + delta).clamp(param.min_value, param.max_value);
        let actual_delta = proposed - param.current_value;

        let reason = if drift.abs() > range * 0.1 {
            format!("Large drift ({drift:.4}), moving toward target")
        } else {
            format!("Fine-tuning toward target (drift {drift:.4})")
        };

        MutationProposal {
            parameter: param.name.clone(),
            current_value: param.current_value,
            proposed_value: proposed,
            delta: actual_delta,
            generation,
            reason,
        }
    }

    /// Record a selection in the history.
    fn record_selection(&self, parameter: &str, generation: u64) {
        let mut history = self.selection_history.write();
        if history.len() >= self.config.history_capacity {
            history.pop_front();
        }
        history.push_back(SelectionRecord {
            parameter: parameter.to_owned(),
        });

        self.last_selected.write().insert(parameter.to_owned(), generation);
    }

    /// Get the number of registered parameters.
    #[must_use]
    pub fn parameter_count(&self) -> usize {
        self.parameters.read().len()
    }

    /// Get registered parameter names.
    #[must_use]
    pub fn parameter_names(&self) -> Vec<String> {
        self.parameters.read().iter().map(|p| p.name.clone()).collect()
    }

    /// Get a snapshot of a parameter's current state.
    #[must_use]
    pub fn get_parameter(&self, name: &str) -> Option<MutableParameter> {
        self.parameters.read().iter().find(|p| p.name == name).cloned()
    }

    /// Get the diversity ratio for a parameter (fraction of recent window).
    #[must_use]
    pub fn diversity_ratio(&self, name: &str) -> f64 {
        let history = self.selection_history.read();
        let window = self.config.diversity_window;
        let recent: Vec<&SelectionRecord> = history.iter().rev().take(window).collect();

        if recent.is_empty() {
            return 0.0;
        }

        let count = recent.iter().filter(|r| r.parameter == name).count();
        #[allow(clippy::cast_precision_loss)] // count and len are small (diversity_window ~20)
        { count as f64 / recent.len() as f64 }
    }

    /// Get aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> MutationSelectorStats {
        self.stats.read().clone()
    }

    /// Clear all state (parameters, history, stats).
    pub fn reset(&self) {
        self.parameters.write().clear();
        *self.round_robin_idx.write() = 0;
        self.last_selected.write().clear();
        self.selection_history.write().clear();
        *self.stats.write() = MutationSelectorStats::default();
    }
}

impl Default for MutationSelector {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_selector() -> MutationSelector {
        MutationSelector::new()
    }

    fn register_test_params(sel: &MutationSelector) {
        let params = vec![
            MutableParameter::new("k_mod", 1.0, 0.01, 1.5, 0.5, "Coupling strength"),
            MutableParameter::new("hebbian_ltp", 0.01, 0.001, 0.1, 0.03, "LTP rate"),
            MutableParameter::new("tick_interval", 5.0, 1.0, 30.0, 5.0, "Tick interval"),
            MutableParameter::new("r_target", 0.93, 0.5, 1.0, 0.85, "R target"),
            MutableParameter::new("decay_rate", 0.995, 0.98, 1.0, 0.99, "Decay per step"),
        ];
        for p in params {
            sel.register_parameter(p).unwrap();
        }
    }

    #[test]
    fn default_config_valid() {
        assert!(MutationSelector::validate_config(&MutationSelectorConfig::default()).is_ok());
    }

    #[test]
    fn config_zero_window_invalid() {
        let config = MutationSelectorConfig {
            diversity_window: 0,
            ..Default::default()
        };
        assert!(MutationSelector::validate_config(&config).is_err());
    }

    #[test]
    fn config_bad_threshold_invalid() {
        let config = MutationSelectorConfig {
            diversity_threshold: 0.0,
            ..Default::default()
        };
        assert!(MutationSelector::validate_config(&config).is_err());
    }

    #[test]
    fn config_bad_delta_invalid() {
        let config = MutationSelectorConfig {
            max_delta: -1.0,
            ..Default::default()
        };
        assert!(MutationSelector::validate_config(&config).is_err());
    }

    #[test]
    fn register_parameter() {
        let sel = make_selector();
        let p = MutableParameter::new("test", 1.0, 0.0, 2.0, 1.5, "Test param");
        assert!(sel.register_parameter(p).is_ok());
        assert_eq!(sel.parameter_count(), 1);
    }

    #[test]
    fn register_duplicate_updates() {
        let sel = make_selector();
        let p1 = MutableParameter::new("test", 1.0, 0.0, 2.0, 1.5, "First");
        let p2 = MutableParameter::new("test", 2.0, 0.0, 3.0, 2.5, "Second");
        sel.register_parameter(p1).unwrap();
        sel.register_parameter(p2).unwrap();
        assert_eq!(sel.parameter_count(), 1);
        let param = sel.get_parameter("test").unwrap();
        assert!((param.current_value - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn register_empty_name_rejected() {
        let sel = make_selector();
        let p = MutableParameter::new("", 1.0, 0.0, 2.0, 1.5, "Bad");
        assert!(sel.register_parameter(p).is_err());
    }

    #[test]
    fn register_min_gt_max_rejected() {
        let sel = make_selector();
        let p = MutableParameter::new("test", 1.0, 2.0, 1.0, 1.5, "Bad range");
        assert!(sel.register_parameter(p).is_err());
    }

    #[test]
    fn select_no_parameters() {
        let sel = make_selector();
        let result = sel.select(1);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RejectionReason::NoParameters));
    }

    #[test]
    fn select_basic() {
        let sel = make_selector();
        register_test_params(&sel);
        let proposal = sel.select(1).unwrap();
        assert!(!proposal.parameter.is_empty());
        assert!(proposal.delta.abs() > 0.0);
    }

    #[test]
    fn select_moves_toward_target() {
        let sel = make_selector();
        // k_mod at 1.0, target 0.5 → should decrease
        sel.register_parameter(MutableParameter::new(
            "k_mod", 1.0, 0.01, 1.5, 0.5, "test",
        )).unwrap();

        let proposal = sel.select(1).unwrap();
        assert_eq!(proposal.parameter, "k_mod");
        assert!(proposal.delta < 0.0, "should move toward target (decrease)");
    }

    #[test]
    fn select_delta_bounded() {
        let sel = make_selector();
        sel.register_parameter(MutableParameter::new(
            "extreme", 0.0, 0.0, 100.0, 100.0, "Large drift",
        )).unwrap();

        let proposal = sel.select(1).unwrap();
        assert!(proposal.delta.abs() <= sel.config.max_delta + f64::EPSILON);
    }

    #[test]
    fn select_proposed_value_in_range() {
        let sel = make_selector();
        sel.register_parameter(MutableParameter::new(
            "bounded", 0.05, 0.0, 1.0, 0.5, "test",
        )).unwrap();

        let proposal = sel.select(1).unwrap();
        assert!(proposal.proposed_value >= 0.0);
        assert!(proposal.proposed_value <= 1.0);
    }

    #[test]
    fn round_robin_cycles() {
        let sel = make_selector();
        register_test_params(&sel);

        let mut selected = Vec::new();
        // Select many times with no cooldown (set cooldown to 0)
        let config = MutationSelectorConfig {
            cooldown_generations: 0,
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        register_test_params(&sel);

        for gen in 0..20 {
            if let Ok(proposal) = sel.select(gen) {
                selected.push(proposal.parameter);
            }
        }

        // Should see multiple different parameters
        let unique: std::collections::HashSet<&String> = selected.iter().collect();
        assert!(unique.len() >= 2, "round-robin should cycle through parameters");
    }

    #[test]
    fn cooldown_enforced() {
        let config = MutationSelectorConfig {
            cooldown_generations: 5,
            diversity_window: 100, // Large window to dilute
            diversity_threshold: 1.0, // Disable diversity gate for this test
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "only_param", 1.0, 0.0, 2.0, 0.5, "test",
        )).unwrap();

        // First selection succeeds
        let p1 = sel.select(1);
        assert!(p1.is_ok());

        // Second selection within cooldown should fail
        let p2 = sel.select(2);
        assert!(p2.is_err());

        // After cooldown expires
        let p3 = sel.select(7); // 7 - 1 = 6 >= 5
        assert!(p3.is_ok());
    }

    #[test]
    fn diversity_gate_enforced() {
        let config = MutationSelectorConfig {
            cooldown_generations: 0, // Disable cooldown
            diversity_window: 4,
            diversity_threshold: 0.5,
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "param_a", 1.0, 0.0, 2.0, 0.5, "A",
        )).unwrap();
        sel.register_parameter(MutableParameter::new(
            "param_b", 1.0, 0.0, 2.0, 0.5, "B",
        )).unwrap();

        // Force param_a to be selected multiple times
        // After 3 selections of param_a in window of 4, ratio = 0.75 > 0.5
        // The diversity gate should kick in
        let mut a_count = 0_u64;
        let mut b_count = 0_u64;
        for gen in 0..20 {
            if let Ok(proposal) = sel.select(gen) {
                if proposal.parameter == "param_a" {
                    a_count += 1;
                } else {
                    b_count += 1;
                }
            }
        }

        // Both should get selected (diversity prevents monopoly)
        assert!(a_count > 0, "param_a should be selected");
        assert!(b_count > 0, "param_b should be selected (diversity enforcement)");
    }

    #[test]
    fn bug_035_no_monopoly() {
        let sel = make_selector();
        register_test_params(&sel);

        let mut counts: HashMap<String, u64> = HashMap::new();

        // Run 100 generations
        for gen in 0..100 {
            if let Ok(proposal) = sel.select(gen) {
                *counts.entry(proposal.parameter.clone()).or_insert(0) += 1;
            }
        }

        // BUG-035 check: no single parameter should exceed 50% of selections
        let total: u64 = counts.values().sum();
        if total > 0 {
            for (param, count) in &counts {
                let ratio = *count as f64 / total as f64;
                assert!(
                    ratio <= 0.6, // Allow slightly above 0.5 due to round-robin mechanics
                    "BUG-035: {param} selected {count}/{total} ({pct:.1}%) — monopoly detected",
                    pct = ratio * 100.0
                );
            }
        }
    }

    #[test]
    fn update_value() {
        let sel = make_selector();
        sel.register_parameter(MutableParameter::new(
            "test", 1.0, 0.0, 2.0, 1.5, "test",
        )).unwrap();

        sel.update_value("test", 1.8).unwrap();
        let param = sel.get_parameter("test").unwrap();
        assert!((param.current_value - 1.8).abs() < f64::EPSILON);
    }

    #[test]
    fn update_value_clamped() {
        let sel = make_selector();
        sel.register_parameter(MutableParameter::new(
            "test", 1.0, 0.0, 2.0, 1.5, "test",
        )).unwrap();

        sel.update_value("test", 5.0).unwrap();
        let param = sel.get_parameter("test").unwrap();
        assert!((param.current_value - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn update_value_unknown() {
        let sel = make_selector();
        assert!(sel.update_value("nonexistent", 1.0).is_err());
    }

    #[test]
    fn parameter_drift() {
        let p = MutableParameter::new("test", 1.0, 0.0, 2.0, 0.5, "test");
        assert!((p.drift() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn parameter_on_target() {
        let p = MutableParameter::new("test", 0.5, 0.0, 1.0, 0.5, "test");
        assert!(p.is_on_target());
    }

    #[test]
    fn parameter_off_target() {
        let p = MutableParameter::new("test", 0.0, 0.0, 1.0, 0.5, "test");
        assert!(!p.is_on_target());
    }

    #[test]
    fn all_on_target_rejected() {
        let sel = make_selector();
        sel.register_parameter(MutableParameter::new(
            "test", 0.5, 0.0, 1.0, 0.5, "On target",
        )).unwrap();

        let result = sel.select(1);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RejectionReason::AllOnTarget));
    }

    #[test]
    fn diversity_ratio_empty() {
        let sel = make_selector();
        assert!((sel.diversity_ratio("anything")).abs() < f64::EPSILON);
    }

    #[test]
    fn diversity_ratio_after_selections() {
        let config = MutationSelectorConfig {
            cooldown_generations: 0,
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "param_a", 1.0, 0.0, 2.0, 0.5, "A",
        )).unwrap();

        sel.select(1).ok();
        let ratio = sel.diversity_ratio("param_a");
        assert!(ratio > 0.0);
    }

    #[test]
    fn stats_tracking() {
        let sel = make_selector();
        register_test_params(&sel);

        sel.select(1).ok();
        sel.select(2).ok();

        let stats = sel.stats();
        assert!(stats.total_attempts >= 2);
    }

    #[test]
    fn parameter_names() {
        let sel = make_selector();
        register_test_params(&sel);
        let names = sel.parameter_names();
        assert_eq!(names.len(), 5);
        assert!(names.contains(&"k_mod".to_string()));
    }

    #[test]
    fn get_parameter_existing() {
        let sel = make_selector();
        register_test_params(&sel);
        let param = sel.get_parameter("k_mod");
        assert!(param.is_some());
        assert_eq!(param.unwrap().name, "k_mod");
    }

    #[test]
    fn get_parameter_nonexistent() {
        let sel = make_selector();
        assert!(sel.get_parameter("nonexistent").is_none());
    }

    #[test]
    fn reset_clears_all() {
        let sel = make_selector();
        register_test_params(&sel);
        sel.select(1).ok();

        sel.reset();
        assert_eq!(sel.parameter_count(), 0);
        assert!((sel.diversity_ratio("k_mod")).abs() < f64::EPSILON);
    }

    #[test]
    fn mutation_proposal_display() {
        let proposal = MutationProposal {
            parameter: "k_mod".into(),
            current_value: 1.0,
            proposed_value: 0.85,
            delta: -0.15,
            generation: 5,
            reason: "test".into(),
        };
        let display = proposal.to_string();
        assert!(display.contains("k_mod"));
        assert!(display.contains("-0.15"));
    }

    #[test]
    fn rejection_reason_display() {
        let r = RejectionReason::Cooldown {
            parameter: "k_mod".into(),
            remaining: 3,
        };
        assert!(r.to_string().contains("cooldown"));

        let r = RejectionReason::NoParameters;
        assert!(r.to_string().contains("no parameters"));
    }

    #[test]
    fn all_blocked_not_all_on_target() {
        // Single parameter off-target, but on cooldown → should return AllBlocked, NOT AllOnTarget
        let config = MutationSelectorConfig {
            cooldown_generations: 10,
            diversity_threshold: 1.0, // disable diversity for this test
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "blocked_param", 1.0, 0.0, 2.0, 0.5, "off target but will be on cooldown",
        )).unwrap();

        // First select succeeds (places on cooldown)
        let first = sel.select(1);
        assert!(first.is_ok());

        // Second select within cooldown → AllBlocked
        let second = sel.select(2);
        assert!(second.is_err());
        assert!(matches!(second.unwrap_err(), RejectionReason::AllBlocked));
    }

    #[test]
    fn all_on_target_still_returns_all_on_target() {
        let sel = make_selector();
        sel.register_parameter(MutableParameter::new(
            "on_target", 0.5, 0.0, 1.0, 0.5, "exactly on target",
        )).unwrap();
        let result = sel.select(1);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RejectionReason::AllOnTarget));
    }

    #[test]
    fn all_blocked_display() {
        let r = RejectionReason::AllBlocked;
        assert!(r.to_string().contains("blocked"));
    }

    #[test]
    fn mixed_on_target_and_blocked_returns_all_blocked() {
        // Two params: one on-target, one off-target but on cooldown → AllBlocked
        let config = MutationSelectorConfig {
            cooldown_generations: 10,
            diversity_threshold: 1.0,
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "on_target", 0.5, 0.0, 1.0, 0.5, "on target",
        )).unwrap();
        sel.register_parameter(MutableParameter::new(
            "off_target", 1.0, 0.0, 2.0, 0.5, "off target",
        )).unwrap();

        // Select the off-target param, placing it on cooldown
        let first = sel.select(1);
        assert!(first.is_ok());

        // Now: on_target skipped (on target), off_target skipped (cooldown) → AllBlocked
        let second = sel.select(2);
        assert!(second.is_err());
        assert!(matches!(second.unwrap_err(), RejectionReason::AllBlocked));
    }

    #[test]
    fn diversity_blocked_returns_all_blocked() {
        // Single param off-target, blocked by diversity gate → AllBlocked
        let config = MutationSelectorConfig {
            cooldown_generations: 0, // disable cooldown
            diversity_window: 2,
            diversity_threshold: 0.4, // low threshold
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "param_a", 1.0, 0.0, 2.0, 0.5, "off target",
        )).unwrap();

        // First select succeeds
        let first = sel.select(0);
        assert!(first.is_ok());

        // Second select — diversity gate blocks (1/1 > 0.4)
        let second = sel.select(1);
        assert!(second.is_err());
        assert!(matches!(second.unwrap_err(), RejectionReason::AllBlocked));
    }

    #[test]
    fn selection_history_bounded() {
        let config = MutationSelectorConfig {
            cooldown_generations: 0,
            history_capacity: 5,
            ..Default::default()
        };
        let sel = MutationSelector::with_config(config);
        sel.register_parameter(MutableParameter::new(
            "param", 1.0, 0.0, 2.0, 0.5, "test",
        )).unwrap();

        for gen in 0..20 {
            sel.select(gen).ok();
        }

        let hist = sel.selection_history.read();
        assert!(hist.len() <= 5);
    }
}
