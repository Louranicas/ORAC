//! # M38: Correlation Engine
//!
//! Discovers correlations between emergence events and fleet coordination
//! patterns. Mines pathways that connect recurring emergence types, parameter
//! changes, and fitness outcomes.
//!
//! ## Layer: L8 (Evolution)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`, `m37_emergence_detector`
//!
//! ## Correlation Types
//!
//! | Type | Description |
//! |------|-------------|
//! | `Temporal` | Events occurring within a time window |
//! | `Causal` | Parameter change followed by emergence event |
//! | `Recurring` | Same pattern repeated N+ times |
//! | `FitnessLinked` | Emergence correlated with fitness delta |

use std::collections::{HashMap, VecDeque};
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::{PvError, PvResult};

use super::m37_emergence_detector::EmergenceType;

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default correlation window size (ticks).
const DEFAULT_WINDOW_TICKS: u64 = 30;

/// Default maximum events buffered.
const DEFAULT_MAX_BUFFER: usize = 10_000;

/// Default minimum correlation confidence to retain.
const DEFAULT_MIN_CONFIDENCE: f64 = 0.5;

/// Default minimum recurring count.
const DEFAULT_MIN_RECURRING_COUNT: u32 = 3;

/// Default correlation history capacity.
const DEFAULT_HISTORY_CAPACITY: usize = 1000;

/// Maximum pathways tracked.
const MAX_PATHWAYS: usize = 500;

// ──────────────────────────────────────────────────────────────
// Enums
// ──────────────────────────────────────────────────────────────

/// Types of correlation links discovered between events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CorrelationType {
    /// Events co-occurred within a time window.
    Temporal,
    /// A parameter change preceded an emergence event.
    Causal,
    /// Same event pattern occurred N+ times.
    Recurring,
    /// Emergence event correlated with fitness change.
    FitnessLinked,
}

impl fmt::Display for CorrelationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Temporal => f.write_str("temporal"),
            Self::Causal => f.write_str("causal"),
            Self::Recurring => f.write_str("recurring"),
            Self::FitnessLinked => f.write_str("fitness_linked"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Data structures
// ──────────────────────────────────────────────────────────────

/// An event ingested for correlation analysis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationEvent {
    /// Event ID (sequential).
    pub id: u64,
    /// Category tag (e.g. "emergence", "mutation", "fitness").
    pub category: String,
    /// Specific event type.
    pub event_type: String,
    /// Numeric value associated with the event.
    pub value: f64,
    /// Tick when the event occurred.
    pub tick: u64,
    /// Optional parameter name involved.
    pub parameter: Option<String>,
}

/// A discovered correlation between events.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Correlation {
    /// Correlation ID (sequential).
    pub id: u64,
    /// Correlation type.
    pub correlation_type: CorrelationType,
    /// Source event IDs.
    pub source_events: Vec<u64>,
    /// Confidence [0.0, 1.0].
    pub confidence: f64,
    /// Tick offset between events (signed).
    pub tick_offset: i64,
    /// Human-readable description.
    pub description: String,
    /// Tick when discovered.
    pub discovered_at_tick: u64,
}

/// A recurring pathway: a pattern that repeats.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pathway {
    /// Pattern key (e.g. `emergence:coherence_lock→mutation:k_mod`).
    pub pattern_key: String,
    /// Number of times observed.
    pub occurrences: u32,
    /// Average confidence across occurrences.
    pub avg_confidence: f64,
    /// Average tick offset.
    pub avg_tick_offset: f64,
    /// Last observed tick.
    pub last_seen_tick: u64,
    /// Whether this pathway has been promoted to "established" (>= `min_recurring_count`).
    pub established: bool,
}

/// Configuration for the `CorrelationEngine`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorrelationEngineConfig {
    /// Time window for temporal correlation (ticks).
    pub window_ticks: u64,
    /// Maximum events buffered.
    pub max_buffer: usize,
    /// Minimum confidence to retain a correlation.
    pub min_confidence: f64,
    /// Minimum occurrences for a recurring pathway.
    pub min_recurring_count: u32,
    /// Maximum correlation history.
    pub history_capacity: usize,
}

