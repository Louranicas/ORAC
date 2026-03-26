# D3: Endpoint & Protocol Catalog

> **ORAC Sidecar** -- 22 HTTP routes, 11-variant BusFrame, 6 bridges, 5 WASM commands
>
> **Source of truth:** `src/m3_hooks/m10_hook_server.rs` (router in `build_router()`), `src/m2_wire/m08_bus_types.rs`, `src/m6_coordination/m30_wasm_bridge.rs`

---

## 1. HTTP Endpoints (22 Routes)

All routes are defined in `m10_hook_server::build_router()` (line 2247). The server listens on port **8133** via Axum. State is shared through `Arc<OracState>`.

### 1.1 Diagnostic Endpoints (5 GET)

#### GET /health

| Field | Value |
|-------|-------|
| Handler | `health_handler` |
| Purpose | Liveness probe returning comprehensive system snapshot |
| Request | None (no body, no query params) |
| Response | `HealthResponse` JSON (30+ fields across 6 feature gates) |
| Side effects | None (read-only) |
| Error responses | Always 200 (never fails) |

Response schema (abbreviated -- feature-gated fields included when compiled with `--features full`):

```json
{
  "status": "healthy",
  "service": "orac-sidecar",
  "version": "0.10.0",
  "port": 8133,
  "sessions": 2,
  "uptime_ticks": 1754,
  "field_r": 0.948,
  "sphere_count": 44,
  "ralph_gen": 1754,
  "ralph_phase": "Learn",
  "ralph_fitness": 0.735,
  "ralph_converged": false,
  "ipc_state": "subscribed",
  "breakers": {
    "pv2": {"state": "Closed", "failures": 0, "successes": 150, "consecutive_failures": 0},
    "synthex": {"state": "Closed", "failures": 0, "successes": 42, "consecutive_failures": 0},
    "me": {"state": "Closed", "failures": 0, "successes": 30, "consecutive_failures": 0},
    "povm": {"state": "Closed", "failures": 0, "successes": 12, "consecutive_failures": 0},
    "rm": {"state": "Closed", "failures": 0, "successes": 28, "consecutive_failures": 0},
    "vms": {"state": "Closed", "failures": 0, "successes": 15, "consecutive_failures": 0}
  },
  "thermal_temperature": 0.838,
  "thermal_target": 0.5,
  "me_fitness": 0.609,
  "me_frozen": false,
  "me_observer_subscribed": true,
  "me_total_correlations": 4160000,
  "me_total_events": 368000,
  "dispatch_total": 512,
  "coupling_connections": 1892,
  "coupling_weight_mean": 0.42,
  "coupling_weight_range": [0.15, 1.0],
  "co_activations_total": 342,
  "hebbian_ltp_total": 342,
  "hebbian_ltd_total": 6200,
  "hebbian_last_tick": 1753,
  "emergence_events": 243,
  "emergence_active_monitors": 3,
  "synthex_stale": false,
  "rm_stale": false
}
```

#### GET /field

| Field | Value |
|-------|-------|
| Handler | `field_handler` |
| Purpose | Cached Kuramoto field state enriched with live PV2 k/k_mod |
| Request | None |
| Response | Dynamic JSON |
| Side effects | Makes live HTTP GET to PV2 `/health` for k/k_mod enrichment |
| Error responses | Falls back to cache-only if PV2 unreachable (still 200) |

Response schema:

```json
{
  "source": "cache_enriched",
  "tick": 1754,
  "r": 0.948,
  "sphere_count": 44,
  "pv2_tick": 3200,
  "stale": false,
  "k": 2.1,
  "k_mod": 1.05,
  "emergence": {
    "total_detected": 243,
    "active_monitors": 3,
    "history_len": 95,
    "by_type": {"CoherenceLock": 12, "BeneficialSync": 45, "FieldStability": 38},
    "recent": [{"type": "BeneficialSync", "severity": "Moderate", "confidence": 0.85}]
  }
}
```

#### GET /thermal

