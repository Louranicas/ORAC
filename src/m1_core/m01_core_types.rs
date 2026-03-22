//! # M01: Core Types
//!
//! Foundational types used across all Pane-Vortex V2 layers: `PaneId`, `TaskId`,
//! `Point3D`, `SphereMemory`, `Buoy`, `OrderParameter`, `FleetMode`.
//!
//! ## Layer: L1 (Foundation)
//! ## Module: M01
//! ## Dependencies: None (leaf module)
//!
//! ## Design Constraints
//! - C1: No upward imports (this is L1 — no dependencies)
//! - C4: Zero unsafe/unwrap/expect
//! - C5: Copy semantics for `Point3D` (24 bytes = 3×f64)
//! - C7: Newtype wrappers for type safety (`PaneId`, `TaskId`, `Phase`)

use std::fmt;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────
// Newtypes for type safety
// ──────────────────────────────────────────────────────────────

/// Unique pane identifier (e.g. "fleet-alpha:left", "claude:session-039").
///
/// Newtype wrapper around `String` for type safety at API boundaries.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(String);

impl PaneId {
    /// Create a new `PaneId` from any string-like value.
    #[must_use]
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Borrow the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for PaneId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PaneId {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

/// Unique task identifier (UUID v4 string).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    /// Generate a new random task ID.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Create a `TaskId` from an existing string (e.g. deserialized from DB).
    #[must_use]
    pub fn from_existing(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the inner string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ──────────────────────────────────────────────────────────────
// 3D embedding types
// ──────────────────────────────────────────────────────────────

/// Point on the unit sphere (3D embedding of spherical surface).
///
/// Copy semantics: 24 bytes = 3×f64.
/// Used for sphere memory placement and buoy positioning.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3D {
    /// X coordinate.
    pub x: f64,
    /// Y coordinate.
    pub y: f64,
    /// Z coordinate.
    pub z: f64,
}

impl Point3D {
    /// Create a new point. Values are not `normalized` — caller should
    /// use [`normalized`](Self::normalized) if unit-sphere semantics are required.
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// North pole of the unit sphere.
    #[must_use]
    pub const fn north() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        }
    }

    /// Construct from spherical coordinates (physics convention: theta=polar, phi=azimuthal).
    #[must_use]
    pub fn from_spherical(theta: f64, phi: f64) -> Self {
        let sin_theta = theta.sin();
        Self {
            x: sin_theta * phi.cos(),
            y: sin_theta * phi.sin(),
            z: theta.cos(),
        }
    }

    /// Dot product.
    #[must_use]
    pub fn dot(self, other: Self) -> f64 {
        self.x.mul_add(other.x, self.y.mul_add(other.y, self.z * other.z))
    }

    /// Euclidean norm.
    #[must_use]
    pub fn norm(self) -> f64 {
        self.dot(self).sqrt()
    }

    /// Normalize to unit length. Returns [`north()`](Self::north) if norm is zero or NaN.
    #[must_use]
    pub fn normalized(self) -> Self {
        let n = self.norm();
        if !n.is_finite() || n < f64::EPSILON {
            return Self::north();
        }
        Self {
            x: self.x / n,
            y: self.y / n,
            z: self.z / n,
        }
    }

    /// Great-circle (angular) distance between two points on the unit sphere.
    #[must_use]
    pub fn angular_distance(self, other: Self) -> f64 {
        let d = self.normalized().dot(other.normalized());
        d.clamp(-1.0, 1.0).acos()
    }

    /// Arc distance (same as angular distance on unit sphere).
    #[must_use]
    pub fn arc_distance(self, other: Self) -> f64 {
        self.angular_distance(other)
    }

    /// Spherical linear interpolation (`slerp`) between two unit-sphere points.
    ///
    /// `t=0.0` returns `self`, `t=1.0` returns `other`.
    #[must_use]
    pub fn slerp(self, other: Self, t: f64) -> Self {
        let a = self.normalized();
        let b = other.normalized();
        let dot = a.dot(b).clamp(-1.0, 1.0);
        let omega = dot.acos();

        if omega.abs() < f64::EPSILON {
            return a;
        }

        let sin_omega = omega.sin();
        let sa = ((1.0 - t) * omega).sin() / sin_omega;
        let sb = (t * omega).sin() / sin_omega;

        Self {
            x: sa.mul_add(a.x, sb * b.x),
            y: sa.mul_add(a.y, sb * b.y),
            z: sa.mul_add(a.z, sb * b.z),
        }
    }
}

impl Default for Point3D {
    fn default() -> Self {
        Self::north()
    }
}

impl PartialEq for Point3D {
    fn eq(&self, other: &Self) -> bool {
        (self.x - other.x).abs() < f64::EPSILON
            && (self.y - other.y).abs() < f64::EPSILON
            && (self.z - other.z).abs() < f64::EPSILON
    }
}

