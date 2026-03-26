# V_L5: Layer 5 Bridges — Verified Source Analysis

> **7,034 LOC | 339 tests | 6 modules | 5 external services + 1 local SQLite**
> **Feature Gate:** `bridges` | **Layer Dependencies:** L1 Core
> **Verified:** 2026-03-25 from source (not D7 — D7 used only for initial cross-reference)

---

## Critical Verifications

### PASS: Blackboard has exactly 10 CREATE TABLE statements

| # | Table | Primary Key | Columns | Purpose |
|---|-------|-------------|---------|---------|
| 1 | `pane_status` | `pane_id TEXT` | 6 cols (pane_id, status, persona, updated_at, phase, tasks_completed) | Per-pane state singleton |
| 2 | `task_history` | `task_id TEXT` | 6 cols (task_id, pane_id, description, outcome, finished_at, duration_secs) | Task completion log |
| 3 | `agent_cards` | `pane_id TEXT` | 5 cols (pane_id, capabilities, domain, model, registered_at) | A2A capability declarations |
| 4 | `ghost_traces` | None (multi-entry) | 6 cols (sphere_id, persona, deregistered_ms, final_phase, total_tools, session_duration_ms) | Deregistration records |
| 5 | `consent_declarations` | `sphere_id TEXT` | 6 cols (sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms) | Per-sphere consent |
| 6 | `consent_audit` | None (append-only) | 5 cols (sphere_id, field_name, old_value, new_value, changed_ms) | Consent change trail |
| 7 | `hebbian_summary` | `id INTEGER AUTOINCREMENT` | 11 cols (id, tick, ltp_count, ltd_count, at_floor_count, total_weight_change, connections_total, weight_mean, weight_min, weight_max, created_at) | STDP snapshot log |
| 8 | `ralph_state` | `id INTEGER CHECK(id=1)` | 10 cols (id, generation, completed_cycles, current_fitness, peak_fitness, total_proposed, total_accepted, total_rolled_back, last_phase, updated_at) | RALPH singleton |
| 9 | `sessions` | `session_id TEXT` | 8 cols (session_id, pane_id, active_task_id, poll_counter, total_tool_calls, started_ms, persona, updated_at) | Active session tracking |
| 10 | `coupling_weights` | `(from_id, to_id) COMPOSITE` | 4 cols (from_id, to_id, weight, updated_at) | Hebbian weight persistence |

**6 secondary indexes:** `idx_task_history_pane`, `idx_task_history_finished`, `idx_ghost_traces_time`, `idx_consent_audit_sphere`, `idx_consent_audit_time`, `idx_hebbian_summary_tick`

**Schema is idempotent:** All use `CREATE TABLE IF NOT EXISTS`, executed in single `execute_batch()` call.

### PASS: RM Bridge uses TSV, not JSON

| Criterion | Status | Evidence |
|-----------|--------|---------|
| `to_tsv()` produces tab-separated output | PASS | `category\tagent\tconfidence\tttl\tcontent` — exactly 4 tabs, 5 fields |
| `from_tsv()` parses tab-separated input | PASS | Splits by `\t`, validates 5+ fields |
| POST calls `raw_http_post_tsv()` | PASS | 3 call sites (post_record, post_records, Bridgeable::post) |
| Content-Type header | PASS | `text/tab-separated-values` |
| No JSON for data serialization | PASS | `serde_json` only for search response *reading* (defensive) |
| Sanitizer strips tabs/newlines | PASS | `sanitize_into()` replaces `\t`, `\n`, `\r` with space |
| Module doc explicitly warns | PASS | Line 3-4: `//! **NEVER JSON** -- TSV only!` |

---

## Module: http_helpers

**Purpose:** Shared raw TCP HTTP client for all bridge modules. Zero external HTTP dependencies.

### Implementation Approach

Raw `std::net::TcpStream` — no hyper, no reqwest, no ureq. Manual HTTP/1.1 request construction, manual response parsing, manual chunked transfer-encoding dechunking.

### Constants

