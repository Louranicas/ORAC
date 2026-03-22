//! # M37: Emergence Detector
//!
//! Detects emergent fleet coordination behaviors that cannot be predicted from
//! individual pane states alone. Uses a ring buffer with TTL-based decay and
//! a 5,000-event cap to track emergence patterns over time.
//!
//! ## Layer: L8 (Evolution)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//!
//! ## Emergence Types (ORAC Fleet)
//!
//! | Type | Detection Criteria |
//! |------|--------------------|
//! | `CoherenceLock` | r > 0.998 sustained for >= threshold ticks |
//! | `ChimeraFormation` | Phase gap > π/3 with r still above sync threshold |
//! | `CouplingRunaway` | K increasing without r improvement |
//! | `HebbianSaturation` | >80% of weights at floor or ceiling |
//! | `DispatchLoop` | Same task dispatched to same pane >=3 times |
//! | `ThermalSpike` | Temperature exceeds damping capacity |
//! | `BeneficialSync` | Fleet spontaneously reaches r > 0.95 |
//! | `ConsentCascade` | Multiple spheres opt out within short window |

use std::collections::{HashMap, VecDeque};
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Maximum emergence records retained.
const DEFAULT_HISTORY_CAPACITY: usize = 5000;

/// Default TTL for emergence records (ticks before decay removal).
const DEFAULT_TTL_TICKS: u64 = 600;

/// Default minimum confidence to register an emergence.
const DEFAULT_MIN_CONFIDENCE: f64 = 0.6;

/// Maximum active monitors.
const MAX_MONITORS: usize = 50;

/// Default coherence lock threshold (r value).
const DEFAULT_COHERENCE_LOCK_R: f64 = 0.998;

/// Default coherence lock duration (ticks).
const DEFAULT_COHERENCE_LOCK_TICKS: u64 = 10;

/// Default coupling runaway K increase without r improvement (ticks).
const DEFAULT_RUNAWAY_WINDOW: u64 = 20;

/// Hebbian weight saturation threshold (fraction of weights at floor/ceiling).
const DEFAULT_SATURATION_RATIO: f64 = 0.8;

// ──────────────────────────────────────────────────────────────
// Enums
// ──────────────────────────────────────────────────────────────

/// Classification of emergent fleet coordination behaviors.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergenceType {
    /// r > 0.998 sustained — field is over-synchronized, no differentiation.
    CoherenceLock,
    /// Phase clusters form with gap > π/3 while r remains above sync threshold.
    ChimeraFormation,
    /// K increasing without corresponding r improvement — coupling runaway.
    CouplingRunaway,
    /// >80% of Hebbian weights pinned at floor or ceiling.
    HebbianSaturation,
    /// Same task dispatched to same pane repeatedly — dispatch loop.
    DispatchLoop,
    /// Temperature exceeds thermal damping capacity.
    ThermalSpike,
    /// Fleet spontaneously reaches high coherence (r > 0.95).
    BeneficialSync,
    /// Multiple spheres opt out of coupling within short window.
    ConsentCascade,
}

impl fmt::Display for EmergenceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CoherenceLock => f.write_str("coherence_lock"),
            Self::ChimeraFormation => f.write_str("chimera_formation"),
            Self::CouplingRunaway => f.write_str("coupling_runaway"),
            Self::HebbianSaturation => f.write_str("hebbian_saturation"),
            Self::DispatchLoop => f.write_str("dispatch_loop"),
            Self::ThermalSpike => f.write_str("thermal_spike"),
            Self::BeneficialSync => f.write_str("beneficial_sync"),
            Self::ConsentCascade => f.write_str("consent_cascade"),
        }
    }
}

/// Severity classification for emergence events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EmergenceSeverity {
    /// Low impact — informational only.
    Low,
    /// Medium impact — may need attention.
    Medium,
    /// High impact — active intervention recommended.
    High,
    /// Critical — immediate response needed.
    Critical,
}