| Field | Value |
|-------|-------|
| Handler | `thermal_handler` |
| Purpose | Cached SYNTHEX thermal state with ORAC bridge metadata |
| Request | None |
| Feature gate | `bridges` (returns error JSON if disabled) |
| Response | Dynamic JSON |
| Side effects | None (reads cached bridge state) |
| Error responses | 200 with `"error"` field if no data cached |

Response schema:

```json
{
  "source": "orac_cache",
  "temperature": 0.838,
  "target": 0.5,
  "pid_output": 0.136,
  "heat_sources": [
    {"id": "HS-001", "reading": 1.0, "weight": 0.3},
    {"id": "HS-002", "reading": 0.0, "weight": 0.35}
  ],
  "k_adjustment": 0.932,
  "bridge_consecutive_failures": 0,
  "last_poll_tick": 1750,
  "orac_tick": 1754
}
```

#### GET /metrics

| Field | Value |
|-------|-------|
| Handler | `metrics_handler` |
| Purpose | Prometheus text format metrics |
| Request | None |
| Response | `text/plain; version=0.0.4; charset=utf-8` |
| Side effects | None |
| Error responses | Always 200 |

Exported metric families:

- `orac_sessions_active` (gauge)
- `orac_uptime_ticks` (counter)
- `orac_panes_by_status{status="Idle|Working|Blocked|Complete"}` (gauge)
- `orac_tasks_completed_total` (counter)
- `orac_task_history_total` (counter)
- `orac_agent_cards_registered` (gauge)
- `orac_dispatch_total` (counter)
- `orac_dispatch_by_domain{domain="Read|Write|Execute|Communicate"}` (counter)
- `orac_breaker_state{bridge="pv2|synthex|me|povm|rm|vms"}` (gauge: 0=Closed, 1=Open, 2=HalfOpen)
- `orac_breaker_failures_total{bridge="..."}` (counter)
- `orac_ralph_generation` (counter)
- `orac_ralph_completed_cycles` (counter)
- `orac_ralph_fitness` (gauge)
- `orac_ralph_peak_fitness` (gauge)
- `orac_ralph_paused` (gauge: 0 or 1)
- `orac_ralph_mutations_total{outcome="accepted|rolled_back|skipped"}` (counter)
- `orac_emergence_detected_total` (counter)
- `orac_emergence_active_monitors` (gauge)

#### GET /field/ghosts

| Field | Value |
|-------|-------|
| Handler | `field_ghosts_handler` |
| Purpose | ORAC-local ghost ring buffer of deregistered spheres |
| Request | None |
| Response | JSON with `ghosts` array and `count` |
| Side effects | None |
| Error responses | Always 200 (empty array if no ghosts) |

Response schema:

```json
{
  "ghosts": [
    {
      "sphere_id": "hostname:pid:abcd1234",
      "persona": "orac-agent",
      "deregistered_ms": 1711100000000,
      "final_phase": 2.14,
      "total_tools": 42,
      "session_duration_ms": 300000
    }
  ],
  "count": 1
}
```

In-memory ring: FIFO, max 20 entries (`MAX_GHOSTS`). Also persisted to SQLite `ghost_traces` table (max 100).

### 1.2 Feature Endpoints (10 GET)

#### GET /traces

| Field | Value |
|-------|-------|
| Handler | `traces_handler` |
| Feature gate | `monitoring` |
| Purpose | OTel-style trace store query (recent spans) |
| Response | JSON with `summary` stats and `recent_spans` array (max 50) |

#### GET /dashboard

| Field | Value |
|-------|-------|
| Handler | `dashboard_endpoint_handler` |
| Feature gate | `monitoring` |
| Purpose | Kuramoto field dashboard snapshot |
| Response | JSON with order parameter history, clusters, chimera detection, r trend stats |

#### GET /tokens

| Field | Value |
|-------|-------|
| Handler | `tokens_handler` |
| Feature gate | `monitoring` |
| Purpose | Token accounting summary (fleet totals, per-pane, budget) |
| Response | JSON with input/output totals, pane count, budget status |

#### GET /coupling

