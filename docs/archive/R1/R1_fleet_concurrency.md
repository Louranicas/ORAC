# R1: ORAC Sidecar Fleet Concurrency Model

> **Codebase:** `orac-sidecar/src/` — 30,524 LOC, 40 modules, 8 layers
> **Lock library:** `parking_lot` 0.12 (non-poisoning, no async awareness)
> **Async runtime:** `tokio` (multi-threaded scheduler)
> **Blocking HTTP:** `ureq` via `spawn_blocking` (never on tokio threads)
> **Date:** 2026-03-25

---

## 1. Shared State (RwLock Fields)

All shared mutable state lives on `OracState` (defined in `m3_hooks/m10_hook_server.rs:431`),
wrapped in `Arc` and passed to every Axum handler via `State<Arc<OracState>>`.

### 1.1 OracState RwLock Fields

| Field | Type | File:Line | Protects | Access Pattern |
|-------|------|-----------|----------|----------------|
| `field_state` | `Arc<RwLock<AppState>>` | m10:435, field_state.rs:389 | Cached PV2 Kuramoto field (r, K, spheres, tick) | Write: field poller (5s), RALPH conductor tick. Read: hook handlers, SYNTHEX poster, RALPH fitness, dashboard |
| `sessions` | `RwLock<HashMap<String, SessionTracker>>` | m10:445 | Per-session hook tracking (tool counts, start time) | Write: SessionStart/Stop hooks. Read: health, blackboard persist |
| `ipc_state` | `RwLock<String>` | m10:449 | IPC bus connection status string | Write: IPC listener task (connect/disconnect/subscribe). Read: health endpoint |
| `ghosts` | `RwLock<VecDeque<OracGhost>>` | m10:451 | Deregistered sphere ghost traces (FIFO, max 20) | Write: `add_ghost()` on Stop hook. Read: `/field/ghosts` endpoint |
| `consents` | `RwLock<HashMap<String, OracConsent>>` | m10:453 | Per-sphere consent declarations (FIX-018) | Write: PUT `/consent/{id}`. Read: every bridge call site via `consent_allows()` |
| `coupling` | `RwLock<CouplingNetwork>` | m10:461 | Hebbian coupling weights, connections, k_modulation | Write: STDP pass (every tick), homeostatic normalization (120 ticks), pruning. Read: SYNTHEX poster, health, STDP diagnostic logging |
| `breakers` | `RwLock<BreakerRegistry>` | m10:464 | Circuit breaker FSMs for 5 external services | Write: `breaker_tick()`, `breaker_success()`, `breaker_failure()`. Read: `breaker_allows()`, `breaker_state_counts()` |

### 1.2 Subsystem-Internal RwLock Fields

Each subsystem struct uses its own `parking_lot::RwLock` for interior mutability. These are
accessed only through `&self` methods on their parent struct (which lives on `OracState`).

#### L8 Evolution — `RalphEngine` (m36_ralph_engine.rs:287)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `phase` | `RwLock<RalphPhase>` | 287 | Current RALPH phase (Recognize/Analyze/Learn/Propose/Harvest) |
| `generation` | `RwLock<u64>` | 289 | Generation counter |
| `completed_cycles` | `RwLock<u64>` | 291 | Total completed 5-phase cycles |
| `paused` | `RwLock<bool>` | 293 | Pause flag |
| `active_mutation` | `RwLock<Option<ActiveMutation>>` | 295 | Currently proposed mutation |
| `mutation_history` | `RwLock<VecDeque<MutationRecord>>` | 297 | Historical mutation records |
| `snapshots` | `RwLock<VecDeque<StateSnapshot>>` | 299 | State snapshots for rollback |
| `stats` | `RwLock<RalphStats>` | 311 | Accumulated statistics |

#### L8 Evolution — `EmergenceDetector` (m37_emergence_detector.rs:306)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `history` | `RwLock<VecDeque<EmergenceRecord>>` | 306 | Emergence event ring buffer |
| `monitors` | `RwLock<HashMap<u64, EmergenceMonitor>>` | 308 | Active emergence monitors |
| `next_record_id` | `RwLock<u64>` | 310 | ID sequence for records |
| `next_monitor_id` | `RwLock<u64>` | 312 | ID sequence for monitors |
| `stats` | `RwLock<EmergenceStats>` | 316 | Accumulated statistics |

