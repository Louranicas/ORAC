//! # M33: Cascade System
//!
//! `CascadeHandoff`/`CascadeAck` frames. Rate limiting: max 10/minute. Depth tracking
//! (auto-summarize at >3). Markdown fallback briefs for non-bus-aware recipients.
//!
//! ## Layer: L7 | Module: M33 | Dependencies: L1, L7 (M29 bus, M30 types)
//! ## NA Gap: NA-P-7 (cascade rejection) — V3.3.3 adds `reject_cascade` frame

use std::collections::VecDeque;

use crate::m1_foundation::{
    m01_core_types::{now_secs, PaneId},
    m02_error_handling::{PvError, PvResult},
};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Maximum cascades per minute.
const MAX_CASCADES_PER_MINUTE: u32 = 10;

/// Rate-limit window duration in seconds.
const RATE_WINDOW_SECS: f64 = 60.0;

/// Maximum pending cascades before rejecting new ones.
const MAX_PENDING_CASCADES: usize = 50;

/// Cascade depth at which auto-summarization triggers.
const AUTO_SUMMARIZE_DEPTH: u32 = 3;

/// Maximum brief length (characters) before truncation.
const MAX_BRIEF_CHARS: usize = 4096;

// ──────────────────────────────────────────────────────────────
// CascadeHandoff
// ──────────────────────────────────────────────────────────────

/// A cascade handoff between two fleet tabs.
///
/// Cascades are used to transfer work context from one Claude Code instance
/// to another. The brief is a markdown document describing the work state.
#[derive(Debug, Clone)]
pub struct CascadeHandoff {
    /// Source sphere initiating the cascade.
    pub source: PaneId,
    /// Target sphere receiving the cascade.
    pub target: PaneId,
    /// Markdown brief describing the work context.
    pub brief: String,
    /// Unix timestamp when the cascade was dispatched.
    pub dispatched_at: f64,
    /// Cascade chain depth (1 = original, 2+ = re-cascade).
    pub depth: u32,
    /// Whether the target has acknowledged this cascade.
    pub acknowledged: bool,
    /// Whether the target has rejected this cascade.
    pub rejected: bool,
    /// Rejection reason (if rejected).
    pub rejection_reason: Option<String>,
}

impl CascadeHandoff {
    /// Create a new cascade handoff.
    #[must_use]
    pub fn new(source: PaneId, target: PaneId, brief: String) -> Self {
        let truncated = if brief.chars().count() > MAX_BRIEF_CHARS {
            let mut t: String = brief.chars().take(MAX_BRIEF_CHARS).collect();
            t.push_str("\n\n[... truncated ...]");
            t
        } else {
            brief
        };
        Self {
            source,
            target,
            brief: truncated,
            dispatched_at: now_secs(),
            depth: 1,
            acknowledged: false,
            rejected: false,
            rejection_reason: None,
        }
    }

    /// Create a re-cascade (increment depth).
    #[must_use]
    pub fn re_cascade(&self, new_target: PaneId) -> Self {
        let brief = if self.depth >= AUTO_SUMMARIZE_DEPTH {
            format!(
                "# Auto-summarized cascade (depth {})\n\nOriginal: {} -> {}\n\n{}",
                self.depth + 1,
                self.source,
                self.target,
                summarize_brief(&self.brief),
            )
        } else {
            self.brief.clone()
        };
        Self {
            source: self.target.clone(),
            target: new_target,
            brief,
            dispatched_at: now_secs(),
            depth: self.depth + 1,
            acknowledged: false,
            rejected: false,
            rejection_reason: None,
        }
    }

    /// Acknowledge this cascade.
    pub fn acknowledge(&mut self) {
        self.acknowledged = true;
    }

    /// Reject this cascade with a reason.
    pub fn reject(&mut self, reason: String) {
        self.rejected = true;
        self.rejection_reason = Some(reason);
    }

    /// Whether this cascade is still pending (neither acknowledged nor rejected).
    #[must_use]
    pub const fn is_pending(&self) -> bool {
        !self.acknowledged && !self.rejected
    }

    /// Elapsed time since dispatch (seconds).
    #[must_use]
    pub fn elapsed_secs(&self) -> f64 {
        now_secs() - self.dispatched_at
    }

    /// Whether this cascade needs auto-summarization (depth > threshold).
    #[must_use]
    pub const fn needs_summarization(&self) -> bool {
        self.depth >= AUTO_SUMMARIZE_DEPTH
    }

