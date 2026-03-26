# D5: Code Flow Maps

> ORAC Sidecar v0.6.0 | 40 modules, 8 layers, 4 binaries
> Generated from verified source: `src/bin/main.rs` (1780 lines), 42 module files

---

## 1. Hook Lifecycle (All 6 Events)

All hooks enter through `POST /hooks/{event}` on the Axum router (m10) and dispatch
to dedicated handler functions in m11-m14. Each handler receives `Arc<OracState>` via
Axum state extraction and a JSON `HookEvent` body.

### 1.1 SessionStart

```
Entry:    POST /hooks/session_start
Handler:  m11_session_hooks::handle_session_start()
Flow:
  1. Parse HookEvent { session_id, pane_id?, persona? }
  2. Generate PaneId via generate_pane_id() if not provided
  3. OracState.register_session(session_id, pane_id)
     WRITES: sessions (RwLock<HashMap<String, SessionTracker>>)
  4. Register sphere in coupling network
     WRITES: coupling (RwLock<CouplingNetwork>) — calls register(pane_id, phase, freq)
  5. Hydrate from POVM bridge [BLOCKING SYNC]
     READS: config.bridges (POVM addr)
     CALLS: PovmBridge::hydrate_summary() → HTTP GET 127.0.0.1:8125/health
  6. Hydrate from RM bridge [BLOCKING SYNC]
     CALLS: RmBridge::search("session") → HTTP GET 127.0.0.1:8130/search?q=session
  7. Insert pane record in blackboard
     WRITES: blackboard (Mutex<Blackboard>) — upsert_pane()
  8. Record OTel trace span
     WRITES: trace_store (TraceStore)
Response: HookResponse { systemMessage: "ORAC: session registered..." }
```

### 1.2 UserPromptSubmit

```
Entry:    POST /hooks/user_prompt_submit
Handler:  m13_prompt_hooks::handle_user_prompt_submit()
Flow:
  1. Parse HookEvent { prompt }
  2. Read cached field state
     READS: field_state (RwLock<AppState>) — field.order.r, spheres.len(), field.tick
  3. Read SYNTHEX thermal
     READS: synthex_bridge.last_response() — temperature, target
  4. Read RALPH state
     READS: ralph.state() — generation, current_fitness, phase
  5. Read coupling network
     READS: coupling (RwLock<CouplingNetwork>) — k_modulation, connections.len()
  6. Read pending tasks from blackboard
     READS: blackboard — recent_tasks()
  7. Read circuit breaker states
     READS: breaker_registry (RwLock<BreakerRegistry>) — state_counts()
  8. Compose systemMessage with field state injection
Response: HookResponse { systemMessage: "[ORAC r=0.92 K=1.21 spheres=44 ...]" }
```

### 1.3 PreToolUse

```
Entry:    POST /hooks/pre_tool_use
Handler:  m12_tool_hooks::handle_pre_tool_use()
Flow:
  1. Parse HookEvent { tool_name, tool_input }
  2. SYNTHEX thermal gate check
     READS: synthex_bridge.last_response() — temperature
     RULE: If temperature > 0.95, block the tool call (thermal overload)
  3. Permission policy check
     READS: OracState.permission_policy
     CALLS: PermissionPolicy::evaluate(tool_name)
Response: HookResponse::empty() (allow) or HookResponse::block(reason)
```

### 1.4 PostToolUse