#### L8 Evolution — `CorrelationEngine` (m38_correlation_engine.rs:193)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `events` | `RwLock<VecDeque<CorrelationEvent>>` | 193 | Event ring buffer |
| `correlations` | `RwLock<VecDeque<Correlation>>` | 195 | Discovered correlations |
| `pathways` | `RwLock<HashMap<String, Pathway>>` | 197 | Established pathways |
| `next_event_id` | `RwLock<u64>` | 199 | ID sequence for events |
| `next_correlation_id` | `RwLock<u64>` | 201 | ID sequence for correlations |
| `stats` | `RwLock<CorrelationStats>` | 205 | Accumulated statistics |

#### L8 Evolution — `FitnessTensor` (m39_fitness_tensor.rs:442)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `history` | `RwLock<VecDeque<FitnessSnapshot>>` | 442 | Fitness snapshot ring buffer |
| `stats` | `RwLock<FitnessTensorStats>` | 448 | Accumulated statistics |

#### L8 Evolution — `MutationSelector` (m40_mutation_selector.rs:248)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `parameters` | `RwLock<Vec<MutableParameter>>` | 248 | Registered mutable parameters |
| `round_robin_idx` | `RwLock<usize>` | 250 | Round-robin selection index |
| `last_selected` | `RwLock<HashMap<String, u64>>` | 252 | Per-parameter last selection tick |
| `selection_history` | `RwLock<VecDeque<SelectionRecord>>` | 254 | Selection history |
| `stats` | `RwLock<MutationSelectorStats>` | 258 | Accumulated statistics |

#### L2 Wire — `WireProtocol` (m09_wire_protocol.rs:170)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `state` | `RwLock<ProtocolState>` | 170 | FSM state (Disconnected→Handshaking→Connected→Subscribing→Active) |
| `session_id` | `RwLock<Option<String>>` | 174 | Current session ID |
| `subscriptions` | `RwLock<Vec<String>>` | 176 | Active topic subscriptions |
| `send_queue` | `RwLock<VecDeque<String>>` | 178 | Outbound message queue |
| `recv_buffer` | `RwLock<VecDeque<BusFrame>>` | 180 | Inbound frame buffer |
| `last_activity` | `RwLock<f64>` | 184 | Timestamp of last activity |
| `stats` | `RwLock<WireStats>` | 186 | Protocol statistics |

#### L6 Coordination — `WasmBridge` (m30_wasm_bridge.rs:301)

| Field | Type | Line | Protects |
|-------|------|------|----------|
| `ring` | `RwLock<EventRingBuffer>` | 301 | Sidecar→WASM event ring (1000 line cap) |
| `command_queue` | `RwLock<VecDeque<WasmCommand>>` | 303 | WASM→sidecar command queue |
| `stats` | `RwLock<WasmBridgeStats>` | 305 | Bridge statistics |

#### L5 Bridges (each has one `RwLock<BridgeState>`)

| Bridge | File:Line | Protects |
|--------|-----------|----------|
| `SynthexBridge` | m22_synthex_bridge.rs:153 | Thermal state, poll stats, last response |
| `MeBridge` | m23_me_bridge.rs:245 | ME fitness, observer data, poll stats |
| `PovmBridge` | m24_povm_bridge.rs:168 | POVM memory/pathway counts, poll stats |
| `RmBridge` | m25_rm_bridge.rs:223 | RM search results, persist stats |

#### L7 Monitoring

| Struct | File:Line | Field | Protects |
|--------|-----------|-------|----------|
| `TraceStore` | m32_otel_traces.rs:574 | `state: RwLock<TraceStoreState>` | In-process OTel span storage |
| `MetricsExport` (Gauge) | m33_metrics_export.rs:167 | `values: RwLock<BTreeMap<Labels, f64>>` | Prometheus gauge values |
| `MetricsExport` (Counter) | m33_metrics_export.rs:226 | `values: RwLock<BTreeMap<Labels, f64>>` | Prometheus counter values |
| `MetricsExport` (Histogram) | m33_metrics_export.rs:300 | `states: RwLock<BTreeMap<Labels, HistogramState>>` | Prometheus histogram buckets |
| `FieldDashboard` | m34_field_dashboard.rs:137 | `state: RwLock<DashboardState>` | Kuramoto dashboard snapshots |
| `TokenAccountant` | m35_token_accounting.rs:228 | `state: RwLock<AccountantState>` | Token usage accounting |

---

## 2. Mutexes

Only one `Mutex` exists in the entire codebase.

| Field | Type | File:Line | Protects | Lock Scope |
|-------|------|-----------|----------|------------|
| `blackboard` | `Option<Mutex<Blackboard>>` | m10:456 | SQLite database connection for persistent fleet state | Acquired via `state.blackboard()` which returns `Option<MutexGuard>`. Held for individual SQL operations (upsert/query/insert). Released immediately after each DB call. |

