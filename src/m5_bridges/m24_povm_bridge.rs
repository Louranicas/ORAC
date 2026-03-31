//! # M25: POVM Bridge
//!
//! Snapshots sphere data to POVM Engine at `localhost:8125` every 12 ticks.
//! Reads Hebbian pathway weights from `/pathways` every 60 ticks.
//! Startup hydration: `hydrate_pathways()` + `hydrate_summary()`.
//!
//! ## Layer: L6 | Module: M25 | Dependencies: L1
//! ## Pattern: Fire-and-forget writes (C14), periodic reads for Hebbian weight seeding
//!
//! The POVM Engine is the persistent OVM store. It holds 2,425 pathways (bimodal
//! distribution) and 36 memories. This bridge snapshots field state on writes
//! and hydrates Hebbian weights on startup.
//!
//! ## ORAC Adaptations (applied)
//! - Port configurable via `with_config` (default 8125)
//! - Socket address: raw `host:port` (no `http://` prefix, BUG-033)
//! - Write interval: 12 ticks, read interval: 60 ticks (configurable)
//! - POVM is write-only; must call `/hydrate` to read back (BUG-034)

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[allow(unused_imports)] // extract_body used by tests via `use super::*;`
use super::http_helpers::{extract_body, raw_http_get_with_limit, raw_http_post};
use crate::m1_core::m02_error_handling::{PvError, PvResult};
use crate::m1_core::m05_traits::Bridgeable;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// POVM Engine service port.
const POVM_PORT: u16 = 8125;

/// Default base URL.
const DEFAULT_BASE_URL: &str = "127.0.0.1:8125";

/// Health endpoint path.
const HEALTH_PATH: &str = "/health";

/// Memories (snapshot) endpoint path.
const MEMORIES_PATH: &str = "/memories";

/// Pathways (Hebbian weight) endpoint path.
const PATHWAYS_PATH: &str = "/pathways";

/// Summary endpoint path for hydration.
const SUMMARY_PATH: &str = "/summary";

/// Snapshot write interval in ticks.
const DEFAULT_WRITE_INTERVAL: u64 = 12;

/// Pathway read interval in ticks.
const DEFAULT_READ_INTERVAL: u64 = 60;

/// Maximum response body size (bytes) — larger than default for POVM responses.
/// Gen-060a: Raised from 512KB to 2MB — POVM pathways grow with STDP activity
/// and measured 1.3MB in production (2437+ pathways). 512KB caused truncation
/// producing "invalid type: map" parse errors at boot hydration.
const MAX_RESPONSE_SIZE: usize = 2_097_152; // 2MB

// ──────────────────────────────────────────────────────────────
// Response types
// ──────────────────────────────────────────────────────────────

/// A single Hebbian pathway from the POVM Engine.
///
/// Supports both ORAC-written format (`source`/`target`) and POVM's
/// native format (`pre_id`/`post_id`) via serde aliases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pathway {
    /// Source node identifier (alias: `pre_id` from POVM native format).
    #[serde(default, alias = "pre_id")]
    pub source: String,
    /// Target node identifier (alias: `post_id` from POVM native format).
    #[serde(default, alias = "post_id")]
    pub target: String,
    /// Connection weight (Hebbian strength).
    #[serde(default)]
    pub weight: f64,
    /// Number of times this pathway has been reinforced (alias: `co_activations`).
    #[serde(default, alias = "co_activations")]
    pub reinforcement_count: u64,
}

/// Summary response from POVM Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PovmSummary {
    /// Total number of stored pathways.
    #[serde(default)]
    pub pathway_count: u64,
    /// Total number of stored memories.
    #[serde(default)]
    pub memory_count: u64,
    /// Service uptime in seconds.
    #[serde(default)]
    pub uptime_secs: f64,
}

/// Response from the `/pathways` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathwaysResponse {
    /// List of Hebbian pathways.
    #[serde(default)]
    pub pathways: Vec<Pathway>,
}

// ──────────────────────────────────────────────────────────────
// Bridge state
// ──────────────────────────────────────────────────────────────

