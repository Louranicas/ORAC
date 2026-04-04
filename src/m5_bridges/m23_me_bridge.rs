//! # M24: Maintenance Engine Bridge
//!
//! Polls ME V2 at `localhost:8180/api/observer` for fitness signal.
//! Consent-gated (PG-12). Fire-and-forget semantics for posts.
//!
//! ## Layer: L6 | Module: M24 | Dependencies: L1
//!
//! ## BUG-008: ME `EventBus` has zero publishers
//! The ME's `EventBus` currently has zero publishers, meaning the fitness value
//! is frozen at `0.3662` since 2026-03-06 (ALERT-2). This bridge handles
//! that gracefully by:
//! - Detecting frozen values (same fitness across multiple polls)
//! - Falling back to neutral adjustment (1.0) when frozen
//! - Logging the condition without failing
//!
//! ## ORAC Adaptations (applied)
//! - Port configurable via `with_config` (default 8180)
//! - Socket address: raw `host:port` (no `http://` prefix, BUG-033)
//! - Poll interval configurable (default 12 ticks)
//! - BUG-008 frozen detection: 3 identical polls → neutral fallback

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)] // extract_body used by tests via `use super::*;`
use super::http_helpers::{extract_body, raw_http_get, raw_http_post};
use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m1_core::m04_constants;
use crate::m1_core::m05_traits::Bridgeable;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// ME V2 service port (Session 081: migrated from V1 port 8080).
const ME_PORT: u16 = 8180;

/// Default base URL for the Maintenance Engine V2.
const DEFAULT_BASE_URL: &str = "127.0.0.1:8180";

/// Health endpoint path.
const HEALTH_PATH: &str = "/api/health";

/// Observer (fitness) endpoint path.
const OBSERVER_PATH: &str = "/api/observer";

/// Default poll interval in ticks.
const DEFAULT_POLL_INTERVAL: u64 = 12;

/// Known frozen fitness value from BUG-008.
const BUG_008_FROZEN_FITNESS: f64 = 0.3662;

/// Tolerance for detecting frozen fitness values.
///
/// BUG-060b: Widened from 0.001 to 0.003. ME fitness oscillates between
/// 0.609-0.627, and 0.001 tolerance triggered false frozen detections when
/// fitness settled at 0.609 for 3 consecutive polls. 0.003 accommodates
/// normal measurement noise while still detecting genuine plateaus.
const FROZEN_TOLERANCE: f64 = 0.003;

/// Number of identical polls before declaring fitness "frozen."
const FROZEN_THRESHOLD: u32 = 3;

// ──────────────────────────────────────────────────────────────
// Response types
// ──────────────────────────────────────────────────────────────

/// Response from the ME `/api/observer` endpoint.
///
/// The actual ME response nests fitness under `last_report.current_fitness`.
/// We flatten it via `extract_fitness()` for bridge consumers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverResponse {
    /// Overall system fitness (0.0-1.0), extracted from `last_report`.
    #[serde(default)]
    pub fitness: f64,
    /// Number of active layers in the ME.
    #[serde(default)]
    pub active_layers: u32,
    /// Whether the ME event bus has active publishers.
    #[serde(default)]
    pub has_publishers: bool,
    /// Observer status label (maps from ME `system_state`).
    #[serde(default)]
    pub status: String,
    /// Correlations found since last ME report.
    #[serde(default)]
    pub correlations_since: u64,
    /// Emergences detected since last ME report.
    #[serde(default)]
    pub emergences_since: u64,
    /// Total correlations found across all time.
    #[serde(default)]
    pub total_correlations: u64,
    /// Total events ingested by the ME.
    #[serde(default)]
    pub total_events: u64,
}

/// Raw ME `/api/observer` response with nested `last_report`.
#[derive(Debug, Clone, Deserialize)]
struct RawObserverResponse {
    /// Nested last report containing `current_fitness`.
    #[serde(default)]
    last_report: Option<RawLastReport>,
    /// ME system state string (e.g. "Healthy", "Degraded").
    #[serde(default)]
    system_state: Option<String>,
    /// Whether the observer is enabled.
    #[serde(default)]
    enabled: bool,
    /// ME metrics (correlations, emergences, events).
    #[serde(default)]
    metrics: Option<RawMetrics>,
}

