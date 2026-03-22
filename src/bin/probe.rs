//! # `orac-probe` — Diagnostic probe for ORAC sidecar
//!
//! Quick connectivity and health checks for the ORAC ecosystem.
//! Tests: HTTP hook server, PV2 IPC bus, bridge endpoints.

use std::process::ExitCode;

fn main() -> ExitCode {
    println!("ORAC probe — connectivity diagnostics\n");

    let checks = [
        ("ORAC HTTP", "127.0.0.1:8133", "/health"),
        ("PV2 daemon", "127.0.0.1:8132", "/health"),
        ("SYNTHEX", "127.0.0.1:8090", "/api/health"),
        ("ME", "127.0.0.1:8080", "/api/health"),
        ("POVM", "127.0.0.1:8125", "/health"),
        ("RM", "127.0.0.1:8130", "/health"),
    ];

    let mut failures = 0_u32;

    for (name, addr, path) in &checks {
        let url = format!("http://{addr}{path}");
        if let Ok(resp) = ureq::get(&url).timeout(std::time::Duration::from_secs(2)).call() {
            println!("  [{:>3}] {name:<14} {addr}", resp.status());
        } else {
            println!("  [---] {name:<14} {addr} (unreachable)");
            failures = failures.saturating_add(1);
        }
    }

    println!();
    if failures == 0 {
        println!("All {n} endpoints reachable.", n = checks.len());
        ExitCode::SUCCESS
    } else {
        println!("{failures}/{n} endpoints unreachable.", n = checks.len());
        ExitCode::FAILURE
    }
}