// ──────────────────────────────────────────────────────────────
// Sphere memory and buoy types
// ──────────────────────────────────────────────────────────────

/// A memory placed on the sphere surface by a tool call.
///
/// Memories decay over time and are pruned when activation drops below threshold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SphereMemory {
    /// Unique ID within the sphere.
    pub id: u64,
    /// Position on the unit sphere.
    pub position: Point3D,
    /// Current activation level (multiplicatively decayed each tick by `DECAY_PER_STEP`).
    pub activation: f64,
    /// Name of the tool that created this memory.
    pub tool_name: String,
    /// Human-readable summary.
    pub summary: String,
    /// Unix timestamp (seconds) when the memory was created.
    pub timestamp: f64,
    /// Confidence value from the tool call (0.0–1.0).
    pub confidence: f64,
}

impl SphereMemory {
    /// Create a new memory at full activation.
    #[must_use]
    pub fn new(id: u64, position: Point3D, tool_name: String, summary: String) -> Self {
        Self {
            id,
            position,
            activation: 1.0,
            tool_name,
            summary,
            timestamp: now_secs(),
            confidence: 1.0,
        }
    }
}

/// Hebbian buoy — a learned cluster on the sphere surface.
///
/// Buoys drift toward high-activation regions and boost nearby memories.
/// They have a home position and slowly drift back when inactive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Buoy {
    /// Current position (drifts toward activation centroids).
    pub position: Point3D,
    /// Original position (drifts back when inactive).
    pub home: Point3D,
    /// Number of times this buoy has been activated.
    pub activation_count: u64,
    /// Angular radius of influence (radians).
    pub influence_radius: f64,
    /// Multiplicative boost applied to memories within influence radius.
    pub boost_multiplier: f64,
    /// Rate at which the buoy drifts toward activation centroids.
    pub learning_rate: f64,
    /// Human-readable label (e.g. "Read cluster", "Edit cluster").
    pub label: String,
}

impl Buoy {
    /// Create a new buoy at the given position.
    #[must_use]
    pub const fn new(position: Point3D, label: String, learning_rate: f64) -> Self {
        Self {
            position,
            home: position,
            activation_count: 0,
            influence_radius: 0.4,
            boost_multiplier: 1.5,
            learning_rate,
            label,
        }
    }

    /// Activation boost at a given point (falls off with angular distance).
    #[must_use]
    pub fn boost_at(&self, point: &Point3D) -> f64 {
        let d = self.position.angular_distance(*point);
        if d < self.influence_radius {
            self.boost_multiplier * (1.0 - d / self.influence_radius)
        } else {
            0.0
        }
    }

    /// Drift toward a centroid (Hebbian learning).
    pub fn drift_toward(&mut self, centroid: &Point3D) {
        self.position = self.position.slerp(*centroid, self.learning_rate);
        self.activation_count = self.activation_count.saturating_add(1);
    }

    /// Drift back toward home position.
    pub fn drift_home(&mut self, rate: f64) {
        self.position = self.position.slerp(self.home, rate);
    }
}

// ──────────────────────────────────────────────────────────────
// Status and work characterisation
// ──────────────────────────────────────────────────────────────

/// Sphere operational status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneStatus {
    /// Not currently working.
    #[default]
    Idle,
    /// Actively processing a tool call.
    Working,
    /// Waiting on an external dependency.
    Blocked,
    /// Finished session, awaiting deregistration.
    Complete,
}

impl fmt::Display for PaneStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Working => write!(f, "Working"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Complete => write!(f, "Complete"),
        }
    }
}

/// Continuous work characterisation replacing binary `Idle`/`Working`.
///
/// All fields are in `[0.0, 1.0]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkSignature {
    /// Rate of memory creation in the recent window.
    pub intensity: f64,
    /// Regularity of memory spacing (high = rhythmic).
    pub rhythm: f64,
    /// Variety of distinct tools used recently.
    pub diversity: f64,
    /// Concentration on a single tool (inverse of diversity).
    pub focus: f64,
}

/// Field context injected into a sphere's `step()` so it can reason about
/// its position relative to the global coupling field.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SphereFieldContext {
    /// Global order parameter `r`.
    pub global_r: f64,
    /// Number of spheres in this sphere's cluster.
    pub my_cluster_size: usize,
    /// Whether this sphere is in a synchronized cluster.
    pub is_synchronized: bool,
    /// This sphere's effective coupling strength.
    pub my_coupling_strength: f64,
    /// Number of active tunnels involving this sphere.
    pub tunnel_count: usize,
}

// ──────────────────────────────────────────────────────────────
// Ghost trace (departed sphere memory)
// ──────────────────────────────────────────────────────────────

