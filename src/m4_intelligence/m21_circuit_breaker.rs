//! # M21: Circuit Breaker
//!
//! Per-pane health gating with a Closed/Open/`HalfOpen` finite state machine.
//! Prevents dispatch to failing panes and auto-recovers via probe requests.
//!
//! ## State Machine
//!
//! ```text
//! ┌────────┐  failure >= threshold  ┌──────┐  timeout  ┌──────────┐
//! │ Closed │ ────────────────────→ │ Open │ ────────→ │ HalfOpen │
//! │        │ ←──────────────────── │      │           │          │
//! └────────┘     success resets    └──────┘           └──────────┘
//!     ↑                                                    │
//!     │              probe succeeds                        │
//!     └────────────────────────────────────────────────────┘
//!                    probe fails → back to Open
//! ```
//!
//! ## Layer: L4 (Intelligence)
//! ## Module: M21
//! ## Dependencies: `m01_core_types`

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::PaneId;

// ──────────────────────────────────────────────────────────────
// Circuit breaker state
// ──────────────────────────────────────────────────────────────

/// Circuit breaker state for a single pane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BreakerState {
    /// Normal operation — requests flow through.
    #[default]
    Closed,
    /// Tripped — requests are rejected to protect the system.
    Open,
    /// Recovery probe — a single test request is allowed through.
    HalfOpen,
}

impl std::fmt::Display for BreakerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "Closed"),
            Self::Open => write!(f, "Open"),
            Self::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Configuration
// ──────────────────────────────────────────────────────────────

/// Configuration for a circuit breaker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerConfig {
    /// Number of consecutive failures before opening the circuit.
    pub failure_threshold: u32,
    /// Number of consecutive successes in `HalfOpen` to close the circuit.
    pub success_threshold: u32,
    /// Ticks to wait in Open state before transitioning to `HalfOpen`.
    pub open_timeout_ticks: u64,
    /// Maximum number of requests allowed through in `HalfOpen` state.
    pub half_open_max_requests: u32,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_timeout_ticks: 30,
            half_open_max_requests: 1,
        }
    }
}

impl BreakerConfig {
    /// Create a configuration tuned for aggressive failure detection.
    #[must_use]
    pub const fn aggressive() -> Self {
        Self {
            failure_threshold: 3,
            success_threshold: 1,
            open_timeout_ticks: 15,
            half_open_max_requests: 1,
        }
    }

    /// Create a configuration tuned for tolerant/slow services.
    #[must_use]
    pub const fn tolerant() -> Self {
        Self {
            failure_threshold: 10,
            success_threshold: 3,
            open_timeout_ticks: 60,
            half_open_max_requests: 2,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Per-pane breaker
// ──────────────────────────────────────────────────────────────

/// Circuit breaker instance for a single pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneBreaker {
    /// Current state.
    state: BreakerState,
    /// Consecutive failure count (reset on success).
    consecutive_failures: u32,
    /// Consecutive success count in `HalfOpen` state.
    half_open_successes: u32,
    /// Requests allowed through in current `HalfOpen` window.
    half_open_requests: u32,
    /// Tick at which the circuit was opened.
    opened_at_tick: u64,
    /// Most recent tick seen via `tick_check` or `record_failure_at` (BUG-L4-002 fallback).
    last_known_tick: u64,
    /// Total failure count (lifetime, never reset).
    total_failures: u64,
    /// Total success count (lifetime, never reset).
    total_successes: u64,
    /// Configuration.
    config: BreakerConfig,
}

impl PaneBreaker {
    /// Create a new breaker in `Closed` state with given config.
    #[must_use]
    pub fn new(config: BreakerConfig) -> Self {
        Self {
            state: BreakerState::Closed,
            consecutive_failures: 0,
            half_open_successes: 0,
            half_open_requests: 0,
            opened_at_tick: 0,
            last_known_tick: 0,
            total_failures: 0,
            total_successes: 0,
            config,
        }
    }

    /// Current breaker state.
    #[must_use]
    pub const fn state(&self) -> BreakerState {
        self.state
    }

    /// Whether this breaker allows requests through.
    #[must_use]
    pub const fn allows_request(&self) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::Open => false,
            BreakerState::HalfOpen => self.half_open_requests < self.config.half_open_max_requests,
        }
    }