```
Entry:    POST /hooks/post_tool_use
Handler:  m12_tool_hooks::handle_post_tool_use()
Flow:
  1. Parse HookEvent { tool_name, tool_input, tool_output, session_id }
  2. Increment total_tool_calls (AtomicU64)
     WRITES: total_tool_calls
  3. Record token usage (chars/4 estimate)
     WRITES: token_accountant (TokenAccountant) — record_pane_usage()
  4. Classify tool semantics
     CALLS: m20_semantic_router::classify_tool(tool_name) → SemanticDomain
     CALLS: m20_semantic_router::classify_content(tool_output) → SemanticDomain
  5. Semantic route for dispatch (if task polling enabled)
     CALLS: m20_semantic_router::route() — composite scoring
     READS: coupling (CouplingNetwork), field_state (spheres)
  6. Record dispatch domain
     WRITES: dispatch_total (AtomicU64), per-domain counters
  7. Update blackboard pane status
     WRITES: blackboard.upsert_pane() — tool_name, status, updated_at
  8. Fire-and-forget bridge posts [ASYNC non-blocking]
     CALLS: fire_and_forget_post() or breaker_guarded_post()
       → PV2 /bus/events (tool event)
       → POVM /memories (if significant tool)
  9. Task poll (1-in-5 calls)
     READS: sessions — poll_counter
     Polls PV2 /bus/tasks for pending tasks, attempts atomic claim
Response: HookResponse::empty()
```

### 1.5 Stop

```
Entry:    POST /hooks/stop
Handler:  m11_session_hooks::handle_stop()
Flow:
  1. Parse HookEvent { session_id }
  2. Remove session from tracker
     WRITES: sessions — remove(session_id), returns SessionTracker
  3. Capture ghost record
     WRITES: ghosts (RwLock<VecDeque<OracGhost>>) — timing, phase, tool_count
  4. Deregister sphere from coupling network
     WRITES: coupling (RwLock<CouplingNetwork>) — deregister(pane_id)
  5. Remove from blackboard
     WRITES: blackboard — remove_pane(), insert_ghost()
  6. Record OTel span
     WRITES: trace_store
Response: HookResponse::empty()
```

### 1.6 PermissionRequest

```
Entry:    POST /hooks/permission_request
Handler:  m14_permission_policy::handle_permission_request()
Flow:
  1. Parse HookEvent { tool_name, tool_input }
  2. Evaluate permission policy
     READS: OracState.permission_policy
     CALLS: PermissionPolicy::evaluate(tool_name) → Decision (Allow/Deny/Notice)
  3. Check per-sphere consent (if sphere_id available)
     READS: consents (RwLock<HashMap<String, OracConsent>>)
Response: HookResponse { decision: "approve"/"deny", reason: "..." }
```

---

## 2. RALPH Evolution Cycle

RALPH (Recognize-Analyze-Learn-Propose-Harvest) is a 5-phase evolutionary optimization
engine. Each tick advances one phase step. The full cycle takes 5 ticks (25 seconds at
default interval).

### Phase Progression

```
Phase 0: Recognize  → Observe current fitness, detect trends
Phase 1: Analyze    → Evaluate correlations, compute system state
Phase 2: Learn      → Feed STDP results, update correlation pathways
Phase 3: Propose    → Select mutation candidate, take state snapshot
Phase 4: Harvest    → Evaluate proposal, accept or rollback

Generation increments after each complete Harvest.
```

### Per-Phase Data Sources

| Phase | Data Source | What It Reads |
|-------|-----------|---------------|
| Recognize | `TensorValues` (12D) | All 12 fitness dimensions from live state |
| Recognize | `EmergenceDetector` | Recent emergence events for context |
| Recognize | VMS query (every 30 ticks) | Semantic memories from VMS `/mcp/tools/call` |
| Analyze | `FitnessTensor::evaluate()` | Weighted fitness score, trend, volatility |
| Analyze | `CorrelationEngine` | Temporal/causal correlations, pathway strengths |
| Learn | `StdpResult` | LTP/LTD counts, weight change delta |
| Learn | `CorrelationEngine::ingest_*()` | Emergence, mutation, fitness change events |
| Propose | `MutationSelector::select()` | Round-robin parameter, diversity gate |
| Propose | `StateSnapshot` | Pre-mutation parameter values for rollback |
| Harvest | `FitnessTensor::evaluate()` | Post-mutation fitness comparison |

### Fitness Tensor Dimensions (12D)

