# D4: Bridge & Pipeline Wiring

> **ORAC Sidecar** -- 6 bridges, 10 blackboard tables, IPC bus, RALPH data pipeline
>
> **Source of truth:** `src/m5_bridges/` (M22-M26), `src/bin/main.rs` (tick loop + bridge polling), `src/m3_hooks/m10_hook_server.rs` (spawn_field_poller)

---

## 1. Bridge Topology Overview

ORAC operates as a **central hub** connecting 6 external services. All data flows through ORAC's `OracState`, which holds cached bridge data, coupling network state, and the RALPH evolution engine.

```
                            +------------------+
                            |   ORAC Sidecar   |
                            |     :8133        |
                            |                  |
                +-----------+ OracState (Arc)  +-----------+
                |           | - field_state    |           |
                |           | - coupling       |           |
                |           | - ralph          |           |
                |           | - blackboard     |           |
                |           | - sessions       |           |
                |           | - breakers       |           |
                |           +--------+---------+           |
                |                    |                     |
    +-----------v------+    +-------v--------+    +-------v---------+
    |  SYNTHEX :8090   |    |   PV2 :8132    |    |   POVM :8125    |
    |  thermal read    |    |  field poll    |    |  pathway read   |
    |  field ingest    |    |  sphere sync   |    |  snapshot write |
    +------------------+    |  task bus      |    +-----------------+
                            |  IPC socket    |
    +------------------+    +----------------+    +-----------------+
    |    ME :8080      |                          |    RM :8130     |
    |  fitness read    |    +----------------+    |  TSV persist    |
    |  (read-only)     |    |   VMS :8120    |    |  search read    |
    +------------------+    |  memory post   |    +-----------------+
                            |  consolidation |
                            |  semantic query|
                            +----------------+
```

### Data Direction Summary

| Bridge | Read (ORAC <- Service) | Write (ORAC -> Service) |
|--------|----------------------|------------------------|
| SYNTHEX | thermal k_adjustment | field state (r, fitness, spheres) |
| ME | fitness, correlations, events | none (read-only bridge) |
| POVM | pathways (startup + periodic) | snapshots, STDP co-activations |
| RM | search results (startup) | RALPH state, emergence events (TSV) |
| VMS | semantic memories (Recognize phase) | observations (r, fitness, spheres) |
| PV2 | field state, spheres, tasks | sphere register/deregister, memory, status, task ops |

---

## 2. Per-Bridge Detail

### 2.1 SYNTHEX Bridge

| Property | Value |
|----------|-------|
| Module | `m5_bridges/m22_synthex_bridge.rs` |
| Target | `127.0.0.1:8090` |
| Protocol | Raw TCP HTTP |
| Poll cadence | Every 6 ticks (configurable, `DEFAULT_POLL_INTERVAL = 6`) |
| Consent | `synthex_write` gates outbound field posts |

**Data read:** `GET /v3/thermal` returns `ThermalResponse { temperature, target, pid_output, heat_sources }`. ORAC computes `thermal_adjustment()` from temperature deviation: `(1.0 - deviation * 0.2).clamp(0.85, 1.15)`. Cold systems get coupling boost (>1.0), hot systems get coupling reduction (<1.0). The adjustment is applied directly to `coupling.k_modulation`.

**Data written:** `POST /api/ingest` with field state JSON every 6 ticks from `main.rs::post_field_to_synthex()`. Payload includes `r`, `ralph_fitness`, `sphere_count`, `tick`, and tool call rate. This feeds SYNTHEX heat sources (HS-001 through HS-004).

**Circuit breaker:** Registered as `"synthex"` with default config. `should_poll()` is checked before each poll. Breaker success/failure recorded after each poll attempt. If breaker is Open, poll is skipped entirely (BUG-GEN05).

**Error handling:**
- `PvError::BridgeUnreachable` on TCP connection failure
- `PvError::BridgeParse` on JSON deserialization failure or non-finite adjustment
- On failure: `record_failure()` increments `consecutive_failures`, sets `stale = true`
- Cached adjustment persists through failures (graceful degradation)

