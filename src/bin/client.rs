//! # `orac-client` — CLI client for ORAC sidecar
//!
//! Sends commands to the running ORAC daemon via HTTP.
//! Usage: `orac-client status | field | blackboard | metrics`

use std::fmt::Write as _;
use std::process::ExitCode;
use std::time::Duration;

/// ORAC HTTP base address.
const ORAC_ADDR: &str = "127.0.0.1:8133";

/// Default request timeout.
const TIMEOUT: Duration = Duration::from_secs(3);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let json_mode = args.iter().any(|a| a == "--json");
    let cmd = args.iter().skip(1).find(|a| !a.starts_with('-')).map_or("help", String::as_str);

    match cmd {
        "status" => cmd_status_maybe_json(json_mode),
        "field" => cmd_field_maybe_json(json_mode),
        "blackboard" => cmd_blackboard_maybe_json(json_mode),
        "metrics" => cmd_metrics(),
        "hook-test" => cmd_hook_test(args.get(2).map(String::as_str)),
        "probe" => cmd_probe(),
        "watch" => cmd_watch(),
        "dispatch" => cmd_dispatch(&args[2..]),
        "fleet" => cmd_fleet(json_mode),
        "completions" => cmd_completions(args.get(2).map(String::as_str)),
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
    println!("  watch           Live dashboard — polls field state every 2s");
    println!("  dispatch <desc> Submit a task to the fleet via PV2 bus");
    println!("  fleet           List registered spheres with status");
    println!("  completions <s> Emit shell completions (bash, zsh, fish)");
    println!();
    println!("Flags:");
    println!("  --json          Machine-readable JSON (status, field, blackboard, fleet)");
    println!();
    println!("Dispatch options:");
    println!("  --target <t>    any_idle (default), field_driven, willing, specific");
    println!("  --submitter <s> Submitter ID (default: orac-client)");
    println!();
    println!("Hook events: SessionStart, Stop, PostToolUse, PreToolUse,");
    println!("             UserPromptSubmit, PermissionRequest");
}

/// GET /health — raw JSON or pretty-print.
fn cmd_status_maybe_json(json_mode: bool) -> ExitCode {
    if json_mode { return cmd_json("/health"); }
    cmd_status()
}

/// GET /field — raw JSON or pretty-print.
fn cmd_field_maybe_json(json_mode: bool) -> ExitCode {
    if json_mode { return cmd_json("/field"); }
    cmd_field()
}

/// GET /blackboard — raw JSON or pretty-print.
fn cmd_blackboard_maybe_json(json_mode: bool) -> ExitCode {
    if json_mode { return cmd_json("/blackboard"); }
    cmd_blackboard()
}

/// List all registered fleet spheres from PV2.
fn cmd_fleet(json_mode: bool) -> ExitCode {
    let url = "http://127.0.0.1:8132/spheres";
    let body = match fetch(url) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("PV2 unreachable: {e}");
            return ExitCode::FAILURE;
        }
    };

    if json_mode {
        println!("{body}");
        return ExitCode::SUCCESS;
    }

    let Ok(v) = serde_json::from_str::<serde_json::Value>(&body) else {
        println!("{body}");
        return ExitCode::SUCCESS;
    };

    let spheres = v.get("spheres").and_then(serde_json::Value::as_array);
    let Some(spheres) = spheres else {
        println!("No spheres data");
        return ExitCode::SUCCESS;
    };

    println!("Fleet Spheres ({} registered)", spheres.len());
    println!("{:-<60}", "");
    println!("{:<30} {:<12} {:<8}", "ID", "STATUS", "FREQ");
    println!("{:-<60}", "");

    for s in spheres {
        let id = s.get("id").and_then(serde_json::Value::as_str).unwrap_or("?");
        let status = s.get("status").and_then(serde_json::Value::as_str).unwrap_or("?");
        let freq = s.get("frequency").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        println!("{id:<30} {status:<12} {freq:.2}");
    }

    ExitCode::SUCCESS
}