impl Default for CorrelationEngineConfig {
    fn default() -> Self {
        Self {
            window_ticks: DEFAULT_WINDOW_TICKS,
            max_buffer: DEFAULT_MAX_BUFFER,
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            min_recurring_count: DEFAULT_MIN_RECURRING_COUNT,
            history_capacity: DEFAULT_HISTORY_CAPACITY,
        }
    }
}

/// Aggregate statistics for the `CorrelationEngine`.
#[derive(Clone, Debug, Default)]
pub struct CorrelationStats {
    /// Total events ingested.
    pub total_events: u64,
    /// Total correlations discovered.
    pub total_correlations: u64,
    /// Count by correlation type.
    pub by_type: HashMap<String, u64>,
    /// Total established pathways.
    pub established_pathways: usize,
    /// Total pathways tracked.
    pub total_pathways: usize,
}

// ──────────────────────────────────────────────────────────────
// CorrelationEngine
// ──────────────────────────────────────────────────────────────

/// Correlation engine for ORAC fleet coordination.
///
/// Ingests events from emergence detection, mutation proposals, and fitness
/// evaluations, then discovers temporal, causal, recurring, and fitness-linked
/// correlations between them.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct CorrelationEngine {
    /// Event buffer (ring buffer, FIFO eviction).
    events: RwLock<VecDeque<CorrelationEvent>>,
    /// Discovered correlations (ring buffer).
    correlations: RwLock<VecDeque<Correlation>>,
    /// Pathway patterns keyed by pattern key.
    pathways: RwLock<HashMap<String, Pathway>>,
    /// Monotonically increasing event ID counter.
    next_event_id: RwLock<u64>,
    /// Monotonically increasing correlation ID counter.
    next_correlation_id: RwLock<u64>,
    /// Configuration.
    config: CorrelationEngineConfig,
    /// Aggregate statistics.
    stats: RwLock<CorrelationStats>,
}

impl fmt::Debug for CorrelationEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CorrelationEngine")
            .field("events", &self.events.read().len())
            .field("correlations", &self.correlations.read().len())
            .field("pathways", &self.pathways.read().len())
            .finish_non_exhaustive()
    }
}