**Frozen detection:** SYNTHEX bridge does not have frozen detection (unlike ME). Staleness is determined by: `stale` flag set, or `2 * poll_interval` ticks elapsed since last successful poll.

### 2.2 ME Bridge

| Property | Value |
|----------|-------|
| Module | `m5_bridges/m23_me_bridge.rs` |
| Target | `127.0.0.1:8080` |
| Protocol | Raw TCP HTTP |
| Poll cadence | Every 12 ticks (`DEFAULT_POLL_INTERVAL = 12`) |
| Consent | None (read-only bridge) |

**Data read:** `GET /api/observer` returns `RawObserverResponse` with nested `last_report.current_fitness`. Bridge flattens to `ObserverResponse { fitness, active_layers, has_publishers, status, correlations_since, emergences_since, total_correlations, total_events }`.

Fitness-to-adjustment mapping: Linear interpolation across `[K_MOD_BUDGET_MIN, K_MOD_BUDGET_MAX]` (0.85 to 1.15). Fitness 0.0 maps to 0.85, fitness 0.5 maps to 1.0 (neutral), fitness 1.0 maps to 1.15.

**Data written:** None. `post()` is a no-op returning `Ok(())`.

**Circuit breaker:** Registered as `"me"` with tolerant config (threshold=10, timeout=15). Breaker success/failure recorded after each poll (BUG-059e fix -- previously never updated).

**Frozen detection (BUG-008):**
- 3 consecutive readings within `FROZEN_TOLERANCE` (0.003) triggers `is_frozen = true`
- Known frozen value `0.3662` is immediately detected regardless of count
- When frozen: returns neutral adjustment (1.0) instead of fitness-based value
- Frozen resets when a different fitness reading arrives

**Error handling:** Same pattern as SYNTHEX -- `record_failure()` on error, cached adjustment persists.

### 2.3 POVM Bridge

| Property | Value |
|----------|-------|
| Module | `m5_bridges/m24_povm_bridge.rs` |
| Target | `127.0.0.1:8125` |
| Protocol | Raw TCP HTTP |
| Write cadence | Every 12 ticks (`DEFAULT_WRITE_INTERVAL = 12`) |
| Read cadence | Every 60 ticks (`DEFAULT_READ_INTERVAL = 60`) |
| Consent | `povm_read` for reads, `povm_write` for writes |

**Data read:** `GET /pathways` returns `PathwaysResponse { pathways: Vec<Pathway> }`. Each `Pathway` has `source` (alias `pre_id`), `target` (alias `post_id`), `weight`, `reinforcement_count` (alias `co_activations`). Used for Hebbian weight hydration at startup and periodic refresh.

**Data written:**
1. Sphere snapshots via `POST /memories` (fire-and-forget, every 12 ticks)
2. STDP pathway co-activations via `POST /pathways` (every 60 ticks from `persist_stdp_to_povm()` in main.rs) -- posts top coupling connections by weight with `pre_id`/`post_id`/`weight` format

**Important:** POVM is write-only in normal operation (BUG-034). You must call `GET /hydrate` to read back stored state -- the memories endpoint is write-only.

**Max response size:** 2 MB (Gen-060a: raised from 512 KB because POVM pathway responses grow with STDP activity, measured at 1.3 MB with 2,437+ pathways).

**Circuit breaker:** Registered as `"povm"` with default config.

### 2.4 RM Bridge

| Property | Value |
|----------|-------|
| Module | `m5_bridges/m25_rm_bridge.rs` |
| Target | `127.0.0.1:8130` |
| Protocol | Raw TCP HTTP, **TSV content type** |
| Poll cadence | Every 30 ticks (`DEFAULT_POLL_INTERVAL = 30`) |
| Consent | `rm_write` for writes, `hydration` for startup reads |

**Data read:** `GET /search?q=discovery` returns `RmSearchResult { entries: Vec<String>, total: u64 }`. Used only during `SessionStart` hydration.

