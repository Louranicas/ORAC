//! # M35: Token Accounting
//!
//! Per-task token cost tracking and fleet budget management.
//! Tracks input/output tokens, estimates cost, and enforces budgets.
//!
//! ## Layer: L7 (Monitoring)
//! ## Module: M35
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//! ## Feature: `monitoring`
//!
//! ## Tracked Dimensions
//!
//! - Per-pane token usage (input + output)
//! - Per-task token usage
//! - Fleet-wide totals
//! - Cost estimation (configurable per-token rate)
//! - Budget enforcement with soft/hard limits

use std::collections::BTreeMap;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::{PaneId, TaskId};
use crate::m1_core::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default cost per input token (USD).
const DEFAULT_INPUT_COST: f64 = 0.000_015;

/// Default cost per output token (USD).
const DEFAULT_OUTPUT_COST: f64 = 0.000_075;

/// Maximum tracked panes (prevents unbounded memory).
const MAX_TRACKED_PANES: usize = 256;

/// Default soft budget limit (USD).
const DEFAULT_SOFT_LIMIT: f64 = 10.0;

/// Default hard budget limit (USD).
const DEFAULT_HARD_LIMIT: f64 = 50.0;

/// Maximum task records retained (FIFO eviction).
const MAX_TASK_RECORDS: usize = 5_000;

// ──────────────────────────────────────────────────────────────
// Token usage record
// ──────────────────────────────────────────────────────────────

/// Token usage for a single entity (pane, task, or fleet total).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input (prompt) tokens.
    pub input_tokens: u64,
    /// Output (completion) tokens.
    pub output_tokens: u64,
    /// Total tokens (input + output).
    pub total_tokens: u64,
    /// Estimated cost in USD.
    pub estimated_cost: f64,
}

impl TokenUsage {
    /// Create a new zero-usage record.
    #[must_use]
    pub const fn zero() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            estimated_cost: 0.0,
        }
    }

    /// Create from input/output counts with cost calculation.
    #[must_use]
    pub fn from_counts(input: u64, output: u64, input_cost: f64, output_cost: f64) -> Self {
        let total = input.saturating_add(output);
        let cost = u64_to_f64(input).mul_add(input_cost, u64_to_f64(output) * output_cost);
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
            estimated_cost: cost,
        }
    }

    /// Add another usage record to this one.
    pub fn add(&mut self, other: &Self) {
        self.input_tokens = self.input_tokens.saturating_add(other.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(other.output_tokens);
        self.total_tokens = self.total_tokens.saturating_add(other.total_tokens);
        self.estimated_cost += other.estimated_cost;
    }

    /// Whether this record is zero (no tokens used).
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.total_tokens == 0
    }
}

// ──────────────────────────────────────────────────────────────
// Task token record
// ──────────────────────────────────────────────────────────────

/// Token record for a specific task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskTokenRecord {
    /// Task ID.
    pub task_id: TaskId,
    /// Pane that ran this task.
    pub pane_id: PaneId,
    /// Token usage.
    pub usage: TokenUsage,
    /// Task description (truncated).
    pub description: String,
}

// ──────────────────────────────────────────────────────────────
// Budget state
// ──────────────────────────────────────────────────────────────

/// Budget enforcement result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BudgetStatus {
    /// Under soft limit.
    Ok,
    /// Between soft and hard limit (warning).
    Warning,
    /// At or above hard limit (blocked).
    Exceeded,
}

impl BudgetStatus {
    /// Whether the budget allows new work.
    #[must_use]
    pub const fn allows_work(&self) -> bool {
        matches!(self, Self::Ok | Self::Warning)
    }
}

/// Budget configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Soft limit in USD (triggers warning).
    pub soft_limit: f64,
    /// Hard limit in USD (blocks new work).
    pub hard_limit: f64,
    /// Cost per input token (USD).
    pub input_cost: f64,
    /// Cost per output token (USD).
    pub output_cost: f64,
}

