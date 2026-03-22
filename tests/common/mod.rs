//! Shared test utilities for ORAC sidecar integration tests.
//!
//! Provides `TestHarness` for spinning up an in-process ORAC instance
//! with controlled configuration, mock bridges, and assertion helpers.

/// Placeholder test harness for integration tests.
///
/// Future: will manage server lifecycle, provide HTTP client,
/// seed blackboard state, and tear down cleanly.
#[allow(dead_code)]
pub struct TestHarness;

#[allow(dead_code)]
impl TestHarness {
    /// Create a new test harness with default config.
    pub fn new() -> Self {
        Self
    }
}

#[test]
fn scaffold() {
    assert!(true);
}