/// Lightweight trace of a deregistered sphere.
///
/// Preserves identity and contribution record after departure.
/// Ghost traces honour the sphere's existence — "remembering those who leave."
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhostTrace {
    /// Sphere ID at time of departure.
    pub id: PaneId,
    /// Persona string.
    pub persona: String,
    /// Tick at which the sphere was deregistered.
    pub deregistered_at: u64,
    /// Total step count during the sphere's lifetime.
    pub total_steps_lived: u64,
    /// Number of memories created.
    pub memory_count: usize,
    /// Most-used tool names.
    pub top_tools: Vec<String>,
    /// Phase at the moment of departure.
    pub phase_at_departure: f64,
    /// Receptivity at departure.
    pub receptivity: f64,
    /// Work signature at departure.
    pub work_signature: WorkSignature,
    /// Strongest coupling neighbors and weights.
    pub strongest_neighbors: Vec<(String, f64)>,
}

// ──────────────────────────────────────────────────────────────
// Inbox message (pending inter-sphere communication)
// ──────────────────────────────────────────────────────────────

/// A message waiting for sphere acknowledgement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboxMessage {
    /// Message ID (unique within the sphere).
    pub id: u64,
    /// Sender sphere ID.
    pub from: String,
    /// Message content.
    pub content: String,
    /// Unix timestamp when received.
    pub received_at: f64,
    /// Whether the sphere has acknowledged this message.
    pub acknowledged: bool,
}

/// Maximum inbox size (FIFO eviction beyond this).
pub const INBOX_MAX: usize = 50;

// ──────────────────────────────────────────────────────────────
// Order parameter
// ──────────────────────────────────────────────────────────────

/// Kuramoto order parameter: magnitude r ∈ [0,1] and mean phase ψ.
///
/// `r = 1.0` means perfect synchronization. `r ≈ 0.0` means incoherent.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OrderParameter {
    /// Magnitude of the mean-field coupling (0.0 = incoherent, 1.0 = locked).
    pub r: f64,
    /// Mean phase angle (radians).
    pub psi: f64,
}

impl OrderParameter {
    /// Create a new order parameter with the given `r` and `psi`.
    #[must_use]
    pub const fn new(r: f64, psi: f64) -> Self {
        Self { r, psi }
    }

    /// Incoherent field: `r = 0.0`, `psi = 0.0`.
    #[must_use]
    pub const fn incoherent() -> Self {
        Self { r: 0.0, psi: 0.0 }
    }
}

impl Default for OrderParameter {
    fn default() -> Self {
        Self::incoherent()
    }
}

// ──────────────────────────────────────────────────────────────
// Decision record (audit trail)
// ──────────────────────────────────────────────────────────────

/// A single decision record for the conductor audit trail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    /// Tick at which the decision was made.
    pub tick: u64,
    /// The action recommended by the field decision engine.
    pub action: FieldAction,
    /// Order parameter r at decision time.
    pub r: f64,
    /// Current k modulation factor.
    pub k_mod: f64,
    /// Number of active spheres.
    pub sphere_count: usize,
}

/// Recommended action based on current field state.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum FieldAction {
    /// Field is stable — no intervention needed.
    #[default]
    Stable,
    /// r is falling — increase coupling.
    NeedsCoherence,
    /// r is too high — decrease coupling to allow exploration.
    NeedsDivergence,
    /// One or more spheres are blocked.
    HasBlockedAgents,
    /// Most spheres are idle.
    IdleFleet,
    /// Freshly registered fleet — warming up.
    FreshFleet,
    /// Recovering from a divergence kick.
    Recovering,
    /// r > 0.99 — field is over-synchronized (ALERT-5).
    OverSynchronized,
}

impl fmt::Display for FieldAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stable => write!(f, "Stable"),
            Self::NeedsCoherence => write!(f, "NeedsCoherence"),
            Self::NeedsDivergence => write!(f, "NeedsDivergence"),
            Self::HasBlockedAgents => write!(f, "HasBlockedAgents"),
            Self::IdleFleet => write!(f, "IdleFleet"),
            Self::FreshFleet => write!(f, "FreshFleet"),
            Self::Recovering => write!(f, "Recovering"),
            Self::OverSynchronized => write!(f, "OverSynchronized"),
        }
    }
}

/// Trend of the order parameter over the rolling window.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum RTrend {
    /// r is increasing over the window.
    Rising,
    /// r is decreasing over the window.
    Falling,
    /// r is approximately constant.
    #[default]
    Stable,
}

// ──────────────────────────────────────────────────────────────
// Bridge adjustment tracking
// ──────────────────────────────────────────────────────────────

/// Tracks the most recent bridge adjustment values.
#[derive(Debug, Clone, Default)]
pub struct BridgeAdjustments {
    /// SYNTHEX thermal adjustment.
    pub synthex_adj: f64,
    /// SAN-K7 nexus adjustment.
    pub nexus_adj: f64,
    /// Maintenance Engine adjustment.
    pub me_adj: f64,
    /// Combined multiplicative effect of all bridges.
    pub combined_effect: f64,
    /// Tick at which these values were last updated.
    pub updated_at: u64,
}