    /// Consecutive failure count.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Total lifetime failures.
    #[must_use]
    pub const fn total_failures(&self) -> u64 {
        self.total_failures
    }

    /// Total lifetime successes.
    #[must_use]
    pub const fn total_successes(&self) -> u64 {
        self.total_successes
    }

    /// Record a successful operation.
    pub fn record_success(&mut self) {
        self.total_successes = self.total_successes.saturating_add(1);

        match self.state {
            BreakerState::Closed => {
                // Reset failure counter on success
                self.consecutive_failures = 0;
            }
            BreakerState::Open => {
                // Shouldn't happen (requests blocked), but treat as recovery
                self.transition_to_closed();
            }
            BreakerState::HalfOpen => {
                self.half_open_successes += 1;
                if self.half_open_successes >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
        }
    }

    /// Record a failed operation.
    ///
    /// Uses `last_known_tick` for Open-state timeout tracking. Prefer
    /// `record_failure_at()` when the current tick is available (BUG-L4-002).
    ///
    /// BUG-L4-003: Only increments `consecutive_failures` in Closed/`HalfOpen`
    /// states (capped at 100). Open-state failures count toward lifetime total
    /// but do not inflate diagnostics.
    ///
    /// BUG-L4-006: Uses `last_known_tick.max(1)` to avoid `opened_at_tick=0`
    /// which would cause premature `HalfOpen` transition on first `tick_check`.
    pub fn record_failure(&mut self) {
        self.total_failures = self.total_failures.saturating_add(1);

        // BUG-L4-006: floor at 1 so opened_at_tick is never 0
        let tick = self.last_known_tick.max(1);
        match self.state {
            BreakerState::Closed => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1).min(100);
                if self.consecutive_failures >= self.config.failure_threshold {
                    self.transition_to_open(tick);
                }
            }
            BreakerState::Open => {
                // Already open — do NOT increment consecutive_failures (BUG-L4-003)
            }
            BreakerState::HalfOpen => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1).min(100);
                // Probe failed — back to Open
                self.transition_to_open(tick);
            }
        }
    }

    /// Record a failed operation with the current tick for timeout tracking.
    ///
    /// BUG-L4-003: Only increments `consecutive_failures` in Closed/`HalfOpen`
    /// states (capped at 100).
    pub fn record_failure_at(&mut self, tick: u64) {
        self.last_known_tick = tick;
        self.total_failures = self.total_failures.saturating_add(1);

        match self.state {
            BreakerState::Closed => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1).min(100);
                if self.consecutive_failures >= self.config.failure_threshold {
                    self.transition_to_open(tick);
                }
            }
            BreakerState::Open => {
                // Do NOT increment consecutive_failures (BUG-L4-003)
            }
            BreakerState::HalfOpen => {
                self.consecutive_failures = self.consecutive_failures.saturating_add(1).min(100);
                self.transition_to_open(tick);
            }
        }
    }

    /// Check timeouts and advance state machine at a given tick.
    ///
    /// Call this once per tick. If the breaker is `Open` and the timeout
    /// has elapsed, it transitions to `HalfOpen`.
    pub fn tick_check(&mut self, current_tick: u64) {
        self.last_known_tick = current_tick;
        if self.state == BreakerState::Open
            && current_tick.saturating_sub(self.opened_at_tick) >= self.config.open_timeout_ticks
        {
            self.transition_to_half_open();
        }
    }

    /// Record a `HalfOpen` probe request (increment the request counter).
    pub fn record_half_open_request(&mut self) {
        if self.state == BreakerState::HalfOpen {
            self.half_open_requests = self.half_open_requests.saturating_add(1);
        }
    }

    /// Force-reset to `Closed` state (manual intervention).
    pub fn force_close(&mut self) {
        self.transition_to_closed();
    }

    /// Force-open the circuit (manual intervention).
    pub fn force_open(&mut self, tick: u64) {
        self.transition_to_open(tick);
    }

    /// Summary for API/dashboard display.
    #[must_use]
    pub fn summary(&self) -> BreakerSummary {
        BreakerSummary {
            state: self.state,
            consecutive_failures: self.consecutive_failures,
            total_failures: self.total_failures,
            total_successes: self.total_successes,
            allows_request: self.allows_request(),
        }
    }

    fn transition_to_open(&mut self, tick: u64) {
        self.state = BreakerState::Open;
        self.opened_at_tick = tick;
        self.half_open_successes = 0;
        self.half_open_requests = 0;
    }

    fn transition_to_half_open(&mut self) {
        self.state = BreakerState::HalfOpen;
        self.half_open_successes = 0;
        self.half_open_requests = 0;
    }

    fn transition_to_closed(&mut self) {
        self.state = BreakerState::Closed;
        self.consecutive_failures = 0;
        self.half_open_successes = 0;
        self.half_open_requests = 0;
    }
}