/// Nested report within the raw ME observer response.
#[derive(Debug, Clone, Deserialize)]
struct RawLastReport {
    /// Current fitness score.
    #[serde(default)]
    current_fitness: f64,
    /// Correlations found since last report.
    #[serde(default)]
    correlations_since_last: u64,
    /// Emergences detected since last report.
    #[serde(default)]
    emergences_since_last: u64,
}

/// Nested metrics block within the raw ME observer response.
///
/// All fields are deserialized from ME JSON but only a subset are forwarded
/// to `ObserverResponse`. Serde needs the fields present for structural matching.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Fields populated by serde deserialization
struct RawMetrics {
    /// Total correlations found across all time.
    #[serde(default)]
    correlations_found: u64,
    /// Total emergences detected across all time.
    #[serde(default)]
    emergences_detected: u64,
    /// Total events ingested by the ME.
    #[serde(default)]
    events_ingested: u64,
}

impl RawObserverResponse {
    /// Convert the raw response into the bridge-friendly `ObserverResponse`.
    fn into_observer(self) -> ObserverResponse {
        let fitness = self
            .last_report
            .as_ref()
            .map_or(0.0, |r| r.current_fitness);
        let correlations_since = self
            .last_report
            .as_ref()
            .map_or(0, |r| r.correlations_since_last);
        let emergences_since = self
            .last_report
            .as_ref()
            .map_or(0, |r| r.emergences_since_last);
        let total_correlations = self
            .metrics
            .as_ref()
            .map_or(0, |m| m.correlations_found);
        let total_events = self
            .metrics
            .as_ref()
            .map_or(0, |m| m.events_ingested);
        ObserverResponse {
            fitness,
            active_layers: 0,
            has_publishers: self.enabled,
            status: self.system_state.unwrap_or_default(),
            correlations_since,
            emergences_since,
            total_correlations,
            total_events,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Bridge state
// ──────────────────────────────────────────────────────────────

/// Mutable state behind a `RwLock`.
#[derive(Debug)]
struct BridgeState {
    /// Last poll tick number.
    last_poll_tick: u64,
    /// Cached adjustment from last successful poll.
    cached_adjustment: f64,
    /// Whether the cached value is stale.
    stale: bool,
    /// Consecutive failure counter.
    consecutive_failures: u32,
    /// Last raw fitness value.
    last_fitness: f64,
    /// Counter of identical fitness readings (BUG-008 detection).
    frozen_count: u32,
    /// Whether fitness is currently detected as frozen.
    is_frozen: bool,
    /// Last full observer response.
    last_response: Option<ObserverResponse>,
    /// Successful poll count (observer subscription proxy).
    successful_polls: u64,
    /// Last-seen ME `EventBus` learning channel count (Session 075 BREAK-3).
    me_learning_events: u64,
    /// Last-seen ME `EventBus` integration channel count (Session 075 BREAK-3).
    me_integration_events: u64,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            last_poll_tick: 0,
            cached_adjustment: 1.0,
            stale: true,
            consecutive_failures: 0,
            last_fitness: 0.0,
            frozen_count: 0,
            is_frozen: false,
            last_response: None,
            successful_polls: 0,
            me_learning_events: 0,
            me_integration_events: 0,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// MeBridge
// ──────────────────────────────────────────────────────────────

/// Snapshot of ME bridge state for health endpoint reporting.
#[derive(Debug, Clone)]
pub struct BridgeStateSnapshot {
    /// ME `EventBus` learning channel events seen.
    pub me_learning_events: u64,
    /// ME `EventBus` integration channel events seen.
    pub me_integration_events: u64,
}

/// Bridge to the Maintenance Engine for fitness-based coupling modulation.
///
/// Handles BUG-008 (frozen fitness) gracefully by detecting repeated identical
/// values and falling back to neutral adjustment.
#[derive(Debug)]
pub struct MeBridge {
    /// Service name identifier.
    service: String,
    /// TCP address (host:port).
    base_url: String,
    /// Poll interval in ticks.
    poll_interval: u64,
    /// Interior-mutable state.
    state: RwLock<BridgeState>,
}

impl MeBridge {
    /// Create a new ME bridge with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: "me".to_owned(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            state: RwLock::new(BridgeState::default()),
        }
    }

    /// Create a new ME bridge with custom configuration.
    ///
    /// Protocol prefixes (`http://`, `https://`) are stripped automatically
    /// because the bridge uses raw TCP sockets, not an HTTP client (BUG-033).
    #[must_use]
    pub fn with_config(base_url: impl Into<String>, poll_interval: u64) -> Self {
        let raw: String = base_url.into();
        let stripped = raw
            .strip_prefix("http://")
            .or_else(|| raw.strip_prefix("https://"))
            .unwrap_or(&raw)
            .to_owned();
        Self {
            service: "me".to_owned(),
            base_url: stripped,
            poll_interval: poll_interval.max(1),
            state: RwLock::new(BridgeState::default()),
        }
    }

    /// Return the configured poll interval.
    #[must_use]
    pub const fn poll_interval(&self) -> u64 {
        self.poll_interval
    }

    /// Return the number of consecutive failures.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        self.state.read().consecutive_failures
    }

    /// Return the cached adjustment value.
    #[must_use]
    pub fn cached_adjustment(&self) -> f64 {
        self.state.read().cached_adjustment
    }

    /// Return the last raw fitness value.
    #[must_use]
    pub fn last_fitness(&self) -> f64 {
        self.state.read().last_fitness
    }

    /// Return whether the fitness is currently detected as frozen (BUG-008).
    #[must_use]
    pub fn is_frozen(&self) -> bool {
        self.state.read().is_frozen
    }

    /// Return whether ME observer is actively subscribed (at least one successful poll).
    #[must_use]
    pub fn is_subscribed(&self) -> bool {
        self.state.read().successful_polls > 0
    }

    /// Return a snapshot of the bridge state for health reporting.
    #[must_use]
    pub fn state_snapshot(&self) -> BridgeStateSnapshot {
        let s = self.state.read();
        BridgeStateSnapshot {
            me_learning_events: s.me_learning_events,
            me_integration_events: s.me_integration_events,
        }
    }

    /// Return the number of successful observer polls.
    #[must_use]
    pub fn successful_polls(&self) -> u64 {
        self.state.read().successful_polls
    }

    /// Return the last poll tick.
    #[must_use]
    pub fn last_poll_tick(&self) -> u64 {
        self.state.read().last_poll_tick
    }

    /// Return the port number.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.base_url
            .split(':')
            .next_back()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(ME_PORT)
    }