| Field | Value |
|-------|-------|
| Handler | `coupling_handler` |
| Purpose | Coupling network stats |
| Response | JSON with connection count, weight distribution (min/max/mean), saturation metrics |

#### GET /hebbian

| Field | Value |
|-------|-------|
| Handler | `hebbian_handler` |
| Purpose | Hebbian STDP stats |
| Response | JSON with `ltp_total`, `ltd_total`, `co_activations`, `last_tick`, `ltp_ltd_ratio` |

#### GET /emergence

| Field | Value |
|-------|-------|
| Handler | `emergence_handler` |
| Feature gate | `evolution` |
| Purpose | Emergence detector stats |
| Response | JSON with event counts by type, active monitor count, recent events |

#### GET /bridges

| Field | Value |
|-------|-------|
| Handler | `bridges_handler` |
| Feature gate | `intelligence` (for breaker data) |
| Purpose | Bridge status: circuit breaker states, last poll ticks |
| Response | JSON with per-bridge status |

#### GET /ralph

| Field | Value |
|-------|-------|
| Handler | `ralph_handler` |
| Feature gate | `evolution` |
| Purpose | RALPH evolution status |
| Response | JSON with generation, fitness, phase, mutations, convergence |

Response schema:

```json
{
  "generation": 1754,
  "phase": "Learn",
  "fitness": 0.735,
  "peak_fitness": 0.75,
  "paused": false,
  "completed_cycles": 350,
  "mutations": {
    "proposed": 400,
    "accepted": 120,
    "rolled_back": 80,
    "skipped": 200
  }
}
```

#### GET /dispatch

| Field | Value |
|-------|-------|
| Handler | `dispatch_handler` |
| Purpose | Semantic routing dispatch stats |
| Response | JSON with total dispatches, per-domain counts, router readiness |

#### GET /blackboard

| Field | Value |
|-------|-------|
| Handler | `blackboard_handler` |
| Purpose | Session tracking + persistent fleet state from SQLite |
| Query params | `?status=Working` (filter by pane status), `?since=1711100000` (filter by update time), `?task_limit=50` (max recent tasks, default 20) |
| Feature gate | `persistence` (for SQLite data) |
| Response | JSON with sessions, pane_status, agent_cards, recent_tasks, ralph snapshot |

Response schema:

```json
{
  "sessions": [{"session_id": "sess-uuid", "pane_id": "host:pid:uuid", "active_task": null, "poll_counter": 42}],
  "fleet_size": 3,
  "uptime_ticks": 1754,
  "pane_status": [{"pane_id": "...", "status": "Working", "persona": "orac-agent", "tasks_completed": 5}],
  "pane_count": 3,
  "agent_cards": [{"pane_id": "...", "capabilities": ["read","write","execute","search"], "domain": "general", "model": "claude-opus-4-6"}],
  "recent_tasks": [{"task_id": "...", "pane_id": "...", "outcome": "completed", "duration_secs": 12.5}],
  "task_count": 20,
  "ralph": {"generation": 1754, "phase": "Learn", "fitness": 0.735}
}
```

### 1.3 Hook Endpoints (6 POST)

All hooks receive `HookEvent` and return `HookResponse`. Hook paths use **PascalCase** (not snake_case).

#### HookEvent (request body for all hooks)

```json
{
  "session_id": "sess-uuid-string",
  "tool_name": "Read",
  "tool_input": {"file_path": "/some/file.rs"},
  "tool_output": "file contents...",
  "prompt": "user prompt text"
}
```

All 5 fields are optional (`#[serde(default)]`). Each handler extracts the fields it needs.

#### HookResponse (response body for all hooks)

Serialized with `camelCase` (`#[serde(rename_all = "camelCase")]`):

```json
{
  "systemMessage": "optional context injection string",
  "decision": "allow",
  "reason": "optional block reason"
}
```

All 3 fields use `skip_serializing_if = "Option::is_none"`.

---

#### POST /hooks/SessionStart

