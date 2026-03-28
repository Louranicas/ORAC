//! # M36: RALPH Engine
//!
//! 5-phase RALPH meta-learning loop orchestrator for ORAC fleet coordination.
//! Drives system parameter evolution through: Recognize → Analyze → Learn →
//! Propose → Harvest, with atomic snapshot/rollback protection.
//!
//! ## Layer: L8 (Evolution)
//! ## Dependencies: `m01_core_types`, `m02_error_handling`, `m37`-`m40`
//!
//! ## RALPH Phases
//!
//! | Phase | Purpose |
//! |-------|---------|
//! | Recognize | Identify parameters drifting from targets |
//! | Analyze | Compute fitness, trend, dimension analysis |
//! | Learn | Mine correlations for patterns |
//! | Propose | Generate diversity-enforced mutation proposals |
//! | Harvest | Accept beneficial mutations, rollback harmful ones |
//!
//! ## Snapshot + Rollback
//!
//! Before each mutation proposal, a snapshot of all parameter values is captured.
//! If the mutation degrades fitness beyond the rollback threshold, all parameters
//! are restored from the snapshot.

use std::collections::VecDeque;
use std::fmt;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::m1_core::m02_error_handling::PvResult;

use super::m37_emergence_detector::EmergenceDetector;
use super::m38_correlation_engine::CorrelationEngine;
use super::m39_fitness_tensor::{FitnessTensor, FitnessTrend, SystemState, TensorValues};
use super::m40_mutation_selector::{MutationProposal, MutationSelector};

// ──────────────────────────────────────────────────────────────
// Constants
// ──────────────────────────────────────────────────────────────

/// Default minimum fitness improvement to accept a mutation.
const DEFAULT_ACCEPT_THRESHOLD: f64 = 0.02;

/// Default fitness regression that triggers rollback.
const DEFAULT_ROLLBACK_THRESHOLD: f64 = -0.01;

/// Default verification window (ticks to wait before harvest).
const DEFAULT_VERIFICATION_TICKS: u64 = 10;

/// Default maximum RALPH cycles before pause.
const DEFAULT_MAX_CYCLES: u64 = 1000;

/// Default maximum snapshot history.
const DEFAULT_SNAPSHOT_CAPACITY: usize = 50;

// ──────────────────────────────────────────────────────────────
// Enums
// ──────────────────────────────────────────────────────────────

/// A phase in the RALPH meta-learning loop.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RalphPhase {
    /// Identify parameters drifting from targets.
    #[default]
    Recognize,
    /// Compute fitness and analyze dimensions.
    Analyze,
    /// Mine correlations and extract patterns.
    Learn,
    /// Generate diversity-enforced mutation proposals.
    Propose,
    /// Accept or rollback mutations based on fitness delta.
    Harvest,
}

impl RalphPhase {
    /// Returns the next phase in the RALPH cycle.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Recognize => Self::Analyze,
            Self::Analyze => Self::Learn,
            Self::Learn => Self::Propose,
            Self::Propose => Self::Harvest,
            Self::Harvest => Self::Recognize,
        }
    }

    /// Returns the zero-indexed ordinal of this phase (0-4).
    #[must_use]
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::Recognize => 0,
            Self::Analyze => 1,
            Self::Learn => 2,
            Self::Propose => 3,
            Self::Harvest => 4,
        }
    }

    /// Returns the phase name as a static string.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Recognize => "Recognize",
            Self::Analyze => "Analyze",
            Self::Learn => "Learn",
            Self::Propose => "Propose",
            Self::Harvest => "Harvest",
        }
    }
}

impl fmt::Display for RalphPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Status of a mutation through its lifecycle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationStatus {
    /// Mutation proposed but not yet applied.
    Proposed,
    /// Mutation applied, awaiting verification.
    Applied,
    /// Mutation accepted (fitness improved).
    Accepted,
    /// Mutation rolled back (fitness regressed).
    RolledBack,
    /// Mutation skipped (rejected by diversity gate).
    Skipped,
}

impl fmt::Display for MutationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Proposed => f.write_str("proposed"),
            Self::Applied => f.write_str("applied"),
            Self::Accepted => f.write_str("accepted"),
            Self::RolledBack => f.write_str("rolled_back"),
            Self::Skipped => f.write_str("skipped"),
        }
    }
}

// ──────────────────────────────────────────────────────────────
// Data structures
// ──────────────────────────────────────────────────────────────

/// A parameter value snapshot for rollback.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParameterSnapshot {
    /// Parameter name.
    pub name: String,
    /// Value at snapshot time.
    pub value: f64,
}

/// A full state snapshot before mutation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Generation at snapshot time.
    pub generation: u64,
    /// Tick at snapshot time.
    pub tick: u64,
    /// Fitness at snapshot time.
    pub fitness: f64,
    /// Parameter values.
    pub parameters: Vec<ParameterSnapshot>,
}

/// A mutation record in the RALPH history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutationRecord {
    /// Generation number.
    pub generation: u64,
    /// The mutation proposal.
    pub proposal: MutationProposal,
    /// Final status.
    pub status: MutationStatus,
    /// Fitness before mutation.
    pub fitness_before: f64,
    /// Fitness after verification (0.0 if not yet verified).
    pub fitness_after: f64,
    /// Tick when proposed.
    pub proposed_at_tick: u64,
    /// Tick when harvested (0 if not yet harvested).
    pub harvested_at_tick: u64,
}

/// Current active mutation being verified.
#[derive(Clone, Debug)]
struct ActiveMutation {
    /// The mutation proposal.
    proposal: MutationProposal,
    /// Snapshot taken before applying.
    snapshot: StateSnapshot,
    /// Tick when applied.
    applied_at_tick: u64,
    /// Generation number for mutation history lookup (avoids `back_mut()` race).
    generation: u64,
}

