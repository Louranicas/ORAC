//! # M20: Semantic Router
//!
//! Content-aware dispatch using Hebbian coupling weights and domain affinity.
//! Decides which pane is best suited for a given task based on:
//!
//! 1. **Domain classification** — tool/task content maps to semantic regions
//!    (Read→0, Write→π/2, Execute→π, Communicate→3π/2)
//! 2. **Pane affinity** — domain overlap with each sphere's work signature
//! 3. **Hebbian coupling** — co-activation history weights stronger candidates
//! 4. **Health gating** — blocked/overloaded panes are penalized
//!
//! ## Layer: L4 (Intelligence)
//! ## Module: M20
//! ## Dependencies: `m01_core_types`, `m15_coupling_network`

use std::collections::HashMap;
use std::f64::consts::{FRAC_PI_2, PI, TAU};

use serde::{Deserialize, Serialize};

use crate::m1_core::m01_core_types::{PaneId, PaneStatus, PaneSphere};
use super::m15_coupling_network::CouplingNetwork;

// ──────────────────────────────────────────────────────────────
// Domain classification
// ──────────────────────────────────────────────────────────────

/// Semantic domain for tool/task classification.
///
/// Each domain maps to a region on the Kuramoto phase ring:
/// - `Read` → 0 rad
/// - `Write` → π/2 rad
/// - `Execute` → π rad
/// - `Communicate` → 3π/2 rad
/// - `Mixed` → mean of constituent domains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SemanticDomain {
    /// File reading, exploration, search (phase ≈ 0).
    Read,
    /// File writing, editing, creation (phase ≈ π/2).
    Write,
    /// Shell execution, builds, tests (phase ≈ π).
    Execute,
    /// Inter-agent messaging, notifications, PR/issue ops (phase ≈ 3π/2).
    Communicate,
}

impl SemanticDomain {
    /// Phase region for this domain on the Kuramoto ring.
    #[must_use]
    pub const fn phase(self) -> f64 {
        match self {
            Self::Read => 0.0,
            Self::Write => FRAC_PI_2,
            Self::Execute => PI,
            Self::Communicate => 3.0 * FRAC_PI_2,
        }
    }

    /// All domains for iteration.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [Self::Read, Self::Write, Self::Execute, Self::Communicate]
    }
}

// ──────────────────────────────────────────────────────────────
// Domain classifier
// ──────────────────────────────────────────────────────────────

/// Maps tool names to their semantic domain.
///
/// Uses prefix/keyword matching against known Claude Code tools.
#[must_use]
pub fn classify_tool(tool_name: &str) -> SemanticDomain {
    let lower = tool_name.to_lowercase();

    // Read-family tools
    if lower.contains("read")
        || lower.contains("glob")
        || lower.contains("grep")
        || lower.contains("search")
        || lower.contains("ls")
        || lower.contains("list")
        || lower.contains("cat")
        || lower.contains("find")
        || lower.contains("explore")
    {
        return SemanticDomain::Read;
    }

    // Write-family tools
    if lower.contains("write")
        || lower.contains("edit")
        || lower.contains("notebook_edit")
        || lower.contains("create_file")
        || lower.contains("patch")
    {
        return SemanticDomain::Write;
    }

    // Execute-family tools
    if lower.contains("bash")
        || lower.contains("exec")
        || lower.contains("run")
        || lower.contains("shell")
        || lower.contains("test")
        || lower.contains("build")
        || lower.contains("compile")
    {
        return SemanticDomain::Execute;
    }

    // Communicate-family tools
    if lower.contains("agent")
        || lower.contains("message")
        || lower.contains("notify")
        || lower.contains("pr")
        || lower.contains("issue")
        || lower.contains("comment")
        || lower.contains("slack")
        || lower.contains("ask")
    {
        return SemanticDomain::Communicate;
    }

    // Default: Read (exploration is safest default)
    SemanticDomain::Read
}

