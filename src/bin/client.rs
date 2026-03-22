//! # `orac-client` — CLI client for ORAC sidecar
//!
//! Sends commands to the running ORAC daemon via HTTP.
//! Usage: `orac-client status | field | blackboard | metrics`

use std::process::ExitCode;
use std::time::Duration;

/// ORAC HTTP base address.
const ORAC_ADDR: &str = "127.0.0.1:8133";

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(3);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map_or("help", String::as_str);

    match cmd {
        "status" => cmd_status(),
        "field" => cmd_field(),
        "blackboard" => cmd_blackboard(),
        "metrics" => cmd_metrics(),
        "help" | "--help" | "-h" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("orac-client: unknown command '{other}'");
            eprintln!("Run 'orac-client help' for usage.");
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!("orac-client — CLI for ORAC sidecar");
    println!("Usage: orac-client <command>");
    println!();
    println!("Commands:");
    println!("  status      Show sidecar health and session info");
    println!("  field       Show Kuramoto field state (r, K, spheres)");
    println!("  blackboard  Show fleet blackboard state");
    println!("  metrics     Dump Prometheus-format metrics (raw)");
}

/// GET /health — pretty-print ORAC status.
fn cmd_status() -> ExitCode {
    let url = format!("http://{ORAC_ADDR}/health");
    let body = match fetch(&url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ORAC unreachable at {ORAC_ADDR}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
        println!("{body}");
        return ExitCode::SUCCESS;
    };

    println!("ORAC Sidecar Status");
    println!("-------------------");
    print_field("status", &v["status"]);
    print_field("service", &v["service"]);
    print_field("port", &v["port"]);
    print_field("sessions", &v["sessions"]);
    print_field("uptime", &v["uptime"]);
    print_field("version", &v["version"]);

    // Print any remaining top-level keys not already shown
    if let Some(obj) = v.as_object() {
        let shown = ["status", "service", "port", "sessions", "uptime", "version"];
        for (k, val) in obj {
            if !shown.contains(&k.as_str()) && !val.is_null() {
                print_field(k, val);
            }
        }
    }

    ExitCode::SUCCESS
}

/// GET /field — show Kuramoto field state.
fn cmd_field() -> ExitCode {
    let url = format!("http://{ORAC_ADDR}/field");
    let body = match fetch(&url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ORAC unreachable at {ORAC_ADDR}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
        println!("{body}");
        return ExitCode::SUCCESS;
    };

    println!("Kuramoto Field State");
    println!("--------------------");
    print_field("r (order)", &v["r"]);
    print_field("K (coupling)", &v["K"]);
    print_field("spheres", &v["spheres"]);
    print_field("psi (mean phase)", &v["psi"]);
    print_field("tick", &v["tick"]);
    print_field("chimeras", &v["chimeras"]);

    if let Some(obj) = v.as_object() {
        let shown = ["r", "K", "spheres", "psi", "tick", "chimeras"];
        for (k, val) in obj {
            if !shown.contains(&k.as_str()) && !val.is_null() {
                print_field(k, val);
            }
        }
    }

    ExitCode::SUCCESS
}

/// GET /blackboard — show fleet blackboard state.
fn cmd_blackboard() -> ExitCode {
    let url = format!("http://{ORAC_ADDR}/blackboard");
    let body = match fetch(&url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ORAC unreachable at {ORAC_ADDR}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
        println!("{body}");
        return ExitCode::SUCCESS;
    };

    println!("Fleet Blackboard");
    println!("----------------");

    // If array (list of pane statuses), print as table
    if let Some(arr) = v.as_array() {
        if arr.is_empty() {
            println!("(no entries)");
        } else {
            for entry in arr {
                print_blackboard_entry(entry);
            }
        }
    } else if let Some(obj) = v.as_object() {
        // Object with sub-keys
        for (k, val) in obj {
            if let Some(arr) = val.as_array() {
                println!("\n{k} ({} entries):", arr.len());
                for entry in arr {
                    print_blackboard_entry(entry);
                }
            } else {
                print_field(k, val);
            }
        }
    } else {
        println!("{body}");
    }

    ExitCode::SUCCESS
}

/// GET /metrics — raw Prometheus output.
fn cmd_metrics() -> ExitCode {
    let url = format!("http://{ORAC_ADDR}/metrics");
    let body = match fetch(&url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("ORAC unreachable at {ORAC_ADDR}: {e}");
            return ExitCode::FAILURE;
        }
    };

    // Metrics are Prometheus text format — print raw
    print!("{body}");
    ExitCode::SUCCESS
}

// --- helpers ---

/// Fetch a URL and return the response body as a string.
fn fetch(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .timeout(TIMEOUT)
        .call()
        .map_err(|e| format!("{e}"))?;

    let status = resp.status();
    let body = resp
        .into_string()
        .map_err(|e| format!("read body: {e}"))?;

    if status != 200 {
        return Err(format!("HTTP {status}: {body}"));
    }

    Ok(body)
}

/// Pretty-print a single key-value field from JSON.
fn print_field(key: &str, val: &serde_json::Value) {
    if val.is_null() {
        return;
    }
    match val {
        serde_json::Value::String(s) => println!("  {key:<18} {s}"),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if (f - f.round()).abs() < f64::EPSILON {
                    #[allow(clippy::cast_possible_truncation)]
                    let i = f as i64;
                    println!("  {key:<18} {i}");
                } else {
                    println!("  {key:<18} {f:.4}");
                }
            } else {
                println!("  {key:<18} {n}");
            }
        }
        serde_json::Value::Bool(b) => println!("  {key:<18} {b}"),
        serde_json::Value::Array(arr) => println!("  {key:<18} [{} items]", arr.len()),
        serde_json::Value::Object(_) => println!("  {key:<18} {{...}}"),
        serde_json::Value::Null => {}
    }
}

/// Print a single blackboard entry (pane status or task).
fn print_blackboard_entry(entry: &serde_json::Value) {
    if let Some(obj) = entry.as_object() {
        let pane = obj
            .get("pane_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let status = obj
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?");
        let tool = obj
            .get("tool_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("-");
        println!("  {pane:<24} {status:<12} {tool}");
    } else {
        println!("  {entry}");
    }
}
