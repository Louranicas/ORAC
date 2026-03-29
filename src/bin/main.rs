//! # `orac-sidecar` — Intelligent Fleet Coordination Proxy
//!
//! Main daemon binary. Starts the HTTP hook server on port 8133
//! and runs the RALPH evolution loop as a background task.

use std::sync::Arc;

use orac_sidecar::m1_core::m01_core_types::{PaneId, PaneSphere};
use orac_sidecar::m1_core::m03_config::PvConfig;
use orac_sidecar::m2_wire::m07_ipc_client::IpcClient;
use orac_sidecar::m2_wire::m08_bus_types::{BusEvent, BusFrame};

#[cfg(all(feature = "intelligence", feature = "evolution"))]
use orac_sidecar::m4_intelligence::m18_hebbian_stdp::apply_stdp;

#[cfg(feature = "api")]
use orac_sidecar::m3_hooks::m10_hook_server::{build_router, spawn_field_poller, OracState};

#[cfg(feature = "evolution")]
use orac_sidecar::m8_evolution::m37_emergence_detector::EmergenceType;
use orac_sidecar::m8_evolution::m39_fitness_tensor::{FitnessDimension, TensorValues};

/// Maximum length of r/K history buffers for emergence detection.
#[cfg(feature = "evolution")]
const EMERGENCE_HISTORY_CAP: usize = 100;

#[tokio::main]
#[allow(clippy::too_many_lines)] // Main orchestrates 8 subsystems: server, RALPH, STDP, bridges, IPC, poller, emergence, persistence
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

        // Hydrate persisted state (RALPH, sessions, coupling weights)
        hydrate_startup_state(&state);

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

/// Hydrate RALPH state, sessions, and coupling weights from persistent storage.
///
/// Called once at startup to restore state across ORAC restarts.
/// Logs results for each hydration step — failures are non-fatal.
#[cfg(feature = "api")]
fn hydrate_startup_state(state: &OracState) {
    // 1. RALPH evolution state from blackboard
    #[cfg(all(feature = "persistence", feature = "evolution"))]
    if let Some(bb) = state.blackboard() {
        match bb.load_ralph_state() {
            Ok(Some(saved)) => {
                state.ralph.hydrate(
                    saved.generation,
                    saved.completed_cycles,
                    saved.peak_fitness,
                );
                tracing::info!(
                    gen = saved.generation,
                    fitness = format!("{:.4}", saved.current_fitness),
                    phase = saved.last_phase,
                    "RALPH state loaded from blackboard"
                );
            }
            Ok(None) => tracing::info!("No saved RALPH state — starting fresh"),
            Err(e) => tracing::warn!("Failed to load RALPH state: {e}"),
        }
    }

    // 2. Active sessions from blackboard
    #[cfg(feature = "persistence")]
    if let Some(bb) = state.blackboard() {
        match bb.load_sessions() {
            Ok(saved) if !saved.is_empty() => {
                let mut sessions = state.sessions.write();
                for s in &saved {
                    sessions.insert(
                        s.session_id.clone(),
                        orac_sidecar::m3_hooks::m10_hook_server::SessionTracker {
                            pane_id: s.pane_id.clone().into(),
                            active_task_id: s.active_task_id.clone(),
                            active_task_claimed_ms: None,
                            poll_counter: s.poll_counter,
                            total_tool_calls: s.total_tool_calls,
                            started_ms: s.started_ms,
                            persona: s.persona.clone(),
                        },
                    );
                }
                tracing::info!(sessions = saved.len(), "Sessions hydrated from blackboard");
            }
            Ok(_) => tracing::info!("No saved sessions — starting fresh"),
            Err(e) => tracing::warn!("Failed to load sessions: {e}"),
        }
    }

    // 3. Coupling weights from blackboard (preferred — exact sphere IDs, no namespace mismatch)
    #[cfg(all(feature = "persistence", feature = "intelligence"))]
    if let Some(bb) = state.blackboard() {
        match bb.load_coupling_weights() {
            Ok(saved) if !saved.is_empty() => {
                let mut network = state.coupling.write();
                let mut restored = 0u32;
                for cw in &saved {
                    let from = PaneId::new(&cw.from_id);
                    let to = PaneId::new(&cw.to_id);
                    if network.get_weight(&from, &to).is_some() {
                        network.set_weight(&from, &to, cw.weight.clamp(0.0, 1.0));
                        restored += 1;
                    }
                }
                tracing::info!(
                    saved = saved.len(),
                    restored,
                    "Coupling weights hydrated from blackboard"
                );
            }
            Ok(_) => tracing::info!("No saved coupling weights — starting with defaults"),
            Err(e) => tracing::warn!("Failed to load coupling weights: {e}"),
        }
    }

    // 4. Coupling weights from POVM pathways (fallback — may have ID namespace mismatch)
    #[cfg(all(feature = "persistence", feature = "bridges"))]
    {
        use orac_sidecar::m5_bridges::m24_povm_bridge::PovmBridge;
        let povm = PovmBridge::new();
        match povm.hydrate_pathways() {
            Ok(pathways) if !pathways.is_empty() => {
                let mut network = state.coupling.write();
                let mut restored = 0u32;
                for pw in &pathways {
                    for conn in &mut network.connections {
                        if conn.from.as_str() == pw.source
                            && conn.to.as_str() == pw.target
                            && pw.weight > 0.0
                        {
                            conn.weight = pw.weight.clamp(0.0, 1.0);
                            restored += 1;
                        }
                    }
                }
                tracing::info!(pathways = pathways.len(), restored, "POVM coupling weights hydrated");
            }
            Ok(_) => tracing::info!("No POVM pathways to hydrate — starting with default weights"),
            Err(e) => tracing::warn!("Failed to hydrate POVM pathways: {e}"),
        }
    }
}

/// Spawn the IPC client as a background task (BUG-041 fix).
///
/// Connects to the PV2 bus via Unix socket and subscribes to `field.*` and
/// `sphere.*` events. Updates `OracState.ipc_state` on connect/disconnect.
/// Runs indefinitely with automatic reconnection on failure.
///
/// BUG-C002 fix: Outer reconnection loop now uses escalating backoff
/// (5s → 10s → 20s → … → 120s cap) to prevent CPU runaway when the
/// PV2 bus is permanently unavailable.
#[cfg(feature = "api")]
fn spawn_ipc_listener(state: Arc<OracState>) {
    /// Initial outer-loop reconnect delay (seconds).
    const RECONNECT_BASE_SECS: u64 = 5;
    /// Maximum outer-loop reconnect delay (seconds).
    /// Kept low (30s) because PV2 bus socket may appear/disappear
    /// during devenv restarts — fast recovery is more important
    /// than preventing CPU load on a local service.
    const RECONNECT_CAP_SECS: u64 = 30;

    tokio::spawn(async move {
        // Brief delay to let Axum bind first
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let mut client = IpcClient::new(PaneId::new("orac-sidecar"));
        let mut reconnect_delay_secs = RECONNECT_BASE_SECS;
        let mut total_reconnects: u64 = 0;
        let mut successful_connects: u64 = 0;

        loop {
            // Connect with exponential backoff (internal: 10 attempts, 100ms→5s)
            match client.connect_with_backoff().await {
                Ok(attempts) => {
                    tracing::info!(attempts, total_reconnects, "IPC client connected to PV2 bus");
                    *state.ipc_state.write() = "connected".into();
                }
                Err(e) => {
                    total_reconnects += 1;
                    if total_reconnects % 10 == 0 {
                        tracing::error!(
                            total_reconnects,
                            delay_secs = reconnect_delay_secs,
                            "IPC connect persistently failing: {e}"
                        );
                    } else {
                        tracing::warn!(
                            delay_secs = reconnect_delay_secs,
                            "IPC connect failed: {e} — retrying"
                        );
                    }
                    *state.ipc_state.write() = format!("failed({total_reconnects}): {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay_secs)).await;
                    reconnect_delay_secs = (reconnect_delay_secs * 2).min(RECONNECT_CAP_SECS);
                    continue;
                }
            }

            // Subscribe to configured event patterns
            let patterns = state.config.ipc.subscribe_patterns.clone();
            match client.subscribe(&patterns).await {
                Ok(count) => {
                    successful_connects += 1;
                    let session = client.session_id().unwrap_or("unknown");
                    tracing::info!(
                        count,
                        session,
                        total_reconnects,
                        successful_connects,
                        "IPC subscribed to field.* + sphere.* — ready"
                    );
                    *state.ipc_state.write() = "subscribed".into();
                    // Reset backoff delay on success (preserve lifetime counters)
                    reconnect_delay_secs = RECONNECT_BASE_SECS;
                }
                Err(e) => {
                    total_reconnects += 1;
                    tracing::warn!(
                        delay_secs = reconnect_delay_secs,
                        "IPC subscribe failed: {e} — reconnecting"
                    );
                    *state.ipc_state.write() = format!("subscribe_failed({total_reconnects}): {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay_secs)).await;
                    reconnect_delay_secs = (reconnect_delay_secs * 2).min(RECONNECT_CAP_SECS);
                    continue;
                }
            }

            // Event receive loop
            let mut ipc_events_processed: u64 = 0;
            loop {
                match client.recv_frame().await {
                    Ok(BusFrame::Event { event }) => {
                        ipc_events_processed += 1;
                        if ipc_events_processed % 100 == 0 {
                            tracing::debug!(total = ipc_events_processed, "IPC events processed");
                        }
                        process_bus_event(&state, &event);
                    }
                    Ok(_frame) => {
                        // Non-event frames (ack, welcome, etc.) — ignore
                    }
                    Err(e) => {
                        total_reconnects += 1;
                        // BUG-064f fix: 300s idle timeout is expected behavior (keepalive
                        // cycle). Use debug level to avoid noisy WARN every 5 minutes.
                        // Only escalate to warn after 5+ consecutive reconnects.
                        if total_reconnects % 5 == 0 {
                            tracing::warn!(
                                total_reconnects,
                                delay_secs = reconnect_delay_secs,
                                "IPC recv error (persistent): {e} — reconnecting"
                            );
                        } else {
                            tracing::debug!(
                                total_reconnects,
                                delay_secs = reconnect_delay_secs,
                                "IPC recv timeout — reconnecting"
                            );
                        }
                        // BUG-057l fix: explicitly disconnect to release socket state
                        // before attempting reconnection (prevents fd/buffer accumulation)
                        let _ = client.disconnect().await;
                        *state.ipc_state.write() = "disconnected".into();
                        tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay_secs)).await;
                        reconnect_delay_secs = (reconnect_delay_secs * 2).min(RECONNECT_CAP_SECS);
                        break; // Break inner loop to reconnect
                    }
                }
            }
        }
    });
}