**Data written:** `POST /put` with `Content-Type: text/tab-separated-values`. Every 60 ticks from `post_state_to_rm()` in main.rs. RALPH state, field metrics, and emergence events are posted as TSV records.

TSV format: `category\tagent\tconfidence\tttl\tcontent`

Record categories used:
- `field_state` -- periodic field snapshots (TTL 300s)
- `decision` -- conductor advisory decisions
- `session` -- session lifecycle events
- `emergence` -- emergence detection events (relayed via `relay_emergence_to_rm()`)

**Agent name:** `"orac-sidecar"` (not `"pane-vortex"`).

**CRITICAL:** RM accepts TSV ONLY. JSON causes parse failures. The `raw_http_post_tsv()` helper sets `Content-Type: text/tab-separated-values`.

**Circuit breaker:** Registered as `"rm"` with default config.

### 2.5 VMS Bridge

| Property | Value |
|----------|-------|
| Module | No dedicated module -- uses HTTP helpers in `main.rs` |
| Target | `127.0.0.1:8120` |
| Protocol | ureq HTTP (not raw TCP) |
| Post cadence | Every 30 ticks |
| Consolidation cadence | Every 300 ticks |
| Query cadence | Every 30 ticks during Recognize phase |
| Consent | None (uses breaker gating only) |

**Data read:** `GET /v1/memories?limit=5` during RALPH Recognize phase (every 30 ticks). Returns semantic memories that feed environmental context into RALPH's correlation engine via `query_vms_for_ralph_context()`.

**Data written:**
1. `POST /v1/observations` every 30 ticks from `post_state_to_vms()` -- payload includes `r`, `ralph_fitness`, `sphere_count`, `tick`, `emergence_count`
2. `POST /v1/adaptation/trigger` every 300 ticks from `trigger_vms_consolidation()` -- triggers memory consolidation (prune stale, crystallize stable patterns)

**Circuit breaker:** Registered as `"vms"` with tolerant config (threshold=10, failure=3, timeout=10).

### 2.6 PV2 Bridge

| Property | Value |
|----------|-------|
| Module | No dedicated bridge module -- uses ureq via `spawn_field_poller`, `fire_and_forget_post`, `breaker_guarded_post` |
| Target | `http://127.0.0.1:8132` (HTTP) + `/run/user/1000/pane-vortex-bus.sock` (IPC) |
| Protocol | ureq HTTP + Unix domain socket (IPC client M07) |
| Field poll cadence | Every 5 seconds (fixed interval in `spawn_field_poller`) |
| Consent | None |

**Data read (HTTP):**
- `GET /health` every 5s: extracts `r`, `tick`, feeds `SharedState` cache
- `GET /spheres` every 5s: parses `PvSphereCompact` structs, rebuilds `field_state.spheres` HashMap, syncs coupling network
- `GET /bus/tasks`: pending task discovery (throttled 1-in-5 in PostToolUse, live in UserPromptSubmit)

**Data written (HTTP):**
- `POST /sphere/{id}/register` on SessionStart
- `POST /sphere/{id}/memory` on PostToolUse
- `POST /sphere/{id}/status` on PostToolUse (set "working") and Stop (set "complete")
- `POST /sphere/{id}/deregister` on Stop
- `POST /bus/claim/{task_id}` on task claim
- `POST /bus/complete/{task_id}` on task completion
- `POST /bus/fail/{task_id}` on Stop (active task cleanup)

**Data read (IPC):** `spawn_ipc_listener()` connects to PV2 Unix domain socket, subscribes to `field.*` + `sphere.*` patterns. Receives `BusFrame::Event` with real-time field and sphere updates. Connection uses escalating backoff (5s to 120s cap, resets on success -- BUG-C002).

**Circuit breaker:** Registered as `"pv2"` with tolerant config (threshold=10, timeout=15). Breaker ticked from both field_poller AND RALPH loop (2x rate). BUG-059c fix: tick breaker BEFORE recording success to trigger Open->HalfOpen transition. BUG-059d fix: only trip breaker on transport/5xx errors, not 4xx (404 on `/sphere/{id}/memory` is normal for missing endpoints).

