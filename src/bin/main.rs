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
use orac_sidecar::m4_intelligence::m18_hebbian_stdp::{apply_stdp_with_ltp, decay_active_weights};

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

        // Spawn blackboard GC on a separate interval (decoupled from RALPH tick)
        #[cfg(feature = "persistence")]
        spawn_blackboard_gc(Arc::clone(&state));

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
#[allow(clippy::too_many_lines)]
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
                // Restore 4 AtomicU64 runtime params (Session 079 convergence trap fix)
                // MUST also update selector so apply_ralph_mutations() propagates
                // restored values instead of overwriting with defaults on tick 1.
                let selector = state.ralph.selector();
                if saved.k_mod_bits != 0 {
                    state.ralph_k_mod.store(saved.k_mod_bits, std::sync::atomic::Ordering::Relaxed);
                    let _ = selector.update_value("k_mod", f64::from_bits(saved.k_mod_bits));
                }
                if saved.hebbian_ltp_bits != 0 {
                    state.ralph_hebbian_ltp.store(saved.hebbian_ltp_bits, std::sync::atomic::Ordering::Relaxed);
                    let _ = selector.update_value("hebbian_ltp", f64::from_bits(saved.hebbian_ltp_bits));
                }
                if saved.decay_rate_bits != 0 {
                    state.ralph_decay_rate.store(saved.decay_rate_bits, std::sync::atomic::Ordering::Relaxed);
                    let _ = selector.update_value("decay_rate", f64::from_bits(saved.decay_rate_bits));
                }
                if saved.tick_interval_ms != 0 {
                    state.ralph_tick_interval_ms.store(saved.tick_interval_ms, std::sync::atomic::Ordering::Relaxed);
                    #[allow(clippy::cast_precision_loss)]
                    let _ = selector.update_value("tick_interval", saved.tick_interval_ms as f64 / 1000.0);
                }
                tracing::info!(
                    gen = saved.generation,
                    fitness = format!("{:.4}", saved.current_fitness),
                    phase = saved.last_phase,
                    k_mod = format!("{:.4}", f64::from_bits(saved.k_mod_bits)),
                    ltp = format!("{:.4}", f64::from_bits(saved.hebbian_ltp_bits)),
                    decay = format!("{:.6}", f64::from_bits(saved.decay_rate_bits)),
                    tick_ms = saved.tick_interval_ms,
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

    // 2b. Register saved sessions as PV2 spheres for coupling network population.
    // Session 081: Without this, step 3 coupling weight hydration finds 0 connections
    // because PV2 has no spheres at startup. This closes the coupling amnesia gap.
    #[cfg(feature = "persistence")]
    {
        let sessions = state.sessions.read();
        let mut registered = 0u32;
        for (_sid, tracker) in sessions.iter() {
            if tracker.persona.is_empty() {
                continue;
            }
            let _url = format!("{}/sphere/{}/register", state.pv2_url, tracker.pane_id.as_str());
            let body = serde_json::json!({
                "persona": tracker.persona,
                "frequency": 0.1,
            })
            .to_string();
            // Use raw_http_post (already imported) instead of fire_and_forget_post
            // (which is in the hook server module and requires async context).
            if orac_sidecar::m5_bridges::http_helpers::raw_http_post(
                &state.pv2_url.replace("http://", ""),
                &format!("/sphere/{}/register", tracker.pane_id.as_str()),
                body.as_bytes(),
                "pv2",
            ).is_ok() {
                registered += 1;
            }
        }
        drop(sessions);
        if registered > 0 {
            // Brief pause to let PV2 process registrations before weight hydration
            std::thread::sleep(std::time::Duration::from_millis(500));
            tracing::info!(registered, "Saved sessions registered as PV2 spheres for coupling hydration");
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
/// Background blackboard GC — runs every 5 minutes on its own `tokio` task.
///
/// Decoupled from the RALPH tick loop to avoid holding the blackboard
/// `Mutex` during latency-sensitive evolution ticks. Prunes:
/// - stale panes (>15 min since last update)
/// - `hebbian_summary` (keep newest 1,000)
/// - `consent_audit` (keep newest 500)
#[cfg(feature = "persistence")]
fn spawn_blackboard_gc(state: Arc<OracState>) {
    tokio::spawn(async move {
        // Offset by 30s so we don't collide with startup hydration
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        tracing::info!("Blackboard GC task started (5-minute interval)");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await; // skip immediate first tick

        loop {
            interval.tick().await;

            let start = std::time::Instant::now();
            if let Some(bb) = state.blackboard() {
                // 15-minute staleness cutoff for panes
                let cutoff = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0.0, |d| d.as_secs_f64())
                    - 900.0;
                let panes_pruned = bb.prune_stale_panes(cutoff).unwrap_or(0);
                let hebbian_pruned = bb.prune_hebbian_summaries(1000).unwrap_or(0);
                let consent_pruned = bb.prune_consent_audit(500).unwrap_or(0);
                let elapsed_ms = start.elapsed().as_millis();

                if panes_pruned + hebbian_pruned + consent_pruned > 0 {
                    tracing::info!(
                        panes_pruned,
                        hebbian_pruned,
                        consent_pruned,
                        elapsed_ms,
                        "Blackboard GC complete"
                    );
                } else {
                    tracing::debug!(elapsed_ms, "Blackboard GC: nothing to prune");
                }
            }
        }
    });
}

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
        // Gen-063a: Use STDP soft ceiling (0.75) not theoretical max (1.0)
        // Session 080 BUG: was 0.85 but m18 ceiling is 0.75 since Session 072
        // so saturation detector fires when weights approach the actual learning bound.
        match detector.detect_hebbian_saturation(&weights, 0.15, 0.75, tick) {
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
    // Session-073 audit: Use 1.5x target as spike threshold. The PID target (0.500)
    // is structurally unreachable (BUG-073-D), so T>target fires 54% of checks.
    // At 1.5x (0.750), only genuine overheating fires. Debounce added in detector.
    #[cfg(feature = "bridges")]
    if tick % 6 == 0 {
        if let Some(resp) = state.synthex_bridge.last_response() {
            let spike_threshold = resp.target * 1.5;
            if let Err(e) = detector.detect_thermal_spike(resp.temperature, spike_threshold, tick) {
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

    // 9. DegenerateMode detection (every 50 ticks — systemic health watchdog)
    // Session 080: Was implemented in m37 but NEVER WIRED. This is the system's
    // last line of defense against metabolic death. Checks 6 stagnation indicators.
    #[cfg(feature = "evolution")]
    if tick % 50 == 0 && tick > 100 {
        let ltp = state.hebbian_ltp_total.load(std::sync::atomic::Ordering::Relaxed);
        let ltd = state.hebbian_ltd_total.load(std::sync::atomic::Ordering::Relaxed);
        #[allow(clippy::cast_precision_loss)] // connection count bounded by SPHERE_CAP^2
        let weight_mean = {
            let net = state.coupling.read();
            if net.connections.is_empty() { 0.0 } else {
                net.connections.iter().map(|c| c.weight).sum::<f64>()
                    / net.connections.len() as f64
            }
        };
        let decision_action = state.field_state.read().prev_decision_action.to_string();
        let snapshot = orac_sidecar::m8_evolution::m37_emergence_detector::DegenerateSnapshot {
            ltp_total: ltp,
            ltd_total: ltd,
            weight_mean,
            r_history: r_slice.to_vec(),
            decision_action,
            last_decision_change_tick: 0, // No tick tracked on OracState; fallback per spec
        };
        match detector.detect_degenerate_mode(&snapshot, tick) {
            Ok(Some(id)) => tracing::warn!(id, "Emergence: DegenerateMode detected — system may be metabolically dead"),
            Err(e) => tracing::debug!("Emergence degenerate_mode check error: {e}"),
            _ => {}
        }
    }
}

/// Post field state to SYNTHEX `/api/ingest` endpoint (METABOLIC-GAP-1 fix).
///
/// Feeds all 4 heat sources with live data:
/// - HS-001 (`r`): Kuramoto order parameter from cached field state
/// - HS-002 (`cascade_heat`): tool call rate since last post (normalized)
/// - HS-003 (`me_fitness`): RALPH current fitness
/// - HS-004 (`cross_sync`): breaker health (1.0 when all closed, 0.0 when all open)
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
            { (sphere_count as f64 / 30.0).min(1.0) }
        }
    };
    #[cfg(not(feature = "intelligence"))]
    #[allow(clippy::cast_precision_loss)]
    let nexus_health = (sphere_count as f64 / 30.0).min(1.0);

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

/// Trigger POVM memory consolidation (Session 073 — `MED-04` fix).
///
/// Sends a POST to POVM `/consolidate` to crystallise mature memories (access >= 5,
/// intensity >= 0.5), decay stale ones by 10%, and prune pathways below threshold.
/// Without periodic triggers, memories accumulate without crystallisation.
#[cfg(feature = "bridges")]
fn trigger_povm_consolidation(state: &OracState, tick: u64) {
    use orac_sidecar::m5_bridges::http_helpers::raw_http_post;

    if !state.breaker_allows("povm") {
        return;
    }

    match raw_http_post("127.0.0.1:8125", "/consolidate", b"{}", "povm") {
        Ok(status) => {
            state.breaker_success("povm");
            tracing::info!(tick, status, "POVM consolidation triggered");
        }
        Err(e) => {
            state.breaker_failure("povm");
            tracing::warn!(tick, "POVM consolidation failed: {e}");
        }
    }
}

/// Hydrate top POVM memories into blackboard `povm_context` table (Session 066).
///
/// Session 076: Bridge RALPH-accepted mutation values to runtime parameters.
///
/// Reads each registered parameter from `RalphEngine.selector().get_parameter()`
/// and writes to the corresponding `AtomicU64` on `OracState`. Runtime consumers
/// (STDP, decay, coupling) read these atomics instead of compile-time constants.
///
/// For `k_mod`, the RALPH value is stored as a baseline that gets multiplied by
/// the SYNTHEX thermal adjustment. This preserves SYNTHEX's real-time modulation
/// while letting RALPH steer the operating point.
#[cfg(feature = "evolution")]
fn apply_ralph_mutations(state: &OracState) {
    let selector = state.ralph.selector();

    if let Some(p) = selector.get_parameter("k_mod") {
        state
            .ralph_k_mod
            .store(p.current_value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    if let Some(p) = selector.get_parameter("hebbian_ltp") {
        state
            .ralph_hebbian_ltp
            .store(p.current_value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    if let Some(p) = selector.get_parameter("decay_rate") {
        state
            .ralph_decay_rate
            .store(p.current_value.to_bits(), std::sync::atomic::Ordering::Relaxed);
    }

    // r_target: Write to AppState governance override so the Conductor uses it.
    if let Some(p) = selector.get_parameter("r_target") {
        state.field_state.write().r_target_override = Some(p.current_value);
    }

    // tick_interval: Store as milliseconds for the evolution loop to read.
    // The loop checks this atomic each tick and resets its tokio::Interval
    // when the value diverges from the current period.
    if let Some(p) = selector.get_parameter("tick_interval") {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let ms = (p.current_value * 1000.0) as u64;
        state
            .ralph_tick_interval_ms
            .store(ms, std::sync::atomic::Ordering::Relaxed);
    }
}

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
    let ralph_state = state.ralph.state();
    let fitness = ralph_state.current_fitness;
    let field_r = state.field_state.read().field.order.r;

    for record in &recent {
        if record.detected_at_tick != tick {
            continue;
        }
        // Session 075 BREAK-2: Post ALL emergence types to ME via bridge.
        // Uses MeBridge::post_emergence() which hits /api/tools/learning-cycle.
        let etype = format!("{:?}", record.emergence_type);
        if let Err(e) = state.me_bridge.post_emergence(&etype, tick, fitness, field_r) {
            tracing::debug!(etype, "ME emergence relay failed: {e}");
        } else {
            tracing::info!(etype, tick, "Emergence relayed to ME");
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

        // Session 076: Track current interval period so we can detect RALPH changes.
        let mut current_interval_ms: u64 = 5000;

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
                        // Session 076: Use RALPH-tuned LTP rate instead of
                        // compile-time HEBBIAN_LTP constant.
                        let ralph_ltp = f64::from_bits(
                            state.ralph_hebbian_ltp.load(std::sync::atomic::Ordering::Relaxed),
                        );
                        let stdp_result = apply_stdp_with_ltp(
                            &mut state.coupling.write(),
                            &spheres,
                            ralph_ltp,
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

                        // Session 073: Selective decay — only decay connections whose
                        // endpoints exist in the current sphere set. Universal decay
                        // (decay_all_weights) caused 80%+ of weights to collapse to
                        // floor because connections between stale sphere IDs receive
                        // decay but never receive LTP. Factor 0.999 is safe with
                        // selective decay: active equilibrium = 0.01/0.001 = 10 → ceiling.
                        {
                            let spheres_for_decay = state.field_state.read().spheres.clone();
                            // Session 076: Use RALPH-tuned decay rate instead of
                            // hardcoded 0.999. RALPH mutates this via "decay_rate" param.
                            let decay = f64::from_bits(
                                state.ralph_decay_rate.load(std::sync::atomic::Ordering::Relaxed),
                            );
                            decay_active_weights(
                                &mut state.coupling.write(),
                                &spheres_for_decay,
                                decay,
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
                            // BUG-064a fix: Use soft ceiling (0.75) and wider epsilon (0.01).
                            // Session 080: was 0.85 but m18 HEBBIAN_SOFT_CEILING is 0.75
                            // Previous code checked weight==1.0 (never true since STDP
                            // caps at 0.85) and floor with 1e-10 epsilon (too tight).
                            let soft_ceiling = 0.75_f64;
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

                    // Session 075 HD-3: Read POVM /hydrate for crystallised memory count.
                    // Crystallised memories are a proxy for mature learning — feed to RALPH
                    // as a positive fitness signal via emergence detection.
                    #[cfg(feature = "bridges")]
                    if tick % 60 == 0 && tick > 0 && state.breaker_allows("povm") {
                        if let Ok(body) = orac_sidecar::m5_bridges::http_helpers::raw_http_get(
                            &state.povm_url, "/hydrate", "povm",
                        ) {
                            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&body) {
                                let crystallised = data.get("crystallised_count")
                                    .and_then(serde_json::Value::as_u64)
                                    .unwrap_or(0);
                                let pathways = data.get("pathway_count")
                                    .and_then(serde_json::Value::as_u64)
                                    .unwrap_or(0);
                                if tick % 300 == 0 {
                                    tracing::info!(
                                        crystallised, pathways,
                                        "POVM hydrate: learning substrate state"
                                    );
                                }
                            }
                        }
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

                    // Session 076: Bridge RALPH mutations to runtime parameters.
                    // RALPH proposes mutations via selector.update_value() into its
                    // internal Vec. This reads accepted values and writes them to the
                    // AtomicU64 fields on OracState that runtime consumers read.
                    #[cfg(feature = "evolution")]
                    apply_ralph_mutations(&state);

                    // Session 072: Push RALPH evolution state to SYNTHEX via Nexus Bus.
                    // Gives SYNTHEX real-time visibility into fitness trajectory and
                    // mutation activity for thermal PID regulation. Every 6 ticks.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if tick % 6 == 0 && state.breaker_allows("synthex") {
                        let rs = state.ralph.state();
                        let event = orac_sidecar::m5_bridges::m22_synthex_bridge::SynthexBridge::make_ralph_event(
                            rs.generation,
                            rs.phase.name(),
                            rs.current_fitness,
                            rs.current_fitness,
                            rs.current_fitness,
                        );
                        if let Err(e) = state.synthex_bridge.nexus_push(&[event]) {
                            tracing::debug!("Nexus RALPH push failed: {e}");
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
                                k_mod_bits: state.ralph_k_mod.load(std::sync::atomic::Ordering::Relaxed),
                                hebbian_ltp_bits: state.ralph_hebbian_ltp.load(std::sync::atomic::Ordering::Relaxed),
                                decay_rate_bits: state.ralph_decay_rate.load(std::sync::atomic::Ordering::Relaxed),
                                tick_interval_ms: state.ralph_tick_interval_ms.load(std::sync::atomic::Ordering::Relaxed),
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

                            // Session 079 Phase 2: Reap stale sessions (>15min, not in PV2 spheres)
                            let fs = state.field_state.read();
                            let live_panes: std::collections::HashSet<_> = fs.spheres.keys().cloned().collect();
                            drop(fs);
                            #[allow(clippy::cast_possible_truncation)]
                            let now_ms = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(0u64, |d| d.as_millis() as u64);
                            let stale_cutoff_ms = now_ms.saturating_sub(900_000); // 15 minutes
                            let mut sessions_w = state.sessions.write();
                            let before = sessions_w.len();
                            sessions_w.retain(|_sid, t| {
                                live_panes.contains(&t.pane_id) || t.started_ms > stale_cutoff_ms
                            });
                            let reaped = before - sessions_w.len();
                            drop(sessions_w);
                            if reaped > 0 {
                                tracing::info!(reaped, remaining = before - reaped, "Stale sessions reaped");
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

                    // Session 078: Persist fitness tensor snapshot every 100 ticks
                    // for post-restart forensics ("why did fitness drop?").
                    #[cfg(all(feature = "persistence", feature = "evolution"))]
                    if tick % 100 == 0 && tick > 0 {
                        if let Some(bb) = state.blackboard() {
                            let rs = state.ralph.state();
                            let dims_json = serde_json::to_string(&tensor.values)
                                .unwrap_or_else(|_| "[]".to_owned());
                            #[allow(clippy::cast_precision_loss)]
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_or(0.0, |d| d.as_secs_f64());
                            let record = orac_sidecar::m5_bridges::m26_blackboard::FitnessSnapshotRecord {
                                tick,
                                generation: rs.generation,
                                overall_score: rs.current_fitness,
                                dimensions_json: dims_json,
                                created_at: now,
                            };
                            if let Err(e) = bb.insert_fitness_snapshot(&record) {
                                tracing::debug!("Fitness snapshot save failed: {e}");
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

                    // Session 072: Push emergence events to SYNTHEX via Nexus Bus.
                    // Cross-service flow: emergence detector → Nexus Bus → SYNTHEX thermal.
                    // Replaces manual curl; events now flow automatically each tick.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if state.breaker_allows("synthex") {
                        let recent = state.ralph.emergence().recent(5);
                        let nexus_events: Vec<_> = recent
                            .iter()
                            .filter(|rec| rec.detected_at_tick == tick)
                            .map(|rec| {
                                orac_sidecar::m5_bridges::m22_synthex_bridge::SynthexBridge::make_emergence_event(
                                    &rec.emergence_type.to_string(),
                                    &format!("{:.2}", rec.confidence),
                                    &format!("tick={tick} type={}", rec.emergence_type),
                                )
                            })
                            .collect();
                        if !nexus_events.is_empty() {
                            if let Err(e) = state.synthex_bridge.nexus_push(&nexus_events) {
                                tracing::debug!("Nexus emergence push failed: {e}");
                            } else {
                                tracing::info!(count = nexus_events.len(), "Nexus emergence events pushed to SYNTHEX");
                            }
                        }
                    }

                    // Gen-060a: Relay new emergence events to RM for cross-session persistence.
                    // Session 073 BUG-073-D fix: Throttle from every tick to every 30 ticks
                    // to prevent RM flooding (was 98.6% ORAC telemetry noise drowning signal).
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if tick % 30 == 0 {
                        relay_emergence_to_rm(&state, tick);
                    }

                    // Session 071 #7: Relay emergence hints to ME evolution chamber.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    relay_emergence_to_me(&state, tick);

                    // Session 078: Broadcast emergence alerts to Atuin KV for
                    // cross-instance visibility. CC instances can read these via
                    // `cc-kv get habitat.alert.latest` without polling ORAC HTTP.
                    // Review fix: Batch all alerts into a single spawn_blocking to
                    // avoid spawning 2 processes per emergence event per tick.
                    {
                        let recent = state.ralph.emergence().recent(3);
                        let alerts: Vec<(String, String)> = recent
                            .iter()
                            .filter(|r| r.detected_at_tick == tick)
                            .map(|r| {
                                let key = format!("habitat.alert.{}", r.emergence_type);
                                let val = format!(
                                    "{}|conf={:.2}|sev={}|tick={}",
                                    r.emergence_type, r.confidence, r.severity_class, tick
                                );
                                (key, val)
                            })
                            .collect();
                        if !alerts.is_empty() {
                            tokio::task::spawn_blocking(move || {
                                let mut latest_val = String::new();
                                for (key, val) in &alerts {
                                    let _ = std::process::Command::new("atuin")
                                        .args(["kv", "set", "--key", key, val])
                                        .stdout(std::process::Stdio::null())
                                        .stderr(std::process::Stdio::null())
                                        .status();
                                    latest_val.clone_from(val);
                                }
                                if !latest_val.is_empty() {
                                    let _ = std::process::Command::new("atuin")
                                        .args(["kv", "set", "--key", "habitat.alert.latest",
                                            &latest_val])
                                        .stdout(std::process::Stdio::null())
                                        .stderr(std::process::Stdio::null())
                                        .status();
                                }
                            });
                        }
                    }

                    // Session 078 Flow 3: Route emergence events to SYNTHEX via
                    // Nexus Push as cc_coordination events. Uses the Nexus Bus
                    // channel (which SYNTHEX expects) instead of /api/ingest
                    // (which expects heat source format). SYNTHEX can detect
                    // cross-session coordination patterns from these events.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if state.breaker_allows("synthex") {
                        let recent = state.ralph.emergence().recent(3);
                        let cc_events: Vec<_> = recent
                            .iter()
                            .filter(|r| r.detected_at_tick == tick)
                            .map(|r| {
                                orac_sidecar::m5_bridges::m22_synthex_bridge::NexusEvent {
                                    event_type: "cc_coordination".to_owned(),
                                    ts: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map_or(0, |d| d.as_secs()),
                                    data: serde_json::json!({
                                        "emergence_type": r.emergence_type.to_string(),
                                        "confidence": r.confidence,
                                        "severity": r.severity,
                                        "severity_class": r.severity_class.to_string(),
                                        "tick": tick,
                                        "fitness_snapshot": r.fitness_snapshot,
                                    }),
                                }
                            })
                            .collect();
                        if !cc_events.is_empty() {
                            if let Err(e) = state.synthex_bridge.nexus_push(&cc_events) {
                                tracing::debug!("Nexus cc_coordination push failed: {e}");
                            }
                        }
                    }

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
                        trigger_povm_consolidation(&state, tick);
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
                            // Session 076: Blend RALPH k_mod baseline with SYNTHEX
                            // thermal adjustment. RALPH steers the operating point,
                            // SYNTHEX modulates around it in real-time.
                            let ralph_k = f64::from_bits(
                                state.ralph_k_mod.load(std::sync::atomic::Ordering::Relaxed),
                            );
                            state.coupling.write().k_modulation = ralph_k * k_adj;
                            state.synthex_bridge.set_last_poll_tick(tick);
                            #[cfg(feature = "intelligence")]
                            state.breaker_success("synthex");

                            // Session 078 Flow 1: Apply thermal feedback to STDP
                            // decay rate. Hot system → faster decay (weights shrink
                            // → coupling reduces → thermal load drops). Cold system
                            // → slower decay (weights persist → coupling strengthens).
                            // k_adj is already (1.0 - deviation*0.2).clamp(0.8, 1.2),
                            // so inverted: hot=0.8 → decay_mult=1.2, cold=1.2 → 0.8.
                            let decay_mult = (2.0 - k_adj).clamp(0.8, 1.2);
                            let base_decay = f64::from_bits(
                                state.ralph_decay_rate.load(std::sync::atomic::Ordering::Relaxed),
                            );
                            let modulated = (base_decay * decay_mult).clamp(0.980, 1.0);
                            state.ralph_decay_rate.store(
                                modulated.to_bits(),
                                std::sync::atomic::Ordering::Relaxed,
                            );
                            if tick % 60 == 0 {
                                tracing::info!(
                                    k_adj = format!("{k_adj:.4}"),
                                    decay_mult = format!("{decay_mult:.4}"),
                                    decay = format!("{modulated:.6}"),
                                    "SYNTHEX thermal → STDP decay modulation"
                                );
                            }
                        } else {
                            state.synthex_bridge.record_failure();
                            #[cfg(feature = "intelligence")]
                            state.breaker_failure("synthex");
                        }
                    }

                    // Session 078 Flow 2: Poll SYNTHEX Nexus Pull for thermal alerts
                    // and diagnostic findings. Bidirectional closure: ORAC sends field
                    // state → SYNTHEX computes PID → SYNTHEX queues alerts → ORAC reads.
                    #[cfg(feature = "bridges")]
                    if state.synthex_bridge.should_nexus_pull(tick)
                        && state.breaker_allows("synthex")
                    {
                        match state.synthex_bridge.nexus_pull() {
                            Ok(events) if !events.is_empty() => {
                                state.synthex_bridge.set_last_nexus_pull_tick(tick);
                                for event in &events {
                                    match event.event_type.as_str() {
                                        "thermal_alert" => {
                                            tracing::warn!(
                                                tick,
                                                data = %event.data,
                                                "SYNTHEX thermal alert received"
                                            );
                                            // Broadcast to Atuin KV for CC visibility
                                            let alert = format!(
                                                "thermal_alert|tick={}|{}",
                                                tick, event.data
                                            );
                                            let _ = std::process::Command::new("atuin")
                                                .args(["kv", "set", "--key",
                                                    "habitat.alert.thermal", &alert])
                                                .stdout(std::process::Stdio::null())
                                                .stderr(std::process::Stdio::null())
                                                .status();
                                        }
                                        "diagnostic_finding" => {
                                            tracing::info!(
                                                tick,
                                                data = %event.data,
                                                "SYNTHEX diagnostic finding"
                                            );
                                        }
                                        _ => {
                                            tracing::debug!(
                                                tick,
                                                event_type = %event.event_type,
                                                "SYNTHEX nexus event (unhandled type)"
                                            );
                                        }
                                    }
                                }
                                #[cfg(feature = "intelligence")]
                                state.breaker_success("synthex");
                            }
                            Ok(_) => {
                                // Empty pull — update tick to prevent re-polling
                                state.synthex_bridge.set_last_nexus_pull_tick(tick);
                            }
                            Err(e) => {
                                if tick % 30 == 0 {
                                    tracing::debug!("SYNTHEX nexus pull failed: {e}");
                                }
                            }
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

                    // Session 075 BREAK-3: Poll ME EventBus stats for learning activity.
                    // Diffs per-channel event counts to detect ME learning/integration events.
                    #[cfg(feature = "bridges")]
                    if tick % 12 == 0 && state.breaker_allows("me") {
                        match state.me_bridge.poll_eventbus_delta() {
                            Ok((learning_delta, integration_delta)) => {
                                if learning_delta > 0 || integration_delta > 0 {
                                    tracing::info!(
                                        learning = learning_delta,
                                        integration = integration_delta,
                                        "ME EventBus activity detected"
                                    );
                                }
                                // Session 081: Feed ME learning events into RALPH correlation.
                                // ME observes 571K+ correlations and detects emergences but
                                // previously nobody consumed them. This closes the gap.
                                #[cfg(feature = "evolution")]
                                if learning_delta > 0 {
                                    let corr = state.ralph.correlation();
                                    #[allow(clippy::cast_precision_loss)]
                                    let relevance = (learning_delta as f64 / 10.0).min(1.0);
                                    corr.ingest(
                                        "me_learning",
                                        "evolution",
                                        relevance,
                                        tick,
                                        Some(&format!("ME learning: {learning_delta} new events")),
                                    );
                                    tracing::info!(
                                        learning_delta,
                                        "ME learning events fed to RALPH correlation engine"
                                    );
                                }
                                #[cfg(feature = "evolution")]
                                if integration_delta > 0 {
                                    let corr = state.ralph.correlation();
                                    #[allow(clippy::cast_precision_loss)]
                                    let relevance = (integration_delta as f64 / 100.0).min(1.0);
                                    corr.ingest(
                                        "me_integration",
                                        "field_state",
                                        relevance,
                                        tick,
                                        Some(&format!("ME integration: {integration_delta} new events")),
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::debug!("ME EventBus stats poll failed: {e}");
                            }
                        }
                    }

                    // METABOLIC-GAP-6 fix: Persist RALPH state to Reasoning Memory
                    // every 60 ticks as TSV for cross-session persistence.
                    #[cfg(all(feature = "bridges", feature = "evolution"))]
                    if tick % 60 == 0 && tick > 0 {
                        post_state_to_rm(&state, tick);
                    }

                    // Blackboard GC moved to spawn_blackboard_gc() background task
                    // to avoid holding the Mutex in the RALPH tick loop.
                    // Zombie session pruning stays here (in-memory only, no DB lock).
                    #[cfg(feature = "persistence")]
                    if tick % 60 == 0 && tick > 0 {
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

                    // Session 076: Dynamically adjust tick interval if RALPH mutated it.
                    // Read RALPH's proposed tick_interval (stored as ms in AtomicU64)
                    // and reset the tokio::Interval if it differs from current.
                    let new_ms = state.ralph_tick_interval_ms.load(
                        std::sync::atomic::Ordering::Relaxed,
                    );
                    if new_ms != current_interval_ms && (1000..=30_000).contains(&new_ms) {
                        interval = tokio::time::interval(
                            std::time::Duration::from_millis(new_ms),
                        );
                        interval.set_missed_tick_behavior(
                            tokio::time::MissedTickBehavior::Skip,
                        );
                        tracing::info!(
                            old_ms = current_interval_ms,
                            new_ms,
                            "RALPH tick interval adjusted",
                        );
                        current_interval_ms = new_ms;
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
