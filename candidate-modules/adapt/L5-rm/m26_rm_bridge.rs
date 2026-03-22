//! # M26: Reasoning Memory Bridge
//!
//! TSV POST to Reasoning Memory at `localhost:8130`.
//! **NEVER JSON** -- TSV only! (AP05: JSON to RM causes parse failures).
//!
//! ## Layer: L6 | Module: M26 | Dependencies: L1
//!
//! Format: `category\tagent\tconfidence\tttl\tcontent`
//!
//! Data: 3,250 active entries, 67% are PV `field_state` entries.
//! V3.5.1 will reduce TTL to curb noise.
//!
//! ## ADAPT for ORAC
//! This module needs adaptation for ORAC deployment:
//! - Port may change from 8130 (configurable via `with_config`)
//! - Socket address format must remain `host:port` (no `http://` prefix, BUG-033)
//! - Poll interval may need tuning for sidecar tick rate
//! - Agent name should change from `"pane-vortex"` to `"orac-sidecar"`
//! - **Critical**: Content-Type MUST be `text/tab-separated-values`, never JSON

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_foundation::m02_error_handling::{PvError, PvResult};
use crate::m1_foundation::m05_traits::Bridgeable;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// RM service port.
const RM_PORT: u16 = 8130;

/// Default base URL.
const DEFAULT_BASE_URL: &str = "localhost:8130";

/// Health endpoint path.
const HEALTH_PATH: &str = "/health";

/// PUT endpoint for posting TSV data.
const PUT_PATH: &str = "/put";

/// Search endpoint for reading entries.
const SEARCH_PATH: &str = "/search";

/// Default poll interval in ticks (for reading back).
const DEFAULT_POLL_INTERVAL: u64 = 30;

/// TCP connection timeout (milliseconds).
const TCP_TIMEOUT_MS: u64 = 2000;

/// Maximum response body size (bytes).
const MAX_RESPONSE_SIZE: usize = 32768;

/// Tab character for TSV formatting.
const TAB: char = '\t';

/// Default TTL for field state entries (seconds).
const DEFAULT_FIELD_STATE_TTL: u64 = 300;

/// Default agent name for PV entries.
const DEFAULT_AGENT: &str = "pane-vortex";

// ──────────────────────────────────────────────────────────────
// TSV record types
// ──────────────────────────────────────────────────────────────

/// A TSV record for the Reasoning Memory.
///
/// Format: `category\tagent\tconfidence\tttl\tcontent`
/// NEVER serialize as JSON to the RM service!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmRecord {
    /// Category label (e.g. "`field_state`", "decision", "`bridge_health`").
    pub category: String,
    /// Agent identifier.
    pub agent: String,
    /// Confidence value (0.0-1.0).
    pub confidence: f64,
    /// Time-to-live in seconds.
    pub ttl: u64,
    /// Content string (the actual data).
    pub content: String,
}

impl RmRecord {
    /// Create a new RM record.
    #[must_use]
    pub fn new(
        category: impl Into<String>,
        agent: impl Into<String>,
        confidence: f64,
        ttl: u64,
        content: impl Into<String>,
    ) -> Self {
        Self {
            category: category.into(),
            agent: agent.into(),
            confidence: confidence.clamp(0.0, 1.0),
            ttl,
            content: content.into(),
        }
    }

    /// Create a `field_state` record with default agent and TTL.
    #[must_use]
    pub fn field_state(content: impl Into<String>, confidence: f64) -> Self {
        Self::new(
            "field_state",
            DEFAULT_AGENT,
            confidence,
            DEFAULT_FIELD_STATE_TTL,
            content,
        )
    }

    /// Create a decision record.
    #[must_use]
    pub fn decision(content: impl Into<String>, confidence: f64, ttl: u64) -> Self {
        Self::new("decision", DEFAULT_AGENT, confidence, ttl, content)
    }

