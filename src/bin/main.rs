//! # `orac-sidecar` — Intelligent Fleet Coordination Proxy
//!
//! Main daemon binary. Starts the HTTP hook server on port 8133
//! and runs the RALPH evolution loop as a background task.

use std::sync::Arc;

use orac_sidecar::m1_core::m03_config::PvConfig;

#[cfg(feature = "api")]
use orac_sidecar::m3_hooks::m10_hook_server::{build_router, spawn_field_poller, OracState};

#[cfg(feature = "evolution")]
use orac_sidecar::m8_evolution::m39_fitness_tensor::TensorValues;

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

    #[cfg(feature = "api")]
    {
        let state = Arc::new(OracState::new(config));

        // Spawn field state poller (non-blocking, updates SharedState from PV2)
        spawn_field_poller(Arc::clone(&state));

        // Shutdown signal: shared between Axum and RALPH
        #[cfg(feature = "evolution")]
        let (halt_send, halt_recv) = tokio::sync::watch::channel(false);
        #[cfg(not(feature = "evolution"))]
        let (halt_send, _) = tokio::sync::watch::channel(false);

        // Spawn RALPH evolution loop (if feature enabled)
        #[cfg(feature = "evolution")]
        spawn_ralph_loop(Arc::clone(&state), halt_recv);

        // Build and start Axum server
        let addr = std::net::SocketAddr::new(
            state
                .config
                .server
                .bind_addr
                .parse()
                .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            state.config.server.port,
        );

        let router = build_router(Arc::clone(&state));
        tracing::info!(%addr, "ORAC hook server starting");

        let listener = match tokio::net::TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("Failed to bind ORAC port {addr}: {e}");
                return;
            }
        };

        if let Err(e) = axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = tokio::signal::ctrl_c().await;
                tracing::info!("ORAC hook server shutting down");
                // Signal RALPH loop to stop
                let _ = halt_send.send(true);
            })
            .await
        {
            tracing::error!("Axum server error: {e}");
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

/// Build a 12D fitness tensor from current ORAC state.
///
/// Populates dimensions from live data where available,
/// uses calibrated placeholders for dimensions not yet wired.
#[cfg(feature = "evolution")]
fn build_tensor_from_state(state: &OracState) -> TensorValues {
    let field = state.field_state.read();
    // Session count is always small (max ~12 panes), safe to convert
    let session_count: f64 = f64::from(u32::try_from(state.session_count()).unwrap_or(0));
    let coordination = (session_count / 9.0).min(1.0);
    let r = field.field.order.r;

    let mut vals = [0.0_f64; 12];
    vals[0] = coordination;  // coordination_quality
    vals[1] = r;             // field_coherence
    vals[2] = 1.0;           // dispatch_accuracy (placeholder)
    vals[3] = 0.75;          // bridge_health (placeholder)
    vals[4] = 1.0;           // consent_compliance (placeholder)
    vals[5] = 0.5;           // learning_rate (placeholder)
    vals[6] = 0.8;           // emergence_stability (placeholder)
    vals[7] = 0.7;           // resource_efficiency (placeholder)
    vals[8] = 0.9;           // communication_fidelity (placeholder)
    vals[9] = 0.5;           // adaptation_speed (placeholder)
    vals[10] = 0.6;          // diversity_index (placeholder)

    // Dim 11: overall_fitness = mean of dims 0-10
    let sum: f64 = vals[..11].iter().sum();
    vals[11] = sum / 11.0;

    TensorValues { values: vals }
}

/// Spawn the RALPH evolution tick loop as a background tokio task.
///
/// Runs every 5 seconds, computing a fitness tensor from live state
/// and feeding it to the RALPH 5-phase engine. Stops on shutdown signal.
#[cfg(feature = "evolution")]
fn spawn_ralph_loop(
    state: Arc<OracState>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        // Skip immediate first tick to let Axum bind first
        interval.tick().await;

        tracing::info!("RALPH evolution loop started (5s interval)");

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick = state.increment_tick();
                    let tensor = build_tensor_from_state(&state);

                    match state.ralph.tick(&tensor, tick) {
                        Ok(phase) => {
                            let ralph_state = state.ralph.state();
                            if tick % 12 == 0 {
                                tracing::info!(
                                    %phase,
                                    gen = ralph_state.generation,
                                    fitness = format!("{:.4}", ralph_state.current_fitness),
                                    "RALPH tick"
                                );
                            }
                        }
                        Err(e) => {
                            tracing::warn!("RALPH tick error: {e}");
                        }
                    }
                }
                _ = shutdown.changed() => {
                    tracing::info!("RALPH evolution loop stopping");
                    break;
                }
            }
        }
    });
}