---

## 3. Cross-Bridge Interactions

These are the data flows where output from one bridge feeds into another through ORAC's internal state.

### 3.1 SYNTHEX thermal -> Coupling k_modulation -> Conductor decision

```
SYNTHEX /v3/thermal
  --> ThermalResponse.thermal_adjustment()
    --> coupling.k_modulation = k_adj
      --> Conductor reads k * k_modulation for PI control
        --> FieldDecision.k_delta advisory
```

The thermal feedback loop: SYNTHEX measures system temperature from 4 heat sources. ORAC posts field state to `/api/ingest` every 6 ticks (HS-001: r, HS-003: fitness, HS-004: sphere_count). SYNTHEX PID controller adjusts temperature. ORAC reads the adjusted temperature, computes coupling modulation, and applies it to the Kuramoto coupling constant.

### 3.2 ME fitness -> Tensor D3 -> RALPH evolution

```
ME /api/observer
  --> ObserverResponse.fitness
    --> me_bridge.last_fitness() cached
      --> build_tensor_from_state() reads me_fitness
        --> TensorValues D2/D3 populated
          --> RALPH.tick(&tensor, tick) consumes for evolution
```

ME fitness feeds the RALPH evolution loop through the fitness tensor. When fitness is frozen (BUG-008), ORAC uses neutral adjustment (1.0) to prevent stale data from corrupting evolution decisions.

### 3.3 STDP weights -> POVM pathways -> Hydration on restart

```
apply_stdp() in RALPH loop
  --> StdpResult { ltp_count, ltd_count, weight changes }
    --> persist_stdp_to_povm() every 60 ticks
      --> POVM POST /pathways with {pre_id, post_id, weight}
        --> On restart: hydrate_startup_state()
          --> POVM GET /pathways
            --> coupling.set_weight() for matched IDs
```

STDP learning creates dynamic coupling weights that persist across restarts via POVM. Note: POVM sphere IDs may not match ORAC coupling IDs (BUG: 2,504 pathways loaded but 0 matched coupling IDs because ORAC uses `hostname:pid:uuid` format while POVM stores PV2 sphere IDs).

### 3.4 PV2 spheres -> Coupling network sync -> STDP

```
PV2 GET /spheres (every 5s via spawn_field_poller)
  --> field_state.spheres HashMap
    --> Coupling network sync (register new, prune departed)
      --> apply_stdp() finds matching connection endpoints
        --> LTP/LTD events drive weight differentiation
```

GAP-A fix: ORAC always syncs PV2 sphere IDs into the coupling network. Without this, STDP never finds matching endpoints and LTP stays at 0 permanently.

### 3.5 Field state -> VMS memories -> RALPH Recognize

```
spawn_field_poller() updates field_state
  --> post_state_to_vms() every 30 ticks
    --> VMS POST /v1/observations
      --> VMS stores as oscillating memory
        --> query_vms_for_ralph_context() during Recognize
          --> VMS GET /v1/memories?limit=5
            --> Correlation engine ingests environmental context
```

### 3.6 Emergence detection -> RM persistence -> Cross-session learning

```
feed_emergence_observations() every RALPH tick
  --> EmergenceDetector checks 8 types
    --> relay_emergence_to_rm() for new events
      --> RM POST /put (TSV: "emergence\torac-sidecar\t{confidence}\t3600\t{description}")
        --> Persists across ORAC restarts for pattern mining
```

### 3.7 RALPH state -> Blackboard + RM -> Cross-restart hydration

```
RALPH.tick() produces generation, fitness, phase
  --> save_ralph_state() to blackboard every 60 ticks
  --> post_state_to_rm() to RM every 60 ticks (TSV)
    --> On restart: hydrate_startup_state()
      --> bb.load_ralph_state() restores generation, cycles, peak_fitness
```

---

## 4. Blackboard Schema

All tables defined in `m5_bridges/m26_blackboard.rs::Blackboard::migrate()` (line 256). SQLite WAL mode. Feature-gated under `persistence`.

### 4.1 Table: pane_status