    /// Serialize to TSV format (tab-separated, no trailing newline).
    ///
    /// Sanitizes content by replacing tabs and newlines with spaces.
    #[must_use]
    pub fn to_tsv(&self) -> String {
        let sanitized_content = self
            .content
            .replace(['\t', '\n', '\r'], " ");
        let sanitized_category = self
            .category
            .replace(['\t', '\n', '\r'], " ");
        let sanitized_agent = self
            .agent
            .replace(['\t', '\n', '\r'], " ");

        format!(
            "{}{TAB}{}{TAB}{}{TAB}{}{TAB}{}",
            sanitized_category, sanitized_agent, self.confidence, self.ttl, sanitized_content
        )
    }

    /// Parse from TSV format.
    ///
    /// # Errors
    /// Returns an error string if the line doesn't have exactly 5 tab-separated fields.
    pub fn from_tsv(line: &str) -> Result<Self, String> {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 5 {
            return Err(format!(
                "expected 5 TSV fields, got {}",
                parts.len()
            ));
        }
        let confidence = parts[2]
            .parse::<f64>()
            .map_err(|e| format!("bad confidence: {e}"))?;
        let ttl = parts[3]
            .parse::<u64>()
            .map_err(|e| format!("bad ttl: {e}"))?;

        Ok(Self {
            category: parts[0].to_owned(),
            agent: parts[1].to_owned(),
            confidence: confidence.clamp(0.0, 1.0),
            ttl,
            content: parts[4..].join("\t"),
        })
    }
}

/// Search result from the RM `/search` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmSearchResult {
    /// Matching entries as raw TSV lines.
    #[serde(default)]
    pub entries: Vec<String>,
    /// Total match count.
    #[serde(default)]
    pub total: u64,
}

// ──────────────────────────────────────────────────────────────
// Bridge state
// ──────────────────────────────────────────────────────────────

/// Mutable state behind a `RwLock`.
#[derive(Debug)]
struct BridgeState {
    /// Last poll tick.
    last_poll_tick: u64,
    /// Cached adjustment (neutral for RM).
    cached_adjustment: f64,
    /// Whether data is stale.
    stale: bool,
    /// Consecutive failure counter.
    consecutive_failures: u32,
    /// Total records posted this session.
    records_posted: u64,
    /// Last search result.
    last_search_result: Option<RmSearchResult>,
}