| Dim | Name | Weight | Source |
|-----|------|--------|--------|
| D0 | `coordination_quality` | 0.10 | `session_count / 9` |
| D1 | `field_coherence` | 0.15 | PV2 cached `r` |
| D2 | `dispatch_accuracy` | 0.08 | `total_tool_calls / tick` |
| D3 | `task_throughput` | 0.12 | ME `last_fitness()` |
| D4 | `error_rate` | 0.10 | Breaker `closed / total` |
| D5 | `latency` | 0.08 | SYNTHEX thermal convergence |
| D6 | `hebbian_health` | 0.08 | Weight mean + variance - collapse penalty |
| D7 | `coupling_stability` | 0.07 | Breaker closed fraction |
| D8 | `thermal_balance` | 0.07 | `abs(temp - target)` distance |
| D9 | `fleet_utilization` | 0.05 | Working spheres / total |
| D10 | `emergence_rate` | 0.05 | `total_detected / 20` normalized |
| D11 | `consent_compliance` | 0.05 | From `collect_tensor()` |

### Accept/Rollback Criteria

```
In RalphEngine::tick() during Harvest phase:
  1. Evaluate post-mutation fitness via FitnessTensor::evaluate()
  2. Compare: delta = new_fitness - pre_snapshot_fitness
  3. Accept if:
     - delta > 0.0 (strict improvement), OR
     - fitness trend is Rising
  4. Rollback if:
     - delta <= 0.0 AND trend is not Rising
     - Restore parameters from StateSnapshot
  5. Increment generation counter regardless
  6. Record MutationRecord with status (Accepted/RolledBack)
```

### Mutation Selection (BUG-035 Fix)

```
MutationSelector::select(generation):
  1. Round-robin index into parameter pool (no favorites)
  2. Cooldown check: parameter must not have been mutated in last 10 generations
  3. Diversity gate: reject if >50% of last 20 mutations hit same parameter
  4. Compute mutation delta: scaled random perturbation within parameter bounds
  5. Return MutationProposal { parameter_name, old_value, new_value, delta }
```

---

## 3. Tick Orchestration Timeline

The RALPH loop runs every 5 seconds. Bridge calls within this loop are **blocking
synchronous** (via `ureq`), not fire-and-forget. All operations execute sequentially
within the `spawn_ralph_loop` tokio task.

### Every Tick

| Order | Operation | Code Location |
|-------|-----------|---------------|
| 1 | `state.increment_tick()` | main.rs |
| 2 | `state.breaker_tick()` | Advance breaker FSMs (Open->HalfOpen after timeout) |
| 3 | Conductor advisory `tick_once()` | m29_tick via m27_conductor |
| 4 | Hebbian STDP `apply_stdp()` | m18_hebbian_stdp, updates coupling weights |
| 5 | Increment LTP/LTD counters | AtomicU64 updates |
| 6 | `build_tensor_from_state()` | Reads all 12 dimensions from live state |
| 7 | `ralph.tick(&tensor, tick)` | 5-phase RALPH engine step |
| 8 | `feed_emergence_observations()` | 8 detector checks |
| 9 | `tick_decay_at()` | Expire old emergence records |
| 10 | Feed emergence into correlation | `correlation().ingest_emergence()` |
| 11 | `relay_emergence_to_rm()` | New events -> RM TSV |

### Cadenced Operations (modular tick intervals)