**Why Mutex (not RwLock):** `rusqlite::Connection` is `!Sync` — SQLite WAL mode allows concurrent
reads at the filesystem level, but the Rust binding requires exclusive access. The `Mutex` serializes
all blackboard operations (reads and writes) through a single connection handle.

**Accessor pattern** (m10:654):
```rust
pub fn blackboard(&self) -> Option<MutexGuard<'_, Blackboard>> {
    self.blackboard.as_ref().map(Mutex::lock)
}
```

---

## 3. Atomic Counters

All atomics are on `OracState` and use `Ordering::Relaxed` (sufficient for monotonic counters
where total ordering is not required).

| Field | Type | File:Line | Purpose | Updated By |
|-------|------|-----------|---------|------------|
| `tick` | `AtomicU64` | m10:447 | Global uptime tick counter | `increment_tick()` in RALPH loop (every 5s) |
| `dispatch_total` | `AtomicU64` | m10:466 | Total semantic routing dispatches | `record_dispatch()` in PostToolUse handler |
| `dispatch_read` | `AtomicU64` | m10:468 | Dispatch count for Read domain | `record_dispatch()` |
| `dispatch_write` | `AtomicU64` | m10:470 | Dispatch count for Write domain | `record_dispatch()` |
| `dispatch_execute` | `AtomicU64` | m10:472 | Dispatch count for Execute domain | `record_dispatch()` |
| `dispatch_communicate` | `AtomicU64` | m10:474 | Dispatch count for Communicate domain | `record_dispatch()` |
| `co_activations_total` | `AtomicU64` | m10:476 | Total co-activation events detected | STDP pass in RALPH loop |
| `hebbian_ltp_total` | `AtomicU64` | m10:478 | Accumulated Hebbian LTP events | STDP pass in RALPH loop |
| `hebbian_ltd_total` | `AtomicU64` | m10:480 | Accumulated Hebbian LTD events | STDP pass in RALPH loop |
| `hebbian_last_tick` | `AtomicU64` | m10:482 | Last tick when STDP ran | STDP pass in RALPH loop |
| `total_tool_calls` | `AtomicU64` | m10:490 | Global tool call counter | PostToolUse handler |
| `tool_calls_at_last_thermal` | `AtomicU64` | m10:492 | Snapshot at last SYNTHEX thermal post | `post_field_to_synthex()` |

**Additionally:** One function-scoped `static AtomicBool`:

| Name | File:Line | Purpose |
|------|-----------|---------|
| `FIRST_POST_DONE` | main.rs:661 | Tracks whether first SYNTHEX POST has completed (triggers PID reset) |

---

## 4. Background Tasks (tokio::spawn)

### 4.1 Long-Running Background Tasks (spawned at startup in main.rs)

| Task | Spawned At | Description | Lifetime | State Access |
|------|-----------|-------------|----------|--------------|
| **Field Poller** | m10:1061, main.rs:57 | Polls PV2 `/health` + `/spheres` every 5s, updates `field_state`, advances breakers, syncs coupling network, updates dashboard | Process lifetime | Writes: `field_state`, `breakers`. Reads: `coupling` |
| **IPC Listener** | main.rs:239→248 | Connects to PV2 Unix socket bus, subscribes to `field.*` + `sphere.*`, reconnects with escalating backoff (5s→30s cap) | Process lifetime | Writes: `ipc_state` |
| **RALPH Loop** | main.rs:1304→1308 | 5s tick interval. Conductor advisory tick, Hebbian STDP pass, homeostatic normalization, emergence detection, bridge polling, RALPH 5-phase evolution | Until shutdown signal (`watch::Receiver<bool>`) | Writes: `field_state` (conductor), `coupling` (STDP, normalization). Reads: `field_state`, `coupling`, `breakers`. Atomics: `tick`, `co_activations_total`, `hebbian_*` |
| **Axum Server** | main.rs:83→104 | HTTP server on port 8133, graceful shutdown on SIGINT | Process lifetime | Reads/Writes via handlers: all `OracState` fields |

### 4.2 Per-Request Spawned Tasks

| Task | Spawned At | Description | Lifetime |
|------|-----------|-------------|----------|
| **fire_and_forget_post** | m10:988 | `tokio::spawn` → `spawn_blocking` → `ureq::post`. Logs errors, does not block caller. | Single HTTP round-trip (2s timeout) |
| **breaker_guarded_post** | m10:1012 | Same as above but checks/updates circuit breaker state before/after the call. | Single HTTP round-trip (2s timeout) |
| **http_get** | m10:1292 | `spawn_blocking` → `ureq::get`. Awaited by caller (not fire-and-forget). | Single HTTP round-trip (configurable timeout) |
| **http_post** | m10:1310 | `spawn_blocking` → `ureq::post`. Awaited by caller (not fire-and-forget). | Single HTTP round-trip (configurable timeout) |
| **RM crystallize (Stop hook)** | m11:202 | `tokio::spawn` → `spawn_blocking` → `ureq::post` to RM. Awaited with 3s timeout cap. | Bounded by 3s timeout |