/// Process a single bus event from the IPC subscription.
///
/// Handles `field.*` events to update the cached field state and
/// `sphere.*` events to track sphere registration/deregistration.
/// Unknown event types are silently ignored.
#[cfg(feature = "api")]
fn process_bus_event(state: &OracState, event: &BusEvent) {
    match event.event_type.as_str() {
        "field.tick" | "field.state" => {
            // Update cached field parameters from bus event data
            if let Some(r) = event.data.get("r").and_then(serde_json::Value::as_f64) {
                let mut guard = state.field_state.write();
                guard.field.order.r = r.clamp(0.0, 1.0);
                guard.field.tick = event.tick;
                guard.record_poll_success();
                if let Some(psi) = event.data.get("psi").and_then(serde_json::Value::as_f64) {
                    guard.field.order.psi = psi;
                }
            }
        }
        "sphere.registered" => {
            if let Some(pane_id) = event.data.get("pane_id").and_then(serde_json::Value::as_str) {
                let id = PaneId::new(pane_id);
                let persona = event
                    .data
                    .get("persona")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("unknown");
                let sphere = PaneSphere::new(id.clone(), persona);
                state.field_state.write().spheres.insert(id, sphere);
                tracing::debug!(pane_id, "IPC: sphere registered in cache");
            }
        }
        "sphere.deregistered" => {
            if let Some(pane_id) = event.data.get("pane_id").and_then(serde_json::Value::as_str) {
                let id = PaneId::new(pane_id);
                state.field_state.write().spheres.remove(&id);
                tracing::debug!(pane_id, "IPC: sphere deregistered from cache");
            }
        }
        "sphere.status" => {
            if let Some(pane_id) = event.data.get("pane_id").and_then(serde_json::Value::as_str) {
                let id = PaneId::new(pane_id);
                if let Some(phase) = event.data.get("phase").and_then(serde_json::Value::as_f64) {
                    if let Some(sphere) = state.field_state.write().spheres.get_mut(&id) {
                        sphere.phase = phase.rem_euclid(std::f64::consts::TAU);
                    }
                }
            }
        }
        _ => {
            // Unknown event types — no-op
        }
    }
}

/// Feed field state observations to the emergence detector (BUG-040 fix).
///
/// Tracking state for monitor-based emergence detectors (`DispatchLoop`, `ConsentCascade`).
#[cfg(feature = "evolution")]
struct MonitorTracking {
    /// Active `DispatchLoop` monitor ID, if any.
    dispatch_monitor: Option<u64>,
    /// Active `ConsentCascade` monitor ID, if any.
    consent_monitor: Option<u64>,
    /// Previous `dispatch_total` snapshot (for delta detection).
    prev_dispatch_total: u64,
    /// Previous count of spheres with `opt_out_hebbian` set.
    prev_opt_out_count: usize,
}

#[cfg(feature = "evolution")]
impl MonitorTracking {
    fn new() -> Self {
        Self {
            dispatch_monitor: None,
            consent_monitor: None,
            prev_dispatch_total: 0,
            prev_opt_out_count: 0,
        }
    }
}

/// Samples r and K from the cached field state and runs emergence detectors:
/// 1. Coherence lock (sustained high r)
/// 2. Coupling runaway (K rising without r improvement)
/// 3. Hebbian saturation (>80% weights at floor/ceiling)
/// 4. Beneficial sync / Field stability
/// 5. Thermal spike
/// 6. Chimera formation
/// 7. `DispatchLoop` (monitor-based: repeated dispatch to same domain)
/// 8. `ConsentCascade` (monitor-based: multiple opt-outs in short window)
#[cfg(feature = "evolution")]
#[allow(clippy::too_many_lines)] // 8 emergence detectors require sequential checks
fn feed_emergence_observations(
    state: &OracState,
    tick: u64,
    r_history: &mut std::collections::VecDeque<f64>,
    k_history: &mut std::collections::VecDeque<f64>,
    monitors: &mut MonitorTracking,
) {
    // Sample current r from cached field state.
    // Guard: skip r=0.0 (field poller hasn't populated yet) to avoid
    // poisoning the emergence stability window with boot-time zeroes.
    let r = state.field_state.read().field.order.r;
    if r <= 0.0 {
        return;
    }
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

    // Guard: need at least 2 samples for meaningful detection
    if r_history.len() < 2 || k_history.len() < 2 {
        return;
    }

    let detector = state.ralph.emergence();

    // Make contiguous slices for detector APIs that expect &[f64]
    let (r_a, r_b) = r_history.as_slices();
    let (k_a, k_b) = k_history.as_slices();
    // VecDeque is contiguous after push_back + pop_front pattern
    let r_slice = if r_b.is_empty() { r_a } else { r_history.make_contiguous() };
    let k_slice = if k_b.is_empty() { k_a } else { k_history.make_contiguous() };

    // 1. Coherence lock detection
    // Gen-059g: Added INFO logging for all emergence types (was debug-only)
    match detector.detect_coherence_lock(r_slice, tick) {
        Ok(Some(id)) => tracing::info!(id, r = format!("{r:.4}"), "Emergence: CoherenceLock detected"),
        Err(e) => tracing::debug!("Emergence coherence_lock check error: {e}"),
        _ => {}
    }

    // 2. Coupling runaway detection
    match detector.detect_coupling_runaway(k_slice, r_slice, tick) {
        Ok(Some(id)) => tracing::info!(id, "Emergence: CouplingRunaway detected"),
        Err(e) => tracing::debug!("Emergence coupling_runaway check error: {e}"),
        _ => {}
    }

    // 3. Hebbian saturation detection (every 12 ticks to reduce overhead)
    if tick % 12 == 0 && !weights.is_empty() {
        // Gen-063a: Use STDP soft ceiling (0.85) not theoretical max (1.0)
        // so saturation detector fires when weights approach the actual learning bound.
        match detector.detect_hebbian_saturation(&weights, 0.15, 0.85, tick) {
            Ok(Some(id)) => tracing::info!(id, "Emergence: HebbianSaturation detected"),
            Err(e) => tracing::debug!("Emergence hebbian_saturation check error: {e}"),
            _ => {}
        }
    }

    // 4. Beneficial sync detection (BUG-GEN11: was missing from feed loop)
    if r_slice.len() >= 2 {
        let prev_r = r_slice[r_slice.len() - 2];
        match detector.detect_beneficial_sync(r, prev_r, tick) {
            Ok(Some(id)) => {
                tracing::info!(
                    id,
                    r = format!("{r:.4}"),
                    prev_r = format!("{prev_r:.4}"),
                    "Emergence: BeneficialSync detected"
                );
            }
            Err(e) => tracing::debug!("Emergence beneficial_sync check error: {e}"),
            _ => {}
        }
    }

    // 4b. Field stability detection (sustained r above threshold)
    if tick % 5 == 0 && r_slice.len() >= 20 {
        let window = &r_slice[r_slice.len() - 20..];
        let min_r = window.iter().copied().fold(f64::INFINITY, f64::min);
        tracing::info!(
            tick,
            history_len = r_slice.len(),
            min_r = format!("{min_r:.4}"),
            current_r = format!("{r:.4}"),
            "field_stability probe"
        );
        match detector.detect_field_stability(r_slice, tick) {
            Ok(Some(id)) => {
                tracing::info!(
                    id,
                    "Emergence: FieldStability detected (sustained r above threshold)"
                );
            }
            Err(e) => tracing::debug!("Emergence field_stability check error: {e}"),
            _ => {}
        }
    }

    // 5. Thermal spike detection (requires SYNTHEX bridge, every 6 ticks)
    #[cfg(feature = "bridges")]
    if tick % 6 == 0 {
        if let Some(resp) = state.synthex_bridge.last_response() {
            if let Err(e) = detector.detect_thermal_spike(resp.temperature, resp.target, tick) {
                tracing::debug!("Emergence thermal_spike check error: {e}");
            }
        }
    }

    // 6. Chimera formation detection (every 12 ticks — slow-evolving state)
    // Detects multi-cluster phase splits where subgroups desynchronize
    // while overall r remains moderate.
    if tick % 12 == 0 {
        let phases: Vec<f64> = {
            let guard = state.field_state.read();
            guard.spheres.values().map(|s| s.phase).collect()
        };
        if phases.len() >= 4 {
            if let Err(e) = detector.detect_chimera(&phases, r, tick) {
                tracing::debug!("Emergence chimera check: {e}");
            }
        }
    }

    // 7. DispatchLoop detection (monitor-based, every 12 ticks)
    // Detects when a single semantic domain receives a disproportionate burst
    // of dispatches, indicating a task routing feedback loop.
    if tick % 12 == 0 {
        let current_total = state.dispatch_total.load(std::sync::atomic::Ordering::Relaxed);
        let delta = current_total.saturating_sub(monitors.prev_dispatch_total);
        monitors.prev_dispatch_total = current_total;

        if delta >= 3 {
            let mid = *monitors.dispatch_monitor.get_or_insert_with(|| {
                detector.start_monitor(
                    EmergenceType::DispatchLoop,
                    tick,
                )
            });
            #[allow(clippy::cast_precision_loss)]
            let value = (delta as f64 / 12.0).min(1.0);
            let _ = detector.add_evidence(
                mid,
                format!("{delta} dispatches in 12 ticks"),
                value,
                tick,
            );
            match detector.check_monitor(mid, tick) {
                Ok(Some(id)) => tracing::info!(id, delta, "Emergence: DispatchLoop detected"),
                Err(e) => tracing::debug!("Emergence dispatch_loop check error: {e}"),
                _ => {}
            }
        }
    }

    // 8. ConsentCascade detection (monitor-based, every 12 ticks)
    // Detects when multiple spheres opt out of Hebbian coupling in a short
    // window, indicating systemic coupling rejection.
    if tick % 12 == 0 {
        let opt_out_count = state
            .field_state
            .read()
            .spheres
            .values()
            .filter(|s| s.opt_out_hebbian)
            .count();
        let increase = opt_out_count.saturating_sub(monitors.prev_opt_out_count);
        monitors.prev_opt_out_count = opt_out_count;

        if increase >= 2 {
            let mid = *monitors.consent_monitor.get_or_insert_with(|| {
                detector.start_monitor(
                    EmergenceType::ConsentCascade,
                    tick,
                )
            });
            #[allow(clippy::cast_precision_loss)]
            let value = (increase as f64 / 5.0).min(1.0);
            let _ = detector.add_evidence(
                mid,
                format!("{increase} new opt-outs in 12 ticks"),
                value,
                tick,
            );
            match detector.check_monitor(mid, tick) {
                Ok(Some(id)) => tracing::info!(id, increase, "Emergence: ConsentCascade detected"),
                Err(e) => tracing::debug!("Emergence consent_cascade check error: {e}"),
                _ => {}
            }
        }
    }
}