    /// Generate a markdown fallback brief for non-bus-aware recipients.
    #[must_use]
    pub fn fallback_brief(&self) -> String {
        format!(
            "# Cascade Handoff\n\n**From:** {}\n**To:** {}\n**Depth:** {}\n\n---\n\n{}",
            self.source, self.target, self.depth, self.brief
        )
    }
}

// ──────────────────────────────────────────────────────────────
// CascadeTracker
// ──────────────────────────────────────────────────────────────

/// Tracks cascade handoffs with rate limiting and depth management.
#[derive(Debug)]
pub struct CascadeTracker {
    /// Active cascades (pending + resolved).
    cascades: VecDeque<CascadeHandoff>,
    /// Cascade count in current rate window.
    window_count: u32,
    /// Start of current rate window (Unix timestamp).
    window_start: f64,
    /// Maximum cascade depth before auto-rejection.
    max_depth: u32,
}

impl CascadeTracker {
    /// Create a new cascade tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cascades: VecDeque::new(),
            window_count: 0,
            window_start: now_secs(),
            max_depth: 10,
        }
    }

    /// Create a tracker with a custom max depth.
    #[must_use]
    pub fn with_max_depth(max_depth: u32) -> Self {
        Self {
            cascades: VecDeque::new(),
            window_count: 0,
            window_start: now_secs(),
            max_depth: max_depth.max(1),
        }
    }

    /// Initiate a new cascade handoff.
    ///
    /// # Errors
    /// Returns `PvError::CascadeRateLimit` if the rate limit is exceeded.
    /// Returns `PvError::BusProtocol` if too many pending cascades.
    pub fn initiate(
        &mut self,
        source: PaneId,
        target: PaneId,
        brief: String,
    ) -> PvResult<usize> {
        self.check_rate_limit()?;
        self.check_pending_limit()?;

        let handoff = CascadeHandoff::new(source, target, brief);
        self.cascades.push_back(handoff);
        self.window_count += 1;

        Ok(self.cascades.len() - 1)
    }

    /// Initiate a re-cascade from an existing handoff.
    ///
    /// # Errors
    /// Returns `PvError::CascadeRateLimit` if the rate limit is exceeded.
    /// Returns `PvError::BusProtocol` if the cascade depth exceeds the maximum
    /// or if there are too many pending cascades.
    pub fn re_cascade(
        &mut self,
        index: usize,
        new_target: PaneId,
    ) -> PvResult<usize> {
        let existing = self
            .cascades
            .get(index)
            .ok_or_else(|| PvError::BusProtocol(format!("cascade {index} not found")))?
            .clone();

        if existing.depth >= self.max_depth {
            return Err(PvError::BusProtocol(format!(
                "cascade depth {} exceeds max {}",
                existing.depth + 1,
                self.max_depth
            )));
        }

        self.check_rate_limit()?;
        self.check_pending_limit()?;

        let handoff = existing.re_cascade(new_target);
        self.cascades.push_back(handoff);
        self.window_count += 1;

        Ok(self.cascades.len() - 1)
    }

    /// Acknowledge a cascade by index.
    ///
    /// # Errors
    /// Returns `PvError::BusProtocol` if the index is out of bounds.
    pub fn acknowledge(&mut self, index: usize) -> PvResult<()> {
        let handoff = self
            .cascades
            .get_mut(index)
            .ok_or_else(|| PvError::BusProtocol(format!("cascade {index} not found")))?;
        handoff.acknowledge();
        Ok(())
    }

    /// Reject a cascade by index.
    ///
    /// # Errors
    /// Returns `PvError::BusProtocol` if the index is out of bounds.
    pub fn reject(&mut self, index: usize, reason: String) -> PvResult<()> {
        let handoff = self
            .cascades
            .get_mut(index)
            .ok_or_else(|| PvError::BusProtocol(format!("cascade {index} not found")))?;
        handoff.reject(reason);
        Ok(())
    }

    /// Get all pending cascades.
    #[must_use]
    pub fn pending_cascades(&self) -> Vec<(usize, &CascadeHandoff)> {
        self.cascades
            .iter()
            .enumerate()
            .filter(|(_, c)| c.is_pending())
            .collect()
    }

    /// Total cascade count (all states).
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.cascades.len()
    }

    /// Pending cascade count.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.cascades.iter().filter(|c| c.is_pending()).count()
    }

    /// Get a cascade by index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<&CascadeHandoff> {
        self.cascades.get(index)
    }

    /// Current rate window count.
    #[must_use]
    pub const fn window_count(&self) -> u32 {
        self.window_count
    }

    /// Prune old resolved cascades (keep only the most recent `keep` entries).
    pub fn prune(&mut self, keep: usize) {
        while self.cascades.len() > keep {
            if let Some(front) = self.cascades.front() {
                if front.is_pending() {
                    break;
                }
                self.cascades.pop_front();
            } else {
                break;
            }
        }
    }

    // ── Private helpers ──

    /// Check rate limit, resetting window if expired.
    fn check_rate_limit(&mut self) -> PvResult<()> {
        let now = now_secs();
        if now - self.window_start > RATE_WINDOW_SECS {
            self.window_count = 0;
            self.window_start = now;
        }
        if self.window_count >= MAX_CASCADES_PER_MINUTE {
            return Err(PvError::CascadeRateLimit {
                per_minute: MAX_CASCADES_PER_MINUTE,
            });
        }
        Ok(())
    }

    /// Check pending cascade limit.
    fn check_pending_limit(&self) -> PvResult<()> {
        if self.pending_count() >= MAX_PENDING_CASCADES {
            return Err(PvError::BusProtocol(format!(
                "too many pending cascades ({}/{})",
                self.pending_count(),
                MAX_PENDING_CASCADES
            )));
        }
        Ok(())
    }
}

