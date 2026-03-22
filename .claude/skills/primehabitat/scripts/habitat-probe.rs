#!/usr/bin/env rust-script
//! Habitat Probe — fast system intelligence for /primehabitat and /deephabitat skills.
//!
//! Replaces inline curl chains with a compiled, typed probe that returns
//! structured JSON. Runs without loading into context (black-box execution).
//!
//! Usage:
//!   habitat-probe pulse          # Quick 3-service check (~30ms)
//!   habitat-probe sweep          # All 16 services health check
//!   habitat-probe field          # PV field state + decision + tunnels
//!   habitat-probe spheres        # Active sphere listing
//!   habitat-probe bus            # Bus state (tasks, events, cascades)
//!   habitat-probe me             # ME observer + fitness breakdown
//!   habitat-probe bridges        # Bridge staleness check
//!   habitat-probe full           # Everything above in one shot
//!
//! ```cargo
//! [dependencies]
//! ureq = "2"
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! ```

use serde_json::{json, Value};
use std::env;
use std::time::Instant;

/// Fire-and-forget GET, returns JSON or error object.
fn get_json(url: &str) -> Value {
    match ureq::get(url).timeout(std::time::Duration::from_secs(2)).call() {
        Ok(resp) => {
            let body = resp.into_string().unwrap_or_default();
            serde_json::from_str(&body).unwrap_or(json!({"raw": body}))
        }
        Err(e) => json!({"error": e.to_string()}),
    }
}

/// Check single port health, return (port, status_code, latency_ms).
fn check_port(port: u16, path: &str) -> (u16, u16, u128) {
    let url = format!("http://127.0.0.1:{port}{path}");
    let start = Instant::now();
    let code = match ureq::get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .call()
    {
        Ok(resp) => resp.status(),
        Err(ureq::Error::Status(code, _)) => code,
        Err(_) => 0,
    };
    (port, code, start.elapsed().as_millis())
}

fn health_path(port: u16) -> &'static str {
    match port {
        8080 | 8090 => "/api/health",
        _ => "/health",
    }
}

fn pulse() -> Value {
    let pv = get_json("http://127.0.0.1:8132/health");
    let povm = get_json("http://127.0.0.1:8125/hydrate");
    let me = get_json("http://127.0.0.1:8080/api/observer");

    json!({
        "pv": {
            "status": pv.get("status"),
            "tick": pv.get("tick"),
            "spheres": pv.get("spheres"),
            "r": pv.get("r"),
            "fleet_mode": pv.get("fleet_mode"),
            "k": pv.get("k"),
        },
        "povm": {
            "memories": povm.get("memory_count"),
            "pathways": povm.get("pathway_count"),
        },
        "me": {
            "fitness": me.pointer("/last_report/current_fitness"),
            "trend": me.get("fitness_trend"),
            "state": me.get("system_state"),
            "tick": me.get("tick_count"),
        },
    })
}

fn sweep() -> Value {
    let ports: Vec<u16> = vec![
        8080, 8081, 8090, 8100, 8101, 8102, 8103, 8104, 8105, 8110, 8120, 8125, 8130, 8132,
        9001, 10001,
    ];

    let start = Instant::now();
    let results: Vec<Value> = ports
        .iter()
        .map(|&p| {
            let (port, code, ms) = check_port(p, health_path(p));
            json!({"port": port, "status": code, "ms": ms})
        })
        .collect();

    let healthy = results.iter().filter(|r| r["status"] == 200).count();
    let total_ms = start.elapsed().as_millis();

    json!({
        "services": results,
        "healthy": healthy,
        "total": ports.len(),
        "sweep_ms": total_ms,
    })
}

fn field() -> Value {
    let health = get_json("http://127.0.0.1:8132/health");
    let r = get_json("http://127.0.0.1:8132/field/r");
    let decision = get_json("http://127.0.0.1:8132/field/decision");

    json!({
        "tick": health.get("tick"),
        "spheres": health.get("spheres"),
        "fleet_mode": health.get("fleet_mode"),
        "r": r.get("r"),
        "psi": r.get("psi"),
        "k": health.get("k"),
        "k_modulation": health.get("k_modulation"),
        "action": decision.get("action"),
        "tunnel_count": decision.get("tunnel_count"),
        "working": decision.get("working_spheres"),
        "idle": decision.get("idle_spheres"),
        "strongest_tunnel": decision.get("strongest_tunnel"),
    })
}

fn spheres() -> Value {
    get_json("http://127.0.0.1:8132/spheres")
}

fn bus() -> Value {
    let info = get_json("http://127.0.0.1:8132/bus/info");
    let tasks = get_json("http://127.0.0.1:8132/bus/tasks");
    let cascades = get_json("http://127.0.0.1:8132/bus/cascades");
    let events = get_json("http://127.0.0.1:8132/bus/events");

    json!({
        "info": info,
        "pending_tasks": tasks.get("tasks"),
        "pending_cascades": cascades.get("cascades"),
        "recent_events": events.get("events"),
    })
}

fn me_detail() -> Value {
    let observer = get_json("http://127.0.0.1:8080/api/observer");
    let eventbus = get_json("http://127.0.0.1:8080/api/eventbus/stats");
    let health = get_json("http://127.0.0.1:8080/api/health");

    json!({
        "observer": observer,
        "eventbus": eventbus,
        "health": health,
    })
}

fn bridges() -> Value {
    get_json("http://127.0.0.1:8132/bridges/health")
}

fn full() -> Value {
    json!({
        "pulse": pulse(),
        "sweep": sweep(),
        "field": field(),
        "bus": bus(),
        "bridges": bridges(),
    })
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("pulse");

    let result = match cmd {
        "pulse" => pulse(),
        "sweep" => sweep(),
        "field" => field(),
        "spheres" => spheres(),
        "bus" => bus(),
        "me" => me_detail(),
        "bridges" => bridges(),
        "full" => full(),
        "--help" | "-h" | "help" => {
            eprintln!("habitat-probe — Fast system intelligence for The Habitat");
            eprintln!();
            eprintln!("USAGE: habitat-probe <command>");
            eprintln!();
            eprintln!("COMMANDS:");
            eprintln!("  pulse     Quick 3-service check (PV + POVM + ME) ~30ms");
            eprintln!("  sweep     All 16 services health check");
            eprintln!("  field     PV field state, decision engine, tunnels");
            eprintln!("  spheres   Active sphere listing with phases");
            eprintln!("  bus       Bus state (tasks, events, cascades)");
            eprintln!("  me        ME observer + fitness + EventBus stats");
            eprintln!("  bridges   Bridge staleness check");
            eprintln!("  full      Everything above in one shot");
            std::process::exit(0);
        }
        other => {
            eprintln!("Unknown command: {other}. Run with --help for usage.");
            std::process::exit(1);
        }
    };

    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
}