```sql
CREATE TABLE IF NOT EXISTS pane_status (
    pane_id         TEXT PRIMARY KEY,
    status          TEXT NOT NULL DEFAULT 'Idle',
    persona         TEXT NOT NULL DEFAULT '',
    updated_at      REAL NOT NULL DEFAULT 0.0,
    phase           REAL NOT NULL DEFAULT 0.0,
    tasks_completed INTEGER NOT NULL DEFAULT 0
);
```

CRUD: `upsert_pane()`, `get_pane()`, `list_panes()`, `remove_pane()`, `pane_count()`, `prune_stale_panes(cutoff)`, `prune_complete_panes(cutoff)`.

### 4.2 Table: task_history

```sql
CREATE TABLE IF NOT EXISTS task_history (
    task_id      TEXT PRIMARY KEY,
    pane_id      TEXT NOT NULL,
    description  TEXT NOT NULL DEFAULT '',
    outcome      TEXT NOT NULL DEFAULT 'completed',
    finished_at  REAL NOT NULL DEFAULT 0.0,
    duration_secs REAL NOT NULL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_task_history_pane ON task_history(pane_id);
CREATE INDEX IF NOT EXISTS idx_task_history_finished ON task_history(finished_at);
```

CRUD: `insert_task()`, `recent_tasks(pane_id, limit)`, `task_count()`, `prune_old_tasks(cutoff)`.

### 4.3 Table: agent_cards

```sql
CREATE TABLE IF NOT EXISTS agent_cards (
    pane_id       TEXT PRIMARY KEY,
    capabilities  TEXT NOT NULL DEFAULT '[]',
    domain        TEXT NOT NULL DEFAULT '',
    model         TEXT NOT NULL DEFAULT '',
    registered_at REAL NOT NULL DEFAULT 0.0
);
```

CRUD: `upsert_card()`, `get_card()`, `list_cards()`, `card_count()`.

Capabilities stored as JSON array string (e.g., `'["read","write","execute","search"]'`).

### 4.4 Table: ghost_traces

```sql
CREATE TABLE IF NOT EXISTS ghost_traces (
    sphere_id           TEXT NOT NULL,
    persona             TEXT NOT NULL DEFAULT '',
    deregistered_ms     INTEGER NOT NULL,
    final_phase         REAL NOT NULL DEFAULT 0.0,
    total_tools         INTEGER NOT NULL DEFAULT 0,
    session_duration_ms INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_ghost_traces_time ON ghost_traces(deregistered_ms);
```

CRUD: `insert_ghost()`, `recent_ghosts(limit)`, `prune_ghosts(max_entries)`.

No primary key -- allows multiple ghost entries for the same sphere_id (different sessions).

### 4.5 Table: consent_declarations

```sql
CREATE TABLE IF NOT EXISTS consent_declarations (
    sphere_id       TEXT PRIMARY KEY,
    synthex_write   INTEGER NOT NULL DEFAULT 1,
    povm_read       INTEGER NOT NULL DEFAULT 1,
    povm_write      INTEGER NOT NULL DEFAULT 0,
    hydration       INTEGER NOT NULL DEFAULT 1,
    updated_ms      INTEGER NOT NULL DEFAULT 0
);
```

CRUD: `upsert_consent()`, `get_consent()`.

Note: `povm_write` defaults to 0 (opt-in only), all others default to 1 (opt-out).

### 4.6 Table: consent_audit

```sql
CREATE TABLE IF NOT EXISTS consent_audit (
    sphere_id   TEXT NOT NULL,
    field_name  TEXT NOT NULL,
    old_value   INTEGER NOT NULL,
    new_value   INTEGER NOT NULL,
    changed_ms  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_consent_audit_sphere ON consent_audit(sphere_id);
CREATE INDEX IF NOT EXISTS idx_consent_audit_time ON consent_audit(changed_ms);
```

CRUD: `insert_consent_audit()`.

Append-only audit trail -- no update or delete operations.

### 4.7 Table: hebbian_summary