/// Generic JSON output for --json flag.
fn cmd_json(path: &str) -> ExitCode {
    let url = format!("http://{ORAC_ADDR}{path}");
    match fetch(&url) {
        Ok(body) => { println!("{body}"); ExitCode::SUCCESS }
        Err(e) => { eprintln!("{e}"); ExitCode::FAILURE }
    }
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

/// Live dashboard — polls /health and /field every 2 seconds.
fn cmd_watch() -> ExitCode {
    println!("ORAC Watch — live dashboard (Ctrl+C to stop)\n");

    loop {
        // Fetch health + field in sequence
        let health = fetch(&format!("http://{ORAC_ADDR}/health")).ok();
        let field = fetch(&format!("http://{ORAC_ADDR}/field")).ok();

        // Clear screen
        print!("\x1b[2J\x1b[H");

        println!("╔══════════════════════════════════════════╗");
        println!("║  ORAC Watch — {}              ║", chrono::Local::now().format("%H:%M:%S"));
        println!("╠══════════════════════════════════════════╣");

        if let Some(h) = &health {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(h) {
                let status = v["status"].as_str().unwrap_or("?");
                let sessions = v["sessions"].as_u64().unwrap_or(0);
                let ticks = v["uptime_ticks"].as_u64().unwrap_or(0);
                println!("║  status:   {status:<28} ║");
                println!("║  sessions: {sessions:<28} ║");
                println!("║  ticks:    {ticks:<28} ║");
            }
        } else {
            println!("║  ORAC: UNREACHABLE                       ║");
        }

        println!("╠══════════════════════════════════════════╣");

        if let Some(f) = &field {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(f) {
                let r = v["r"].as_f64().unwrap_or(0.0);
                let k = v["k"].as_f64().unwrap_or(0.0);
                let sph = v["sphere_count"].as_u64().unwrap_or(0);
                let tick = v["pv2_tick"].as_u64().unwrap_or(0);
                println!("║  r:        {r:<28.4} ║");
                println!("║  K:        {k:<28.4} ║");
                println!("║  spheres:  {sph:<28} ║");
                println!("║  pv2_tick: {tick:<28} ║");
            }
        } else {
            println!("║  PV2: UNREACHABLE                        ║");
        }

        println!("╚══════════════════════════════════════════╝");
        println!("\n  Refreshing every 2s — Ctrl+C to stop");

        std::thread::sleep(Duration::from_secs(2));
    }
}

/// Emit shell completion script for the given shell.
///
/// Usage: `eval "$(orac-client completions bash)"` in `.bashrc`,
/// or `orac-client completions zsh > _orac-client` in fpath.
fn cmd_completions(shell: Option<&str>) -> ExitCode {
    let Some(shell) = shell else {
        eprintln!("Usage: orac-client completions <bash|zsh|fish>");
        return ExitCode::FAILURE;
    };

    match shell {
        "bash" => print!("{}", bash_completions()),
        "zsh" => print!("{}", zsh_completions()),
        "fish" => print!("{}", fish_completions()),
        other => {
            eprintln!("Unknown shell '{other}'. Supported: bash, zsh, fish");
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}

/// Subcommands for completion scripts.
const SUBCOMMANDS: &str = "status field blackboard metrics hook-test probe watch dispatch fleet completions help";

/// Hook event names for `hook-test` completion.
const HOOK_EVENTS: &str = "SessionStart Stop PostToolUse PreToolUse UserPromptSubmit PermissionRequest";

/// Dispatch targets for `--target` completion.
const DISPATCH_TARGETS: &str = "any_idle field_driven willing specific";

fn bash_completions() -> String {
    format!(
        r#"_orac_client() {{
    local cur prev cmds
    COMPREPLY=()
    cur="${{COMP_WORDS[COMP_CWORD]}}"
    prev="${{COMP_WORDS[COMP_CWORD-1]}}"
    cmds="{SUBCOMMANDS}"

    case "$prev" in
        orac-client)
            COMPREPLY=( $(compgen -W "$cmds --json" -- "$cur") )
            return 0
            ;;
        hook-test)
            COMPREPLY=( $(compgen -W "{HOOK_EVENTS}" -- "$cur") )
            return 0
            ;;
        completions)
            COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
            return 0
            ;;
        --target)
            COMPREPLY=( $(compgen -W "{DISPATCH_TARGETS}" -- "$cur") )
            return 0
            ;;
    esac

    if [[ "$cur" == -* ]]; then
        COMPREPLY=( $(compgen -W "--json --target --submitter" -- "$cur") )
    fi
}}
complete -F _orac_client orac-client
"#
    )
}

fn zsh_completions() -> String {
    format!(
        r#"#compdef orac-client

_orac_client() {{
    local -a commands hook_events targets shells
    commands=({SUBCOMMANDS})
    hook_events=({HOOK_EVENTS})
    targets=({DISPATCH_TARGETS})
    shells=(bash zsh fish)

    _arguments -C \
        '1:command:->cmd' \
        '*::arg:->args' \
        '--json[Machine-readable JSON output]'

    case "$state" in
        cmd)
            _describe 'command' commands
            ;;
        args)
            case "$words[1]" in
                hook-test)
                    _describe 'event' hook_events
                    ;;
                completions)
                    _describe 'shell' shells
                    ;;
                dispatch)
                    _arguments \
                        '--target[Dispatch target]:target:('"${{targets[*]}}"')' \
                        '--submitter[Submitter ID]:submitter:'
                    ;;
            esac
            ;;
    esac
}}