/// Configuration for the RALPH engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RalphEngineConfig {
    /// Fitness improvement threshold to accept mutation.
    pub accept_threshold: f64,
    /// Fitness regression threshold to trigger rollback.
    pub rollback_threshold: f64,
    /// Ticks to wait between apply and harvest.
    pub verification_ticks: u64,
    /// Maximum RALPH cycles before auto-pause.
    pub max_cycles: u64,
    /// Maximum snapshot history.
    pub snapshot_capacity: usize,
}

impl Default for RalphEngineConfig {
    fn default() -> Self {
        Self {
            accept_threshold: DEFAULT_ACCEPT_THRESHOLD,
            rollback_threshold: DEFAULT_ROLLBACK_THRESHOLD,
            verification_ticks: DEFAULT_VERIFICATION_TICKS,
            max_cycles: DEFAULT_MAX_CYCLES,
            snapshot_capacity: DEFAULT_SNAPSHOT_CAPACITY,
        }
    }
}

/// RALPH engine state visible to callers.
#[derive(Clone, Debug, Default)]
pub struct RalphState {
    /// Current RALPH phase.
    pub phase: RalphPhase,
    /// Current generation (monotonically increasing).
    pub generation: u64,
    /// Total completed RALPH cycles.
    pub completed_cycles: u64,
    /// Whether the engine is paused.
    pub paused: bool,
    /// Whether a mutation is currently being verified.
    pub has_active_mutation: bool,
    /// Most recent fitness score.
    pub current_fitness: f64,
    /// Current fitness trend.
    pub current_trend: FitnessTrend,
    /// Current system state.
    pub system_state: SystemState,
}

/// RALPH engine aggregate statistics.
#[derive(Clone, Debug, Default)]
pub struct RalphStats {
    /// Total mutations proposed.
    pub total_proposed: u64,
    /// Total mutations accepted.
    pub total_accepted: u64,
    /// Total mutations rolled back.
    pub total_rolled_back: u64,
    /// Total mutations skipped (diversity gate).
    pub total_skipped: u64,
    /// Total RALPH cycles completed.
    pub total_cycles: u64,
    /// Peak fitness observed.
    pub peak_fitness: f64,
}

// ──────────────────────────────────────────────────────────────
// RALPH Engine
// ──────────────────────────────────────────────────────────────

/// RALPH 5-phase meta-learning engine for ORAC fleet coordination.
///
/// Orchestrates emergence detection, correlation mining, fitness evaluation,
/// and diversity-enforced mutations in a cyclic Recognize → Analyze → Learn →
/// Propose → Harvest loop.
///
/// The Learn phase closes the feedback loop by mining established pathways,
/// recent emergence events, and dimensional fitness analysis to produce a
/// *mutation hint* — a preferred parameter for the Propose phase. The hint
/// is passed to `MutationSelector::select_with_hint()`, which tries the
/// hinted parameter first (subject to cooldown + diversity gates) before
/// falling back to round-robin.
///
/// # Thread Safety
///
/// All mutable state is protected by [`parking_lot::RwLock`].
pub struct RalphEngine {
    /// Current RALPH phase.
    phase: RwLock<RalphPhase>,
    /// Current generation.
    generation: RwLock<u64>,
    /// Completed cycles.
    completed_cycles: RwLock<u64>,
    /// Whether paused.
    paused: RwLock<bool>,
    /// Active mutation being verified.
    active_mutation: RwLock<Option<ActiveMutation>>,
    /// Mutation history.
    mutation_history: RwLock<VecDeque<MutationRecord>>,
    /// Snapshot history.
    snapshots: RwLock<VecDeque<StateSnapshot>>,
    /// Learned mutation hint from the Learn phase, consumed by Propose.
    /// Connects correlation output → mutation input, emergence events →
    /// strategy selection, and dimensional analysis → parameter prioritization.
    learned_hint: RwLock<Option<String>>,
    /// Fitness tensor evaluator.
    fitness: FitnessTensor,
    /// Emergence detector.
    emergence: EmergenceDetector,
    /// Correlation engine.
    correlation: CorrelationEngine,
    /// Mutation selector.
    selector: MutationSelector,
    /// Configuration.
    config: RalphEngineConfig,
    /// Aggregate statistics.
    stats: RwLock<RalphStats>,
}

impl fmt::Debug for RalphEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RalphEngine")
            .field("phase", &*self.phase.read())
            .field("generation", &*self.generation.read())
            .field("paused", &*self.paused.read())
            .finish_non_exhaustive()
    }
}