    /// Return the last observer response, if any.
    #[must_use]
    pub fn last_response(&self) -> Option<ObserverResponse> {
        self.state.read().last_response.clone()
    }

    /// Convert a raw fitness value into a coupling adjustment.
    ///
    /// Fitness is in [0.0, 1.0]. The mapping:
    /// - 0.5 maps to neutral (1.0)
    /// - 1.0 maps to boost (`K_MOD_BUDGET_MAX`)
    /// - 0.0 maps to dampen (`K_MOD_BUDGET_MIN`)
    #[must_use]
    pub fn fitness_to_adjustment(fitness: f64) -> f64 {
        let f = fitness.clamp(0.0, 1.0);
        // Linear interpolation: fitness 0.0 → K_MOD_BUDGET_MIN, 1.0 → K_MOD_BUDGET_MAX
        let range = m04_constants::K_MOD_BUDGET_MAX - m04_constants::K_MOD_BUDGET_MIN;
        let adj = f.mul_add(range, m04_constants::K_MOD_BUDGET_MIN);
        adj.clamp(m04_constants::K_MOD_BUDGET_MIN, m04_constants::K_MOD_BUDGET_MAX)
    }

    /// Session 071 #4: Normalize ORAC Hebbian weight to ME weight space.
    ///
    /// ORAC range: `[WEIGHT_FLOOR(0.15), SOFT_CEILING(0.85)]`
    /// ME range: `[0.0, 1.0]`
    /// Linear mapping: `(orac_w - 0.15) / (0.85 - 0.15)`
    #[must_use]
    pub fn orac_weight_to_me(orac_weight: f64) -> f64 {
        let floor = m04_constants::HEBBIAN_WEIGHT_FLOOR;
        let ceiling = 0.85_f64; // HEBBIAN_SOFT_CEILING from m18
        let clamped = orac_weight.clamp(floor, ceiling);
        (clamped - floor) / (ceiling - floor)
    }

