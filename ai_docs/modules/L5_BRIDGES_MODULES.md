---
title: "Layer 5: Bridges — Module Documentation"
date: 2026-03-22
tags: [modules, bridges, L5, orac-sidecar]
plan_ref: "ORAC_PLAN.md"
obsidian: "[[Session 050 — ORAC Sidecar Architecture]]"
layer: L5
modules: [m22, m23, m24, m25, m26]
---

# Layer 5: Bridges (m22-m26)

> Fire-and-forget outbound connections to ULTRAPLATE services.
> Each bridge is raw TCP HTTP with consent-check stubs and configurable poll intervals.
> **Target LOC:** ~2,500 | **Target tests:** 50+
> **Source:** adapt from PV2 M22/M24-M26, m26 new | **Phase:** 3

---

## Overview

Layer 5 provides bidirectional communication with the ULTRAPLATE service mesh.
Bridges are the sensory organs of the sidecar — they poll external services for
thermal, fitness, and memory signals that modulate the Kuramoto coupling field.
All bridges implement the `Bridgeable` trait (m05) and follow fire-and-forget
semantics: writes are spawned via `tokio::spawn`, reads are polled on tick
intervals, and failures emit events rather than retrying silently.

**Feature gate:** `#[cfg(feature = "bridges")]`

**Critical invariants (non-negotiable):**

1. **RM is TSV only** — JSON causes parse failure (AP05/A12). Content-Type MUST be `text/tab-separated-values`.
2. **POVM is write-only** — must call `/hydrate` to read back state (BUG-034/AP15).
3. **Bridge URLs are raw `SocketAddr`** — no `http://` prefix (BUG-033/AP13). Correct: `"localhost:8090"`. Wrong: `"http://localhost:8090"`.
4. **All bridges include `_consent_check()` stub** — placeholder for P21 consent-gated reads.
5. **SYNTHEX cascade amplification** — use `amp` directly, not `1.0/amp` (Session 012 bug).

**Design constraints:**
- Fire-and-forget: `tokio::spawn`, raw `TcpStream` HTTP, no hyper overhead
- Timeout per bridge request: 2s TCP connect, 5s configurable
- Bridge failures emit events — never retry silently
- Retry policy: exponential backoff, max 3 attempts, jitter
- All `poll()` returns owned values through `RwLock` (never `&T`)

---

## m22 — SYNTHEX Bridge

**Source:** `src/m5_bridges/m22_synthex_bridge.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types`, `m02_error_handling`, `m04_constants`, `m05_traits::Bridgeable`
**Hot-Swap:** adapt from PV2 M22

### Design Decisions

- **Raw TCP HTTP** — no hyper dependency, minimal overhead for fire-and-forget writes
- **Thermal → coupling modulation** — cold temperatures boost coupling (>1.0), hot reduce (<1.0)
- **FMA for float arithmetic** — `deviation.mul_add(-0.2, 1.0)` not `1.0 - deviation * 0.2`
- **Configurable poll interval** — default 6 ticks, tunable via `with_config()`
- **BUG-033 enforced** — `base_url` field stores `"localhost:8090"`, never `"http://localhost:8090"`
- **ALERT-1 awareness** — SYNTHEX synergy is 0.15-0.5 (from Session 040), bridge handles gracefully

### Types to Implement