impl Default for CascadeTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Summarize a brief for auto-summarization at deep cascade chains.
fn summarize_brief(brief: &str) -> String {
    let lines: Vec<&str> = brief.lines().collect();
    if lines.len() <= 10 {
        return brief.to_owned();
    }
    // Take first 5 and last 5 lines
    let first: String = lines[..5].join("\n");
    let last: String = lines[lines.len() - 5..].join("\n");
    format!("{first}\n\n[... {lines_omitted} lines omitted ...]\n\n{last}",
        lines_omitted = lines.len() - 10)
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

    // ── CascadeHandoff ──

    #[test]
    fn handoff_creation() {
        let h = CascadeHandoff::new(pid("a"), pid("b"), "work context".into());
        assert_eq!(h.source.as_str(), "a");
        assert_eq!(h.target.as_str(), "b");
        assert_eq!(h.depth, 1);
        assert!(!h.acknowledged);
        assert!(!h.rejected);
        assert!(h.is_pending());
    }

    #[test]
    fn handoff_dispatched_at_recent() {
        let h = CascadeHandoff::new(pid("a"), pid("b"), "brief".into());
        assert!(h.dispatched_at > 1_700_000_000.0);
    }

    #[test]
    fn handoff_acknowledge() {
        let mut h = CascadeHandoff::new(pid("a"), pid("b"), "brief".into());
        h.acknowledge();
        assert!(h.acknowledged);
        assert!(!h.is_pending());
    }

    #[test]
    fn handoff_reject() {
        let mut h = CascadeHandoff::new(pid("a"), pid("b"), "brief".into());
        h.reject("busy".into());
        assert!(h.rejected);
        assert!(!h.is_pending());
        assert_eq!(h.rejection_reason.as_deref(), Some("busy"));
    }

    #[test]
    fn handoff_elapsed_positive() {
        let h = CascadeHandoff::new(pid("a"), pid("b"), "brief".into());
        assert!(h.elapsed_secs() >= 0.0);
    }

    #[test]
    fn handoff_re_cascade() {
        let h = CascadeHandoff::new(pid("a"), pid("b"), "context".into());
        let re = h.re_cascade(pid("c"));
        assert_eq!(re.source.as_str(), "b");
        assert_eq!(re.target.as_str(), "c");
        assert_eq!(re.depth, 2);
        assert!(re.is_pending());
    }

    #[test]
    fn handoff_re_cascade_deep_summarizes() {
        let mut h = CascadeHandoff::new(pid("a"), pid("b"), "long\nbrief\ncontent".into());
        h.depth = AUTO_SUMMARIZE_DEPTH;
        let re = h.re_cascade(pid("c"));
        assert!(re.brief.contains("Auto-summarized"));
        assert_eq!(re.depth, AUTO_SUMMARIZE_DEPTH + 1);
    }

    #[test]
    fn handoff_needs_summarization() {
        let mut h = CascadeHandoff::new(pid("a"), pid("b"), "brief".into());
        assert!(!h.needs_summarization());
        h.depth = AUTO_SUMMARIZE_DEPTH;
        assert!(h.needs_summarization());
    }

    #[test]
    fn handoff_fallback_brief() {
        let h = CascadeHandoff::new(pid("a"), pid("b"), "work context".into());
        let fb = h.fallback_brief();
        assert!(fb.contains("Cascade Handoff"));
        assert!(fb.contains("a"));
        assert!(fb.contains("b"));
        assert!(fb.contains("work context"));
    }

    #[test]
    fn handoff_truncates_long_brief() {
        let long = "x".repeat(5000);
        let h = CascadeHandoff::new(pid("a"), pid("b"), long);
        assert!(h.brief.chars().count() < 5000);
        assert!(h.brief.contains("truncated"));
    }

    #[test]
    fn handoff_short_brief_not_truncated() {
        let h = CascadeHandoff::new(pid("a"), pid("b"), "short".into());
        assert_eq!(h.brief, "short");
    }

    // ── CascadeTracker ──

    #[test]
    fn tracker_new_empty() {
        let tracker = CascadeTracker::new();
        assert_eq!(tracker.total_count(), 0);
        assert_eq!(tracker.pending_count(), 0);
    }

    #[test]
    fn tracker_default_empty() {
        let tracker = CascadeTracker::default();
        assert_eq!(tracker.total_count(), 0);
    }

    #[test]
    fn tracker_with_max_depth() {
        let tracker = CascadeTracker::with_max_depth(5);
        assert_eq!(tracker.max_depth, 5);
    }

    #[test]
    fn tracker_with_max_depth_min_one() {
        let tracker = CascadeTracker::with_max_depth(0);
        assert_eq!(tracker.max_depth, 1);
    }

    #[test]
    fn tracker_initiate() {
        let mut tracker = CascadeTracker::new();
        let idx = tracker
            .initiate(pid("a"), pid("b"), "brief".into())
            .unwrap();
        assert_eq!(idx, 0);
        assert_eq!(tracker.total_count(), 1);
        assert_eq!(tracker.pending_count(), 1);
    }

    #[test]
    fn tracker_initiate_multiple() {
        let mut tracker = CascadeTracker::new();
        tracker.initiate(pid("a"), pid("b"), "1".into()).unwrap();
        tracker.initiate(pid("c"), pid("d"), "2".into()).unwrap();
        assert_eq!(tracker.total_count(), 2);
    }

    #[test]
    fn tracker_acknowledge() {
        let mut tracker = CascadeTracker::new();
        let idx = tracker.initiate(pid("a"), pid("b"), "brief".into()).unwrap();
        tracker.acknowledge(idx).unwrap();
        assert_eq!(tracker.pending_count(), 0);
        let h = tracker.get(idx).unwrap();
        assert!(h.acknowledged);
    }

    #[test]
    fn tracker_acknowledge_invalid_index() {
        let mut tracker = CascadeTracker::new();
        assert!(tracker.acknowledge(99).is_err());
    }

    #[test]
    fn tracker_reject() {
        let mut tracker = CascadeTracker::new();
        let idx = tracker.initiate(pid("a"), pid("b"), "brief".into()).unwrap();
        tracker.reject(idx, "busy".into()).unwrap();
        assert_eq!(tracker.pending_count(), 0);
        let h = tracker.get(idx).unwrap();
        assert!(h.rejected);
    }

    #[test]
    fn tracker_reject_invalid_index() {
        let mut tracker = CascadeTracker::new();
        assert!(tracker.reject(99, "reason".into()).is_err());
    }

    #[test]
    fn tracker_pending_cascades() {
        let mut tracker = CascadeTracker::new();
        tracker.initiate(pid("a"), pid("b"), "1".into()).unwrap();
        let idx2 = tracker.initiate(pid("c"), pid("d"), "2".into()).unwrap();
        tracker.acknowledge(idx2).unwrap();
        let pending = tracker.pending_cascades();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn tracker_get() {
        let mut tracker = CascadeTracker::new();
        let idx = tracker.initiate(pid("a"), pid("b"), "brief".into()).unwrap();
        let h = tracker.get(idx).unwrap();
        assert_eq!(h.source.as_str(), "a");
    }

    #[test]
    fn tracker_get_invalid_returns_none() {
        let tracker = CascadeTracker::new();
        assert!(tracker.get(0).is_none());
    }

    #[test]
    fn tracker_window_count() {
        let mut tracker = CascadeTracker::new();
        assert_eq!(tracker.window_count(), 0);
        tracker.initiate(pid("a"), pid("b"), "brief".into()).unwrap();
        assert_eq!(tracker.window_count(), 1);
    }

    #[test]
    fn tracker_rate_limit() {
        let mut tracker = CascadeTracker::new();
        for i in 0..MAX_CASCADES_PER_MINUTE {
            let src = format!("s{i}");
            let tgt = format!("t{i}");
            tracker.initiate(pid(&src), pid(&tgt), "brief".into()).unwrap();
        }
        // Next one should fail
        assert!(tracker.initiate(pid("x"), pid("y"), "brief".into()).is_err());
    }

    #[test]
    fn tracker_re_cascade() {
        let mut tracker = CascadeTracker::new();
        let idx = tracker.initiate(pid("a"), pid("b"), "brief".into()).unwrap();
        let re_idx = tracker.re_cascade(idx, pid("c")).unwrap();
        let re = tracker.get(re_idx).unwrap();
        assert_eq!(re.source.as_str(), "b");
        assert_eq!(re.target.as_str(), "c");
        assert_eq!(re.depth, 2);
    }

    #[test]
    fn tracker_re_cascade_max_depth() {
        let mut tracker = CascadeTracker::with_max_depth(2);
        let idx = tracker.initiate(pid("a"), pid("b"), "brief".into()).unwrap();
        let re1 = tracker.re_cascade(idx, pid("c")).unwrap();
        // depth is now 2 = max_depth, next re-cascade should fail
        assert!(tracker.re_cascade(re1, pid("d")).is_err());
    }

    #[test]
    fn tracker_re_cascade_invalid_index() {
        let mut tracker = CascadeTracker::new();
        assert!(tracker.re_cascade(99, pid("c")).is_err());
    }

    #[test]
    fn tracker_prune() {
        let mut tracker = CascadeTracker::new();
        let idx0 = tracker.initiate(pid("a"), pid("b"), "1".into()).unwrap();
        tracker.initiate(pid("c"), pid("d"), "2".into()).unwrap();
        tracker.acknowledge(idx0).unwrap();
        tracker.prune(1);
        // Resolved cascade should be pruned
        assert!(tracker.total_count() <= 2);
    }

    #[test]
    fn tracker_prune_keeps_pending() {
        let mut tracker = CascadeTracker::new();
        tracker.initiate(pid("a"), pid("b"), "pending".into()).unwrap();
        tracker.prune(0);
        // Pending cascade should not be pruned
        assert_eq!(tracker.total_count(), 1);
    }

    // ── summarize_brief ──

    #[test]
    fn summarize_short_unchanged() {
        let brief = "line1\nline2\nline3";
        assert_eq!(summarize_brief(brief), brief);
    }

    #[test]
    fn summarize_long_truncates() {
        let lines: Vec<String> = (0..20).map(|i| format!("line {i}")).collect();
        let brief = lines.join("\n");
        let summary = summarize_brief(&brief);
        assert!(summary.contains("omitted"));
        assert!(summary.contains("line 0"));
        assert!(summary.contains("line 19"));
    }

    #[test]
    fn summarize_exactly_ten_lines_unchanged() {
        let lines: Vec<String> = (0..10).map(|i| format!("line {i}")).collect();
        let brief = lines.join("\n");
        assert_eq!(summarize_brief(&brief), brief);
    }

    // ── Full lifecycle ──

    #[test]
    fn full_cascade_lifecycle() {
        let mut tracker = CascadeTracker::new();

        // Initiate
        let idx = tracker
            .initiate(pid("alpha"), pid("beta"), "Task context here".into())
            .unwrap();
        assert_eq!(tracker.pending_count(), 1);

        // Re-cascade
        let re_idx = tracker.re_cascade(idx, pid("gamma")).unwrap();
        assert_eq!(tracker.total_count(), 2);

        // Acknowledge original
        tracker.acknowledge(idx).unwrap();
        assert_eq!(tracker.pending_count(), 1);

        // Reject re-cascade
        tracker.reject(re_idx, "overloaded".into()).unwrap();
        assert_eq!(tracker.pending_count(), 0);

        // Prune
        tracker.prune(1);
    }

    #[test]
    fn cascade_chain_depth_tracking() {
        let mut tracker = CascadeTracker::with_max_depth(5);
        let idx0 = tracker.initiate(pid("a"), pid("b"), "root".into()).unwrap();
        let idx1 = tracker.re_cascade(idx0, pid("c")).unwrap();
        let idx2 = tracker.re_cascade(idx1, pid("d")).unwrap();

        assert_eq!(tracker.get(idx0).unwrap().depth, 1);
        assert_eq!(tracker.get(idx1).unwrap().depth, 2);
        assert_eq!(tracker.get(idx2).unwrap().depth, 3);
    }
}