/// Per-bridge staleness flags packed into a `u8` bitfield.
///
/// Each bit indicates whether the corresponding bridge's last poll result
/// has exceeded its configured interval. Used by the conductor to decide
/// whether to trust cached adjustments.
///
/// Bit layout: `SYNTHEX | NEXUS | POVM | RM | VMS | ME` (bits 0--5).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BridgeStaleness(u8);

impl BridgeStaleness {
    /// Bit mask for the SYNTHEX bridge.
    pub const SYNTHEX: u8 = 1 << 0;
    /// Bit mask for the Nexus bridge.
    pub const NEXUS: u8 = 1 << 1;
    /// Bit mask for the POVM bridge.
    pub const POVM: u8 = 1 << 2;
    /// Bit mask for the Reasoning Memory bridge.
    pub const RM: u8 = 1 << 3;
    /// Bit mask for the Vortex Memory System bridge.
    pub const VMS: u8 = 1 << 4;
    /// Bit mask for the Maintenance Engine bridge.
    pub const ME: u8 = 1 << 5;

    /// Create a new empty (all-fresh) staleness set.
    #[must_use]
    pub const fn new() -> Self {
        Self(0)
    }

    /// Set a bridge as stale.
    pub fn set_stale(&mut self, mask: u8) {
        self.0 |= mask;
    }

    /// Clear a bridge's stale flag.
    pub fn set_fresh(&mut self, mask: u8) {
        self.0 &= !mask;
    }

    /// Check whether a specific bridge is stale.
    #[must_use]
    pub const fn is_stale(self, mask: u8) -> bool {
        self.0 & mask != 0
    }

    /// Returns `true` if any bridge data is stale.
    #[must_use]
    pub const fn any_stale(self) -> bool {
        self.0 != 0
    }

    /// Returns the number of stale bridges.
    #[must_use]
    pub const fn stale_count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the SYNTHEX bridge data is stale.
    #[must_use]
    pub const fn synthex_stale(self) -> bool {
        self.is_stale(Self::SYNTHEX)
    }

    /// Whether the Nexus bridge data is stale.
    #[must_use]
    pub const fn nexus_stale(self) -> bool {
        self.is_stale(Self::NEXUS)
    }

    /// Whether the POVM bridge data is stale.
    #[must_use]
    pub const fn povm_stale(self) -> bool {
        self.is_stale(Self::POVM)
    }

    /// Whether the Reasoning Memory bridge data is stale.
    #[must_use]
    pub const fn rm_stale(self) -> bool {
        self.is_stale(Self::RM)
    }

    /// Whether the Vortex Memory System bridge data is stale.
    #[must_use]
    pub const fn vms_stale(self) -> bool {
        self.is_stale(Self::VMS)
    }

    /// Whether the Maintenance Engine bridge data is stale.
    #[must_use]
    pub const fn me_stale(self) -> bool {
        self.is_stale(Self::ME)
    }
}

// ──────────────────────────────────────────────────────────────
// Fleet mode
// ──────────────────────────────────────────────────────────────

/// Operational mode reflecting fleet size and coupling confidence.
///
/// The mode determines which field dynamics are meaningful:
/// - `Solo`: no coupling possible (single sphere)
/// - `Pair`: limited two-body dynamics
/// - `Small`: minimal chimera detection possible (3--4 spheres)
/// - `Full`: complete Kuramoto dynamics (5+ spheres)
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FleetMode {
    /// Single sphere — no coupling dynamics possible.
    #[default]
    Solo,
    /// 2 spheres — limited dynamics.
    Pair,
    /// 3--4 spheres — minimal chimera possible.
    Small,
    /// 5+ spheres — full dynamics.
    Full,
}

impl FleetMode {
    /// Determine `FleetMode` from sphere count.
    ///
    /// - 0 or 1 => `Solo`
    /// - 2 => `Pair`
    /// - 3 or 4 => `Small`
    /// - 5+ => `Full`
    #[must_use]
    pub const fn from_count(n: usize) -> Self {
        match n {
            0 | 1 => Self::Solo,
            2 => Self::Pair,
            3 | 4 => Self::Small,
            _ => Self::Full,
        }
    }

    /// Human-readable name for this fleet mode.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Solo => "Solo",
            Self::Pair => "Pair",
            Self::Small => "Small",
            Self::Full => "Full",
        }
    }
}

impl fmt::Display for FleetMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ──────────────────────────────────────────────────────────────
// Utility
// ──────────────────────────────────────────────────────────────