| Field | Value |
|-------|-------|
| Handler | `m11_session_hooks::handle_session_start` |
| Timeout | 5s |
| Fields consumed | `session_id` |
| Fields returned | `systemMessage` (hydration summary) |
| Consent checks | `hydration` (gates POVM + RM reads) |
| Bridge calls | PV2 POST `/sphere/{id}/register` (fire-and-forget), POVM GET `/hydrate`, RM GET `/search?q=discovery` |
| Side effects | Registers sphere in coupling network, creates session tracker, upserts blackboard `pane_status` + `agent_cards` |

#### POST /hooks/Stop

| Field | Value |
|-------|-------|
| Handler | `m11_session_hooks::handle_stop` |
| Timeout | 5s |
| Fields consumed | `session_id` |
| Fields returned | `systemMessage` (empty or deregistration confirmation) |
| Consent checks | `povm_write` (gates POVM snapshot), `rm_write` (gates RM crystallize) |
| Bridge calls | PV2 POST `/bus/fail/{task_id}` (if active task), PV2 POST `/sphere/{id}/status`, POVM POST `/snapshots` (breaker-gated), RM POST `/put` (breaker-gated, awaited with 3s timeout), PV2 POST `/sphere/{id}/deregister` |
| Side effects | Removes session tracker, creates ghost trace (in-memory ring + SQLite), removes blackboard `pane_status` |

#### POST /hooks/PostToolUse

| Field | Value |
|-------|-------|
| Handler | `m12_tool_hooks::handle_post_tool_use` |
| Timeout | 3s |
| Fields consumed | `tool_name`, `tool_input`, `tool_output` |
| Fields returned | `systemMessage` (task injection if claimed) |
| Consent checks | None (tool hooks use breaker gating only) |
| Bridge calls | PV2 POST `/sphere/{id}/memory` (breaker-gated), PV2 POST `/sphere/{id}/status` (breaker-gated), PV2 GET `/bus/tasks` (throttled 1-in-5), PV2 POST `/bus/claim/{task_id}` (atomic claim) |
| Side effects | Increments `total_tool_calls`, records semantic dispatch, records OTel trace span, records token usage estimate (chars/4), upserts blackboard `pane_status`, records task completion in `task_history` if `TASK_COMPLETE` detected |

#### POST /hooks/PreToolUse

| Field | Value |
|-------|-------|
| Handler | `m12_tool_hooks::handle_pre_tool_use` |
| Timeout | 2s |
| Fields consumed | `tool_name`, `tool_input` |
| Fields returned | `decision` ("allow" always -- thermal warnings in `systemMessage`) |
| Consent checks | None |
| Bridge calls | SYNTHEX cached thermal state (no live call -- uses bridge cache) |
| Side effects | None (read-only thermal check) |

Thermal gate logic: Warns if temperature > 30% above target for write operations (`Edit`, `Write`, `Bash`). Never blocks -- advisory only.

#### POST /hooks/UserPromptSubmit

| Field | Value |
|-------|-------|
| Handler | `m13_prompt_hooks::handle_user_prompt_submit` |
| Timeout | 3s |
| Fields consumed | `prompt` |
| Fields returned | `systemMessage` (field state + pending tasks) |
| Consent checks | None |
| Bridge calls | SYNTHEX GET `/v3/thermal` (live), PV2 GET `/bus/tasks` (live, breaker-gated) |
| Side effects | Advances breaker tick, periodic blackboard GC (every 720 ticks: prunes complete panes + old tasks after 24h) |

Skips short prompts (< 20 chars) with empty response.

#### POST /hooks/PermissionRequest

| Field | Value |
|-------|-------|
| Handler | `m14_permission_policy::handle_permission_request` |
| Timeout | 2s |
| Fields consumed | `tool_name` |
| Fields returned | `decision` ("allow" or "block"), optional `reason` |
| Consent checks | None |
| Bridge calls | None |
| Side effects | None (stateless policy evaluation) |

Policy rules:

| Category | Tools | Decision |
|----------|-------|----------|
| Always approve | Read, Glob, Grep, LS, Agent, WebSearch, WebFetch, TodoRead, TodoWrite | Allow |
| Approve with notice | Edit, Write, Bash, NotebookEdit | AllowWithNotice |
| Always deny | (configurable, empty by default) | Deny |
| Default | All others | Allow (permissive fleet policy) |