impl BudgetConfig {
    /// Create with default values.
    #[must_use]
    pub fn new() -> Self {
        Self {
            soft_limit: DEFAULT_SOFT_LIMIT,
            hard_limit: DEFAULT_HARD_LIMIT,
            input_cost: DEFAULT_INPUT_COST,
            output_cost: DEFAULT_OUTPUT_COST,
        }
    }

    /// Create with custom limits.
    #[must_use]
    pub fn with_limits(soft: f64, hard: f64) -> Self {
        Self {
            soft_limit: soft.max(0.0),
            hard_limit: hard.max(soft.max(0.0)),
            input_cost: DEFAULT_INPUT_COST,
            output_cost: DEFAULT_OUTPUT_COST,
        }
    }

    /// Validate the budget configuration.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if limits are invalid.
    pub fn validate(&self) -> PvResult<()> {
        if self.soft_limit < 0.0 {
            return Err(PvError::ConfigValidation(
                "soft_limit must be >= 0".into(),
            ));
        }
        if self.hard_limit < self.soft_limit {
            return Err(PvError::ConfigValidation(
                "hard_limit must be >= soft_limit".into(),
            ));
        }
        if self.input_cost < 0.0 || !self.input_cost.is_finite() {
            return Err(PvError::ConfigValidation(
                "input_cost must be finite and >= 0".into(),
            ));
        }
        if self.output_cost < 0.0 || !self.output_cost.is_finite() {
            return Err(PvError::ConfigValidation(
                "output_cost must be finite and >= 0".into(),
            ));
        }
        Ok(())
    }
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// TokenAccountant (main registry)
// ──────────────────────────────────────────────────────────────

/// Central token accounting registry.
///
/// Thread-safe via interior mutability. Tracks per-pane,
/// per-task, and fleet-wide token usage with budget enforcement.
#[derive(Debug)]
pub struct TokenAccountant {
    /// Interior-mutable state.
    state: RwLock<AccountantState>,
}

#[derive(Debug)]
struct AccountantState {
    /// Per-pane usage.
    pane_usage: BTreeMap<PaneId, TokenUsage>,
    /// Recent task records (FIFO ring).
    task_records: Vec<TaskTokenRecord>,
    /// Fleet-wide total.
    fleet_total: TokenUsage,
    /// Budget configuration.
    budget: BudgetConfig,
}

impl TokenAccountant {
    /// Create a new accountant with default budget.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: RwLock::new(AccountantState {
                pane_usage: BTreeMap::new(),
                task_records: Vec::new(),
                fleet_total: TokenUsage::zero(),
                budget: BudgetConfig::new(),
            }),
        }
    }

    /// Create with a custom budget configuration.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if the budget is invalid.
    pub fn with_budget(budget: BudgetConfig) -> PvResult<Self> {
        budget.validate()?;
        Ok(Self {
            state: RwLock::new(AccountantState {
                pane_usage: BTreeMap::new(),
                task_records: Vec::new(),
                fleet_total: TokenUsage::zero(),
                budget,
            }),
        })
    }

    /// Record token usage for a pane.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if too many panes are tracked.
    pub fn record_pane_usage(
        &self,
        pane_id: &PaneId,
        input_tokens: u64,
        output_tokens: u64,
    ) -> PvResult<()> {
        let mut state = self.state.write();
        if !state.pane_usage.contains_key(pane_id) && state.pane_usage.len() >= MAX_TRACKED_PANES {
            return Err(PvError::ConfigValidation(format!(
                "pane tracking limit ({MAX_TRACKED_PANES}) exceeded"
            )));
        }
        let usage = TokenUsage::from_counts(
            input_tokens,
            output_tokens,
            state.budget.input_cost,
            state.budget.output_cost,
        );
        state
            .pane_usage
            .entry(pane_id.clone())
            .or_insert_with(TokenUsage::zero)
            .add(&usage);
        state.fleet_total.add(&usage);
        Ok(())
    }

    /// Record token usage for a task.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if the task limit is exceeded.
    pub fn record_task_usage(
        &self,
        task_id: &TaskId,
        pane_id: &PaneId,
        input_tokens: u64,
        output_tokens: u64,
        description: &str,
    ) -> PvResult<()> {
        let mut state = self.state.write();
        let usage = TokenUsage::from_counts(
            input_tokens,
            output_tokens,
            state.budget.input_cost,
            state.budget.output_cost,
        );

        // FIFO eviction
        if state.task_records.len() >= MAX_TASK_RECORDS {
            state.task_records.remove(0);
        }

        state.task_records.push(TaskTokenRecord {
            task_id: task_id.clone(),
            pane_id: pane_id.clone(),
            usage: usage.clone(),
            description: description.chars().take(256).collect(),
        });

        // Also add to pane and fleet totals
        state
            .pane_usage
            .entry(pane_id.clone())
            .or_insert_with(TokenUsage::zero)
            .add(&usage);
        state.fleet_total.add(&usage);
        Ok(())
    }

    /// Get usage for a specific pane.
    #[must_use]
    pub fn pane_usage(&self, pane_id: &PaneId) -> TokenUsage {
        self.state
            .read()
            .pane_usage
            .get(pane_id)
            .cloned()
            .unwrap_or_else(TokenUsage::zero)
    }

    /// Get fleet-wide total usage.
    #[must_use]
    pub fn fleet_total(&self) -> TokenUsage {
        self.state.read().fleet_total.clone()
    }

    /// Get the most recent `n` task records.
    #[must_use]
    pub fn recent_tasks(&self, n: usize) -> Vec<TaskTokenRecord> {
        let state = self.state.read();
        state
            .task_records
            .iter()
            .rev()
            .take(n)
            .cloned()
            .collect()
    }

    /// Get all pane usage as a map.
    #[must_use]
    pub fn all_pane_usage(&self) -> BTreeMap<PaneId, TokenUsage> {
        self.state.read().pane_usage.clone()
    }

    /// Number of tracked panes.
    #[must_use]
    pub fn tracked_pane_count(&self) -> usize {
        self.state.read().pane_usage.len()
    }

    /// Number of task records.
    #[must_use]
    pub fn task_record_count(&self) -> usize {
        self.state.read().task_records.len()
    }

    /// Check current budget status.
    #[must_use]
    pub fn budget_status(&self) -> BudgetStatus {
        let state = self.state.read();
        let cost = state.fleet_total.estimated_cost;
        if cost >= state.budget.hard_limit {
            BudgetStatus::Exceeded
        } else if cost >= state.budget.soft_limit {
            BudgetStatus::Warning
        } else {
            BudgetStatus::Ok
        }
    }

    /// Get the current budget configuration.
    #[must_use]
    pub fn budget(&self) -> BudgetConfig {
        self.state.read().budget.clone()
    }

    /// Update the budget configuration.
    ///
    /// # Errors
    /// Returns `PvError::ConfigValidation` if the budget is invalid.
    pub fn set_budget(&self, budget: BudgetConfig) -> PvResult<()> {
        budget.validate()?;
        self.state.write().budget = budget;
        Ok(())
    }

    /// Remaining budget before hard limit (USD).
    #[must_use]
    pub fn remaining_budget(&self) -> f64 {
        let state = self.state.read();
        (state.budget.hard_limit - state.fleet_total.estimated_cost).max(0.0)
    }

    /// Budget utilization as a fraction (0.0–1.0+).
    #[must_use]
    pub fn budget_utilization(&self) -> f64 {
        let state = self.state.read();
        if state.budget.hard_limit <= 0.0 {
            return 0.0;
        }
        state.fleet_total.estimated_cost / state.budget.hard_limit
    }

    /// Get a summary for the dashboard.
    #[must_use]
    pub fn summary(&self) -> AccountingSummary {
        let state = self.state.read();
        AccountingSummary {
            fleet_total: state.fleet_total.clone(),
            pane_count: state.pane_usage.len(),
            task_count: state.task_records.len(),
            budget_status: if state.fleet_total.estimated_cost >= state.budget.hard_limit {
                BudgetStatus::Exceeded
            } else if state.fleet_total.estimated_cost >= state.budget.soft_limit {
                BudgetStatus::Warning
            } else {
                BudgetStatus::Ok
            },
            remaining_budget: (state.budget.hard_limit - state.fleet_total.estimated_cost).max(0.0),
            utilization: if state.budget.hard_limit > 0.0 {
                state.fleet_total.estimated_cost / state.budget.hard_limit
            } else {
                0.0
            },
        }
    }

    /// Clear all usage data (keeps budget config).
    pub fn clear(&self) {
        let mut state = self.state.write();
        state.pane_usage.clear();
        state.task_records.clear();
        state.fleet_total = TokenUsage::zero();
    }
}

