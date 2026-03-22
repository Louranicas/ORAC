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
        "hook-test" => cmd_hook_test(args.get(2).map(String::as_str)),
        "probe" => cmd_probe(),
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
    println!("  status          Show sidecar health and session info");
    println!("  field           Show Kuramoto field state (r, K, spheres)");
    println!("  blackboard      Show fleet blackboard state");
    println!("  metrics         Dump Prometheus-format metrics (raw)");
    println!("  hook-test <evt> Send test payload to a hook endpoint");
    println!("  probe           Run connectivity checks (ORAC, PV2, SYNTHEX, ME, POVM, RM)");
    println!();
    println!("Hook events: SessionStart, Stop, PostToolUse, PreToolUse,");
    println!("             UserPromptSubmit, PermissionRequest");
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

/// POST /hooks/<event> — send a test payload and show the response.
fn cmd_hook_test(event: Option<&str>) -> ExitCode {
    let Some(event) = event else {
        eprintln!("Usage: orac-client hook-test <event>");
        eprintln!("Events: SessionStart, Stop, PostToolUse, PreToolUse,");
        eprintln!("        UserPromptSubmit, PermissionRequest");
        return ExitCode::FAILURE;
    };

    let payload = match event {
        "SessionStart" | "Stop" => serde_json::json!({
            "session_id": "test-session-001"
        }),
        "PostToolUse" => serde_json::json!({
            "tool_name": "Read",
            "tool_input": {"file_path": "/tmp/test"},
            "tool_output": "file contents here"
        }),
        "PreToolUse" => serde_json::json!({
            "tool_name": "Bash",
            "tool_input": {"command": "echo test"}
        }),
        "UserPromptSubmit" => serde_json::json!({
            "prompt": "orac-client hook test"
        }),
        "PermissionRequest" => serde_json::json!({
            "tool_name": "Read",
            "tool_input": {"file_path": "/etc/passwd"}
        }),
        other => {
            eprintln!("Unknown hook event: '{other}'");
            eprintln!("Valid: SessionStart, Stop, PostToolUse, PreToolUse,");
            eprintln!("       UserPromptSubmit, PermissionRequest");
            return ExitCode::FAILURE;
        }
    };

    let url = format!("http://{ORAC_ADDR}/hooks/{event}");
    let body = match post_json(&url, &payload) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Hook request failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("Hook Response ({event})");
    println!("{}", "-".repeat(20 + event.len()));
    format_hook_response(&body);

    ExitCode::SUCCESS
}

/// Run connectivity probe against all ORAC ecosystem services.
fn cmd_probe() -> ExitCode {
    println!("ORAC probe — connectivity diagnostics\n");

    let checks: &[(&str, &str, &str)] = &[
        ("ORAC HTTP", "127.0.0.1:8133", "/health"),
        ("PV2 daemon", "127.0.0.1:8132", "/health"),
        ("SYNTHEX", "127.0.0.1:8090", "/api/health"),
        ("ME", "127.0.0.1:8080", "/api/health"),
        ("POVM", "127.0.0.1:8125", "/health"),
        ("RM", "127.0.0.1:8130", "/health"),
    ];

    let mut failures = 0_u32;

    for (name, addr, path) in checks {
        let url = format!("http://{addr}{path}");
        if let Ok(resp) = ureq::get(&url)
            .timeout(std::time::Duration::from_secs(2))
            .call()
        {
            println!("  [{:>3}] {name:<14} {addr}", resp.status());
        } else {
            println!("  [---] {name:<14} {addr} (unreachable)");
            failures = failures.saturating_add(1);
        }
    }

    println!();
    if failures == 0 {
        println!("All {} endpoints reachable.", checks.len());
        ExitCode::SUCCESS
    } else {
        println!("{failures}/{} endpoints unreachable.", checks.len());
        ExitCode::FAILURE
    }
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

/// POST JSON to a URL and return the parsed response.
fn post_json(url: &str, body: &serde_json::Value) -> Result<serde_json::Value, String> {
    let body_str = serde_json::to_string(body).map_err(|e| format!("serialize: {e}"))?;
    let resp = ureq::post(url)
        .set("Content-Type", "application/json")
        .timeout(TIMEOUT)
        .send_string(&body_str)
        .map_err(|e| format!("{e}"))?;

    let status = resp.status();
    let text = resp
        .into_string()
        .map_err(|e| format!("read body: {e}"))?;

    if status != 200 {
        return Err(format!("HTTP {status}: {text}"));
    }

    serde_json::from_str(&text).map_err(|e| format!("parse JSON: {e}"))
}

/// Pretty-print a hook response (may contain `systemMessage` and/or `decision`).
fn format_hook_response(v: &serde_json::Value) {
    if let Some(obj) = v.as_object() {
        if obj.is_empty() {
            println!("  (empty response — no action taken)");
            return;
        }
        if let Some(msg) = obj.get("systemMessage").and_then(serde_json::Value::as_str) {
            println!("  systemMessage: {msg}");
        }
        if let Some(dec) = obj.get("decision").and_then(serde_json::Value::as_str) {
            println!("  decision:      {dec}");
        }
        if let Some(reason) = obj.get("reason").and_then(serde_json::Value::as_str) {
            println!("  reason:        {reason}");
        }
        // Print any other keys not already shown
        let shown = ["systemMessage", "decision", "reason"];
        for (k, val) in obj {
            if !shown.contains(&k.as_str()) && !val.is_null() {
                print_field(k, val);
            }
        }
    } else {
        println!("  {v}");
    }
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