```rust
/// Response from the SYNTHEX `/v3/thermal` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalResponse {
    /// Current temperature reading.
    pub temperature: f64,
    /// Target temperature (PID setpoint).
    pub target: f64,
    /// PID controller output.
    pub pid_output: f64,
    /// Heat source readings (HS-001 through HS-004).
    #[serde(default)]
    pub heat_sources: Vec<HeatSource>,
}

/// A single SYNTHEX heat source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatSource {
    /// Heat source identifier (e.g. "HS-001").
    pub id: String,
    /// Current reading value.
    pub reading: f64,
    /// Weight in the composite temperature.
    pub weight: f64,
}

/// Mutable state behind a `RwLock` for the SYNTHEX bridge.
#[derive(Debug)]
struct BridgeState {
    last_poll_tick: u64,
    cached_adjustment: f64,          // Default: 1.0 (neutral)
    stale: bool,                     // Default: true
    consecutive_failures: u32,
    last_response: Option<ThermalResponse>,
}

/// Bridge to SYNTHEX service for thermal coupling modulation.
/// Implements `Bridgeable` trait. Uses raw TCP HTTP.
#[derive(Debug)]
pub struct SynthexBridge {
    service: String,                 // "synthex"
    base_url: String,                // "localhost:8090" — NO http:// prefix!
    poll_interval: u64,              // Default: 6 ticks
    state: RwLock<BridgeState>,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `new()` | `-> Self` | Default bridge (`localhost:8090`, 6-tick poll) |
| `with_config()` | `(base_url: impl Into<String>, poll_interval: u64) -> Self` | Custom config |
| `poll_interval()` | `&self -> u64` | Read poll interval (`const fn`) |
| `cached_adjustment()` | `&self -> f64` | Current thermal adjustment through `RwLock` |
| `consecutive_failures()` | `&self -> u32` | Failure count through `RwLock` |
| `last_response()` | `&self -> Option<ThermalResponse>` | Last thermal data (`.clone()` through lock) |
| `thermal_adjustment()` | `&ThermalResponse -> f64` | `deviation.mul_add(-0.2, 1.0).clamp(K_MOD_BUDGET_MIN, K_MOD_BUDGET_MAX)` |
| `_consent_check()` | `&self -> bool` | Stub, always returns `true` (P21) |

**`Bridgeable` trait implementation:**
- `service_name()` -> `"synthex"`
- `poll()` -> raw TCP GET to `/v3/thermal`, parse JSON, compute `thermal_adjustment()`
- `post()` -> raw TCP POST to `/api/ingest`, fire-and-forget
- `health()` -> raw TCP GET to `/api/health`, check 200
- `is_stale()` -> `current_tick - last_poll_tick >= poll_interval`

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `thermal_adjustment_cold` | unit | Cold (temp < target) -> adjustment > 1.0 |
| `thermal_adjustment_hot` | unit | Hot (temp > target) -> adjustment < 1.0 |
| `thermal_adjustment_neutral` | unit | On-target -> adjustment == 1.0 |
| `thermal_adjustment_clamp` | unit | Extreme deviation clamps to K_MOD_BUDGET bounds |
| `bridge_state_default` | unit | Initial state: adjustment=1.0, stale=true, failures=0 |
| `with_config_min_interval` | unit | `poll_interval.max(1)` prevents zero |
| `is_stale_boundary` | unit | Exact interval boundary returns stale |
| `base_url_no_http_prefix` | unit | Asserts `base_url` does not contain "http://" |
| `poll_live` | integration | Requires SYNTHEX :8090 running |

### Cross-References

- `m04_constants::K_MOD_BUDGET_MIN/MAX` — clamp bounds for thermal adjustment
- `m05_traits::Bridgeable` — trait this module implements
- `m27_conductor` — consumes the thermal k-adjustment in bridge composition
- `[[Synthex (The brain of the developer environment)]]`
- `[[ULTRAPLATE — Bugs and Known Issues]]` BUG-033

---

## m23 — ME Bridge

**Source:** `src/m5_bridges/m23_me_bridge.rs`
**LOC Target:** ~450
**Depends on:** `m01_core_types`, `m02_error_handling`, `m04_constants`, `m05_traits::Bridgeable`
**Hot-Swap:** adapt from PV2 M24

### Design Decisions

- **BUG-008 frozen detection** — ME `EventBus` has zero publishers since 2026-03-06; fitness frozen at 0.3662. Bridge detects repeated identical values and falls back to neutral (1.0)
- **Frozen threshold** — 3 consecutive identical polls (within `FROZEN_TOLERANCE = 0.001`) triggers frozen flag
- **Neutral fallback** — when frozen, `poll()` returns 1.0 (no coupling modulation) instead of propagating stale data
- **Graceful degradation** — bridge logs frozen state, does not fail or retry aggressively

### Types to Implement

```rust
/// Response from the ME `/api/observer` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverResponse {
    #[serde(default)]
    pub fitness: f64,           // 0.0-1.0
    #[serde(default)]
    pub active_layers: u32,
    #[serde(default)]
    pub has_publishers: bool,   // False when BUG-008 active
    #[serde(default)]
    pub status: String,
}

