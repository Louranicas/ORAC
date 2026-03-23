//! L8 Evolution integration tests — RALPH loop.
//!
//! Tests the autonomous evolution system:
//! - RALPH iteration cycle (Recognize → Analyze → Learn → Propose → Harvest)
//! - Generation advancement across ticks
//! - Phase progression through the 5-phase cycle
//! - Pause at `max_cycles` threshold

#![cfg(feature = "evolution")]

mod common;

use orac_sidecar::m8_evolution::m36_ralph_engine::{RalphEngine, RalphEngineConfig, RalphPhase};
use orac_sidecar::m8_evolution::m39_fitness_tensor::TensorValues;

/// Build a mock tensor with plausible values for testing.
fn mock_tensor() -> TensorValues {
    let mut vals = [0.5_f64; 12];
    vals[0] = 0.8;  // coordination_quality
    vals[1] = 0.95; // field_coherence
    vals[3] = 0.75; // bridge_health
    vals[11] = 0.7; // overall_fitness
    TensorValues { values: vals }
}

#[test]
fn ralph_ticks_advance_generation() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    // Initial state
    let state = engine.state();
    assert_eq!(state.generation, 0);
    assert_eq!(state.phase, RalphPhase::Recognize);

    // Tick through one full RALPH cycle (5 phases)
    for tick in 0..5 {
        let result = engine.tick(&tensor, tick);
        assert!(result.is_ok(), "tick {tick} failed: {:?}", result.err());
    }

    // After 5 ticks: one full cycle completed, generation should have advanced
    let state = engine.state();
    assert!(
        state.generation > 0,
        "generation should advance after a full cycle, got {}",
        state.generation,
    );
}

#[test]
fn ralph_ten_ticks_advance_generation_monotonically() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    let mut prev_gen = 0_u64;
    for tick in 0..10 {
        let _ = engine.tick(&tensor, tick);
        let gen = engine.state().generation;
        assert!(
            gen >= prev_gen,
            "generation must not decrease: was {prev_gen}, now {gen} at tick {tick}",
        );
        prev_gen = gen;
    }

    // After 10 ticks (2 full cycles), generation should be at least 2
    assert!(
        prev_gen >= 2,
        "expected generation >= 2 after 10 ticks, got {prev_gen}",
    );
}

#[test]
fn ralph_phases_cycle_through_all_five() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    let mut observed_phases = Vec::new();
    for tick in 0..5 {
        let phase = engine.tick(&tensor, tick).unwrap();
        observed_phases.push(phase);
    }

    assert_eq!(observed_phases[0], RalphPhase::Recognize);
    assert_eq!(observed_phases[1], RalphPhase::Analyze);
    assert_eq!(observed_phases[2], RalphPhase::Learn);
    assert_eq!(observed_phases[3], RalphPhase::Propose);
    // Phase 4 is Harvest, but it may stay in Harvest if verification_ticks > 0
}

#[test]
fn ralph_paused_does_not_advance() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    engine.pause();

    for tick in 0..10 {
        let _ = engine.tick(&tensor, tick);
    }

    let state = engine.state();
    assert_eq!(state.generation, 0, "paused engine should not advance generation");
    assert!(state.paused);
}

#[test]
fn ralph_resume_after_pause() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    engine.pause();
    let _ = engine.tick(&tensor, 0);
    assert_eq!(engine.state().generation, 0);

    engine.resume();
    for tick in 1..6 {
        let _ = engine.tick(&tensor, tick);
    }
    assert!(engine.state().generation > 0, "resumed engine should advance");
}

#[test]
fn ralph_auto_pauses_at_max_cycles() {
    let config = RalphEngineConfig {
        max_cycles: 2,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = mock_tensor();

    // Run enough ticks for 3 full cycles (15 ticks)
    for tick in 0..15 {
        let _ = engine.tick(&tensor, tick);
    }

    let state = engine.state();
    assert!(state.paused, "engine should auto-pause after max_cycles");
    assert!(
        state.completed_cycles >= 2,
        "should have completed at least 2 cycles, got {}",
        state.completed_cycles,
    );
}

#[test]
fn ralph_stats_track_proposals() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    // Run 10 ticks
    for tick in 0..10 {
        let _ = engine.tick(&tensor, tick);
    }

    let stats = engine.stats();
    // After 2 full cycles, there should be some proposal activity
    let total = stats.total_proposed + stats.total_skipped;
    assert!(
        total >= 1,
        "should have at least 1 proposal attempt, got proposed={} skipped={}",
        stats.total_proposed,
        stats.total_skipped,
    );
}

#[test]
fn ralph_fitness_tensor_accessible() {
    let engine = RalphEngine::new();
    let tensor = mock_tensor();

    // Tick through Recognize + Analyze (which calls fitness.evaluate)
    let _ = engine.tick(&tensor, 0);
    let _ = engine.tick(&tensor, 1);

    // Fitness tensor should have been evaluated
    let fitness = engine.fitness().current_fitness();
    assert!(
        fitness.is_some(),
        "fitness should be evaluated after Recognize+Analyze ticks",
    );
}