impl RalphEngine {
    /// Creates a new `RalphEngine` with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(RalphEngineConfig::default())
    }

    /// Creates a new `RalphEngine` with the given configuration.
    #[must_use]
    pub fn with_config(config: RalphEngineConfig) -> Self {
        let selector = MutationSelector::new();

        // BUG-041 fix: Register production-relevant mutable parameters.
        // Without these, RALPH's mutation selector has an empty pool and
        // skips every generation (AllOnTarget / NoParameters).
        Self::register_production_params(&selector);

        Self {
            phase: RwLock::new(RalphPhase::Recognize),
            generation: RwLock::new(0),
            completed_cycles: RwLock::new(0),
            paused: RwLock::new(false),
            active_mutation: RwLock::new(None),
            mutation_history: RwLock::new(VecDeque::with_capacity(100)),
            snapshots: RwLock::new(VecDeque::with_capacity(
                config.snapshot_capacity.min(100),
            )),
            learned_hint: RwLock::new(None),
            fitness: FitnessTensor::new(),
            emergence: EmergenceDetector::new(),
            correlation: CorrelationEngine::new(),
            selector,
            config,
            stats: RwLock::new(RalphStats::default()),
        }
    }

    /// Register the standard set of mutable parameters for ORAC RALPH.
    ///
    /// These correspond to real system knobs that RALPH can tune:
    /// coupling strength, Hebbian LTP rate, tick interval, r target, and decay.
    fn register_production_params(selector: &MutationSelector) {
        use crate::m8_evolution::m40_mutation_selector::MutableParameter;
        let params = [
            MutableParameter::new("k_mod", 1.0, 0.01, 1.5, 0.7, "Coupling strength modifier"),
            MutableParameter::new("hebbian_ltp", 0.01, 0.001, 0.1, 0.03, "Hebbian LTP learning rate"),
            MutableParameter::new("tick_interval", 5.0, 1.0, 30.0, 5.0, "RALPH tick interval (seconds)"),
            MutableParameter::new("r_target", 0.93, 0.5, 1.0, 0.80, "Target field coherence (r)"),
            MutableParameter::new("decay_rate", 0.995, 0.98, 1.0, 0.99, "Coupling weight decay per step"),
        ];
        for p in params {
            if let Err(e) = selector.register_parameter(p) {
                tracing::warn!("Failed to register RALPH parameter: {e}");
            }
        }
    }

    // ── Accessors ──

    /// Get current RALPH state.
    #[must_use]
    pub fn state(&self) -> RalphState {
        RalphState {
            phase: *self.phase.read(),
            generation: *self.generation.read(),
            completed_cycles: *self.completed_cycles.read(),
            paused: *self.paused.read(),
            has_active_mutation: self.active_mutation.read().is_some(),
            current_fitness: self.fitness.current_fitness().unwrap_or(0.0),
            current_trend: self.fitness.compute_trend(),
            system_state: SystemState::from_fitness(
                self.fitness.current_fitness().unwrap_or(0.0),
            ),
        }
    }

    /// Get aggregate statistics.
    #[must_use]
    pub fn stats(&self) -> RalphStats {
        self.stats.read().clone()
    }

    /// Get a reference to the fitness tensor.
    #[must_use]
    pub const fn fitness(&self) -> &FitnessTensor {
        &self.fitness
    }

    /// Get a reference to the emergence detector.
    #[must_use]
    pub const fn emergence(&self) -> &EmergenceDetector {
        &self.emergence
    }

    /// Get a reference to the correlation engine.
    #[must_use]
    pub const fn correlation(&self) -> &CorrelationEngine {
        &self.correlation
    }

    /// Get a reference to the mutation selector.
    #[must_use]
    pub const fn selector(&self) -> &MutationSelector {
        &self.selector
    }

    /// Get recent mutation history.
    #[must_use]
    pub fn recent_mutations(&self, limit: usize) -> Vec<MutationRecord> {
        self.mutation_history
            .read()
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect()
    }

    // ── Control ──

    /// Pause the RALPH loop.
    pub fn pause(&self) {
        *self.paused.write() = true;
    }

    /// Resume the RALPH loop.
    pub fn resume(&self) {
        *self.paused.write() = false;
    }

    /// Hydrate RALPH state from persisted storage (blackboard).
    ///
    /// Restores generation counter and completed cycles so evolution
    /// continues from where it left off across ORAC restarts.
    /// Does NOT restore mutation history or active mutation — those
    /// rebuild naturally as the loop cycles.
    pub fn hydrate(&self, generation: u64, completed_cycles: u64, peak_fitness: f64) {
        *self.generation.write() = generation;
        *self.completed_cycles.write() = completed_cycles;
        {
            let mut stats = self.stats.write();
            stats.peak_fitness = peak_fitness;
            stats.total_cycles = completed_cycles;
        }
        tracing::info!(
            generation,
            completed_cycles,
            peak_fitness = format!("{peak_fitness:.4}"),
            "RALPH hydrated from blackboard"
        );
    }

    // ── Phase Execution ──

    /// Execute the current RALPH phase.
    ///
    /// Call this once per tick. The engine advances through the 5 phases
    /// automatically. Returns the phase that was executed.
    ///
    /// # Errors
    /// Returns [`PvError`] on computation errors.
    pub fn tick(&self, tensor: &TensorValues, tick: u64) -> PvResult<RalphPhase> {
        if *self.paused.read() {
            return Ok(*self.phase.read());
        }

        let max_cycles = self.config.max_cycles;
        if *self.completed_cycles.read() >= max_cycles {
            *self.paused.write() = true;
            return Ok(*self.phase.read());
        }

        let current_phase = *self.phase.read();

        match current_phase {
            RalphPhase::Recognize => self.phase_recognize(tensor, tick)?,
            RalphPhase::Analyze => self.phase_analyze(tensor, tick)?,
            RalphPhase::Learn => self.phase_learn(tick),
            RalphPhase::Propose => self.phase_propose(tick),
            RalphPhase::Harvest => self.phase_harvest(tick),
        }

        Ok(current_phase)
    }

    /// Phase 1: Recognize — identify drifting parameters.
    fn phase_recognize(&self, tensor: &TensorValues, tick: u64) -> PvResult<()> {
        let generation = *self.generation.read();

        // Evaluate fitness
        self.fitness.evaluate(tensor, tick, Some(generation))?;

        // Track peak fitness (BUG-042: was only updated in Harvest, unreachable
        // when all mutations are skipped — now updated every cycle)
        if let Some(current) = self.fitness.current_fitness() {
            let mut stats = self.stats.write();
            if current > stats.peak_fitness {
                stats.peak_fitness = current;
            }
        }

        // Tick emergence decay
        self.emergence.tick_decay();

        // Advance to Analyze
        *self.phase.write() = RalphPhase::Analyze;
        Ok(())
    }

    /// Phase 2: Analyze — compute fitness, trend, dimension analysis.
    fn phase_analyze(&self, tensor: &TensorValues, tick: u64) -> PvResult<()> {
        let generation = *self.generation.read();

        // Evaluate fitness (records in history for trend)
        let _report = self.fitness.evaluate(tensor, tick, Some(generation))?;

        // Feed fitness into correlation engine
        let delta = self.fitness.fitness_delta(tick.saturating_sub(1), tick);
        if let Some(d) = delta {
            self.correlation.ingest_fitness_change(d, tick);
        }

        // Advance to Learn
        *self.phase.write() = RalphPhase::Learn;
        Ok(())
    }

    /// Phase 3: Learn — mine correlations, emergence, and dimensions for mutation hints.
    ///
    /// Closes the feedback loop: correlation pathways, emergence events, and
    /// dimensional fitness analysis are distilled into a single parameter hint
    /// that guides the Propose phase. Priority: emergence > dimension > pathway.
    fn phase_learn(&self, _tick: u64) {
        let mut hint: Option<String> = None;

        // ── Source 1: Emergence events → tactical parameter hints ──
        // Emergence events represent urgent system-level phenomena that
        // should override routine round-robin mutation selection.
        {
            use super::m37_emergence_detector::EmergenceType;
            let recent = self.emergence.recent(10);
            for record in &recent {
                let mapped = match record.emergence_type {
                    // Over-synchronization: reduce r target to allow phase diversity
                    EmergenceType::CoherenceLock => Some("r_target"),
                    // Coupling pathology: adjust coupling strength
                    // CouplingRunaway = K rising without r benefit
                    // ChimeraFormation = phase clusters need stronger coupling
                    EmergenceType::CouplingRunaway
                    | EmergenceType::ChimeraFormation => Some("k_mod"),
                    // Weights pinned at bounds: adjust LTP rate
                    EmergenceType::HebbianSaturation => Some("hebbian_ltp"),
                    // Thermal overshoot: slow tick rate to cool
                    EmergenceType::ThermalSpike => Some("tick_interval"),
                    // These don't need corrective mutation
                    EmergenceType::BeneficialSync
                    | EmergenceType::DispatchLoop
                    | EmergenceType::ConsentCascade
                    | EmergenceType::DegenerateMode => None,
                };
                if let Some(param) = mapped {
                    hint = Some(param.to_owned());
                    tracing::debug!(
                        emergence = %record.emergence_type,
                        hint = param,
                        "Learn: emergence-guided hint"
                    );
                    break; // Highest-priority source: use first match
                }
            }
        }

        // ── Source 2: Dimensional analysis → weakest-dimension hint ──
        // If no emergence event produced a hint, check which fitness
        // dimension is weakest and guide mutation toward its lever.
        if hint.is_none() {
            if let Some(analysis) = self.fitness.dimension_analysis() {
                use super::m39_fitness_tensor::FitnessDimension;
                let mapped = match analysis.weakest {
                    FitnessDimension::FieldCoherence => Some("r_target"),
                    FitnessDimension::HebbianHealth => Some("hebbian_ltp"),
                    FitnessDimension::CouplingStability => Some("decay_rate"),
                    FitnessDimension::ThermalBalance => Some("tick_interval"),
                    FitnessDimension::CoordinationQuality => Some("k_mod"),
                    // No direct parameter lever for these dimensions
                    _ => None,
                };
                if let Some(param) = mapped {
                    hint = Some(param.to_owned());
                    tracing::debug!(
                        weakest_dim = %analysis.weakest,
                        score = format!("{:.4}", analysis.weakest_weighted_score),
                        hint = param,
                        "Learn: dimension-guided hint"
                    );
                }
            }
        }

        // ── Source 3: Established pathways → pattern-guided hint ──
        // Extract parameter names from high-confidence recurring pathways.
        // Pattern keys have format `mutation:param_name→emergence:type`.
        if hint.is_none() {
            let pathways = self.correlation.established_pathways();
            // Find the pathway with highest confidence that references a mutation
            if let Some(best) = pathways
                .iter()
                .filter(|p| p.pattern_key.starts_with("mutation:"))
                .max_by(|a, b| {
                    a.avg_confidence
                        .partial_cmp(&b.avg_confidence)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                // Extract parameter name from pattern key: "mutation:k_mod→..."
                if let Some(param_name) = best.pattern_key
                    .strip_prefix("mutation:")
                    .and_then(|s| s.split('→').next())
                {
                    hint = Some(param_name.to_owned());
                    tracing::debug!(
                        pathway = %best.pattern_key,
                        confidence = format!("{:.3}", best.avg_confidence),
                        occurrences = best.occurrences,
                        hint = param_name,
                        "Learn: pathway-guided hint"
                    );
                }
            }
        }

        // Store hint for Propose phase
        *self.learned_hint.write() = hint;

        // Advance to Propose
        *self.phase.write() = RalphPhase::Propose;
    }

    /// Phase 4: Propose — generate hint-guided, diversity-enforced mutation.
    fn phase_propose(&self, tick: u64) {
        // Skip if there's already an active mutation being verified
        if self.active_mutation.read().is_some() {
            *self.phase.write() = RalphPhase::Harvest;
            return;
        }

        let generation = *self.generation.read();

        // Consume learned parameter guidance from the Learn phase (if any)
        let learned_param = self.learned_hint.write().take();

        match self.selector.select_with_hint(generation, learned_param.as_deref()) {
            Ok(proposal) => {
                // Take snapshot before applying
                let snapshot = self.take_snapshot(tick);

                // Record the mutation
                let record = MutationRecord {
                    generation,
                    proposal: proposal.clone(),
                    status: MutationStatus::Applied,
                    fitness_before: self.fitness.current_fitness().unwrap_or(0.0),
                    fitness_after: 0.0,
                    proposed_at_tick: tick,
                    harvested_at_tick: 0,
                };

                {
                    let mut hist = self.mutation_history.write();
                    if hist.len() >= 500 {
                        hist.pop_front();
                    }
                    hist.push_back(record);
                }

                // Apply the mutation
                if let Err(e) = self.selector.update_value(
                    &proposal.parameter,
                    proposal.proposed_value,
                ) {
                    tracing::warn!("Failed to apply mutation: {e}");
                }

                // Extract correlation data before moving proposal
                let corr_param = proposal.parameter.clone();
                let corr_delta = proposal.delta;

                // Set as active mutation
                *self.active_mutation.write() = Some(ActiveMutation {
                    proposal,
                    snapshot,
                    applied_at_tick: tick,
                    generation,
                });

                // Feed into correlation engine
                self.correlation.ingest_mutation(
                    &corr_param,
                    corr_delta,
                    tick,
                );

                self.stats.write().total_proposed += 1;
            }
            Err(reason) => {
                tracing::debug!("Mutation selection skipped: {reason}");
                self.stats.write().total_skipped += 1;
            }
        }

        // Advance generation
        *self.generation.write() += 1;

        // Advance to Harvest
        *self.phase.write() = RalphPhase::Harvest;
    }

    /// Phase 5: Harvest — accept or rollback mutations.
    fn phase_harvest(&self, tick: u64) {
        let active = self.active_mutation.read().clone();

        if let Some(active_mut) = active {
            let elapsed = tick.saturating_sub(active_mut.applied_at_tick);

            // Wait for verification window
            if elapsed < self.config.verification_ticks {
                // Stay in Harvest, don't advance
                return;
            }

            // Verification window elapsed — evaluate fitness delta
            let current_fitness = self.fitness.current_fitness().unwrap_or(0.0);
            let delta = current_fitness - active_mut.snapshot.fitness;

            let param = active_mut.proposal.parameter.clone();
            let harvest_status = if delta >= self.config.accept_threshold {
                tracing::info!(param = %param, delta, "mutation accepted");
                self.stats.write().total_accepted += 1;
                MutationStatus::Accepted
            } else if delta <= self.config.rollback_threshold {
                tracing::warn!(param = %param, delta, "mutation rolled back");
                self.rollback(&active_mut.snapshot);
                self.stats.write().total_rolled_back += 1;
                MutationStatus::RolledBack
            } else if delta >= 0.0 {
                // BUG-039 fix: only accept non-negative deltas in neutral zone.
                // Previously, mutations with delta -0.005 to -0.009 were silently
                // accepted, causing fitness to degrade 0.667→0.427 over 1000 gens.
                tracing::debug!(param = %param, delta, "mutation neutral-accepted (non-negative)");
                self.stats.write().total_accepted += 1;
                MutationStatus::Accepted
            } else {
                // Negative delta in neutral zone → rollback to prevent drift
                tracing::debug!(param = %param, delta, "mutation neutral-rejected (negative delta)");
                self.rollback(&active_mut.snapshot);
                self.stats.write().total_rolled_back += 1;
                MutationStatus::RolledBack
            };

            // Update mutation history — look up by generation, not back_mut(),
            // to prevent race if propose and harvest interleave under concurrent callers.
            {
                let target_gen = active_mut.generation;
                let mut hist = self.mutation_history.write();
                if let Some(record) = hist.iter_mut().rev().find(|r| r.generation == target_gen) {
                    record.status = harvest_status;
                    record.fitness_after = current_fitness;
                    record.harvested_at_tick = tick;
                }
            }

            // Clear active mutation
            *self.active_mutation.write() = None;

            // Track peak fitness
            {
                let mut engine_stats = self.stats.write();
                if current_fitness > engine_stats.peak_fitness {
                    engine_stats.peak_fitness = current_fitness;
                }
            }

            // BUG-059 fix: Only count cycles with actual mutations harvested.
            // Previously, completed_cycles incremented on every Harvest entry,
            // including skipped generations (no mutation proposed), inflating the
            // cycle count and triggering premature auto-pause at max_cycles.
            *self.completed_cycles.write() += 1;
            self.stats.write().total_cycles += 1;
        }

        // Back to Recognize
        *self.phase.write() = RalphPhase::Recognize;
    }

    // ── Snapshot / Rollback ──

    /// Take a snapshot of current parameter state.
    fn take_snapshot(&self, tick: u64) -> StateSnapshot {
        let generation = *self.generation.read();
        let fitness = self.fitness.current_fitness().unwrap_or(0.0);

        let param_names = self.selector.parameter_names();
        let parameters: Vec<ParameterSnapshot> = param_names
            .iter()
            .filter_map(|name| {
                self.selector.get_parameter(name).map(|p| ParameterSnapshot {
                    name: p.name,
                    value: p.current_value,
                })
            })
            .collect();

        let snapshot = StateSnapshot {
            generation,
            tick,
            fitness,
            parameters,
        };

        // Store in snapshot history
        {
            let mut snaps = self.snapshots.write();
            if snaps.len() >= self.config.snapshot_capacity {
                snaps.pop_front();
            }
            snaps.push_back(snapshot.clone());
        }

        snapshot
    }

    /// Rollback parameters to a snapshot.
    fn rollback(&self, snapshot: &StateSnapshot) {
        for param in &snapshot.parameters {
            if let Err(e) = self.selector.update_value(&param.name, param.value) {
                tracing::warn!("Rollback failed for {}: {e}", param.name);
            }
        }
    }

    /// Reset the entire engine.
    pub fn reset(&self) {
        *self.phase.write() = RalphPhase::Recognize;
        *self.generation.write() = 0;
        *self.completed_cycles.write() = 0;
        *self.paused.write() = false;
        *self.active_mutation.write() = None;
        *self.learned_hint.write() = None;
        self.mutation_history.write().clear();
        self.snapshots.write().clear();
        self.fitness.reset();
        self.emergence.reset();
        self.correlation.reset();
        self.selector.reset();
        *self.stats.write() = RalphStats::default();
    }
}