### 4.3 Test-Only Spawns

| Location | Description |
|----------|-------------|
| m07_ipc_client.rs:689,793,829,877,918 | Mock Unix socket servers for IPC client integration tests |

---

## 5. Fire-and-Forget Calls

The `fire_and_forget_post` function (m10:988) is the primary fire-and-forget pattern.
It spawns a `tokio::spawn` → `spawn_blocking` → `ureq::post` chain that runs independently
of the calling handler.

### 5.1 Call Sites

| Caller | File:Line | Target Service | Purpose |
|--------|-----------|----------------|---------|
| `handle_session_start` | m11:51 | PV2 | Register sphere |
| `handle_session_start` | m11:160 | PV2 | Fail tasks on error |
| `handle_session_start` | m11:170 | PV2 | Update status |
| `handle_session_start` | m11:187 | POVM | Persist session memory |
| `handle_stop` | m11:233 | RM | Crystallize session (non-intelligence build) |
| `handle_stop` | m11:309 | PV2 | Deregister sphere |
| `handle_post_tool_use` | m12:116 | PV2 | Store tool memory |
| `handle_post_tool_use` | m12:128 | PV2 | Update pane status |
| `handle_post_tool_use` | m12:171 | PV2 | Complete task |
| `post_field_to_synthex` | m10:2010 | SYNTHEX | Post thermal heat sources |

### 5.2 Breaker-Guarded Fire-and-Forget

`breaker_guarded_post` (m10:1012) is used when circuit breaker state matters.
It checks `breaker_allows()` before calling, then records success/failure on the breaker
after the response. Used for SYNTHEX thermal posts and bridge communications.

### 5.3 Safety Properties

- **No state locks held:** Fire-and-forget tasks clone all data before spawning.
  The `Arc<OracState>` is cloned, URL and body are owned `String`s.
- **Timeout bounded:** All `ureq` calls have a 2-second timeout.
- **Error visibility:** Errors are logged at `warn` level (upgraded from `debug` by T2 fix).
- **No retry:** Failed calls are logged and dropped. Retry is handled at the caller
  level (e.g., field poller retries on next 5s tick).

---

## 6. Lock Ordering Rules

### 6.1 Canonical Lock Order

The system enforces the following lock acquisition order to prevent deadlocks:

```
1. field_state  (read or write)
2. coupling     (read or write)
3. breakers     (read or write)
4. sessions     (read or write)
5. consents     (read or write)
6. ghosts       (read or write)
7. ipc_state    (read or write)
8. blackboard   (Mutex)
```

**Rule:** A lock with a higher number must NEVER be held while acquiring a lock with a lower number.

### 6.2 Enforced Lock Ordering Patterns

#### Pattern A: field_state → coupling (RALPH STDP pass, main.rs:1349-1379)

```rust
// BUG-060 fix: Clone spheres first (read lock), then acquire coupling write lock.
// Lock ordering: field_state read DROPPED before coupling write.
let spheres = state.field_state.read().spheres.clone();  // read, clone, drop
drop(spheres);  // explicit drop (diagnostic block)
let spheres = state.field_state.read().spheres.clone();  // re-read
let stdp_result = apply_stdp(
    &mut state.coupling.write(),  // coupling write acquired AFTER field_state dropped
    &spheres,
);
```

#### Pattern B: field_state → coupling (SYNTHEX poster, main.rs:664-668)

```rust
let (r, sphere_count) = {
    let fs = state.field_state.read();
    (fs.field.order.r, fs.spheres.len())
};  // field_state read lock dropped here
let k_mod = state.coupling.read().k_modulation;  // coupling read acquired after
```

#### Pattern C: field_state → coupling (Dashboard update, m10:1194-1198)

```rust
let guard = state.field_state.read();
let k_eff = {
    let c = state.coupling.read();  // coupling read while field_state read held
    c.k * c.k_modulation
};  // coupling read dropped, field_state read still held
```

**Note:** Pattern C holds both `field_state` read and `coupling` read simultaneously.
This is safe because both are read locks and `parking_lot::RwLock` allows concurrent reads.
The ordering (field_state before coupling) is consistent with the canonical order.