/// Mutable state with frozen-detection fields.
#[derive(Debug)]
struct BridgeState {
    last_poll_tick: u64,
    cached_adjustment: f64,
    stale: bool,
    consecutive_failures: u32,
    last_fitness: f64,
    frozen_count: u32,          // Incremented when fitness == last_fitness
    is_frozen: bool,            // True when frozen_count >= FROZEN_THRESHOLD
    last_response: Option<ObserverResponse>,
}

/// Bridge to the Maintenance Engine for fitness-based coupling modulation.
#[derive(Debug)]
pub struct MeBridge {
    service: String,             // "me"
    base_url: String,            // "localhost:8080" — NO http://!
    poll_interval: u64,          // Default: 12 ticks
    state: RwLock<BridgeState>,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `new()` | `-> Self` | Default bridge (`localhost:8080`, 12-tick poll) |
| `with_config()` | `(base_url, poll_interval) -> Self` | Custom config |
| `is_frozen()` | `&self -> bool` | BUG-008 detection through `RwLock` |
| `last_fitness()` | `&self -> f64` | Raw fitness value through `RwLock` |
| `_consent_check()` | `&self -> bool` | Stub, always `true` (P21) |

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `frozen_detection_threshold` | unit | 3 identical fitness readings trigger `is_frozen` |
| `frozen_fallback_neutral` | unit | Frozen state returns adjustment=1.0 |
| `frozen_recovery` | unit | Changed fitness resets frozen count |
| `observer_response_defaults` | unit | `serde(default)` handles missing fields |
| `bridge_state_default` | unit | Initial: not frozen, zero failures |
| `frozen_tolerance_boundary` | unit | Values within 0.001 count as identical |
| `poll_live` | integration | Requires ME :8080 running |

### Cross-References

- BUG-008: ME EventBus zero publishers (ALERT-2)
- `m04_constants::K_MOD_BUDGET_MIN/MAX` — adjustment clamp bounds
- `m27_conductor` — consumes the fitness adjustment
- `[[The Maintenance Engine V2]]`

---

## m24 — POVM Bridge

**Source:** `src/m5_bridges/m24_povm_bridge.rs`
**LOC Target:** ~550
**Depends on:** `m01_core_types`, `m02_error_handling`, `m05_traits::Bridgeable`
**Hot-Swap:** adapt from PV2 M25

### Design Decisions

- **Write-only by default (BUG-034)** — POVM does not expose raw state via GET. Must call `/hydrate` explicitly to read back. This is documented but still a trap for new contributors.
- **Dual intervals** — write (snapshot) every 12 ticks, read (pathway hydration) every 60 ticks
- **Startup hydration** — `hydrate_pathways()` + `hydrate_summary()` called on init, before first tick
- **No k_adj production** — unlike SYNTHEX/ME, POVM is a storage bridge. `cached_adjustment` is always 1.0 (neutral).
- **Pathway seeding** — hydrated pathways seed the Hebbian coupling network (SYS-5)

### Types to Implement

```rust
/// A single Hebbian pathway from the POVM Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pathway {
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub weight: f64,              // Hebbian strength
    #[serde(default)]
    pub reinforcement_count: u64,
}

/// Summary response from POVM Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PovmSummary {
    #[serde(default)]
    pub pathway_count: u64,       // 2,425 typical (bimodal distribution)
    #[serde(default)]
    pub memory_count: u64,        // 36 typical
    #[serde(default)]
    pub uptime_secs: f64,
}

/// Response from the `/pathways` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathwaysResponse {
    #[serde(default)]
    pub pathways: Vec<Pathway>,
}

/// Mutable state with separate read/write tick tracking.
#[derive(Debug)]
struct BridgeState {
    last_write_tick: u64,         // Snapshot writes
    last_read_tick: u64,          // Pathway hydration reads
    cached_adjustment: f64,       // Always 1.0 — POVM does not produce k_adj
    stale: bool,
    consecutive_failures: u32,
    cached_pathways: Vec<Pathway>,
    last_summary: Option<PovmSummary>,
    hydrated: bool,               // False until first hydrate_pathways() succeeds
}

/// Bridge to the POVM Engine for persistent sphere snapshots
/// and Hebbian weight seeding.
#[derive(Debug)]
pub struct PovmBridge {
    service: String,              // "povm"
    base_url: String,             // "localhost:8125" — NO http://!
    write_interval: u64,          // Default: 12 ticks
    read_interval: u64,           // Default: 60 ticks
    state: RwLock<BridgeState>,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `new()` | `-> Self` | Default config (`localhost:8125`, write=12, read=60) |
| `with_config()` | `(base_url, write_interval, read_interval) -> Self` | Custom intervals |
| `hydrate_pathways()` | `&self -> PvResult<Vec<Pathway>>` | Startup hydration via `/hydrate` then `/pathways` |
| `hydrate_summary()` | `&self -> PvResult<PovmSummary>` | Startup summary via `/summary` |
| `is_hydrated()` | `&self -> bool` | Whether initial hydration succeeded |
| `cached_pathways()` | `&self -> Vec<Pathway>` | Clone through `RwLock` |
| `_consent_check()` | `&self -> bool` | Stub, always `true` (P21) |

**Anti-pattern warning:**
```rust
// WRONG — POVM is write-only! GET /memories returns nothing useful.
let data = povm.poll()?;  // Returns stale or empty data

// CORRECT — must hydrate first, then read pathways
povm.post(snapshot_bytes)?;       // Fire-and-forget write
let paths = povm.hydrate_pathways()?;  // Explicit hydration read
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `write_only_default` | unit | `cached_adjustment` is always 1.0 |
| `dual_intervals` | unit | Write and read intervals are independent |
| `hydration_flag` | unit | `is_hydrated()` false before, true after hydrate |
| `pathway_deserialize` | unit | Handles missing fields with `serde(default)` |
| `summary_deserialize` | unit | Handles missing fields gracefully |
| `base_url_no_http` | unit | Asserts no `http://` prefix |
| `hydrate_live` | integration | Requires POVM :8125 running |

### Cross-References

- BUG-034: POVM write-only — must call `/hydrate` (AP15)
- BUG-033: Raw `SocketAddr`, no `http://` prefix (AP13)
- `m18_hebbian_stdp` — consumes hydrated pathway weights for coupling initialization
- `m04_constants::SNAPSHOT_INTERVAL` — 60-tick snapshot cycle
- `[[POVM Engine]]`

---

## m25 — RM Bridge

**Source:** `src/m5_bridges/m25_rm_bridge.rs`
**LOC Target:** ~550
**Depends on:** `m01_core_types`, `m02_error_handling`, `m05_traits::Bridgeable`
**Hot-Swap:** adapt from PV2 M26

### Design Decisions

- **TSV ONLY (AP05/A12)** — the single most critical rule in this module. JSON to RM causes silent parse failures. Content-Type MUST be `text/tab-separated-values`.
- **Content sanitization** — tabs, newlines, and carriage returns in content are replaced with spaces before TSV encoding. This prevents field boundary corruption.
- **Agent name adaptation** — PV2 uses `"pane-vortex"`, ORAC uses `"orac-sidecar"`. The `DEFAULT_AGENT` constant must change during adaptation.
- **No k_adj production** — like POVM, RM is a persistence bridge. `cached_adjustment` is always 1.0.
- **TSV format** — `category\tagent\tconfidence\tttl\tcontent` (5 tab-separated fields)

### Types to Implement

```rust
/// A TSV record for the Reasoning Memory.
/// Format: `category\tagent\tconfidence\tttl\tcontent`
/// **NEVER serialize as JSON to the RM service!**
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmRecord {
    pub category: String,     // e.g. "field_state", "decision", "bridge_health"
    pub agent: String,        // "orac-sidecar" (NOT "pane-vortex")
    pub confidence: f64,      // 0.0-1.0, clamped on construction
    pub ttl: u64,             // Seconds
    pub content: String,      // The actual data payload
}

impl RmRecord {
    /// Create a new RM record with confidence clamping.
    pub fn new(
        category: impl Into<String>,
        agent: impl Into<String>,
        confidence: f64,
        ttl: u64,
        content: impl Into<String>,
    ) -> Self;

    /// Create a `field_state` record with default agent and TTL.
    pub fn field_state(content: impl Into<String>, confidence: f64) -> Self;

    /// Create a decision record.
    pub fn decision(content: impl Into<String>, confidence: f64, ttl: u64) -> Self;

    /// Serialize to TSV format. Sanitizes tabs/newlines in all fields.
    pub fn to_tsv(&self) -> String;

    /// Parse from TSV format.
    /// # Errors
    /// Returns error if line doesn't have exactly 5 tab-separated fields.
    pub fn from_tsv(line: &str) -> Result<Self, String>;
}

/// Search result from the RM `/search` endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RmSearchResult {
    #[serde(default)]
    pub entries: Vec<String>,   // Raw TSV lines
    #[serde(default)]
    pub total: u64,
}

/// Bridge to Reasoning Memory for TSV-based cross-session persistence.
#[derive(Debug)]
pub struct RmBridge {
    service: String,            // "rm"
    base_url: String,           // "localhost:8130" — NO http://!
    poll_interval: u64,         // Default: 30 ticks
    state: RwLock<BridgeState>,
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `new()` | `-> Self` | Default config (`localhost:8130`, 30-tick poll) |
| `with_config()` | `(base_url, poll_interval) -> Self` | Custom config |
| `post_record()` | `&self, record: &RmRecord -> PvResult<()>` | POST TSV to `/put` |
| `search()` | `&self, query: &str -> PvResult<RmSearchResult>` | GET `/search?q={query}` |
| `records_posted()` | `&self -> u64` | Session write counter through `RwLock` |
| `_consent_check()` | `&self -> bool` | Stub, always `true` (P21) |

**Critical anti-pattern — NEVER do this:**
```rust
// BANNED — A12 violation. JSON to RM causes parse failure.
let body = serde_json::to_string(&record)?;
bridge.post(body.as_bytes())?;

// CORRECT — TSV only, always.
let body = record.to_tsv();
bridge.post(body.as_bytes())?;
```

**TSV sanitization example:**
```rust
// Input with embedded tabs and newlines
let record = RmRecord::new(
    "field_state",
    "orac-sidecar",
    0.95,
    300,
    "r=0.997\tK=2.42\nchimera=false",  // Contains tab + newline
);
// Output: "field_state\torac-sidecar\t0.95\t300\tr=0.997 K=2.42 chimera=false"
//         tabs and newlines replaced with spaces ^          ^
```

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `to_tsv_basic` | unit | 5-field tab-separated output |
| `to_tsv_sanitize_tabs` | unit | Tabs in content replaced with spaces |
| `to_tsv_sanitize_newlines` | unit | `\n` and `\r` in content replaced with spaces |
| `from_tsv_roundtrip` | unit | `from_tsv(to_tsv())` recovers original |
| `from_tsv_too_few_fields` | unit | Returns error for < 5 fields |
| `from_tsv_bad_confidence` | unit | Non-numeric confidence returns error |
| `confidence_clamped` | unit | Values outside 0.0-1.0 are clamped |
| `field_state_convenience` | unit | Helper sets category + agent + TTL defaults |
| `records_posted_increments` | unit | Counter tracks successful posts |
| `base_url_no_http` | unit | No `http://` prefix |
| `post_live` | integration | Requires RM :8130 running |
| `search_live` | integration | Requires RM :8130 running |

### Cross-References

- AP05/A12: TSV-only rule (ANTI_PATTERNS.md)
- BUG-033: Raw `SocketAddr`, no `http://` prefix
- `m04_constants::MEMORY_PRUNE_INTERVAL` — 200-step prune cycle
- Port 8130: `POST /put` (TSV body), `GET /search?q=`, `GET /entries`
- `[[ULTRAPLATE — Bugs and Known Issues]]`

---

## m26 — Blackboard

**Source:** `src/m5_bridges/m26_blackboard.rs`
**LOC Target:** ~500
**Depends on:** `m01_core_types`, `m02_error_handling`
**Hot-Swap:** NEW (no PV2 equivalent)

### Design Decisions

- **SQLite with WAL** — `PRAGMA journal_mode=WAL` (P13) for concurrent read/write from multiple modules
- **Local, not networked** — unlike other bridges, the blackboard is a local SQLite database. No TCP, no `Bridgeable` trait.
- **Shared fleet state** — pane status, task history, agent cards. This is the coordination substrate for multi-pane awareness.
- **Schema-first** — all table schemas defined as `const` strings. `.schema` before writing SQL.
- **Query via `.claude/queries/blackboard.sql`** — pre-written queries for common operations

### Types to Implement

```rust
/// Pane status record in the blackboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneRecord {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// Current status (idle, working, blocked).
    pub status: String,
    /// Active task description, if any.
    pub active_task: Option<String>,
    /// Last heartbeat timestamp.
    pub last_heartbeat: f64,
    /// Persona label.
    pub persona: String,
}

/// Task history entry in the blackboard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    /// Unique task identifier.
    pub task_id: String,
    /// Pane that executed the task.
    pub pane_id: PaneId,
    /// Task description.
    pub description: String,
    /// Start timestamp.
    pub started_at: f64,
    /// Completion timestamp, if finished.
    pub completed_at: Option<f64>,
    /// Task outcome (success, failure, cancelled).
    pub outcome: Option<String>,
}

/// Agent card for fleet-wide identity awareness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// Agent persona name.
    pub persona: String,
    /// Specialization domains.
    pub domains: Vec<String>,
    /// Current workload (0.0-1.0).
    pub workload: f64,
    /// Session start timestamp.
    pub session_start: f64,
}

/// SQLite blackboard handle.
#[derive(Debug)]
pub struct Blackboard {
    /// Database connection (WAL mode).
    conn: parking_lot::Mutex<rusqlite::Connection>,
    /// Database file path.
    path: String,
}

impl Blackboard {
    /// Open or create a blackboard database.
    /// Applies WAL journal mode and creates tables if missing.
    pub fn open(path: impl Into<String>) -> PvResult<Self>;

    /// Insert or update a pane status record.
    pub fn upsert_pane(&self, record: &PaneRecord) -> PvResult<()>;

    /// Get all pane records.
    pub fn all_panes(&self) -> PvResult<Vec<PaneRecord>>;

    /// Insert a task history entry.
    pub fn insert_task(&self, record: &TaskRecord) -> PvResult<()>;

    /// Get recent tasks for a pane.
    pub fn recent_tasks(&self, pane_id: &PaneId, limit: usize) -> PvResult<Vec<TaskRecord>>;

    /// Upsert an agent card.
    pub fn upsert_agent_card(&self, card: &AgentCard) -> PvResult<()>;

    /// Get all agent cards.
    pub fn all_agent_cards(&self) -> PvResult<Vec<AgentCard>>;

    /// Remove stale panes (no heartbeat in `threshold_secs`).
    pub fn prune_stale_panes(&self, threshold_secs: f64) -> PvResult<usize>;
}
```

### Key Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `open()` | `(path) -> PvResult<Self>` | Create/open DB with WAL + table creation |
| `upsert_pane()` | `(&self, &PaneRecord) -> PvResult<()>` | INSERT OR REPLACE pane status |
| `all_panes()` | `(&self) -> PvResult<Vec<PaneRecord>>` | Read all pane records |
| `insert_task()` | `(&self, &TaskRecord) -> PvResult<()>` | Append task history |
| `recent_tasks()` | `(&self, &PaneId, limit) -> PvResult<Vec<TaskRecord>>` | Paginated task query |
| `upsert_agent_card()` | `(&self, &AgentCard) -> PvResult<()>` | INSERT OR REPLACE agent card |
| `all_agent_cards()` | `(&self) -> PvResult<Vec<AgentCard>>` | Read all agent cards |
| `prune_stale_panes()` | `(&self, threshold_secs) -> PvResult<usize>` | Remove panes with no heartbeat |

### Tests

| Test | Kind | Validates |
|------|------|-----------|
| `open_creates_tables` | unit | Tables exist after `open()` |
| `upsert_pane_idempotent` | unit | Same pane_id overwrites, not duplicates |
| `insert_task_append` | unit | Multiple tasks accumulate |
| `recent_tasks_limit` | unit | Respects limit parameter |
| `prune_stale_removes` | unit | Panes older than threshold are removed |
| `wal_mode_enabled` | unit | `PRAGMA journal_mode` returns "wal" |
| `agent_card_roundtrip` | unit | Upsert then read returns identical data |
| `concurrent_read_write` | unit | Two threads can read/write simultaneously |
| `empty_db_returns_empty` | unit | All queries return empty vecs on fresh DB |

### Cross-References

- `PRAGMA journal_mode=WAL` (P13)
- `.claude/queries/blackboard.sql` — pre-written query templates
- `m30_wasm_bridge` — reads pane status from blackboard for WASM events
- `m29_tick` — may update blackboard on snapshot ticks
- No `Bridgeable` trait — this is a local store, not a network bridge

---

## Service Endpoint Reference

| Bridge | Port | Health | Key Endpoints | Protocol |
|--------|------|--------|---------------|----------|
| SYNTHEX | 8090 | `/api/health` | `/v3/thermal`, `/v3/diagnostics`, `POST /api/ingest` | JSON over raw TCP HTTP |
| ME | 8080 | `/api/health` | `/api/observer` (fitness, correlations) | JSON over raw TCP HTTP |
| POVM | 8125 | `/health` | `/memories`, `/pathways`, `/hydrate`, `/consolidate` | JSON over raw TCP HTTP |
| RM | 8130 | `/health` | `POST /put` (**TSV!**), `GET /search?q=`, `GET /entries` | **TSV** (POST), JSON (GET) |
| Blackboard | — | — | Local SQLite | `rusqlite` |

## Patterns and Anti-Patterns

**Patterns to follow:**
- P1: Builder pattern for all bridge constructors with 3+ config params
- P2: All trait methods `&self` with interior mutability via `parking_lot::RwLock`
- P6: `Timestamp` newtype for all temporal values (not `SystemTime`)
- P7: Owned returns from `RwLock` — `.read().field.clone()`, never `&T`
- P13: SQLite WAL mode for concurrent access
- P21: `_consent_check()` stub on all bridges

**Anti-patterns to avoid:**
- A1/A2: No `unwrap()` or `expect()` outside tests
- A12: **NEVER** JSON to Reasoning Memory
- BUG-033: **NEVER** `http://` prefix on bridge URLs
- BUG-034: **NEVER** assume POVM GET returns useful data without `/hydrate`
- Session 012: **NEVER** invert cascade amplification (`1.0/amp`)