/// Classify a task description by keyword density across domains.
///
/// Returns the domain with the highest keyword hit count.
/// Falls back to `Read` if no keywords match.
#[must_use]
pub fn classify_content(content: &str) -> SemanticDomain {
    let lower = content.to_lowercase();

    let read_score = count_keywords(
        &lower,
        &["read", "find", "search", "explore", "look", "check", "inspect", "analyze"],
    );
    let write_score = count_keywords(
        &lower,
        &["write", "edit", "create", "modify", "update", "change", "add", "remove"],
    );
    let execute_score = count_keywords(
        &lower,
        &["run", "test", "build", "execute", "compile", "deploy", "install", "start"],
    );
    let communicate_score = count_keywords(
        &lower,
        &["send", "message", "notify", "share", "report", "comment", "review", "pr"],
    );

    let max = read_score.max(write_score).max(execute_score).max(communicate_score);
    if max == 0 {
        return SemanticDomain::Read;
    }

    if max == read_score {
        SemanticDomain::Read
    } else if max == write_score {
        SemanticDomain::Write
    } else if max == execute_score {
        SemanticDomain::Execute
    } else {
        SemanticDomain::Communicate
    }
}

/// Count how many keywords appear in the text.
fn count_keywords(text: &str, keywords: &[&str]) -> usize {
    keywords.iter().filter(|kw| text.contains(**kw)).count()
}

// ──────────────────────────────────────────────────────────────
// Route request
// ──────────────────────────────────────────────────────────────

/// A dispatch request to be routed to the best pane.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    /// Semantic domain of the task.
    pub domain: SemanticDomain,
    /// Optional: originating pane (excluded from candidates if `exclude_self`).
    pub origin: Option<PaneId>,
    /// Whether to exclude the origin pane from candidates.
    pub exclude_self: bool,
    /// Optional: preferred pane (gets a bonus in scoring).
    pub preferred: Option<PaneId>,
}

impl RouteRequest {
    /// Create a request for a given domain.
    #[must_use]
    pub fn new(domain: SemanticDomain) -> Self {
        Self {
            domain,
            origin: None,
            exclude_self: false,
            preferred: None,
        }
    }

    /// Set the origin pane and exclude it from candidates.
    #[must_use]
    pub fn from_pane(mut self, pane: PaneId) -> Self {
        self.origin = Some(pane);
        self.exclude_self = true;
        self
    }

    /// Set a preferred pane (gets a scoring bonus).
    #[must_use]
    pub fn prefer(mut self, pane: PaneId) -> Self {
        self.preferred = Some(pane);
        self
    }
}

// ──────────────────────────────────────────────────────────────
// Route result
// ──────────────────────────────────────────────────────────────

/// Result of a routing decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    /// Selected target pane.
    pub target: PaneId,
    /// Composite score for the selected pane.
    pub score: f64,
    /// All candidates with their scores, sorted descending.
    pub candidates: Vec<CandidateScore>,
    /// Reason string for audit trail.
    pub reason: String,
}

/// A scored routing candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateScore {
    /// Pane ID.
    pub pane: PaneId,
    /// Domain affinity component (0.0–1.0).
    pub domain_affinity: f64,
    /// Hebbian coupling component (0.0–1.0).
    pub hebbian_weight: f64,
    /// Status penalty (0.0 = blocked, 1.0 = available).
    pub availability: f64,
    /// Composite score.
    pub total: f64,
}

// ──────────────────────────────────────────────────────────────
// Scoring weights
// ──────────────────────────────────────────────────────────────

/// Weight for domain affinity in composite score.
const DOMAIN_WEIGHT: f64 = 0.4;
/// Weight for Hebbian coupling in composite score.
const HEBBIAN_WEIGHT: f64 = 0.35;
/// Weight for availability (status) in composite score.
const AVAILABILITY_WEIGHT: f64 = 0.25;
/// Bonus for preferred pane.
const PREFERRED_BONUS: f64 = 0.15;

// ──────────────────────────────────────────────────────────────
// Router
// ──────────────────────────────────────────────────────────────