### 1.4 Consent Endpoint (1 GET + 1 PUT)

#### GET /consent/{sphere_id}

| Field | Value |
|-------|-------|
| Handler | `consent_get_handler` |
| Purpose | Read consent declarations for a sphere |
| Path param | `sphere_id` (string) |
| Response | `OracConsent` JSON |
| Side effects | Creates fully-open default if absent |

#### PUT /consent/{sphere_id}

| Field | Value |
|-------|-------|
| Handler | `consent_put_handler` |
| Purpose | Update consent fields (partial update) |
| Path param | `sphere_id` (string) |
| Request body | `ConsentUpdateRequest` JSON (all fields optional) |
| Response | JSON with `updated` field list |
| Side effects | Persists audit trail to blackboard `consent_audit` table |

---

## 2. Hook Event Schemas

### HookEvent Struct

Defined in `m10_hook_server.rs` (line 222):

| Field | Type | Present in |
|-------|------|-----------|
| `session_id` | `Option<String>` | SessionStart, Stop |
| `tool_name` | `Option<String>` | PostToolUse, PreToolUse, PermissionRequest |
| `tool_input` | `Option<serde_json::Value>` | PostToolUse, PreToolUse, PermissionRequest |
| `tool_output` | `Option<String>` | PostToolUse |
| `prompt` | `Option<String>` | UserPromptSubmit |

### HookResponse Struct

Defined in `m10_hook_server.rs` (line 249), serde `rename_all = "camelCase"`:

| Field | JSON key | Type | Used by |
|-------|----------|------|---------|
| `system_message` | `systemMessage` | `Option<String>` | All hooks |
| `decision` | `decision` | `Option<String>` | PreToolUse, PermissionRequest |
| `reason` | `reason` | `Option<String>` | PermissionRequest (block reason) |

### Per-Hook Field Consumption

| Hook | session_id | tool_name | tool_input | tool_output | prompt |
|------|-----------|-----------|------------|-------------|--------|
| SessionStart | YES | - | - | - | - |
| Stop | YES | - | - | - | - |
| PostToolUse | (via session lookup) | YES | YES (truncated to 200 chars) | YES (TASK_COMPLETE check) | - |
| PreToolUse | - | YES (write check) | YES (summary) | - | - |
| UserPromptSubmit | - | - | - | - | YES (skip if < 20 chars) |
| PermissionRequest | - | YES (policy lookup) | - | - | - |

---

## 3. Wire Protocol

### BusFrame Enum (11 Variants)

Defined in `m2_wire/m08_bus_types.rs` (line 275). NDJSON wire format with `#[serde(tag = "type")]` internally-tagged enum.

| # | Variant | Direction | Fields | Purpose |
|---|---------|-----------|--------|---------|
| 1 | `Handshake` | Client -> Server | `pane_id: PaneId`, `version: String` | Initial identity registration |
| 2 | `Welcome` | Server -> Client | `session_id: String`, `version: String` | Handshake accepted |
| 3 | `Subscribe` | Client -> Server | `patterns: Vec<String>` | Subscribe to event patterns |
| 4 | `Subscribed` | Server -> Client | `count: usize` | Subscription confirmed |
| 5 | `Submit` | Client -> Server | `task: BusTask` | Submit fleet task |
| 6 | `TaskSubmitted` | Server -> Client | `task_id: TaskId` | Task submission acknowledged |
| 7 | `Event` | Server -> Client | `event: BusEvent` | Event notification (subscription match) |
| 8 | `Cascade` | Bidirectional | `source: PaneId`, `target: PaneId`, `brief: String` | Cascade handoff request |
| 9 | `CascadeAck` | Bidirectional | `source: PaneId`, `target: PaneId`, `accepted: bool` | Cascade acknowledgement |
| 10 | `Disconnect` | Client -> Server | `reason: String` | Graceful disconnect |
| 11 | `Error` | Server -> Client | `code: u16`, `message: String` | Error notification |