/// Current time as seconds since UNIX epoch.
///
/// Returns `0.0` if the system clock is unavailable (e.g. before epoch --
/// should never happen in practice).
#[must_use]
pub fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// Wrapping phase difference in `[-π, π]`.
///
/// Computes `a - b` modulo `TAU`, then folds into the signed half-circle.
/// Positive means `a` leads `b`; negative means `a` lags.
#[must_use]
pub fn phase_diff(a: f64, b: f64) -> f64 {
    let d = (a - b).rem_euclid(std::f64::consts::TAU);
    if d > std::f64::consts::PI {
        d - std::f64::consts::TAU
    } else {
        d
    }
}

/// Map a tool name to a deterministic phase region on `[0, 2π)`.
///
/// Known tool families (`Read`, `Write`, `Edit`, `Grep`, `Bash`, `Glob`,
/// `Agent`) are placed at fixed octants. Unknown tools are hashed via FNV-1a
/// into the full circle. Semantically related tools cluster together,
/// improving Hebbian buoy formation.
#[must_use]
pub fn semantic_phase_region(tool_name: &str) -> f64 {
    use std::f64::consts::TAU;
    let prefix = tool_name.split(':').next().unwrap_or(tool_name);
    let base = match prefix {
        s if s.starts_with("Read") || s.starts_with("read") => 0.0,
        s if s.starts_with("Write") || s.starts_with("write") => TAU * 0.125,
        s if s.starts_with("Edit") || s.starts_with("edit") => TAU * 0.25,
        s if s.starts_with("Grep") || s.starts_with("grep") || s.starts_with("search") => {
            TAU * 0.375
        }
        s if s.starts_with("Bash") || s.starts_with("bash") || s.starts_with("shell") => {
            TAU * 0.5
        }
        s if s.starts_with("Glob") || s.starts_with("glob") || s.starts_with("find") => {
            TAU * 0.625
        }
        s if s.starts_with("Agent") || s.starts_with("agent") || s.starts_with("Task") => {
            TAU * 0.75
        }
        _ => {
            // Hash remaining tool names into [0, 2π)
            // Use upper 32 bits for lossless f64 conversion
            let h: u64 = tool_name
                .bytes()
                .fold(0x517c_c1b7_2722_0a95_u64, |acc, b| {
                    acc.wrapping_mul(0x0100_0000_01b3).wrapping_add(u64::from(b))
                });
            let h32 = (h >> 32) as u32;
            (f64::from(h32) / f64::from(u32::MAX)) * TAU
        }
    };
    base.rem_euclid(TAU)
}

// ──────────────────────────────────────────────────────────────
// PaneSphere — ORAC-native sphere representation
// ──────────────────────────────────────────────────────────────

/// A pane sphere's state as observed by the sidecar.
///
/// This is the ORAC-native representation of a fleet agent's state,
/// adapted from PV2's `PaneSphere`. Contains only the fields that
/// the sidecar's intelligence and coordination layers need.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSphere {
    /// Unique identifier for this sphere.
    pub id: PaneId,
    /// Human-readable persona name.
    pub persona: String,
    /// Current operational status.
    pub status: PaneStatus,
    /// Current phase on the Kuramoto ring \[0, 2π).
    pub phase: f64,
    /// Natural frequency.
    pub frequency: f64,
    /// Buoy network for spatial memory.
    pub buoys: Vec<Buoy>,
    /// Sphere memories.
    pub memories: Vec<SphereMemory>,
    /// Whether this sphere opts out of Hebbian learning.
    pub opt_out_hebbian: bool,
    /// Activity count in the last 30 seconds (burst detection).
    pub activity_30s: usize,
    /// Total tick steps this sphere has lived.
    pub total_steps: u64,
    /// Receptivity to coupling field modulation \[0.0, 1.0\].
    pub receptivity: f64,
    /// Work signature for this sphere.
    pub work_signature: WorkSignature,
    /// Field context injected by the daemon.
    pub field_context: Option<SphereFieldContext>,
}

impl Default for PaneSphere {
    fn default() -> Self {
        Self {
            id: PaneId::default(),
            persona: String::new(),
            status: PaneStatus::default(),
            phase: 0.0,
            frequency: 1.0,
            buoys: Vec::new(),
            memories: Vec::new(),
            opt_out_hebbian: false,
            activity_30s: 0,
            total_steps: 0,
            receptivity: 1.0,
            work_signature: WorkSignature::default(),
            field_context: None,
        }
    }
}

impl PaneSphere {
    /// Create a new `PaneSphere` with the given id and persona.
    #[must_use]
    pub fn new(id: PaneId, persona: impl Into<String>) -> Self {
        Self {
            id,
            persona: persona.into(),
            ..Self::default()
        }
    }
}

/// Activation zones for memory pruning thresholds.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ActivationZones {
    /// Memories above this are considered active.
    pub active_threshold: f64,
    /// Memories below this are candidates for pruning.
    pub prune_threshold: f64,
    /// Maximum memories per sphere.
    pub capacity: usize,
}