#[test]
fn ralph_state_accessor_consistent() {
    let engine = RalphEngine::new();
    let state = engine.state();

    assert_eq!(state.phase, RalphPhase::Recognize);
    assert_eq!(state.generation, 0);
    assert_eq!(state.completed_cycles, 0);
    assert!(!state.paused);
    assert!(!state.has_active_mutation);
}

// ── Convergence tests (default max_cycles=1000) ──

#[test]
fn ralph_convergence_default_config_pauses_at_1000() {
    // Use verification_ticks=0 so each cycle is exactly 5 ticks.
    // This proves the auto-pause mechanism works at the full default limit.
    let config = RalphEngineConfig {
        max_cycles: 1000,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = mock_tensor();

    // 1000 cycles × 5 phases = 5000 ticks minimum. Run 5200 to ensure
    // we're past the limit even with any Harvest stalls.
    for tick in 0..5200 {
        let _ = engine.tick(&tensor, tick);
    }

    let state = engine.state();
    assert!(state.paused, "engine must auto-pause at max_cycles=1000");
    assert!(
        state.completed_cycles >= 1000,
        "completed_cycles must reach 1000, got {}",
        state.completed_cycles,
    );
}

#[test]
fn ralph_convergence_stats_consistent_at_pause() {
    let config = RalphEngineConfig {
        max_cycles: 100,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = mock_tensor();

    for tick in 0..600 {
        let _ = engine.tick(&tensor, tick);
    }

    let state = engine.state();
    let stats = engine.stats();

    assert!(state.paused);
    assert_eq!(stats.total_cycles, state.completed_cycles);
    // Every cycle passes through Propose, which either proposes or skips
    assert!(
        stats.total_proposed + stats.total_skipped >= state.completed_cycles,
        "proposal+skip ({} + {}) must >= completed_cycles ({})",
        stats.total_proposed,
        stats.total_skipped,
        state.completed_cycles,
    );
    // accepted + rolled_back <= proposed
    assert!(
        stats.total_accepted + stats.total_rolled_back <= stats.total_proposed,
        "accepted+rolled_back ({} + {}) must <= proposed ({})",
        stats.total_accepted,
        stats.total_rolled_back,
        stats.total_proposed,
    );
    // Generation must have advanced at least once per cycle (Propose increments it)
    assert!(
        state.generation >= state.completed_cycles,
        "generation ({}) must >= completed_cycles ({})",
        state.generation,
        state.completed_cycles,
    );
}

#[test]
fn ralph_convergence_fitness_evaluated() {
    let config = RalphEngineConfig {
        max_cycles: 50,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = mock_tensor();

    for tick in 0..300 {
        let _ = engine.tick(&tensor, tick);
    }

    let state = engine.state();
    assert!(state.paused);
    // Fitness must have been evaluated (non-zero after Recognize+Analyze)
    assert!(
        state.current_fitness > 0.0,
        "fitness must be > 0 after convergence, got {}",
        state.current_fitness,
    );
    // Peak fitness tracks max seen during Harvest — only updated when
    // an active mutation is harvested, so it may be 0 if all mutations
    // were skipped by the diversity gate. When set, it must be positive.
    let stats = engine.stats();
    if stats.total_accepted + stats.total_rolled_back > 0 {
        assert!(
            stats.peak_fitness > 0.0,
            "peak must be > 0 when mutations were harvested, got {}",
            stats.peak_fitness,
        );
    }
}

#[test]
fn ralph_convergence_no_ticks_after_pause() {
    let config = RalphEngineConfig {
        max_cycles: 5,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = mock_tensor();

    // Run until paused
    for tick in 0..50 {
        let _ = engine.tick(&tensor, tick);
    }
    assert!(engine.state().paused);

    let gen_at_pause = engine.state().generation;
    let cycles_at_pause = engine.state().completed_cycles;

    // Run 100 more ticks — nothing should change
    for tick in 50..150 {
        let _ = engine.tick(&tensor, tick);
    }

    assert_eq!(
        engine.state().generation, gen_at_pause,
        "generation must not advance while paused",
    );
    assert_eq!(
        engine.state().completed_cycles, cycles_at_pause,
        "completed_cycles must not advance while paused",
    );
}

#[test]
fn ralph_convergence_resume_after_max_cycles() {
    let config = RalphEngineConfig {
        max_cycles: 5,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = mock_tensor();

    // Run until paused
    for tick in 0..50 {
        let _ = engine.tick(&tensor, tick);
    }
    assert!(engine.state().paused);
    let gen_before = engine.state().generation;

    // Resume — engine should immediately re-pause because completed_cycles >= max_cycles
    engine.resume();
    let _ = engine.tick(&tensor, 100);

    // After resume + 1 tick, it checks completed_cycles >= max_cycles and re-pauses
    assert!(
        engine.state().paused,
        "engine should re-pause after resume because cycles already >= max",
    );
    assert_eq!(
        engine.state().generation, gen_before,
        "no new generation should be produced",
    );
}
