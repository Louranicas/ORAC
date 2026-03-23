//! # `orac-sidecar` — Intelligent Fleet Coordination Proxy
//!
//! Main daemon binary. Starts the HTTP hook server on port 8133
//! and runs the RALPH evolution loop as a background task.

use std::sync::Arc;

use orac_sidecar::m1_core::m01_core_types::PaneId;
use orac_sidecar::m1_core::m03_config::PvConfig;
use orac_sidecar::m2_wire::m07_ipc_client::IpcClient;
use orac_sidecar::m2_wire::m08_bus_types::BusFrame;

#[cfg(feature = "api")]
use orac_sidecar::m3_hooks::m10_hook_server::{build_router, spawn_field_poller, OracState};

#[cfg(feature = "evolution")]
use orac_sidecar::m8_evolution::m39_fitness_tensor::{FitnessDimension, TensorValues};

/// Maximum length of r/K history buffers for emergence detection.
#[cfg(feature = "evolution")]
const EMERGENCE_HISTORY_CAP: usize = 100;

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

        // Spawn IPC client connection to PV2 bus (BUG-041 fix)
        spawn_ipc_listener(Arc::clone(&state));

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

/// Spawn the IPC client as a background task (BUG-041 fix).
///
/// Connects to the PV2 bus via Unix socket and subscribes to `field.*` and
/// `sphere.*` events. Updates `OracState.ipc_state` on connect/disconnect.
/// Runs indefinitely with automatic reconnection on failure.
#[cfg(feature = "api")]
fn spawn_ipc_listener(state: Arc<OracState>) {
    tokio::spawn(async move {
        // Brief delay to let Axum bind first
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let mut client = IpcClient::new(PaneId::new("orac-sidecar"));

        loop {
            // Connect with exponential backoff
            match client.connect_with_backoff().await {
                Ok(attempts) => {
                    tracing::info!(attempts, "IPC client connected to PV2 bus");
                    *state.ipc_state.write() = "connected".into();
                }
                Err(e) => {
                    tracing::warn!("IPC connect failed: {e} — retrying in 30s");
                    *state.ipc_state.write() = format!("failed: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                    continue;
                }
            }

            // Subscribe to field and sphere events
            let patterns = vec!["field.*".into(), "sphere.*".into()];
            match client.subscribe(&patterns).await {
                Ok(count) => {
                    tracing::info!(count, "IPC subscribed to field.* + sphere.*");
                    *state.ipc_state.write() = "subscribed".into();
                }
                Err(e) => {
                    tracing::warn!("IPC subscribe failed: {e} — reconnecting");
                    *state.ipc_state.write() = format!("subscribe_failed: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            }

            // Event receive loop
            loop {
                match client.recv_frame().await {
                    Ok(BusFrame::Event { event }) => {
                        tracing::debug!(event_type = ?event, "IPC event received");
                        // Update field state cache from bus events when available
                    }
                    Ok(_frame) => {
                        // Non-event frames (ack, welcome, etc.) — ignore
                    }
                    Err(e) => {
                        tracing::warn!("IPC recv error: {e} — reconnecting");
                        *state.ipc_state.write() = "disconnected".into();
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        break; // Break inner loop to reconnect
                    }
                }
            }
        }
    });
}

/// Feed field state observations to the emergence detector (BUG-040 fix).
///
/// Samples r and K from the cached field state and runs 3 detectors:
/// 1. Coherence lock (r > 0.998 sustained)
/// 2. Coupling runaway (K rising without r improvement)
/// 3. Hebbian saturation (>80% weights at floor/ceiling)
#[cfg(feature = "evolution")]
fn feed_emergence_observations(
    state: &OracState,
    tick: u64,
    r_history: &mut std::collections::VecDeque<f64>,
    k_history: &mut std::collections::VecDeque<f64>,
) {
    // Sample current r from cached field state
    let r = state.field_state.read().field.order.r;
    r_history.push_back(r);
    if r_history.len() > EMERGENCE_HISTORY_CAP {
        r_history.pop_front();
    }

    // Sample effective K from coupling network (K * k_modulation)
    let (k_eff, weights) = {
        let network = state.coupling.read();
        let k = network.k * network.k_modulation;
        let w: Vec<f64> = network.connections.iter().map(|c| c.weight).collect();
        (k, w)
    };
    k_history.push_back(k_eff);
    if k_history.len() > EMERGENCE_HISTORY_CAP {
        k_history.pop_front();
    }

    let detector = state.ralph.emergence();

    // Make contiguous slices for detector APIs that expect &[f64]
    let (r_a, r_b) = r_history.as_slices();
    let (k_a, k_b) = k_history.as_slices();
    // VecDeque is contiguous after push_back + pop_front pattern
    let r_slice = if r_b.is_empty() { r_a } else { r_history.make_contiguous() };
    let k_slice = if k_b.is_empty() { k_a } else { k_history.make_contiguous() };

    // 1. Coherence lock detection
    if let Err(e) = detector.detect_coherence_lock(r_slice, tick) {
        tracing::debug!("Emergence coherence_lock check error: {e}");
    }

    // 2. Coupling runaway detection
    if let Err(e) = detector.detect_coupling_runaway(k_slice, r_slice, tick) {
        tracing::debug!("Emergence coupling_runaway check error: {e}");
    }

    // 3. Hebbian saturation detection (every 12 ticks to reduce overhead)
    if tick % 12 == 0 && !weights.is_empty() {
        if let Err(e) = detector.detect_hebbian_saturation(&weights, 0.01, 0.99, tick) {
            tracing::debug!("Emergence hebbian_saturation check error: {e}");
        }
    }
}

/// Build a 12D fitness tensor from current ORAC state.
///
/// Populates dimensions from live data where available,
/// uses calibrated placeholders for dimensions not yet wired.
#[cfg(feature = "evolution")]
fn build_tensor_from_state(state: &OracState) -> TensorValues {
    // collect_tensor reads from blackboard (D3 task_throughput, D4 error_rate,
    // D9 fleet_utilization), field_state (D1 field_coherence), and consents
    // (D11 consent_compliance). Remaining dimensions default to 0.5 (neutral).
    #[cfg(feature = "evolution")]
    let mut tensor = state.collect_tensor();
    #[cfg(not(feature = "evolution"))]
    let mut tensor = TensorValues::uniform(0.5);

    // D0: coordination_quality — session count / 9 fleet panes
    let session_count: f64 = f64::from(u32::try_from(state.session_count()).unwrap_or(0));
    tensor.set(FitnessDimension::CoordinationQuality, (session_count / 9.0).min(1.0));

    tensor
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

        let mut r_history: std::collections::VecDeque<f64> = std::collections::VecDeque::with_capacity(EMERGENCE_HISTORY_CAP);
        let mut k_history: std::collections::VecDeque<f64> = std::collections::VecDeque::with_capacity(EMERGENCE_HISTORY_CAP);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick = state.increment_tick();

                    // Advance circuit breaker FSMs (Open→HalfOpen after timeout)
                    #[cfg(feature = "intelligence")]
                    state.breaker_tick();

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

                    // BUG-043 fix: Record a trace span for each RALPH tick
                    #[cfg(feature = "monitoring")]
                    {
                        use orac_sidecar::m7_monitoring::m32_otel_traces::SpanBuilder;
                        if let Ok(span) = SpanBuilder::start("orac.tick.ralph") {
                            state.trace_store.record(span.finish_ok());
                        }
                    }

                    // BUG-040 fix: Feed field state to emergence detector every tick
                    feed_emergence_observations(&state, tick, &mut r_history, &mut k_history);
                }
                _ = shutdown.changed() => {
                    tracing::info!("RALPH evolution loop stopping");
                    break;
                }
            }
        }
    });
}
