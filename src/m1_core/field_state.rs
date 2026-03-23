//! # ORAC Field State
//!
//! Sidecar-native field state types, replacing PV2's `m3_field` module.
//! The sidecar observes and caches field state from the PV2 daemon;
//! it does not own the authoritative field.
//!
//! ## Layer: L1 (Core) | Dependencies: `m01_core_types`

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use super::m01_core_types::{
    DecisionRecord, FieldAction, FleetMode, OrderParameter, PaneId, PaneSphere, RTrend,
};
use super::m04_constants;

// ──────────────────────────────────────────────────────────────
// Harmonics — sub-cluster analysis
// ──────────────────────────────────────────────────────────────

/// Harmonic decomposition of the field (per-cluster order parameters).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Harmonics {
    /// Per-cluster order parameters.
    pub clusters: Vec<OrderParameter>,
    /// Chimera detection flag.
    pub chimera_detected: bool,
    /// Number of clusters found.
    pub cluster_count: usize,
}

// ──────────────────────────────────────────────────────────────
// FieldState — cached snapshot from PV2
// ──────────────────────────────────────────────────────────────

/// Cached field state snapshot, updated from PV2 daemon ticks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FieldState {
    /// Kuramoto order parameter (synchronization measure).
    #[serde(alias = "order_parameter")]
    pub order: OrderParameter,
    /// Current tick number.
    pub tick: u64,
    /// Current fleet mode based on sphere count.
    pub fleet_mode: FleetMode,
    /// R-trend direction.
    pub r_trend: RTrend,
    /// Recent decision records for audit trail.
    pub recent_decisions: Vec<DecisionRecord>,
    /// Harmonic decomposition.
    pub harmonics: Harmonics,
}