impl ActivationZones {
    /// Default activation zones.
    #[must_use]
    pub fn standard() -> Self {
        Self {
            active_threshold: 0.3,
            prune_threshold: 0.05,
            capacity: 500,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    // ── PaneId ──

    #[test]
    fn pane_id_from_str() {
        let id = PaneId::from("test-pane");
        assert_eq!(id.as_str(), "test-pane");
    }

    #[test]
    fn pane_id_from_string() {
        let id = PaneId::from(String::from("fleet-alpha:left"));
        assert_eq!(id.as_str(), "fleet-alpha:left");
    }

    #[test]
    fn pane_id_display() {
        let id = PaneId::new("my-sphere");
        assert_eq!(format!("{id}"), "my-sphere");
    }

    #[test]
    fn pane_id_equality() {
        let a = PaneId::new("x");
        let b = PaneId::new("x");
        assert_eq!(a, b);
    }

    #[test]
    fn pane_id_hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(PaneId::new("a"));
        set.insert(PaneId::new("b"));
        set.insert(PaneId::new("a"));
        assert_eq!(set.len(), 2);
    }

    // ── TaskId ──

    #[test]
    fn task_id_generates_unique() {
        let a = TaskId::new();
        let b = TaskId::new();
        assert_ne!(a.as_str(), b.as_str());
    }

    #[test]
    fn task_id_from_str_roundtrip() {
        let id = TaskId::from_existing("abc-123");
        assert_eq!(id.as_str(), "abc-123");
    }

    // ── Point3D ──

    #[test]
    fn point3d_north() {
        let p = Point3D::north();
        assert_relative_eq!(p.z, 1.0, epsilon = 1e-10);
        assert_relative_eq!(p.x, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_from_spherical_north_pole() {
        let p = Point3D::from_spherical(0.0, 0.0);
        assert_relative_eq!(p.z, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_from_spherical_equator() {
        let p = Point3D::from_spherical(FRAC_PI_2, 0.0);
        assert_relative_eq!(p.x, 1.0, epsilon = 1e-10);
        assert_relative_eq!(p.z, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_dot_product() {
        let a = Point3D::new(1.0, 0.0, 0.0);
        let b = Point3D::new(0.0, 1.0, 0.0);
        assert_relative_eq!(a.dot(b), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_dot_self() {
        let p = Point3D::new(1.0, 2.0, 3.0);
        assert_relative_eq!(p.dot(p), 14.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_norm() {
        let p = Point3D::new(3.0, 4.0, 0.0);
        assert_relative_eq!(p.norm(), 5.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_normalized_unit() {
        let p = Point3D::new(3.0, 4.0, 0.0).normalized();
        assert_relative_eq!(p.norm(), 1.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_normalized_zero_returns_north() {
        let p = Point3D::new(0.0, 0.0, 0.0).normalized();
        assert_eq!(p, Point3D::north());
    }

    #[test]
    fn point3d_angular_distance_same() {
        let p = Point3D::north();
        assert_relative_eq!(p.angular_distance(p), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_angular_distance_opposite() {
        let a = Point3D::new(0.0, 0.0, 1.0);
        let b = Point3D::new(0.0, 0.0, -1.0);
        assert_relative_eq!(a.angular_distance(b), PI, epsilon = 1e-10);
    }

    #[test]
    fn point3d_angular_distance_orthogonal() {
        let a = Point3D::new(1.0, 0.0, 0.0);
        let b = Point3D::new(0.0, 1.0, 0.0);
        assert_relative_eq!(a.angular_distance(b), FRAC_PI_2, epsilon = 1e-10);
    }

    #[test]
    fn point3d_slerp_endpoints() {
        let a = Point3D::new(1.0, 0.0, 0.0);
        let b = Point3D::new(0.0, 1.0, 0.0);
        let s0 = a.slerp(b, 0.0);
        let s1 = a.slerp(b, 1.0);
        assert_relative_eq!(s0.x, 1.0, epsilon = 1e-10);
        assert_relative_eq!(s1.y, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_slerp_midpoint() {
        let a = Point3D::new(1.0, 0.0, 0.0);
        let b = Point3D::new(0.0, 1.0, 0.0);
        let mid = a.slerp(b, 0.5);
        assert_relative_eq!(mid.norm(), 1.0, epsilon = 1e-10);
        assert_relative_eq!(mid.x, mid.y, epsilon = 1e-10);
    }

    #[test]
    fn point3d_slerp_same_point() {
        let a = Point3D::new(1.0, 0.0, 0.0);
        let mid = a.slerp(a, 0.5);
        assert_relative_eq!(mid.x, 1.0, epsilon = 1e-10);
    }

    #[test]
    fn point3d_default_is_north() {
        assert_eq!(Point3D::default(), Point3D::north());
    }

    // ── SphereMemory ──

    #[test]
    fn sphere_memory_creation() {
        let m = SphereMemory::new(1, Point3D::north(), "Read".into(), "test".into());
        assert_eq!(m.id, 1);
        assert_relative_eq!(m.activation, 1.0);
        assert!(m.timestamp > 0.0);
    }

    // ── Buoy ──

    #[test]
    fn buoy_creation() {
        let b = Buoy::new(Point3D::north(), "test".into(), 0.1);
        assert_eq!(b.activation_count, 0);
        assert_relative_eq!(b.influence_radius, 0.4);
    }

    #[test]
    fn buoy_boost_at_center() {
        let b = Buoy::new(Point3D::north(), "test".into(), 0.1);
        let boost = b.boost_at(&Point3D::north());
        assert_relative_eq!(boost, b.boost_multiplier, epsilon = 1e-10);
    }

    #[test]
    fn buoy_boost_outside_radius() {
        let b = Buoy::new(Point3D::north(), "test".into(), 0.1);
        let far = Point3D::new(1.0, 0.0, 0.0);
        assert_relative_eq!(b.boost_at(&far), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn buoy_drift_toward() {
        let mut b = Buoy::new(Point3D::new(1.0, 0.0, 0.0), "test".into(), 0.5);
        let target = Point3D::new(0.0, 1.0, 0.0);
        b.drift_toward(&target);
        assert!(b.position.y > 0.0, "buoy should drift toward target");
        assert_eq!(b.activation_count, 1);
    }

    #[test]
    fn buoy_drift_home() {
        let mut b = Buoy::new(Point3D::new(1.0, 0.0, 0.0), "test".into(), 0.1);
        b.position = Point3D::new(0.0, 1.0, 0.0);
        b.drift_home(0.5);
        assert!(b.position.x > 0.0, "buoy should drift toward home");
    }

    // ── PaneStatus ──

    #[test]
    fn pane_status_default_is_idle() {
        assert_eq!(PaneStatus::default(), PaneStatus::Idle);
    }

    #[test]
    fn pane_status_display() {
        assert_eq!(format!("{}", PaneStatus::Working), "Working");
        assert_eq!(format!("{}", PaneStatus::Blocked), "Blocked");
    }

    // ── WorkSignature ──

    #[test]
    fn work_signature_default() {
        let ws = WorkSignature::default();
        assert_relative_eq!(ws.intensity, 0.0);
        assert_relative_eq!(ws.rhythm, 0.0);
    }

    // ── OrderParameter ──

    #[test]
    fn order_parameter_default() {
        let op = OrderParameter::default();
        assert_relative_eq!(op.r, 0.0);
        assert_relative_eq!(op.psi, 0.0);
    }

    // ── FieldAction ──

    #[test]
    fn field_action_default_is_stable() {
        assert_eq!(FieldAction::default(), FieldAction::Stable);
    }

    #[test]
    fn field_action_display() {
        assert_eq!(format!("{}", FieldAction::NeedsCoherence), "NeedsCoherence");
        assert_eq!(
            format!("{}", FieldAction::OverSynchronized),
            "OverSynchronized"
        );
    }

    // ── RTrend ──

    #[test]
    fn r_trend_default_is_stable() {
        assert_eq!(RTrend::default(), RTrend::Stable);
    }

    // ── FleetMode ──

    #[test]
    fn fleet_mode_from_count() {
        assert_eq!(FleetMode::from_count(0), FleetMode::Solo);
        assert_eq!(FleetMode::from_count(1), FleetMode::Solo);
        assert_eq!(FleetMode::from_count(2), FleetMode::Pair);
        assert_eq!(FleetMode::from_count(3), FleetMode::Small);
        assert_eq!(FleetMode::from_count(4), FleetMode::Small);
        assert_eq!(FleetMode::from_count(5), FleetMode::Full);
        assert_eq!(FleetMode::from_count(200), FleetMode::Full);
    }

    #[test]
    fn fleet_mode_name_and_display() {
        assert_eq!(FleetMode::Solo.name(), "Solo");
        assert_eq!(FleetMode::Pair.name(), "Pair");
        assert_eq!(FleetMode::Small.name(), "Small");
        assert_eq!(FleetMode::Full.name(), "Full");
        assert_eq!(format!("{}", FleetMode::Full), "Full");
    }

    // ── phase_diff ──

    #[test]
    fn phase_diff_same() {
        assert_relative_eq!(phase_diff(1.0, 1.0), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn phase_diff_wraps() {
        let d = phase_diff(0.1, TAU - 0.1);
        assert_relative_eq!(d, 0.2, epsilon = 1e-10);
    }

    #[test]
    fn phase_diff_negative() {
        let d = phase_diff(0.0, PI);
        assert_relative_eq!(d.abs(), PI, epsilon = 1e-10);
    }

    #[test]
    fn phase_diff_bounded() {
        for i in 0..100 {
            let a = (i as f64) * 0.1;
            let b = (i as f64) * 0.07 + 1.0;
            let d = phase_diff(a, b);
            assert!(d >= -PI && d <= PI, "phase_diff out of bounds: {d}");
        }
    }

    // ── semantic_phase_region ──

    #[test]
    fn semantic_phase_region_bounded() {
        for name in &["Read", "Write", "Edit", "Bash", "unknown-tool", "Grep", "Agent"] {
            let p = semantic_phase_region(name);
            assert!(p >= 0.0 && p < TAU, "{name} → {p} out of [0, 2π)");
        }
    }

    #[test]
    fn semantic_phase_region_deterministic() {
        let a = semantic_phase_region("Read:file");
        let b = semantic_phase_region("Read:file");
        assert_relative_eq!(a, b);
    }

    #[test]
    fn semantic_phase_region_read_and_write_differ() {
        let r = semantic_phase_region("Read");
        let w = semantic_phase_region("Write");
        assert!((r - w).abs() > 0.1);
    }

    // ── GhostTrace ──

    #[test]
    fn ghost_trace_creation() {
        let ghost = GhostTrace {
            id: PaneId::new("departed"),
            persona: "explorer".into(),
            deregistered_at: 100,
            total_steps_lived: 50,
            memory_count: 10,
            top_tools: vec!["Read".into()],
            phase_at_departure: 1.5,
            receptivity: 0.8,
            work_signature: WorkSignature::default(),
            strongest_neighbors: vec![("peer".into(), 0.9)],
        };
        assert_eq!(ghost.id.as_str(), "departed");
        assert_eq!(ghost.memory_count, 10);
    }

    // ── InboxMessage ──

    #[test]
    fn inbox_message_creation() {
        let msg = InboxMessage {
            id: 1,
            from: "peer".into(),
            content: "hello".into(),
            received_at: now_secs(),
            acknowledged: false,
        };
        assert!(!msg.acknowledged);
    }

    // ── BridgeAdjustments ──

    #[test]
    fn bridge_adjustments_default() {
        let ba = BridgeAdjustments::default();
        assert_relative_eq!(ba.combined_effect, 0.0);
    }

    // ── BridgeStaleness ──

    #[test]
    fn bridge_staleness_default_all_fresh() {
        let bs = BridgeStaleness::default();
        assert!(!bs.synthex_stale());
        assert!(!bs.nexus_stale());
        assert!(!bs.me_stale());
        assert!(!bs.any_stale());
        assert_eq!(bs.stale_count(), 0);
    }

    #[test]
    fn bridge_staleness_set_and_clear() {
        let mut bs = BridgeStaleness::new();
        bs.set_stale(BridgeStaleness::SYNTHEX);
        bs.set_stale(BridgeStaleness::ME);
        assert!(bs.synthex_stale());
        assert!(bs.me_stale());
        assert!(!bs.nexus_stale());
        assert!(bs.any_stale());
        assert_eq!(bs.stale_count(), 2);

        bs.set_fresh(BridgeStaleness::SYNTHEX);
        assert!(!bs.synthex_stale());
        assert_eq!(bs.stale_count(), 1);
    }

    // ── now_secs ──

    #[test]
    fn now_secs_is_positive() {
        assert!(now_secs() > 1_700_000_000.0);
    }

    // ── Serde roundtrips ──

    #[test]
    fn pane_id_serde_roundtrip() {
        let id = PaneId::new("test");
        let json = serde_json::to_string(&id).unwrap();
        let back: PaneId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn point3d_serde_roundtrip() {
        let p = Point3D::new(1.0, 2.0, 3.0);
        let json = serde_json::to_string(&p).unwrap();
        let back: Point3D = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn order_parameter_serde_roundtrip() {
        let op = OrderParameter { r: 0.95, psi: 1.2 };
        let json = serde_json::to_string(&op).unwrap();
        let back: OrderParameter = serde_json::from_str(&json).unwrap();
        assert_relative_eq!(op.r, back.r);
    }

    #[test]
    fn field_action_serde_roundtrip() {
        let action = FieldAction::OverSynchronized;
        let json = serde_json::to_string(&action).unwrap();
        let back: FieldAction = serde_json::from_str(&json).unwrap();
        assert_eq!(action, back);
    }

    #[test]
    fn fleet_mode_serde_roundtrip() {
        let mode = FleetMode::Full;
        let json = serde_json::to_string(&mode).unwrap();
        let back: FleetMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }

    #[test]
    fn sphere_memory_serde_roundtrip() {
        let m = SphereMemory::new(42, Point3D::north(), "Read".into(), "test".into());
        let json = serde_json::to_string(&m).unwrap();
        let back: SphereMemory = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 42);
    }

    #[test]
    fn buoy_serde_roundtrip() {
        let b = Buoy::new(Point3D::north(), "cluster".into(), 0.1);
        let json = serde_json::to_string(&b).unwrap();
        let back: Buoy = serde_json::from_str(&json).unwrap();
        assert_eq!(back.label, "cluster");
    }
}
