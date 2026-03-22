//! # `orac-client` — CLI client for ORAC sidecar
//!
//! Sends commands to the running ORAC daemon via HTTP.
//! Usage: `orac-client status | field | spheres | health`

fn main() {
    // TODO: Phase 1 — clap argument parsing
    // TODO: Phase 1 — HTTP client to localhost:8133
    // TODO: Commands: status, field, spheres, health, hooks, bridges

    let args: Vec<String> = std::env::args().collect();
    let cmd = args.get(1).map_or("help", String::as_str);

    match cmd {
        "help" | "--help" | "-h" => {
            println!("orac-client — CLI for ORAC sidecar");
            println!("Usage: orac-client <command>");
            println!();
            println!("Commands:");
            println!("  status    Show sidecar status");
            println!("  field     Show field state (r, psi, K)");
            println!("  spheres   List registered spheres");
            println!("  health    Health check");
        }
        other => {
            println!("orac-client: command '{other}' not yet implemented (scaffold)");
        }
    }
}