impl CorrelationEngine {
    /// Creates a new `CorrelationEngine` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(CorrelationEngineConfig::default())
    }

    /// Creates a new `CorrelationEngine` with the given configuration.
    #[must_use]
    pub fn with_config(config: CorrelationEngineConfig) -> Self {
        Self {
            events: RwLock::new(VecDeque::with_capacity(
                config.max_buffer.min(10_000),
            )),
            correlations: RwLock::new(VecDeque::with_capacity(
                config.history_capacity.min(1000),
            )),
            pathways: RwLock::new(HashMap::new()),
            next_event_id: RwLock::new(1),
            next_correlation_id: RwLock::new(1),
            config,
            stats: RwLock::new(CorrelationStats::default()),
        }
    }

    /// Validates the configuration.
    ///
    /// # Errors
    /// Returns [`PvError::ConfigValidation`] if any parameter is invalid.
    pub fn validate_config(config: &CorrelationEngineConfig) -> PvResult<()> {
        if config.window_ticks == 0 {
            return Err(PvError::ConfigValidation(
                "window_ticks must be > 0".into(),
            ));
        }
        if config.max_buffer == 0 {
            return Err(PvError::ConfigValidation(
                "max_buffer must be > 0".into(),
            ));
        }
        if config.min_confidence < 0.0 || config.min_confidence > 1.0 {
            return Err(PvError::ConfigValidation(
                "min_confidence must be in [0.0, 1.0]".into(),
            ));
        }
        if config.min_recurring_count == 0 {
            return Err(PvError::ConfigValidation(
                "min_recurring_count must be > 0".into(),
            ));
        }
        Ok(())
    }

    /// Ingest an event for correlation analysis.
    ///
    /// Returns the event ID and any correlations discovered with existing events.
    ///
    /// BUG-Gen19 fix: Also purges expired events (older than `window_ticks`
    /// from the most recent event tick) on each ingestion call. Without this,
    /// the buffer could grow to `max_buffer` with stale events that will never
    /// produce correlations.
    pub fn ingest(
        &self,
        category: &str,
        event_type: &str,
        value: f64,
        tick: u64,
        parameter: Option<&str>,
    ) -> (u64, Vec<u64>) {
        let event_id = {
            let mut counter = self.next_event_id.write();
            let id = *counter;
            *counter += 1;
            id
        };

        let event = CorrelationEvent {
            id: event_id,
            category: category.to_owned(),
            event_type: event_type.to_owned(),
            value,
            tick,
            parameter: parameter.map(String::from),
        };

        // Find temporal correlations with recent events
        let corr_ids = self.find_temporal_correlations(&event);

        // Buffer the event and purge expired
        {
            let mut events = self.events.write();
            // BUG-Gen19: purge events outside the correlation window
            Self::purge_expired_events(&mut events, tick, self.config.window_ticks);
            if events.len() >= self.config.max_buffer {
                events.pop_front();
            }
            events.push_back(event);
        }

        self.stats.write().total_events += 1;

        (event_id, corr_ids)
    }

    /// Purge events older than `window_ticks` from `reference_tick`.
    ///
    /// Events where `reference_tick - event.tick > window_ticks` are removed
    /// from the front of the deque. Since events are appended in tick order,
    /// we can stop at the first non-expired event.
    fn purge_expired_events(
        events: &mut VecDeque<CorrelationEvent>,
        reference_tick: u64,
        window_ticks: u64,
    ) {
        while let Some(front) = events.front() {
            if reference_tick.saturating_sub(front.tick) > window_ticks {
                events.pop_front();
            } else {
                break;
            }
        }
    }

    /// Ingest an emergence event specifically.
    pub fn ingest_emergence(
        &self,
        emergence_type: EmergenceType,
        confidence: f64,
        tick: u64,
    ) -> (u64, Vec<u64>) {
        self.ingest(
            "emergence",
            &emergence_type.to_string(),
            confidence,
            tick,
            None,
        )
    }

    /// Ingest a mutation event.
    pub fn ingest_mutation(
        &self,
        parameter: &str,
        delta: f64,
        tick: u64,
    ) -> (u64, Vec<u64>) {
        self.ingest("mutation", "parameter_change", delta, tick, Some(parameter))
    }

    /// Ingest a fitness change event.
    pub fn ingest_fitness_change(
        &self,
        fitness_delta: f64,
        tick: u64,
    ) -> (u64, Vec<u64>) {
        self.ingest("fitness", "delta", fitness_delta, tick, None)
    }

    /// Find temporal correlations between a new event and buffered events.
    fn find_temporal_correlations(&self, new_event: &CorrelationEvent) -> Vec<u64> {
        let events = self.events.read();
        let mut corr_ids = Vec::new();

        for existing in events.iter().rev() {
            let tick_diff = new_event.tick.saturating_sub(existing.tick);
            if tick_diff > self.config.window_ticks {
                break;
            }

            // Don't self-correlate same category/type
            if existing.category == new_event.category
                && existing.event_type == new_event.event_type
            {
                continue;
            }

            // Temporal correlation: different events within window.
            // BUG-061: Same-tick events (tick_diff=0) should not get
            // maximum confidence — they likely come from the same tick
            // cycle, not a causal chain. Use max(tick_diff, 1) to cap at ~0.97.
            let effective_diff = tick_diff.max(1);
            #[allow(clippy::cast_precision_loss)] // effective_diff and window_ticks are small coordination values
            let confidence =
                1.0 - (effective_diff as f64 / (self.config.window_ticks as f64).max(1.0));
            if confidence < self.config.min_confidence {
                continue;
            }

            #[allow(clippy::cast_possible_wrap)] // tick_diff bounded by window_ticks (small)
            let tick_offset = tick_diff as i64;
            let corr_id = self.record_correlation(
                CorrelationType::Temporal,
                vec![existing.id, new_event.id],
                confidence,
                tick_offset,
                format!(
                    "{}.{} ↔ {}.{} (Δ{tick_diff} ticks)",
                    existing.category, existing.event_type,
                    new_event.category, new_event.event_type,
                ),
                new_event.tick,
            );

            // Update pathway
            let pattern_key = format!(
                "{}:{}→{}:{}",
                existing.category, existing.event_type,
                new_event.category, new_event.event_type,
            );
            #[allow(clippy::cast_precision_loss)] // tick_diff bounded by window_ticks
            let tick_diff_f = tick_diff as f64;
            self.update_pathway(&pattern_key, confidence, tick_diff_f, new_event.tick);

            corr_ids.push(corr_id);
        }

        corr_ids
    }

    /// Record a correlation and return its ID.
    fn record_correlation(
        &self,
        correlation_type: CorrelationType,
        source_events: Vec<u64>,
        confidence: f64,
        tick_offset: i64,
        description: String,
        tick: u64,
    ) -> u64 {
        let id = {
            let mut counter = self.next_correlation_id.write();
            let id = *counter;
            *counter += 1;
            id
        };

        let correlation = Correlation {
            id,
            correlation_type,
            source_events,
            confidence,
            tick_offset,
            description,
            discovered_at_tick: tick,
        };

        {
            let mut corrs = self.correlations.write();
            if corrs.len() >= self.config.history_capacity {
                corrs.pop_front();
            }
            corrs.push_back(correlation);
        }

        {
            let mut stats = self.stats.write();
            stats.total_correlations += 1;
            *stats
                .by_type
                .entry(correlation_type.to_string())
                .or_insert(0) += 1;
        }

        id
    }

    /// Record a causal correlation explicitly.
    ///
    /// Use when a parameter change is followed by an emergence event.
    pub fn record_causal(
        &self,
        cause_event_id: u64,
        effect_event_id: u64,
        confidence: f64,
        tick_offset: i64,
        description: String,
        tick: u64,
    ) -> u64 {
        self.record_correlation(
            CorrelationType::Causal,
            vec![cause_event_id, effect_event_id],
            confidence.clamp(0.0, 1.0),
            tick_offset,
            description,
            tick,
        )
    }

    /// Record a fitness-linked correlation.
    pub fn record_fitness_linked(
        &self,
        emergence_event_id: u64,
        fitness_event_id: u64,
        fitness_delta: f64,
        tick: u64,
    ) -> u64 {
        let confidence = fitness_delta.abs().clamp(0.0, 1.0);
        let direction = if fitness_delta > 0.0 {
            "improvement"
        } else {
            "degradation"
        };
        self.record_correlation(
            CorrelationType::FitnessLinked,
            vec![emergence_event_id, fitness_event_id],
            confidence,
            0,
            format!("Fitness {direction}: Δ{fitness_delta:.4}"),
            tick,
        )
    }

    /// Update or create a pathway pattern.
    fn update_pathway(
        &self,
        pattern_key: &str,
        confidence: f64,
        tick_offset: f64,
        tick: u64,
    ) {
        let mut pathways = self.pathways.write();

        // Cap pathway count
        if pathways.len() >= MAX_PATHWAYS && !pathways.contains_key(pattern_key) {
            // Evict least recently seen
            let evict_key = pathways
                .iter()
                .min_by_key(|(_, p)| p.last_seen_tick)
                .map(|(k, _)| k.clone());
            if let Some(key) = evict_key {
                pathways.remove(&key);
            }
        }

        let pathway = pathways.entry(pattern_key.to_owned()).or_insert(Pathway {
            pattern_key: pattern_key.to_owned(),
            occurrences: 0,
            avg_confidence: 0.0,
            avg_tick_offset: 0.0,
            last_seen_tick: tick,
            established: false,
        });

        let n = f64::from(pathway.occurrences);
        pathway.avg_confidence =
            (n * pathway.avg_confidence + confidence) / (n + 1.0);
        pathway.avg_tick_offset =
            (n * pathway.avg_tick_offset + tick_offset) / (n + 1.0);
        pathway.occurrences += 1;
        pathway.last_seen_tick = tick;
        pathway.established =
            pathway.occurrences >= self.config.min_recurring_count;
    }

    /// Get all established pathways (recurring patterns).
    #[must_use]
    pub fn established_pathways(&self) -> Vec<Pathway> {
        self.pathways
            .read()
            .values()
            .filter(|p| p.established)
            .cloned()
            .collect()
    }

    /// Get all pathways sorted by occurrence count (descending).
    #[must_use]
    pub fn all_pathways_sorted(&self) -> Vec<Pathway> {
        let pathways = self.pathways.read();
        let mut sorted: Vec<Pathway> = pathways.values().cloned().collect();
        sorted.sort_by(|a, b| b.occurrences.cmp(&a.occurrences));
        sorted
    }

    /// Get recent correlations (up to `limit`).
    #[must_use]
    pub fn recent_correlations(&self, limit: usize) -> Vec<Correlation> {
        self.correlations
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    /// Get correlations filtered by type.
    #[must_use]
    pub fn correlations_by_type(&self, ctype: CorrelationType) -> Vec<Correlation> {
        self.correlations
            .read()
            .iter()
            .filter(|c| c.correlation_type == ctype)
            .cloned()
            .collect()
    }

    /// Get the total number of events buffered.
    #[must_use]
    pub fn event_count(&self) -> usize {
        self.events.read().len()
    }

    /// Get the total number of correlations in history.
    #[must_use]
    pub fn correlation_count(&self) -> usize {
        self.correlations.read().len()
    }

    /// Get the total number of pathways tracked.
    #[must_use]
    pub fn pathway_count(&self) -> usize {
        self.pathways.read().len()
    }

    /// Get aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> CorrelationStats {
        let mut s = self.stats.read().clone();
        let pathways = self.pathways.read();
        s.total_pathways = pathways.len();
        s.established_pathways = pathways.values().filter(|p| p.established).count();
        s
    }

    /// Clear all state.
    pub fn reset(&self) {
        self.events.write().clear();
        self.correlations.write().clear();
        self.pathways.write().clear();
        *self.stats.write() = CorrelationStats::default();
    }
}