| Constant | Value | Notes |
|----------|-------|-------|
| `DEFAULT_TCP_TIMEOUT_MS` | 2000 | 2s connect/read timeout |
| `DEFAULT_MAX_RESPONSE_SIZE` | 32,768 | 32KB default (raised from 8KB in BUG-060i) |

### Public API (10 functions)

| Function | Signature | Content-Type | Returns |
|----------|-----------|-------------|---------|
| `raw_http_get` | `(addr, path, service) -> PvResult<String>` | — | Full body |
| `raw_http_get_with_limit` | `(addr, path, service, max_bytes) -> PvResult<String>` | — | Size-limited body |
| `raw_http_post` | `(addr, path, body: &[u8], service) -> PvResult<u16>` | `application/json` | Status code |
| `raw_http_post_with_response` | `(addr, path, body: &[u8], service) -> PvResult<String>` | `application/json` | Full body |
| `raw_http_post_tsv` | `(addr, path, tsv: &str, service) -> PvResult<u16>` | `text/tab-separated-values` | Status code |
| `extract_status_code` | `(raw) -> Option<u16>` | — | Parsed status |
| `extract_body` | `(raw) -> Option<String>` | — | Body after `\r\n\r\n` |
| `extract_headers` | `(raw) -> Option<&str>` | — | Header block |
| `is_chunked_transfer` | `(raw) -> bool` | — | Chunked detection |
| `dechunk_body` | `(chunked) -> Option<String>` | — | Dechunked body |

### Private Workhorse

```rust
fn raw_http_post_with_content_type(addr, path, body: &[u8], content_type, service) -> PvResult<u16>
```
Called by both `raw_http_post()` (JSON) and `raw_http_post_tsv()` (TSV).

### Error Types

| Error | Trigger |
|-------|---------|
| `PvError::BridgeUnreachable` | TCP connect/read/write failure, address parse failure |
| `PvError::BridgeError` | HTTP status >= 400, or status 0 (timeout/empty) |
| `PvError::BridgeParse` | No `\r\n\r\n` separator, malformed status line, bad chunked encoding |

### Fire-and-Forget in `raw_http_post_with_content_type`

The POST-status-only function deliberately drops the response body after reading the status line. After writing the request:
1. `let _ = stream.flush();` — flush ignored
2. Reads only status line into small buffer
3. Parses status code with `unwrap_or(0)`
4. Returns status code, discards body

This is by design for fire-and-forget POST scenarios where only acceptance matters.

### Chunked Transfer Handling

