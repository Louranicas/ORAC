//! Benchmark: RALPH tick CPU cost.
//!
//! Measures wall-clock cost of `RalphEngine::tick()` across varying
//! sphere counts (1, 8, 16, 32, 64). Uses `std::time::Instant` for
//! timing — no external benchmark crate required.
//!
//! Run: `cargo run --release --features full --bin ralph-bench`

#![allow(
    clippy::cast_precision_loss,
    clippy::similar_names,
    clippy::redundant_closure_for_method_calls
)]

#[cfg(not(feature = "evolution"))]
fn main() {
    eprintln!("ralph-bench requires --features evolution (or full)");
    std::process::exit(1);
}

#[cfg(feature = "evolution")]
fn main() {
    let sphere_counts: [u64; 5] = [1, 8, 16, 32, 64];
    let ticks_per_run: u64 = 500;
    let warmup_ticks: u64 = 50;

    println!("RALPH Tick Benchmark");
    println!("====================");
    println!(
        "{:<10} {:>12} {:>12} {:>12} {:>12}",
        "Spheres", "Total (ms)", "Per-tick", "Min", "Max"
    );
    println!("{}", "-".repeat(60));

    for &sphere_count in &sphere_counts {
        run_for_spheres(sphere_count, ticks_per_run, warmup_ticks);
    }

    println!("\nAll measurements at --release optimization level.");
    println!("Per-tick target: < 100 us for 64 spheres (5s interval budget = 5,000,000 us).");
}

#[cfg(feature = "evolution")]
fn run_for_spheres(sphere_count: u64, ticks_per_run: u64, warmup_ticks: u64) {
    use orac_sidecar::m8_evolution::m36_ralph_engine::{RalphEngine, RalphEngineConfig};
    use std::time::Instant;

    let config = RalphEngineConfig {
        max_cycles: 10_000,
        verification_ticks: 0,
        ..RalphEngineConfig::default()
    };
    let engine = RalphEngine::with_config(config);
    let tensor = build_tensor_for_spheres(sphere_count);

    // Warmup
    for tick in 0..warmup_ticks {
        let _ = engine.tick(&tensor, tick);
    }

    // Timed run
    let capacity = usize::try_from(ticks_per_run).unwrap_or(500);
    let mut durations = Vec::with_capacity(capacity);
    for tick in warmup_ticks..(warmup_ticks + ticks_per_run) {
        let start = Instant::now();
        let _ = engine.tick(&tensor, tick);
        durations.push(start.elapsed());
    }

    let total: std::time::Duration = durations.iter().sum();
    let total_us = total.as_micros();
    let per_tick_us = total_us / u128::from(ticks_per_run);
    let min_us = durations.iter().min().map_or(0, |d| d.as_micros());
    let max_us = durations.iter().max().map_or(0, |d| d.as_micros());

    let total_ms = total_us as f64 / 1000.0;

    println!(
        "{sphere_count:<10} {total_ms:>12.2} {per_tick_us:>9} us {min_us:>9} us {max_us:>9} us",
    );

    let es = engine.state();
    let et = engine.stats();
    let gen = es.generation;
    let cyc = es.completed_cycles;
    let pro = et.total_proposed;
    let acc = et.total_accepted;
    let rlb = et.total_rolled_back;
    let fit = es.current_fitness;
    eprintln!(
        "  [spheres={sphere_count}] gen={gen} cycles={cyc} proposed={pro} accepted={acc} rolled_back={rlb} fitness={fit:.4}",
    );
}

#[cfg(feature = "evolution")]
fn build_tensor_for_spheres(
    sphere_count: u64,
) -> orac_sidecar::m8_evolution::m39_fitness_tensor::TensorValues {
    let sc = sphere_count as f64;
    let coordination = (sc / 64.0).min(1.0);
    let r = 1.0 - (sc * 0.001).min(0.1);

    let mut vals = [0.5_f64; 12];
    vals[0] = coordination;
    vals[1] = r;
    vals[2] = 1.0;
    vals[3] = 0.75;
    vals[4] = 1.0;
    vals[5] = 0.5;
    vals[6] = 0.8;
    vals[7] = 0.7;
    vals[8] = 0.9;
    vals[9] = 0.5;
    vals[10] = 0.6;

    let sum: f64 = vals[..11].iter().sum();
    vals[11] = sum / 11.0;

    orac_sidecar::m8_evolution::m39_fitness_tensor::TensorValues { values: vals }
}