/// Post field state to SYNTHEX `/api/ingest` endpoint (METABOLIC-GAP-1 fix).
///
/// Feeds all 4 heat sources with live data:
/// - HS-001 (`r`): Kuramoto order parameter from cached field state
/// - HS-002 (`cascade_heat`): tool call rate since last post (normalized)
/// - HS-003 (`me_fitness`): RALPH current fitness
/// - HS-004 (`nexus_health`): breaker closed fraction or sphere count fallback
#[cfg(all(feature = "bridges", feature = "evolution"))]
#[allow(clippy::items_after_statements, clippy::too_many_lines)] // static follows use; 7 heat source computations
fn post_field_to_synthex(state: &OracState, tick: u64) {
    use std::sync::atomic::Ordering;

    // Gen 17: Track first successful SYNTHEX post to trigger PID reset.
    static FIRST_POST_DONE: std::sync::atomic::AtomicBool =
        std::sync::atomic::AtomicBool::new(false);

    let (r, sphere_count) = {
        let fs = state.field_state.read();
        (fs.field.order.r, fs.spheres.len())
    };
    let k_mod = state.coupling.read().k_modulation;
    let me_fitness = state.ralph.state().current_fitness;

    // Gen 11: Compute nexus_health from breaker closed fraction (preferred)
    // or sphere count fallback when intelligence feature not enabled
    #[cfg(feature = "intelligence")]
    let nexus_health = {
        let (closed, open, half_open) = state.breaker_state_counts();
        let total = closed + open + half_open;
        if total > 0 {
            #[allow(clippy::cast_precision_loss)]
            { closed as f64 / total as f64 }
        } else {
            #[allow(clippy::cast_precision_loss)]
            { (sphere_count as f64 / 10.0).min(1.0) }
        }
    };
    #[cfg(not(feature = "intelligence"))]
    #[allow(clippy::cast_precision_loss)]
    let nexus_health = (sphere_count as f64 / 10.0).min(1.0);

    // Compute cascade_heat from tool call rate (tools per 6-tick window, normalized)
    let current_calls = state.total_tool_calls.load(Ordering::Relaxed);
    let prev_calls = state.tool_calls_at_last_thermal.swap(current_calls, Ordering::Relaxed);
    let call_delta = current_calls.saturating_sub(prev_calls);
    // Normalize: 0 calls = 0.0, 30+ calls per window = 1.0
    #[allow(clippy::cast_precision_loss)]
    let cascade_heat = (call_delta as f64 / 30.0).min(1.0);

    // HS-003 resonance: coupling weight mean + variance as resonance proxy.
    // Gen-059g: Enhanced with variance — high variance = diverse learning = higher resonance.
    let (resonance, weight_variance) = {
        let net = state.coupling.read();
        if net.connections.is_empty() {
            (me_fitness, 0.0) // fallback
        } else {
            #[allow(clippy::cast_precision_loss)]
            let n = net.connections.len() as f64;
            let w_mean = net.connections.iter().map(|c| c.weight).sum::<f64>() / n;
            let variance = net.connections.iter()
                .map(|c| (c.weight - w_mean).powi(2))
                .sum::<f64>() / n;
            // Resonance: weight mean + sqrt(variance) bonus
            let res = (w_mean + variance.sqrt() * 2.0).min(1.0);
            (res, variance)
        }
    };

    // GAP-C fix: include coupling and STDP metrics for thermal regulation
    let (coupling_connections, co_activations) = {
        let net = state.coupling.read();
        (net.connections.len(), state.co_activations_total.load(Ordering::Relaxed))
    };

    // Gen-059g: Feed emergence metrics into SYNTHEX thermal regulation.
    // Cross-service flow: ORAC emergence detector → SYNTHEX heat source.
    let (emergence_heat, emergence_diversity) = {
        let em_stats = state.ralph.emergence().stats();
        #[allow(clippy::cast_precision_loss)]
        let heat = (em_stats.total_detected as f64 / 10.0).min(1.0);
        // Diversity: count of unique emergence types that have fired (0-8)
        #[allow(clippy::cast_precision_loss)]
        let diversity = (em_stats.by_type.len() as f64 / 8.0).min(1.0);
        (heat, diversity)
    };

    let payload = serde_json::json!({
        "r": r,
        "k_mod": k_mod,
        "spheres": sphere_count,
        "me_fitness": me_fitness,
        "nexus_health": nexus_health,
        "cascade_heat": cascade_heat,
        "resonance": resonance,
        "cross_sync": nexus_health,
        "coupling_connections": coupling_connections,
        "co_activations": co_activations,
        "emergence_heat": emergence_heat,
        "emergence_diversity": emergence_diversity,
        "weight_variance": weight_variance,
    });

    let result = state.synthex_bridge.post_field_state(
        payload.to_string().as_bytes(),
    );

    // Gen 17: On first successful post, trigger SYNTHEX PID reset via decay cycle.
    // The PID has accumulated negative integral while temperature was 0.0.
    // Resetting prevents overshoot when real data first arrives.
    if result.is_ok() && !FIRST_POST_DONE.swap(true, Ordering::Relaxed) {
        if let Err(e) = orac_sidecar::m5_bridges::http_helpers::raw_http_post(
            "127.0.0.1:8090",
            "/v3/decay/trigger",
            b"",
            "synthex",
        ) {
            tracing::debug!("SYNTHEX PID reset on first post failed: {e}");
        } else {
            tracing::info!("SYNTHEX PID reset triggered (first thermal ingest)");
        }
    }

    match &result {
        Ok(()) => {
            if tick % 12 == 0 {
                tracing::info!(
                    r = format!("{r:.3}"),
                    me_fitness = format!("{me_fitness:.3}"),
                    spheres = sphere_count,
                    "SYNTHEX ingest posted"
                );
            }
        }
        Err(e) => {
            if tick % 12 == 0 {
                tracing::warn!("SYNTHEX ingest post failed: {e}");
            }
        }
    }
}

