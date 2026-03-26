# R1 Fleet SQL Catalog -- ORAC Sidecar

> **Source:** `src/m5_bridges/m26_blackboard.rs` (2,604 LOC, 10 tables, 33 public methods)
> **Migration:** `migrations/001_blackboard.sql` (v1 schema, superseded by inline `migrate()`)
> **Query Templates:** `.claude/queries/` (3 files: blackboard.sql, hook_events.sql, fleet_state.sql)
> **DB Engine:** SQLite via `rusqlite 0.32` (feature-gated: `persistence`)
> **DB Path:** `~/.local/share/orac/blackboard.db` (fallback: `/tmp/orac/blackboard.db`)
> **Connection:** Single `rusqlite::Connection` wrapped in `parking_lot::Mutex` on `OracState`
> **Generated:** 2026-03-25 from m26_blackboard.rs line-by-line extraction

---

## 1. Table DDL

All tables are created in `Blackboard::migrate()` (lines 256-375) via a single `execute_batch` call.
The `IF NOT EXISTS` clause makes migrations idempotent.

### 1.1 `pane_status` -- Current fleet pane registration

```sql
CREATE TABLE IF NOT EXISTS pane_status (
    pane_id          TEXT PRIMARY KEY,
    status           TEXT NOT NULL DEFAULT 'Idle',
    persona          TEXT NOT NULL DEFAULT '',
    updated_at       REAL NOT NULL DEFAULT 0.0,
    phase            REAL NOT NULL DEFAULT 0.0,
    tasks_completed  INTEGER NOT NULL DEFAULT 0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `pane_id` | TEXT | PRIMARY KEY | Unique pane/sphere identifier |
| `status` | TEXT | NOT NULL, DEFAULT 'Idle' | One of: Idle, Working, Blocked, Complete |
| `persona` | TEXT | NOT NULL, DEFAULT '' | Human-readable agent persona name |
| `updated_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds (f64) of last update |
| `phase` | REAL | NOT NULL, DEFAULT 0.0 | Kuramoto ring phase (radians) |
| `tasks_completed` | INTEGER | NOT NULL, DEFAULT 0 | Cumulative task completion count |

### 1.2 `task_history` -- Completed/failed task audit trail

```sql
CREATE TABLE IF NOT EXISTS task_history (
    task_id        TEXT PRIMARY KEY,
    pane_id        TEXT NOT NULL,
    description    TEXT NOT NULL DEFAULT '',
    outcome        TEXT NOT NULL DEFAULT 'completed',
    finished_at    REAL NOT NULL DEFAULT 0.0,
    duration_secs  REAL NOT NULL DEFAULT 0.0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `task_id` | TEXT | PRIMARY KEY | Unique task identifier |
| `pane_id` | TEXT | NOT NULL | Pane that executed the task |
| `description` | TEXT | NOT NULL, DEFAULT '' | Brief task description |
| `outcome` | TEXT | NOT NULL, DEFAULT 'completed' | "completed" or "failed" |
| `finished_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds (f64) when finished |
| `duration_secs` | REAL | NOT NULL, DEFAULT 0.0 | Wall-clock duration in seconds |

### 1.3 `agent_cards` -- A2A-inspired capability declarations

```sql
CREATE TABLE IF NOT EXISTS agent_cards (
    pane_id        TEXT PRIMARY KEY,
    capabilities   TEXT NOT NULL DEFAULT '[]',
    domain         TEXT NOT NULL DEFAULT '',
    model          TEXT NOT NULL DEFAULT '',
    registered_at  REAL NOT NULL DEFAULT 0.0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `pane_id` | TEXT | PRIMARY KEY | FK-like reference to pane_status.pane_id |
| `capabilities` | TEXT | NOT NULL, DEFAULT '[]' | JSON array of skill strings |
| `domain` | TEXT | NOT NULL, DEFAULT '' | Specialization (e.g. "rust", "frontend", "devops") |
| `model` | TEXT | NOT NULL, DEFAULT '' | Model identifier (e.g. "opus-4.6") |
| `registered_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds of card registration |

### 1.4 `ghost_traces` -- Deregistered sphere departure records

