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

/// Phase proximity threshold for cluster grouping (pi/6 radians = 30 degrees).
const CLUSTER_PROXIMITY: f64 = std::f64::consts::FRAC_PI_6;

/// Minimum phase gap between clusters to indicate chimera state (pi/3 radians).
const CHIMERA_GAP: f64 = std::f64::consts::FRAC_PI_3;

/// Global `r` threshold below which chimera detection is enabled.
/// When `r >= 0.95` the field is nearly synchronized — no chimera possible.
const CHIMERA_R_THRESHOLD: f64 = 0.95;

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
        // BUG-L1-010: n is always ≤ SPHERE_CAP (200), so cast is lossless.
        // BUG-M004 fix: explicit guard + debug_assert prevents division by zero
        // if the early return above is ever removed during refactoring.
        debug_assert!(n > 0, "compute called with empty spheres (early return should have triggered)");
        #[allow(clippy::cast_precision_loss)]
        let count = n as f64;
        let r = if count > 0.0 {
            sin_sum.mul_add(sin_sum, cos_sum * cos_sum).sqrt() / count
        } else {
            0.0
        };
        let psi = sin_sum.atan2(cos_sum);

        let order = OrderParameter {
            r: r.clamp(0.0, 1.0),
            psi,
        };

        let harmonics = Self::compute_harmonics(spheres, &order);

        Self {
            order,
            tick,
            fleet_mode: FleetMode::from_count(n),
            r_trend: RTrend::default(),
            recent_decisions: Vec::new(),
            harmonics,
        }
    }

    /// Compute harmonic decomposition by clustering spheres by phase proximity.
    ///
    /// Groups spheres whose phases are within [`CLUSTER_PROXIMITY`] (pi/6) of
    /// each other, computes a per-cluster order parameter, and detects chimera
    /// states (2+ clusters with gap > pi/3 while global `r` < 0.95).
    fn compute_harmonics(
        spheres: &HashMap<PaneId, PaneSphere>,
        global_order: &OrderParameter,
    ) -> Harmonics {
        if spheres.is_empty() {
            return Harmonics::default();
        }

        // Collect and sort phases (wrapped to [0, 2pi))
        let mut phases: Vec<f64> = spheres
            .values()
            .map(|sp| sp.phase.rem_euclid(std::f64::consts::TAU))
            .collect();
        phases.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Greedy cluster assignment: walk sorted phases, start new cluster
        // when angular distance exceeds CLUSTER_PROXIMITY.
        let mut clusters: Vec<Vec<f64>> = Vec::new();
        let mut current_cluster: Vec<f64> = vec![phases[0]];

        for &phase in &phases[1..] {
            let last = current_cluster.last().copied().unwrap_or(0.0);
            let gap = phase - last;
            if gap <= CLUSTER_PROXIMITY {
                current_cluster.push(phase);
            } else {
                clusters.push(std::mem::take(&mut current_cluster));
                current_cluster = vec![phase];
            }
        }
        clusters.push(current_cluster);

        // Check wrap-around: if the gap between the last phase and the first
        // phase (modulo 2pi) is within proximity, merge the first and last clusters.
        if clusters.len() >= 2 {
            let last_cluster_last = clusters.last().and_then(|c| c.last().copied()).unwrap_or(0.0);
            let first_cluster_first = clusters.first().and_then(|c| c.first().copied()).unwrap_or(0.0);
            let wrap_gap = (first_cluster_first + std::f64::consts::TAU) - last_cluster_last;
            if wrap_gap <= CLUSTER_PROXIMITY {
                let first = clusters.remove(0);
                if let Some(last) = clusters.last_mut() {
                    last.extend(first);
                }
            }
        }

        // Compute per-cluster order parameter
        let cluster_orders: Vec<OrderParameter> = clusters
            .iter()
            .map(|cluster| {
                #[allow(clippy::cast_precision_loss)] // cluster sizes are small
                let n = cluster.len() as f64;
                if n < 1.0 {
                    return OrderParameter::new(0.0, 0.0);
                }
                let (sin_sum, cos_sum) =
                    cluster.iter().fold((0.0_f64, 0.0_f64), |(s, c), &ph| {
                        (s + ph.sin(), c + ph.cos())
                    });
                let r = sin_sum.mul_add(sin_sum, cos_sum * cos_sum).sqrt() / n;
                let psi = sin_sum.atan2(cos_sum);
                OrderParameter::new(r.clamp(0.0, 1.0), psi)
            })
            .collect();

        // Chimera detection: 2+ clusters with phase gap > pi/3 while global r < 0.95
        let chimera_detected = if clusters.len() >= 2 && global_order.r < CHIMERA_R_THRESHOLD {
            // Check if any pair of cluster centroids has a gap exceeding CHIMERA_GAP.
            // We use the centroid psi from each cluster's order parameter.
            let centroids: Vec<f64> = cluster_orders.iter().map(|o| o.psi).collect();
            let mut found = false;
            for i in 0..centroids.len() {
                for j in (i + 1)..centroids.len() {
                    let diff = (centroids[i] - centroids[j]).abs();
                    let angular = diff.min(std::f64::consts::TAU - diff);
                    if angular > CHIMERA_GAP {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            found
        } else {
            false
        };

        let cluster_count = cluster_orders.len();

        Harmonics {
            clusters: cluster_orders,
            chimera_detected,
            cluster_count,
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

    // ── Harmonics computation ──

    #[test]
    fn compute_harmonics_empty() {
        let spheres = HashMap::new();
        let fs = FieldState::compute(&spheres, 0);
        assert!(fs.harmonics.clusters.is_empty());
        assert!(!fs.harmonics.chimera_detected);
        assert_eq!(fs.harmonics.cluster_count, 0);
    }

    #[test]
    fn compute_harmonics_single_sphere() {
        let mut spheres = HashMap::new();
        spheres.insert(
            PaneId::new("a"),
            PaneSphere { phase: 1.0, ..PaneSphere::default() },
        );
        let fs = FieldState::compute(&spheres, 1);
        assert_eq!(fs.harmonics.cluster_count, 1);
        assert!(!fs.harmonics.chimera_detected);
        assert_eq!(fs.harmonics.clusters.len(), 1);
        // Single sphere cluster → r = 1.0
        assert!((fs.harmonics.clusters[0].r - 1.0).abs() < 0.01);
    }

    #[test]
    fn compute_harmonics_synchronized_field() {
        // All phases at ~0.5 rad → single cluster, no chimera
        let mut spheres = HashMap::new();
        for i in 0..5 {
            spheres.insert(
                PaneId::new(format!("s{i}")),
                PaneSphere {
                    phase: 0.5 + (i as f64 * 0.01),
                    ..PaneSphere::default()
                },
            );
        }
        let fs = FieldState::compute(&spheres, 1);
        assert_eq!(fs.harmonics.cluster_count, 1);
        assert!(!fs.harmonics.chimera_detected);
        assert!(fs.harmonics.clusters[0].r > 0.99);
    }

    #[test]
    fn compute_harmonics_two_clusters() {
        // Two groups: phase ~0.0 and phase ~pi → 2 clusters
        let mut spheres = HashMap::new();
        for i in 0..3 {
            spheres.insert(
                PaneId::new(format!("a{i}")),
                PaneSphere {
                    phase: 0.1 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        for i in 0..3 {
            spheres.insert(
                PaneId::new(format!("b{i}")),
                PaneSphere {
                    phase: std::f64::consts::PI + 0.1 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        let fs = FieldState::compute(&spheres, 1);
        assert!(fs.harmonics.cluster_count >= 2);
    }

    #[test]
    fn compute_harmonics_chimera_detected() {
        // Two well-separated clusters with low global r → chimera
        let mut spheres = HashMap::new();
        for i in 0..4 {
            spheres.insert(
                PaneId::new(format!("a{i}")),
                PaneSphere {
                    phase: 0.0 + 0.05 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        for i in 0..4 {
            spheres.insert(
                PaneId::new(format!("b{i}")),
                PaneSphere {
                    phase: std::f64::consts::PI + 0.05 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        let fs = FieldState::compute(&spheres, 1);
        // Global r should be low with two opposing clusters
        assert!(fs.order.r < 0.5);
        assert!(fs.harmonics.chimera_detected);
        assert!(fs.harmonics.cluster_count >= 2);
    }

    #[test]
    fn compute_harmonics_no_chimera_when_highly_synced() {
        // All phases tightly packed → high global r, no chimera
        let mut spheres = HashMap::new();
        for i in 0..10 {
            spheres.insert(
                PaneId::new(format!("s{i}")),
                PaneSphere {
                    phase: 1.0 + 0.001 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        let fs = FieldState::compute(&spheres, 1);
        assert!(fs.order.r > 0.95);
        assert!(!fs.harmonics.chimera_detected);
    }

    #[test]
    fn compute_harmonics_per_cluster_r() {
        // Two tight clusters → each cluster has high r
        let mut spheres = HashMap::new();
        for i in 0..3 {
            spheres.insert(
                PaneId::new(format!("a{i}")),
                PaneSphere {
                    phase: 0.5 + 0.01 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        for i in 0..3 {
            spheres.insert(
                PaneId::new(format!("b{i}")),
                PaneSphere {
                    phase: 3.5 + 0.01 * i as f64,
                    ..PaneSphere::default()
                },
            );
        }
        let fs = FieldState::compute(&spheres, 1);
        for cluster_order in &fs.harmonics.clusters {
            assert!(cluster_order.r > 0.99, "each tight cluster should have high r");
        }
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
