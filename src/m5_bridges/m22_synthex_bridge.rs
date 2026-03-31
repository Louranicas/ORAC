//! # M22: SYNTHEX Bridge
//!
//! Bidirectional REST bridge to SYNTHEX at `localhost:8090`.
//! Polls `/v3/thermal` every 6 ticks for thermal `k_adjustment`.
//! Posts field state to `/api/ingest` (fire-and-forget).
//!
//! ## Layer: L6 | Module: M22 | Dependencies: L1
//! ## Pattern: Raw TCP HTTP, fire-and-forget writes (C14), consent-gated reads (C8)
//!
//! The thermal adjustment feeds into the consent gate (M28) before being applied
//! to the coupling field. SYNTHEX synergy is at 0.15-0.5 (ALERT-1 from Session 040).
//!
//! ## ORAC Adaptations (applied)
//! - Port configurable via `with_config` (default 8090)
//! - Socket address: raw `host:port` (no `http://` prefix, BUG-033)
//! - Poll interval configurable (default 6 ticks)

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m1_core::m04_constants;
use crate::m1_core::m05_traits::Bridgeable;

use super::http_helpers::{raw_http_get, raw_http_post};
#[cfg(test)]
use super::http_helpers::extract_body;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// SYNTHEX service port.
const SYNTHEX_PORT: u16 = 8090;

/// Default base URL for SYNTHEX.
const DEFAULT_BASE_URL: &str = "127.0.0.1:8090";

/// Health endpoint path.
const HEALTH_PATH: &str = "/api/health";

/// Thermal poll endpoint path.
const THERMAL_PATH: &str = "/v3/thermal";

/// Ingest endpoint path for posting field state.
const INGEST_PATH: &str = "/api/ingest";

/// Nexus Bus push endpoint — ORAC sends events to SYNTHEX.
const NEXUS_PUSH_PATH: &str = "/v3/nexus/push";

/// Nexus Bus pull endpoint — ORAC retrieves events from SYNTHEX.
const NEXUS_PULL_PATH: &str = "/v3/nexus/pull";

/// Default poll interval in ticks.
const DEFAULT_POLL_INTERVAL: u64 = 6;

/// Nexus Bus pull interval in ticks (pull SYNTHEX events every 3 ticks).
const NEXUS_PULL_INTERVAL: u64 = 3;

// ──────────────────────────────────────────────────────────────
// Nexus Bus event types
// ──────────────────────────────────────────────────────────────

/// An event pushed from ORAC to SYNTHEX via the Nexus Bus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusEvent {
    /// Event type discriminator.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Unix timestamp (seconds).
    pub ts: u64,
    /// Event-specific payload.
    pub data: serde_json::Value,
}

/// Envelope for pushing events to SYNTHEX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusPushEnvelope {
    /// Batch of events to deliver.
    pub events: Vec<NexusEvent>,
}

/// Envelope returned when pulling events from SYNTHEX.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NexusPullEnvelope {
    /// Batch of events queued by SYNTHEX.
    pub events: Vec<NexusEvent>,
}

// TCP_TIMEOUT_MS and MAX_RESPONSE_SIZE now in http_helpers (BUG-042)

// ──────────────────────────────────────────────────────────────
// Response types
// ──────────────────────────────────────────────────────────────

/// Response from the SYNTHEX `/v3/thermal` endpoint.
///
/// BUG-033 fix: matches actual SYNTHEX V3 API response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalResponse {
    /// Current temperature reading.
    pub temperature: f64,
    /// Target temperature (PID setpoint).
    pub target: f64,
    /// PID controller output.
    pub pid_output: f64,
    /// Heat source readings (HS-001 through HS-004).
    #[serde(default)]
    pub heat_sources: Vec<HeatSource>,
}

/// A single SYNTHEX heat source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatSource {
    /// Heat source identifier (e.g. "HS-001").
    pub id: String,
    /// Current reading value.
    pub reading: f64,
    /// Weight in the composite temperature.
    pub weight: f64,
}