impl Default for CorrelationEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> CorrelationEngine {
        CorrelationEngine::new()
    }

    #[test]
    fn default_config_valid() {
        assert!(CorrelationEngine::validate_config(&CorrelationEngineConfig::default()).is_ok());
    }

    #[test]
    fn config_zero_window_invalid() {
        let config = CorrelationEngineConfig {
            window_ticks: 0,
            ..Default::default()
        };
        assert!(CorrelationEngine::validate_config(&config).is_err());
    }

    #[test]
    fn config_zero_buffer_invalid() {
        let config = CorrelationEngineConfig {
            max_buffer: 0,
            ..Default::default()
        };
        assert!(CorrelationEngine::validate_config(&config).is_err());
    }

    #[test]
    fn config_bad_confidence_invalid() {
        let config = CorrelationEngineConfig {
            min_confidence: -0.1,
            ..Default::default()
        };
        assert!(CorrelationEngine::validate_config(&config).is_err());
    }

    #[test]
    fn config_zero_recurring_invalid() {
        let config = CorrelationEngineConfig {
            min_recurring_count: 0,
            ..Default::default()
        };
        assert!(CorrelationEngine::validate_config(&config).is_err());
    }

    #[test]
    fn ingest_basic() {
        let eng = make_engine();
        let (id, _) = eng.ingest("test", "event_a", 1.0, 10, None);
        assert_eq!(id, 1);
        assert_eq!(eng.event_count(), 1);
    }

    #[test]
    fn ingest_returns_correlations() {
        let eng = make_engine();
        eng.ingest("emergence", "coherence_lock", 0.9, 10, None);
        let (_, corrs) = eng.ingest("mutation", "k_change", 0.5, 11, Some("k_mod"));
        // Should find temporal correlation (Δ1 tick, within 30-tick window)
        assert!(!corrs.is_empty());
        assert_eq!(eng.correlation_count(), 1);
    }

    #[test]
    fn no_self_correlation() {
        let eng = make_engine();
        eng.ingest("emergence", "spike", 0.9, 10, None);
        let (_, corrs) = eng.ingest("emergence", "spike", 0.8, 11, None);
        // Same category+type should not correlate
        assert!(corrs.is_empty());
    }

    #[test]
    fn no_correlation_outside_window() {
        let eng = make_engine();
        eng.ingest("emergence", "spike", 0.9, 10, None);
        let (_, corrs) = eng.ingest("mutation", "change", 0.5, 100, None);
        // Δ90 ticks > 30-tick window
        assert!(corrs.is_empty());
    }

    #[test]
    fn ingest_emergence_helper() {
        let eng = make_engine();
        let (id, _) = eng.ingest_emergence(EmergenceType::ThermalSpike, 0.8, 10);
        assert!(id > 0);
        assert_eq!(eng.event_count(), 1);
    }

    #[test]
    fn ingest_mutation_helper() {
        let eng = make_engine();
        let (id, _) = eng.ingest_mutation("k_mod", 0.05, 10);
        assert!(id > 0);
    }

    #[test]
    fn ingest_fitness_change_helper() {
        let eng = make_engine();
        let (id, _) = eng.ingest_fitness_change(0.03, 10);
        assert!(id > 0);
    }

    #[test]
    fn event_buffer_bounded() {
        let config = CorrelationEngineConfig {
            max_buffer: 5,
            // BUG-Gen19: use a large window so tick-based purging does not
            // interfere with this test's intent (FIFO cap enforcement).
            window_ticks: 10_000,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        for i in 0..10 {
            eng.ingest("test", "event", 1.0, i * 100, None);
        }
        assert_eq!(eng.event_count(), 5);
    }

    #[test]
    fn correlation_history_bounded() {
        let config = CorrelationEngineConfig {
            history_capacity: 3,
            window_ticks: 1000,
            min_confidence: 0.0,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        // Generate many correlations
        for i in 0..10_u64 {
            eng.ingest("cat_a", &format!("type_{i}"), 1.0, i, None);
            eng.ingest("cat_b", &format!("type_{i}"), 1.0, i, None);
        }
        assert!(eng.correlation_count() <= 3);
    }

    #[test]
    fn pathway_creation() {
        let eng = make_engine();

        // Same pattern 3 times → established
        for i in 0..3 {
            let tick = i * 2;
            eng.ingest("emergence", "coherence_lock", 0.9, tick, None);
            eng.ingest("mutation", "k_change", 0.5, tick + 1, Some("k_mod"));
        }

        let established = eng.established_pathways();
        assert!(!established.is_empty());
    }

    #[test]
    fn pathway_not_established_below_threshold() {
        let eng = make_engine();

        // Only 2 occurrences (need 3)
        for i in 0..2 {
            let tick = i * 100; // Far apart to avoid cross-correlating
            eng.ingest("emergence", "lock", 0.9, tick, None);
            eng.ingest("mutation", "change", 0.5, tick + 1, None);
        }

        let established = eng.established_pathways();
        assert!(established.is_empty());
    }

    #[test]
    fn pathway_cap_enforced() {
        let config = CorrelationEngineConfig {
            window_ticks: 1000,
            min_confidence: 0.0,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        // Generate many unique patterns
        for i in 0..MAX_PATHWAYS + 10 {
            eng.ingest("cat_a", &format!("unique_{i}"), 1.0, i as u64, None);
            eng.ingest("cat_b", &format!("unique_{i}"), 1.0, i as u64, None);
        }

        assert!(eng.pathway_count() <= MAX_PATHWAYS);
    }

    #[test]
    fn record_causal_manual() {
        let eng = make_engine();
        let corr_id = eng.record_causal(1, 2, 0.9, 5, "test causal".into(), 10);
        assert!(corr_id > 0);

        let correlations = eng.correlations_by_type(CorrelationType::Causal);
        assert_eq!(correlations.len(), 1);
        assert!((correlations[0].confidence - 0.9).abs() < f64::EPSILON);
    }

    #[test]
    fn record_fitness_linked_manual() {
        let eng = make_engine();
        let corr_id = eng.record_fitness_linked(1, 2, 0.05, 20);
        assert!(corr_id > 0);

        let correlations = eng.correlations_by_type(CorrelationType::FitnessLinked);
        assert_eq!(correlations.len(), 1);
    }

    #[test]
    fn recent_correlations_newest_first() {
        let config = CorrelationEngineConfig {
            min_confidence: 0.0,
            window_ticks: 1000,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        for i in 0..5 {
            eng.record_causal(i, i + 1, 0.8, 1, format!("corr {i}"), i as u64);
        }

        let recent = eng.recent_correlations(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].discovered_at_tick, 4);
    }

    #[test]
    fn all_pathways_sorted_by_occurrences() {
        let config = CorrelationEngineConfig {
            window_ticks: 100,
            min_confidence: 0.0,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        // Create two patterns with different occurrence counts
        for i in 0..5 {
            eng.ingest("cat_a", "type_1", 1.0, i, None);
            eng.ingest("cat_b", "type_1", 1.0, i, None);
        }
        for i in 0..2 {
            eng.ingest("cat_a", "type_2", 1.0, i + 50, None);
            eng.ingest("cat_b", "type_2", 1.0, i + 50, None);
        }

        let sorted = eng.all_pathways_sorted();
        if sorted.len() >= 2 {
            assert!(sorted[0].occurrences >= sorted[1].occurrences);
        }
    }

    #[test]
    fn stats_tracking() {
        let eng = make_engine();
        eng.ingest("test", "a", 1.0, 1, None);
        eng.ingest("test", "b", 1.0, 2, None);

        let stats = eng.stats();
        assert_eq!(stats.total_events, 2);
    }

    #[test]
    fn reset_clears_all() {
        let eng = make_engine();
        eng.ingest("test", "a", 1.0, 1, None);
        eng.record_causal(1, 2, 0.9, 1, "test".into(), 1);

        eng.reset();
        assert_eq!(eng.event_count(), 0);
        assert_eq!(eng.correlation_count(), 0);
        assert_eq!(eng.pathway_count(), 0);
    }

    #[test]
    fn correlation_type_display() {
        assert_eq!(CorrelationType::Temporal.to_string(), "temporal");
        assert_eq!(CorrelationType::Causal.to_string(), "causal");
        assert_eq!(CorrelationType::Recurring.to_string(), "recurring");
        assert_eq!(CorrelationType::FitnessLinked.to_string(), "fitness_linked");
    }

    #[test]
    fn correlation_confidence_clamped() {
        let eng = make_engine();
        let corr_id = eng.record_causal(1, 2, 1.5, 1, "over".into(), 1);
        let corrs = eng.recent_correlations(1);
        assert_eq!(corrs[0].id, corr_id);
        assert!((corrs[0].confidence - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn pathway_avg_confidence_updated() {
        let config = CorrelationEngineConfig {
            window_ticks: 100,
            min_confidence: 0.0,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        for i in 0..3 {
            eng.ingest("emerge", "lock", 0.9, i, None);
            eng.ingest("mut", "change", 0.5, i, None);
        }

        let pathways = eng.all_pathways_sorted();
        assert!(!pathways.is_empty());
        assert!(pathways[0].avg_confidence > 0.0);
    }

    #[test]
    fn ingest_with_parameter() {
        let eng = make_engine();
        let (id, _) = eng.ingest("mutation", "delta", 0.05, 10, Some("k_mod"));
        assert!(id > 0);

        let events = eng.events.read();
        let event = events.back().unwrap();
        assert_eq!(event.parameter.as_deref(), Some("k_mod"));
    }

    #[test]
    fn multiple_correlation_types_mixed() {
        let eng = make_engine();
        eng.record_causal(1, 2, 0.8, 1, "causal".into(), 10);
        eng.record_fitness_linked(3, 4, 0.05, 20);

        let causal = eng.correlations_by_type(CorrelationType::Causal);
        let fitness = eng.correlations_by_type(CorrelationType::FitnessLinked);
        assert_eq!(causal.len(), 1);
        assert_eq!(fitness.len(), 1);
        assert_eq!(eng.correlation_count(), 2);
    }

    #[test]
    fn stats_by_type_accurate() {
        let eng = make_engine();
        eng.record_causal(1, 2, 0.8, 1, "a".into(), 1);
        eng.record_causal(3, 4, 0.7, 1, "b".into(), 2);
        eng.record_fitness_linked(5, 6, 0.05, 3);

        let stats = eng.stats();
        assert_eq!(stats.by_type.get("causal"), Some(&2));
        assert_eq!(stats.by_type.get("fitness_linked"), Some(&1));
    }

    /// BUG-Gen19: Verify tick-based event purging during ingestion.
    #[test]
    fn ingest_purges_expired_events_by_tick() {
        let config = CorrelationEngineConfig {
            window_ticks: 10,
            max_buffer: 10_000,
            min_confidence: 0.0,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        // Ingest 5 events at tick 0
        for _ in 0..5 {
            eng.ingest("test", "early", 1.0, 0, None);
        }
        assert_eq!(eng.event_count(), 5);

        // Ingest 1 event at tick 5 — still within window
        eng.ingest("test", "mid", 1.0, 5, None);
        assert_eq!(eng.event_count(), 6);

        // Ingest at tick 15 — tick 0 events are 15 ticks old > window(10)
        eng.ingest("test", "late", 1.0, 15, None);
        // The 5 events at tick=0 should have been purged (age 15 > 10)
        // Only tick=5 and tick=15 events remain
        assert_eq!(eng.event_count(), 2);
    }

    /// BUG-Gen19: Verify purge does not remove events within the window.
    #[test]
    fn ingest_preserves_events_within_window() {
        let config = CorrelationEngineConfig {
            window_ticks: 30,
            max_buffer: 10_000,
            min_confidence: 0.0,
            ..Default::default()
        };
        let eng = CorrelationEngine::with_config(config);

        // Events at ticks 10, 20, 30
        eng.ingest("a", "type1", 1.0, 10, None);
        eng.ingest("b", "type2", 1.0, 20, None);
        eng.ingest("c", "type3", 1.0, 30, None);

        // All within 30 ticks of tick=30
        assert_eq!(eng.event_count(), 3);

        // Ingest at tick 35 — tick 10 is now 25 ticks old, still <= 30
        eng.ingest("d", "type4", 1.0, 35, None);
        assert_eq!(eng.event_count(), 4);

        // Ingest at tick 45 — tick 10 is 35 ticks old > 30, purged
        eng.ingest("e", "type5", 1.0, 45, None);
        // tick=10 purged, ticks 20,30,35,45 remain
        assert_eq!(eng.event_count(), 4);
    }

    /// BUG-Gen19: Verify purge handles empty buffer gracefully.
    #[test]
    fn ingest_purge_empty_buffer() {
        let eng = make_engine();
        // First ingest into empty buffer should work fine
        let (id, _) = eng.ingest("test", "first", 1.0, 100, None);
        assert!(id > 0);
        assert_eq!(eng.event_count(), 1);
    }
}