/// Route a request to the best available pane.
///
/// Scoring formula per candidate:
/// ```text
/// total = DOMAIN_WEIGHT * affinity
///       + HEBBIAN_WEIGHT * coupling
///       + AVAILABILITY_WEIGHT * status_score
///       + PREFERRED_BONUS (if preferred)
/// ```
///
/// # Errors
///
/// Returns `None` if no eligible candidates exist.
#[must_use]
pub fn route<S: std::hash::BuildHasher>(
    request: &RouteRequest,
    spheres: &HashMap<PaneId, PaneSphere, S>,
    network: &CouplingNetwork,
) -> Option<RouteResult> {
    let target_phase = request.domain.phase();

    let mut candidates: Vec<CandidateScore> = spheres
        .iter()
        .filter(|(id, _)| {
            // Exclude self if requested
            if request.exclude_self {
                if let Some(ref origin) = request.origin {
                    if *id == origin {
                        return false;
                    }
                }
            }
            true
        })
        .filter(|(_, sphere)| {
            // Exclude completed spheres
            sphere.status != PaneStatus::Complete
        })
        .map(|(id, sphere)| {
            // 1. Domain affinity: cosine similarity between sphere's phase and target
            let phase_diff = (sphere.phase - target_phase).rem_euclid(TAU);
            let angular_distance = if phase_diff > PI { TAU - phase_diff } else { phase_diff };
            let domain_affinity = 1.0 - (angular_distance / PI);

            // 2. Hebbian coupling weight (from origin if available)
            let hebbian_weight = request
                .origin
                .as_ref()
                .and_then(|origin| network.get_weight(origin, id))
                .unwrap_or(0.5); // Neutral if no origin

            // 3. Availability based on status
            let availability = match sphere.status {
                PaneStatus::Idle => 1.0,
                PaneStatus::Working => 0.6,
                PaneStatus::Blocked | PaneStatus::Complete => 0.0,
            };

            // Composite score
            let mut total = DOMAIN_WEIGHT
                .mul_add(domain_affinity, HEBBIAN_WEIGHT.mul_add(hebbian_weight, AVAILABILITY_WEIGHT * availability));

            // Preferred bonus
            if let Some(ref preferred) = request.preferred {
                if id == preferred {
                    total += PREFERRED_BONUS;
                }
            }

            CandidateScore {
                pane: id.clone(),
                domain_affinity,
                hebbian_weight,
                availability,
                total,
            }
        })
        .collect();

    // Filter out zero-availability candidates (blocked/complete)
    candidates.retain(|c| c.availability > f64::EPSILON);

    // Sort by total score descending
    candidates.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));

    let best = candidates.first()?;

    Some(RouteResult {
        target: best.pane.clone(),
        score: best.total,
        reason: format!(
            "routed to {} (domain={:.2}, hebbian={:.2}, avail={:.2}, total={:.2})",
            best.pane, best.domain_affinity, best.hebbian_weight, best.availability, best.total
        ),
        candidates,
    })
}