impl EmergenceSeverity {
    /// Classify severity from a numeric score [0.0, 1.0].
    #[must_use]
    pub fn from_score(score: f64) -> Self {
        if score >= 0.9 {
            Self::Critical
        } else if score >= 0.7 {
            Self::High
        } else if score >= 0.4 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

impl fmt::Display for EmergenceSeverity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => f.write_str("low"),
            Self::Medium => f.write_str("medium"),
            Self::High => f.write_str("high"),
            Self::Critical => f.write_str("critical"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Data structures
// ──────────────────────────────────────────────────────────────

/// A detected emergent behavior record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceRecord {
    /// Unique record ID.
    pub id: u64,
    /// Classification of the detected emergence.
    pub emergence_type: EmergenceType,
    /// Detection confidence [0.0, 1.0].
    pub confidence: f64,
    /// Severity score [0.0, 1.0].
    pub severity: f64,
    /// Severity classification.
    pub severity_class: EmergenceSeverity,
    /// Pane IDs affected by this emergence.
    pub affected_panes: Vec<String>,
    /// Human-readable description.
    pub description: String,
    /// Tick at which this emergence was detected.
    pub detected_at_tick: u64,
    /// TTL remaining (decremented each tick, removed at 0).
    pub ttl: u64,
    /// Optional recommended action.
    pub recommended_action: Option<String>,
}

/// An evidence observation contributing to emergence detection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceEvidence {
    /// What was observed.
    pub observation: String,
    /// Numeric value associated with the observation.
    pub value: f64,
    /// Tick when observed.
    pub tick: u64,
}

/// An active monitor tracking accumulating evidence for one emergence type.
#[derive(Clone, Debug)]
pub struct EmergenceMonitor {
    /// Monitor ID (sequential).
    pub id: u64,
    /// The behavior type being watched.
    pub behavior_type: EmergenceType,
    /// Accumulated evidence.
    pub evidence: Vec<EmergenceEvidence>,
    /// Current accumulated confidence [0.0, 1.0].
    pub confidence: f64,
    /// Tick when this monitor was created.
    pub created_at_tick: u64,
    /// Whether this monitor has fired (triggered emergence).
    pub fired: bool,
}

/// Configuration for the `EmergenceDetector`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EmergenceDetectorConfig {
    /// Maximum emergence records retained (cap 5,000).
    pub history_capacity: usize,
    /// TTL for emergence records (ticks).
    pub ttl_ticks: u64,
    /// Minimum confidence to register an emergence.
    pub min_confidence: f64,
    /// r threshold for coherence lock detection.
    pub coherence_lock_r: f64,
    /// Sustained ticks for coherence lock.
    pub coherence_lock_ticks: u64,
    /// K runaway detection window (ticks).
    pub runaway_window: u64,
    /// Hebbian weight saturation ratio.
    pub saturation_ratio: f64,
}

impl Default for EmergenceDetectorConfig {
    fn default() -> Self {
        Self {
            history_capacity: DEFAULT_HISTORY_CAPACITY,
            ttl_ticks: DEFAULT_TTL_TICKS,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            coherence_lock_r: DEFAULT_COHERENCE_LOCK_R,
            coherence_lock_ticks: DEFAULT_COHERENCE_LOCK_TICKS,
            runaway_window: DEFAULT_RUNAWAY_WINDOW,
            saturation_ratio: DEFAULT_SATURATION_RATIO,
        }
    }
}

/// Aggregate statistics for the `EmergenceDetector`.
#[derive(Clone, Debug, Default)]
pub struct EmergenceStats {
    /// Total emergence records detected.
    pub total_detected: u64,
    /// Count by emergence type.
    pub by_type: HashMap<String, u64>,
    /// Count by severity class.
    pub by_severity: HashMap<String, u64>,
    /// Number of active monitors.
    pub active_monitors: usize,
    /// Total tick decay passes executed.
    pub decay_passes: u64,
    /// Total records expired by TTL.
    pub total_expired: u64,
}

/// Parameters for recording an emergence detection.
///
/// Used to avoid excessive function argument counts in [`EmergenceDetector::record_emergence`].
#[derive(Clone, Debug)]
pub struct EmergenceParams {
    /// Classification of the detected emergence.
    pub emergence_type: EmergenceType,
    /// Detection confidence [0.0, 1.0].
    pub confidence: f64,
    /// Severity score [0.0, 1.0].
    pub severity: f64,
    /// Pane IDs affected by this emergence.
    pub affected_panes: Vec<String>,
    /// Human-readable description.
    pub description: String,
    /// Tick at which this emergence was detected.
    pub tick: u64,
    /// Optional recommended action.
    pub recommended_action: Option<String>,
}

// ──────────────────────────────────────────────────────────────
// EmergenceDetector
// ──────────────────────────────────────────────────────────────

/// Emergence detector for ORAC fleet coordination.
///
/// Maintains a ring buffer of emergence records with TTL-based decay,
/// capped at 5,000 records. Monitors accumulate evidence and fire
/// when confidence exceeds the threshold.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct EmergenceDetector {
    /// Detected emergence history (ring buffer, FIFO eviction).
    history: RwLock<VecDeque<EmergenceRecord>>,
    /// Active monitors keyed by monitor ID.
    monitors: RwLock<HashMap<u64, EmergenceMonitor>>,
    /// Monotonically increasing record ID counter.
    next_record_id: RwLock<u64>,
    /// Monotonically increasing monitor ID counter.
    next_monitor_id: RwLock<u64>,
    /// Configuration.
    config: EmergenceDetectorConfig,
    /// Aggregate statistics.
    stats: RwLock<EmergenceStats>,
}

impl fmt::Debug for EmergenceDetector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmergenceDetector")
            .field("history_len", &self.history.read().len())
            .field("monitors", &self.monitors.read().len())
            .finish_non_exhaustive()
    }
}