#### Pattern D: Atomic-only access (no lock needed)

```rust
state.tick.fetch_add(1, Ordering::Relaxed);  // no lock
state.co_activations_total.fetch_add(count, Ordering::Relaxed);  // no lock
```

#### Pattern E: Blackboard Mutex (always acquired alone)

```rust
if let Some(bb) = state.blackboard() {  // Mutex acquired
    bb.upsert_pane_status(...);  // single SQL operation
}  // MutexGuard dropped immediately after use
```

The blackboard Mutex is never held while any RwLock is held.

### 6.3 Critical Anti-Pattern: Coupling → field_state (NEVER)

The system explicitly avoids acquiring `field_state` while holding `coupling`:

```
// WRONG — would violate lock order:
let net = state.coupling.write();       // coupling write held
let fs = state.field_state.read();      // DEADLOCK RISK with Pattern A

// CORRECT — clone first, release, then acquire:
let k_mod = state.coupling.read().k_modulation;  // read, extract, drop
let r = state.field_state.read().field.order.r;   // safe: coupling released
```

---

## 7. Deadlock Prevention

### 7.1 Design Principles

1. **parking_lot (not std):** `parking_lot::RwLock` is non-poisoning and has deadlock detection
   in debug builds (`RUST_BACKTRACE=1`). It also provides `try_read()`/`try_write()` for
   diagnostic use, though ORAC does not currently use them.

2. **Interior mutability via `&self`:** All trait methods take `&self` (rule C2 from CLAUDE.md).
   This means the outer `Arc` is never write-locked — only interior `RwLock` fields are.

3. **Owned returns from RwLock:** Rule C7 requires `.read().get(key).cloned()`, never returning
   `&T` through a lock guard. This ensures lock guards are dropped promptly.

4. **Minimal lock scope:** All lock acquisitions use brace blocks `{ let guard = lock.write(); ... }`
   to ensure guards are dropped before acquiring the next lock.

5. **Clone-before-spawn:** Fire-and-forget tasks clone all needed data (`Arc::clone`, `String` owned)
   before the `tokio::spawn` boundary. No lock guards cross spawn points.

6. **No nested write locks on the same RwLock:** The code never acquires a write lock on
   a field while already holding a read or write lock on the same field.

### 7.2 Blocking HTTP via spawn_blocking

All `ureq` HTTP calls (synchronous, blocking) are wrapped in `tokio::task::spawn_blocking`
to prevent blocking the tokio executor threads:

```rust
tokio::task::spawn_blocking(move || {
    ureq::post(&url)
        .timeout(Duration::from_millis(2000))
        .send_string(&body)
})
```

This is critical because `ureq` performs synchronous DNS resolution and TCP I/O.
Without `spawn_blocking`, a slow HTTP call would block the entire tokio worker thread,
starving other tasks.

### 7.3 Shutdown Coordination

The RALPH loop uses `tokio::sync::watch::channel` for graceful shutdown:

```rust
let (halt_send, halt_recv) = tokio::sync::watch::channel(false);

// RALPH loop uses tokio::select! to check shutdown
tokio::select! {
    _ = interval.tick() => { /* evolution tick */ }
    _ = shutdown.changed() => { break; }  // clean exit
}

// Axum shutdown handler sends the signal
let _ = halt_send.send(true);
```

This ensures RALPH completes its current tick before stopping — no locks are held at the
shutdown boundary.

### 7.4 Potential Concern: Long Lock Hold in RALPH Tick

The RALPH loop (main.rs:1332-1347) holds `field_state.write()` for the duration of
`tick_once()` (conductor advisory computation). If `tick_once` is slow, this blocks
all readers (hook handlers, field poller, dashboard). In practice, `tick_once` is pure
computation on cached state with no I/O, so the hold time is microseconds.

### 7.5 Summary: Why ORAC Doesn't Deadlock

| Risk | Mitigation |
|------|-----------|
| Lock inversion (A→B then B→A) | Canonical ordering enforced; BUG-060 comment documents it |
| Lock held across await points | `parking_lot` locks are `!Send` — compiler prevents this |
| Lock held across spawn | Clone-before-spawn pattern; no guards cross spawn boundaries |
| Blocking tokio threads | All `ureq` calls in `spawn_blocking` |
| Shutdown races | `watch::channel` + `tokio::select!` for clean RALPH exit |
| Mutex + RwLock nesting | Blackboard Mutex always acquired alone, never while holding RwLock |
| Self-deadlock (re-entrant write) | `parking_lot::RwLock` is not re-entrant; code never re-acquires same lock |