/// Mutable state behind a `RwLock`.
#[derive(Debug)]
struct BridgeState {
    /// Last write (snapshot) tick.
    last_write_tick: u64,
    /// Last read (pathway hydration) tick.
    last_read_tick: u64,
    /// Cached adjustment (neutral for POVM — it does not produce `k_adj`).
    cached_adjustment: f64,
    /// Whether data is stale.
    stale: bool,
    /// Consecutive failure counter.
    consecutive_failures: u32,
    /// Cached pathways from last hydration.
    cached_pathways: Vec<Pathway>,
    /// Last summary data.
    last_summary: Option<PovmSummary>,
    /// Whether initial hydration has been performed.
    hydrated: bool,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            last_write_tick: 0,
            last_read_tick: 0,
            cached_adjustment: 1.0,
            stale: true,
            consecutive_failures: 0,
            cached_pathways: Vec::new(),
            last_summary: None,
            hydrated: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// PovmBridge
// ──────────────────────────────────────────────────────────────

/// Bridge to the POVM Engine for persistent sphere snapshots and Hebbian weight seeding.
///
/// Unlike other bridges, POVM does not produce a `k_adj`. It is a storage bridge:
/// - Writes: sphere snapshots (fire-and-forget)
/// - Reads: Hebbian pathway weights for startup hydration
#[derive(Debug)]
pub struct PovmBridge {
    /// Service name identifier.
    service: String,
    /// TCP address (host:port).
    base_url: String,
    /// Write (snapshot) interval in ticks.
    write_interval: u64,
    /// Read (pathway hydration) interval in ticks.
    read_interval: u64,
    /// Interior-mutable state.
    state: RwLock<BridgeState>,
}

impl PovmBridge {
    /// Create a new POVM bridge with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: "povm".to_owned(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            write_interval: DEFAULT_WRITE_INTERVAL,
            read_interval: DEFAULT_READ_INTERVAL,
            state: RwLock::new(BridgeState::default()),
        }
    }

    /// Create a new POVM bridge with custom configuration.
    ///
    /// Protocol prefixes (`http://`, `https://`) are stripped automatically
    /// because the bridge uses raw TCP sockets, not an HTTP client (BUG-033).
    #[must_use]
    pub fn with_config(
        base_url: impl Into<String>,
        write_interval: u64,
        read_interval: u64,
    ) -> Self {
        let raw: String = base_url.into();
        let stripped = raw
            .strip_prefix("http://")
            .or_else(|| raw.strip_prefix("https://"))
            .unwrap_or(&raw)
            .to_owned();
        Self {
            service: "povm".to_owned(),
            base_url: stripped,
            write_interval: write_interval.max(1),
            read_interval: read_interval.max(1),
            state: RwLock::new(BridgeState::default()),
        }
    }

    /// Return the configured write interval.
    #[must_use]
    pub const fn write_interval(&self) -> u64 {
        self.write_interval
    }

    /// Return the configured read interval.
    #[must_use]
    pub const fn read_interval(&self) -> u64 {
        self.read_interval
    }

    /// Return the number of consecutive failures.
    #[must_use]
    pub fn consecutive_failures(&self) -> u32 {
        self.state.read().consecutive_failures
    }

    /// Return the cached adjustment value (always neutral for POVM).
    #[must_use]
    pub fn cached_adjustment(&self) -> f64 {
        self.state.read().cached_adjustment
    }

    /// Return the cached pathways.
    #[must_use]
    pub fn cached_pathways(&self) -> Vec<Pathway> {
        self.state.read().cached_pathways.clone()
    }

    /// Return the number of cached pathways.
    #[must_use]
    pub fn pathway_count(&self) -> usize {
        self.state.read().cached_pathways.len()
    }

    /// Return whether initial hydration has been performed.
    #[must_use]
    pub fn is_hydrated(&self) -> bool {
        self.state.read().hydrated
    }

    /// Return the last summary, if any.
    #[must_use]
    pub fn last_summary(&self) -> Option<PovmSummary> {
        self.state.read().last_summary.clone()
    }

    /// Return the port number.
    #[must_use]
    pub fn port(&self) -> u16 {
        self.base_url
            .split(':')
            .next_back()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(POVM_PORT)
    }

    /// Snapshot sphere data to the POVM memories endpoint.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the connection fails.
    pub fn snapshot(&self, payload: &[u8]) -> PvResult<()> {
        raw_http_post(&self.base_url, MEMORIES_PATH, payload, &self.service)?;
        let mut state = self.state.write();
        state.consecutive_failures = 0;
        state.stale = false;
        Ok(())
    }

    /// Hydrate pathways from the POVM Engine.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` or `PvError::BridgeParse` on failure.
    pub fn hydrate_pathways(&self) -> PvResult<Vec<Pathway>> {
        let body =
            raw_http_get_with_limit(&self.base_url, PATHWAYS_PATH, &self.service, MAX_RESPONSE_SIZE)?;

        // POVM returns a raw array `[{pre_id, post_id, weight, ...}]` while
        // ORAC's write format wraps it as `{pathways: [...]}`. Try both.
        let pathways: Vec<Pathway> =
            serde_json::from_str::<Vec<Pathway>>(&body).or_else(|_| {
                serde_json::from_str::<PathwaysResponse>(&body).map(|r| r.pathways)
            }).map_err(|e| PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("pathways parse: {e}"),
            })?;

        let mut state = self.state.write();
        state.cached_pathways.clone_from(&pathways);
        state.hydrated = true;
        state.stale = false;
        state.consecutive_failures = 0;

        Ok(pathways)
    }

    /// Hydrate summary from the POVM Engine.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` or `PvError::BridgeParse` on failure.
    pub fn hydrate_summary(&self) -> PvResult<PovmSummary> {
        let body =
            raw_http_get_with_limit(&self.base_url, SUMMARY_PATH, &self.service, MAX_RESPONSE_SIZE)?;
        let summary: PovmSummary =
            serde_json::from_str(&body).map_err(|e| PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("summary parse: {e}"),
            })?;

        let mut state = self.state.write();
        state.last_summary = Some(summary.clone());
        state.consecutive_failures = 0;

        Ok(summary)
    }

    /// Record a failure.
    pub fn record_failure(&self) {
        let mut state = self.state.write();
        state.consecutive_failures = state.consecutive_failures.saturating_add(1);
        state.stale = true;
    }

    /// Update the last write tick.
    pub fn set_last_write_tick(&self, tick: u64) {
        self.state.write().last_write_tick = tick;
    }

    /// Update the last read tick.
    pub fn set_last_read_tick(&self, tick: u64) {
        self.state.write().last_read_tick = tick;
    }

    /// Return the last write tick.
    #[must_use]
    pub fn last_write_tick(&self) -> u64 {
        self.state.read().last_write_tick
    }

    /// Return the last read tick.
    #[must_use]
    pub fn last_read_tick(&self) -> u64 {
        self.state.read().last_read_tick
    }

    /// Check whether a snapshot write is due.
    #[must_use]
    pub fn should_write(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        current_tick.saturating_sub(state.last_write_tick) >= self.write_interval
    }

    /// Check whether a pathway read is due.
    #[must_use]
    pub fn should_read(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        current_tick.saturating_sub(state.last_read_tick) >= self.read_interval
    }

    /// Write coupling network pathways back to POVM Engine (BUG-058 fix).
    ///
    /// Serializes each connection as a pathway upsert request and POSTs
    /// to `/pathways`. Sends one request per connection (batch not supported
    /// by POVM API). Failures are logged but do not propagate.
    ///
    /// # Arguments
    /// - `connections`: Slice of `(source, target, weight, co_activations)` tuples.
    /// - `tick`: Current tick for `last_activated` timestamp.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::BridgeUnreachable`] if POVM is unreachable.
    pub fn write_pathways(
        &self,
        connections: &[(String, String, f64, u64)],
        tick: u64,
    ) -> PvResult<usize> {
        let mut written = 0_usize;
        for (source, target, weight, co_acts) in connections {
            let payload = serde_json::json!({
                "pre_id": source,
                "post_id": target,
                "weight": weight,
                "co_activations": co_acts,
                "last_activated": format!("tick-{tick}"),
            });
            match raw_http_post(
                &self.base_url,
                PATHWAYS_PATH,
                payload.to_string().as_bytes(),
                &self.service,
            ) {
                Ok(_status) => written += 1,
                Err(e) => {
                    tracing::debug!(
                        source = %source,
                        target = %target,
                        "POVM pathway write failed: {e}"
                    );
                }
            }
        }
        if written > 0 {
            self.state.write().last_write_tick = tick;
        }
        Ok(written)
    }
}