| Cadence | What Runs | Bridge Call? |
|---------|-----------|-------------|
| `%5` (25s) | Field stability probe (r_slice window=20) | No |
| `%6` (30s) | `post_field_to_synthex()` | YES: POST 127.0.0.1:8090/api/ingest |
| `%6` (30s) | Thermal spike detection | No (reads cached response) |
| config interval | SYNTHEX thermal poll `poll_thermal()` (via `should_poll(tick)`) | YES: GET 127.0.0.1:8090/v3/thermal |
| `%6` (30s) | Blackboard hebbian_summary persist | YES: SQLite insert |
| `%12` (60s) | ME observer poll `poll_observer()` | YES: GET 127.0.0.1:8080/api/observer |
| `%12` (60s) | STDP summary logging | No |
| `%12` (60s) | Chimera formation detection | No (reads cached phases) |
| `%12` (60s) | HebbianSaturation detection | No |
| `%12` (60s) | DispatchLoop detection (monitor-based) | No |
| `%12` (60s) | ConsentCascade detection (monitor-based) | No |
| `%12` (60s) | RALPH tick logging | No |
| `%30` (2.5m) | `post_state_to_vms()` | YES: POST 127.0.0.1:8120/mcp/tools/call |
| `%30` (2.5m) | VMS semantic query (Recognize phase only) | YES: POST 127.0.0.1:8120/mcp/tools/call |
| `%60` (5m) | RALPH state persist to blackboard | YES: SQLite upsert |
| `%60` (5m) | Session persist to blackboard | YES: SQLite upsert |
| `%60` (5m) | Coupling weight persist to blackboard | YES: SQLite upsert |
| `%60` (5m) | STDP→POVM pathway persist | YES: POST 127.0.0.1:8125/pathways (x10) |
| `%60` (5m) | `post_state_to_rm()` | YES: POST 127.0.0.1:8130/put (TSV) |
| `%60` (5m) | Blackboard stale pane pruning | YES: SQLite DELETE |
| `%120` (10m) | Homeostatic weight normalization | No (in-memory) |
| `%300` (25m) | `trigger_vms_consolidation()` | YES: POST 127.0.0.1:8120/v1/adaptation/trigger |

### All bridge calls in the tick loop are BLOCKING SYNC

The RALPH tick loop runs on a single tokio task. All bridge calls use `ureq` (blocking
HTTP client) or `rusqlite` (blocking SQLite). No `tokio::spawn` or `.await` on bridge
calls within the loop body. This means a slow bridge response delays the entire tick.

The circuit breaker pattern (m21) mitigates this: when a bridge enters `Open` state,
calls are skipped entirely via `state.breaker_allows(service)` guard checks.

---

## 4. Daemon Startup Sequence

```
main() [src/bin/main.rs]
  |
  |-- 1. Initialize tracing (tracing_subscriber with env filter)
  |
  |-- 2. Load PvConfig::load()
  |      config/default.toml -> config/production.toml -> PV2_* env vars
  |      Falls back to PvConfig::default() on failure
  |
  |-- 3. Create OracState::new(config) [wrapped in Arc]
  |      Initializes:
  |        - sessions: RwLock<HashMap> (empty)
  |        - coupling: RwLock<CouplingNetwork::new()>
  |        - field_state: RwLock<AppState::default()>
  |        - ralph: RalphEngine::new()
  |        - synthex_bridge: SynthexBridge::new()
  |        - me_bridge: MeBridge::new()
  |        - rm_bridge: RmBridge::new()
  |        - blackboard: Mutex<Blackboard::open()> (SQLite, 9 tables)
  |        - breaker_registry: RwLock<BreakerRegistry::new()>
  |        - token_accountant: TokenAccountant::new()
  |        - trace_store: TraceStore::new()
  |        - metrics_registry: MetricsRegistry::new()
  |        - field_dashboard: FieldDashboard::new()
  |        - tick: AtomicU64(0)
  |        - various atomic counters (all 0)
  |
  |-- 4. hydrate_startup_state(&state)
  |      4a. Load RALPH state from blackboard
  |          blackboard.load_ralph_state() -> ralph.hydrate(gen, cycles, peak_fitness)
  |      4b. Load sessions from blackboard
  |          blackboard.load_sessions() -> sessions.write().insert(...)
  |      4c. Load coupling weights from blackboard (preferred source)
  |          blackboard.load_coupling_weights() -> coupling.write().set_weight(...)
  |      4d. Load coupling weights from POVM (fallback)
  |          PovmBridge::hydrate_pathways() -> coupling.write().connections[].weight
  |
  |-- 5. spawn_field_poller(state)
  |      Tokio task: polls PV2 :8132/health every 5s
  |      Updates: field_state.write() (r, K, spheres, tick)
  |      Also syncs PV2 sphere IDs into coupling network (GAP-A fix)
  |
  |-- 6. spawn_ipc_listener(state)
  |      Tokio task: connects to /run/user/1000/pane-vortex-bus.sock
  |      Subscribes to field.* + sphere.* events
  |      Event loop: process_bus_event() updates field_state + spheres
  |      Reconnects with exponential backoff (5s->30s cap)
  |
  |-- 7. spawn_ralph_loop(state, halt_recv)
  |      Tokio task: 5s interval tick loop (see Section 3)
  |      Runs RALPH 5-phase evolution + all cadenced operations
  |      Stops on halt_recv signal (shutdown)
  |
  |-- 8. Build Axum router via build_router(state)
  |      Routes:
  |        GET  /health
  |        GET  /field
  |        GET  /blackboard
  |        GET  /metrics
  |        GET  /traces
  |        GET  /dashboard
  |        GET  /tokens
  |        GET  /consent/{sphere_id}
  |        PUT  /consent/{sphere_id}
  |        GET  /field/ghosts
  |        POST /hooks/session_start
  |        POST /hooks/user_prompt_submit
  |        POST /hooks/pre_tool_use
  |        POST /hooks/post_tool_use
  |        POST /hooks/stop
  |        POST /hooks/permission_request
  |
  |-- 9. Bind TCP listener on config.server.port (default 8133)
  |
  |-- 10. axum::serve(listener, router).with_graceful_shutdown(ctrl_c)
  |       On SIGINT: sends halt=true to RALPH loop, then shuts down Axum
  |
  STEADY STATE: 4 concurrent tasks
    - Axum HTTP server (hook handling)
    - Field poller (PV2 health polling)
    - IPC listener (PV2 bus event subscription)
    - RALPH loop (evolution + bridge orchestration)
```