Client-originated frames: `Handshake`, `Subscribe`, `Submit`, `Disconnect`.
Server-originated frames: `Welcome`, `Subscribed`, `TaskSubmitted`, `Event`, `Error`.
Bidirectional frames: `Cascade`, `CascadeAck`.

### ProtocolState FSM (6 States)

Defined in `m2_wire/m09_wire_protocol.rs` (line 60):

```
Disconnected --> Handshaking --> Connected --> Subscribing --> Active
     ^                                                          |
     +----------------------- (error/timeout) ------------------+
                                                                |
                                                           Closing
```

| State | can_send_data | can_receive_events | is_terminal |
|-------|--------------|-------------------|-------------|
| Disconnected | false | false | true |
| Handshaking | false | false | false |
| Connected | false | true | false |
| Subscribing | false | true | false |
| Active | true | true | false |
| Closing | false | false | true |

### Wire Protocol Constants

| Constant | Value | Location |
|----------|-------|----------|
| `PROTOCOL_VERSION` | `"2.0"` | `m09_wire_protocol.rs` |
| `DEFAULT_KEEPALIVE_SECS` | 30 | `m09_wire_protocol.rs` |
| `MAX_FRAME_SIZE` | 65,536 bytes (64 KB) | `m09_wire_protocol.rs` |
| `MAX_SEND_QUEUE` | 1,000 frames | `m09_wire_protocol.rs` |
| `MAX_RECV_BUFFER` | 500 frames | `m09_wire_protocol.rs` |

### Event Pattern Matching

`BusEvent::matches_pattern()` supports glob-style patterns:

| Pattern | Matches |
|---------|---------|
| `"*"` | Everything |
| `"field.*"` | Trailing wildcard (starts-with) |
| `"*.tick"` | Leading wildcard (ends-with) |
| `"field.*.tick"` | Mid-string wildcard (starts-with AND ends-with) |
| `"field.tick"` | Exact string equality |

### Frame Validation

`FrameValidation` enum:

| Variant | Meaning |
|---------|---------|
| `Valid` | Frame valid for current protocol state |
| `UnexpectedFrame { frame_type, state }` | Wrong frame for current state |
| `Oversized { size }` | Exceeds `MAX_FRAME_SIZE` |
| `Malformed { reason }` | Deserialization failed |

### BusTask Lifecycle

```
Pending --> Claimed --> Completed
                   \-> Failed
Claimed --> Pending (requeue, increments requeue_count)
```

`TaskTarget` variants: `Specific { pane_id }`, `AnyIdle` (default), `FieldDriven`, `Willing`.

---

## 4. WASM Bridge

Defined in `m6_coordination/m30_wasm_bridge.rs`.

### Paths

| Path | Direction | Format |
|------|-----------|--------|
| `/tmp/swarm-commands.pipe` | WASM -> ORAC | Named FIFO pipe |
| `/tmp/swarm-events.jsonl` | ORAC -> WASM | Ring-buffered JSONL file |

### Commands (5)

`WasmCommand` enum, tagged with `#[serde(tag = "cmd")]`:

| Command | Fields | Purpose |
|---------|--------|---------|
| `dispatch` | `pane: String`, `task: String` | Dispatch task to a pane |
| `status` | (none) | Request fleet status |
| `field_state` | (none) | Request field state (r, K, phases) |
| `list_panes` | (none) | Request pane list |
| `ping` | (none) | Keepalive |

### Event Types

`WasmEvent` struct with `event`, `tick`, `data` fields. Factory methods:

| Factory | Event tag | Data |
|---------|-----------|------|
| `tick_event(tick, r, k)` | `"tick"` | `{"r": 0.95, "k": 2.1}` |
| `task_completed(tick, task_id, pane)` | `"task_completed"` | `{"task_id": "abc", "pane": "fleet-1"}` |
| `pong(tick)` | `"pong"` | `null` |

### Ring Buffer

`EventRingBuffer`: In-memory `VecDeque<String>` with FIFO eviction.