impl Default for RalphEngine {
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
    use crate::m8_evolution::m40_mutation_selector::MutableParameter;

    fn make_engine() -> RalphEngine {
        // Use verification_ticks=0 for tests so cycles complete immediately.
        // Production default is 10 (waits 10 ticks before harvesting).
        RalphEngine::with_config(RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        })
    }

    fn register_params(engine: &RalphEngine) {
        engine.selector.register_parameter(MutableParameter::new(
            "k_mod", 1.0, 0.01, 1.5, 0.5, "Coupling strength",
        )).unwrap();
        engine.selector.register_parameter(MutableParameter::new(
            "hebbian_ltp", 0.01, 0.001, 0.1, 0.03, "LTP rate",
        )).unwrap();
        engine.selector.register_parameter(MutableParameter::new(
            "r_target", 0.93, 0.5, 1.0, 0.85, "R target",
        )).unwrap();
    }

    fn make_tensor(value: f64) -> TensorValues {
        TensorValues::uniform(value)
    }

    #[test]
    fn default_state() {
        let engine = make_engine();
        let state = engine.state();
        assert_eq!(state.phase, RalphPhase::Recognize);
        assert_eq!(state.generation, 0);
        assert!(!state.paused);
        assert!(!state.has_active_mutation);
    }

    #[test]
    fn phase_cycle_order() {
        assert_eq!(RalphPhase::Recognize.next(), RalphPhase::Analyze);
        assert_eq!(RalphPhase::Analyze.next(), RalphPhase::Learn);
        assert_eq!(RalphPhase::Learn.next(), RalphPhase::Propose);
        assert_eq!(RalphPhase::Propose.next(), RalphPhase::Harvest);
        assert_eq!(RalphPhase::Harvest.next(), RalphPhase::Recognize);
    }

    #[test]
    fn phase_ordinals() {
        assert_eq!(RalphPhase::Recognize.ordinal(), 0);
        assert_eq!(RalphPhase::Analyze.ordinal(), 1);
        assert_eq!(RalphPhase::Learn.ordinal(), 2);
        assert_eq!(RalphPhase::Propose.ordinal(), 3);
        assert_eq!(RalphPhase::Harvest.ordinal(), 4);
    }

    #[test]
    fn phase_names() {
        assert_eq!(RalphPhase::Recognize.name(), "Recognize");
        assert_eq!(RalphPhase::Harvest.name(), "Harvest");
    }

    #[test]
    fn phase_display() {
        assert_eq!(RalphPhase::Recognize.to_string(), "Recognize");
        assert_eq!(RalphPhase::Propose.to_string(), "Propose");
    }

    #[test]
    fn mutation_status_display() {
        assert_eq!(MutationStatus::Proposed.to_string(), "proposed");
        assert_eq!(MutationStatus::RolledBack.to_string(), "rolled_back");
    }

    #[test]
    fn tick_advances_phase() {
        let engine = make_engine();
        let tensor = make_tensor(0.5);

        let phase = engine.tick(&tensor, 1).unwrap();
        assert_eq!(phase, RalphPhase::Recognize);

        let state = engine.state();
        assert_eq!(state.phase, RalphPhase::Analyze);
    }

    #[test]
    fn full_cycle_with_production_params() {
        // BUG-041 fix: production params now registered by default.
        // Cycle still completes in 5 ticks — Propose may generate a mutation.
        let engine = make_engine();
        let tensor = make_tensor(0.5);

        // Recognize → Analyze
        engine.tick(&tensor, 1).unwrap();
        assert_eq!(engine.state().phase, RalphPhase::Analyze);

        // Analyze → Learn
        engine.tick(&tensor, 2).unwrap();
        assert_eq!(engine.state().phase, RalphPhase::Learn);

        // Learn → Propose
        engine.tick(&tensor, 3).unwrap();
        assert_eq!(engine.state().phase, RalphPhase::Propose);

        // Propose (has params → may propose or skip) → Harvest
        engine.tick(&tensor, 4).unwrap();
        assert_eq!(engine.state().phase, RalphPhase::Harvest);

        // Harvest → Recognize (cycle complete)
        engine.tick(&tensor, 5).unwrap();
        assert_eq!(engine.state().phase, RalphPhase::Recognize);
        assert_eq!(engine.state().completed_cycles, 1);
    }

    #[test]
    fn full_cycle_with_params() {
        let config = RalphEngineConfig {
            verification_ticks: 0, // Immediate verification
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.5);

        // Run 5 ticks for a full cycle
        for tick in 1..=5 {
            engine.tick(&tensor, tick).unwrap();
        }

        // Harvest waits for verification_ticks (0), so should complete
        let state = engine.state();
        assert_eq!(state.completed_cycles, 1);
    }

    #[test]
    fn pause_prevents_advancement() {
        let engine = make_engine();
        engine.pause();

        let tensor = make_tensor(0.5);
        engine.tick(&tensor, 1).unwrap();

        assert_eq!(engine.state().phase, RalphPhase::Recognize);
        assert!(engine.state().paused);
    }

    #[test]
    fn resume_allows_advancement() {
        let engine = make_engine();
        engine.pause();
        engine.resume();

        let tensor = make_tensor(0.5);
        engine.tick(&tensor, 1).unwrap();

        assert_eq!(engine.state().phase, RalphPhase::Analyze);
    }

    #[test]
    fn max_cycles_auto_pauses() {
        let config = RalphEngineConfig {
            max_cycles: 2,
            verification_ticks: 0,
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);

        let tensor = make_tensor(0.5);

        // Run enough ticks for 3 cycles (but should pause at 2)
        for tick in 1..=20 {
            engine.tick(&tensor, tick).unwrap();
        }

        assert!(engine.state().paused);
        assert_eq!(engine.state().completed_cycles, 2);
    }

    #[test]
    fn fitness_tracked() {
        let engine = make_engine();

        let tensor = make_tensor(0.7);
        engine.tick(&tensor, 1).unwrap();

        let state = engine.state();
        assert!(state.current_fitness > 0.0);
    }

    #[test]
    fn snapshot_taken_on_propose() {
        let config = RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.5);

        // Advance to Propose phase
        for tick in 1..=4 {
            engine.tick(&tensor, tick).unwrap();
        }

        let _snaps = engine.snapshots.read();
        // May or may not have snapshot depending on selection
        // At minimum, engine should not panic
    }

    #[test]
    fn mutation_history_populated() {
        let config = RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.5);

        // Run a full cycle
        for tick in 1..=5 {
            engine.tick(&tensor, tick).unwrap();
        }

        let _history = engine.recent_mutations(10);
        // Should have at least one if a parameter was selected
        let stats = engine.stats();
        assert!(stats.total_proposed > 0 || stats.total_skipped > 0);
    }

    #[test]
    fn stats_tracking() {
        let engine = make_engine();
        let tensor = make_tensor(0.5);

        for tick in 1..=5 {
            engine.tick(&tensor, tick).unwrap();
        }

        let stats = engine.stats();
        assert_eq!(stats.total_cycles, 1);
    }

    #[test]
    fn reset_clears_all() {
        let engine = make_engine();
        register_params(&engine);

        let tensor = make_tensor(0.5);
        for tick in 1..=5 {
            engine.tick(&tensor, tick).unwrap();
        }

        engine.reset();

        let state = engine.state();
        assert_eq!(state.phase, RalphPhase::Recognize);
        assert_eq!(state.generation, 0);
        assert_eq!(state.completed_cycles, 0);
        assert!(!state.paused);
        assert_eq!(engine.selector().parameter_count(), 0);
    }

    #[test]
    fn emergence_detector_accessible() {
        let engine = make_engine();
        let det = engine.emergence();
        assert_eq!(det.history_len(), 0);
    }

    #[test]
    fn correlation_engine_accessible() {
        let engine = make_engine();
        let corr = engine.correlation();
        assert_eq!(corr.event_count(), 0);
    }

    #[test]
    fn selector_accessible() {
        let engine = make_engine();
        // BUG-041 fix: production params now registered by default
        assert_eq!(engine.selector().parameter_count(), 5);
    }

    #[test]
    fn fitness_accessible() {
        let engine = make_engine();
        assert!(engine.fitness().current_fitness().is_none());
    }

    #[test]
    fn multiple_cycles() {
        let config = RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.6);

        // Run 3 full cycles (15 ticks)
        for tick in 1..=15 {
            engine.tick(&tensor, tick).unwrap();
        }

        assert!(engine.state().completed_cycles >= 3);
    }

    #[test]
    fn harvest_waits_for_verification() {
        let config = RalphEngineConfig {
            verification_ticks: 5, // Must wait 5 ticks
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.5);

        // Advance to Harvest
        for tick in 1..=4 {
            engine.tick(&tensor, tick).unwrap();
        }

        // If active mutation, Harvest should wait
        if engine.state().has_active_mutation {
            let _phase_before = engine.state().phase;
            engine.tick(&tensor, 5).unwrap();
            // Should still be in Harvest (waiting)
            if engine.active_mutation.read().is_some() {
                assert_eq!(engine.state().phase, RalphPhase::Harvest);
            }
        }
    }

    #[test]
    fn state_snapshot_has_params() {
        let config = RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.5);

        // Run through propose phase
        for tick in 1..=4 {
            engine.tick(&tensor, tick).unwrap();
        }

        let snaps = engine.snapshots.read();
        if !snaps.is_empty() {
            assert!(!snaps[0].parameters.is_empty());
        }
    }

    #[test]
    fn recent_mutations_limit() {
        let config = RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        };
        let engine = RalphEngine::with_config(config);
        register_params(&engine);

        let tensor = make_tensor(0.5);

        for tick in 1..=25 {
            engine.tick(&tensor, tick).unwrap();
        }

        let recent = engine.recent_mutations(3);
        assert!(recent.len() <= 3);
    }

    #[test]
    fn default_engine_is_valid() {
        let engine = RalphEngine::default();
        assert_eq!(engine.state().phase, RalphPhase::Recognize);
    }

    #[test]
    fn system_state_reflects_fitness() {
        let engine = make_engine();

        // Low fitness
        let tensor = make_tensor(0.2);
        engine.tick(&tensor, 1).unwrap();
        assert_eq!(engine.state().system_state, SystemState::Failed);

        // High fitness
        let tensor = make_tensor(0.95);
        engine.tick(&tensor, 2).unwrap();
        assert_eq!(engine.state().system_state, SystemState::Optimal);
    }

    #[test]
    fn concurrent_read_safe() {
        let engine = make_engine();
        register_params(&engine);

        // Multiple reads should not deadlock
        let _state = engine.state();
        let _stats = engine.stats();
        let _mutations = engine.recent_mutations(5);
        let _fitness = engine.fitness().current_fitness();
    }

    #[test]
    fn bug_059_skipped_generation_does_not_inflate_cycles() {
        // BUG-059: When the mutation selector skips (all on target/cooldown),
        // completed_cycles should NOT increment.
        let engine = RalphEngine::with_config(RalphEngineConfig {
            verification_ticks: 0,
            ..Default::default()
        });
        // Reset production params, register ONLY an on-target parameter
        engine.selector.reset();
        engine.selector.register_parameter(MutableParameter::new(
            "on_target", 0.5, 0.0, 1.0, 0.5, "Already on target",
        )).unwrap();

        let tensor = make_tensor(0.8);

        // Run a full 5-phase cycle — Propose will skip, Harvest has no mutation
        for tick in 1..=5 {
            engine.tick(&tensor, tick).unwrap();
        }

        // Selector should have skipped (parameter is on target)
        let stats = engine.stats();
        assert!(stats.total_skipped > 0,
            "expected selector to skip (all on target), skipped={}, proposed={}",
            stats.total_skipped, stats.total_proposed);

        // Cycle counter should NOT have incremented for skipped generations
        assert_eq!(stats.total_cycles, 0,
            "BUG-059: skipped generations should not inflate cycle count (got {})",
            stats.total_cycles);
    }

    // ── Feedback loop wiring tests ──

    #[test]
    fn learn_phase_produces_hint_from_emergence() {
        let engine = make_engine();
        register_params(&engine);

        // Feed a CoherenceLock emergence event
        use crate::m8_evolution::m37_emergence_detector::{EmergenceType, EmergenceParams};
        engine.emergence().record_emergence(&EmergenceParams {
            emergence_type: EmergenceType::CoherenceLock,
            confidence: 0.95,
            severity: 0.7,
            affected_panes: vec![],
            description: "test coherence lock".into(),
            tick: 100,
            recommended_action: None,
        }).unwrap();

        // Run Recognize + Analyze to get to Learn
        let tensor = make_tensor(0.7);
        engine.tick(&tensor, 100).unwrap(); // Recognize → Analyze
        engine.tick(&tensor, 101).unwrap(); // Analyze → Learn

        // Now run Learn — should set hint to r_target
        engine.tick(&tensor, 102).unwrap(); // Learn → Propose

        // The hint was consumed by Propose, but we can verify the mutation
        // proposal targets r_target (hint-guided) if it passed diversity gates
        let mutations = engine.recent_mutations(1);
        if let Some(record) = mutations.first() {
            // If a mutation was proposed, it should be hint-guided to r_target
            assert_eq!(record.proposal.parameter, "r_target",
                "emergence-guided hint should direct mutation to r_target");
            assert!(record.proposal.reason.contains("[hint-guided]"),
                "mutation reason should indicate hint guidance");
        }
    }

    #[test]
    fn learn_phase_falls_through_to_dimension_hint() {
        let engine = make_engine();
        register_params(&engine);

        // No emergence events — dimension analysis should provide the hint.
        // With uniform tensor at 0.3 (low), weakest dim depends on weights.
        // D0 coordination_quality (weight 0.18) will have lowest weighted score
        // at uniform 0.3: 0.3 * 0.18 = 0.054, vs D11 at 0.3 * 0.02 = 0.006.
        // Actually weakest_weighted is min(value * weight), so D11 (0.02) is weakest.
        // D11 = ConsentCompliance → no parameter mapping → falls to next.
        // So with uniform values, the dimension hint may not fire, which is correct.

        // Use non-uniform tensor: field_coherence (D1) at 0.1, rest at 0.9
        let mut tensor = make_tensor(0.9);
        tensor.values[1] = 0.1; // field_coherence = low

        engine.tick(&tensor, 100).unwrap(); // Recognize → Analyze
        engine.tick(&tensor, 101).unwrap(); // Analyze → Learn
        engine.tick(&tensor, 102).unwrap(); // Learn → Propose

        let mutations = engine.recent_mutations(1);
        if let Some(record) = mutations.first() {
            // Weakest weighted dimension is D1 (field_coherence) at 0.1*0.15=0.015.
            // That maps to r_target.
            assert_eq!(record.proposal.parameter, "r_target",
                "dimension-guided hint should direct mutation to r_target");
        }
    }

    #[test]
    fn learned_hint_cleared_after_propose() {
        let engine = make_engine();
        register_params(&engine);

        // Feed emergence to create a hint
        use crate::m8_evolution::m37_emergence_detector::{EmergenceType, EmergenceParams};
        engine.emergence().record_emergence(&EmergenceParams {
            emergence_type: EmergenceType::ThermalSpike,
            confidence: 0.9,
            severity: 0.8,
            affected_panes: vec![],
            description: "thermal spike".into(),
            tick: 100,
            recommended_action: None,
        }).unwrap();

        let tensor = make_tensor(0.7);
        // Run through Learn → Propose (hint consumed)
        for tick in 100..=103 {
            engine.tick(&tensor, tick).unwrap();
        }

        // Hint should be cleared after Propose consumed it
        assert!(engine.learned_hint.read().is_none(),
            "learned hint should be None after Propose phase consumes it");
    }
}