```sql
CREATE TABLE IF NOT EXISTS ghost_traces (
    sphere_id            TEXT NOT NULL,
    persona              TEXT NOT NULL DEFAULT '',
    deregistered_ms      INTEGER NOT NULL,
    final_phase          REAL NOT NULL DEFAULT 0.0,
    total_tools          INTEGER NOT NULL DEFAULT 0,
    session_duration_ms  INTEGER NOT NULL DEFAULT 0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `sphere_id` | TEXT | NOT NULL | Sphere ID at time of departure (no PK -- duplicates allowed) |
| `persona` | TEXT | NOT NULL, DEFAULT '' | Persona string at departure |
| `deregistered_ms` | INTEGER | NOT NULL | Epoch milliseconds when deregistered |
| `final_phase` | REAL | NOT NULL, DEFAULT 0.0 | Kuramoto phase at departure |
| `total_tools` | INTEGER | NOT NULL, DEFAULT 0 | Total tool calls during session |
| `session_duration_ms` | INTEGER | NOT NULL, DEFAULT 0 | Session wall-clock duration (ms) |

**Note:** No PRIMARY KEY -- the same `sphere_id` can appear multiple times (one ghost per deregistration event).

### 1.5 `consent_declarations` -- Per-sphere bridge permission grants

```sql
CREATE TABLE IF NOT EXISTS consent_declarations (
    sphere_id      TEXT PRIMARY KEY,
    synthex_write  INTEGER NOT NULL DEFAULT 1,
    povm_read      INTEGER NOT NULL DEFAULT 1,
    povm_write     INTEGER NOT NULL DEFAULT 0,
    hydration      INTEGER NOT NULL DEFAULT 1,
    updated_ms     INTEGER NOT NULL DEFAULT 0
);
```

| Column | Type | Constraint | Default | Description |
|--------|------|-----------|---------|-------------|
| `sphere_id` | TEXT | PRIMARY KEY | -- | Sphere this consent applies to |
| `synthex_write` | INTEGER | NOT NULL | 1 (true) | Allow SYNTHEX bridge writes |
| `povm_read` | INTEGER | NOT NULL | 1 (true) | Allow POVM bridge reads |
| `povm_write` | INTEGER | NOT NULL | 0 (false) | Allow POVM bridge writes |
| `hydration` | INTEGER | NOT NULL | 1 (true) | Allow session hydration from POVM + RM |
| `updated_ms` | INTEGER | NOT NULL | 0 | Epoch milliseconds of last update |

### 1.6 `consent_audit` -- Immutable consent change audit trail

```sql
CREATE TABLE IF NOT EXISTS consent_audit (
    sphere_id   TEXT NOT NULL,
    field_name  TEXT NOT NULL,
    old_value   INTEGER NOT NULL,
    new_value   INTEGER NOT NULL,
    changed_ms  INTEGER NOT NULL
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `sphere_id` | TEXT | NOT NULL | Sphere whose consent was modified |
| `field_name` | TEXT | NOT NULL | Name of the changed field (e.g. "synthex_write") |
| `old_value` | INTEGER | NOT NULL | Previous value (0/1 boolean) |
| `new_value` | INTEGER | NOT NULL | New value (0/1 boolean) |
| `changed_ms` | INTEGER | NOT NULL | Epoch milliseconds when change occurred |

**Note:** Append-only table. No PK. Provides full audit trail of all consent mutations.

### 1.7 `hebbian_summary` -- STDP learning batch summaries

```sql
CREATE TABLE IF NOT EXISTS hebbian_summary (
    id                   INTEGER PRIMARY KEY AUTOINCREMENT,
    tick                 INTEGER NOT NULL,
    ltp_count            INTEGER NOT NULL DEFAULT 0,
    ltd_count            INTEGER NOT NULL DEFAULT 0,
    at_floor_count       INTEGER NOT NULL DEFAULT 0,
    total_weight_change  REAL NOT NULL DEFAULT 0.0,
    connections_total    INTEGER NOT NULL DEFAULT 0,
    weight_mean          REAL NOT NULL DEFAULT 0.0,
    weight_min           REAL NOT NULL DEFAULT 0.0,
    weight_max           REAL NOT NULL DEFAULT 0.0,
    created_at           REAL NOT NULL DEFAULT 0.0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Auto-generated row ID |
| `tick` | INTEGER | NOT NULL | Tick number when summary was recorded |
| `ltp_count` | INTEGER | NOT NULL, DEFAULT 0 | Number of LTP (potentiation) events |
| `ltd_count` | INTEGER | NOT NULL, DEFAULT 0 | Number of LTD (depression) events |
| `at_floor_count` | INTEGER | NOT NULL, DEFAULT 0 | Connections at weight floor |
| `total_weight_change` | REAL | NOT NULL, DEFAULT 0.0 | Total absolute weight change |
| `connections_total` | INTEGER | NOT NULL, DEFAULT 0 | Total coupling connections |
| `weight_mean` | REAL | NOT NULL, DEFAULT 0.0 | Mean connection weight |
| `weight_min` | REAL | NOT NULL, DEFAULT 0.0 | Minimum connection weight |
| `weight_max` | REAL | NOT NULL, DEFAULT 0.0 | Maximum connection weight |
| `created_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds of record creation |

**Note:** Multiple records per tick are allowed (no unique constraint on `tick`).

### 1.8 `ralph_state` -- RALPH evolution engine singleton state

```sql
CREATE TABLE IF NOT EXISTS ralph_state (
    id                 INTEGER PRIMARY KEY CHECK (id = 1),
    generation         INTEGER NOT NULL DEFAULT 0,
    completed_cycles   INTEGER NOT NULL DEFAULT 0,
    current_fitness    REAL NOT NULL DEFAULT 0.5,
    peak_fitness       REAL NOT NULL DEFAULT 0.0,
    total_proposed     INTEGER NOT NULL DEFAULT 0,
    total_accepted     INTEGER NOT NULL DEFAULT 0,
    total_rolled_back  INTEGER NOT NULL DEFAULT 0,
    last_phase         TEXT NOT NULL DEFAULT 'Recognize',
    updated_at         REAL NOT NULL DEFAULT 0.0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `id` | INTEGER | PRIMARY KEY, CHECK (id = 1) | Singleton row enforced by CHECK constraint |
| `generation` | INTEGER | NOT NULL, DEFAULT 0 | RALPH generation counter |
| `completed_cycles` | INTEGER | NOT NULL, DEFAULT 0 | Completed RALPH cycles |
| `current_fitness` | REAL | NOT NULL, DEFAULT 0.5 | Most recent fitness score |
| `peak_fitness` | REAL | NOT NULL, DEFAULT 0.0 | Peak fitness observed across all generations |
| `total_proposed` | INTEGER | NOT NULL, DEFAULT 0 | Total mutations proposed |
| `total_accepted` | INTEGER | NOT NULL, DEFAULT 0 | Total mutations accepted |
| `total_rolled_back` | INTEGER | NOT NULL, DEFAULT 0 | Total mutations rolled back |
| `last_phase` | TEXT | NOT NULL, DEFAULT 'Recognize' | Last RALPH phase (Recognize/Analyze/Learn/Propose/Harvest) |
| `updated_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds of last save |

**Singleton pattern:** `CHECK (id = 1)` ensures exactly one row. Upsert uses `ON CONFLICT(id) DO UPDATE`.

### 1.9 `sessions` -- Active session persistence for cross-restart hydration

```sql
CREATE TABLE IF NOT EXISTS sessions (
    session_id        TEXT PRIMARY KEY,
    pane_id           TEXT NOT NULL,
    active_task_id    TEXT,
    poll_counter      INTEGER NOT NULL DEFAULT 0,
    total_tool_calls  INTEGER NOT NULL DEFAULT 0,
    started_ms        INTEGER NOT NULL,
    persona           TEXT NOT NULL DEFAULT '',
    updated_at        REAL NOT NULL DEFAULT 0.0
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `session_id` | TEXT | PRIMARY KEY | Session identifier from hook registration |
| `pane_id` | TEXT | NOT NULL | Associated sphere/pane ID |
| `active_task_id` | TEXT | nullable | Current active task (NULL if none) |
| `poll_counter` | INTEGER | NOT NULL, DEFAULT 0 | Tool call count for throttling |
| `total_tool_calls` | INTEGER | NOT NULL, DEFAULT 0 | Total tool calls in session |
| `started_ms` | INTEGER | NOT NULL | Session start time (epoch ms) |
| `persona` | TEXT | NOT NULL, DEFAULT '' | Persona string |
| `updated_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds of last save |

### 1.10 `coupling_weights` -- Hebbian coupling network persistence

```sql
CREATE TABLE IF NOT EXISTS coupling_weights (
    from_id     TEXT NOT NULL,
    to_id       TEXT NOT NULL,
    weight      REAL NOT NULL,
    updated_at  REAL NOT NULL DEFAULT 0.0,
    PRIMARY KEY (from_id, to_id)
);
```

| Column | Type | Constraint | Description |
|--------|------|-----------|-------------|
| `from_id` | TEXT | NOT NULL, composite PK | Source sphere/pane ID |
| `to_id` | TEXT | NOT NULL, composite PK | Target sphere/pane ID |
| `weight` | REAL | NOT NULL | Connection weight [0.0, 1.0] |
| `updated_at` | REAL | NOT NULL, DEFAULT 0.0 | Unix epoch seconds of last update |

---

## 2. Indexes

All indexes are created in `migrate()` with `IF NOT EXISTS` for idempotency.

| Index | Table | Columns | Purpose |
|-------|-------|---------|---------|
| `idx_task_history_pane` | `task_history` | `pane_id` | Filter tasks by pane in `recent_tasks()` |
| `idx_task_history_finished` | `task_history` | `finished_at` | Order by finish time, prune old tasks |
| `idx_ghost_traces_time` | `ghost_traces` | `deregistered_ms` | Order ghosts by departure time, prune oldest |
| `idx_consent_audit_sphere` | `consent_audit` | `sphere_id` | Filter audit entries by sphere |
| `idx_consent_audit_time` | `consent_audit` | `changed_ms` | Order audit entries chronologically |
| `idx_hebbian_summary_tick` | `hebbian_summary` | `tick DESC` | Retrieve most recent summaries efficiently |

### Index DDL (exact SQL)

```sql
CREATE INDEX IF NOT EXISTS idx_task_history_pane
    ON task_history(pane_id);

CREATE INDEX IF NOT EXISTS idx_task_history_finished
    ON task_history(finished_at);

CREATE INDEX IF NOT EXISTS idx_ghost_traces_time
    ON ghost_traces(deregistered_ms);

CREATE INDEX IF NOT EXISTS idx_consent_audit_sphere
    ON consent_audit(sphere_id);

CREATE INDEX IF NOT EXISTS idx_consent_audit_time
    ON consent_audit(changed_ms);

CREATE INDEX IF NOT EXISTS idx_hebbian_summary_tick
    ON hebbian_summary(tick DESC);
```

**Note:** Tables with PRIMARY KEY (pane_status, task_history, agent_cards, consent_declarations, ralph_state, sessions, coupling_weights) have implicit indexes on their PK columns. The explicit indexes above cover non-PK query patterns.

---

## 3. CRUD Methods

All methods are on `impl Blackboard` (feature-gated: `#[cfg(feature = "persistence")]`).
All return `PvResult<T>` where errors are `PvError::Database(String)`.

### 3.1 Pane Status (7 methods)

#### `upsert_pane` (line 384)

```rust
pub fn upsert_pane(&self, record: &PaneRecord) -> PvResult<()>
```

```sql
INSERT INTO pane_status (pane_id, status, persona, updated_at, phase, tasks_completed)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
ON CONFLICT(pane_id) DO UPDATE SET
    status = excluded.status,
    persona = excluded.persona,
    updated_at = excluded.updated_at,
    phase = excluded.phase,
    tasks_completed = excluded.tasks_completed
```

Insert or update a pane's status record. Uses SQLite upsert (`ON CONFLICT ... DO UPDATE`).

#### `get_pane` (line 413)

```rust
pub fn get_pane(&self, pane_id: &PaneId) -> PvResult<Option<PaneRecord>>
```

```sql
SELECT pane_id, status, persona, updated_at, phase, tasks_completed
FROM pane_status WHERE pane_id = ?1
```

Retrieve a single pane record by ID. Returns `None` if not found (uses `OptionalExtension`).

#### `list_panes` (line 439)

```rust
pub fn list_panes(&self) -> PvResult<Vec<PaneRecord>>
```

```sql
SELECT pane_id, status, persona, updated_at, phase, tasks_completed
FROM pane_status ORDER BY pane_id
```

List all registered panes, sorted alphabetically by `pane_id`.

#### `remove_pane` (line 473)

```rust
pub fn remove_pane(&self, pane_id: &PaneId) -> PvResult<bool>
```

```sql
DELETE FROM pane_status WHERE pane_id = ?1
```

Remove a pane record. Returns `true` if a row was deleted, `false` if not found.

#### `pane_count` (line 489)

```rust
pub fn pane_count(&self) -> PvResult<usize>
```

```sql
SELECT COUNT(*) FROM pane_status
```

Count total registered panes.

#### `prune_stale_panes` (line 503)

```rust
pub fn prune_stale_panes(&self, cutoff_secs: f64) -> PvResult<usize>
```

Two-step deletion (no FK cascade in SQLite by default):

```sql
-- Step 1: Remove associated agent_cards
DELETE FROM agent_cards WHERE pane_id IN
    (SELECT pane_id FROM pane_status WHERE updated_at < ?1)

-- Step 2: Remove stale pane_status entries
DELETE FROM pane_status WHERE updated_at < ?1
```

Remove panes with `updated_at` older than `cutoff_secs`. Also removes their agent cards. Returns count of deleted pane records.

#### `prune_complete_panes` (line 532)

```rust
pub fn prune_complete_panes(&self, cutoff_secs: f64) -> PvResult<usize>
```

```sql
-- Step 1: Remove associated agent_cards for old Complete panes
DELETE FROM agent_cards WHERE pane_id IN
    (SELECT pane_id FROM pane_status WHERE status = 'Complete' AND updated_at < ?1)

-- Step 2: Remove old Complete panes only
DELETE FROM pane_status WHERE status = 'Complete' AND updated_at < ?1
```

Like `prune_stale_panes` but only removes panes with status `'Complete'`. Preserves Idle/Working/Blocked panes regardless of age.

### 3.2 Task History (4 methods)

#### `insert_task` (line 576)

```rust
pub fn insert_task(&self, task: &TaskRecord) -> PvResult<()>
```

```sql
INSERT OR REPLACE INTO task_history
    (task_id, pane_id, description, outcome, finished_at, duration_secs)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
```

Insert or replace a task record. `INSERT OR REPLACE` means re-inserting with the same `task_id` overwrites the previous record.

#### `recent_tasks` (line 600)

```rust
pub fn recent_tasks(&self, pane_id: &PaneId, limit: usize) -> PvResult<Vec<TaskRecord>>
```

```sql
SELECT task_id, pane_id, description, outcome, finished_at, duration_secs
FROM task_history WHERE pane_id = ?1
ORDER BY finished_at DESC LIMIT ?2
```

Get the most recent tasks for a specific pane, ordered by `finished_at` descending.

#### `task_count` (line 635)

```rust
pub fn task_count(&self) -> PvResult<usize>
```

```sql
SELECT COUNT(*) FROM task_history
```

Count total task records across all panes.

#### `prune_old_tasks` (line 558)

```rust
pub fn prune_old_tasks(&self, cutoff_secs: f64) -> PvResult<usize>
```

```sql
DELETE FROM task_history WHERE finished_at < ?1
```

Remove tasks finished before the cutoff timestamp. Returns count of deleted records.

### 3.3 Agent Cards (5 methods)

#### `upsert_card` (line 648)

```rust
pub fn upsert_card(&self, card: &AgentCard) -> PvResult<()>
```

```sql
INSERT INTO agent_cards (pane_id, capabilities, domain, model, registered_at)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(pane_id) DO UPDATE SET
    capabilities = excluded.capabilities,
    domain = excluded.domain,
    model = excluded.model,
    registered_at = excluded.registered_at
```

Insert or update an agent card. The `capabilities` `Vec<String>` is serialized to JSON via `serde_json::to_string()` before storage.

#### `get_card` (line 677)

```rust
pub fn get_card(&self, pane_id: &PaneId) -> PvResult<Option<AgentCard>>
```

```sql
SELECT pane_id, capabilities, domain, model, registered_at
FROM agent_cards WHERE pane_id = ?1
```

Retrieve an agent card by pane ID. Deserializes `capabilities` from JSON; uses `unwrap_or_default()` on parse failure (graceful degradation).

#### `list_cards` (line 705)

```rust
pub fn list_cards(&self) -> PvResult<Vec<AgentCard>>
```

```sql
SELECT pane_id, capabilities, domain, model, registered_at
FROM agent_cards ORDER BY pane_id
```

List all agent cards, sorted alphabetically by `pane_id`.

#### `remove_card` (line 741)

```rust
pub fn remove_card(&self, pane_id: &PaneId) -> PvResult<bool>
```

```sql
DELETE FROM agent_cards WHERE pane_id = ?1
```

Remove an agent card. Returns `true` if a row was deleted.

#### `card_count` (line 757)

```rust
pub fn card_count(&self) -> PvResult<usize>
```

```sql
SELECT COUNT(*) FROM agent_cards
```

Count total registered agent cards.

### 3.4 Ghost Traces (4 methods)

#### `insert_ghost` (line 770)

```rust
pub fn insert_ghost(&self, ghost: &GhostRecord) -> PvResult<()>
```

```sql
INSERT INTO ghost_traces
    (sphere_id, persona, deregistered_ms, final_phase, total_tools, session_duration_ms)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
```

Insert a ghost trace. Multiple ghosts with the same `sphere_id` are allowed.

#### `recent_ghosts` (line 794)

```rust
pub fn recent_ghosts(&self, limit: usize) -> PvResult<Vec<GhostRecord>>
```

```sql
SELECT sphere_id, persona, deregistered_ms, final_phase,
       total_tools, session_duration_ms
FROM ghost_traces
ORDER BY deregistered_ms DESC LIMIT ?1
```

Get the most recent ghost traces, ordered by deregistration time descending.

#### `ghost_count` (line 830)

```rust
pub fn ghost_count(&self) -> PvResult<usize>
```

```sql
SELECT COUNT(*) FROM ghost_traces
```

Count total ghost trace records.

#### `prune_ghosts` (line 841)

```rust
pub fn prune_ghosts(&self, keep: usize) -> PvResult<usize>
```

```sql
DELETE FROM ghost_traces WHERE rowid NOT IN
    (SELECT rowid FROM ghost_traces ORDER BY deregistered_ms DESC LIMIT ?1)
```

Prune old ghosts, keeping only the `keep` most recent entries. Uses `rowid` subquery to select oldest records for deletion.

### 3.5 Consent Declarations (3 methods)

#### `upsert_consent` (line 860)

```rust
pub fn upsert_consent(&self, record: &ConsentRecord) -> PvResult<()>
```

```sql
INSERT INTO consent_declarations (sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
ON CONFLICT(sphere_id) DO UPDATE SET
    synthex_write = excluded.synthex_write,
    povm_read = excluded.povm_read,
    povm_write = excluded.povm_write,
    hydration = excluded.hydration,
    updated_ms = excluded.updated_ms
```

Insert or update a consent declaration for a sphere.

#### `get_consent_record` (line 889)

```rust
pub fn get_consent_record(&self, sphere_id: &str) -> PvResult<Option<ConsentRecord>>
```

```sql
SELECT sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms
FROM consent_declarations WHERE sphere_id = ?1
```

Retrieve a consent declaration by sphere ID. Returns `None` if not found.

#### `list_consents` (line 915)

```rust
pub fn list_consents(&self) -> PvResult<Vec<ConsentRecord>>
```

```sql
SELECT sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms
FROM consent_declarations ORDER BY updated_ms DESC
```

List all consent declarations, ordered by most recently updated first.

### 3.6 Consent Audit (2 methods)

#### `insert_consent_audit` (line 951)

```rust
pub fn insert_consent_audit(&self, entry: &ConsentAuditEntry) -> PvResult<()>
```

```sql
INSERT INTO consent_audit (sphere_id, field_name, old_value, new_value, changed_ms)
VALUES (?1, ?2, ?3, ?4, ?5)
```

Append a consent change to the audit trail. Append-only (no update/delete exposed).

#### `recent_consent_audit` (line 973)

```rust
pub fn recent_consent_audit(&self, sphere_id: &str, limit: usize) -> PvResult<Vec<ConsentAuditEntry>>
```

```sql
SELECT sphere_id, field_name, old_value, new_value, changed_ms
FROM consent_audit WHERE sphere_id = ?1
ORDER BY changed_ms DESC LIMIT ?2
```

Get recent audit entries for a specific sphere, ordered by change time descending.

### 3.7 Hebbian STDP Summary (3 methods)

#### `insert_hebbian_summary` (line 1014)

```rust
pub fn insert_hebbian_summary(&self, record: &HebbianSummaryRecord) -> PvResult<()>
```

```sql
INSERT INTO hebbian_summary
    (tick, ltp_count, ltd_count, at_floor_count, total_weight_change,
     connections_total, weight_mean, weight_min, weight_max, created_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
```

Insert one STDP summary record per tick batch.

#### `recent_hebbian_summaries` (line 1043)

```rust
pub fn recent_hebbian_summaries(&self, limit: u32) -> PvResult<Vec<HebbianSummaryRecord>>
```

```sql
SELECT tick, ltp_count, ltd_count, at_floor_count, total_weight_change,
       connections_total, weight_mean, weight_min, weight_max, created_at
FROM hebbian_summary ORDER BY tick DESC LIMIT ?1
```

Retrieve recent Hebbian summaries, most recent tick first.

#### `hebbian_summary_count` (line 1084)

```rust
pub fn hebbian_summary_count(&self) -> PvResult<u64>
```

```sql
SELECT COUNT(*) FROM hebbian_summary
```

Count total Hebbian summary records.

### 3.8 RALPH State (2 methods)

#### `save_ralph_state` (line 1096)

```rust
pub fn save_ralph_state(&self, rs: &SavedRalphState) -> PvResult<()>
```

```sql
INSERT INTO ralph_state (id, generation, completed_cycles, current_fitness,
    peak_fitness, total_proposed, total_accepted, total_rolled_back, last_phase,
    updated_at) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
ON CONFLICT(id) DO UPDATE SET
    generation=?1, completed_cycles=?2, current_fitness=?3, peak_fitness=?4,
    total_proposed=?5, total_accepted=?6, total_rolled_back=?7, last_phase=?8,
    updated_at=?9
```

Save RALPH evolution state. Always writes to `id=1` (singleton). The `updated_at` value is computed via `SystemTime::now()` at call time, not from the `SavedRalphState` struct.

#### `load_ralph_state` (line 1130)

```rust
pub fn load_ralph_state(&self) -> PvResult<Option<SavedRalphState>>
```

```sql
SELECT generation, completed_cycles, current_fitness, peak_fitness,
       total_proposed, total_accepted, total_rolled_back, last_phase
FROM ralph_state WHERE id = 1
```

Load persisted RALPH state. Returns `None` if no state has been saved yet.

### 3.9 Sessions (3 methods)

#### `save_sessions` (line 1165)

```rust
pub fn save_sessions(&self, sessions: &[SavedSession]) -> PvResult<()>
```

```sql
-- Wrapped in unchecked_transaction (BEGIN ... COMMIT)
-- For each session in the batch:
INSERT INTO sessions (session_id, pane_id, active_task_id, poll_counter,
    total_tool_calls, started_ms, persona, updated_at)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
ON CONFLICT(session_id) DO UPDATE SET
    pane_id=?2, active_task_id=?3, poll_counter=?4,
    total_tool_calls=?5, persona=?7, updated_at=?8
```

Batch-upsert active sessions within a single transaction. The `updated_at` is computed via `SystemTime::now()` at the start of the method. Note: `started_ms` is NOT updated on conflict (deliberately preserves original start time).

#### `load_sessions` (line 1204)

```rust
pub fn load_sessions(&self) -> PvResult<Vec<SavedSession>>
```

```sql
SELECT session_id, pane_id, active_task_id, poll_counter,
       total_tool_calls, started_ms, persona FROM sessions
```

Load all persisted sessions. No ordering guarantee.

#### `remove_session` (line 1240)

```rust
pub fn remove_session(&self, session_id: &str) -> PvResult<()>
```

```sql
DELETE FROM sessions WHERE session_id = ?1
```

Remove a session from the blackboard. No-op if session does not exist (no error raised).

### 3.10 Coupling Weights (2 methods)

#### `save_coupling_weights` (line 1259)

```rust
pub fn save_coupling_weights(&self, weights: &[SavedCouplingWeight]) -> PvResult<()>
```

```sql
-- Wrapped in unchecked_transaction (BEGIN ... COMMIT)
-- For each weight in the batch:
INSERT INTO coupling_weights (from_id, to_id, weight, updated_at)
VALUES (?1, ?2, ?3, ?4)
ON CONFLICT(from_id, to_id) DO UPDATE SET weight=?3, updated_at=?4
```

Batch-upsert coupling weights within a single transaction. The `updated_at` is computed via `SystemTime::now()` at the start of the method.

#### `load_coupling_weights` (line 1286)

```rust
pub fn load_coupling_weights(&self) -> PvResult<Vec<SavedCouplingWeight>>
```

```sql
SELECT from_id, to_id, weight FROM coupling_weights
```

Load all persisted coupling weights. No ordering guarantee.

### 3.11 Constructors and Migration (3 methods)

#### `open` (line 233)

```rust
pub fn open(path: &str) -> PvResult<Self>
```

Open or create a file-backed SQLite database at the given path. Calls `migrate()` immediately after opening.

#### `in_memory` (line 246)

```rust
pub fn in_memory() -> PvResult<Self>
```

Create an in-memory SQLite database (for tests). Calls `migrate()` immediately.

#### `migrate` (line 256) -- private

```rust
fn migrate(&self) -> PvResult<()>
```

Runs the full 10-table DDL + 6 index creation via a single `execute_batch`. All statements use `IF NOT EXISTS` for idempotency. Called automatically by `open()` and `in_memory()`.

---

## 4. Query Templates

Three external SQL template files exist in `.claude/queries/` for operational use outside of Rust code (diagnostics, debugging, fleet monitoring).

### 4.1 `.claude/queries/blackboard.sql`

Queries against a `blackboard` key-value table. Note: this references a conceptual `blackboard` table that is not part of the current `m26_blackboard.rs` inline schema -- it may target a separate store or represent an earlier design iteration.

```sql
-- Recent fleet knowledge entries
SELECT key, value, source_sphere, updated_at
FROM blackboard ORDER BY updated_at DESC LIMIT 20;

-- Entries by source sphere
SELECT key, value, source_sphere, updated_at
FROM blackboard WHERE source_sphere = ? ORDER BY updated_at DESC;

-- Stale entries (older than N seconds)
SELECT key, source_sphere, updated_at,
       ROUND((julianday('now') - julianday(updated_at)) * 86400) as age_secs
FROM blackboard
WHERE (julianday('now') - julianday(updated_at)) * 86400 > ?
ORDER BY updated_at ASC;

-- Entry count by source
SELECT source_sphere, COUNT(*) as entries, MAX(updated_at) as last_update
FROM blackboard GROUP BY source_sphere ORDER BY entries DESC;

-- Search blackboard by key pattern
SELECT key, value, source_sphere, updated_at
FROM blackboard WHERE key LIKE ? ORDER BY updated_at DESC LIMIT 20;
```

### 4.2 `.claude/queries/hook_events.sql`

Queries for the hook event tracking system (table: `hook_events`). This table is managed by a separate module (likely m10_hook_server), not m26_blackboard.

```sql
-- Recent hook events
SELECT id, event_type, session_id, tool_name, decision, latency_us, created_at
FROM hook_events ORDER BY created_at DESC LIMIT 30;

-- Hook event distribution
SELECT event_type, COUNT(*) as count, AVG(latency_us) as avg_latency_us
FROM hook_events GROUP BY event_type ORDER BY count DESC;

-- Permission decisions (PermissionRequest hooks only)
SELECT session_id, tool_name, decision, reason, created_at
FROM hook_events WHERE event_type = 'PermissionRequest'
ORDER BY created_at DESC LIMIT 20;

-- Denied actions (security audit trail)
SELECT session_id, event_type, tool_name, reason, created_at
FROM hook_events WHERE decision = 'deny'
ORDER BY created_at DESC LIMIT 50;

-- Per-session hook summary
SELECT session_id, COUNT(*) as total_hooks,
       SUM(CASE WHEN decision = 'approve' THEN 1 ELSE 0 END) as approved,
       SUM(CASE WHEN decision = 'deny' THEN 1 ELSE 0 END) as denied,
       AVG(latency_us) as avg_latency_us
FROM hook_events GROUP BY session_id ORDER BY total_hooks DESC LIMIT 15;

-- Slow hooks (latency > 1ms = 1000us threshold)
SELECT id, event_type, session_id, tool_name, latency_us, created_at
FROM hook_events WHERE latency_us > 1000
ORDER BY latency_us DESC LIMIT 20;

-- Thermal gate blocks (PreToolUse denials)
SELECT session_id, tool_name, reason, created_at
FROM hook_events WHERE event_type = 'PreToolUse' AND decision = 'deny'
ORDER BY created_at DESC LIMIT 20;
```

### 4.3 `.claude/queries/fleet_state.sql`

Queries for fleet state tracking. References tables from the PV2 field tracking system and ORAC bridge health monitoring.

```sql
-- Current sphere registrations (via PV2 HTTP API cache)
SELECT sphere_id, status, persona, phase, frequency, last_heartbeat
FROM sphere_cache ORDER BY last_heartbeat DESC;

-- Field snapshots (from IPC bus subscription)
SELECT tick, ROUND(r, 3) as r, sphere_count, ROUND(k_mod, 3) as k_mod,
       decision_action, timestamp
FROM field_snapshots ORDER BY tick DESC LIMIT ?;

-- R trend over last hour (720 ticks at 5s)
SELECT MIN(r) as r_min, MAX(r) as r_max, AVG(r) as r_avg, COUNT(*) as samples
FROM field_snapshots WHERE tick > (SELECT MAX(tick) - 720 FROM field_snapshots);

-- Chimera events
SELECT tick, sphere_count, decision_action, timestamp
FROM field_snapshots WHERE chimera_detected = 1 ORDER BY tick DESC LIMIT 20;

-- Bridge health status
SELECT bridge_name, port, last_status, last_check, consecutive_failures
FROM bridge_health ORDER BY last_check DESC;

-- Circuit breaker state
SELECT sphere_id, state, failure_count, last_failure, next_retry_at
FROM circuit_breaker_state ORDER BY last_failure DESC;

-- Hebbian co-activation log
SELECT tool_a, tool_b, weight, co_activations, last_updated
FROM hebbian_weights ORDER BY weight DESC LIMIT 30;

-- Task routing decisions
SELECT task_id, source_sphere, target_sphere, routing_method, score, created_at
FROM routing_decisions ORDER BY created_at DESC LIMIT 20;
```

---

## 5. WAL Configuration

### 5.1 Current State in m26_blackboard.rs

The `migrate()` method in `m26_blackboard.rs` does **not** issue any `PRAGMA` statements. There is no explicit WAL mode activation, no `journal_mode`, no `busy_timeout`, and no `synchronous` pragma in the blackboard module.

### 5.2 Design Intent (from ai_docs and config)

The ORAC design documents specify WAL mode as pattern **P13**:

- `ai_docs/modules/L5_BRIDGES_MODULES.md` line 505: "SQLite with WAL -- `PRAGMA journal_mode=WAL` (P13) for concurrent read/write"
- `ai_docs/modules/L5_BRIDGES_MODULES.md` line 627: "PRAGMA journal_mode=WAL (P13)"
- `.claude/patterns.json` P13: "PRAGMA journal_mode=WAL on all SQLite connections"

### 5.3 Config Support

The `PersistenceConfig` struct in `m1_core/m03_config.rs` includes:

```rust
pub struct PersistenceConfig {
    pub snapshot_interval: u64,
    pub wal_busy_timeout_ms: u64,  // default: 5000
    pub bus_db_path: String,
    pub field_db_path: String,
}
```

The `wal_busy_timeout_ms` field exists (default 5000ms) but is not wired into the `Blackboard::open()` path. This represents a gap between config capability and runtime behavior.

### 5.4 Recommended PRAGMAs (not yet implemented)

Per P13 and `PersistenceConfig`, the following would be expected at connection open time:

```sql
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
PRAGMA synchronous = NORMAL;  -- safe with WAL
```

---

## 6. Migration History

### 6.1 Migration Approach

ORAC uses **inline schema migration** via `Blackboard::migrate()` -- a single Rust method that calls `execute_batch()` with the full DDL on every `open()` or `in_memory()` call.

- **No versioning table:** There is no `schema_version` or `migrations` tracking table.
- **Idempotency:** All `CREATE TABLE` and `CREATE INDEX` statements use `IF NOT EXISTS`.
- **Single batch:** All 10 tables and 6 indexes are created in one `execute_batch` call.
- **Additive only:** New tables/columns can be appended to the batch. Destructive changes (column drops, renames) would require manual migration logic.

### 6.2 Migration File: `migrations/001_blackboard.sql`

This file contains the **v1 schema** (5 tables, 6 indexes) and is **superseded** by the inline `migrate()` method which has grown to 10 tables.

**v1 tables (migration file):**

| Table | Status |
|-------|--------|
| `pane_status` | Superseded (v1 had `last_seen INTEGER`, `tool_name TEXT`; current has `persona TEXT`, `updated_at REAL`, `tasks_completed INTEGER`) |
| `task_history` | Superseded (v1 had `status TEXT`, `created_at INTEGER`, `completed_at INTEGER`, FK; current has `outcome TEXT`, `finished_at REAL`, `duration_secs REAL`, no FK) |
| `agent_cards` | Superseded (v1 had `token_budget INTEGER`, FK; current has `model TEXT`, `registered_at REAL`, no FK) |
| `coupling_snapshot` | Superseded (v1 name `coupling_snapshot`; current name `coupling_weights`) |
| `fleet_metrics` | Removed from current schema (not in `migrate()`) |

**v1 indexes not in current schema:**

| Index | Table | Status |
|-------|-------|--------|
| `idx_pane_status_status` | `pane_status` | Removed (not in current migrate) |
| `idx_pane_status_last_seen` | `pane_status` | Removed (column renamed to `updated_at`) |
| `idx_task_history_created_at` | `task_history` | Removed (column removed) |
| `idx_task_history_status` | `task_history` | Removed (column renamed to `outcome`) |
| `idx_agent_cards_domain` | `agent_cards` | Removed |
| `idx_coupling_snapshot_updated_at` | `coupling_snapshot` | Removed (table renamed) |
| `idx_fleet_metrics_timestamp` | `fleet_metrics` | Removed (table removed) |

### 6.3 Schema Evolution Summary

| Phase | Tables | Source |
|-------|--------|--------|
| v1 (Session 053) | 5: pane_status, task_history, agent_cards, coupling_snapshot, fleet_metrics | `migrations/001_blackboard.sql` |
| v2 (Session 055) | +3: ghost_traces, consent_declarations, consent_audit | inline `migrate()` |
| v3 (Session 059) | +2: ralph_state, hebbian_summary | inline `migrate()` |
| v4 (Session 060) | +2: sessions, coupling_weights (replaced coupling_snapshot) | inline `migrate()` |
| **Current** | **10 tables, 6 indexes** | inline `migrate()` only |

---

## Appendix A: Data Type Quick Reference

| Rust Type | SQLite Affinity | Notes |
|-----------|----------------|-------|
| `PaneId` (newtype over String) | TEXT | Serialized via `.as_str()` |
| `PaneStatus` (enum) | TEXT | Serialized via `Display`, parsed via `parse_status()` |
| `String` | TEXT | Direct binding |
| `f64` | REAL | Used for timestamps (epoch seconds) and weights |
| `u64` | INTEGER | Counts, durations (ms), ticks |
| `bool` | INTEGER | 0/1 mapping (rusqlite handles this automatically) |
| `Option<String>` | TEXT (nullable) | NULL when `None` (e.g. `active_task_id`) |
| `Vec<String>` | TEXT | JSON-serialized via `serde_json` |

## Appendix B: Transaction Usage

Two methods use explicit transactions via `unchecked_transaction()`:

| Method | Reason |
|--------|--------|
| `save_sessions` | Batch upsert of multiple session records atomically |
| `save_coupling_weights` | Batch upsert of multiple weight records atomically |

All other methods execute single statements (implicit autocommit transactions).

## Appendix C: Test Coverage

The module contains **79 tests** organized in 5 test modules:

| Module | Tests | Focus |
|--------|-------|-------|
| `tests` (root) | 5 | parse_status, data type construction |
| `sqlite_tests` | 35 | Pane CRUD, task CRUD, agent card CRUD, ghost traces, pruning, persistence across reopen |
| `consent_tests` | 8 | Consent upsert/get/list, audit insert/query/ordering/isolation |
| `hebbian_tests` | 10 | Summary insert/count, ordering, field round-trip, edge cases |
| `session_tests` | 14 | Session save/load/remove, coupling weight save/load/upsert |

All SQLite tests are feature-gated with `#[cfg(feature = "persistence")]` and use `Blackboard::in_memory()` except the 4 persistence-across-reopen tests which use temp file databases.