| Constant | Value |
|----------|-------|
| `RING_LINE_CAP` | 1,000 lines |
| `MAX_COMMAND_LEN` | 8,192 bytes |
| `MAX_EVENT_LEN` | 8,192 bytes |

---

## 5. Bridge Client APIs

All bridges use raw `TcpStream` HTTP (no HTTP library). Addresses are raw `host:port` (no `http://` prefix, BUG-033). Default TCP timeout: 2,000ms. Default max response: 32,768 bytes.

### 5.1 SYNTHEX Bridge (M22)

| Target | `127.0.0.1:8090` |
|--------|------------------|

| Call | Method | URL | Payload | Response | Cadence | Consent |
|------|--------|-----|---------|----------|---------|---------|
| Health check | GET | `/api/health` | - | 200 OK | on demand | none |
| Poll thermal | GET | `/v3/thermal` | - | `ThermalResponse` JSON | every 6 ticks (configurable) | none |
| Post field state | POST | `/api/ingest` | field state bytes | ignored | every 6 ticks (from main.rs) | `synthex_write` |

`ThermalResponse`: `{ temperature, target, pid_output, heat_sources: [{id, reading, weight}] }`

Thermal adjustment formula: `(1.0 - (temperature - target) * 0.2).clamp(K_MOD_BUDGET_MIN, K_MOD_BUDGET_MAX)` where budget is `[0.85, 1.15]`.

### 5.2 ME Bridge (M23)

| Target | `127.0.0.1:8080` |
|--------|------------------|

| Call | Method | URL | Payload | Response | Cadence | Consent |
|------|--------|-----|---------|----------|---------|---------|
| Health check | GET | `/api/health` | - | 200 OK | on demand | none |
| Poll observer | GET | `/api/observer` | - | `RawObserverResponse` JSON | every 12 ticks | none |

`RawObserverResponse` nests fitness under `last_report.current_fitness`. Bridge flattens to `ObserverResponse`.

BUG-008 frozen detection: 3 consecutive readings within `FROZEN_TOLERANCE` (0.003) triggers `is_frozen` flag. Known frozen value: `0.3662`. When frozen, returns neutral adjustment (1.0).

Post is a no-op (`post()` returns `Ok(())` -- ME bridge is read-only).

### 5.3 POVM Bridge (M24)

| Target | `127.0.0.1:8125` |
|--------|------------------|

| Call | Method | URL | Payload | Response | Cadence | Consent |
|------|--------|-----|---------|----------|---------|---------|
| Health check | GET | `/health` | - | 200 OK | on demand | none |
| Snapshot write | POST | `/memories` | sphere snapshot JSON | ignored | every 12 ticks | `povm_write` |
| Pathway read | GET | `/pathways` | - | `PathwaysResponse` JSON | every 60 ticks | `povm_read` |
| Summary read | GET | `/summary` | - | `PovmSummary` JSON | startup only | none |
| Hydration | GET | `/hydrate` | - | hydration data | SessionStart only | `hydration` |

Max response size for POVM: **2 MB** (raised from 512 KB -- Gen-060a). POVM pathway responses grow with STDP activity and measured 1.3 MB in production (2,437+ pathways).

`Pathway` struct supports dual format via serde aliases: `source`/`target` (ORAC-written) and `pre_id`/`post_id` (POVM native).

### 5.4 RM Bridge (M25)

| Target | `127.0.0.1:8130` |
|--------|------------------|

| Call | Method | URL | Payload | Response | Cadence | Consent |
|------|--------|-----|---------|----------|---------|---------|
| Health check | GET | `/health` | - | 200 OK | on demand | none |
| Post record | POST | `/put` | **TSV** (Content-Type: `text/tab-separated-values`) | ignored | every 60 ticks | `rm_write` |
| Search | GET | `/search?q=discovery` | - | `RmSearchResult` JSON | SessionStart only | `hydration` |

**CRITICAL: RM accepts TSV ONLY -- JSON causes parse failures.**

TSV format: `category\tagent\tconfidence\tttl\tcontent`