---

## 5. Hebbian STDP Flow

Spike-Timing-Dependent Plasticity runs every tick within the RALPH loop.

```
Every tick in spawn_ralph_loop:
  |
  |-- 1. Clone spheres from field_state (read lock, then drop)
  |      spheres = state.field_state.read().spheres.clone()
  |
  |-- 2. apply_stdp(&mut coupling.write(), &spheres)
  |      For each connection (from, to) in coupling.connections:
  |        a. Look up sphere_a = spheres[from], sphere_b = spheres[to]
  |        b. Co-activation test: are_coactive(sphere_a, sphere_b)
  |           - Both must have PaneStatus::Working
  |           - Phase difference < pi/3 (within PHASE_GAP_THRESHOLD)
  |        c. If co-active → LTP (Long-Term Potentiation)
  |           - base_rate = HEBBIAN_LTP (0.01)
  |           - compute_ltp_rate(): burst_multiplier (3x if tool_calls>5),
  |             newcomer_multiplier (2x if first 50 ticks)
  |           - new_weight = old_weight + rate * (1.0 - old_weight)
  |           - Clamp to [HEBBIAN_WEIGHT_FLOOR(0.15), 1.0]
  |           - ltp_count += 1
  |        d. If NOT co-active → LTD (Long-Term Depression)
  |           - rate = HEBBIAN_LTD (0.002)
  |           - new_weight = old_weight - rate * (old_weight - HEBBIAN_WEIGHT_FLOOR)
  |           - Clamp to [HEBBIAN_WEIGHT_FLOOR(0.15), 1.0]
  |           - ltd_count += 1
  |      Returns StdpResult { ltp_count, ltd_count, at_floor_count, total_weight_change }
  |
  |-- 3. Update atomic counters
  |      co_activations_total += ltp_count
  |      hebbian_ltp_total += ltp_count
  |      hebbian_ltd_total += ltd_count
  |      hebbian_last_tick = tick
  |
  |-- 4. POVM persistence (every 60 ticks)
  |      persist_stdp_to_povm():
  |        - Take top 10 connections by weight
  |        - POST each as individual pathway to 127.0.0.1:8125/pathways
  |        - Payload: { pre_id, post_id, weight, co_activations }
  |
  |-- 5. Homeostatic normalization (every 120 ticks)
  |      For each connection in coupling.connections:
  |        - If weight == 1.0 (ceiling): multiplicative decay toward mean
  |          new = 0.98 * weight + 0.02 * w_mean
  |        - If weight == floor (0.15): additive boost toward mean
  |          new = weight + 0.005, capped at w_mean
  |        - Clamp to [floor, 1.0]
  |
  |-- 6. Blackboard summary (every 6 ticks)
  |      Insert HebbianSummaryRecord:
  |        { tick, ltp_count, ltd_count, at_floor_count,
  |          total_weight_change, connections_total,
  |          weight_mean, weight_min, weight_max }
```