_orac_client "$@"
"#
    )
}

fn fish_completions() -> String {
    let mut out = String::from(
        "# Fish completions for orac-client\n\
         complete -c orac-client -e\n\n",
    );

    // Subcommands
    let descs = [
        ("status", "Show sidecar health and session info"),
        ("field", "Show Kuramoto field state"),
        ("blackboard", "Show fleet blackboard state"),
        ("metrics", "Dump Prometheus-format metrics"),
        ("hook-test", "Send test payload to a hook endpoint"),
        ("probe", "Run connectivity checks"),
        ("watch", "Live dashboard"),
        ("dispatch", "Submit a task to the fleet"),
        ("fleet", "List registered spheres"),
        ("completions", "Emit shell completions"),
        ("help", "Show help"),
    ];

    for (cmd, desc) in &descs {
        let _ = writeln!(
            out,
            "complete -c orac-client -n '__fish_use_subcommand' -a '{cmd}' -d '{desc}'"
        );
    }

    // Global flag
    out.push_str(
        "complete -c orac-client -l json -d 'Machine-readable JSON output'\n\n",
    );

    // hook-test events
    for event in HOOK_EVENTS.split_whitespace() {
        let _ = writeln!(
            out,
            "complete -c orac-client -n '__fish_seen_subcommand_from hook-test' -a '{event}'"
        );
    }

    // completions shells
    for shell in &["bash", "zsh", "fish"] {
        let _ = writeln!(
            out,
            "complete -c orac-client -n '__fish_seen_subcommand_from completions' -a '{shell}'"
        );
    }

    // dispatch flags
    out.push_str(
        "\ncomplete -c orac-client -n '__fish_seen_subcommand_from dispatch' -l target -ra 'any_idle field_driven willing specific'\n\
         complete -c orac-client -n '__fish_seen_subcommand_from dispatch' -l submitter -r\n"
    );

    out
}

/// PV2 HTTP base address.
const PV2_ADDR: &str = "127.0.0.1:8132";

/// Submit a task to the PV2 bus via `POST /bus/submit`.
///
/// Usage: `orac-client dispatch "Review src/lib.rs" [--target any_idle] [--submitter me]`
fn cmd_dispatch(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("Usage: orac-client dispatch <description> [--target <t>] [--submitter <s>]");
        eprintln!("Targets: any_idle (default), field_driven, willing, specific");
        return ExitCode::FAILURE;
    }

    let mut description = String::new();
    let mut target = "any_idle";
    let mut submitter = "orac-client";
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--target" => {
                i += 1;
                if i < args.len() {
                    target = leak_str(&args[i]);
                } else {
                    eprintln!("--target requires a value");
                    return ExitCode::FAILURE;
                }
            }
            "--submitter" => {
                i += 1;
                if i < args.len() {
                    submitter = leak_str(&args[i]);
                } else {
                    eprintln!("--submitter requires a value");
                    return ExitCode::FAILURE;
                }
            }
            other => {
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(other);
            }
        }
        i += 1;
    }

    if description.is_empty() {
        eprintln!("Error: task description is required");
        return ExitCode::FAILURE;
    }

    let valid_targets = ["any_idle", "field_driven", "willing", "specific"];
    if !valid_targets.contains(&target) {
        eprintln!("Invalid target '{target}'. Valid: {}", valid_targets.join(", "));
        return ExitCode::FAILURE;
    }

    let payload = serde_json::json!({
        "description": description,
        "target": target,
        "submitter": submitter,
    });

    let url = format!("http://{PV2_ADDR}/bus/submit");
    let body = match post_json(&url, &payload) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Task submission failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    println!("Task Submitted");
    println!("--------------");
    if let Some(id) = body["task_id"].as_str() {
        println!("  task_id:     {id}");
    }
    if let Some(s) = body["status"].as_str() {
        println!("  status:      {s}");
    }
    println!("  description: {description}");
    println!("  target:      {target}");
    println!("  submitter:   {submitter}");

    ExitCode::SUCCESS
}

/// Leak a `String` to get a `&'static str` for option parsing.
///
/// Safe for CLI tools — the program exits shortly after.
fn leak_str(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
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

    if !(200..300).contains(&status) {
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