/// Persist STDP weight changes to POVM as pathway co-activations (METABOLIC-GAP-4 fix).
///
/// Posts the top coupling connections (by weight) to POVM `/pathways` endpoint,
/// creating persistent Hebbian pathways that survive daemon restarts.
#[cfg(all(feature = "bridges", feature = "intelligence"))]
fn persist_stdp_to_povm(
    state: &OracState,
    stdp_result: &orac_sidecar::m4_intelligence::m18_hebbian_stdp::StdpResult,
    tick: u64,
) {
    use orac_sidecar::m5_bridges::http_helpers::raw_http_post;

    let network = state.coupling.read();
    // Take top 10 connections by weight for persistence
    let mut top_conns: Vec<_> = network
        .connections
        .iter()
        .map(|c| (c.from.as_str().to_owned(), c.to.as_str().to_owned(), c.weight))
        .collect();
    top_conns.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    top_conns.truncate(10);
    drop(network);

    // BUG-059f fix: Send individual pathway upserts instead of bulk format.
    // POVM POST /pathways expects a single {pre_id, post_id, weight, co_activations}
    // object, not a {pathways: [...]} array. The bulk format silently fails.
    let mut ok_count = 0u32;
    let mut err_count = 0u32;
    for (from, to, weight) in &top_conns {
        let payload = serde_json::json!({
            "pre_id": from,
            "post_id": to,
            "weight": weight,
            // RALPH Gen 7 fix: Use cumulative co_activations_total from OracState
            // instead of per-tick ltp_count. Previous code sent the small per-tick
            // delta which got overwritten on each 60-tick persist cycle, keeping
            // POVM co_activations near zero. Cumulative total accumulates properly.
            "co_activations": state.co_activations_total.load(std::sync::atomic::Ordering::Relaxed),
        });
        if raw_http_post(
            "127.0.0.1:8125",
            "/pathways",
            payload.to_string().as_bytes(),
            "povm",
        )
        .is_ok()
        {
            ok_count += 1;
        } else {
            err_count += 1;
        }
    }

    // Record breaker outcome based on majority result
    #[cfg(feature = "intelligence")]
    if ok_count > err_count {
        state.breaker_success("povm");
    } else if err_count > 0 {
        state.breaker_failure("povm");
    }

    if tick % 120 == 0 && ok_count > 0 {
        tracing::info!(
            ltp = stdp_result.ltp_count,
            persisted = ok_count,
            failed = err_count,
            "POVM pathways persisted from STDP"
        );
    }
}

/// Post RALPH evolution state to VMS as a memory (METABOLIC-GAP-2 fix).
///
/// Every 30 ticks (~2.5 min), writes a memory to VMS with current RALPH
/// generation, fitness, phase, and field coherence. This feeds VMS's
/// memory field, enabling intent routing and pattern detection.
#[cfg(all(feature = "bridges", feature = "evolution"))]
fn post_state_to_vms(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::http_helpers::raw_http_post;

    // T4: VMS breaker guard — skip if breaker is Open
    if !state.breaker_allows("vms") {
        return;
    }

    let ralph_state = state.ralph.state();
    let r = state.field_state.read().field.order.r;
    let sphere_count = state.field_state.read().spheres.len();

    let payload = serde_json::json!({
        "tool": "write_memory",
        "params": {
            "content": {
                "type": "field_observation",
                "tick": tick,
                "r": format!("{r:.4}"),
                "ralph_gen": ralph_state.generation,
                "ralph_fitness": format!("{:.4}", ralph_state.current_fitness),
                "ralph_phase": format!("{}", ralph_state.phase),
                "spheres": sphere_count,
            },
            "region": "field_state"
        }
    });

    match raw_http_post(
        "127.0.0.1:8120",
        "/mcp/tools/call",
        payload.to_string().as_bytes(),
        "vms",
    ) {
        Ok(_status) => {
            state.breaker_success("vms");
            if tick % 60 == 0 {
                tracing::info!(
                    r = format!("{r:.3}"),
                    gen = ralph_state.generation,
                    "VMS field observation posted"
                );
            }
        }
        Err(e) => {
            state.breaker_failure("vms");
            if tick % 60 == 0 {
                tracing::warn!("VMS memory post failed: {e}");
            }
        }
    }
}

/// Trigger VMS memory consolidation (IGNITION-1c).
///
/// Sends a POST to VMS `/v1/adaptation/trigger` to initiate decay, pruning, and
/// crystallization of accumulated memories. Without this trigger, VMS memories
/// accumulate indefinitely at `morphogenic_cycle=0`.
#[cfg(feature = "bridges")]
fn trigger_vms_consolidation(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::http_helpers::raw_http_post;

    // T4: VMS breaker guard
    if !state.breaker_allows("vms") {
        return;
    }

    // BUG-SCAN-006 fix: VMS expects `RegionIntensity` structs in an array,
    // not a flat JSON object. Previous format returned 422.
    let payload = serde_json::json!({
        "intensities": [{"region": "consolidation", "intensity": 1.0}]
    });
    match raw_http_post(
        "127.0.0.1:8120",
        "/v1/adaptation/trigger",
        payload.to_string().as_bytes(),
        "vms",
    ) {
        Ok(status) => {
            state.breaker_success("vms");
            tracing::info!(tick, status, "VMS consolidation triggered");
        }
        Err(e) => {
            state.breaker_failure("vms");
            tracing::warn!(tick, "VMS consolidation trigger failed: {e}");
        }
    }
}