---

## 6. Semantic Routing Flow

Content-aware dispatch from tool events to fleet panes.

```
PostToolUse hook handler (m12_tool_hooks):
  |
  |-- 1. classify_tool(tool_name) → SemanticDomain
  |      Read  → Read domain (phase 0.0)
  |      Write → Write domain (phase pi/2)
  |      Bash  → Execute domain (phase pi)
  |      Grep/Glob → Read
  |      Edit  → Write
  |      (word-boundary matching + fallback heuristics)
  |
  |-- 2. classify_content(tool_output) → SemanticDomain
  |      Keyword scoring across 4 domains:
  |        Read:        "read", "file", "found", "search", "content"
  |        Write:       "write", "edit", "create", "update", "modify"
  |        Execute:     "run", "execute", "build", "test", "compile"
  |        Communicate: "send", "post", "deploy", "push", "publish"
  |      Highest-scoring domain wins; ties go to Read (default)
  |
  |-- 3. Composite scoring via route()
  |      For each candidate sphere in field_state.spheres:
  |        a. domain_affinity = cos(sphere.phase - domain.phase())
  |           Weight: 40%
  |        b. hebbian_weight = coupling.get_weight(from_pane, candidate)
  |           Weight: 35%
  |        c. availability = 1.0 if Idle, 0.5 if Working, 0.0 if Blocked
  |           Weight: 25%
  |        d. preferred_bonus = 0.1 if candidate is in RouteRequest.preferred
  |        e. total_score = 0.40 * domain + 0.35 * hebbian + 0.25 * availability + bonus
  |
  |-- 4. Return RouteResult
  |      { chosen_pane, domain, scores: Vec<CandidateScore> }
  |      chosen_pane = candidate with highest total_score
  |
  |-- 5. Record dispatch
  |      state.record_dispatch(domain)
  |      Increments: dispatch_total + domain-specific counter
```

---

## 7. Cross-Service Data Pipeline

Complete map of all data flowing between external services through ORAC.

### Primary Data Flows (Tick Loop)

```
PV2 (:8132) ──GET /health──→ spawn_field_poller → field_state (r, K, spheres, tick)
PV2 (:8132) ──IPC socket──→ spawn_ipc_listener → field_state (sphere register/deregister, phases)

SYNTHEX (:8090) ──GET /v3/thermal──→ synthex_bridge → k_modulation, cached ThermalResponse
ORAC ──POST /api/ingest──→ SYNTHEX (:8090)     [%6: r, me_fitness, cascade_heat, nexus_health,
                                                  resonance, emergence_heat, emergence_diversity]

ME (:8080) ──GET /api/observer──→ me_bridge → last_fitness, is_frozen, is_subscribed     [%12]

VMS (:8120) ←──POST /mcp/tools/call (write_memory)──  ORAC  [%30: field observation]
VMS (:8120) ──POST /mcp/tools/call (query_relevant)──→ ORAC  [%30: Recognize phase context]
VMS (:8120) ←──POST /v1/adaptation/trigger──           ORAC  [%300: consolidation trigger]

POVM (:8125) ←──POST /pathways──  ORAC  [%60: top 10 coupling weights as co-activations]
POVM (:8125) ──GET /hydrate──→ ORAC    [startup: pathway weights into coupling network]

RM (:8130) ←──POST /put──  ORAC  [%60: RALPH state as TSV record]
RM (:8130) ←──POST /put──  ORAC  [per-tick: new emergence events as TSV]
RM (:8130) ──GET /search──→ ORAC [startup: session history hydration]
```