impl EmergenceDetector {
    /// Creates a new `EmergenceDetector` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(EmergenceDetectorConfig::default())
    }

    /// Creates a new `EmergenceDetector` with the given configuration.
    #[must_use]
    pub fn with_config(config: EmergenceDetectorConfig) -> Self {
        Self {
            history: RwLock::new(VecDeque::with_capacity(
                config.history_capacity.min(5000),
            )),
            monitors: RwLock::new(HashMap::new()),
            next_record_id: RwLock::new(1),
            next_monitor_id: RwLock::new(1),
            config,
            stats: RwLock::new(EmergenceStats::default()),
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if any parameter is invalid.
    pub fn validate_config(config: &EmergenceDetectorConfig) -> PvResult<()> {
        if config.history_capacity == 0 {
            return Err(PvError::ConfigValidation(
                "history_capacity must be > 0".into(),
            ));
        }
        if config.ttl_ticks == 0 {
            return Err(PvError::ConfigValidation(
                "ttl_ticks must be > 0".into(),
            ));
        }
        if config.min_confidence < 0.0 || config.min_confidence > 1.0 {
            return Err(PvError::ConfigValidation(
                "min_confidence must be in [0.0, 1.0]".into(),
            ));
        }
        if config.coherence_lock_r < 0.0 || config.coherence_lock_r > 1.0 {
            return Err(PvError::ConfigValidation(
                "coherence_lock_r must be in [0.0, 1.0]".into(),
            ));
        }
        if config.coherence_lock_ticks == 0 {
            return Err(PvError::ConfigValidation(
                "coherence_lock_ticks must be > 0".into(),
            ));
        }
        Ok(())
    }

    /// Record an emergence detection.
    ///
    /// The record is added to the ring buffer (evicting oldest if at capacity).
    /// Records below `min_confidence` are silently dropped.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if confidence or severity are non-finite.
    pub fn record_emergence(
        &self,
        params: &EmergenceParams,
    ) -> PvResult<Option<u64>> {
        if !params.confidence.is_finite() || !params.severity.is_finite() {
            let bad_val = if params.confidence.is_finite() {
                params.severity
            } else {
                params.confidence
            };
            return Err(PvError::NonFinite {
                field: "confidence_or_severity",
                value: bad_val,
            });
        }

        let confidence = params.confidence.clamp(0.0, 1.0);
        let severity = params.severity.clamp(0.0, 1.0);

        if confidence < self.config.min_confidence {
            return Ok(None);
        }

        let id = {
            let mut counter = self.next_record_id.write();
            let id = *counter;
            *counter += 1;
            id
        };

        let severity_class = EmergenceSeverity::from_score(severity);

        let record = EmergenceRecord {
            id,
            emergence_type: params.emergence_type,
            confidence,
            severity,
            severity_class,
            affected_panes: params.affected_panes.clone(),
            description: params.description.clone(),
            detected_at_tick: params.tick,
            ttl: self.config.ttl_ticks,
            recommended_action: params.recommended_action.clone(),
        };

        {
            let mut hist = self.history.write();
            if hist.len() >= self.config.history_capacity {
                hist.pop_front();
            }
            hist.push_back(record);
        }

        // Update stats
        {
            let mut detector_stats = self.stats.write();
            detector_stats.total_detected += 1;
            *detector_stats
                .by_type
                .entry(params.emergence_type.to_string())
                .or_insert(0) += 1;
            *detector_stats
                .by_severity
                .entry(severity_class.to_string())
                .or_insert(0) += 1;
        }

        Ok(Some(id))
    }

    /// Detect coherence lock: r > threshold for sustained ticks.
    ///
    /// Call with the recent r history. Returns a record ID if detected.
    ///
    /// # Errors
    /// Returns [`PvError`] on invalid input.
    pub fn detect_coherence_lock(
        &self,
        r_history: &[f64],
        tick: u64,
    ) -> PvResult<Option<u64>> {
        let threshold = self.config.coherence_lock_r;
        let required_ticks = usize::try_from(self.config.coherence_lock_ticks).unwrap_or(usize::MAX);

        if r_history.len() < required_ticks {
            return Ok(None);
        }

        let tail = &r_history[r_history.len() - required_ticks..];
        let all_locked = tail.iter().all(|&r| r > threshold);

        if all_locked {
            #[allow(clippy::cast_precision_loss)] // tail.len() is bounded by coherence_lock_ticks (small)
            let tail_len_f = tail.len() as f64;
            let avg_r = tail.iter().sum::<f64>() / tail_len_f;
            let confidence = ((avg_r - threshold) / (1.0 - threshold)).clamp(0.0, 1.0);
            self.record_emergence(&EmergenceParams {
                emergence_type: EmergenceType::CoherenceLock,
                confidence: 0.6f64.mul_add(confidence, 0.4),
                severity: 0.7,
                affected_panes: Vec::new(),
                description: format!("r > {threshold:.3} for {required_ticks} ticks (avg r = {avg_r:.4})"),
                tick,
                recommended_action: Some("Reduce K to allow phase differentiation".into()),
            })
        } else {
            Ok(None)
        }
    }

    /// Detect chimera formation from phase data.
    ///
    /// A chimera is detected when distinct phase clusters exist with gap > π/3.
    ///
    /// # Errors
    /// Returns [`PvError`] on invalid input.
    pub fn detect_chimera(
        &self,
        phases: &[f64],
        r: f64,
        tick: u64,
    ) -> PvResult<Option<u64>> {
        if phases.len() < 3 {
            return Ok(None);
        }

        let mut sorted = phases.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let gap_threshold = std::f64::consts::FRAC_PI_3;
        let mut max_gap = 0.0_f64;
        for i in 1..sorted.len() {
            let gap = sorted[i] - sorted[i - 1];
            if gap > max_gap {
                max_gap = gap;
            }
        }
        // Also check wrap-around gap
        if let (Some(&first), Some(&last)) = (sorted.first(), sorted.last()) {
            let wrap_gap = std::f64::consts::TAU - last + first;
            if wrap_gap > max_gap {
                max_gap = wrap_gap;
            }
        }

        if max_gap > gap_threshold && r > 0.3 {
            let confidence = (max_gap / std::f64::consts::PI).clamp(0.0, 1.0);
            self.record_emergence(&EmergenceParams {
                emergence_type: EmergenceType::ChimeraFormation,
                confidence,
                severity: 0.5,
                affected_panes: Vec::new(),
                description: format!("Phase gap {max_gap:.3} rad with r = {r:.3}"),
                tick,
                recommended_action: Some("Monitor — chimeras can be beneficial".into()),
            })
        } else {
            Ok(None)
        }
    }

    /// Detect coupling runaway: K increasing without r improvement.
    ///
    /// # Errors
    /// Returns [`PvError`] on invalid input.
    pub fn detect_coupling_runaway(
        &self,
        k_history: &[f64],
        r_history: &[f64],
        tick: u64,
    ) -> PvResult<Option<u64>> {
        let window = usize::try_from(self.config.runaway_window).unwrap_or(usize::MAX);

        if k_history.len() < window || r_history.len() < window {
            return Ok(None);
        }

        let k_tail = &k_history[k_history.len() - window..];
        let r_tail = &r_history[r_history.len() - window..];

        let k_slope = linear_regression_slope(k_tail);
        let r_slope = linear_regression_slope(r_tail);

        // K rising but r flat or falling
        if k_slope > 0.01 && r_slope < 0.005 {
            let confidence = (k_slope * 10.0).clamp(0.0, 1.0);
            self.record_emergence(&EmergenceParams {
                emergence_type: EmergenceType::CouplingRunaway,
                confidence,
                severity: 0.8,
                affected_panes: Vec::new(),
                description: format!("K slope {k_slope:.4}, r slope {r_slope:.4} over {window} ticks"),
                tick,
                recommended_action: Some("Clamp K; investigate why coupling is ineffective".into()),
            })
        } else {
            Ok(None)
        }
    }

    /// Detect Hebbian weight saturation.
    ///
    /// # Errors
    /// Returns [`PvError`] on invalid input.
    pub fn detect_hebbian_saturation(
        &self,
        weights: &[f64],
        floor: f64,
        ceiling: f64,
        tick: u64,
    ) -> PvResult<Option<u64>> {
        if weights.is_empty() {
            return Ok(None);
        }

        let saturated = weights
            .iter()
            .filter(|&&w| (w - floor).abs() < 0.01 || (w - ceiling).abs() < 0.01)
            .count();

        #[allow(clippy::cast_precision_loss)] // counts bounded by weight array size
        let ratio = saturated as f64 / weights.len() as f64;

        if ratio >= self.config.saturation_ratio {
            self.record_emergence(&EmergenceParams {
                emergence_type: EmergenceType::HebbianSaturation,
                confidence: ratio,
                severity: 0.6,
                affected_panes: Vec::new(),
                description: format!(
                    "{saturated}/{} weights at floor ({floor:.2}) or ceiling ({ceiling:.2})",
                    weights.len()
                ),
                tick,
                recommended_action: Some("Adjust STDP rates or add weight randomization".into()),
            })
        } else {
            Ok(None)
        }
    }

    /// Detect a thermal spike.
    ///
    /// # Errors
    /// Returns [`PvError`] on invalid input.
    pub fn detect_thermal_spike(
        &self,
        temperature: f64,
        damping_capacity: f64,
        tick: u64,
    ) -> PvResult<Option<u64>> {
        if temperature > damping_capacity && damping_capacity > 0.0 {
            let severity = ((temperature - damping_capacity) / damping_capacity).clamp(0.0, 1.0);
            self.record_emergence(&EmergenceParams {
                emergence_type: EmergenceType::ThermalSpike,
                confidence: 0.9,
                severity,
                affected_panes: Vec::new(),
                description: format!("Temperature {temperature:.2} exceeds damping capacity {damping_capacity:.2}"),
                tick,
                recommended_action: Some("Throttle dispatch; allow cooling period".into()),
            })
        } else {
            Ok(None)
        }
    }

    /// Detect beneficial spontaneous synchronization.
    ///
    /// # Errors
    /// Returns [`PvError`] on invalid input.
    pub fn detect_beneficial_sync(
        &self,
        r: f64,
        previous_r: f64,
        tick: u64,
    ) -> PvResult<Option<u64>> {
        if r > 0.95 && previous_r < 0.8 {
            let improvement = r - previous_r;
            // Scale confidence: a jump of 0.15+ is high confidence
            let confidence = (improvement / 0.15).clamp(0.0, 1.0);
            self.record_emergence(&EmergenceParams {
                emergence_type: EmergenceType::BeneficialSync,
                confidence,
                severity: 0.2,
                affected_panes: Vec::new(),
                description: format!("r jumped from {previous_r:.3} to {r:.3}"),
                tick,
                recommended_action: None,
            })
        } else {
            Ok(None)
        }
    }

    /// Start a new emergence monitor for a specific behavior type.
    ///
    /// Returns the monitor ID.
    pub fn start_monitor(&self, behavior_type: EmergenceType, tick: u64) -> u64 {
        let id = {
            let mut counter = self.next_monitor_id.write();
            let id = *counter;
            *counter += 1;
            id
        };

        let monitor = EmergenceMonitor {
            id,
            behavior_type,
            evidence: Vec::new(),
            confidence: 0.0,
            created_at_tick: tick,
            fired: false,
        };

        let mut monitors = self.monitors.write();
        // Enforce cap
        if monitors.len() >= MAX_MONITORS {
            // Remove oldest unfired monitor
            let oldest_key = monitors
                .iter()
                .filter(|(_, m)| !m.fired)
                .min_by_key(|(_, m)| m.created_at_tick)
                .map(|(&k, _)| k);
            if let Some(key) = oldest_key {
                monitors.remove(&key);
            }
        }
        monitors.insert(id, monitor);

        id
    }

    /// Add evidence to a monitor.
    ///
    /// # Errors
    /// Returns [`PvError::Internal`] if the monitor ID is not found.
    pub fn add_evidence(
        &self,
        monitor_id: u64,
        observation: String,
        value: f64,
        tick: u64,
    ) -> PvResult<()> {
        let mut monitors = self.monitors.write();
        let monitor = monitors
            .get_mut(&monitor_id)
            .ok_or_else(|| PvError::Internal(format!("monitor {monitor_id} not found")))?;

        monitor.evidence.push(EmergenceEvidence {
            observation,
            value,
            tick,
        });

        // Recompute confidence from evidence count + values
        #[allow(clippy::cast_precision_loss)] // evidence count is small (monitor cap 50)
        let n = monitor.evidence.len() as f64;
        let avg_value: f64 = monitor.evidence.iter().map(|e| e.value).sum::<f64>() / n;
        monitor.confidence = (n / 10.0).min(1.0) * avg_value.clamp(0.0, 1.0);

        Ok(())
    }

    /// Check a monitor and fire emergence if confidence exceeds threshold.
    ///
    /// # Errors
    /// Returns [`PvError`] on internal errors.
    pub fn check_monitor(&self, monitor_id: u64, tick: u64) -> PvResult<Option<u64>> {
        let (should_fire, behavior_type, confidence) = {
            let monitors = self.monitors.read();
            let monitor = monitors
                .get(&monitor_id)
                .ok_or_else(|| PvError::Internal(format!("monitor {monitor_id} not found")))?;

            if monitor.fired {
                return Ok(None);
            }

            (
                monitor.confidence >= self.config.min_confidence,
                monitor.behavior_type,
                monitor.confidence,
            )
        };

        if should_fire {
            // Mark as fired
            {
                let mut monitors = self.monitors.write();
                if let Some(m) = monitors.get_mut(&monitor_id) {
                    m.fired = true;
                }
            }

            self.record_emergence(&EmergenceParams {
                emergence_type: behavior_type,
                confidence,
                severity: 0.5,
                affected_panes: Vec::new(),
                description: format!("Monitor {monitor_id} fired for {behavior_type}"),
                tick,
                recommended_action: None,
            })
        } else {
            Ok(None)
        }
    }

    /// Tick: decay TTLs and remove expired records.
    pub fn tick_decay(&self) {
        let mut hist = self.history.write();
        let before = hist.len();

        // Decrement TTLs
        for record in hist.iter_mut() {
            record.ttl = record.ttl.saturating_sub(1);
        }

        // Remove expired
        hist.retain(|r| r.ttl > 0);

        let expired = before - hist.len();

        drop(hist);

        let mut stats = self.stats.write();
        stats.decay_passes += 1;
        stats.total_expired += expired as u64;
        stats.active_monitors = self.monitors.read().len();
    }

    /// Get the number of emergence records in history.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.read().len()
    }

    /// Get recent emergence records (up to `limit`).
    #[must_use]
    pub fn recent(&self, limit: usize) -> Vec<EmergenceRecord> {
        let hist = self.history.read();
        hist.iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get emergence records filtered by type.
    #[must_use]
    pub fn by_type(&self, emergence_type: EmergenceType) -> Vec<EmergenceRecord> {
        self.history
            .read()
            .iter()
            .filter(|r| r.emergence_type == emergence_type)
            .cloned()
            .collect()
    }

    /// Count records by type in current history.
    #[must_use]
    pub fn type_counts(&self) -> HashMap<EmergenceType, usize> {
        let hist = self.history.read();
        let mut counts = HashMap::new();
        for record in hist.iter() {
            *counts.entry(record.emergence_type).or_insert(0) += 1;
        }
        counts
    }

    /// Get aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> EmergenceStats {
        self.stats.read().clone()
    }

    /// Get the count of active monitors.
    #[must_use]
    pub fn active_monitor_count(&self) -> usize {
        self.monitors.read().len()
    }

    /// Clear all history, monitors, and reset stats.
    pub fn reset(&self) {
        self.history.write().clear();
        self.monitors.write().clear();
        *self.stats.write() = EmergenceStats::default();
    }
}

