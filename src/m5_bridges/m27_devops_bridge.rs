//! # M27: DevOps Engine Bridge
//!
//! Lightweight polling bridge to DevOps Engine (port 8081).
//! Extracts pipeline health metrics for ORAC fitness tensor D3 (`TaskThroughput`).
//!
//! ## Layer: L5 (Bridges)
//! ## Service: DevOps Engine (:8081)
//! ## Protocol: HTTP (`GET /health`)

use parking_lot::RwLock;

use super::http_helpers;
use crate::m1_core::m02_error_handling::PvResult;

/// Default base URL for the DevOps Engine.
const DEFAULT_BASE_URL: &str = "127.0.0.1:8081";

/// DevOps Engine bridge — polls health for pipeline metrics.
pub struct DevopsBridge {
    /// Base URL (host:port, no `http://` prefix).
    base_url: String,
    /// Last polled health status (1.0 = healthy, 0.0 = unreachable).
    last_health: RwLock<f64>,
    /// Consecutive poll failures.
    failures: RwLock<u32>,
    /// Maximum consecutive failures before marking stale.
    max_failures: u32,
}

impl DevopsBridge {
    /// Creates a new `DevopsBridge` with the default URL.
    #[must_use]
    pub fn new() -> Self {
        Self::with_url(DEFAULT_BASE_URL)
    }

    /// Creates a new `DevopsBridge` with a custom URL.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            base_url: url.to_owned(),
            last_health: RwLock::new(0.0),
            failures: RwLock::new(0),
            max_failures: 5,
        }
    }

    /// Poll the DevOps Engine health endpoint.
    ///
    /// Returns the health score (1.0 = healthy, 0.0 = unreachable).
    ///
    /// # Errors
    ///
    /// Returns [`PvError`] if the HTTP request fails (connection refused, timeout).
    pub fn poll_health(&self) -> PvResult<f64> {
        match http_helpers::raw_http_get(&self.base_url, "/health", "devops") {
            Ok(_body) => {
                *self.last_health.write() = 1.0;
                *self.failures.write() = 0;
                Ok(1.0)
            }
            Err(e) => {
                let mut fails = self.failures.write();
                *fails = fails.saturating_add(1);
                drop(fails);
                if *self.failures.read() >= self.max_failures {
                    *self.last_health.write() = 0.0;
                }
                Err(e)
            }
        }
    }

    /// Last known health score without polling.
    #[must_use]
    pub fn last_health(&self) -> f64 {
        *self.last_health.read()
    }

    /// Whether the bridge considers the service stale.
    #[must_use]
    pub fn is_stale(&self) -> bool {
        *self.failures.read() >= self.max_failures
    }
}

impl Default for DevopsBridge {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devops_bridge_default_creates() {
        let bridge = DevopsBridge::new();
        assert_eq!(bridge.base_url, "127.0.0.1:8081");
        assert!((bridge.last_health() - 0.0).abs() < f64::EPSILON);
        assert!(!bridge.is_stale());
    }

    #[test]
    fn devops_bridge_stale_after_max_failures() {
        let bridge = DevopsBridge::with_url("127.0.0.1:19999");
        for _ in 0..5 {
            let _ = bridge.poll_health();
        }
        assert!(bridge.is_stale());
    }
}