/// Hydrate top POVM memories into blackboard `povm_context` table (Session 066).
///
/// Every 60 ticks (~300s), fetches memories from POVM sorted by intensity,
/// then upserts the top 10 into the blackboard for tick-loop access.
/// ACP-F5: Runs in async context alongside bridge polling, NOT in `tick_once()`.
#[cfg(all(feature = "persistence", feature = "bridges"))]
fn hydrate_povm_to_blackboard(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::http_helpers::raw_http_get;

    let resp = match raw_http_get("127.0.0.1:8125", "/memories?limit=20", "povm") {
        Ok(body) => body,
        Err(e) => {
            tracing::debug!("POVM hydration fetch failed: {e}");
            return;
        }
    };
    let memories: Vec<serde_json::Value> = match serde_json::from_str(&resp) {
        Ok(m) => m,
        Err(e) => {
            tracing::debug!("POVM hydration parse failed: {e}");
            return;
        }
    };

    let Some(bb) = state.blackboard() else {
        return;
    };

    // Sort by intensity descending and take top 10
    let mut sorted = memories;
    sorted.sort_by(|a, b| {
        let ia = a.get("intensity").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let ib = b.get("intensity").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        ib.partial_cmp(&ia).unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.truncate(10);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64());

    let mut injected = 0u32;
    for mem in &sorted {
        let id = mem.get("id").and_then(serde_json::Value::as_str).unwrap_or("");
        let content = mem.get("content").and_then(serde_json::Value::as_str).unwrap_or("");
        let intensity = mem.get("intensity").and_then(serde_json::Value::as_f64).unwrap_or(0.0);
        let crystallised = mem.get("crystallised").and_then(serde_json::Value::as_bool).unwrap_or(false);

        if id.is_empty() {
            continue;
        }

        // Truncate content to 200 chars for summary
        let summary: String = content.chars().take(200).collect();

        let sql = "INSERT OR REPLACE INTO povm_context \
                   (memory_id, content_summary, intensity, crystallised, injected_tick, injected_at) \
                   VALUES (?1, ?2, ?3, ?4, ?5, ?6)";
        if bb.execute_sql(sql, &[id, &summary, &intensity.to_string(), &(i32::from(crystallised)).to_string(), &tick.to_string(), &now.to_string()]).is_ok() {
            injected += 1;
        }
    }

    if injected > 0 {
        tracing::info!(injected, tick, "POVM memories hydrated into blackboard");
    }
}

/// Query VMS for semantic memories relevant to current field state (IGNITION-1d).
///
/// During RALPH Recognize phase, asks VMS for the 5 most relevant memories to
/// the current field state. Feeds results into RALPH's correlation engine as
/// environmental context, enabling evolution informed by accumulated observations.
#[cfg(all(feature = "bridges", feature = "evolution"))]
fn query_vms_for_ralph_context(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::http_helpers::raw_http_post_with_response;

    // T4: VMS breaker guard
    if !state.breaker_allows("vms") {
        return;
    }

    let r = state.field_state.read().field.order.r;
    let fitness = state.ralph.state().current_fitness;

    // Gen-063f: Use REST /v1/query_semantic instead of MCP /mcp/tools/call.
    // ACP-VMS finding: MCP query_semantic is "unknown tool" on running binary (30/47 tools),
    // but REST /v1/query_semantic WORKS. Returns {results: [{address, content(STRING),
    // geometric_relevance, tensor_similarity}], query_tensor, geometric_count} with k-truncation.
    // Response ~660 bytes for k=2 (vs 527KB+ from query_relevant).
    let query_payload = serde_json::json!({
        "query": format!("field r={r:.3} fitness={fitness:.3}"),
        "k": 3,
        "threshold": 0.5
    });

    match raw_http_post_with_response(
        "127.0.0.1:8120",
        "/v1/query_semantic",
        query_payload.to_string().as_bytes(),
        "vms",
    ) {
        Ok(body) => {
            state.breaker_success("vms");

            // REST /v1/query_semantic returns direct JSON (no MCP envelope):
            // {results: [{address, content: STRING, geometric_relevance, tensor_similarity}]}
            match serde_json::from_str::<serde_json::Value>(&body) {
                Ok(json) => {
                    let results = json.get("results")
                        .and_then(serde_json::Value::as_array);
                    let mem_count = results.map_or(0, Vec::len);
                    if mem_count > 0 {
                        // Gen-063f: Feed VMS memories into RALPH correlation engine.
                        let corr = state.ralph.correlation();
                        let mut ingested = 0u32;
                        if let Some(mems) = results {
                            for mem in mems {
                                // REST query_semantic returns content as a STRING
                                let content = mem.get("content")
                                    .and_then(serde_json::Value::as_str)
                                    .unwrap_or("");
                                if !content.is_empty() {
                                    let relevance = mem.get("geometric_relevance")
                                        .or_else(|| mem.get("tensor_similarity"))
                                        .and_then(serde_json::Value::as_f64)
                                        .unwrap_or(0.5);
                                    corr.ingest(
                                        "vms_memory",
                                        "field_state",
                                        relevance,
                                        tick,
                                        Some(content.get(..64).unwrap_or(content)),
                                    );
                                    ingested += 1;
                                }
                            }
                        }
                        tracing::info!(
                            tick, mem_count, ingested,
                            "VMS→RALPH: fed memories into correlation engine"
                        );
                    }
                }
                Err(e) => {
                    if tick % 60 == 0 {
                        tracing::warn!("VMS query response parse failed: {e}");
                    }
                }
            }
        }
        Err(e) => {
            state.breaker_failure("vms");
            if tick % 60 == 0 {
                tracing::warn!("VMS query failed: {e}");
            }
        }
    }
}

/// Persist RALPH evolution state to Reasoning Memory as TSV (METABOLIC-GAP-6 fix).
///
/// Every 60 ticks (~5 min), writes a TSV record to RM with current RALPH
/// generation, fitness, phase, field coherence, and ME fitness. This creates
/// cross-session persistence that survives daemon restarts.
#[cfg(all(feature = "bridges", feature = "evolution"))]
fn post_state_to_rm(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::m25_rm_bridge::RmRecord;

    let ralph_state = state.ralph.state();
    let r = state.field_state.read().field.order.r;
    let sphere_count = state.field_state.read().spheres.len();

    #[cfg(feature = "bridges")]
    let me_fitness = state.me_bridge.last_fitness();
    #[cfg(not(feature = "bridges"))]
    let me_fitness = 0.0;

    let content = format!(
        "tick={tick} r={r:.4} gen={} fitness={:.4} phase={} spheres={sphere_count} me_fitness={me_fitness:.4}",
        ralph_state.generation,
        ralph_state.current_fitness,
        ralph_state.phase,
    );

    let record = RmRecord::new(
        "shared_state",
        "orac-sidecar",
        0.90,
        600, // 10-minute TTL
        content,
    );

    match state.rm_bridge.post_record(&record) {
        Ok(()) => {
            if tick % 120 == 0 {
                tracing::info!(
                    gen = ralph_state.generation,
                    fitness = format!("{:.4}", ralph_state.current_fitness),
                    "RM state persisted"
                );
            }
        }
        Err(e) => {
            if tick % 120 == 0 {
                tracing::debug!("RM state persistence failed: {e}");
            }
        }
    }
}

/// Relay emergence events to Reasoning Memory for cross-session persistence.
///
/// Session 071 #7: Relay emergence hints to ME V2 evolution chamber.
/// `CoherenceLock` → ME hint "lower `r_target`". `ThermalSpike` → ME hint "thermal".
/// Creates cross-pollination: ORAC fleet intelligence feeds ME service evolution.
///
/// **Data flow:** ORAC `EmergenceDetector` → ME `:8080` `/api/tools/learning-cycle` (HTTP POST)
#[cfg(all(feature = "bridges", feature = "evolution"))]
fn relay_emergence_to_me(state: &OracState, tick: u64) {
    let recent = state.ralph.emergence().recent(3);
    for record in &recent {
        if record.detected_at_tick != tick {
            continue;
        }
        // Only relay actionable emergence types
        let hint = match record.emergence_type {
            EmergenceType::CoherenceLock => "coherence_lock: lower r_target or increase diversity",
            EmergenceType::ThermalSpike => "thermal_spike: reduce load or cool system",
            EmergenceType::CouplingRunaway => "coupling_runaway: reduce K modulation",
            _ => continue,
        };
        let body = format!(
            r#"{{"tool_id":"learning-cycle","params":{{"hint":"{hint}","source":"orac-emergence","tick":{tick}}}}}"#,
        );
        let addr = "127.0.0.1:8080";
        let request = format!(
            "POST /api/tools/learning-cycle HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        );
        // Fire-and-forget — ME processes asynchronously
        if let Ok(stream) = std::net::TcpStream::connect_timeout(
            &addr.parse().unwrap_or_else(|_| std::net::SocketAddr::from(([127,0,0,1], 8080))),
            std::time::Duration::from_secs(1),
        ) {
            use std::io::Write;
            let mut s = stream;
            let _ = s.write_all(request.as_bytes());
            tracing::info!(etype = %record.emergence_type, hint, "Emergence hint relayed to ME");
        }
    }
}

/// When new emergence events are detected this tick, posts them to RM as TSV
/// records with category "emergence". This enables cross-session correlation:
/// future sessions can search RM for historical emergence patterns.
///
/// **Data flow:** ORAC `EmergenceDetector` → RM :8130 (TSV)
#[cfg(all(feature = "bridges", feature = "evolution"))]
fn relay_emergence_to_rm(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::m25_rm_bridge::RmRecord;

    let recent = state.ralph.emergence().recent(5);
    for record in &recent {
        if record.detected_at_tick != tick {
            continue;
        }
        let content = format!(
            "type={} severity={} confidence={:.2} tick={tick} desc={}",
            record.emergence_type,
            record.severity_class,
            record.confidence,
            record.description,
        );
        let rm_record = RmRecord::new(
            "emergence",
            "orac-sidecar",
            record.confidence,
            1800, // 30-minute TTL for emergence events
            content,
        );
        if let Err(e) = state.rm_bridge.post_record(&rm_record) {
            tracing::debug!("RM emergence relay failed: {e}");
        } else {
            tracing::info!(
                etype = record.emergence_type.to_string(),
                conf = format!("{:.2}", record.confidence),
                "Emergence event relayed to RM"
            );
        }
    }
}

/// Build a 12D fitness tensor from current ORAC state.
///
/// Populates dimensions from live data where available,
/// uses calibrated placeholders for dimensions not yet wired.
///
/// # Dimensions populated from live state
/// - D0: `coordination_quality` — session count / 9 fleet panes
/// - D1: `field_coherence` — from cached PV2 field (via `collect_tensor`)
/// - D5: `coupling_stability` — derived from breaker state
/// - D10: `bridge_health` — fraction of breakers in Closed state
#[cfg(feature = "evolution")]
#[allow(clippy::too_many_lines)] // 12 fitness dimensions each with independent computation
fn build_tensor_from_state(state: &OracState) -> TensorValues {
    // collect_tensor reads from blackboard (D3 task_throughput, D4 error_rate,
    // D9 fleet_utilization), field_state (D1 field_coherence), and consents
    // (D11 consent_compliance). Remaining dimensions default to 0.5 (neutral).
    let mut tensor = state.collect_tensor();

    // D0: coordination_quality — session count / 9 fleet panes
    // u32→f64 is lossless; unwrap_or(0) caps at u32::MAX (impossible with 9 fleet panes)
    let session_count: f64 = f64::from(u32::try_from(state.session_count()).unwrap_or(0));
    tensor.set(FitnessDimension::CoordinationQuality, (session_count / 9.0).min(1.0));

    // D4: error_rate — derived from breaker state.
    // Gen-059g: All Closed = 1.0 (no errors), any Open = lower score.
    #[cfg(feature = "intelligence")]
    {
        let (closed, open, half_open) = state.breaker_state_counts();
        let total = closed + open + half_open;
        if total > 0 {
            #[allow(clippy::cast_precision_loss)]
            let error_rate = closed as f64 / total as f64;
            tensor.set(FitnessDimension::ErrorRate, error_rate);
        }
    }

    // D2: dispatch_accuracy — tool call rate as dispatch activity proxy.
    // Gen-059g: Normalized tool calls per tick (active dispatching).
    {
        let calls = state.total_tool_calls.load(std::sync::atomic::Ordering::Relaxed);
        let tick = state.tick.load(std::sync::atomic::Ordering::Relaxed);
        if tick > 0 {
            #[allow(clippy::cast_precision_loss)]
            let rate = (calls as f64 / tick as f64).min(1.0);
            // Even low activity gets some credit (0.3 base + scaled rate)
            tensor.set(FitnessDimension::DispatchAccuracy, (0.3 + rate * 0.7).min(1.0));
        }
    }

    // D6: hebbian_health — coupling weight mean + variance (healthy = differentiated weights)
    // Gen-059g: Enhanced to penalize weight floor collapse. When >50% of weights
    // are at floor (0.15), health drops sharply. Variance bonus rewards differentiation.
    #[cfg(feature = "intelligence")]
    {
        let net = state.coupling.read();
        if !net.connections.is_empty() {
            #[allow(clippy::cast_precision_loss)]
            let n = net.connections.len() as f64;
            let w_mean = net.connections.iter().map(|c| c.weight).sum::<f64>() / n;
            // Variance: measure weight differentiation
            let variance = net.connections.iter()
                .map(|c| (c.weight - w_mean).powi(2))
                .sum::<f64>() / n;
            // Floor collapse ratio: fraction of weights within 0.01 of floor
            #[allow(clippy::cast_precision_loss)]
            let at_floor = net.connections.iter()
                .filter(|c| (c.weight - 0.15).abs() < 0.01)
                .count() as f64 / n;
            // Base: weight mean normalized
            let base = ((w_mean - 0.15) / 0.85).clamp(0.0, 1.0);
            // Variance bonus: sqrt(variance) * 5, capped at 0.2
            let var_bonus = (variance.sqrt() * 5.0).min(0.2);
            // Collapse penalty: if >50% at floor, penalize harshly
            let collapse_penalty = if at_floor > 0.5 { (at_floor - 0.5) * 0.6 } else { 0.0 };
            let health = (base + var_bonus - collapse_penalty).clamp(0.0, 1.0);
            tensor.set(FitnessDimension::HebbianHealth, health);
        }
    }

    // D7: coupling_stability — fraction of circuit breakers in Closed state
    #[cfg(feature = "intelligence")]
    {
        let (closed, open, half_open) = state.breaker_state_counts();
        let total = closed + open + half_open;
        if total > 0 {
            #[allow(clippy::cast_precision_loss)]
            let closed_frac = closed as f64 / total as f64;
            tensor.set(FitnessDimension::CouplingStability, closed_frac);
        }
    }

    // D8: thermal_balance — how close SYNTHEX temperature is to target (1.0 = on target)
    #[cfg(feature = "bridges")]
    {
        let thermal = state.synthex_bridge.last_response();
        if let Some(resp) = thermal {
            let delta = (resp.temperature - resp.target).abs();
            // 0.0 delta → 1.0 score, 0.5 delta → 0.0 score
            let balance = (1.0 - delta * 2.0).clamp(0.0, 1.0);
            tensor.set(FitnessDimension::ThermalBalance, balance);
        }
    }

    // D5: latency — SYNTHEX thermal responsiveness as latency proxy.
    // BUG-060d: Was hardcoded at 0.85, capping fitness. Now uses thermal
    // convergence quality: closer to target → higher score. This allows
    // D5 to reach 1.0 when thermal perfectly converges.
    #[cfg(feature = "bridges")]
    {
        let freshness: f64 = if let Some(resp) = state.synthex_bridge.last_response() {
            let delta = (resp.temperature - resp.target).abs();
            // Perfect convergence (delta=0) → 1.0, delta≥0.25 → 0.5
            (1.0 - delta * 2.0).clamp(0.5, 1.0)
        } else {
            0.2
        };
        tensor.set(FitnessDimension::Latency, freshness);
    }

    // D3: task_throughput — ME fitness as proxy for system task health
    // (replaces neutral 0.5 placeholder when ME bridge is active)
    #[cfg(feature = "bridges")]
    {
        let me_fitness = state.me_bridge.last_fitness();
        if me_fitness > 0.0 && !state.me_bridge.is_frozen() {
            tensor.set(FitnessDimension::TaskThroughput, me_fitness);
        }
    }

    // D9: fleet_utilization — ratio of Working spheres to total from PV2 field state.
    // Gen-059g: Cross-service data flow PV2→ORAC fitness tensor. Previously stubbed at 0.5.
    {
        let guard = state.field_state.read();
        let total = guard.spheres.len();
        if total > 0 {
            let working = guard
                .spheres
                .values()
                .filter(|sp| {
                    matches!(
                        sp.status,
                        orac_sidecar::m1_core::m01_core_types::PaneStatus::Working
                    )
                })
                .count();
            #[allow(clippy::cast_precision_loss)]
            let utilization = working as f64 / total as f64;
            tensor.set(FitnessDimension::FleetUtilization, utilization.max(0.05));
        }
    }

    // D10: emergence_rate — beneficial emergence detection rate
    #[cfg(feature = "evolution")]
    {
        let em_stats = state.ralph.emergence().stats();
        let total = em_stats.total_detected;
        // Normalize: 0 = 0.5 (neutral), 1-5 = 0.6-0.8, 10+ = 1.0
        #[allow(clippy::cast_precision_loss)]
        let score = if total == 0 {
            0.5
        } else {
            (0.5 + (total as f64 / 20.0).min(0.5)).min(1.0)
        };
        tensor.set(FitnessDimension::EmergenceRate, score);
    }

    tensor
}

/// Spawn the RALPH evolution tick loop as a background tokio task.
///
/// Runs every 5 seconds, computing a fitness tensor from live state
/// and feeding it to the RALPH 5-phase engine. Stops on shutdown signal.
#[cfg(feature = "evolution")]
#[allow(clippy::too_many_lines)]
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
        let mut monitor_tracking = MonitorTracking::new();

        // Conductor for advisory field decisions (Phase 3 of tick loop)
        let conductor = orac_sidecar::m6_coordination::m27_conductor::Conductor::new();

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick = state.increment_tick();

                    // Advance circuit breaker FSMs (Open→HalfOpen after timeout)
                    #[cfg(feature = "intelligence")]
                    state.breaker_tick();

                    // Run conductor advisory tick on cached field state
                    {
                        let mut app_state = state.field_state.write();
                        let tick_result = orac_sidecar::m6_coordination::m29_tick::tick_once(
                            &mut app_state,
                            &conductor,
                        );
                        if tick % 60 == 0 {
                            tracing::debug!(
                                r = format!("{:.4}", tick_result.order_parameter.r),
                                action = ?tick_result.decision.action,
                                k_delta = format!("{:.4}", tick_result.decision.k_delta),
                                governance = tick_result.governance_active,
                                "Conductor advisory tick"
                            );
                        }
                    }

                    // ── Phase 4: Hebbian STDP pass (BUG-060 fix) ──
                    // Clone spheres first (read lock), then acquire coupling write lock.
                    // Lock ordering: field_state read dropped before coupling write.
                    #[cfg(feature = "intelligence")]
                    {
                        let spheres = state.field_state.read().spheres.clone();

                        // GAP-E diagnostic: log sphere/connection ID mismatch
                        if tick % 60 == 0 {
                            let net = state.coupling.read();
                            let conn_ids: std::collections::HashSet<_> = net.connections.iter()
                                .flat_map(|c| [c.from.clone(), c.to.clone()])
                                .collect();
                            let sphere_ids: std::collections::HashSet<_> = spheres.keys().cloned().collect();
                            let matched = conn_ids.intersection(&sphere_ids).count();
                            let working = spheres.values().filter(|s| s.status == orac_sidecar::m1_core::m01_core_types::PaneStatus::Working).count();
                            tracing::info!(
                                conn_ids = conn_ids.len(),
                                sphere_ids = sphere_ids.len(),
                                matched,
                                working,
                                "STDP ID alignment check"
                            );
                        }
                        drop(spheres);

                        let spheres = state.field_state.read().spheres.clone();
                        let stdp_result = apply_stdp(
                            &mut state.coupling.write(),
                            &spheres,
                        );
                        // GAP-E fix: increment co_activations counter from STDP results
                        if stdp_result.ltp_count > 0 {
                            state.co_activations_total.fetch_add(
                                stdp_result.ltp_count as u64,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                            // GAP-A fix: increment hebbian_ltp_total for health endpoint
                            state.hebbian_ltp_total.fetch_add(
                                stdp_result.ltp_count as u64,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                        }

                        // GAP-A fix: increment hebbian_ltd_total for health endpoint
                        if stdp_result.ltd_count > 0 {
                            state.hebbian_ltd_total.fetch_add(
                                stdp_result.ltd_count as u64,
                                std::sync::atomic::Ordering::Relaxed,
                            );
                        }

                        if stdp_result.ltp_count > 0 || stdp_result.ltd_count > 0 {
                            // BUG-SCAN-001 fix: record last tick STDP ran for /health
                            state.hebbian_last_tick.store(tick, std::sync::atomic::Ordering::Relaxed);

                            tracing::debug!(
                                ltp = stdp_result.ltp_count,
                                ltd = stdp_result.ltd_count,
                                floor = stdp_result.at_floor_count,
                                delta = format!("{:.6}", stdp_result.total_weight_change),
                                "STDP Phase 4: Hebbian update applied"
                            );

                            // METABOLIC-GAP-4 fix: Persist significant STDP weight changes
                            // to POVM every 60 ticks as pathway co-activations
                            #[cfg(all(feature = "bridges", feature = "intelligence"))]
                            if tick % 60 == 0 {
                                persist_stdp_to_povm(&state, &stdp_result, tick);
                            }
                        }
                        if tick % 12 == 0 {
                            let net = state.coupling.read();
                            let conn_count = net.connections.len();
                            // BUG-057p fix: guard fold against empty connections
                            // (f64::MAX/MIN as initial values are misleading in logs)
                            if conn_count > 0 {
                                let (w_min, w_max) = net.connections.iter().fold(
                                    (f64::MAX, f64::MIN),
                                    |(lo, hi), c| (lo.min(c.weight), hi.max(c.weight)),
                                );
                                tracing::info!(
                                    connections = conn_count,
                                    w_min = format!("{w_min:.4}"),
                                    w_max = format!("{w_max:.4}"),
                                    ltp = stdp_result.ltp_count,
                                    ltd = stdp_result.ltd_count,
                                    "STDP summary"
                                );
                            } else {
                                tracing::debug!(
                                    connections = 0,
                                    "STDP summary: no connections (no active sessions)"
                                );
                            }
                        }
                    }

                    // G1b: Homeostatic weight normalization — pull saturated weights
                    // toward the mean every 120 ticks. Ceiling weights decay by 2%,
                    // floor weights get a small additive boost. This prevents binary
                    // weight collapse (99%+ at floor or ceiling).
                    if tick % 120 == 0 && tick > 0 {
                        let mut net = state.coupling.write();
                        if !net.connections.is_empty() {
                            #[allow(clippy::cast_precision_loss)]
                            let n = net.connections.len() as f64;
                            let w_mean = net.connections.iter().map(|c| c.weight).sum::<f64>() / n;
                            let floor = orac_sidecar::m1_core::m04_constants::HEBBIAN_WEIGHT_FLOOR;
                            let mut nudged = 0u32;
                            // BUG-064a fix: Use soft ceiling (0.85) and wider epsilon (0.01).
                            // Previous code checked weight==1.0 (never true since STDP
                            // caps at 0.85) and floor with 1e-10 epsilon (too tight).
                            let soft_ceiling = 0.85_f64;
                            for conn in &mut net.connections {
                                let old = conn.weight;
                                if (conn.weight - soft_ceiling).abs() < 0.01 {
                                    // Ceiling: multiplicative decay toward mean
                                    conn.weight = 0.98f64.mul_add(conn.weight, 0.02 * w_mean);
                                } else if (conn.weight - floor).abs() < 0.01 {
                                    // Floor: small additive boost toward mean
                                    conn.weight = (conn.weight + 0.005).min(w_mean);
                                }
                                conn.weight = conn.weight.clamp(floor, 1.0);
                                if (conn.weight - old).abs() > 1e-10 {
                                    nudged += 1;
                                }
                            }
                            if nudged > 0 {
                                tracing::info!(
                                    nudged,
                                    w_mean = format!("{w_mean:.4}"),
                                    "G1b homeostatic normalization applied"
                                );
                            }
                        }
                    }

                    // ── GAP-2 fix: Persist STDP summary to blackboard every 6 ticks ──
                    #[cfg(all(feature = "persistence", feature = "intelligence"))]
                    if tick % 6 == 0 && tick > 0 {
                        if let Some(bb) = state.blackboard() {
                            let net = state.coupling.read();
                            let conn_count = net.connections.len();
                            let (w_min, w_max, w_sum) = net.connections.iter().fold(
                                (f64::MAX, f64::MIN, 0.0_f64),
                                |(lo, hi, sum), c| (lo.min(c.weight), hi.max(c.weight), sum + c.weight),
                            );
                            #[allow(clippy::cast_precision_loss)]
                            let w_mean = if conn_count > 0 { w_sum / conn_count as f64 } else { 0.0 };
                            let floor_count = net.connections.iter()
                                .filter(|c| (c.weight - 0.15).abs() < 0.01)
                                .count();
                            drop(net);

                            // BUG-SCAN-004 fix: read cumulative LTP/LTD from atomics
                            // instead of hardcoded zeros.
                            let ltp = state.hebbian_ltp_total.load(std::sync::atomic::Ordering::Relaxed);
                            let ltd = state.hebbian_ltd_total.load(std::sync::atomic::Ordering::Relaxed);
                            let record = orac_sidecar::m5_bridges::m26_blackboard::HebbianSummaryRecord {
                                tick,
                                ltp_count: ltp,
                                ltd_count: ltd,
                                at_floor_count: floor_count as u64,
                                total_weight_change: w_sum,
                                connections_total: conn_count as u64,
                                weight_mean: w_mean,
                                weight_min: if conn_count > 0 { w_min } else { 0.0 },
                                weight_max: if conn_count > 0 { w_max } else { 0.0 },
                                created_at: std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map_or(0.0, |d| d.as_secs_f64()),
                            };
                            if let Err(e) = bb.insert_hebbian_summary(&record) {
                                tracing::debug!("Blackboard hebbian_summary write failed: {e}");
                            }
                        }
                    }

                    // IGNITION-1d: Query VMS semantic memories during Recognize phase.
                    // Feeds VMS environmental context into correlation engine so
                    // RALPH evolution benefits from accumulated memory patterns.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if state.ralph.state().phase == orac_sidecar::m8_evolution::m36_ralph_engine::RalphPhase::Recognize
                        && tick % 30 == 0
                    {
                        query_vms_for_ralph_context(&state, tick);
                    }

                    // SESSION-066: Periodic POVM hydration into blackboard.
                    // Every 60 ticks (~300s), fetch top memories from POVM and
                    // inject into povm_context table for tick-loop access.
                    // ACP-F5 fix: runs in async context (not tick_once) to
                    // avoid blocking the synchronous tick function.
                    #[cfg(all(feature = "persistence", feature = "bridges"))]
                    if tick % 60 == 0 && tick > 0 {
                        hydrate_povm_to_blackboard(&state, tick);
                    }

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

                    // GAP-C fix: Persist RALPH state to blackboard every 60 ticks
                    // so evolution survives ORAC restarts.
                    #[cfg(all(feature = "persistence", feature = "evolution"))]
                    if tick % 60 == 0 && tick > 0 {
                        if let Some(bb) = state.blackboard() {
                            let rs = state.ralph.state();
                            let agg = state.ralph.stats();
                            let saved = orac_sidecar::m5_bridges::m26_blackboard::SavedRalphState {
                                generation: rs.generation,
                                completed_cycles: rs.completed_cycles,
                                current_fitness: rs.current_fitness,
                                peak_fitness: agg.peak_fitness,
                                total_proposed: agg.total_proposed,
                                total_accepted: agg.total_accepted,
                                total_rolled_back: agg.total_rolled_back,
                                last_phase: rs.phase.name().to_owned(),
                            };
                            if let Err(e) = bb.save_ralph_state(&saved) {
                                tracing::debug!("RALPH state save failed: {e}");
                            }

                            // Persist active sessions alongside RALPH state
                            let sessions_guard = state.sessions.read();
                            let saved_sessions: Vec<_> = sessions_guard
                                .iter()
                                .map(|(sid, t)| {
                                    orac_sidecar::m5_bridges::m26_blackboard::SavedSession {
                                        session_id: sid.clone(),
                                        pane_id: t.pane_id.as_str().to_owned(),
                                        active_task_id: t.active_task_id.clone(),
                                        poll_counter: t.poll_counter,
                                        total_tool_calls: t.total_tool_calls,
                                        started_ms: t.started_ms,
                                        persona: t.persona.clone(),
                                    }
                                })
                                .collect();
                            drop(sessions_guard);
                            if let Err(e) = bb.save_sessions(&saved_sessions) {
                                tracing::debug!("Session save failed: {e}");
                            }

                            // IGNITION-1e: Persist coupling weights to blackboard
                            // every 60 ticks so Hebbian learning survives restarts.
                            // Gen-064a: Filter to only save connections where BOTH endpoints
                            // are registered PV2 spheres. Prevents accumulating dead
                            // orac:PID:UUID entries that can never be restored.
                            // Gen-064b: Save by PERSONA (stable across restarts) instead
                            // of PaneId (UUID, changes every restart). NAM Ch2: substrate
                            // independence — relationships must survive re-instantiation.
                            #[cfg(feature = "intelligence")]
                            {
                                let fs = state.field_state.read();
                                let sphere_ids: std::collections::HashSet<_> =
                                    fs.spheres.keys().cloned().collect();
                                let id_to_persona: std::collections::HashMap<_, _> = fs
                                    .spheres
                                    .iter()
                                    .filter(|(_, s)| !s.persona.is_empty())
                                    .map(|(id, s)| (id.clone(), s.persona.clone()))
                                    .collect();
                                drop(fs);
                                let net = state.coupling.read();
                                let saved_weights: Vec<_> = net
                                    .connections
                                    .iter()
                                    .filter(|c| sphere_ids.contains(&c.from) && sphere_ids.contains(&c.to))
                                    .filter_map(|c| {
                                        let from_p = id_to_persona.get(&c.from)?;
                                        let to_p = id_to_persona.get(&c.to)?;
                                        Some(orac_sidecar::m5_bridges::m26_blackboard::SavedCouplingWeight {
                                            from_id: from_p.clone(),
                                            to_id: to_p.clone(),
                                            weight: c.weight,
                                        })
                                    })
                                    .collect();
                                drop(net);
                                if let Err(e) = bb.save_coupling_weights(&saved_weights) {
                                    tracing::debug!("Coupling weight save failed: {e}");
                                }
                            }
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
                    feed_emergence_observations(&state, tick, &mut r_history, &mut k_history, &mut monitor_tracking);

                    // Decay expired emergence records (prevents unbounded growth)
                    state.ralph.emergence().tick_decay_at(tick);

                    // Feed detected emergence events into correlation engine for
                    // pathway mining. Only ingests events detected THIS tick to
                    // avoid duplicate processing. Also start monitors for high-value
                    // emergence types (G5 fix: active_monitors was always 0).
                    {
                        let recent = state.ralph.emergence().recent(5);
                        for record in &recent {
                            if record.detected_at_tick == tick {
                                state.ralph.correlation().ingest_emergence(
                                    record.emergence_type,
                                    record.confidence,
                                    tick,
                                );
                                // G5: Start monitors for sustained tracking of
                                // metabolically significant emergence types.
                                match record.emergence_type {
                                    EmergenceType::HebbianSaturation
                                    | EmergenceType::ThermalSpike
                                    | EmergenceType::CouplingRunaway => {
                                        state.ralph.emergence().start_monitor(
                                            record.emergence_type,
                                            tick,
                                        );
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Gen-060a: Relay new emergence events to RM for cross-session persistence.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    relay_emergence_to_rm(&state, tick);

                    // Session 071 #7: Relay emergence hints to ME evolution chamber.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    relay_emergence_to_me(&state, tick);

                    // METABOLIC-GAP-1 fix: Post field state to SYNTHEX /api/ingest
                    // every 6 ticks to feed heat sources (HS-001 r, HS-003 fitness,
                    // HS-004 sphere count). This activates SYNTHEX thermal regulation.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if tick % 6 == 0 {
                        post_field_to_synthex(&state, tick);
                    }

                    // METABOLIC-GAP-2 fix: Post field observations to VMS every 30 ticks
                    // (~2.5 min) to feed memory field and enable pattern detection.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if tick % 30 == 0 && tick > 0 {
                        post_state_to_vms(&state, tick);
                    }

                    // IGNITION-1c: Trigger VMS consolidation every 300 ticks (~25 min).
                    // Consolidation prunes stale memories and crystallizes stable patterns,
                    // enabling the Level 1→2 memory promotion pipeline.
                    #[cfg(feature = "bridges")]
                    if tick % 300 == 0 && tick > 0 {
                        trigger_vms_consolidation(&state, tick);
                    }

                    // Poll SYNTHEX thermal for k_adjustment
                    // feedback. The thermal controller modulates coupling strength
                    // based on system temperature (cold → boost, hot → reduce).
                    // BUG-GEN05 fix: Guard with breaker check — skip poll when
                    // SYNTHEX breaker is Open to avoid wasted requests.
                    #[cfg(feature = "bridges")]
                    if state.synthex_bridge.should_poll(tick)
                        && state.breaker_allows("synthex")
                    {
                        if let Ok(k_adj) = state.synthex_bridge.poll_thermal() {
                            state.coupling.write().k_modulation = k_adj;
                            state.synthex_bridge.set_last_poll_tick(tick);
                            #[cfg(feature = "intelligence")]
                            state.breaker_success("synthex");
                        } else {
                            state.synthex_bridge.record_failure();
                            #[cfg(feature = "intelligence")]
                            state.breaker_failure("synthex");
                        }
                    }

                    // METABOLIC-GAP-5 fix: Poll ME observer for fitness signal.
                    // Feeds ME fitness into ORAC tensor (D2 task_throughput proxy) and
                    // detects BUG-008 frozen fitness condition. Every 12 ticks (~1min).
                    #[cfg(feature = "bridges")]
                    if tick % 12 == 0 && state.breaker_allows("me") {
                        match state.me_bridge.poll_observer() {
                            Ok(me_adj) => {
                                state.me_bridge.set_last_poll_tick(tick);
                                // BUG-059e fix: Record ME breaker success. Previously
                                // me_bridge.poll_observer() tracked bridge-internal
                                // state but never updated the circuit breaker registry,
                                // leaving ME breaker at 0/0 permanently.
                                #[cfg(feature = "intelligence")]
                                state.breaker_success("me");
                                if tick % 60 == 0 {
                                    tracing::info!(
                                        fitness = format!("{:.4}", state.me_bridge.last_fitness()),
                                        frozen = state.me_bridge.is_frozen(),
                                        adjustment = format!("{me_adj:.4}"),
                                        "ME observer polled"
                                    );
                                }
                            }
                            Err(e) => {
                                state.me_bridge.record_failure();
                                #[cfg(feature = "intelligence")]
                                state.breaker_failure("me");
                                if tick % 60 == 0 {
                                    tracing::warn!("ME observer poll failed: {e}");
                                }
                            }
                        }
                    }

                    // METABOLIC-GAP-6 fix: Persist RALPH state to Reasoning Memory
                    // every 60 ticks as TSV for cross-session persistence.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if tick % 60 == 0 && tick > 0 {
                        post_state_to_rm(&state, tick);
                    }

                    // Blackboard hygiene: prune stale pane entries every 60 ticks (~5min).
                    // Keeps fleet_size accurate and prevents unbounded growth from
                    // ghost sessions that never deregistered (BUG-059b).
                    #[cfg(feature = "persistence")]
                    if tick % 60 == 0 && tick > 0 {
                        if let Some(bb) = state.blackboard() {
                            // 15-minute staleness cutoff
                            let cutoff = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(0.0, |d| d.as_secs_f64()) - 900.0;
                            match bb.prune_stale_panes(cutoff) {
                                Ok(0) => {}
                                Ok(n) => tracing::info!(pruned = n, "Blackboard: pruned stale panes"),
                                Err(e) => tracing::debug!("Blackboard prune error: {e}"),
                            }
                            // BUG-064q+r fix: Prune unbounded tables every 60 ticks.
                            // hebbian_summary grows ~14,400 rows/day without pruning.
                            // consent_audit grows ~2,700 rows/day without pruning.
                            if let Err(e) = bb.prune_hebbian_summaries(1000) {
                                tracing::debug!("hebbian_summary prune error: {e}");
                            }
                            if let Err(e) = bb.prune_consent_audit(500) {
                                tracing::debug!("consent_audit prune error: {e}");
                            }

                            // BUG-064m fix: Prune zombie sessions from in-memory map.
                            // Sessions from crashed Claude instances (0 tool calls, >1 hour)
                            // accumulate indefinitely. Remove stale sessions every 60 ticks.
                            {
                                let now_ms = orac_sidecar::m3_hooks::m10_hook_server::epoch_ms();
                                let one_hour_ms = 3_600_000;
                                let mut sessions = state.sessions.write();
                                let stale_ids: Vec<String> = sessions
                                    .iter()
                                    .filter(|(_, t)| {
                                        t.total_tool_calls == 0
                                            && now_ms.saturating_sub(t.started_ms) > one_hour_ms
                                    })
                                    .map(|(id, _)| id.clone())
                                    .collect();
                                if !stale_ids.is_empty() {
                                    let count = stale_ids.len();
                                    for id in &stale_ids {
                                        sessions.remove(id);
                                    }
                                    tracing::info!(
                                        pruned = count,
                                        "Zombie sessions pruned (0 tools, >1h old)"
                                    );
                                }
                            }
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