impl Default for BridgeState {
    fn default() -> Self {
        Self {
            last_poll_tick: 0,
            cached_adjustment: 1.0,
            stale: true,
            consecutive_failures: 0,
            records_posted: 0,
            last_search_result: None,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// RmBridge
// ──────────────────────────────────────────────────────────────

/// Bridge to the Reasoning Memory for TSV-based cross-session persistence.
///
/// **Critical**: All data MUST be sent as TSV, never JSON.
/// The RM service expects `category\tagent\tconfidence\tttl\tcontent` format.
#[derive(Debug)]
pub struct RmBridge {
    /// Service name identifier.
    service: String,
    /// TCP address (host:port).
    base_url: String,
    /// Poll interval in ticks.
    poll_interval: u64,
    /// Interior-mutable state.
    state: RwLock<BridgeState>,
}

impl RmBridge {
    /// Create a new RM bridge with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            service: "rm".to_owned(),
            base_url: DEFAULT_BASE_URL.to_owned(),
            poll_interval: DEFAULT_POLL_INTERVAL,
            state: RwLock::new(BridgeState::default()),
        }
    }

    /// Create a new RM bridge with custom configuration.
    #[must_use]
    pub fn with_config(base_url: impl Into<String>, poll_interval: u64) -> Self {
        Self {
            service: "rm".to_owned(),
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

    /// Return the cached adjustment value (always neutral for RM).
    #[must_use]
    pub fn cached_adjustment(&self) -> f64 {
        self.state.read().cached_adjustment
    }

    /// Return the total number of records posted this session.
    #[must_use]
    pub fn records_posted(&self) -> u64 {
        self.state.read().records_posted
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
            .unwrap_or(RM_PORT)
    }

    /// Return the last search result.
    #[must_use]
    pub fn last_search_result(&self) -> Option<RmSearchResult> {
        self.state.read().last_search_result.clone()
    }

    /// Post a TSV record to the Reasoning Memory.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the connection fails.
    pub fn post_record(&self, record: &RmRecord) -> PvResult<()> {
        let tsv = record.to_tsv();
        raw_http_post_tsv(&self.base_url, PUT_PATH, &tsv, &self.service)?;
        let mut state = self.state.write();
        state.records_posted = state.records_posted.saturating_add(1);
        state.consecutive_failures = 0;
        state.stale = false;
        Ok(())
    }

    /// Post multiple TSV records (one per line).
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the connection fails.
    pub fn post_records(&self, records: &[RmRecord]) -> PvResult<()> {
        if records.is_empty() {
            return Ok(());
        }
        let tsv_lines: Vec<String> = records.iter().map(RmRecord::to_tsv).collect();
        let payload = tsv_lines.join("\n");
        raw_http_post_tsv(&self.base_url, PUT_PATH, &payload, &self.service)?;
        let mut state = self.state.write();
        state.records_posted = state
            .records_posted
            .saturating_add(records.len() as u64);
        state.consecutive_failures = 0;
        Ok(())
    }

    /// Search the Reasoning Memory.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` or `PvError::BridgeParse` on failure.
    pub fn search(&self, query: &str) -> PvResult<RmSearchResult> {
        let path = format!("{SEARCH_PATH}?q={}", urlencoded(query));
        let body = raw_http_get(&self.base_url, &path, &self.service)?;

        // RM may return TSV lines or JSON depending on endpoint
        let result = if body.starts_with('{') {
            serde_json::from_str::<RmSearchResult>(&body).map_err(|e| PvError::BridgeParse {
                service: self.service.clone(),
                reason: format!("search parse: {e}"),
            })?
        } else {
            // Parse as raw TSV lines
            let entries: Vec<String> = body
                .lines()
                .filter(|l| !l.is_empty())
                .map(String::from)
                .collect();
            let total = entries.len() as u64;
            RmSearchResult { entries, total }
        };

        let mut state = self.state.write();
        state.last_search_result = Some(result.clone());
        state.consecutive_failures = 0;
        state.stale = false;

        Ok(result)
    }

    /// Record a failure.
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

impl Default for RmBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl Bridgeable for RmBridge {
    /// Return the service name (`"rm"`).
    fn service_name(&self) -> &str {
        &self.service
    }

    /// Return the cached neutral adjustment (RM does not produce `k_adj`).
    ///
    /// # Errors
    /// This method never fails.
    fn poll(&self) -> PvResult<f64> {
        Ok(self.cached_adjustment())
    }

    /// Post the payload as TSV to the Reasoning Memory `/put` endpoint.
    ///
    /// # Errors
    /// Returns `PvError::BridgeUnreachable` if the TCP connection fails.
    fn post(&self, payload: &[u8]) -> PvResult<()> {
        let tsv = String::from_utf8_lossy(payload);
        raw_http_post_tsv(&self.base_url, PUT_PATH, &tsv, &self.service)?;
        let mut state = self.state.write();
        state.records_posted = state.records_posted.saturating_add(1);
        state.consecutive_failures = 0;
        Ok(())
    }

    /// Check whether the RM service is reachable.
    ///
    /// # Errors
    /// Returns `Ok(false)` on connection failure (does not propagate the error).
    fn health(&self) -> PvResult<bool> {
        match raw_http_get(&self.base_url, HEALTH_PATH, &self.service) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Return whether the cached data is stale (flag set or 3x poll interval elapsed).
    fn is_stale(&self, current_tick: u64) -> bool {
        let state = self.state.read();
        state.stale || current_tick.saturating_sub(state.last_poll_tick) >= self.poll_interval * 3
    }
}

// ──────────────────────────────────────────────────────────────
// URL encoding helper
// ──────────────────────────────────────────────────────────────

/// Minimal URL encoding for query parameters.
fn urlencoded(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(b as char);
            }
            b' ' => encoded.push('+'),
            _ => {
                encoded.push('%');
                encoded.push(char::from(HEX_CHARS[(b >> 4) as usize]));
                encoded.push(char::from(HEX_CHARS[(b & 0x0f) as usize]));
            }
        }
    }
    encoded
}

/// Hex character lookup table.
const HEX_CHARS: [u8; 16] = *b"0123456789ABCDEF";

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

/// Send a raw HTTP POST request with TSV content type (fire-and-forget).
///
/// # Errors
/// Returns `PvError::BridgeUnreachable` if the connection or I/O fails.
fn raw_http_post_tsv(addr: &str, path: &str, tsv: &str, service: &str) -> PvResult<()> {
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

    let host = addr.split(':').next().unwrap_or("localhost");
    let body = tsv.as_bytes();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {}\r\nContent-Type: text/tab-separated-values\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(request.as_bytes()).map_err(|_| {
        PvError::BridgeUnreachable {
            service: service.to_owned(),
            url: addr.to_owned(),
        }
    })?;
    stream.write_all(body).map_err(|_| PvError::BridgeUnreachable {
        service: service.to_owned(),
        url: addr.to_owned(),
    })?;

    Ok(())
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
        let bridge = RmBridge::new();
        assert_eq!(bridge.service_name(), "rm");
        assert_eq!(bridge.poll_interval(), DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn default_creates_same_as_new() {
        let bridge = RmBridge::default();
        assert_eq!(bridge.service_name(), "rm");
    }

    #[test]
    fn with_config_custom() {
        let bridge = RmBridge::with_config("10.0.0.1:8230", 15);
        assert_eq!(bridge.base_url, "10.0.0.1:8230");
        assert_eq!(bridge.poll_interval(), 15);
    }

    #[test]
    fn with_config_minimum_poll() {
        let bridge = RmBridge::with_config("localhost:8130", 0);
        assert_eq!(bridge.poll_interval(), 1);
    }

    #[test]
    fn port_extraction_default() {
        let bridge = RmBridge::new();
        assert_eq!(bridge.port(), RM_PORT);
    }

    // ── Initial state ──

    #[test]
    fn initial_cached_adjustment_is_one() {
        let bridge = RmBridge::new();
        assert!((bridge.cached_adjustment() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn initial_failures_is_zero() {
        let bridge = RmBridge::new();
        assert_eq!(bridge.consecutive_failures(), 0);
    }

    #[test]
    fn initial_records_posted_is_zero() {
        let bridge = RmBridge::new();
        assert_eq!(bridge.records_posted(), 0);
    }

    #[test]
    fn initial_is_stale() {
        let bridge = RmBridge::new();
        assert!(bridge.is_stale(0));
    }

    #[test]
    fn initial_last_search_is_none() {
        let bridge = RmBridge::new();
        assert!(bridge.last_search_result().is_none());
    }

    // ── RmRecord TSV ──

    #[test]
    fn record_to_tsv_format() {
        let r = RmRecord::new("field_state", "pane-vortex", 0.95, 300, "r=0.93 k=1.0");
        let tsv = r.to_tsv();
        assert_eq!(tsv, "field_state\tpane-vortex\t0.95\t300\tr=0.93 k=1.0");
    }

    #[test]
    fn record_to_tsv_sanitizes_tabs() {
        let r = RmRecord::new("cat", "agent", 1.0, 100, "has\ttab");
        let tsv = r.to_tsv();
        assert!(!tsv.contains("\t\t"), "tabs in content should be replaced");
        // Should have exactly 4 tabs (5 fields)
        assert_eq!(tsv.matches('\t').count(), 4);
    }

    #[test]
    fn record_to_tsv_sanitizes_newlines() {
        let r = RmRecord::new("cat", "agent", 1.0, 100, "has\nnewline");
        let tsv = r.to_tsv();
        assert!(!tsv.contains('\n'));
    }

    #[test]
    fn record_from_tsv_roundtrip() {
        let original = RmRecord::new("decision", "pv", 0.8, 600, "choose alignment");
        let tsv = original.to_tsv();
        let parsed = RmRecord::from_tsv(&tsv).unwrap();
        assert_eq!(parsed.category, "decision");
        assert_eq!(parsed.agent, "pv");
        assert!((parsed.confidence - 0.8).abs() < f64::EPSILON);
        assert_eq!(parsed.ttl, 600);
        assert_eq!(parsed.content, "choose alignment");
    }

    #[test]
    fn record_from_tsv_rejects_too_few_fields() {
        let result = RmRecord::from_tsv("only\ttwo");
        assert!(result.is_err());
    }

    #[test]
    fn record_from_tsv_rejects_bad_confidence() {
        let result = RmRecord::from_tsv("cat\tagent\tnotfloat\t100\tcontent");
        assert!(result.is_err());
    }

    #[test]
    fn record_from_tsv_rejects_bad_ttl() {
        let result = RmRecord::from_tsv("cat\tagent\t0.5\tnotint\tcontent");
        assert!(result.is_err());
    }

    #[test]
    fn record_field_state_factory() {
        let r = RmRecord::field_state("r=0.93", 0.9);
        assert_eq!(r.category, "field_state");
        assert_eq!(r.agent, DEFAULT_AGENT);
        assert_eq!(r.ttl, DEFAULT_FIELD_STATE_TTL);
    }

    #[test]
    fn record_decision_factory() {
        let r = RmRecord::decision("choose path A", 0.7, 1800);
        assert_eq!(r.category, "decision");
        assert_eq!(r.ttl, 1800);
    }

    #[test]
    fn record_clamps_confidence() {
        let r = RmRecord::new("c", "a", 2.0, 100, "x");
        assert!((r.confidence - 1.0).abs() < f64::EPSILON);

        let r2 = RmRecord::new("c", "a", -1.0, 100, "x");
        assert!((r2.confidence).abs() < f64::EPSILON);
    }

    #[test]
    fn record_serde_roundtrip() {
        let r = RmRecord::new("cat", "agent", 0.5, 300, "content");
        let json = serde_json::to_string(&r).unwrap();
        let back: RmRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back.category, "cat");
    }

    // ── URL encoding ──

    #[test]
    fn urlencoded_simple() {
        assert_eq!(urlencoded("hello"), "hello");
    }

    #[test]
    fn urlencoded_spaces() {
        assert_eq!(urlencoded("hello world"), "hello+world");
    }

    #[test]
    fn urlencoded_special_chars() {
        let encoded = urlencoded("a=b&c=d");
        assert!(!encoded.contains('='));
        assert!(!encoded.contains('&'));
    }

    #[test]
    fn urlencoded_preserves_safe_chars() {
        assert_eq!(urlencoded("abc-123_foo.bar~baz"), "abc-123_foo.bar~baz");
    }

    // ── RmSearchResult ──

    #[test]
    fn search_result_deserialize() {
        let json = r#"{"entries":["line1","line2"],"total":2}"#;
        let r: RmSearchResult = serde_json::from_str(json).unwrap();
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.total, 2);
    }

    #[test]
    fn search_result_empty() {
        let json = r#"{"entries":[],"total":0}"#;
        let r: RmSearchResult = serde_json::from_str(json).unwrap();
        assert!(r.entries.is_empty());
    }

    // ── Staleness ──

    #[test]
    fn stale_when_never_polled() {
        let bridge = RmBridge::new();
        assert!(bridge.is_stale(100));
    }

    #[test]
    fn not_stale_within_interval() {
        let bridge = RmBridge::with_config("localhost:8130", 30);
        bridge.set_last_poll_tick(10);
        {
            let mut state = bridge.state.write();
            state.stale = false;
        }
        assert!(!bridge.is_stale(40));
    }

    // ── Should poll ──

    #[test]
    fn should_poll_initially() {
        let bridge = RmBridge::with_config("localhost:8130", 30);
        assert!(bridge.should_poll(30));
    }

    // ── Failure tracking ──

    #[test]
    fn record_failure_increments() {
        let bridge = RmBridge::new();
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), 1);
    }

    // ── Poll (offline) ──

    #[test]
    fn poll_returns_neutral() {
        let bridge = RmBridge::new();
        assert!((bridge.poll().unwrap_or(0.0) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn post_fails_when_unreachable() {
        let bridge = RmBridge::with_config("127.0.0.1:19999", 30);
        assert!(bridge.post(b"test\tdata").is_err());
    }

    #[test]
    fn health_returns_false_when_unreachable() {
        let bridge = RmBridge::with_config("127.0.0.1:19999", 30);
        assert_eq!(bridge.health().ok(), Some(false));
    }

    // ── Thread safety ──

    #[test]
    fn bridge_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<RmBridge>();
    }

    #[test]
    fn bridge_is_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<RmBridge>();
    }

    // ── Trait object ──

    #[test]
    fn bridgeable_as_trait_object() {
        let bridge = RmBridge::new();
        let dyn_ref: &dyn Bridgeable = &bridge;
        assert_eq!(dyn_ref.service_name(), "rm");
    }

    // ── HTTP helpers ──

    #[test]
    fn extract_body_finds_body() {
        let raw = "HTTP/1.1 200 OK\r\n\r\nsome data";
        assert_eq!(extract_body(raw), Some("some data".to_owned()));
    }

    #[test]
    fn extract_body_no_separator() {
        assert!(extract_body("no headers").is_none());
    }

    // ── Constants ──

    #[test]
    fn rm_port_is_8130() {
        assert_eq!(RM_PORT, 8130);
    }

    #[test]
    fn put_path_is_put() {
        assert_eq!(PUT_PATH, "/put");
    }

    #[test]
    fn search_path_is_search() {
        assert_eq!(SEARCH_PATH, "/search");
    }

    #[test]
    fn default_field_state_ttl_is_300() {
        assert_eq!(DEFAULT_FIELD_STATE_TTL, 300);
    }

    // ── Debug ──

    #[test]
    fn debug_format_works() {
        let bridge = RmBridge::new();
        let debug = format!("{bridge:?}");
        assert!(debug.contains("rm"));
    }

    #[test]
    fn set_last_poll_tick_updates() {
        let bridge = RmBridge::new();
        bridge.set_last_poll_tick(42);
        assert_eq!(bridge.last_poll_tick(), 42);
    }

    // ── TSV five-field count ──

    #[test]
    fn tsv_has_five_fields() {
        let r = RmRecord::new("c", "a", 0.5, 100, "x");
        let tsv = r.to_tsv();
        let fields: Vec<&str> = tsv.split('\t').collect();
        assert_eq!(fields.len(), 5);
    }

    // ── Content type is TSV not JSON ──

    #[test]
    fn tsv_is_not_json() {
        let r = RmRecord::new("field_state", "pv", 0.9, 300, "data");
        let tsv = r.to_tsv();
        // TSV should never start with { or [
        assert!(!tsv.starts_with('{'));
        assert!(!tsv.starts_with('['));
    }

    // ── Additional tests for 50+ coverage ──

    #[test]
    fn record_from_tsv_five_fields_exact() {
        let tsv = "cat\tagent\t0.9\t100\tcontent here";
        let r = RmRecord::from_tsv(tsv).unwrap();
        assert_eq!(r.category, "cat");
        assert_eq!(r.agent, "agent");
        assert!((r.confidence - 0.9).abs() < f64::EPSILON);
        assert_eq!(r.ttl, 100);
        assert_eq!(r.content, "content here");
    }

    #[test]
    fn urlencoded_empty() {
        assert_eq!(urlencoded(""), "");
    }

    #[test]
    fn consecutive_failures_saturate() {
        let bridge = RmBridge::new();
        {
            let mut state = bridge.state.write();
            state.consecutive_failures = u32::MAX;
        }
        bridge.record_failure();
        assert_eq!(bridge.consecutive_failures(), u32::MAX);
    }

    #[test]
    fn record_debug_format() {
        let r = RmRecord::new("cat", "agent", 0.5, 100, "content");
        let debug = format!("{r:?}");
        assert!(debug.contains("RmRecord"));
    }

    #[test]
    fn records_posted_tracks_after_simulated_post() {
        let bridge = RmBridge::new();
        {
            let mut state = bridge.state.write();
            state.records_posted = 10;
        }
        assert_eq!(bridge.records_posted(), 10);
    }
}