### 8 Additional Named Flows

| Flow | Cadence | Source | Destination | Data |
|------|---------|--------|-------------|------|
| `relay_emergence_to_rm` | every tick (new events only) | EmergenceDetector | RM :8130 | Emergence type, severity, confidence, description (TSV) |
| `session_persist` | %60 | sessions HashMap | Blackboard SQLite | session_id, pane_id, poll_counter, tool_calls, persona |
| `coupling_weight_persist` | %60 | CouplingNetwork | Blackboard SQLite | from_id, to_id, weight (all connections) |
| `pane_pruning` | %60 | Blackboard SQLite | (self-mutation) | DELETE stale panes older than 15 minutes |
| `thermal_spike_detection` | %6 | synthex_bridge cached response | EmergenceDetector | temperature vs target delta check |
| `chimera_detection` | %12 | field_state sphere phases | EmergenceDetector | Phase distribution cluster analysis |
| `dispatch_loop_detection` | %12 | dispatch_total AtomicU64 | EmergenceDetector (monitor) | Dispatch delta over 12-tick window |
| `consent_cascade_detection` | %12 | field_state spheres opt_out_hebbian | EmergenceDetector (monitor) | Opt-out count increase over 12-tick window |

### Hook-Triggered Flows (Event-Driven, Not Tick-Based)

```
SessionStart:
  ORAC → POVM :8125 GET /health (hydrate summary)
  ORAC → RM :8130 GET /search (session history)
  ORAC → Blackboard (upsert_pane)
  ORAC → CouplingNetwork (register sphere)

PostToolUse:
  ORAC → PV2 :8132 POST /bus/events (tool event notification) [fire-and-forget]
  ORAC → POVM :8125 POST /memories (significant tools) [breaker-guarded]
  ORAC → Blackboard (upsert_pane with tool_name)
  ORAC → TokenAccountant (record_pane_usage)

Stop:
  ORAC → CouplingNetwork (deregister sphere)
  ORAC → Blackboard (remove_pane, insert_ghost)
  ORAC → ghosts VecDeque (OracGhost record)
```

### Data Flow Diagram (Simplified)

```
                    ┌──────────┐
          IPC sock  │   PV2    │  HTTP /health
       ┌───────────→│  :8132   │←──────────┐
       │            └──────────┘           │
       │                                   │
       │  ┌──────────┐    ┌──────────┐     │
       │  │ SYNTHEX  │    │   ME     │     │
       │  │  :8090   │    │  :8080   │     │
       │  └─────┬────┘    └────┬─────┘     │
       │  %6 ↕  │ingest   %12 │observer   │ %5s poll
       │        ↓              ↓           │
  ┌────┴────────────────────────────────────┴───┐
  │                ORAC :8133                    │
  │  ┌─────────┐  ┌────────┐  ┌──────────────┐  │
  │  │ Coupling │  │ RALPH  │  │ Emergence    │  │
  │  │ Network  │  │ Engine │  │ Detector     │  │
  │  └────┬─────┘  └───┬────┘  └──────┬───────┘  │
  │       │ %60        │ %60          │ per-tick  │
  │       ↓            ↓              ↓           │
  │  ┌─────────┐  ┌────────┐  ┌──────────────┐  │
  │  │Blackboard│  │  RM    │  │    POVM      │  │
  │  │ SQLite   │  │ :8130  │  │   :8125      │  │
  │  └─────────┘  └────────┘  └──────────────┘  │
  └──────────────────────┬──────────────────────┘
                         │ %30 ↕ write/query
                    ┌────┴─────┐
                    │   VMS    │
                    │  :8120   │
                    └──────────┘
```