```sql
CREATE TABLE IF NOT EXISTS hebbian_summary (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    tick                INTEGER NOT NULL,
    ltp_count           INTEGER NOT NULL DEFAULT 0,
    ltd_count           INTEGER NOT NULL DEFAULT 0,
    at_floor_count      INTEGER NOT NULL DEFAULT 0,
    total_weight_change REAL NOT NULL DEFAULT 0.0,
    connections_total   INTEGER NOT NULL DEFAULT 0,
    weight_mean         REAL NOT NULL DEFAULT 0.0,
    weight_min          REAL NOT NULL DEFAULT 0.0,
    weight_max          REAL NOT NULL DEFAULT 0.0,
    created_at          REAL NOT NULL DEFAULT 0.0
);

CREATE INDEX IF NOT EXISTS idx_hebbian_summary_tick ON hebbian_summary(tick DESC);
```

CRUD: `insert_hebbian_summary()`, `recent_hebbian_summaries(limit)`.

Written every 6 ticks from the RALPH loop. Cumulative LTP/LTD counts read from atomic counters (BUG-SCAN-004 fix).

### 4.8 Table: ralph_state

```sql
CREATE TABLE IF NOT EXISTS ralph_state (
    id                  INTEGER PRIMARY KEY CHECK (id = 1),
    generation          INTEGER NOT NULL DEFAULT 0,
    completed_cycles    INTEGER NOT NULL DEFAULT 0,
    current_fitness     REAL NOT NULL DEFAULT 0.5,
    peak_fitness        REAL NOT NULL DEFAULT 0.0,
    total_proposed      INTEGER NOT NULL DEFAULT 0,
    total_accepted      INTEGER NOT NULL DEFAULT 0,
    total_rolled_back   INTEGER NOT NULL DEFAULT 0,
    last_phase          TEXT NOT NULL DEFAULT 'Recognize',
    updated_at          REAL NOT NULL DEFAULT 0.0
);
```

CRUD: `save_ralph_state()` (UPSERT with `id = 1`), `load_ralph_state()`.

Singleton row (`CHECK (id = 1)`) -- only one RALPH state persists. Saved every 60 ticks.

### 4.9 Table: sessions

```sql
CREATE TABLE IF NOT EXISTS sessions (
    session_id      TEXT PRIMARY KEY,
    pane_id         TEXT NOT NULL,
    active_task_id  TEXT,
    poll_counter    INTEGER NOT NULL DEFAULT 0,
    total_tool_calls INTEGER NOT NULL DEFAULT 0,
    started_ms      INTEGER NOT NULL,
    persona         TEXT NOT NULL DEFAULT '',
    updated_at      REAL NOT NULL DEFAULT 0.0
);
```

CRUD: `save_sessions()` (bulk UPSERT), `load_sessions()`, `remove_session()`.

GAP-C fix: sessions now persisted to survive ORAC restarts. Saved every 60 ticks alongside RALPH state.

### 4.10 Table: coupling_weights

```sql
CREATE TABLE IF NOT EXISTS coupling_weights (
    from_id     TEXT NOT NULL,
    to_id       TEXT NOT NULL,
    weight      REAL NOT NULL,
    updated_at  REAL NOT NULL DEFAULT 0.0,
    PRIMARY KEY (from_id, to_id)
);
```

CRUD: `save_coupling_weights()` (bulk UPSERT), `load_coupling_weights()`.

IGNITION-1e: Persists Hebbian coupling weights every 60 ticks so learning survives restarts. Composite primary key on `(from_id, to_id)`.

### Migration File Divergence Note

The DDL above is the live schema from code (`Blackboard::migrate()`). There is a separate `migrations/` directory at the project root, but the runtime schema is authoritative -- the migration files may lag behind code changes.

---

## 5. IPC Bus Event Flow

### Connection Lifecycle

`spawn_ipc_listener()` in `main.rs`:

1. Creates `IpcClient` with ORAC pane ID
2. Connects to `/run/user/1000/pane-vortex-bus.sock`
3. Sends `BusFrame::Handshake { pane_id, version: "2.0" }`
4. Receives `BusFrame::Welcome { session_id, version }`
5. Sends `BusFrame::Subscribe { patterns: ["field.*", "sphere.*"] }`
6. Receives `BusFrame::Subscribed { count }`
7. Enters event loop: processes incoming `BusFrame::Event` frames

Connection state tracked in `OracState.ipc_state`: `"disconnected"` -> `"connecting"` -> `"connected"` -> `"subscribed"`.

### Subscribe Patterns

ORAC subscribes to:
- `field.*` -- field tick events, coupling updates, phase changes
- `sphere.*` -- sphere registration, deregistration, status changes

### Event Processing

`process_bus_event()` routes `BusEvent` by `event_type`:

| Event type pattern | Processing |
|-------------------|------------|
| `field.tick` | Update cached field state timestamp |
| `sphere.registered` | Register new sphere in coupling network |
| `sphere.deregistered` | Create ghost trace, deregister from coupling network |
| `sphere.status` | Update cached sphere status |

### Reconnection

On disconnect or error: escalating backoff starting at 5s, doubling to max 120s cap. Counter resets on successful connection (BUG-L004 fix).

---

## 6. RALPH Data Pipeline

RALPH (Recognize-Analyze-Learn-Propose-Harvest) is ORAC's 5-phase meta-learning loop running every 5 seconds in `spawn_ralph_loop()`.

### Which Bridges Feed Which RALPH Phases

#### Recognize Phase

**Inputs:**
- `field_state` (from PV2 via `spawn_field_poller`) -- r, tick, sphere count
- `coupling` network -- connection count, weight distribution
- VMS semantic memories (via `query_vms_for_ralph_context()` every 30 ticks) -- environmental context fed to correlation engine
- `me_bridge.last_fitness()` -- ME fitness signal
- `synthex_bridge.last_response()` -- thermal state

**Purpose:** Identify parameters drifting from targets. Builds `TensorValues` from live state.

#### Analyze Phase

**Inputs:**
- `TensorValues` from Recognize -- 12-dimensional fitness vector
- `FitnessTensor` internal history -- trend detection via linear regression

**Bridge feeds into tensor dimensions:**
- D1 `field_coherence`: from PV2 cached `r` + dashboard stddev bonus
- D3 `task_throughput`: from blackboard `pane_status.tasks_completed`
- D4 `error_rate`: from blackboard `task_history` failed/total ratio
- D9 `fleet_utilization`: from blackboard Working/total pane ratio
- D11 `consent_compliance`: from in-memory consent map

**Purpose:** Compute composite fitness score, analyze per-dimension trends.

#### Learn Phase

**Inputs:**
- Emergence events (from `feed_emergence_observations()` -- 8 detector types)
- Correlation engine history

**Emergence detectors and their bridge data sources:**
1. `CoherenceLock` -- r > 0.998 for 50+ ticks (PV2 field state)
2. `ChimeraFormation` -- phase gap > pi/3 between clusters (PV2 spheres)
3. `CouplingRunaway` -- mean weight > 0.9 (coupling network)
4. `HebbianSaturation` -- > 80% connections at weight floor (coupling network)
5. `DispatchLoop` -- same domain > 80% of last 20 dispatches (semantic router)
6. `ThermalSpike` -- temperature > 2x target (SYNTHEX bridge)
7. `BeneficialSync` -- r > 0.85 sustained for 20+ ticks (PV2 field state)
8. `ConsentCascade` -- (monitor-based: multiple opt-outs in short window)

**Purpose:** Mine temporal, causal, and fitness-linked correlations.

#### Propose Phase

**Inputs:**
- Correlation patterns from Learn
- `MutationSelector` with diversity enforcement (BUG-035 fix: round-robin, 10-gen cooldown, >50% diversity rejection)

**Purpose:** Generate parameter mutation proposals. Snapshot current state before mutation.

#### Harvest Phase

**Inputs:**
- Post-mutation fitness tensor (new `TensorValues` after verification window of 10 ticks)
- Pre-mutation snapshot for rollback comparison