impl Default for PaneBreaker {
    fn default() -> Self {
        Self::new(BreakerConfig::default())
    }
}

// ──────────────────────────────────────────────────────────────
// Breaker summary (for API responses)
// ──────────────────────────────────────────────────────────────

/// Summary of a breaker's current state for API/dashboard display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakerSummary {
    /// Current state.
    pub state: BreakerState,
    /// Consecutive failure count.
    pub consecutive_failures: u32,
    /// Total lifetime failures.
    pub total_failures: u64,
    /// Total lifetime successes.
    pub total_successes: u64,
    /// Whether requests are currently allowed.
    pub allows_request: bool,
}

// ──────────────────────────────────────────────────────────────
// Fleet breaker registry
// ──────────────────────────────────────────────────────────────

/// Registry of per-pane circuit breakers.
///
/// Manages breaker instances for all known panes. Unknown panes get
/// a default breaker on first access.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BreakerRegistry {
    /// Per-pane breakers.
    breakers: HashMap<PaneId, PaneBreaker>,
    /// Default configuration for new breakers.
    default_config: BreakerConfig,
}

impl BreakerRegistry {
    /// Create a new registry with the given default config.
    #[must_use]
    pub fn new(default_config: BreakerConfig) -> Self {
        Self {
            breakers: HashMap::new(),
            default_config,
        }
    }

    /// Get or create a breaker for a pane.
    pub fn get_or_create(&mut self, pane: &PaneId) -> &mut PaneBreaker {
        let config = self.default_config.clone();
        self.breakers
            .entry(pane.clone())
            .or_insert_with(|| PaneBreaker::new(config))
    }

    /// Get a breaker for a pane (read-only).
    #[must_use]
    pub fn get(&self, pane: &PaneId) -> Option<&PaneBreaker> {
        self.breakers.get(pane)
    }

    /// Register a pane with a custom config.
    pub fn register(&mut self, pane: PaneId, config: BreakerConfig) {
        self.breakers.insert(pane, PaneBreaker::new(config));
    }

    /// Remove a pane's breaker (deregistration).
    pub fn deregister(&mut self, pane: &PaneId) -> Option<PaneBreaker> {
        self.breakers.remove(pane)
    }

    /// Check whether a pane allows requests.
    ///
    /// Returns `true` for unknown panes (optimistic default).
    #[must_use]
    pub fn allows_request(&self, pane: &PaneId) -> bool {
        self.breakers
            .get(pane)
            .map_or(true, PaneBreaker::allows_request)
    }

    /// Record success for a pane.
    pub fn record_success(&mut self, pane: &PaneId) {
        self.get_or_create(pane).record_success();
    }

    /// Record failure for a pane at a given tick.
    pub fn record_failure(&mut self, pane: &PaneId, tick: u64) {
        self.get_or_create(pane).record_failure_at(tick);
    }

    /// Advance all breakers by one tick.
    pub fn tick_all(&mut self, current_tick: u64) {
        for breaker in self.breakers.values_mut() {
            breaker.tick_check(current_tick);
        }
    }