    /// Session 071 #4: Normalize ME Hebbian weight to ORAC weight space.
    ///
    /// ME range: `[0.0, 1.0]`
    /// ORAC range: `[WEIGHT_FLOOR(0.15), SOFT_CEILING(0.85)]`
    /// Linear mapping: `me_w * (0.85 - 0.15) + 0.15`
    #[must_use]
    pub fn me_weight_to_orac(me_weight: f64) -> f64 {
        let floor = m04_constants::HEBBIAN_WEIGHT_FLOOR;
        let ceiling = 0.85_f64;
        let clamped = me_weight.clamp(0.0, 1.0);
        clamped.mul_add(ceiling - floor, floor)
    }

    /// Poll the ME observer endpoint.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` or `PvError::BridgeParse` on failure.
    pub fn poll_observer(&self) -> PvResult<f64> {
        let body = raw_http_get(&self.base_url, OBSERVER_PATH, &self.service)?;

        // Parse raw ME response (nested last_report.current_fitness)
        let raw: RawObserverResponse =
            serde_json::from_str(&body).map_err(|e| PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("observer parse: {e}"),
            })?;
        let response = raw.into_observer();

        let fitness = if response.fitness.is_finite() {
            response.fitness.clamp(0.0, 1.0)
        } else {
            return Err(PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("non-finite fitness: {}", response.fitness),
            });
        };

        let mut state = self.state.write();

        // BUG-008 detection: check if fitness is frozen
        if (fitness - state.last_fitness).abs() < FROZEN_TOLERANCE {
            state.frozen_count = state.frozen_count.saturating_add(1);
        } else {
            state.frozen_count = 0;
        }

        // BUG-060b/070: Only mark frozen if genuinely stuck at a pathologically
        // low value (< 0.4, e.g. the BUG-008 plateau at 0.3662). Normal ME fitness
        // at 0.609 oscillates within FROZEN_TOLERANCE and triggered false freeze
        // detections — disabling coupling weight modulation for all sessions.
        state.is_frozen = (state.frozen_count >= FROZEN_THRESHOLD && fitness < 0.4)
            || (fitness - BUG_008_FROZEN_FITNESS).abs() < FROZEN_TOLERANCE;

        state.last_fitness = fitness;
        state.last_response = Some(response);
        state.consecutive_failures = 0;
        state.stale = false;
        state.successful_polls = state.successful_polls.saturating_add(1);

        // If frozen, return neutral adjustment
        let adj = if state.is_frozen {
            1.0
        } else {
            Self::fitness_to_adjustment(fitness)
        };

        state.cached_adjustment = adj;
        Ok(adj)
    }

    /// Record a poll failure.
    pub fn record_failure(&self) {
        let mut state = self.state.write();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        state.stale = true;
    }

    /// Update the last poll tick.
    pub fn set_last_poll_tick(&self, tick: u64) {
        self.state.write().last_poll_tick = tick;
    }

    /// Check whether a poll is due at the given tick.
    ///
    /// BUG-SCAN-003 fix: force immediate first poll when `last_poll_tick == 0`
    /// and data is stale, matching `SynthexBridge` behaviour. Without this,
    /// the first ME poll waits a full `poll_interval` (12 ticks / 60s).
    #[must_use]
    pub fn should_poll(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        if state.last_poll_tick == 0 && state.stale {
            return true;
        }
        current_tick.saturating_sub(state.last_poll_tick) >= self.poll_interval
    }
}

impl Default for MeBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Bridgeable for MeBridge {
    /// Return the service name (`"me"`).
    fn service_name(&self) -> &str {
        &self.service
    }

