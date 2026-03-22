//! # `orac-sidecar` — Intelligent Fleet Coordination Proxy
//!
//! Main daemon binary. Starts the HTTP hook server on port 8133,
//! connects to the PV2 IPC bus, and runs the sidecar tick loop.

use std::sync::Arc;

use orac_sidecar::m1_core::m03_config::PvConfig;

#[cfg(feature = "api")]
use orac_sidecar::m3_hooks::m10_hook_server::{start_server, OracState};

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load configuration
    let config = PvConfig::load().unwrap_or_else(|e| {
        tracing::warn!("Failed to load config, using defaults: {e}");
        PvConfig::default()
    });

    tracing::info!(
        port = config.server.port,
        "ORAC sidecar starting"
    );

    // Phase 1: Start HTTP hook server
    #[cfg(feature = "api")]
    {
        let state = Arc::new(OracState::new(config));

        if let Err(e) = start_server(state).await {
            tracing::error!("Hook server failed: {e}");
        }
    }

    // Fallback: if api feature not enabled, just wait for signal
    #[cfg(not(feature = "api"))]
    {
        let _config = config;
        tracing::info!("ORAC running without API feature — hook server disabled");
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::error!("Failed to listen for ctrl-c: {e}");
        }
    }

    tracing::info!("ORAC sidecar shutting down");
}