impl FieldState {
    /// Compute field state from current sphere phases.
    ///
    /// Calculates global order parameter, harmonics, and fleet mode.
    #[must_use]
    pub fn compute(spheres: &HashMap<PaneId, PaneSphere>, tick: u64) -> Self {
        let n = spheres.len();
        if n == 0 {
            return Self { tick, ..Self::default() };
        }

        // Kuramoto order parameter: r * e^(i*psi) = (1/N) * sum(e^(i*theta_j))
        let (sin_sum, cos_sum) = spheres.values().fold((0.0_f64, 0.0_f64), |(s, c), sp| {
            (s + sp.phase.sin(), c + sp.phase.cos())
        });
        // BUG-L1-010: n is always ≤ SPHERE_CAP (200), so cast is lossless
        #[allow(clippy::cast_precision_loss)]
        let count = n as f64;
        let r = sin_sum.mul_add(sin_sum, cos_sum * cos_sum).sqrt() / count;
        let psi = sin_sum.atan2(cos_sum);

        let order = OrderParameter {
            r: r.clamp(0.0, 1.0),
            psi,
        };

        Self {
            order,
            tick,
            fleet_mode: FleetMode::from_count(n),
            r_trend: RTrend::default(),
            recent_decisions: Vec::new(),
            harmonics: Harmonics::default(),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// FieldDecision — conductor output
// ──────────────────────────────────────────────────────────────

/// A field-level decision produced by the conductor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDecision {
    /// The action to take.
    pub action: FieldAction,
    /// Coupling strength adjustment delta.
    pub k_delta: f64,
    /// Reason string for audit trail.
    pub reason: String,
}

impl Default for FieldDecision {
    fn default() -> Self {
        Self {
            action: FieldAction::Stable,
            k_delta: 0.0,
            reason: String::new(),
        }
    }
}

impl FieldDecision {
    /// Create a "recovering" decision.
    #[must_use]
    pub fn recovering(reason: impl Into<String>) -> Self {
        Self {
            action: FieldAction::Recovering,
            k_delta: 0.0,
            reason: reason.into(),
        }
    }

    /// Create a stable decision with optional message.
    #[must_use]
    pub fn stable(reason: impl Into<String>) -> Self {
        Self {
            action: FieldAction::Stable,
            k_delta: 0.0,
            reason: reason.into(),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// AppState — sidecar application state
// ──────────────────────────────────────────────────────────────

/// ORAC sidecar application state.
///
/// Unlike PV2's `AppState` which owns the authoritative field,
/// ORAC's `AppState` caches observed state from the daemon and
/// manages sidecar-local state (hook tracking, bridge health, etc.).
#[derive(Debug)]
pub struct AppState {
    /// Known spheres (cached from PV2 daemon).
    pub spheres: HashMap<PaneId, PaneSphere>,
    /// Cached field state.
    pub field: FieldState,
    /// Current tick counter (local shadow).
    pub tick: u64,
    /// Sidecar start time.
    pub started_at: f64,
    /// Optional R-target override from governance.
    pub r_target_override: Option<f64>,
    /// EMA of divergence signal.
    pub divergence_ema: f64,
    /// EMA of coherence signal.
    pub coherence_ema: f64,
    /// Divergence cooldown counter (ticks remaining).
    pub divergence_cooldown: u32,
    /// Previous decision action for thrashing detection.
    pub prev_decision_action: FieldAction,
    /// R-history ring buffer for trend analysis.
    pub r_history: VecDeque<f64>,
    /// Warmup ticks remaining.
    warmup_remaining: u32,
    /// Consecutive failed PV2 polls (reset to 0 on success).
    pub consecutive_misses: u32,
}

/// Default warmup ticks before conductor is active.
/// Uses the canonical value from `m04_constants`.
const WARMUP_TICKS: u32 = m04_constants::WARMUP_TICKS;

impl Default for AppState {
    fn default() -> Self {
        Self {
            spheres: HashMap::new(),
            field: FieldState::default(),
            tick: 0,
            started_at: super::m01_core_types::now_secs(),
            r_target_override: None,
            divergence_ema: 0.0,
            coherence_ema: 0.0,
            divergence_cooldown: 0,
            prev_decision_action: FieldAction::Stable,
            r_history: VecDeque::with_capacity(60),
            warmup_remaining: WARMUP_TICKS,
            consecutive_misses: 0,
        }
    }
}

impl AppState {
    /// Whether the sidecar is still in warmup phase.
    #[must_use]
    pub fn is_warming_up(&self) -> bool {
        self.warmup_remaining > 0
    }

    /// Advance warmup counter by one tick.
    pub fn tick_warmup(&mut self) {
        self.warmup_remaining = self.warmup_remaining.saturating_sub(1);
    }

    /// Push a new R value into the history ring buffer.
    pub fn push_r(&mut self, r: f64) {
        if self.r_history.len() >= 60 {
            self.r_history.pop_front();
        }
        self.r_history.push_back(r);
    }

    /// Update exponential moving averages for divergence and coherence signals.
    ///
    /// `divergence` and `coherence` are instantaneous signal values (typically
    /// derived from `r_target - r` and `r` respectively). The EMA smoothing
    /// uses `alpha` = 0.2 (matching conductor constants).
    pub fn update_emas(&mut self, divergence: f64, coherence: f64) {
        const ALPHA: f64 = 0.2;
        self.divergence_ema = ALPHA.mul_add(divergence, (1.0 - ALPHA) * self.divergence_ema);
        self.coherence_ema = ALPHA.mul_add(coherence, (1.0 - ALPHA) * self.coherence_ema);
    }

    /// Decrement divergence cooldown by one tick (saturating at 0).
    ///
    /// Must be called once per tick to allow the conductor to exit
    /// `Recovering` state after the cooldown period expires.
    pub fn tick_cooldown(&mut self) {
        self.divergence_cooldown = self.divergence_cooldown.saturating_sub(1);
    }

    /// Record a successful PV2 poll (resets miss counter).
    pub fn record_poll_success(&mut self) {
        self.consecutive_misses = 0;
    }

    /// Record a missed PV2 poll (increments miss counter).
    pub fn record_poll_miss(&mut self) {
        self.consecutive_misses = self.consecutive_misses.saturating_add(1);
    }

    /// Whether the cached field state is stale (3+ consecutive missed polls = 15s+).
    #[must_use]
    pub const fn is_stale(&self) -> bool {
        self.consecutive_misses >= STALE_THRESHOLD
    }

    /// Number of consecutive missed PV2 polls.
    #[must_use]
    pub const fn consecutive_misses(&self) -> u32 {
        self.consecutive_misses
    }
}

/// Number of consecutive missed polls before field state is considered stale.
const STALE_THRESHOLD: u32 = 3;

/// Thread-safe shared application state.
pub type SharedState = Arc<RwLock<AppState>>;

/// Create a new shared state instance.
#[must_use]
pub fn new_shared_state() -> SharedState {
    Arc::new(RwLock::new(AppState::default()))
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_state_default() {
        let fs = FieldState::default();
        assert_eq!(fs.tick, 0);
        assert!(fs.recent_decisions.is_empty());
    }

    #[test]
    fn field_state_no_duplicate_order_field() {
        let fs = FieldState::default();
        // Only `order` exists now — no `order_parameter` divergence hazard
        assert!(fs.order.r.abs() < f64::EPSILON);
    }

    #[test]
    fn field_state_serde_alias_backward_compat() {
        // JSON with old "order_parameter" key should deserialize into "order"
        let json = r#"{"order_parameter":{"r":0.85,"psi":1.2},"tick":42,"fleet_mode":"Solo","r_trend":"Stable","recent_decisions":[],"harmonics":{"clusters":[],"chimera_detected":false,"cluster_count":0}}"#;
        let fs: FieldState = serde_json::from_str(json).unwrap();
        assert!((fs.order.r - 0.85).abs() < f64::EPSILON);
        assert_eq!(fs.tick, 42);
    }

    #[test]
    fn field_state_compute_empty() {
        let spheres = HashMap::new();
        let fs = FieldState::compute(&spheres, 42);
        assert_eq!(fs.tick, 42);
    }

    #[test]
    fn field_decision_default_is_stable() {
        let fd = FieldDecision::default();
        assert_eq!(fd.action, FieldAction::Stable);
        assert!((fd.k_delta).abs() < f64::EPSILON);
    }

    #[test]
    fn field_decision_recovering() {
        let fd = FieldDecision::recovering("test");
        assert_eq!(fd.action, FieldAction::Recovering);
    }

    #[test]
    fn app_state_default() {
        let state = AppState::default();
        assert!(state.spheres.is_empty());
        assert_eq!(state.tick, 0);
        assert!(state.is_warming_up());
    }

    #[test]
    fn app_state_warmup() {
        let mut state = AppState::default();
        assert!(state.is_warming_up());
        for _ in 0..WARMUP_TICKS {
            state.tick_warmup();
        }
        assert!(!state.is_warming_up());
    }

    #[test]
    fn app_state_r_history() {
        let mut state = AppState::default();
        for i in 0..70 {
            state.push_r(i as f64 * 0.01);
        }
        assert_eq!(state.r_history.len(), 60);
    }

    #[test]
    fn shared_state_creation() {
        let state = new_shared_state();
        let guard = state.read();
        assert!(guard.spheres.is_empty());
    }

    // ── Staleness detection ──

    #[test]
    fn fresh_state_is_not_stale() {
        let state = AppState::default();
        assert!(!state.is_stale());
        assert_eq!(state.consecutive_misses(), 0);
    }

    #[test]
    fn one_miss_is_not_stale() {
        let mut state = AppState::default();
        state.record_poll_miss();
        assert!(!state.is_stale());
        assert_eq!(state.consecutive_misses(), 1);
    }

    #[test]
    fn two_misses_is_not_stale() {
        let mut state = AppState::default();
        state.record_poll_miss();
        state.record_poll_miss();
        assert!(!state.is_stale());
        assert_eq!(state.consecutive_misses(), 2);
    }

    #[test]
    fn three_misses_is_stale() {
        let mut state = AppState::default();
        for _ in 0..3 {
            state.record_poll_miss();
        }
        assert!(state.is_stale());
        assert_eq!(state.consecutive_misses(), 3);
    }

    #[test]
    fn success_resets_miss_counter() {
        let mut state = AppState::default();
        state.record_poll_miss();
        state.record_poll_miss();
        assert_eq!(state.consecutive_misses(), 2);

        state.record_poll_success();
        assert_eq!(state.consecutive_misses(), 0);
        assert!(!state.is_stale());
    }

    #[test]
    fn success_after_stale_clears_staleness() {
        let mut state = AppState::default();
        for _ in 0..5 {
            state.record_poll_miss();
        }
        assert!(state.is_stale());
        assert_eq!(state.consecutive_misses(), 5);

        state.record_poll_success();
        assert!(!state.is_stale());
        assert_eq!(state.consecutive_misses(), 0);
    }

    // ── EMA updates ──

    #[test]
    fn update_emas_from_zero() {
        let mut state = AppState::default();
        state.update_emas(1.0, 0.5);
        // alpha=0.2, so: 0.2*1.0 + 0.8*0.0 = 0.2
        assert!((state.divergence_ema - 0.2).abs() < 1e-10);
        // 0.2*0.5 + 0.8*0.0 = 0.1
        assert!((state.coherence_ema - 0.1).abs() < 1e-10);
    }

    #[test]
    fn update_emas_converges() {
        let mut state = AppState::default();
        for _ in 0..100 {
            state.update_emas(1.0, 1.0);
        }
        // After many iterations, EMA converges to the signal value
        assert!((state.divergence_ema - 1.0).abs() < 0.01);
        assert!((state.coherence_ema - 1.0).abs() < 0.01);
    }

    // ── Cooldown decrement ──

    #[test]
    fn tick_cooldown_decrements() {
        let mut state = AppState::default();
        state.divergence_cooldown = 3;
        state.tick_cooldown();
        assert_eq!(state.divergence_cooldown, 2);
        state.tick_cooldown();
        assert_eq!(state.divergence_cooldown, 1);
        state.tick_cooldown();
        assert_eq!(state.divergence_cooldown, 0);
    }

    #[test]
    fn tick_cooldown_saturates_at_zero() {
        let mut state = AppState::default();
        assert_eq!(state.divergence_cooldown, 0);
        state.tick_cooldown();
        assert_eq!(state.divergence_cooldown, 0);
    }

    #[test]
    fn tick_cooldown_exits_recovering() {
        let mut state = AppState::default();
        state.divergence_cooldown = 1;
        assert!(state.divergence_cooldown > 0);
        state.tick_cooldown();
        assert_eq!(state.divergence_cooldown, 0);
    }

    #[test]
    fn miss_counter_saturates() {
        let mut state = AppState::default();
        for _ in 0..100_000 {
            state.record_poll_miss();
        }
        assert!(state.is_stale());
        // Should not overflow
        assert!(state.consecutive_misses() >= 3);
    }
}