impl ThermalResponse {
    /// Compute the thermal k-adjustment from temperature deviation.
    ///
    /// V1 pattern: `(1.0 - deviation * 0.2).clamp(0.8, 1.2)`
    /// Cold → boost coupling (>1.0), hot → reduce coupling (<1.0).
    ///
    /// BUG-H001 fix: validates inputs are finite before computation.
    /// Returns neutral 1.0 if temperature or target is NaN/INF.
    #[must_use]
    pub fn thermal_adjustment(&self) -> f64 {
        if !self.temperature.is_finite() || !self.target.is_finite() {
            return 1.0;
        }
        let deviation = self.temperature - self.target;
        deviation.mul_add(-0.2, 1.0).clamp(
            m04_constants::K_MOD_BUDGET_MIN,
            m04_constants::K_MOD_BUDGET_MAX,
        )
    }
}

// ──────────────────────────────────────────────────────────────
// Bridge state (interior mutability)
// ──────────────────────────────────────────────────────────────

/// Mutable state behind a `RwLock` for the SYNTHEX bridge.
#[derive(Debug)]
struct BridgeState {
    /// Last poll tick number.
    last_poll_tick: u64,
    /// Cached adjustment from the last successful poll.
    cached_adjustment: f64,
    /// Whether the cached value is stale.
    stale: bool,
    /// Number of consecutive poll failures.
    consecutive_failures: u32,
    /// Last thermal response for diagnostics.
    last_response: Option<ThermalResponse>,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            last_poll_tick: 0,
            cached_adjustment: 1.0,
            stale: true,
            consecutive_failures: 0,
            last_response: None,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// SynthexBridge
// ──────────────────────────────────────────────────────────────

/// Bridge to SYNTHEX service for thermal coupling modulation.
///
/// Implements the `Bridgeable` trait for integration with the consent gate.
/// Uses raw TCP HTTP for minimal overhead (fire-and-forget pattern).
#[derive(Debug)]
pub struct SynthexBridge {
    /// Service name identifier.
    service: String,
    /// TCP address (host:port).
    base_url: String,
    /// Poll interval in ticks.
    poll_interval: u64,
    /// Interior-mutable state.
    state: RwLock<BridgeState>,
}

impl SynthexBridge {
    /// Create a new SYNTHEX bridge with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: "synthex".to_owned(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            state: RwLock::new(BridgeState::default()),
        }
    }

    /// Create a new SYNTHEX bridge with a custom base URL and poll interval.
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
            service: "synthex".to_owned(),
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

    /// Return the last thermal response, if any.
    #[must_use]
    pub fn last_response(&self) -> Option<ThermalResponse> {
        self.state.read().last_response.clone()
    }

    /// Return the last poll tick.
    #[must_use]
    pub fn last_poll_tick(&self) -> u64 {
        self.state.read().last_poll_tick
    }

    /// Return the port number extracted from the base URL.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.base_url
            .split(':')
            .next_back()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(SYNTHEX_PORT)
    }

    /// Poll the SYNTHEX thermal endpoint.
    ///
    /// Returns the thermal adjustment factor, clamped to the
    /// `k_mod` budget range.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if TCP connection fails.
    /// Returns `PvError::BridgeParse` if the response cannot be parsed.
    pub fn poll_thermal(&self) -> PvResult<f64> {
        let body = raw_http_get(&self.base_url, THERMAL_PATH, &self.service)?;
        let response: ThermalResponse = serde_json::from_str(&body).map_err(|e| {
            PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("thermal parse: {e}"),
            }
        })?;

        // Compute adjustment from temperature/target deviation (BUG-033 fix)
        let clamped = response.thermal_adjustment();
        if !clamped.is_finite() {
            return Err(PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("non-finite thermal_adjustment: {clamped}"),
            });
        }

        {
            let mut state = self.state.write();
            state.cached_adjustment = clamped;
            state.stale = false;
            state.consecutive_failures = 0;
            state.last_response = Some(response);
        }

        Ok(clamped)
    }

    /// Post field state to the SYNTHEX ingest endpoint.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if TCP connection fails.
    /// Returns `PvError::BridgeError` if SYNTHEX responds with 4xx/5xx.
    pub fn post_field_state(&self, payload: &[u8]) -> PvResult<()> {
        raw_http_post(&self.base_url, INGEST_PATH, payload, &self.service)?;
        Ok(())
    }

    /// Record a poll failure, incrementing the consecutive failure counter.
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
    /// BUG-M002 fix: forces immediate poll when `last_poll_tick` is 0
    /// (initial state), avoiding a one-interval startup delay.
    #[must_use]
    pub fn should_poll(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        if state.last_poll_tick == 0 && state.stale {
            return true; // Force immediate first poll
        }
        current_tick.saturating_sub(state.last_poll_tick) >= self.poll_interval
    }

    // ──────────────────────────────────────────────────────────────
    // Nexus Bus operations
    // ──────────────────────────────────────────────────────────────

    /// Push a batch of events to SYNTHEX via the Nexus Bus.
    ///
    /// Events are delivered as a JSON array to `POST /v3/nexus/push`.
    /// Fire-and-forget: errors are logged but do not block the caller.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if SYNTHEX is down.
    pub fn nexus_push(&self, events: &[NexusEvent]) -> PvResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let envelope = NexusPushEnvelope {
            events: events.to_vec(),
        };
        let payload = serde_json::to_vec(&envelope).map_err(|e| PvError::BridgeParse {
            service: self.service.clone(),
            reason: format!("nexus push serialize: {e}"),
        })?;
        raw_http_post(&self.base_url, NEXUS_PUSH_PATH, &payload, &self.service)?;
        Ok(())
    }

    /// Pull queued events from SYNTHEX via the Nexus Bus.
    ///
    /// Returns events that SYNTHEX has generated since the last pull
    /// (thermal alerts, decay completions, diagnostic findings, etc.).
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if SYNTHEX is down.
    /// Returns `PvError::BridgeParse` if the response is malformed.
    pub fn nexus_pull(&self) -> PvResult<Vec<NexusEvent>> {
        let body = raw_http_get(&self.base_url, NEXUS_PULL_PATH, &self.service)?;
        let envelope: NexusPullEnvelope =
            serde_json::from_str(&body).map_err(|e| PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("nexus pull parse: {e}"),
            })?;
        Ok(envelope.events)
    }

    /// Check whether a Nexus Bus pull is due at the given tick.
    #[must_use]
    pub fn should_nexus_pull(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        current_tick.saturating_sub(state.last_poll_tick) >= NEXUS_PULL_INTERVAL
    }

    /// Create a field update event for the Nexus Bus.
    #[must_use]
    pub fn make_field_event(r: f64, k: f64, spheres: u64, k_mod: f64) -> NexusEvent {
        NexusEvent {
            event_type: "field_update".to_owned(),
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            data: serde_json::json!({
                "r": r,
                "k": k,
                "spheres": spheres,
                "k_mod": k_mod,
            }),
        }
    }

    /// Create a RALPH mutation event for the Nexus Bus.
    #[must_use]
    pub fn make_ralph_event(
        gen: u64,
        param: &str,
        old_val: f64,
        new_val: f64,
        fitness: f64,
    ) -> NexusEvent {
        NexusEvent {
            event_type: "ralph_mutation".to_owned(),
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            data: serde_json::json!({
                "gen": gen,
                "param": param,
                "old_val": old_val,
                "new_val": new_val,
                "fitness": fitness,
            }),
        }
    }

    /// Create an emergence detection event for the Nexus Bus.
    #[must_use]
    pub fn make_emergence_event(
        emergence_type: &str,
        severity: &str,
        details: &str,
    ) -> NexusEvent {
        NexusEvent {
            event_type: "emergence".to_owned(),
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            data: serde_json::json!({
                "emergence_type": emergence_type,
                "severity": severity,
                "details": details,
            }),
        }
    }

    /// Create an STDP weight shift event for the Nexus Bus.
    #[must_use]
    pub fn make_stdp_event(
        source: &str,
        target: &str,
        old_w: f64,
        new_w: f64,
    ) -> NexusEvent {
        NexusEvent {
            event_type: "stdp_shift".to_owned(),
            ts: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
            data: serde_json::json!({
                "source": source,
                "target": target,
                "old_w": old_w,
                "new_w": new_w,
            }),
        }
    }
}