**Acceptance criteria:**
- Improvement >= `DEFAULT_ACCEPT_THRESHOLD` (0.02): accept mutation
- Regression <= `DEFAULT_ROLLBACK_THRESHOLD` (-0.01): rollback to snapshot
- Between thresholds: neutral -- keep mutation (no rollback)

**Purpose:** Accept beneficial mutations, rollback harmful ones.

### Persistence Pipeline

Every 60 ticks, the RALPH loop persists to 3 destinations:

1. **Blackboard** (SQLite): RALPH state (`ralph_state` table), sessions (`sessions` table), coupling weights (`coupling_weights` table)
2. **POVM** (HTTP): STDP pathway co-activations (`POST /pathways` every 60 ticks)
3. **RM** (TSV): RALPH state + emergence events (`POST /put` every 60 ticks)

On startup, `hydrate_startup_state()` restores from blackboard (preferred) and falls back to POVM pathways for coupling weight seeding.

### Tick-Based Scheduling Summary

| Tick modulo | Action | Bridge(s) involved |
|-------------|--------|-------------------|
| Every tick (5s) | Conductor advisory, STDP, emergence detection | PV2 (cached), coupling |
| % 6 | Post field to SYNTHEX, STDP summary to blackboard | SYNTHEX, SQLite |
| % 12 | Poll ME observer, coupling stats log | ME |
| % 30 | Post observations to VMS, query VMS for RALPH | VMS |
| % 60 | Persist RALPH/sessions/weights to blackboard + RM, STDP to POVM | SQLite, RM, POVM |
| % 120 | Homeostatic weight normalization | coupling (internal) |
| % 300 | Trigger VMS consolidation | VMS |
| % 720 | Blackboard GC (prune complete panes + old tasks) | SQLite |

---

## Appendix: Constants Reference

| Constant | Value | Source |
|----------|-------|--------|
| ORAC HTTP port | 8133 | `PvConfig.server.port` |
| Field poll interval | 5s | `spawn_field_poller()` |
| RALPH tick interval | 5s | `spawn_ralph_loop()` |
| SYNTHEX poll interval | 6 ticks | `DEFAULT_POLL_INTERVAL` in m22 |
| ME poll interval | 12 ticks | `DEFAULT_POLL_INTERVAL` in m23 |
| POVM write interval | 12 ticks | `DEFAULT_WRITE_INTERVAL` in m24 |
| POVM read interval | 60 ticks | `DEFAULT_READ_INTERVAL` in m24 |
| RM poll interval | 30 ticks | `DEFAULT_POLL_INTERVAL` in m25 |
| VMS observation interval | 30 ticks | `main.rs` |
| VMS consolidation interval | 300 ticks | `main.rs` |
| TCP timeout | 2,000 ms | `DEFAULT_TCP_TIMEOUT_MS` in http_helpers |
| Max response (default) | 32,768 bytes | `DEFAULT_MAX_RESPONSE_SIZE` in http_helpers |
| Max response (POVM) | 2,097,152 bytes (2 MB) | `MAX_RESPONSE_SIZE` in m24 |
| Frozen tolerance (ME) | 0.003 | `FROZEN_TOLERANCE` in m23 |
| Frozen threshold | 3 polls | `FROZEN_THRESHOLD` in m23 |
| K_MOD_BUDGET_MIN | 0.85 | m04_constants |
| K_MOD_BUDGET_MAX | 1.15 | m04_constants |
| HEBBIAN_WEIGHT_FLOOR | 0.15 | m04_constants |
| HEBBIAN_LTP | 0.01 | m04_constants |
| HEBBIAN_LTD | 0.002 | m04_constants |
| MAX_GHOSTS (in-memory) | 20 | m10_hook_server |
| MAX_GHOSTS (SQLite) | 100 | add_ghost() prune |
| RING_LINE_CAP (WASM) | 1,000 | m30_wasm_bridge |
| MAX_FRAME_SIZE (wire) | 65,536 bytes | m09_wire_protocol |
| DEFAULT_KEEPALIVE_SECS | 30 | m09_wire_protocol |