    /// Number of registered panes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.breakers.len()
    }

    /// Whether the registry is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.breakers.is_empty()
    }

    /// Count panes in each state.
    #[must_use]
    pub fn state_counts(&self) -> (usize, usize, usize) {
        let mut closed = 0;
        let mut open = 0;
        let mut half_open = 0;
        for b in self.breakers.values() {
            match b.state() {
                BreakerState::Closed => closed += 1,
                BreakerState::Open => open += 1,
                BreakerState::HalfOpen => half_open += 1,
            }
        }
        (closed, open, half_open)
    }

    /// Summaries for all panes (for API display).
    #[must_use]
    pub fn all_summaries(&self) -> HashMap<PaneId, BreakerSummary> {
        self.breakers
            .iter()
            .map(|(id, b)| (id.clone(), b.summary()))
            .collect()
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn config_fast() -> BreakerConfig {
        BreakerConfig {
            failure_threshold: 3,
            success_threshold: 1,
            open_timeout_ticks: 5,
            half_open_max_requests: 1,
        }
    }

    // ── BreakerState ──

    #[test]
    fn state_default_is_closed() {
        assert_eq!(BreakerState::default(), BreakerState::Closed);
    }

    #[test]
    fn state_display() {
        assert_eq!(format!("{}", BreakerState::Closed), "Closed");
        assert_eq!(format!("{}", BreakerState::Open), "Open");
        assert_eq!(format!("{}", BreakerState::HalfOpen), "HalfOpen");
    }

    // ── BreakerConfig ──

    #[test]
    fn config_default_reasonable() {
        let c = BreakerConfig::default();
        assert!(c.failure_threshold > 0);
        assert!(c.success_threshold > 0);
        assert!(c.open_timeout_ticks > 0);
        assert!(c.half_open_max_requests > 0);
    }

    #[test]
    fn config_aggressive_faster_than_default() {
        let a = BreakerConfig::aggressive();
        let d = BreakerConfig::default();
        assert!(a.failure_threshold <= d.failure_threshold);
        assert!(a.open_timeout_ticks <= d.open_timeout_ticks);
    }

    #[test]
    fn config_tolerant_slower_than_default() {
        let t = BreakerConfig::tolerant();
        let d = BreakerConfig::default();
        assert!(t.failure_threshold >= d.failure_threshold);
        assert!(t.open_timeout_ticks >= d.open_timeout_ticks);
    }

    // ── PaneBreaker lifecycle ──

    #[test]
    fn breaker_starts_closed() {
        let b = PaneBreaker::default();
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allows_request());
    }

    #[test]
    fn breaker_opens_after_threshold_failures() {
        let mut b = PaneBreaker::new(config_fast());
        b.record_failure_at(0);
        b.record_failure_at(1);
        assert_eq!(b.state(), BreakerState::Closed);
        b.record_failure_at(2);
        assert_eq!(b.state(), BreakerState::Open);
        assert!(!b.allows_request());
    }

    #[test]
    fn breaker_success_resets_failures() {
        let mut b = PaneBreaker::new(config_fast());
        b.record_failure_at(0);
        b.record_failure_at(1);
        b.record_success();
        assert_eq!(b.consecutive_failures(), 0);
        assert_eq!(b.state(), BreakerState::Closed);
    }

    #[test]
    fn breaker_open_to_halfopen_after_timeout() {
        let mut b = PaneBreaker::new(config_fast());
        // Trip the breaker
        for i in 0..3 {
            b.record_failure_at(i);
        }
        assert_eq!(b.state(), BreakerState::Open);

        // Tick forward past timeout (5 ticks)
        b.tick_check(3); // 3 - 2 = 1 tick, not enough
        assert_eq!(b.state(), BreakerState::Open);

        b.tick_check(8); // 8 - 2 = 6 ticks >= 5, transition
        assert_eq!(b.state(), BreakerState::HalfOpen);
    }

    #[test]
    fn breaker_halfopen_allows_limited_requests() {
        let mut b = PaneBreaker::new(config_fast());
        for i in 0..3 {
            b.record_failure_at(i);
        }
        b.tick_check(100); // force to HalfOpen
        assert!(b.allows_request());
        b.record_half_open_request();
        assert!(!b.allows_request()); // max 1 in HalfOpen
    }

    #[test]
    fn breaker_halfopen_success_closes() {
        let mut b = PaneBreaker::new(config_fast());
        for i in 0..3 {
            b.record_failure_at(i);
        }
        b.tick_check(100);
        assert_eq!(b.state(), BreakerState::HalfOpen);
        b.record_success(); // success_threshold = 1
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allows_request());
    }

    #[test]
    fn breaker_halfopen_failure_reopens() {
        let mut b = PaneBreaker::new(config_fast());
        for i in 0..3 {
            b.record_failure_at(i);
        }
        b.tick_check(100);
        assert_eq!(b.state(), BreakerState::HalfOpen);
        b.record_failure_at(101);
        assert_eq!(b.state(), BreakerState::Open);
    }

    #[test]
    fn breaker_full_cycle() {
        let mut b = PaneBreaker::new(config_fast());

        // Closed → Open
        for i in 0..3 {
            b.record_failure_at(i);
        }
        assert_eq!(b.state(), BreakerState::Open);

        // Open → HalfOpen
        b.tick_check(100);
        assert_eq!(b.state(), BreakerState::HalfOpen);

        // HalfOpen → Closed
        b.record_success();
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allows_request());
    }

    // ── Counters ──

    #[test]
    fn breaker_total_counters_accumulate() {
        let mut b = PaneBreaker::new(config_fast());
        b.record_success();
        b.record_success();
        b.record_failure_at(0);
        assert_eq!(b.total_successes(), 2);
        assert_eq!(b.total_failures(), 1);
    }

    #[test]
    fn breaker_total_failures_survive_reset() {
        let mut b = PaneBreaker::new(config_fast());
        b.record_failure_at(0);
        b.record_failure_at(1);
        b.record_success(); // resets consecutive but not total
        assert_eq!(b.total_failures(), 2);
        assert_eq!(b.consecutive_failures(), 0);
    }

    // ── Force controls ──

    #[test]
    fn breaker_force_close() {
        let mut b = PaneBreaker::new(config_fast());
        for i in 0..3 {
            b.record_failure_at(i);
        }
        assert_eq!(b.state(), BreakerState::Open);
        b.force_close();
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allows_request());
    }

    #[test]
    fn breaker_force_open() {
        let mut b = PaneBreaker::new(config_fast());
        assert_eq!(b.state(), BreakerState::Closed);
        b.force_open(10);
        assert_eq!(b.state(), BreakerState::Open);
        assert!(!b.allows_request());
    }

    // ── Summary ──

    #[test]
    fn breaker_summary_reflects_state() {
        let mut b = PaneBreaker::new(config_fast());
        b.record_failure_at(0);
        let s = b.summary();
        assert_eq!(s.state, BreakerState::Closed);
        assert_eq!(s.consecutive_failures, 1);
        assert_eq!(s.total_failures, 1);
        assert!(s.allows_request);
    }

    // ── record_failure (no tick) ──

    #[test]
    fn record_failure_without_tick() {
        let mut b = PaneBreaker::new(config_fast());
        for _ in 0..3 {
            b.record_failure();
        }
        assert_eq!(b.state(), BreakerState::Open);
    }

    // ── BreakerRegistry ──

    #[test]
    fn registry_empty_by_default() {
        let r = BreakerRegistry::new(BreakerConfig::default());
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn registry_get_or_create_inserts() {
        let mut r = BreakerRegistry::new(BreakerConfig::default());
        r.get_or_create(&pid("a"));
        assert_eq!(r.len(), 1);
        assert!(!r.is_empty());
    }

    #[test]
    fn registry_get_or_create_idempotent() {
        let mut r = BreakerRegistry::new(BreakerConfig::default());
        r.get_or_create(&pid("a"));
        r.get_or_create(&pid("a"));
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn registry_register_custom_config() {
        let mut r = BreakerRegistry::new(BreakerConfig::default());
        r.register(pid("a"), BreakerConfig::aggressive());
        let b = r.get(&pid("a"));
        assert!(b.is_some());
    }

    #[test]
    fn registry_deregister() {
        let mut r = BreakerRegistry::new(BreakerConfig::default());
        r.get_or_create(&pid("a"));
        let removed = r.deregister(&pid("a"));
        assert!(removed.is_some());
        assert!(r.is_empty());
    }

    #[test]
    fn registry_deregister_unknown() {
        let mut r = BreakerRegistry::new(BreakerConfig::default());
        assert!(r.deregister(&pid("x")).is_none());
    }

    #[test]
    fn registry_allows_request_unknown_pane() {
        let r = BreakerRegistry::new(BreakerConfig::default());
        assert!(r.allows_request(&pid("unknown")));
    }

    #[test]
    fn registry_allows_request_open_breaker() {
        let mut r = BreakerRegistry::new(config_fast());
        for i in 0..3 {
            r.record_failure(&pid("a"), i);
        }
        assert!(!r.allows_request(&pid("a")));
    }

    #[test]
    fn registry_record_success() {
        let mut r = BreakerRegistry::new(config_fast());
        r.record_failure(&pid("a"), 0);
        r.record_success(&pid("a"));
        let b = r.get(&pid("a"));
        assert_eq!(b.map(PaneBreaker::consecutive_failures), Some(0));
    }

    #[test]
    fn registry_tick_all() {
        let mut r = BreakerRegistry::new(config_fast());
        for i in 0..3 {
            r.record_failure(&pid("a"), i);
        }
        assert_eq!(r.get(&pid("a")).map(PaneBreaker::state), Some(BreakerState::Open));
        r.tick_all(100);
        assert_eq!(r.get(&pid("a")).map(PaneBreaker::state), Some(BreakerState::HalfOpen));
    }

    #[test]
    fn registry_state_counts() {
        let mut r = BreakerRegistry::new(config_fast());
        r.get_or_create(&pid("a")); // Closed
        for i in 0..3 {
            r.record_failure(&pid("b"), i); // Open
        }
        r.record_failure(&pid("c"), 0);
        r.record_failure(&pid("c"), 1);
        r.record_failure(&pid("c"), 2);
        r.tick_all(100); // b and c → HalfOpen
        let (closed, _open, half_open) = r.state_counts();
        assert_eq!(closed, 1);
        assert_eq!(half_open, 2);
    }

    #[test]
    fn registry_all_summaries() {
        let mut r = BreakerRegistry::new(BreakerConfig::default());
        r.get_or_create(&pid("a"));
        r.get_or_create(&pid("b"));
        let summaries = r.all_summaries();
        assert_eq!(summaries.len(), 2);
    }

    // ── Edge cases ──

    #[test]
    fn breaker_no_panic_on_overflow() {
        let mut b = PaneBreaker::new(config_fast());
        b.total_failures = u64::MAX;
        b.record_failure_at(0); // should saturate, not panic
        assert_eq!(b.total_failures(), u64::MAX);
    }

    #[test]
    fn breaker_tick_check_no_op_when_closed() {
        let mut b = PaneBreaker::new(config_fast());
        b.tick_check(1000);
        assert_eq!(b.state(), BreakerState::Closed);
    }

    #[test]
    fn breaker_tick_check_no_op_when_halfopen() {
        let mut b = PaneBreaker::new(config_fast());
        for i in 0..3 {
            b.record_failure_at(i);
        }
        b.tick_check(100); // Open → HalfOpen
        b.tick_check(200); // Already HalfOpen, no-op
        assert_eq!(b.state(), BreakerState::HalfOpen);
    }

    #[test]
    fn breaker_open_success_transitions_to_closed() {
        let mut b = PaneBreaker::new(config_fast());
        for i in 0..3 {
            b.record_failure_at(i);
        }
        assert_eq!(b.state(), BreakerState::Open);
        // Unexpected success while Open (shouldn't happen normally)
        b.record_success();
        assert_eq!(b.state(), BreakerState::Closed);
    }

    #[test]
    fn registry_multiple_panes_independent() {
        let mut r = BreakerRegistry::new(config_fast());
        // Trip pane a
        for i in 0..3 {
            r.record_failure(&pid("a"), i);
        }
        // Pane b is fine
        r.record_success(&pid("b"));

        assert!(!r.allows_request(&pid("a")));
        assert!(r.allows_request(&pid("b")));
    }

    #[test]
    fn breaker_success_threshold_two() {
        let config = BreakerConfig {
            failure_threshold: 3,
            success_threshold: 2,
            open_timeout_ticks: 5,
            half_open_max_requests: 3,
        };
        let mut b = PaneBreaker::new(config);
        for i in 0..3 {
            b.record_failure_at(i);
        }
        b.tick_check(100);
        assert_eq!(b.state(), BreakerState::HalfOpen);
        b.record_success();
        assert_eq!(b.state(), BreakerState::HalfOpen); // needs 2
        b.record_success();
        assert_eq!(b.state(), BreakerState::Closed); // now closed
    }

    #[test]
    fn breaker_default_impl() {
        let b = PaneBreaker::default();
        assert_eq!(b.state(), BreakerState::Closed);
        assert!(b.allows_request());
        assert_eq!(b.total_failures(), 0);
        assert_eq!(b.total_successes(), 0);
    }

    // ── Bridge isolation (ORAC topology) ──

    #[test]
    fn orac_bridge_isolation_trip_pv2_others_unaffected() {
        let mut r = BreakerRegistry::new(config_fast());
        // Register all 5 ORAC bridges
        for svc in &["pv2", "synthex", "me", "povm", "rm"] {
            r.get_or_create(&pid(svc));
        }
        assert_eq!(r.len(), 5);

        // Trip pv2 breaker
        for tick in 0..3 {
            r.record_failure(&pid("pv2"), tick);
        }
        assert!(!r.allows_request(&pid("pv2")));

        // All others must remain closed
        assert!(r.allows_request(&pid("synthex")));
        assert!(r.allows_request(&pid("me")));
        assert!(r.allows_request(&pid("povm")));
        assert!(r.allows_request(&pid("rm")));

        // State counts: 4 closed, 1 open, 0 half-open
        let (closed, open, half_open) = r.state_counts();
        assert_eq!(closed, 4);
        assert_eq!(open, 1);
        assert_eq!(half_open, 0);
    }

    #[test]
    fn orac_bridge_isolation_trip_two_others_unaffected() {
        let mut r = BreakerRegistry::new(config_fast());
        for svc in &["pv2", "synthex", "me", "povm", "rm"] {
            r.get_or_create(&pid(svc));
        }

        // Trip synthex and povm
        for tick in 0..3 {
            r.record_failure(&pid("synthex"), tick);
            r.record_failure(&pid("povm"), tick);
        }
        assert!(!r.allows_request(&pid("synthex")));
        assert!(!r.allows_request(&pid("povm")));

        // pv2, me, rm untouched
        assert!(r.allows_request(&pid("pv2")));
        assert!(r.allows_request(&pid("me")));
        assert!(r.allows_request(&pid("rm")));

        let (closed, open, _) = r.state_counts();
        assert_eq!(closed, 3);
        assert_eq!(open, 2);
    }

    #[test]
    fn orac_bridge_isolation_recovery_independent() {
        let mut r = BreakerRegistry::new(config_fast());
        for svc in &["pv2", "synthex", "me", "povm", "rm"] {
            r.get_or_create(&pid(svc));
        }

        // Trip all 5
        for tick in 0..3 {
            for svc in &["pv2", "synthex", "me", "povm", "rm"] {
                r.record_failure(&pid(svc), tick);
            }
        }
        let (_, open, _) = r.state_counts();
        assert_eq!(open, 5);

        // Advance ticks to transition all to HalfOpen
        r.tick_all(100);
        let (_, _, half_open) = r.state_counts();
        assert_eq!(half_open, 5);

        // Recover only pv2 and rm
        r.record_success(&pid("pv2"));
        r.record_success(&pid("rm"));

        // pv2 and rm closed, others still half-open
        assert!(r.allows_request(&pid("pv2")));
        assert!(r.allows_request(&pid("rm")));
        let (closed, _, half_open) = r.state_counts();
        assert_eq!(closed, 2);
        assert_eq!(half_open, 3);
    }

    #[test]
    fn orac_bridge_isolation_successes_dont_cross() {
        let mut r = BreakerRegistry::new(config_fast());
        for svc in &["pv2", "synthex"] {
            r.get_or_create(&pid(svc));
        }

        // Accumulate 2 failures on pv2 (below threshold of 3)
        r.record_failure(&pid("pv2"), 0);
        r.record_failure(&pid("pv2"), 1);
        assert_eq!(
            r.get(&pid("pv2")).map(PaneBreaker::consecutive_failures),
            Some(2)
        );

        // Success on synthex does NOT reset pv2 failures
        r.record_success(&pid("synthex"));
        assert_eq!(
            r.get(&pid("pv2")).map(PaneBreaker::consecutive_failures),
            Some(2)
        );
        assert_eq!(
            r.get(&pid("synthex")).map(PaneBreaker::consecutive_failures),
            Some(0)
        );
    }
}