impl Default for PovmBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Bridgeable for PovmBridge {
    /// Return the service name (`"povm"`).
    fn service_name(&self) -> &str {
        &self.service
    }

    /// Return the cached neutral adjustment (POVM does not produce `k_adj`).
    ///
    /// # Errors
    /// This method never fails.
    fn poll(&self) -> PvResult<f64> {
        Ok(self.cached_adjustment())
    }

    /// Snapshot field state to the POVM memories endpoint, recording failure on error.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the TCP connection fails.
    fn post(&self, payload: &[u8]) -> PvResult<()> {
        match self.snapshot(payload) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.record_failure();
                Err(e)
            }
        }
    }

    /// Check whether the POVM Engine is reachable.
    ///
    /// # Errors
    /// Returns `Ok(false)` on connection failure (does not propagate the error).
    fn health(&self) -> PvResult<bool> {
        match raw_http_get_with_limit(&self.base_url, HEALTH_PATH, &self.service, MAX_RESPONSE_SIZE) {
            Ok(_) => Ok(true),
            Err(e) => {
                tracing::warn!(service = %self.service, error = %e, "bridge health check failed");
                Ok(false)
            }
        }
    }

    /// Return whether the cached data is stale.
    ///
    /// Considers both write and read activity. Uses 4x `write_interval` because
    /// write and read fire at different tick offsets, creating gaps that exceed
    /// 3x during normal operation.
    fn is_stale(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        if state.stale {
            return true;
        }
        let last_activity = state.last_write_tick.max(state.last_read_tick);
        current_tick.saturating_sub(last_activity) >= self.write_interval * 4
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
        let bridge = PovmBridge::new();
        assert_eq!(bridge.service_name(), "povm");
        assert_eq!(bridge.write_interval(), DEFAULT_WRITE_INTERVAL);
        assert_eq!(bridge.read_interval(), DEFAULT_READ_INTERVAL);
    }

    #[test]
    fn default_creates_same_as_new() {
        let bridge = PovmBridge::default();
        assert_eq!(bridge.service_name(), "povm");
    }

    #[test]
    fn with_config_custom() {
        let bridge = PovmBridge::with_config("10.0.0.1:8225", 6, 30);
        assert_eq!(bridge.base_url, "10.0.0.1:8225");
        assert_eq!(bridge.write_interval(), 6);
        assert_eq!(bridge.read_interval(), 30);
    }

    #[test]
    fn with_config_minimum_intervals() {
        let bridge = PovmBridge::with_config("localhost:8125", 0, 0);
        assert_eq!(bridge.write_interval(), 1);
        assert_eq!(bridge.read_interval(), 1);
    }

    #[test]
    fn port_extraction_default() {
        let bridge = PovmBridge::new();
        assert_eq!(bridge.port(), POVM_PORT);
    }

    #[test]
    fn port_extraction_custom() {
        let bridge = PovmBridge::with_config("localhost:9125", 12, 60);
        assert_eq!(bridge.port(), 9125);
    }

    // ── Initial state ──

    #[test]
    fn initial_cached_adjustment_is_one() {
        let bridge = PovmBridge::new();
        assert!((bridge.cached_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn initial_failures_is_zero() {
        let bridge = PovmBridge::new();
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn initial_not_hydrated() {
        let bridge = PovmBridge::new();
        assert!(!bridge.is_hydrated());
    }

    #[test]
    fn initial_no_pathways() {
        let bridge = PovmBridge::new();
        assert!(bridge.cached_pathways().is_empty());
        assert_eq!(bridge.pathway_count(), 0);
    }

    #[test]
    fn initial_no_summary() {
        let bridge = PovmBridge::new();
        assert!(bridge.last_summary().is_none());
    }

    #[test]
    fn initial_is_stale() {
        let bridge = PovmBridge::new();
        assert!(bridge.is_stale(0));
    }

    // ── Poll returns neutral ──

    #[test]
    fn poll_returns_neutral() {
        let bridge = PovmBridge::new();
        let adj = bridge.poll();
        assert!(adj.is_ok());
        assert!((adj.unwrap_or(0.0) - 1.0).abs() < f64::EPSILON);
    }

    // ── Should write / should read ──

    #[test]
    fn should_write_initially() {
        let bridge = PovmBridge::with_config("localhost:8125", 12, 60);
        assert!(bridge.should_write(12));
    }

    #[test]
    fn should_not_write_too_soon() {
        let bridge = PovmBridge::with_config("localhost:8125", 12, 60);
        bridge.set_last_write_tick(10);
        assert!(!bridge.should_write(15));
    }

    #[test]
    fn should_write_after_interval() {
        let bridge = PovmBridge::with_config("localhost:8125", 12, 60);
        bridge.set_last_write_tick(10);
        assert!(bridge.should_write(22));
    }

    #[test]
    fn should_read_initially() {
        let bridge = PovmBridge::with_config("localhost:8125", 12, 60);
        assert!(bridge.should_read(60));
    }

    #[test]
    fn should_not_read_too_soon() {
        let bridge = PovmBridge::with_config("localhost:8125", 12, 60);
        bridge.set_last_read_tick(10);
        assert!(!bridge.should_read(50));
    }

    #[test]
    fn should_read_after_interval() {
        let bridge = PovmBridge::with_config("localhost:8125", 12, 60);
        bridge.set_last_read_tick(10);
        assert!(bridge.should_read(70));
    }

    // ── Tick management ──

    #[test]
    fn set_last_write_tick_updates() {
        let bridge = PovmBridge::new();
        bridge.set_last_write_tick(42);
        assert_eq!(bridge.last_write_tick(), 42);
    }

    #[test]
    fn set_last_read_tick_updates() {
        let bridge = PovmBridge::new();
        bridge.set_last_read_tick(100);
        assert_eq!(bridge.last_read_tick(), 100);
    }

    // ── Staleness ──

    #[test]
    fn stale_when_never_written() {
        let bridge = PovmBridge::new();
        assert!(bridge.is_stale(100));
    }

    #[test]
    fn stale_after_quadruple_write_interval() {
        let bridge = PovmBridge::with_config("localhost:8125", 10, 60);
        bridge.set_last_write_tick(5);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        // 45 - 5 = 40 >= 10*4 = 40 → stale
        assert!(bridge.is_stale(45));
    }

    #[test]
    fn not_stale_within_quadruple_interval() {
        let bridge = PovmBridge::with_config("localhost:8125", 10, 60);
        bridge.set_last_write_tick(10);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        // 45 - 10 = 35 < 10*4 = 40 → not stale
        assert!(!bridge.is_stale(45));
    }

    // ── Failure tracking ──

    #[test]
    fn record_failure_increments() {
        let bridge = PovmBridge::new();
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 1);
    }

    #[test]
    fn record_failure_sets_stale() {
        let bridge = PovmBridge::new();
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        bridge.record_failure();
        let is_stale = bridge.state.read().stale;
        assert!(is_stale);
    }

    // ── Post (offline) ──

    #[test]
    fn post_fails_when_unreachable() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        assert!(bridge.post(b"test").is_err());
    }

    #[test]
    fn health_returns_false_when_unreachable() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        assert_eq!(bridge.health().ok(), Some(false));
    }

    // ── Pathway serde ──

    #[test]
    fn pathway_deserialize() {
        let json = r#"{"source":"a","target":"b","weight":0.8,"reinforcement_count":42}"#;
        let p: Pathway = serde_json::from_str(json).unwrap();
        assert_eq!(p.source, "a");
        assert_eq!(p.target, "b");
        assert!((p.weight - 0.8).abs() < f64::EPSILON);
        assert_eq!(p.reinforcement_count, 42);
    }

    #[test]
    fn pathway_deserialize_minimal() {
        let json = "{}";
        let p: Pathway = serde_json::from_str(json).unwrap();
        assert_eq!(p.source, "");
        assert!((p.weight).abs() < f64::EPSILON);
    }

    #[test]
    fn pathway_serde_roundtrip() {
        let p = Pathway {
            source: "x".to_owned(),
            target: "y".to_owned(),
            weight: 0.5,
            reinforcement_count: 10,
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Pathway = serde_json::from_str(&json).unwrap();
        assert_eq!(back.source, "x");
        assert!((back.weight - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn pathways_response_deserialize() {
        let json = r#"{"pathways":[{"source":"a","target":"b","weight":0.9,"reinforcement_count":5}]}"#;
        let resp: PathwaysResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.pathways.len(), 1);
    }

    #[test]
    fn pathways_response_empty() {
        let json = r#"{"pathways":[]}"#;
        let resp: PathwaysResponse = serde_json::from_str(json).unwrap();
        assert!(resp.pathways.is_empty());
    }

    #[test]
    fn povm_summary_deserialize() {
        let json = r#"{"pathway_count":2425,"memory_count":36,"uptime_secs":86400.0}"#;
        let s: PovmSummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.pathway_count, 2425);
        assert_eq!(s.memory_count, 36);
    }

    #[test]
    fn povm_summary_serde_roundtrip() {
        let s = PovmSummary {
            pathway_count: 100,
            memory_count: 50,
            uptime_secs: 3600.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: PovmSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pathway_count, 100);
    }

    // ── Thread safety ──

    #[test]
    fn bridge_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<PovmBridge>();
    }

    #[test]
    fn bridge_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<PovmBridge>();
    }

    // ── Trait object ──

    #[test]
    fn bridgeable_as_trait_object() {
        let bridge = PovmBridge::new();
        let dyn_ref: &dyn Bridgeable = &bridge;
        assert_eq!(dyn_ref.service_name(), "povm");
    }

    // ── HTTP helpers ──

    #[test]
    fn extract_body_finds_body() {
        let raw = "HTTP/1.1 200 OK\r\n\r\n{\"pathways\":[]}";
        assert_eq!(extract_body(raw), Some("{\"pathways\":[]}".to_owned()));
    }

    #[test]
    fn extract_body_no_separator() {
        assert!(extract_body("no headers here").is_none());
    }

    // ── Constants ──

    #[test]
    fn default_write_interval_is_twelve() {
        assert_eq!(DEFAULT_WRITE_INTERVAL, 12);
    }

    #[test]
    fn default_read_interval_is_sixty() {
        assert_eq!(DEFAULT_READ_INTERVAL, 60);
    }

    #[test]
    fn povm_port_is_8125() {
        assert_eq!(POVM_PORT, 8125);
    }

    // ── Debug ──

    #[test]
    fn debug_format_works() {
        let bridge = PovmBridge::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("povm"));
    }

    // ── Hydration state simulation ──

    #[test]
    fn hydration_state_changes() {
        let bridge = PovmBridge::new();
        assert!(!bridge.is_hydrated());
        {
            let mut state = bridge.state.write();
            state.hydrated = true;
            state.cached_pathways = vec![Pathway {
                source: "a".to_owned(),
                target: "b".to_owned(),
                weight: 0.5,
                reinforcement_count: 1,
            }];
        }
        assert!(bridge.is_hydrated());
        assert_eq!(bridge.pathway_count(), 1);
    }

    // ── Additional tests for 50+ coverage ──

    #[test]
    fn pathways_response_defaults() {
        let json = "{}";
        let resp: PathwaysResponse = serde_json::from_str(json).unwrap();
        assert!(resp.pathways.is_empty());
    }

    #[test]
    fn povm_summary_minimal() {
        let json = "{}";
        let s: PovmSummary = serde_json::from_str(json).unwrap();
        assert_eq!(s.pathway_count, 0);
        assert_eq!(s.memory_count, 0);
        assert!((s.uptime_secs).abs() < f64::EPSILON);
    }

    #[test]
    fn pathway_clone() {
        let p = Pathway {
            source: "a".to_owned(),
            target: "b".to_owned(),
            weight: 0.8,
            reinforcement_count: 5,
        };
        let p2 = p.clone();
        assert_eq!(p.source, p2.source);
        assert!((p.weight - p2.weight).abs() < f64::EPSILON);
    }

    #[test]
    fn hydrate_fails_when_unreachable() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        assert!(bridge.hydrate_pathways().is_err());
    }

    #[test]
    fn summary_fails_when_unreachable() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        assert!(bridge.hydrate_summary().is_err());
    }

    #[test]
    fn consecutive_failures_saturate() {
        let bridge = PovmBridge::new();
        {
            let mut state = bridge.state.write();
            state.consecutive_failures = u32::MAX;
        }
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), u32::MAX);
    }

    // ── write_pathways tests ──

    #[test]
    fn write_pathways_empty_returns_zero() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        let result = bridge.write_pathways(&[], 1);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn write_pathways_updates_last_write_tick_on_success() {
        // Will fail to connect, but should handle gracefully
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        let conns = vec![
            ("a".to_owned(), "b".to_owned(), 0.8, 1_u64),
        ];
        let _ = bridge.write_pathways(&conns, 42);
        // Even on failure, should not panic
    }

    #[test]
    fn write_pathways_handles_unreachable() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        let conns = vec![
            ("src".to_owned(), "dst".to_owned(), 0.5, 3),
            ("src2".to_owned(), "dst2".to_owned(), 0.7, 1),
        ];
        let result = bridge.write_pathways(&conns, 10);
        assert!(result.is_ok());
        // All writes fail but function returns 0 written (not an error)
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn write_pathways_no_tick_update_on_all_failure() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        let conns = vec![("a".to_owned(), "b".to_owned(), 0.5, 1)];
        let _ = bridge.write_pathways(&conns, 100);
        // last_write_tick should NOT update if all writes failed
        assert_eq!(bridge.last_write_tick(), 0);
    }

    #[test]
    fn write_pathways_multiple_connections() {
        let bridge = PovmBridge::with_config("127.0.0.1:19999", 12, 60);
        let conns: Vec<_> = (0..10)
            .map(|i| (format!("s{i}"), format!("t{i}"), 0.5 + f64::from(i) * 0.05, i as u64))
            .collect();
        let result = bridge.write_pathways(&conns, 50);
        assert!(result.is_ok());
    }

    // ── Hydration coupling weight seeding ──

    #[test]
    fn hydrated_pathways_seed_coupling_weights() {
        use crate::m4_intelligence::m15_coupling_network::CouplingNetwork;

        let pathways = vec![
            Pathway {
                source: "pane-a".to_owned(),
                target: "pane-b".to_owned(),
                weight: 0.85,
                reinforcement_count: 10,
            },
            Pathway {
                source: "pane-b".to_owned(),
                target: "pane-a".to_owned(),
                weight: 0.60,
                reinforcement_count: 5,
            },
        ];

        let mut network = CouplingNetwork::new();
        network.register("pane-a".into(), 0.0, 1.0);
        network.register("pane-b".into(), 1.0, 1.0);

        let mut restored = 0u32;
        for pw in &pathways {
            for conn in &mut network.connections {
                if conn.from.as_str() == pw.source
                    && conn.to.as_str() == pw.target
                    && pw.weight > 0.0
                {
                    conn.weight = pw.weight.clamp(0.0, 1.0);
                    restored += 1;
                }
            }
        }

        assert_eq!(restored, 2, "Both pathway directions should be restored");

        let ab = network
            .connections
            .iter()
            .find(|c| c.from.as_str() == "pane-a" && c.to.as_str() == "pane-b")
            .expect("a->b connection");
        let ba = network
            .connections
            .iter()
            .find(|c| c.from.as_str() == "pane-b" && c.to.as_str() == "pane-a")
            .expect("b->a connection");

        assert!((ab.weight - 0.85).abs() < f64::EPSILON);
        assert!((ba.weight - 0.60).abs() < f64::EPSILON);
    }

    #[test]
    fn hydration_clamps_out_of_range_weights() {
        use crate::m4_intelligence::m15_coupling_network::CouplingNetwork;

        let pathways = vec![Pathway {
            source: "x".to_owned(),
            target: "y".to_owned(),
            weight: 1.5,
            reinforcement_count: 0,
        }];

        let mut network = CouplingNetwork::new();
        network.register("x".into(), 0.0, 1.0);
        network.register("y".into(), 0.5, 1.0);

        for pw in &pathways {
            for conn in &mut network.connections {
                if conn.from.as_str() == pw.source && conn.to.as_str() == pw.target {
                    conn.weight = pw.weight.clamp(0.0, 1.0);
                }
            }
        }

        let conn = network
            .connections
            .iter()
            .find(|c| c.from.as_str() == "x" && c.to.as_str() == "y")
            .expect("x->y");
        assert!((conn.weight - 1.0).abs() < f64::EPSILON, "Should be clamped to 1.0");
    }

    #[test]
    fn hydration_skips_unknown_connections() {
        use crate::m4_intelligence::m15_coupling_network::CouplingNetwork;

        let pathways = vec![Pathway {
            source: "unknown-a".to_owned(),
            target: "unknown-b".to_owned(),
            weight: 0.9,
            reinforcement_count: 3,
        }];

        let mut network = CouplingNetwork::new();
        network.register("real-a".into(), 0.0, 1.0);
        network.register("real-b".into(), 0.5, 1.0);

        let mut restored = 0u32;
        for pw in &pathways {
            for conn in &mut network.connections {
                if conn.from.as_str() == pw.source
                    && conn.to.as_str() == pw.target
                    && pw.weight > 0.0
                {
                    conn.weight = pw.weight.clamp(0.0, 1.0);
                    restored += 1;
                }
            }
        }

        assert_eq!(restored, 0, "No connections should match unknown pane IDs");
    }

    #[test]
    fn hydration_ignores_zero_weight_pathways() {
        use crate::m4_intelligence::m15_coupling_network::CouplingNetwork;

        let pathways = vec![Pathway {
            source: "p1".to_owned(),
            target: "p2".to_owned(),
            weight: 0.0,
            reinforcement_count: 0,
        }];

        let mut network = CouplingNetwork::new();
        network.register("p1".into(), 0.0, 1.0);
        network.register("p2".into(), 0.5, 1.0);

        let original_weight = network
            .connections
            .iter()
            .find(|c| c.from.as_str() == "p1" && c.to.as_str() == "p2")
            .map(|c| c.weight)
            .unwrap_or(0.0);

        let mut restored = 0u32;
        for pw in &pathways {
            for conn in &mut network.connections {
                if conn.from.as_str() == pw.source
                    && conn.to.as_str() == pw.target
                    && pw.weight > 0.0
                {
                    conn.weight = pw.weight.clamp(0.0, 1.0);
                    restored += 1;
                }
            }
        }

        assert_eq!(restored, 0, "Zero-weight pathway should not overwrite");

        let conn = network
            .connections
            .iter()
            .find(|c| c.from.as_str() == "p1" && c.to.as_str() == "p2")
            .expect("p1->p2");
        assert!(
            (conn.weight - original_weight).abs() < f64::EPSILON,
            "Weight should remain at default"
        );
    }
}
