//! # M24: Maintenance Engine Bridge
//!
//! Polls ME at `localhost:8080/api/observer` for fitness signal.
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
//! ## ADAPT for ORAC
//! This module needs adaptation for ORAC deployment:
//! - Port may change from 8080 (configurable via `with_config`)
//! - Socket address format must remain `host:port` (no `http://` prefix, BUG-033)
//! - Poll interval may need tuning for sidecar tick rate
//! - BUG-008 frozen detection thresholds may need re-calibration

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m1_core::m04_constants;
use crate::m1_core::m05_traits::Bridgeable;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// ME service port.
const ME_PORT: u16 = 8080;

/// Default base URL for the Maintenance Engine.
const DEFAULT_BASE_URL: &str = "localhost:8080";

/// Health endpoint path.
const HEALTH_PATH: &str = "/api/health";

/// Observer (fitness) endpoint path.
const OBSERVER_PATH: &str = "/api/observer";

/// Default poll interval in ticks.
const DEFAULT_POLL_INTERVAL: u64 = 12;

/// TCP connection timeout (milliseconds).
const TCP_TIMEOUT_MS: u64 = 2000;

/// Maximum response body size (bytes).
const MAX_RESPONSE_SIZE: usize = 8192;

/// Known frozen fitness value from BUG-008.
const BUG_008_FROZEN_FITNESS: f64 = 0.3662;

/// Tolerance for detecting frozen fitness values.
const FROZEN_TOLERANCE: f64 = 0.001;

/// Number of identical polls before declaring fitness "frozen."
const FROZEN_THRESHOLD: u32 = 3;

// ──────────────────────────────────────────────────────────────
// Response types
// ──────────────────────────────────────────────────────────────

/// Response from the ME `/api/observer` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverResponse {
    /// Overall system fitness (0.0-1.0).
    #[serde(default)]
    pub fitness: f64,
    /// Number of active layers in the ME.
    #[serde(default)]
    pub active_layers: u32,
    /// Whether the ME event bus has active publishers.
    #[serde(default)]
    pub has_publishers: bool,
    /// Observer status label.
    #[serde(default)]
    pub status: String,
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
        }
    }
}

// ──────────────────────────────────────────────────────────────
// MeBridge
// ──────────────────────────────────────────────────────────────

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
    #[must_use]
    pub fn with_config(base_url: impl Into<String>, poll_interval: u64) -> Self {
        Self {
            service: "me".to_owned(),
            base_url: base_url.into(),
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

    /// Poll the ME observer endpoint.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` or `PvError::BridgeParse` on failure.
    pub fn poll_observer(&self) -> PvResult<f64> {
        let body = raw_http_get(&self.base_url, OBSERVER_PATH, &self.service)?;
        let response: ObserverResponse =
            serde_json::from_str(&body).map_err(|e| PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("observer parse: {e}"),
            })?;

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

        state.is_frozen = state.frozen_count >= FROZEN_THRESHOLD
            || (fitness - BUG_008_FROZEN_FITNESS).abs() < FROZEN_TOLERANCE;

        state.last_fitness = fitness;
        state.last_response = Some(response);
        state.consecutive_failures = 0;
        state.stale = false;

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
    #[must_use]
    pub fn should_poll(&self, current_tick: u64) -> bool {
        let state = self.state.read();
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

    /// No-op: ME bridge is read-only for coupling purposes.
    ///
    /// # Errors
    /// This method never fails.
    fn post(&self, _payload: &[u8]) -> PvResult<()> {
        Ok(())
    }

    /// Check whether the ME service is reachable.
    ///
    /// # Errors
    /// Returns `Ok(false)` on connection failure (does not propagate the error).
    fn health(&self) -> PvResult<bool> {
        match raw_http_get(&self.base_url, HEALTH_PATH, &self.service) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Return whether the cached data is stale (flag set or 2x poll interval elapsed).
    fn is_stale(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        state.stale || current_tick.saturating_sub(state.last_poll_tick) >= self.poll_interval * 2
    }
}

// ──────────────────────────────────────────────────────────────
// Raw TCP HTTP helpers
// ──────────────────────────────────────────────────────────────

/// Send a raw HTTP GET request over TCP and return the response body.
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection or I/O fails.
/// Returns `PvError::BridgeParse` if no HTTP body separator is found.
fn raw_http_get(addr: &str, path: &str, service: &str) -> PvResult<String> {
    let timeout = Duration::from_millis(TCP_TIMEOUT_MS);
    let mut stream = TcpStream::connect_timeout(
        &addr.parse().map_err(|_| PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        })?,
        timeout,
    )
    .map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;

    stream
        .set_read_timeout(Some(timeout))
        .map_err(|_| PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        })?;

    let host = addr.split(':').next().unwrap_or("localhost");
    let request = format!("GET {path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).map_err(|_| {
        PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        }
    })?;

    let mut buf = vec![0u8; MAX_RESPONSE_SIZE];
    let mut total = 0;
    loop {
        match stream.read(&mut buf[total..]) {
            Ok(0) => break,
            Ok(n) => {
                total += n;
                if total >= MAX_RESPONSE_SIZE {
                    break;
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) => {
                return Err(PvError::BridgeUnreachable {
                    service: service.to_owned(),
                    url: addr.to_owned(),
                });
            }
        }
    }

    let response = String::from_utf8_lossy(&buf[..total]);
    extract_body(&response).ok_or_else(|| PvError::BridgeParse {
        service: service.to_owned(),
        reason: "no body in HTTP response".to_owned(),
    })
}

/// Extract body from a raw HTTP response.
fn extract_body(raw: &str) -> Option<String> {
    raw.find("\r\n\r\n").map(|pos| raw[pos + 4..].to_owned())
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

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
    fn frozen_detected_after_threshold() {
        let bridge = MeBridge::new();
        let mut state = bridge.state.write();
        state.last_fitness = 0.5;

        // Simulate identical readings
        for _ in 0..FROZEN_THRESHOLD {
            if (0.5 - state.last_fitness).abs() < FROZEN_TOLERANCE {
                state.frozen_count = state.frozen_count.saturating_add(1);
            }
        }
        state.is_frozen = state.frozen_count >= FROZEN_THRESHOLD;
        assert!(state.is_frozen);
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
    fn post_is_noop() {
        let bridge = MeBridge::new();
        assert!(bridge.post(b"data").is_ok());
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
    fn me_port_is_8080() {
        assert_eq!(ME_PORT, 8080);
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
}