impl Default for TokenAccountant {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary for the dashboard endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountingSummary {
    /// Fleet-wide total usage.
    pub fleet_total: TokenUsage,
    /// Number of tracked panes.
    pub pane_count: usize,
    /// Number of task records.
    pub task_count: usize,
    /// Current budget status.
    pub budget_status: BudgetStatus,
    /// Remaining budget in USD.
    pub remaining_budget: f64,
    /// Budget utilization fraction.
    pub utilization: f64,
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Losslessly convert `u64` to `f64` by splitting into two `u32` halves.
///
/// Each half converts exactly via `f64::from(u32)`. Values under 2^53
/// round-trip perfectly; larger values get the nearest representable `f64`.
fn u64_to_f64(v: u64) -> f64 {
    let high = f64::from(u32::try_from(v >> 32).unwrap_or(u32::MAX));
    let low = f64::from(u32::try_from(v & 0xFFFF_FFFF).unwrap_or(u32::MAX));
    high.mul_add(4_294_967_296.0, low)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── TokenUsage ──

    #[test]
    fn usage_zero() {
        let u = TokenUsage::zero();
        assert!(u.is_zero());
        assert_eq!(u.total_tokens, 0);
        assert!((u.estimated_cost - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn usage_from_counts() {
        let u = TokenUsage::from_counts(100, 50, 0.01, 0.02);
        assert_eq!(u.input_tokens, 100);
        assert_eq!(u.output_tokens, 50);
        assert_eq!(u.total_tokens, 150);
        // cost = 100*0.01 + 50*0.02 = 1.0 + 1.0 = 2.0
        assert!((u.estimated_cost - 2.0).abs() < 1e-6);
    }

    #[test]
    fn usage_add() {
        let mut a = TokenUsage::from_counts(10, 5, 0.01, 0.02);
        let b = TokenUsage::from_counts(20, 10, 0.01, 0.02);
        a.add(&b);
        assert_eq!(a.input_tokens, 30);
        assert_eq!(a.output_tokens, 15);
        assert_eq!(a.total_tokens, 45);
    }

    #[test]
    fn usage_is_zero_after_add_zero() {
        let mut a = TokenUsage::zero();
        a.add(&TokenUsage::zero());
        assert!(a.is_zero());
    }

    #[test]
    fn usage_default_is_zero() {
        let u = TokenUsage::default();
        assert!(u.is_zero());
    }

    #[test]
    fn usage_serializes() {
        let u = TokenUsage::from_counts(100, 50, 0.01, 0.02);
        let json = serde_json::to_string(&u);
        assert!(json.is_ok());
    }

    #[test]
    fn usage_roundtrip_json() {
        let u = TokenUsage::from_counts(100, 50, 0.01, 0.02);
        let json = serde_json::to_string(&u).unwrap();
        let back: TokenUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(back.input_tokens, 100);
        assert_eq!(back.output_tokens, 50);
    }

    #[test]
    fn usage_saturating_add() {
        let mut a = TokenUsage {
            input_tokens: u64::MAX - 1,
            output_tokens: 0,
            total_tokens: u64::MAX - 1,
            estimated_cost: 0.0,
        };
        let b = TokenUsage {
            input_tokens: 10,
            output_tokens: 0,
            total_tokens: 10,
            estimated_cost: 0.0,
        };
        a.add(&b);
        assert_eq!(a.input_tokens, u64::MAX);
    }

    // ── BudgetConfig ──

    #[test]
    fn budget_config_new() {
        let b = BudgetConfig::new();
        assert!((b.soft_limit - DEFAULT_SOFT_LIMIT).abs() < f64::EPSILON);
        assert!((b.hard_limit - DEFAULT_HARD_LIMIT).abs() < f64::EPSILON);
    }

    #[test]
    fn budget_config_default() {
        let b = BudgetConfig::default();
        assert!((b.soft_limit - DEFAULT_SOFT_LIMIT).abs() < f64::EPSILON);
    }

    #[test]
    fn budget_config_with_limits() {
        let b = BudgetConfig::with_limits(5.0, 20.0);
        assert!((b.soft_limit - 5.0).abs() < f64::EPSILON);
        assert!((b.hard_limit - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn budget_config_with_limits_hard_clamps() {
        let b = BudgetConfig::with_limits(10.0, 5.0);
        // hard should be >= soft
        assert!(b.hard_limit >= b.soft_limit);
    }

    #[test]
    fn budget_config_with_limits_negative_clamps() {
        let b = BudgetConfig::with_limits(-5.0, 10.0);
        assert!(b.soft_limit >= 0.0);
    }

    #[test]
    fn budget_config_validate_ok() {
        let b = BudgetConfig::new();
        assert!(b.validate().is_ok());
    }

    #[test]
    fn budget_config_validate_negative_soft() {
        let b = BudgetConfig {
            soft_limit: -1.0,
            ..BudgetConfig::new()
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn budget_config_validate_hard_less_than_soft() {
        let b = BudgetConfig {
            soft_limit: 20.0,
            hard_limit: 10.0,
            ..BudgetConfig::new()
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn budget_config_validate_negative_cost() {
        let b = BudgetConfig {
            input_cost: -0.01,
            ..BudgetConfig::new()
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn budget_config_validate_nan_cost() {
        let b = BudgetConfig {
            output_cost: f64::NAN,
            ..BudgetConfig::new()
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn budget_config_serializes() {
        let b = BudgetConfig::new();
        let json = serde_json::to_string(&b);
        assert!(json.is_ok());
    }

    // ── BudgetStatus ──

    #[test]
    fn budget_status_ok_allows_work() {
        assert!(BudgetStatus::Ok.allows_work());
    }

    #[test]
    fn budget_status_warning_allows_work() {
        assert!(BudgetStatus::Warning.allows_work());
    }

    #[test]
    fn budget_status_exceeded_blocks_work() {
        assert!(!BudgetStatus::Exceeded.allows_work());
    }

    // ── TokenAccountant ──

    #[test]
    fn accountant_new() {
        let a = TokenAccountant::new();
        assert!(a.fleet_total().is_zero());
        assert_eq!(a.tracked_pane_count(), 0);
        assert_eq!(a.task_record_count(), 0);
    }

    #[test]
    fn accountant_default() {
        let a = TokenAccountant::default();
        assert!(a.fleet_total().is_zero());
    }

    #[test]
    fn accountant_with_budget() {
        let b = BudgetConfig::with_limits(5.0, 20.0);
        let a = TokenAccountant::with_budget(b);
        assert!(a.is_ok());
    }

    #[test]
    fn accountant_with_invalid_budget() {
        let b = BudgetConfig {
            soft_limit: -1.0,
            ..BudgetConfig::new()
        };
        let a = TokenAccountant::with_budget(b);
        assert!(a.is_err());
    }

    #[test]
    fn accountant_record_pane_usage() {
        let a = TokenAccountant::new();
        let pane = PaneId::new("fleet-alpha");
        assert!(a.record_pane_usage(&pane, 100, 50).is_ok());
        let usage = a.pane_usage(&pane);
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
    }

    #[test]
    fn accountant_pane_usage_accumulates() {
        let a = TokenAccountant::new();
        let pane = PaneId::new("fleet-alpha");
        a.record_pane_usage(&pane, 100, 50).unwrap();
        a.record_pane_usage(&pane, 200, 100).unwrap();
        let usage = a.pane_usage(&pane);
        assert_eq!(usage.input_tokens, 300);
        assert_eq!(usage.output_tokens, 150);
    }

    #[test]
    fn accountant_fleet_total() {
        let a = TokenAccountant::new();
        a.record_pane_usage(&PaneId::new("a"), 100, 50).unwrap();
        a.record_pane_usage(&PaneId::new("b"), 200, 100).unwrap();
        let total = a.fleet_total();
        assert_eq!(total.input_tokens, 300);
        assert_eq!(total.output_tokens, 150);
    }

    #[test]
    fn accountant_record_task_usage() {
        let a = TokenAccountant::new();
        let task = TaskId::from_existing("task-001");
        let pane = PaneId::new("fleet-alpha");
        assert!(a.record_task_usage(&task, &pane, 500, 200, "test task").is_ok());
        assert_eq!(a.task_record_count(), 1);
        // Also added to pane and fleet totals
        assert_eq!(a.pane_usage(&pane).input_tokens, 500);
        assert_eq!(a.fleet_total().total_tokens, 700);
    }

    #[test]
    fn accountant_recent_tasks() {
        let a = TokenAccountant::new();
        let pane = PaneId::new("test");
        for i in 0..5 {
            let task = TaskId::from_existing(format!("t-{i}"));
            a.record_task_usage(&task, &pane, 10, 5, &format!("task {i}")).unwrap();
        }
        let recent = a.recent_tasks(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].task_id.as_str(), "t-4");
    }

    #[test]
    fn accountant_all_pane_usage() {
        let a = TokenAccountant::new();
        a.record_pane_usage(&PaneId::new("a"), 10, 5).unwrap();
        a.record_pane_usage(&PaneId::new("b"), 20, 10).unwrap();
        let all = a.all_pane_usage();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn accountant_unknown_pane_zero() {
        let a = TokenAccountant::new();
        let usage = a.pane_usage(&PaneId::new("nonexistent"));
        assert!(usage.is_zero());
    }

    #[test]
    fn accountant_budget_status_ok() {
        let a = TokenAccountant::new();
        assert_eq!(a.budget_status(), BudgetStatus::Ok);
    }

    #[test]
    fn accountant_budget_status_warning() {
        let budget = BudgetConfig::with_limits(0.0001, 10.0);
        let a = TokenAccountant::with_budget(budget).unwrap();
        a.record_pane_usage(&PaneId::new("a"), 1000, 500).unwrap();
        assert_eq!(a.budget_status(), BudgetStatus::Warning);
    }

    #[test]
    fn accountant_remaining_budget() {
        let a = TokenAccountant::new();
        let remaining = a.remaining_budget();
        assert!((remaining - DEFAULT_HARD_LIMIT).abs() < 1e-6);
    }

    #[test]
    fn accountant_budget_utilization_zero() {
        let a = TokenAccountant::new();
        assert!((a.budget_utilization() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn accountant_set_budget() {
        let a = TokenAccountant::new();
        let new_budget = BudgetConfig::with_limits(100.0, 200.0);
        assert!(a.set_budget(new_budget).is_ok());
        assert!((a.budget().hard_limit - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn accountant_set_invalid_budget() {
        let a = TokenAccountant::new();
        let bad_budget = BudgetConfig {
            soft_limit: -5.0,
            ..BudgetConfig::new()
        };
        assert!(a.set_budget(bad_budget).is_err());
    }

    #[test]
    fn accountant_summary() {
        let a = TokenAccountant::new();
        a.record_pane_usage(&PaneId::new("a"), 100, 50).unwrap();
        let summary = a.summary();
        assert_eq!(summary.pane_count, 1);
        assert_eq!(summary.fleet_total.total_tokens, 150);
        assert_eq!(summary.budget_status, BudgetStatus::Ok);
    }

    #[test]
    fn accountant_summary_serializes() {
        let a = TokenAccountant::new();
        let summary = a.summary();
        let json = serde_json::to_string(&summary);
        assert!(json.is_ok());
    }

    #[test]
    fn accountant_clear() {
        let a = TokenAccountant::new();
        a.record_pane_usage(&PaneId::new("a"), 100, 50).unwrap();
        a.clear();
        assert!(a.fleet_total().is_zero());
        assert_eq!(a.tracked_pane_count(), 0);
        assert_eq!(a.task_record_count(), 0);
    }

    // ── Task record description truncation ──

    #[test]
    fn task_record_description_truncated() {
        let a = TokenAccountant::new();
        let long_desc = "x".repeat(500);
        a.record_task_usage(
            &TaskId::from_existing("t"),
            &PaneId::new("p"),
            10,
            5,
            &long_desc,
        )
        .unwrap();
        let records = a.recent_tasks(1);
        assert!(records[0].description.len() <= 256);
    }

    // ── Thread safety ──

    #[test]
    fn accountant_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TokenAccountant>();
    }

    #[test]
    fn accountant_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TokenAccountant>();
    }

    // ── Constants ──

    #[test]
    fn default_input_cost_reasonable() {
        assert!(DEFAULT_INPUT_COST > 0.0);
        assert!(DEFAULT_INPUT_COST < 1.0);
    }

    #[test]
    fn default_output_cost_reasonable() {
        assert!(DEFAULT_OUTPUT_COST > 0.0);
        assert!(DEFAULT_OUTPUT_COST < 1.0);
    }

    #[test]
    fn output_cost_higher_than_input() {
        assert!(DEFAULT_OUTPUT_COST > DEFAULT_INPUT_COST);
    }

    #[test]
    fn max_tracked_panes_reasonable() {
        assert!(MAX_TRACKED_PANES >= 32);
        assert!(MAX_TRACKED_PANES <= 1024);
    }

    #[test]
    fn default_limits_ordered() {
        assert!(DEFAULT_SOFT_LIMIT < DEFAULT_HARD_LIMIT);
    }

    // ── TaskTokenRecord ──

    #[test]
    fn task_token_record_serializes() {
        let r = TaskTokenRecord {
            task_id: TaskId::from_existing("t-1"),
            pane_id: PaneId::new("p-1"),
            usage: TokenUsage::from_counts(100, 50, 0.01, 0.02),
            description: "test".into(),
        };
        let json = serde_json::to_string(&r);
        assert!(json.is_ok());
    }

    // ── FIFO eviction ──

    #[test]
    fn task_records_fifo_eviction() {
        let budget = BudgetConfig::with_limits(999_999.0, 999_999.0);
        let a = TokenAccountant::with_budget(budget).unwrap();
        let pane = PaneId::new("p");
        for i in 0..(MAX_TASK_RECORDS + 10) {
            let t = TaskId::from_existing(format!("t-{i}"));
            a.record_task_usage(&t, &pane, 1, 0, "x").unwrap();
        }
        assert!(a.task_record_count() <= MAX_TASK_RECORDS);
    }
}