- Agent name: `"orac-sidecar"` (not `"pane-vortex"`)
- Default field_state TTL: 300 seconds
- Content is sanitized: tabs and newlines replaced with spaces (BUG-L002 single-pass)

### 5.5 VMS Bridge (via HTTP helpers)

| Target | `127.0.0.1:8120` |
|--------|------------------|

VMS communication happens through `http_post`/`http_get` helpers in `main.rs`, not a dedicated bridge module.

| Call | Method | URL | Payload | Cadence |
|------|--------|-----|---------|---------|
| Post observations | POST | `/v1/observations` | JSON (r, fitness, sphere_count, tick) | every 30 ticks |
| Trigger consolidation | POST | `/v1/adaptation/trigger` | empty JSON | every 300 ticks |
| Semantic query | GET | `/v1/memories?limit=5` | - | every 30 ticks during Recognize phase |

### 5.6 PV2 Bridge (via ureq HTTP)

| Target | `http://127.0.0.1:8132` |
|--------|-------------------------|

PV2 communication uses `ureq` via `http_get`/`fire_and_forget_post`/`breaker_guarded_post` -- not raw TCP.

| Call | Method | URL | Purpose | Cadence |
|------|--------|-----|---------|---------|
| Health poll | GET | `/health` | Field state cache update | every 5s (spawn_field_poller) |
| Sphere list | GET | `/spheres` | Coupling network sync | every 5s (spawn_field_poller) |
| Register sphere | POST | `/sphere/{id}/register` | Session registration | SessionStart |
| Record memory | POST | `/sphere/{id}/memory` | Tool memory recording | PostToolUse |
| Update status | POST | `/sphere/{id}/status` | Sphere status update | PostToolUse, Stop |
| Deregister | POST | `/sphere/{id}/deregister` | Session teardown | Stop |
| Poll tasks | GET | `/bus/tasks` | Pending task discovery | PostToolUse (1-in-5), UserPromptSubmit |
| Claim task | POST | `/bus/claim/{task_id}` | Atomic task claim | PostToolUse |
| Complete task | POST | `/bus/complete/{task_id}` | Task completion | PostToolUse (TASK_COMPLETE detected) |
| Fail task | POST | `/bus/fail/{task_id}` | Task failure | Stop (active task cleanup) |

PV2 IPC bus socket path: `/run/user/1000/pane-vortex-bus.sock`

---

## 6. Consent Model

### OracConsent Struct

Defined in `m10_hook_server.rs` (line 117):

| Field | Type | Default | Purpose |
|-------|------|---------|---------|
| `synthex_write` | `bool` | `true` | Allow SYNTHEX bridge writes |
| `povm_read` | `bool` | `true` | Allow POVM bridge reads |
| `povm_write` | `bool` | **`false`** | Allow POVM bridge writes (opt-in only) |
| `rm_write` | `bool` | `true` | Allow RM bridge writes |
| `hydration` | `bool` | `true` | Allow session hydration from POVM + RM |
| `updated_ms` | `u64` | `0` | Last update timestamp (0 = never explicitly updated) |

`fully_open()` creates default consent: all true except `povm_write` (false). `updated_ms` = 0 (sentinel for uncommitted defaults).

### Handler -> Consent Field Mapping

| Handler | Consent field checked | Effect if denied |
|---------|----------------------|------------------|
| `handle_session_start` | `hydration` | POVM + RM hydration skipped |
| `handle_stop` | `povm_write` | POVM snapshot skipped |
| `handle_stop` | `rm_write` | RM crystallize skipped |
| `post_field_to_synthex` (main.rs) | `synthex_write` | SYNTHEX ingest skipped |
| `persist_stdp_to_povm` (main.rs) | `povm_write` | POVM pathway write skipped |

### ConsentUpdateRequest

Partial update -- only specified fields are changed:

```json
{
  "synthex_write": true,
  "povm_read": false,
  "povm_write": true,
  "hydration": false
}
```

All fields are `Option<bool>`. Absent fields are not modified. Updates are audit-logged to the `consent_audit` blackboard table.