/// Compute domain affinity between a sphere and a semantic domain.
///
/// Returns a value in `[0.0, 1.0]` where 1.0 means the sphere's phase
/// is exactly at the domain's phase region.
#[must_use]
pub fn domain_affinity(sphere_phase: f64, domain: SemanticDomain) -> f64 {
    let target = domain.phase();
    let diff = (sphere_phase - target).rem_euclid(TAU);
    let angular = if diff > PI { TAU - diff } else { diff };
    1.0 - (angular / PI)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    fn sphere_at_phase(id: &str, phase: f64, status: PaneStatus) -> PaneSphere {
        let mut s = PaneSphere::new(pid(id), "test");
        s.phase = phase;
        s.status = status;
        s
    }

    fn test_network(panes: &[&str]) -> CouplingNetwork {
        let mut net = CouplingNetwork::new();
        for (i, p) in panes.iter().enumerate() {
            let phase = f64::from(u32::try_from(i).unwrap_or(0)) * 0.5;
            net.register(pid(p), phase, 0.1);
        }
        net
    }

    // ── SemanticDomain ──

    #[test]
    fn domain_phases_are_distinct() {
        let phases: Vec<f64> = SemanticDomain::all().iter().map(|d| d.phase()).collect();
        for i in 0..phases.len() {
            for j in (i + 1)..phases.len() {
                assert!((phases[i] - phases[j]).abs() > 0.1, "domains should have distinct phases");
            }
        }
    }

    #[test]
    fn domain_phases_in_range() {
        for d in SemanticDomain::all() {
            let p = d.phase();
            assert!(p >= 0.0 && p < TAU, "phase {p} should be in [0, 2π)");
        }
    }

    #[test]
    fn domain_read_phase_is_zero() {
        assert_relative_eq!(SemanticDomain::Read.phase(), 0.0);
    }

    #[test]
    fn domain_write_phase_is_pi_half() {
        assert_relative_eq!(SemanticDomain::Write.phase(), FRAC_PI_2);
    }

    #[test]
    fn domain_execute_phase_is_pi() {
        assert_relative_eq!(SemanticDomain::Execute.phase(), PI);
    }

    #[test]
    fn domain_communicate_phase_is_three_pi_half() {
        assert_relative_eq!(SemanticDomain::Communicate.phase(), 3.0 * FRAC_PI_2);
    }

    // ── classify_tool ──

    #[test]
    fn classify_read_tool() {
        assert_eq!(classify_tool("Read"), SemanticDomain::Read);
        assert_eq!(classify_tool("Glob"), SemanticDomain::Read);
        assert_eq!(classify_tool("Grep"), SemanticDomain::Read);
    }

    #[test]
    fn classify_write_tool() {
        assert_eq!(classify_tool("Write"), SemanticDomain::Write);
        assert_eq!(classify_tool("Edit"), SemanticDomain::Write);
    }

    #[test]
    fn classify_execute_tool() {
        assert_eq!(classify_tool("Bash"), SemanticDomain::Execute);
    }

    #[test]
    fn classify_communicate_tool() {
        assert_eq!(classify_tool("Agent"), SemanticDomain::Communicate);
    }

    #[test]
    fn classify_unknown_defaults_to_read() {
        assert_eq!(classify_tool("UnknownTool"), SemanticDomain::Read);
    }

    #[test]
    fn classify_case_insensitive() {
        assert_eq!(classify_tool("BASH"), SemanticDomain::Execute);
        assert_eq!(classify_tool("read"), SemanticDomain::Read);
        assert_eq!(classify_tool("WRITE"), SemanticDomain::Write);
    }

    // ── classify_content ──

    #[test]
    fn classify_content_read() {
        let domain = classify_content("please read and search the file for errors");
        assert_eq!(domain, SemanticDomain::Read);
    }

    #[test]
    fn classify_content_write() {
        let domain = classify_content("create a new file and edit the module to add a function");
        assert_eq!(domain, SemanticDomain::Write);
    }

    #[test]
    fn classify_content_execute() {
        let domain = classify_content("run the test suite and build the binary");
        assert_eq!(domain, SemanticDomain::Execute);
    }

    #[test]
    fn classify_content_communicate() {
        let domain = classify_content("send the review comment and share the PR");
        assert_eq!(domain, SemanticDomain::Communicate);
    }

    #[test]
    fn classify_content_empty_defaults_to_read() {
        assert_eq!(classify_content(""), SemanticDomain::Read);
    }

    // ── domain_affinity ──

    #[test]
    fn affinity_perfect_match() {
        let a = domain_affinity(0.0, SemanticDomain::Read);
        assert_relative_eq!(a, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn affinity_opposite_is_zero() {
        // Read is at 0, opposite is at π
        let a = domain_affinity(PI, SemanticDomain::Read);
        assert_relative_eq!(a, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn affinity_half_way() {
        // π/2 away from Read (at 0)
        let a = domain_affinity(FRAC_PI_2, SemanticDomain::Read);
        assert_relative_eq!(a, 0.5, epsilon = 1e-10);
    }

    #[test]
    fn affinity_symmetric() {
        let a1 = domain_affinity(0.3, SemanticDomain::Read);
        let a2 = domain_affinity(TAU - 0.3, SemanticDomain::Read);
        assert_relative_eq!(a1, a2, epsilon = 1e-10);
    }

    #[test]
    fn affinity_always_bounded() {
        for phase in [0.0, 0.5, 1.0, PI, TAU - 0.1] {
            for domain in SemanticDomain::all() {
                let a = domain_affinity(phase, domain);
                assert!(a >= 0.0 && a <= 1.0, "affinity {a} out of bounds for phase {phase}");
            }
        }
    }

    // ── RouteRequest builder ──

    #[test]
    fn route_request_builder() {
        let req = RouteRequest::new(SemanticDomain::Write)
            .from_pane(pid("alpha"))
            .prefer(pid("beta"));
        assert_eq!(req.domain, SemanticDomain::Write);
        assert_eq!(req.origin.as_ref().map(PaneId::as_str), Some("alpha"));
        assert!(req.exclude_self);
        assert_eq!(req.preferred.as_ref().map(PaneId::as_str), Some("beta"));
    }

    #[test]
    fn route_request_default_no_exclusion() {
        let req = RouteRequest::new(SemanticDomain::Read);
        assert!(!req.exclude_self);
        assert!(req.origin.is_none());
        assert!(req.preferred.is_none());
    }

    // ── route() ──

    #[test]
    fn route_empty_spheres_returns_none() {
        let spheres = HashMap::new();
        let net = CouplingNetwork::new();
        let req = RouteRequest::new(SemanticDomain::Read);
        assert!(route(&req, &spheres, &net).is_none());
    }

    #[test]
    fn route_single_available_sphere() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        let net = test_network(&["a"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("a"));
    }

    #[test]
    fn route_excludes_completed_spheres() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Complete));
        let net = test_network(&["a"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        assert!(route(&req, &spheres, &net).is_none());
    }

    #[test]
    fn route_excludes_self() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        spheres.insert(pid("b"), sphere_at_phase("b", 0.5, PaneStatus::Idle));
        let net = test_network(&["a", "b"]);
        let req = RouteRequest::new(SemanticDomain::Read).from_pane(pid("a"));
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("b"));
    }

    #[test]
    fn route_prefers_matching_domain() {
        let mut spheres = HashMap::new();
        // Sphere at Read phase (0) should win for Read tasks
        spheres.insert(pid("reader"), sphere_at_phase("reader", 0.0, PaneStatus::Idle));
        // Sphere at Execute phase (π) should lose for Read tasks
        spheres.insert(pid("executor"), sphere_at_phase("executor", PI, PaneStatus::Idle));
        let net = test_network(&["reader", "executor"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("reader"));
    }

    #[test]
    fn route_avoids_blocked_panes() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Blocked));
        spheres.insert(pid("b"), sphere_at_phase("b", PI, PaneStatus::Idle));
        let net = test_network(&["a", "b"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        // Should pick b even though a is closer in phase — a is blocked
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("b"));
    }

    #[test]
    fn route_all_blocked_returns_none() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Blocked));
        spheres.insert(pid("b"), sphere_at_phase("b", PI, PaneStatus::Blocked));
        let net = test_network(&["a", "b"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        assert!(route(&req, &spheres, &net).is_none());
    }

    #[test]
    fn route_preferred_gets_bonus() {
        let mut spheres = HashMap::new();
        // Both at same phase, same status
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        spheres.insert(pid("b"), sphere_at_phase("b", 0.0, PaneStatus::Idle));
        let net = test_network(&["a", "b"]);
        let req = RouteRequest::new(SemanticDomain::Read).prefer(pid("b"));
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("b"));
    }

    #[test]
    fn route_result_has_candidates() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        spheres.insert(pid("b"), sphere_at_phase("b", 1.0, PaneStatus::Idle));
        let net = test_network(&["a", "b"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|r| r.candidates.len()), Some(2));
    }

    #[test]
    fn route_candidates_sorted_descending() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        spheres.insert(pid("b"), sphere_at_phase("b", 1.0, PaneStatus::Idle));
        spheres.insert(pid("c"), sphere_at_phase("c", 2.0, PaneStatus::Idle));
        let net = test_network(&["a", "b", "c"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        let r = result.as_ref();
        assert!(r.is_some());
        let candidates = &r.map(|r| &r.candidates);
        if let Some(cs) = candidates {
            for w in cs.windows(2) {
                assert!(w[0].total >= w[1].total, "candidates should be sorted descending");
            }
        }
    }

    #[test]
    fn route_result_reason_not_empty() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        let net = test_network(&["a"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        assert!(!result.as_ref().map_or(true, |r| r.reason.is_empty()));
    }

    #[test]
    fn route_hebbian_weight_influences_selection() {
        let mut spheres = HashMap::new();
        // Both at same phase, same status
        spheres.insert(pid("origin"), sphere_at_phase("origin", 0.0, PaneStatus::Idle));
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        spheres.insert(pid("b"), sphere_at_phase("b", 0.0, PaneStatus::Idle));

        let mut net = test_network(&["origin", "a", "b"]);
        // Give "a" a higher coupling weight from "origin"
        net.set_weight(&pid("origin"), &pid("a"), 0.9);
        net.set_weight(&pid("origin"), &pid("b"), 0.1);

        let req = RouteRequest::new(SemanticDomain::Read).from_pane(pid("origin"));
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("a"));
    }

    #[test]
    fn route_working_sphere_penalized() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.0, PaneStatus::Idle));
        spheres.insert(pid("b"), sphere_at_phase("b", 0.0, PaneStatus::Working));
        let net = test_network(&["a", "b"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        assert!(result.is_some());
        // Idle should beat Working (all else equal)
        assert_eq!(result.as_ref().map(|r| r.target.as_str()), Some("a"));
    }

    // ── CandidateScore ──

    #[test]
    fn candidate_score_fields_bounded() {
        let mut spheres = HashMap::new();
        spheres.insert(pid("a"), sphere_at_phase("a", 0.5, PaneStatus::Idle));
        let net = test_network(&["a"]);
        let req = RouteRequest::new(SemanticDomain::Read);
        let result = route(&req, &spheres, &net);
        let cs = &result.as_ref().map(|r| &r.candidates[0]);
        if let Some(c) = cs {
            assert!(c.domain_affinity >= 0.0 && c.domain_affinity <= 1.0);
            assert!(c.availability >= 0.0 && c.availability <= 1.0);
            assert!(c.total >= 0.0);
        }
    }

    // ── count_keywords ──

    #[test]
    fn count_keywords_empty() {
        assert_eq!(count_keywords("", &["a", "b"]), 0);
    }

    #[test]
    fn count_keywords_all_match() {
        assert_eq!(count_keywords("a b c", &["a", "b", "c"]), 3);
    }

    #[test]
    fn count_keywords_partial_match() {
        assert_eq!(count_keywords("a x c", &["a", "b", "c"]), 2);
    }

    #[test]
    fn count_keywords_no_duplicates() {
        // "read" appears twice in text but keyword only counted once
        assert_eq!(count_keywords("read read read", &["read"]), 1);
    }

    // ── Scoring weights ──

    #[test]
    fn scoring_weights_sum_to_one() {
        let sum = DOMAIN_WEIGHT + HEBBIAN_WEIGHT + AVAILABILITY_WEIGHT;
        assert_relative_eq!(sum, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn scoring_weights_all_positive() {
        assert!(DOMAIN_WEIGHT > 0.0);
        assert!(HEBBIAN_WEIGHT > 0.0);
        assert!(AVAILABILITY_WEIGHT > 0.0);
    }

    #[test]
    fn preferred_bonus_positive() {
        assert!(PREFERRED_BONUS > 0.0);
    }
}