impl Default for EmergenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Helper
// ──────────────────────────────────────────────────────────────

/// Simple linear regression slope for evenly-spaced samples.
#[allow(clippy::cast_precision_loss)] // indices and sample counts are small
fn linear_regression_slope(samples: &[f64]) -> f64 {
    let n = samples.len();
    if n < 2 {
        return 0.0;
    }

    let nf = n as f64;
    let mut sum_x = 0.0_f64;
    let mut cross_sum = 0.0_f64;
    let mut sum_y = 0.0_f64;
    let mut sum_x2 = 0.0_f64;

    for (i, &y) in samples.iter().enumerate() {
        let x = i as f64;
        sum_x += x;
        sum_y += y;
        cross_sum += x * y;
        sum_x2 += x * x;
    }

    let denom = nf.mul_add(sum_x2, -(sum_x * sum_x));
    if denom.abs() < f64::EPSILON {
        return 0.0;
    }

    nf.mul_add(cross_sum, -(sum_x * sum_y)) / denom
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_detector() -> EmergenceDetector {
        EmergenceDetector::new()
    }

    #[test]
    fn default_config_valid() {
        assert!(EmergenceDetector::validate_config(&EmergenceDetectorConfig::default()).is_ok());
    }

    #[test]
    fn config_zero_history_invalid() {
        let config = EmergenceDetectorConfig {
            history_capacity: 0,
            ..Default::default()
        };
        assert!(EmergenceDetector::validate_config(&config).is_err());
    }

    #[test]
    fn config_zero_ttl_invalid() {
        let config = EmergenceDetectorConfig {
            ttl_ticks: 0,
            ..Default::default()
        };
        assert!(EmergenceDetector::validate_config(&config).is_err());
    }

    #[test]
    fn config_bad_confidence_invalid() {
        let config = EmergenceDetectorConfig {
            min_confidence: 1.5,
            ..Default::default()
        };
        assert!(EmergenceDetector::validate_config(&config).is_err());
    }

    /// Helper to construct `EmergenceParams` concisely in tests.
    fn ep(
        emergence_type: EmergenceType,
        confidence: f64,
        severity: f64,
        affected_panes: Vec<String>,
        description: impl Into<String>,
        tick: u64,
        recommended_action: Option<String>,
    ) -> EmergenceParams {
        EmergenceParams {
            emergence_type,
            confidence,
            severity,
            affected_panes,
            description: description.into(),
            tick,
            recommended_action,
        }
    }

    #[test]
    fn record_emergence_basic() {
        let det = make_detector();
        let id = det
            .record_emergence(&ep(
                EmergenceType::ThermalSpike,
                0.8,
                0.5,
                vec!["pane-1".into()],
                "test",
                10,
                None,
            ))
            .unwrap();
        assert!(id.is_some());
        assert_eq!(det.history_len(), 1);
    }

    #[test]
    fn record_below_confidence_dropped() {
        let det = make_detector();
        let id = det
            .record_emergence(&ep(
                EmergenceType::ThermalSpike,
                0.1, // Below default 0.6
                0.5,
                Vec::new(),
                "low conf",
                1,
                None,
            ))
            .unwrap();
        assert!(id.is_none());
        assert_eq!(det.history_len(), 0);
    }

    #[test]
    fn record_nan_rejected() {
        let det = make_detector();
        assert!(det
            .record_emergence(&ep(
                EmergenceType::ThermalSpike,
                f64::NAN,
                0.5,
                Vec::new(),
                "nan",
                1,
                None,
            ))
            .is_err());
    }

    #[test]
    fn history_bounded() {
        let config = EmergenceDetectorConfig {
            history_capacity: 3,
            min_confidence: 0.0, // Accept everything
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        for i in 0..5 {
            det.record_emergence(&ep(
                EmergenceType::BeneficialSync,
                0.9,
                0.5,
                Vec::new(),
                format!("record {i}"),
                i,
                None,
            ))
            .unwrap();
        }
        assert_eq!(det.history_len(), 3);
    }

    #[test]
    fn ttl_decay_removes_expired() {
        let config = EmergenceDetectorConfig {
            ttl_ticks: 2,
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        det.record_emergence(&ep(
            EmergenceType::ThermalSpike,
            0.9,
            0.5,
            Vec::new(),
            "test",
            1,
            None,
        ))
        .unwrap();

        assert_eq!(det.history_len(), 1);
        det.tick_decay(); // TTL 2 → 1
        assert_eq!(det.history_len(), 1);
        det.tick_decay(); // TTL 1 → 0, removed
        assert_eq!(det.history_len(), 0);
    }

    #[test]
    fn ttl_decay_stats_updated() {
        let config = EmergenceDetectorConfig {
            ttl_ticks: 1,
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        det.record_emergence(&ep(
            EmergenceType::ThermalSpike,
            0.9,
            0.5,
            Vec::new(),
            "test",
            1,
            None,
        ))
        .unwrap();

        det.tick_decay();
        let stats = det.stats();
        assert_eq!(stats.decay_passes, 1);
        assert_eq!(stats.total_expired, 1);
    }

    #[test]
    fn detect_coherence_lock_triggered() {
        let det = make_detector();
        // 10 ticks of r > 0.998
        let r_history: Vec<f64> = vec![0.999; 10];
        let result = det.detect_coherence_lock(&r_history, 100).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn detect_coherence_lock_not_triggered() {
        let det = make_detector();
        let r_history: Vec<f64> = vec![0.95; 10]; // Below 0.998
        let result = det.detect_coherence_lock(&r_history, 100).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_coherence_lock_insufficient_data() {
        let det = make_detector();
        let r_history: Vec<f64> = vec![0.999; 5]; // Less than 10
        let result = det.detect_coherence_lock(&r_history, 100).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_chimera_triggered() {
        let det = make_detector();
        // Two clusters separated by > π/3
        let phases = vec![0.1, 0.2, 0.15, 2.0, 2.1, 2.05];
        let result = det.detect_chimera(&phases, 0.7, 50).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn detect_chimera_not_triggered_small_gap() {
        let det = make_detector();
        // Phases evenly distributed around the circle — many small gaps, no gap > π/3
        use std::f64::consts::TAU;
        let n = 8;
        let phases: Vec<f64> = (0..n).map(|i| i as f64 * TAU / n as f64).collect();
        let result = det.detect_chimera(&phases, 0.7, 50).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_chimera_insufficient_phases() {
        let det = make_detector();
        let phases = vec![0.1, 0.2]; // Need >= 3
        let result = det.detect_chimera(&phases, 0.7, 50).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_coupling_runaway_triggered() {
        let det = make_detector();
        // K rising steeply, r flat
        let k: Vec<f64> = (0..20).map(|i| 1.0 + i as f64 * 0.2).collect();
        let r: Vec<f64> = vec![0.5; 20];
        let result = det.detect_coupling_runaway(&k, &r, 100).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn detect_coupling_runaway_not_triggered() {
        let det = make_detector();
        // K flat, r flat
        let k: Vec<f64> = vec![1.0; 20];
        let r: Vec<f64> = vec![0.5; 20];
        let result = det.detect_coupling_runaway(&k, &r, 100).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_coupling_runaway_insufficient_data() {
        let det = make_detector();
        let k: Vec<f64> = vec![1.0; 5]; // Less than window of 20
        let r: Vec<f64> = vec![0.5; 5];
        let result = det.detect_coupling_runaway(&k, &r, 100).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_hebbian_saturation_triggered() {
        let det = make_detector();
        // 9 out of 10 at floor
        let weights = vec![0.15, 0.15, 0.15, 0.15, 0.15, 0.15, 0.15, 0.15, 0.15, 0.5];
        let result = det
            .detect_hebbian_saturation(&weights, 0.15, 1.0, 50)
            .unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn detect_hebbian_saturation_not_triggered() {
        let det = make_detector();
        let weights = vec![0.3, 0.4, 0.5, 0.6, 0.7];
        let result = det
            .detect_hebbian_saturation(&weights, 0.15, 1.0, 50)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_hebbian_saturation_empty() {
        let det = make_detector();
        let result = det
            .detect_hebbian_saturation(&[], 0.15, 1.0, 50)
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_thermal_spike_triggered() {
        let det = make_detector();
        let result = det.detect_thermal_spike(1.5, 1.0, 50).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn detect_thermal_spike_not_triggered() {
        let det = make_detector();
        let result = det.detect_thermal_spike(0.8, 1.0, 50).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_beneficial_sync_triggered() {
        let det = make_detector();
        let result = det.detect_beneficial_sync(0.97, 0.6, 50).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn detect_beneficial_sync_not_triggered() {
        let det = make_detector();
        let result = det.detect_beneficial_sync(0.85, 0.6, 50).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn detect_beneficial_sync_already_high() {
        let det = make_detector();
        let result = det.detect_beneficial_sync(0.97, 0.95, 50).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn monitor_lifecycle() {
        let det = make_detector();
        let mid = det.start_monitor(EmergenceType::DispatchLoop, 1);
        assert_eq!(det.active_monitor_count(), 1);

        det.add_evidence(mid, "loop detected".into(), 0.8, 2).unwrap();
        det.add_evidence(mid, "loop repeated".into(), 0.9, 3).unwrap();

        // Not yet enough confidence
        let result = det.check_monitor(mid, 4).unwrap();
        assert!(result.is_none());

        // Add more evidence to push over threshold
        for i in 4..15 {
            det.add_evidence(mid, format!("loop {i}"), 0.9, i as u64)
                .unwrap();
        }

        let result = det.check_monitor(mid, 15).unwrap();
        assert!(result.is_some());

        // Already fired — should return None
        let result = det.check_monitor(mid, 16).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn monitor_cap_enforced() {
        let det = make_detector();
        for i in 0..MAX_MONITORS + 5 {
            det.start_monitor(EmergenceType::ThermalSpike, i as u64);
        }
        assert!(det.active_monitor_count() <= MAX_MONITORS);
    }

    #[test]
    fn add_evidence_unknown_monitor() {
        let det = make_detector();
        assert!(det.add_evidence(999, "test".into(), 0.5, 1).is_err());
    }

    #[test]
    fn check_monitor_unknown() {
        let det = make_detector();
        assert!(det.check_monitor(999, 1).is_err());
    }

    #[test]
    fn recent_returns_newest_first() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        for i in 0..5 {
            det.record_emergence(&ep(
                EmergenceType::BeneficialSync,
                0.9,
                0.3,
                Vec::new(),
                format!("record {i}"),
                i as u64,
                None,
            ))
            .unwrap();
        }

        let recent = det.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].detected_at_tick, 4);
        assert_eq!(recent[2].detected_at_tick, 2);
    }

    #[test]
    fn by_type_filters_correctly() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        det.record_emergence(&ep(EmergenceType::ThermalSpike, 0.9, 0.5, Vec::new(), "a", 1, None)).unwrap();
        det.record_emergence(&ep(EmergenceType::BeneficialSync, 0.8, 0.3, Vec::new(), "b", 2, None)).unwrap();
        det.record_emergence(&ep(EmergenceType::ThermalSpike, 0.7, 0.6, Vec::new(), "c", 3, None)).unwrap();

        let spikes = det.by_type(EmergenceType::ThermalSpike);
        assert_eq!(spikes.len(), 2);
    }

    #[test]
    fn type_counts_correct() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        det.record_emergence(&ep(EmergenceType::ThermalSpike, 0.9, 0.5, Vec::new(), "a", 1, None)).unwrap();
        det.record_emergence(&ep(EmergenceType::ThermalSpike, 0.8, 0.5, Vec::new(), "b", 2, None)).unwrap();
        det.record_emergence(&ep(EmergenceType::ChimeraFormation, 0.7, 0.5, Vec::new(), "c", 3, None)).unwrap();

        let counts = det.type_counts();
        assert_eq!(counts[&EmergenceType::ThermalSpike], 2);
        assert_eq!(counts[&EmergenceType::ChimeraFormation], 1);
    }

    #[test]
    fn stats_total_detected() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        for i in 0..5 {
            det.record_emergence(&ep(EmergenceType::ThermalSpike, 0.9, 0.5, Vec::new(), format!("{i}"), i as u64, None)).unwrap();
        }

        let stats = det.stats();
        assert_eq!(stats.total_detected, 5);
    }

    #[test]
    fn reset_clears_all() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        det.record_emergence(&ep(EmergenceType::ThermalSpike, 0.9, 0.5, Vec::new(), "test", 1, None)).unwrap();
        det.start_monitor(EmergenceType::DispatchLoop, 1);

        det.reset();
        assert_eq!(det.history_len(), 0);
        assert_eq!(det.active_monitor_count(), 0);
        assert_eq!(det.stats().total_detected, 0);
    }

    #[test]
    fn severity_classification() {
        assert_eq!(EmergenceSeverity::from_score(0.95), EmergenceSeverity::Critical);
        assert_eq!(EmergenceSeverity::from_score(0.8), EmergenceSeverity::High);
        assert_eq!(EmergenceSeverity::from_score(0.5), EmergenceSeverity::Medium);
        assert_eq!(EmergenceSeverity::from_score(0.2), EmergenceSeverity::Low);
    }

    #[test]
    fn emergence_type_display() {
        assert_eq!(EmergenceType::CoherenceLock.to_string(), "coherence_lock");
        assert_eq!(EmergenceType::ChimeraFormation.to_string(), "chimera_formation");
        assert_eq!(EmergenceType::BeneficialSync.to_string(), "beneficial_sync");
    }

    #[test]
    fn severity_display() {
        assert_eq!(EmergenceSeverity::Critical.to_string(), "critical");
        assert_eq!(EmergenceSeverity::Low.to_string(), "low");
    }

    #[test]
    fn confidence_clamped_to_range() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        let id = det.record_emergence(&ep(
            EmergenceType::ThermalSpike,
            1.5, // Will be clamped to 1.0
            2.0, // Will be clamped to 1.0
            Vec::new(),
            "clamped",
            1,
            None,
        )).unwrap();

        assert!(id.is_some());
        let records = det.recent(1);
        assert!((records[0].confidence - 1.0).abs() < f64::EPSILON);
        assert!((records[0].severity - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn record_with_recommended_action() {
        let config = EmergenceDetectorConfig {
            min_confidence: 0.0,
            ..Default::default()
        };
        let det = EmergenceDetector::with_config(config);

        det.record_emergence(&ep(
            EmergenceType::CouplingRunaway,
            0.9,
            0.8,
            vec!["pane-a".into(), "pane-b".into()],
            "runaway",
            1,
            Some("Clamp K".into()),
        )).unwrap();

        let records = det.recent(1);
        assert_eq!(records[0].recommended_action.as_deref(), Some("Clamp K"));
        assert_eq!(records[0].affected_panes.len(), 2);
    }
}