1. `is_chunked_transfer()` scans headers for `transfer-encoding: chunked` (case-insensitive)
2. `dechunk_body()` parses hex chunk sizes, concatenates data, terminates at size 0
3. Graceful truncation if chunk claims more bytes than available (takes what's there, breaks)

### Tests: 23

Status extraction (6), body extraction (4), header extraction (2), chunked detection (5), dechunking (6), connectivity failures (4 — GET/POST/TSV/large-limit all verify `BridgeUnreachable` on offline service).

---

## Module: m22_synthex_bridge

**Purpose:** SYNTHEX thermal read + Hebbian writeback. Port 8090.

### Key Types

```
SynthexBridge { service, base_url, poll_interval, state: RwLock<BridgeState> }
BridgeState   { last_poll_tick, cached_adjustment, stale, consecutive_failures, last_response }
ThermalResponse { temperature, target, pid_output, heat_sources: Vec<HeatSource> }
HeatSource    { id, reading, weight }
```

### Constants

| Constant | Value |
|----------|-------|
| `SYNTHEX_PORT` | 8090 |
| `DEFAULT_BASE_URL` | `"127.0.0.1:8090"` |
| `THERMAL_PATH` | `"/v3/thermal"` |
| `INGEST_PATH` | `"/api/ingest"` |
| `DEFAULT_POLL_INTERVAL` | 6 ticks |

### Core Flow

**Thermal Poll (`poll_thermal`):**
1. GET `/v3/thermal` via raw TCP
2. Parse JSON: `{ temperature, target, pid_output, heat_sources: [...] }`
3. Compute k_adjustment: `(1.0 - deviation * 0.2).clamp(K_MOD_BUDGET_MIN, K_MOD_BUDGET_MAX)` using FMA
4. NaN/INF guard: returns neutral 1.0 on invalid inputs, returns error on invalid output
5. Update RwLock state: cache adjustment, clear stale, reset failure counter

**k_adjustment Semantics:**
- Cold (temp < target) -> boost coupling (> 1.0)
- Hot (temp > target) -> reduce coupling (< 1.0)
- At target -> neutral 1.0

**Field State Post (`post_field_state`):**
- POST `/api/ingest` with raw byte payload — fire-and-forget (no response validation)

### BUG Fixes in This Module

- **BUG-033:** URLs stripped of `http://` prefix (raw socket)
- **BUG-M002:** First poll fires immediately (avoids startup delay)
- **BUG-H001:** NaN/INF validation before computation

### Bridgeable Trait

| Method | Behavior |
|--------|----------|
| `poll()` | `poll_thermal()` + `record_failure()` on error |
| `post(payload)` | `post_field_state(payload)` — fire-and-forget |
| `health()` | GET `/api/health`, returns `Ok(false)` on failure (never error) |
| `is_stale(tick)` | Explicit flag OR 2x poll_interval elapsed |

### Tests: 37

Construction (6), initial state (5), staleness (5), should_poll (3), failure tracking (3), ThermalResponse serde (5), HTTP extract (4), poll offline (3), adjustment clamping (5), thread safety (2), trait object (1), tick updates (2), constants (4), BridgeState default (1), multiple failures (1), interleaved ops (1), debug (2).

---

## Module: m23_me_bridge

**Purpose:** Maintenance Engine fitness signal read + frozen detection. Port 8080.

### Key Types

```
MeBridge              { service, base_url, poll_interval, state: RwLock<BridgeState> }
BridgeState           { last_poll_tick, cached_adjustment, stale, consecutive_failures,
                        last_fitness, frozen_count, is_frozen, last_response, successful_polls }
ObserverResponse      { fitness, active_layers, has_publishers, status, correlations_since,
                        emergences_since, total_correlations, total_events }
RawObserverResponse   { last_report: Option<RawLastReport>, system_state, enabled, metrics }
RawLastReport         { current_fitness, correlations_since_last, emergences_since_last }
RawMetrics            { correlations_found, emergences_detected, events_ingested }
```

### Constants

| Constant | Value | Notes |
|----------|-------|-------|
| `ME_PORT` | 8080 | |
| `DEFAULT_BASE_URL` | `"127.0.0.1:8080"` | |
| `OBSERVER_PATH` | `"/api/observer"` | |
| `DEFAULT_POLL_INTERVAL` | 12 ticks | |
| `BUG_008_FROZEN_FITNESS` | 0.3662 | Known stuck value |
| `FROZEN_TOLERANCE` | 0.003 | Widened from 0.001 (BUG-060b) |
| `FROZEN_THRESHOLD` | 3 | Consecutive identical readings |

### Core Flow

**Observer Poll (`poll_observer`):**
1. GET `/api/observer` via raw TCP
2. Parse into `RawObserverResponse` (handles nested `last_report.current_fitness` — GAP-7 fix)
3. Convert via `into_observer()` — flattens nested structure to `ObserverResponse`
4. Validate fitness: NaN/INF check, clamp [0.0, 1.0]
5. Frozen detection: count identical readings (tolerance 0.003), threshold at 3 consecutive
6. If frozen: return neutral 1.0. Otherwise: `fitness_to_adjustment(fitness)`
7. Update state: cache fitness, adjustment, response; increment successful_polls

**Fitness-to-Adjustment Formula:**
```
adj = K_MOD_BUDGET_MIN + fitness * (K_MOD_BUDGET_MAX - K_MOD_BUDGET_MIN)
```
Linear interpolation: fitness 0.0 -> dampen, fitness 0.5 -> neutral 1.0, fitness 1.0 -> boost.

**Frozen Detection (Two-Pronged):**
1. **Repeated readings:** `|fitness - last_fitness| < 0.003` for 3+ consecutive polls
2. **Known stuck value:** `|fitness - 0.3662| < 0.003` (BUG-008)

Either condition -> `is_frozen = true` -> return neutral 1.0 (avoids negative feedback loop).

### Bridgeable Trait

| Method | Behavior |
|--------|----------|
| `poll()` | `poll_observer()` + `record_failure()` on error |
| `post(payload)` | **No-op** — ME bridge is read-only (`Ok(())`) |
| `health()` | GET `/api/health`, returns `Ok(false)` on failure |
| `is_stale(tick)` | Explicit flag OR 2x poll_interval elapsed |

### Subscription Proxy

`successful_polls: u64` incremented on each successful poll. `is_subscribed()` returns `successful_polls > 0`. This serves as proxy evidence that ORAC is receiving ME fitness signals.

### Tests: 40

Construction (5), initial state (6), fitness_to_adjustment (6), BUG-008 frozen (4), staleness (3), should_poll (2), failure tracking (2), poll offline (2), health (1), post no-op (1), serde (5), thread safety (2), trait object (1), HTTP helpers (2), constants (4), debug (1), poll tick (1), frozen threshold (2), subscription (2).

---

## Module: m24_povm_bridge

**Purpose:** POVM Engine memory hydration + pathway crystallization. Port 8125.

### Key Types

```
PovmBridge          { service, base_url, write_interval, read_interval, state: RwLock<BridgeState> }
BridgeState         { last_write_tick, last_read_tick, cached_adjustment, stale, consecutive_failures,
                      cached_pathways, last_summary, hydrated }
Pathway             { source (#[alias="pre_id"]), target (#[alias="post_id"]),
                      weight, reinforcement_count (#[alias="co_activations"]) }
PovmSummary         { pathway_count, memory_count, uptime_secs }
PathwaysResponse    { pathways: Vec<Pathway> }
```

### Constants

| Constant | Value | Notes |
|----------|-------|-------|
| `POVM_PORT` | 8125 | |
| `MEMORIES_PATH` | `"/memories"` | Snapshot POST |
| `PATHWAYS_PATH` | `"/pathways"` | Weight read/write |
| `SUMMARY_PATH` | `"/summary"` | Summary read |
| `DEFAULT_WRITE_INTERVAL` | 12 ticks | |
| `DEFAULT_READ_INTERVAL` | 60 ticks | |
| `MAX_RESPONSE_SIZE` | 2,097,152 (2MB) | Raised from 512KB in Gen-060a |

### Core Flows

**Pathway Hydration (`hydrate_pathways`):**
1. GET `/pathways` with 2MB size limit
2. Dual-format parse: try `Vec<Pathway>` (POVM native array) first, fallback to `PathwaysResponse` (wrapped)
3. Serde aliases handle format divergence: `pre_id` -> `source`, `post_id` -> `target`, `co_activations` -> `reinforcement_count`
4. Cache pathways, mark hydrated, reset failures

**Pathway Crystallization (`write_pathways`):**
1. Input: `&[(String, String, f64, u64)]` — (source, target, weight, co_activations)
2. Serialize each to JSON: `{"pre_id": source, "post_id": target, "weight": w, "co_activations": c, "last_activated": "tick-N"}`
3. POST individually to `/pathways` (one per connection — POVM API doesn't support batch)
4. Per-pathway failures logged but not propagated (partial success model)
5. Returns count of successful upserts. Only updates tick if count > 0

**Memory Snapshot (`snapshot`):**
- POST `/memories` with raw byte payload — fire-and-forget

### Known Issue: POVM ID Mismatch

ORAC sphere IDs use format `orac-<hostname>:<pid>:<uuid>`. POVM pathway source/target fields use different ID formats. Result: 2,504 pathways hydrated but 0 matched coupling IDs. Bridge code is correct — issue is orchestration-layer ID scheme alignment.

### Bridgeable Trait

| Method | Behavior |
|--------|----------|
| `poll()` | Returns `cached_adjustment()` — always 1.0 (neutral, POVM doesn't adjust K) |
| `post(payload)` | `snapshot(payload)` + `record_failure()` on error |
| `health()` | GET `/health`, returns `Ok(false)` on failure |
| `is_stale(tick)` | Explicit flag OR 4x write_interval elapsed (accounts for read/write phase offset) |

### Tests: 56

Construction (5), initial state (6), poll (1), should_write/read (6), tick management (2), staleness (3), failure tracking (2), POST offline (2), Pathway serde (7), PathwaysResponse (2), PovmSummary (3), thread safety (2), trait object (1), HTTP helpers (2), constants (3), debug (1), hydration state (1), additional (2), write_pathways (5), coupling weight seeding (5).

---

## Module: m25_rm_bridge

**Purpose:** Reasoning Memory cross-session **TSV** persistence. Port 8130. **NEVER JSON.**

### Key Types

```
RmBridge        { service, base_url, poll_interval, state: RwLock<BridgeState> }
BridgeState     { last_poll_tick, cached_adjustment, stale, consecutive_failures,
                  records_posted, last_search_result }
RmRecord        { category, agent, confidence, ttl, content }
RmSearchResult  { entries: Vec<String>, total }
```

### Constants

| Constant | Value | Notes |
|----------|-------|-------|
| `RM_PORT` | 8130 | |
| `PUT_PATH` | `"/put"` | TSV POST endpoint |
| `SEARCH_PATH` | `"/search"` | Query endpoint |
| `DEFAULT_POLL_INTERVAL` | 30 ticks | |
| `DEFAULT_FIELD_STATE_TTL` | 300 seconds | |
| `DEFAULT_AGENT` | `"orac-sidecar"` | |

### TSV Format

**`to_tsv()` output:** `category\tagent\tconfidence\tttl\tcontent`

5 fields separated by 4 tab characters. `sanitize_into()` replaces any `\t`, `\n`, `\r` in field values with space before output.

**`from_tsv()` input:** Splits by `\t`, requires 5+ fields, clamps confidence to [0.0, 1.0], rejoins overflow tabs into content field.

### Convenience Constructors

| Constructor | Category | Agent | TTL |
|-------------|----------|-------|-----|
| `RmRecord::new(...)` | Custom | Custom | Custom |
| `RmRecord::field_state(content, confidence)` | `"field_state"` | `"orac-sidecar"` | 300s |
| `RmRecord::decision(content, confidence, ttl)` | `"decision"` | `"orac-sidecar"` | Custom |

### Core Flows

**Post Record (`post_record`):**
1. Serialize record via `to_tsv()` — produces tab-separated string
2. POST via `raw_http_post_tsv()` to `/put` — Content-Type: `text/tab-separated-values`
3. Update state: increment `records_posted`, reset failures

**Batch Post (`post_records`):**
1. Concatenate all records with `\n` separator: `records.iter().map(|r| r.to_tsv()).collect::<Vec<_>>().join("\n")`
2. Single POST via `raw_http_post_tsv()` to `/put`

**Search (`search`):**
1. GET `/search?q=<urlencoded(query)>`
2. Response parsing: if body starts with `{` -> parse as JSON (`RmSearchResult`); otherwise -> split by newlines as TSV lines
3. Cache result in `last_search_result`

### URL Encoding

Custom `urlencoded()` function: preserves alphanumeric + `-_. ~`, replaces space with `+`, percent-encodes everything else. Uses `HEX_CHARS` lookup table.

### Bridgeable Trait

| Method | Behavior |
|--------|----------|
| `poll()` | Returns `cached_adjustment()` — always 1.0 (neutral, RM doesn't adjust K) |
| `post(payload)` | Convert bytes to string via `from_utf8_lossy`, POST TSV to `/put` |
| `health()` | GET `/health`, returns `Ok(false)` on failure |
| `is_stale(tick)` | Explicit flag OR 2x poll_interval elapsed |

### Tests: 45

Construction (5), initial state (5), RmRecord TSV (10), URL encoding (5), RmSearchResult (2), staleness (2), should_poll (1), failure tracking (1), poll offline (3), thread safety (2), trait object (1), HTTP helpers (2), constants (4), debug (2), TSV fields (1), TSV-not-JSON (1), additional (6).

---

## Module: m26_blackboard

**Purpose:** SQLite shared fleet state. 10 tables, 39 public methods, 116 tests. Feature-gated: `persistence`.

### Architecture

```
Blackboard { conn: rusqlite::Connection }
```

Not `Send + Sync` by itself — ORAC wraps in `Arc<RwLock<Blackboard>>` externally.

### Constructors

| Method | Backing |
|--------|---------|
| `Blackboard::open(path)` | File-backed SQLite |
| `Blackboard::in_memory()` | In-memory SQLite (tests) |

Both call `migrate()` internally. Schema is idempotent (`CREATE TABLE IF NOT EXISTS`).

### Table Summary (10 tables, 6 indexes)

| Table | PK | Rows | Pruning | Role |
|-------|-----|------|---------|------|
| `pane_status` | pane_id | Singleton/pane | Age-based (`prune_stale_panes`), status-based (`prune_complete_panes`) | Current pane state |
| `task_history` | task_id | Append | Age-based (`prune_old_tasks`) | Task completion log |
| `agent_cards` | pane_id | Singleton/pane | Cascading (deleted with pane) | A2A capabilities |
| `ghost_traces` | None | Append | Rank-based keep-N (`prune_ghosts`) | Deregistration audit |
| `consent_declarations` | sphere_id | Singleton/sphere | None | Per-sphere consent gates |
| `consent_audit` | None | Append-only | None | Consent change trail |
| `hebbian_summary` | AUTOINCREMENT | Append | None | STDP snapshot log |
| `ralph_state` | id=1 (CHECK) | Exactly 1 row | Upsert-only | RALPH evolution state |
| `sessions` | session_id | Singleton/session | Manual (`remove_session`) | Active session tracking |
| `coupling_weights` | (from_id, to_id) | Singleton/pair | Upsert-only | Hebbian weight persistence |

### Public API by Subsystem (39 methods)

**Panes (7):** `upsert_pane`, `get_pane`, `list_panes`, `remove_pane`, `pane_count`, `prune_stale_panes`, `prune_complete_panes`

**Tasks (4):** `insert_task`, `recent_tasks`, `task_count`, `prune_old_tasks`

**Cards (5):** `upsert_card`, `get_card`, `list_cards`, `remove_card`, `card_count`

**Ghosts (4):** `insert_ghost`, `recent_ghosts`, `ghost_count`, `prune_ghosts`

**Consents (3):** `upsert_consent`, `get_consent_record`, `list_consents`

**Consent Audit (2):** `insert_consent_audit`, `recent_consent_audit`

**Hebbian (3):** `insert_hebbian_summary`, `recent_hebbian_summaries`, `hebbian_summary_count`

**RALPH (2):** `save_ralph_state`, `load_ralph_state`

**Sessions (3):** `save_sessions`, `load_sessions`, `remove_session`

**Coupling (2):** `save_coupling_weights`, `load_coupling_weights`

### Data Types (11 structs)

`PaneRecord`, `TaskRecord`, `AgentCard`, `GhostRecord`, `ConsentRecord`, `ConsentAuditEntry`, `HebbianSummaryRecord`, `SavedSession`, `SavedRalphState`, `SavedCouplingWeight`, `Blackboard`

### Error Handling

All methods return `PvResult<T>`. Rusqlite errors wrapped as `PvError::Database(format!("context: {e}"))`. No `unwrap()` or `panic()` in production code.

### Transaction Usage

`save_sessions()` and `save_coupling_weights()` use `unchecked_transaction()` for atomic batch writes with explicit `commit()`.

### WAL Mode

**NOT configured.** Uses SQLite default DELETE journal mode. Implications: synchronous writes, single-writer lock. Acceptable for ORAC's access pattern (single `Arc<RwLock<>>` wrapper).

### Tests: 116

Data types (8), SQLite core (50), consent (9), hebbian (10), sessions (8), coupling weights (5), persistence-across-restart (4), cross-table lifecycle (2), pruning strategies (16+), field roundtrips (4+).

---

## Layer: mod.rs

**Doc comment:**
```
//! Layer 5: Bridges — Direct service communication — thermal read, fitness signal,
//! memory hydration, TSV persistence. Adapt M22, M24-M26 from PV2.
```

**6 pub mod declarations:** `http_helpers`, `m22_synthex_bridge`, `m23_me_bridge`, `m24_povm_bridge`, `m25_rm_bridge`, `m26_blackboard`

**No pub use re-exports.** No layer-level constants or functions. Pure coordinator.

**Critical rules documented in mod.rs:**
1. RM is TSV-only (AP05 — JSON causes parse failure)
2. POVM is write-only (BUG-034 — must call `/hydrate` to read back)
3. Bridge URLs: raw SocketAddr, no `http://` prefix (BUG-033)
4. All bridges include `_consent_check()` stub

---

## Cross-Module Patterns

### Bridgeable Trait (5 implementations)

Every bridge implements `Bridgeable` from `m05_traits`:

| Bridge | `poll()` returns | `post()` does | `health()` endpoint |
|--------|-----------------|---------------|-------------------|
| SYNTHEX | k_adjustment (0.8-1.2) | POST `/api/ingest` | GET `/api/health` |
| ME | fitness adjustment (0.8-1.2) | No-op (read-only) | GET `/api/health` |
| POVM | 1.0 (neutral always) | POST `/memories` | GET `/health` |
| RM | 1.0 (neutral always) | POST `/put` (TSV) | GET `/health` |
| Blackboard | N/A (not Bridgeable) | N/A | N/A |

Only SYNTHEX and ME produce actual K-adjustments. POVM and RM are persistence-only (neutral 1.0).

### Interior Mutability Pattern

All 4 bridge structs use `state: RwLock<BridgeState>` for thread-safe shared access:
- Public API: `&self` only (no `&mut self`)
- Read lock for accessors: `.state.read()`
- Write lock for mutations: `.state.write()`
- Scoped locks: drop guard before next acquisition

### Error Handling Consistency

| Pattern | Used By |
|---------|---------|
| `PvError::BridgeUnreachable` | All bridges (TCP failure) |
| `PvError::BridgeParse` | SYNTHEX, ME, POVM (JSON parse failure) |
| `PvError::Database` | Blackboard (rusqlite failure) |
| `health()` returns `Ok(false)` | All bridges (never propagates error) |
| `record_failure()` auto-call | All bridges via `Bridgeable::poll()` |

### Fire-and-Forget Call Sites

| Module | Function | What's Ignored |
|--------|----------|---------------|
| http_helpers | `raw_http_post_with_content_type` | Response body (reads status only) |
| m22 SYNTHEX | `post_field_state` | Response validation |
| m23 ME | `post` | No-op (ME is read-only) |
| m24 POVM | `snapshot` | Response validation |
| m24 POVM | `write_pathways` (per-pathway) | Individual pathway failures (logged, not propagated) |
| m25 RM | `post_record` / `post_records` | Response body (status code only) |

### Poll Interval Defaults

| Bridge | Interval | BUG-M002/SCAN-003 Fix |
|--------|----------|----------------------|
| SYNTHEX | 6 ticks | First poll immediate if stale |
| ME | 12 ticks | First poll immediate if stale |
| POVM write | 12 ticks | Standard interval |
| POVM read | 60 ticks | Standard interval |
| RM | 30 ticks | Standard interval |

---

## Aggregate Statistics

| Metric | Value |
|--------|-------|
| Total LOC (layer) | ~7,034 |
| Total tests (layer) | 339 (23 + 37 + 40 + 56 + 45 + 116 + mod.rs) |
| Public functions (http_helpers) | 10 |
| Public methods (bridges) | 82 (SYNTHEX 15 + ME 16 + POVM 20 + RM 16 + Blackboard 39) |
| Structs defined | 27 |
| Constants defined | 28 |
| CREATE TABLE statements | 10 |
| Secondary indexes | 6 |
| External services connected | 5 (SYNTHEX:8090, ME:8080, POVM:8125, RM:8130, PV2:8132 via helpers) |
| Bridgeable implementations | 4 |
| Fire-and-forget call sites | 6+ |
| Serde aliases (format compat) | 3 (POVM: pre_id/post_id/co_activations) |
| BUG fixes referenced | 12+ (BUG-001, -008, -033, -034, -035, -042, -060b, -060i, GAP-7, M002, H001, SCAN-003) |