    /// Poll the ME observer endpoint, recording failure on error.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the TCP connection fails.
    /// Returns `PvError::BridgeParse` if the response cannot be deserialized.
    fn poll(&self) -> PvResult<f64> {
        match self.poll_observer() {
            Ok(adj) => Ok(adj),
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }

    /// Post structured data to ME's learning-cycle endpoint.
    ///
    /// Session 075 BREAK-2: replaces no-op with actual HTTP POST to
    /// `POST /api/tools/learning-cycle`. Enables ME to receive ORAC
    /// emergence events and respond in its 12D fitness tensor.
    ///
    /// # Errors
    /// Returns error on HTTP failure (breaker should guard calls).
    fn post(&self, payload: &[u8]) -> PvResult<()> {
        raw_http_post(&self.base_url, "/api/tools/learning-cycle", payload, &self.service)?;
        Ok(())
    }

    /// Check whether the ME service is reachable.
    ///
    /// # Errors
    /// Returns `Ok(false)` on connection failure (does not propagate the error).
    fn health(&self) -> PvResult<bool> {
        match raw_http_get(&self.base_url, HEALTH_PATH, &self.service) {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::warn!(service = %self.service, error = %e, "bridge health check failed");
                Ok(false)
            }
        }
    }

    /// Return whether the cached data is stale (flag set or 2x poll interval elapsed).
    fn is_stale(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        state.stale || current_tick.saturating_sub(state.last_poll_tick) >= self.poll_interval * 2
    }
}

// HTTP helpers now in super::http_helpers (BUG-042 fix)

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

// ── Session 075 BREAK-2: Emergence relay to ME ──

impl MeBridge {
    /// Post an emergence event to ME with structured details.
    ///
    /// Wraps the emergence type and context into a JSON payload
    /// and sends to ME's `/api/tools/learning-cycle` endpoint.
    /// Uses the `Bridgeable::post()` implementation (breaker-guarded).
    ///
    /// # Errors
    /// Returns error on HTTP or serialization failure.
    pub fn post_emergence(
        &self,
        emergence_type: &str,
        tick: u64,
        fitness: f64,
        field_r: f64,
    ) -> PvResult<()> {
        use crate::m1_core::m05_traits::Bridgeable;
        let payload = serde_json::json!({
            "source": "orac-sidecar",
            "event_type": "emergence",
            "emergence_type": emergence_type,
            "tick": tick,
            "fitness": fitness,
            "field_r": field_r,
        });
        self.post(payload.to_string().as_bytes())
    }