impl Default for SynthexBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Bridgeable for SynthexBridge {
    /// Return the service name (`"synthex"`).
    fn service_name(&self) -> &str {
        &self.service
    }

    /// Poll the SYNTHEX thermal endpoint, recording failure on error.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the TCP connection fails.
    /// Returns `PvError::BridgeParse` if the response cannot be deserialized.
    fn poll(&self) -> PvResult<f64> {
        match self.poll_thermal() {
            Ok(adj) => Ok(adj),
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }

    /// Post field state to the SYNTHEX ingest endpoint.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the TCP connection fails.
    fn post(&self, payload: &[u8]) -> PvResult<()> {
        self.post_field_state(payload)
    }

    /// Check whether the SYNTHEX service is reachable.
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Construction ──

    #[test]
    fn new_creates_default_bridge() {
        let bridge = SynthexBridge::new();
        assert_eq!(bridge.service_name(), "synthex");
        assert_eq!(bridge.poll_interval(), DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn default_creates_same_as_new() {
        let bridge = SynthexBridge::default();
        assert_eq!(bridge.service_name(), "synthex");
    }

    #[test]
    fn with_config_custom_url() {
        let bridge = SynthexBridge::with_config("192.168.1.1:9090", 10);
        assert_eq!(bridge.base_url, "192.168.1.1:9090");
        assert_eq!(bridge.poll_interval(), 10);
    }

    #[test]
    fn with_config_minimum_poll_interval() {
        let bridge = SynthexBridge::with_config("localhost:8090", 0);
        assert_eq!(bridge.poll_interval(), 1);
    }

    #[test]
    fn port_extraction_default() {
        let bridge = SynthexBridge::new();
        assert_eq!(bridge.port(), SYNTHEX_PORT);
    }

    #[test]
    fn port_extraction_custom() {
        let bridge = SynthexBridge::with_config("localhost:9999", 6);
        assert_eq!(bridge.port(), 9999);
    }

    // ── Initial state ──

    #[test]
    fn initial_cached_adjustment_is_one() {
        let bridge = SynthexBridge::new();
        assert!((bridge.cached_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn initial_consecutive_failures_is_zero() {
        let bridge = SynthexBridge::new();
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn initial_last_response_is_none() {
        let bridge = SynthexBridge::new();
        assert!(bridge.last_response().is_none());
    }

    #[test]
    fn initial_last_poll_tick_is_zero() {
        let bridge = SynthexBridge::new();
        assert_eq!(bridge.last_poll_tick(), 0);
    }

    #[test]
    fn initial_is_stale() {
        let bridge = SynthexBridge::new();
        assert!(bridge.is_stale(0));
    }

    // ── Staleness ──

    #[test]
    fn stale_when_never_polled() {
        let bridge = SynthexBridge::new();
        assert!(bridge.is_stale(10));
    }

    #[test]
    fn stale_after_double_interval() {
        let bridge = SynthexBridge::with_config("localhost:8090", 5);
        bridge.set_last_poll_tick(10);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        // Current tick 20 = 10 ticks since last poll, 2*5=10 → stale
        assert!(bridge.is_stale(20));
    }

    #[test]
    fn not_stale_within_interval() {
        let bridge = SynthexBridge::with_config("localhost:8090", 10);
        bridge.set_last_poll_tick(5);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        // Current tick 10 = 5 ticks since last poll, 2*10=20 → not stale
        assert!(!bridge.is_stale(10));
    }

    // ── Should poll ──

    #[test]
    fn should_poll_initially() {
        let bridge = SynthexBridge::with_config("localhost:8090", 5);
        assert!(bridge.should_poll(5));
    }

    #[test]
    fn should_not_poll_too_soon() {
        let bridge = SynthexBridge::with_config("localhost:8090", 10);
        bridge.set_last_poll_tick(5);
        assert!(!bridge.should_poll(10));
    }

    #[test]
    fn should_poll_after_interval() {
        let bridge = SynthexBridge::with_config("localhost:8090", 5);
        bridge.set_last_poll_tick(10);
        assert!(bridge.should_poll(15));
    }

    // ── Failure tracking ──

    #[test]
    fn record_failure_increments_counter() {
        let bridge = SynthexBridge::new();
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 1);
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 2);
    }

    #[test]
    fn record_failure_sets_stale() {
        let bridge = SynthexBridge::new();
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        bridge.record_failure();
        let is_stale = bridge.state.read().stale;
        assert!(is_stale);
    }

    #[test]
    fn consecutive_failures_saturates() {
        let bridge = SynthexBridge::new();
        {
            let mut state = bridge.state.write();
            state.consecutive_failures = u32::MAX;
        }
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), u32::MAX);
    }

    // ── ThermalResponse serde (BUG-033 fix: matches actual SYNTHEX API) ──

    fn test_thermal_response() -> ThermalResponse {
        ThermalResponse {
            temperature: 0.572,
            target: 0.5,
            pid_output: 0.136,
            heat_sources: vec![
                HeatSource { id: "HS-001".into(), reading: 1.0, weight: 0.3 },
                HeatSource { id: "HS-002".into(), reading: 0.0, weight: 0.35 },
            ],
        }
    }

    #[test]
    fn thermal_response_deserialize_full() {
        let json = r#"{"temperature":0.572,"target":0.5,"pid_output":0.136,"heat_sources":[{"id":"HS-001","reading":1.0,"weight":0.3}]}"#;
        let resp: ThermalResponse = serde_json::from_str(json).unwrap();
        assert!((resp.temperature - 0.572).abs() < 1e-6);
        assert!((resp.target - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn thermal_response_deserialize_minimal() {
        let json = r#"{"temperature":0.5,"target":0.5,"pid_output":0.0}"#;
        let resp: ThermalResponse = serde_json::from_str(json).unwrap();
        assert!(resp.heat_sources.is_empty());
    }

    #[test]
    fn thermal_response_serialize_roundtrip() {
        let resp = test_thermal_response();
        let json = serde_json::to_string(&resp).unwrap();
        let back: ThermalResponse = serde_json::from_str(&json).unwrap();
        assert!((back.temperature - 0.572).abs() < 1e-6);
        assert_eq!(back.heat_sources.len(), 2);
    }

    #[test]
    fn thermal_response_deserialize_rejects_invalid() {
        let json = r#"{"not_a_field": 42}"#;
        let result = serde_json::from_str::<ThermalResponse>(json);
        assert!(result.is_err());
    }

    #[test]
    fn thermal_adjustment_cold_boosts() {
        let resp = ThermalResponse {
            temperature: 0.3, target: 0.5, pid_output: 0.0, heat_sources: vec![],
        };
        // deviation = -0.2, adj = 1.0 - (-0.2 * 0.2) = 1.04
        assert!(resp.thermal_adjustment() > 1.0);
    }

    #[test]
    fn thermal_adjustment_hot_reduces() {
        let resp = ThermalResponse {
            temperature: 0.8, target: 0.5, pid_output: 0.0, heat_sources: vec![],
        };
        // deviation = 0.3, adj = 1.0 - (0.3 * 0.2) = 0.94
        assert!(resp.thermal_adjustment() < 1.0);
    }

    #[test]
    fn thermal_adjustment_at_target_is_neutral() {
        let resp = ThermalResponse {
            temperature: 0.5, target: 0.5, pid_output: 0.0, heat_sources: vec![],
        };
        assert!((resp.thermal_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thermal_adjustment_nan_returns_neutral() {
        let resp = ThermalResponse {
            temperature: f64::NAN,
            target: 0.5,
            pid_output: 0.0,
            heat_sources: vec![],
        };
        assert!((resp.thermal_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn thermal_adjustment_inf_returns_neutral() {
        let resp = ThermalResponse {
            temperature: 0.5,
            target: f64::INFINITY,
            pid_output: 0.0,
            heat_sources: vec![],
        };
        assert!((resp.thermal_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    // ── HTTP helpers ──

    #[test]
    fn extract_body_finds_body() {
        let raw = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"ok\":true}";
        let body = extract_body(raw);
        assert_eq!(body, Some("{\"ok\":true}".to_owned()));
    }

    #[test]
    fn extract_body_no_separator() {
        let raw = "just some text without headers";
        assert!(extract_body(raw).is_none());
    }

    #[test]
    fn extract_body_empty_body() {
        let raw = "HTTP/1.1 204 No Content\r\n\r\n";
        assert_eq!(extract_body(raw), Some(String::new()));
    }

    #[test]
    fn extract_body_multiline_body() {
        let raw = "HTTP/1.1 200 OK\r\n\r\n{\"a\":1,\n\"b\":2}";
        let body = extract_body(raw);
        assert_eq!(body, Some("{\"a\":1,\n\"b\":2}".to_owned()));
    }

    // ── Poll (offline — service not running) ──

    #[test]
    fn poll_fails_when_service_unreachable() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let result = bridge.poll();
        assert!(result.is_err());
    }

    #[test]
    fn poll_increments_failure_on_error() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let _ = bridge.poll();
        assert!(bridge.consecutive_failures() >= 1);
    }

    #[test]
    fn health_returns_false_when_unreachable() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let result = bridge.health();
        assert_eq!(result.ok(), Some(false));
    }

    #[test]
    fn post_fails_when_service_unreachable() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let result = bridge.post(b"test");
        assert!(result.is_err());
    }

    // ── Service name ──

    #[test]
    fn service_name_is_synthex() {
        let bridge = SynthexBridge::new();
        assert_eq!(bridge.service_name(), "synthex");
    }

    // ── Adjustment clamping simulation ──

    #[test]
    fn cached_adjustment_stays_in_budget() {
        let bridge = SynthexBridge::new();
        // Simulate updating cached value
        {
            let mut state = bridge.state.write();
            state.cached_adjustment = 2.0_f64.clamp(
                m04_constants::K_MOD_BUDGET_MIN,
                m04_constants::K_MOD_BUDGET_MAX,
            );
        }
        let adj = bridge.cached_adjustment();
        assert!(adj >= m04_constants::K_MOD_BUDGET_MIN);
        assert!(adj <= m04_constants::K_MOD_BUDGET_MAX);
    }

    #[test]
    fn cached_adjustment_clamps_low() {
        let bridge = SynthexBridge::new();
        {
            let mut state = bridge.state.write();
            state.cached_adjustment = 0.5_f64.clamp(
                m04_constants::K_MOD_BUDGET_MIN,
                m04_constants::K_MOD_BUDGET_MAX,
            );
        }
        assert!((bridge.cached_adjustment() - m04_constants::K_MOD_BUDGET_MIN).abs() < f64::EPSILON);
    }

    #[test]
    fn cached_adjustment_clamps_high() {
        let bridge = SynthexBridge::new();
        {
            let mut state = bridge.state.write();
            state.cached_adjustment = 2.0_f64.clamp(
                m04_constants::K_MOD_BUDGET_MIN,
                m04_constants::K_MOD_BUDGET_MAX,
            );
        }
        assert!((bridge.cached_adjustment() - m04_constants::K_MOD_BUDGET_MAX).abs() < f64::EPSILON);
    }

    #[test]
    fn cached_adjustment_preserves_valid_value() {
        let bridge = SynthexBridge::new();
        {
            let mut state = bridge.state.write();
            state.cached_adjustment = 1.05;
        }
        assert!((bridge.cached_adjustment() - 1.05).abs() < f64::EPSILON);
    }

    // ── Thread safety ──

    #[test]
    fn bridge_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<SynthexBridge>();
    }

    #[test]
    fn bridge_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<SynthexBridge>();
    }

    // ── Bridgeable trait object ──

    #[test]
    fn bridgeable_as_trait_object() {
        let bridge = SynthexBridge::new();
        let _dyn_ref: &dyn Bridgeable = &bridge;
        assert_eq!(_dyn_ref.service_name(), "synthex");
    }

    // ── set_last_poll_tick ──

    #[test]
    fn set_last_poll_tick_updates() {
        let bridge = SynthexBridge::new();
        bridge.set_last_poll_tick(42);
        assert_eq!(bridge.last_poll_tick(), 42);
    }

    #[test]
    fn set_last_poll_tick_zero() {
        let bridge = SynthexBridge::new();
        bridge.set_last_poll_tick(100);
        bridge.set_last_poll_tick(0);
        assert_eq!(bridge.last_poll_tick(), 0);
    }

    // ── Constants ──

    #[test]
    fn default_poll_interval_is_six() {
        assert_eq!(DEFAULT_POLL_INTERVAL, 6);
    }

    #[test]
    fn synthex_port_is_8090() {
        assert_eq!(SYNTHEX_PORT, 8090);
    }

    #[test]
    fn health_path_is_api_health() {
        assert_eq!(HEALTH_PATH, "/api/health");
    }

    #[test]
    fn thermal_path_is_v3_thermal() {
        assert_eq!(THERMAL_PATH, "/v3/thermal");
    }

    // max_response_size test moved to http_helpers (BUG-042)

    // ── BridgeState default ──

    #[test]
    fn bridge_state_default_values() {
        let state = BridgeState::default();
        assert_eq!(state.last_poll_tick, 0);
        assert!((state.cached_adjustment - 1.0).abs() < f64::EPSILON);
        assert!(state.stale);
        assert_eq!(state.consecutive_failures, 0);
        assert!(state.last_response.is_none());
    }

    // ── Multiple failures don't corrupt state ──

    #[test]
    fn multiple_failures_increment_correctly() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        for _ in 0..5 {
            let _ = bridge.poll();
        }
        assert_eq!(bridge.consecutive_failures(), 5);
    }

    // ── Interleaved operations ──

    #[test]
    fn set_poll_tick_after_failure() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let _ = bridge.poll();
        bridge.set_last_poll_tick(100);
        assert_eq!(bridge.last_poll_tick(), 100);
        assert!(bridge.consecutive_failures() >= 1);
    }

    // ── Debug trait ──

    #[test]
    fn debug_format_works() {
        let bridge = SynthexBridge::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("synthex"));
    }

    #[test]
    fn thermal_response_debug() {
        let resp = test_thermal_response();
        let debug = format!("{resp:?}");
        assert!(debug.contains("ThermalResponse"));
    }

    // ── Nexus Bus types ──

    #[test]
    fn nexus_event_serialize_roundtrip() {
        let event = NexusEvent {
            event_type: "field_update".to_owned(),
            ts: 1_711_700_000,
            data: serde_json::json!({"r": 0.95, "k": 1.5}),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: NexusEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(back.event_type, "field_update");
        assert_eq!(back.ts, 1_711_700_000);
    }

    #[test]
    fn nexus_push_envelope_serialize() {
        let envelope = NexusPushEnvelope {
            events: vec![NexusEvent {
                event_type: "emergence".to_owned(),
                ts: 0,
                data: serde_json::json!({"type": "CoherenceLock"}),
            }],
        };
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("emergence"));
        assert!(json.contains("events"));
    }

    #[test]
    fn nexus_pull_envelope_deserialize() {
        let json = r#"{"events":[{"type":"thermal_alert","ts":100,"data":{"temperature":0.85}}]}"#;
        let envelope: NexusPullEnvelope = serde_json::from_str(json).unwrap();
        assert_eq!(envelope.events.len(), 1);
        assert_eq!(envelope.events[0].event_type, "thermal_alert");
    }

    #[test]
    fn nexus_pull_empty_events() {
        let json = r#"{"events":[]}"#;
        let envelope: NexusPullEnvelope = serde_json::from_str(json).unwrap();
        assert!(envelope.events.is_empty());
    }

    #[test]
    fn nexus_push_empty_is_noop() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let result = bridge.nexus_push(&[]);
        assert!(result.is_ok());
    }

    #[test]
    fn nexus_push_fails_when_unreachable() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let event = NexusEvent {
            event_type: "test".to_owned(),
            ts: 0,
            data: serde_json::json!({}),
        };
        let result = bridge.nexus_push(&[event]);
        assert!(result.is_err());
    }

    #[test]
    fn nexus_pull_fails_when_unreachable() {
        let bridge = SynthexBridge::with_config("127.0.0.1:19999", 6);
        let result = bridge.nexus_pull();
        assert!(result.is_err());
    }

    #[test]
    fn make_field_event_has_correct_type() {
        let event = SynthexBridge::make_field_event(0.95, 1.5, 6, 1.0);
        assert_eq!(event.event_type, "field_update");
        assert!(event.ts > 0);
        let r = event.data.get("r").and_then(|v| v.as_f64()).unwrap();
        assert!((r - 0.95).abs() < f64::EPSILON);
    }

    #[test]
    fn make_ralph_event_has_correct_type() {
        let event = SynthexBridge::make_ralph_event(17000, "K", 1.5, 1.8, 0.83);
        assert_eq!(event.event_type, "ralph_mutation");
        let gen = event.data.get("gen").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(gen, 17000);
    }

    #[test]
    fn make_emergence_event_has_correct_type() {
        let event = SynthexBridge::make_emergence_event("CoherenceLock", "HIGH", "r > 0.999");
        assert_eq!(event.event_type, "emergence");
    }

    #[test]
    fn make_stdp_event_has_correct_type() {
        let event = SynthexBridge::make_stdp_event("pane-1", "pane-2", 0.3, 0.45);
        assert_eq!(event.event_type, "stdp_shift");
        let old = event.data.get("old_w").and_then(|v| v.as_f64()).unwrap();
        assert!((old - 0.3).abs() < f64::EPSILON);
    }

    #[test]
    fn should_nexus_pull_respects_interval() {
        let bridge = SynthexBridge::with_config("localhost:8090", 6);
        bridge.set_last_poll_tick(10);
        // NEXUS_PULL_INTERVAL = 3, tick 13 = 3 ticks since last poll
        assert!(bridge.should_nexus_pull(13));
    }

    #[test]
    fn should_nexus_pull_too_soon() {
        let bridge = SynthexBridge::with_config("localhost:8090", 6);
        bridge.set_last_poll_tick(10);
        // Only 1 tick since last poll
        assert!(!bridge.should_nexus_pull(11));
    }

    // ── Nexus Bus constants ──

    #[test]
    fn nexus_push_path_is_correct() {
        assert_eq!(NEXUS_PUSH_PATH, "/v3/nexus/push");
    }

    #[test]
    fn nexus_pull_path_is_correct() {
        assert_eq!(NEXUS_PULL_PATH, "/v3/nexus/pull");
    }

    #[test]
    fn nexus_pull_interval_is_three() {
        assert_eq!(NEXUS_PULL_INTERVAL, 3);
    }
}