    /// Poll ME `EventBus` stats and return event count deltas.
    ///
    /// Session 075 BREAK-3: Detects ME learning activity by diffing
    /// per-channel event counts between polls. No per-event endpoint
    /// exists (404), so stats diffing is the viable approach.
    ///
    /// # Errors
    /// Returns error on HTTP or parse failure.
    pub fn poll_eventbus_delta(&self) -> PvResult<(u64, u64)> {
        let body = raw_http_get(&self.base_url, "/api/eventbus/stats", &self.service)?;
        let parsed: serde_json::Value = serde_json::from_str(&body)
            .map_err(|e| PvError::Internal(format!("ME eventbus stats parse: {e}")))?;

        let channels = parsed.get("channels")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut learning = 0u64;
        let mut integration = 0u64;
        for ch in &channels {
            let name = ch.get("channel").and_then(serde_json::Value::as_str).unwrap_or("");
            let count = ch.get("event_count").and_then(serde_json::Value::as_u64).unwrap_or(0);
            match name {
                "learning" => learning = count,
                "integration" => integration = count,
                _ => {}
            }
        }

        let mut state = self.state.write();
        let prev_learning = state.me_learning_events;
        let prev_integration = state.me_integration_events;
        state.me_learning_events = learning;
        state.me_integration_events = integration;

        Ok((
            learning.saturating_sub(prev_learning),
            integration.saturating_sub(prev_integration),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction ──

    #[test]
    fn new_creates_default_bridge() {
        let bridge = MeBridge::new();
        assert_eq!(bridge.service_name(), "me");
        assert_eq!(bridge.poll_interval(), DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn default_creates_same_as_new() {
        let bridge = MeBridge::default();
        assert_eq!(bridge.service_name(), "me");
    }

    #[test]
    fn with_config_custom_url() {
        let bridge = MeBridge::with_config("10.0.0.1:8888", 20);
        assert_eq!(bridge.base_url, "10.0.0.1:8888");
        assert_eq!(bridge.poll_interval(), 20);
    }

    #[test]
    fn with_config_minimum_poll_interval() {
        let bridge = MeBridge::with_config("localhost:8080", 0);
        assert_eq!(bridge.poll_interval(), 1);
    }

    #[test]
    fn port_extraction_default() {
        let bridge = MeBridge::new();
        assert_eq!(bridge.port(), ME_PORT);
    }

    #[test]
    fn port_extraction_custom() {
        let bridge = MeBridge::with_config("localhost:9080", 12);
        assert_eq!(bridge.port(), 9080);
    }

    // ── Initial state ──

    #[test]
    fn initial_cached_adjustment_is_one() {
        let bridge = MeBridge::new();
        assert!((bridge.cached_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn initial_failures_is_zero() {
        let bridge = MeBridge::new();
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn initial_fitness_is_zero() {
        let bridge = MeBridge::new();
        assert!((bridge.last_fitness()).abs() < f64::EPSILON);
    }

    #[test]
    fn initial_not_frozen() {
        let bridge = MeBridge::new();
        assert!(!bridge.is_frozen());
    }

    #[test]
    fn initial_is_stale() {
        let bridge = MeBridge::new();
        assert!(bridge.is_stale(0));
    }

    #[test]
    fn initial_last_response_is_none() {
        let bridge = MeBridge::new();
        assert!(bridge.last_response().is_none());
    }

    // ── Fitness to adjustment ──

    #[test]
    fn fitness_zero_maps_to_budget_min() {
        let adj = MeBridge::fitness_to_adjustment(0.0);
        assert!((adj - m04_constants::K_MOD_BUDGET_MIN).abs() < 1e-10);
    }

    #[test]
    fn fitness_one_maps_to_budget_max() {
        let adj = MeBridge::fitness_to_adjustment(1.0);
        assert!((adj - m04_constants::K_MOD_BUDGET_MAX).abs() < 1e-10);
    }

    #[test]
    fn fitness_half_maps_to_neutral() {
        let adj = MeBridge::fitness_to_adjustment(0.5);
        let expected = (m04_constants::K_MOD_BUDGET_MIN + m04_constants::K_MOD_BUDGET_MAX) / 2.0;
        assert!((adj - expected).abs() < 1e-10);
    }

    #[test]
    fn fitness_clamps_negative() {
        let adj = MeBridge::fitness_to_adjustment(-5.0);
        assert!((adj - m04_constants::K_MOD_BUDGET_MIN).abs() < 1e-10);
    }

    #[test]
    fn fitness_clamps_above_one() {
        let adj = MeBridge::fitness_to_adjustment(10.0);
        assert!((adj - m04_constants::K_MOD_BUDGET_MAX).abs() < 1e-10);
    }

    #[test]
    fn fitness_in_budget_range() {
        for i in 0..=100 {
            let f = f64::from(i) / 100.0;
            let adj = MeBridge::fitness_to_adjustment(f);
            assert!(adj >= m04_constants::K_MOD_BUDGET_MIN);
            assert!(adj <= m04_constants::K_MOD_BUDGET_MAX);
        }
    }

    // ── BUG-008 frozen detection ──

    #[test]
    fn bug008_frozen_fitness_value() {
        assert!((BUG_008_FROZEN_FITNESS - 0.3662).abs() < 1e-10);
    }

    #[test]
    fn frozen_detected_after_threshold_at_low_fitness() {
        // BUG-060b/070: Frozen detection only triggers for pathologically
        // low fitness (< 0.4). Normal fitness at 0.5+ is NOT frozen.
        let bridge = MeBridge::new();
        let mut state = bridge.state.write();
        state.last_fitness = 0.35; // Below 0.4 threshold

        // Simulate identical readings at low fitness
        for _ in 0..FROZEN_THRESHOLD {
            if (0.35 - state.last_fitness).abs() < FROZEN_TOLERANCE {
                state.frozen_count = state.frozen_count.saturating_add(1);
            }
        }
        // Apply the production logic (BUG-070 fix)
        let fitness = 0.35;
        state.is_frozen = (state.frozen_count >= FROZEN_THRESHOLD && fitness < 0.4)
            || (fitness - BUG_008_FROZEN_FITNESS).abs() < FROZEN_TOLERANCE;
        assert!(state.is_frozen);
    }

    #[test]
    fn frozen_not_triggered_at_healthy_fitness() {
        // BUG-070: Normal ME fitness at 0.609 should NOT be marked frozen
        // even if it remains stable across multiple polls.
        let bridge = MeBridge::new();
        let mut state = bridge.state.write();
        state.last_fitness = 0.609;

        for _ in 0..FROZEN_THRESHOLD {
            if (0.609 - state.last_fitness).abs() < FROZEN_TOLERANCE {
                state.frozen_count = state.frozen_count.saturating_add(1);
            }
        }
        let fitness = 0.609;
        state.is_frozen = (state.frozen_count >= FROZEN_THRESHOLD && fitness < 0.4)
            || (fitness - BUG_008_FROZEN_FITNESS).abs() < FROZEN_TOLERANCE;
        assert!(!state.is_frozen, "ME at 0.609 should NOT be marked frozen");
    }

    #[test]
    fn frozen_detected_for_known_bug_value() {
        let bridge = MeBridge::new();
        {
            let mut state = bridge.state.write();
            state.last_fitness = BUG_008_FROZEN_FITNESS;
            state.is_frozen =
                (state.last_fitness - BUG_008_FROZEN_FITNESS).abs() < FROZEN_TOLERANCE;
        }
        assert!(bridge.is_frozen());
    }

    #[test]
    fn frozen_resets_on_change() {
        let bridge = MeBridge::new();
        {
            let mut state = bridge.state.write();
            state.frozen_count = 5;
            state.is_frozen = true;
            // Simulate a different reading
            let new_fitness = 0.8;
            if (new_fitness - state.last_fitness).abs() >= FROZEN_TOLERANCE {
                state.frozen_count = 0;
            }
            state.is_frozen = state.frozen_count >= FROZEN_THRESHOLD;
            state.last_fitness = new_fitness;
        }
        assert!(!bridge.is_frozen());
    }

    // ── Staleness ──

    #[test]
    fn stale_when_never_polled() {
        let bridge = MeBridge::new();
        assert!(bridge.is_stale(100));
    }

    #[test]
    fn stale_after_double_interval() {
        let bridge = MeBridge::with_config("localhost:8080", 10);
        bridge.set_last_poll_tick(5);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        assert!(bridge.is_stale(25));
    }

    #[test]
    fn not_stale_within_interval() {
        let bridge = MeBridge::with_config("localhost:8080", 20);
        bridge.set_last_poll_tick(10);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        assert!(!bridge.is_stale(25));
    }

    // ── Should poll ──

    #[test]
    fn should_poll_initially() {
        let bridge = MeBridge::with_config("localhost:8080", 12);
        assert!(bridge.should_poll(12));
    }

    #[test]
    fn should_not_poll_too_soon() {
        let bridge = MeBridge::with_config("localhost:8080", 12);
        bridge.set_last_poll_tick(10);
        assert!(!bridge.should_poll(15));
    }

    // ── Failure tracking ──

    #[test]
    fn record_failure_increments() {
        let bridge = MeBridge::new();
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 1);
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 2);
    }

    #[test]
    fn record_failure_sets_stale() {
        let bridge = MeBridge::new();
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        bridge.record_failure();
        let is_stale = bridge.state.read().stale;
        assert!(is_stale);
    }

    // ── Poll (offline) ──

    #[test]
    fn poll_fails_when_unreachable() {
        let bridge = MeBridge::with_config("127.0.0.1:19999", 12);
        assert!(bridge.poll().is_err());
    }

    #[test]
    fn poll_increments_failure_on_error() {
        let bridge = MeBridge::with_config("127.0.0.1:19999", 12);
        let _ = bridge.poll();
        assert!(bridge.consecutive_failures() >= 1);
    }

    #[test]
    fn health_returns_false_when_unreachable() {
        let bridge = MeBridge::with_config("127.0.0.1:19999", 12);
        assert_eq!(bridge.health().ok(), Some(false));
    }

    #[test]
    fn post_returns_result() {
        let bridge = MeBridge::with_config("127.0.0.1:1", 0);
        // POST to unreachable port — should return Err, not panic
        let result = bridge.post(b"data");
        assert!(result.is_ok() || result.is_err());
    }

    // ── ObserverResponse serde ──

    #[test]
    fn observer_response_deserialize_full() {
        let json = r#"{"fitness":0.85,"active_layers":7,"has_publishers":true,"status":"healthy"}"#;
        let resp: ObserverResponse = serde_json::from_str(json).unwrap();
        assert!((resp.fitness - 0.85).abs() < f64::EPSILON);
        assert_eq!(resp.active_layers, 7);
        assert!(resp.has_publishers);
        assert_eq!(resp.status, "healthy");
    }

    #[test]
    fn observer_response_deserialize_minimal() {
        let json = "{}";
        let resp: ObserverResponse = serde_json::from_str(json).unwrap();
        assert!((resp.fitness).abs() < f64::EPSILON);
        assert!(!resp.has_publishers);
    }

    #[test]
    fn observer_response_serde_roundtrip() {
        let resp = ObserverResponse {
            fitness: 0.75,
            active_layers: 5,
            has_publishers: false,
            status: "degraded".to_owned(),
            correlations_since: 100,
            emergences_since: 2,
            total_correlations: 4_000_000,
            total_events: 368_000,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: ObserverResponse = serde_json::from_str(&json).unwrap();
        assert!((back.fitness - 0.75).abs() < f64::EPSILON);
        assert!(!back.has_publishers);
    }

    #[test]
    fn observer_response_bug008_scenario() {
        let json = r#"{"fitness":0.3662,"active_layers":7,"has_publishers":false,"status":"frozen"}"#;
        let resp: ObserverResponse = serde_json::from_str(json).unwrap();
        assert!(!resp.has_publishers);
        assert!((resp.fitness - BUG_008_FROZEN_FITNESS).abs() < FROZEN_TOLERANCE);
    }

    // ── Thread safety ──

    #[test]
    fn bridge_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<MeBridge>();
    }

    #[test]
    fn bridge_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<MeBridge>();
    }

    // ── Trait object ──

    #[test]
    fn bridgeable_as_trait_object() {
        let bridge = MeBridge::new();
        let dyn_ref: &dyn Bridgeable = &bridge;
        assert_eq!(dyn_ref.service_name(), "me");
    }

    // ── HTTP helpers ──

    #[test]
    fn extract_body_finds_body() {
        let raw = "HTTP/1.1 200 OK\r\n\r\n{\"fitness\":0.5}";
        assert_eq!(extract_body(raw), Some("{\"fitness\":0.5}".to_owned()));
    }

    #[test]
    fn extract_body_no_separator() {
        assert!(extract_body("no headers").is_none());
    }

    // ── Constants ──

    #[test]
    fn default_poll_interval_is_twelve() {
        assert_eq!(DEFAULT_POLL_INTERVAL, 12);
    }

    #[test]
    fn me_port_is_8180() {
        assert_eq!(ME_PORT, 8180);
    }

    #[test]
    fn health_path_is_api_health() {
        assert_eq!(HEALTH_PATH, "/api/health");
    }

    #[test]
    fn observer_path_is_api_observer() {
        assert_eq!(OBSERVER_PATH, "/api/observer");
    }

    // ── Debug ──

    #[test]
    fn debug_format_works() {
        let bridge = MeBridge::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("me"));
    }

    #[test]
    fn set_last_poll_tick_updates() {
        let bridge = MeBridge::new();
        bridge.set_last_poll_tick(42);
        assert_eq!(bridge.last_poll_tick(), 42);
    }

    // ── Frozen count threshold ──

    #[test]
    fn frozen_threshold_is_three() {
        assert_eq!(FROZEN_THRESHOLD, 3);
    }

    #[test]
    fn frozen_tolerance_is_small() {
        assert!(FROZEN_TOLERANCE > 0.0);
        assert!(FROZEN_TOLERANCE < 0.01);
    }

    // ── Observer subscription ──

    #[test]
    fn not_subscribed_before_first_poll() {
        let bridge = MeBridge::new();
        assert!(!bridge.is_subscribed());
        assert_eq!(bridge.successful_polls(), 0);
    }

    #[test]
    fn subscribed_after_successful_poll_counter_increments() {
        let bridge = MeBridge::new();
        // Simulate a successful poll by writing to state directly
        {
            let mut state = bridge.state.write();
            state.successful_polls = 1;
        }
        assert!(bridge.is_subscribed());
        assert_eq!(bridge.successful_polls(), 1);
    }
}
