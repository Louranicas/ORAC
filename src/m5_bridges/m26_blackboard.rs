//! # M26: Blackboard
//!
//! `SQLite`-backed shared fleet state for cross-pane coordination.
//! Stores pane status, task history, and agent capability cards.
//!
//! ## Tables
//!
//! - `pane_status` — current status and metadata for each registered pane
//! - `task_history` — completed/failed task records for audit trail
//! - `agent_cards` — capability declarations per pane (A2A-inspired)
//!
//! ## Layer: L5 (Bridges)
//! ## Module: M26
//! ## Dependencies: `m01_core_types`, `m02_error_handling`
//! ## Feature Gate: `persistence` (`rusqlite`)

use crate::m1_core::{
    m01_core_types::{PaneId, PaneStatus},
    m02_error_handling::{PvError, PvResult},
};

#[cfg(feature = "persistence")]
use rusqlite::{params, Connection, OptionalExtension};

// ──────────────────────────────────────────────────────────────
// Data types
// ──────────────────────────────────────────────────────────────

/// A pane's current status record on the blackboard.
#[derive(Debug, Clone)]
pub struct PaneRecord {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// Current operational status.
    pub status: PaneStatus,
    /// Human-readable persona name.
    pub persona: String,
    /// Unix timestamp of last update.
    pub updated_at: f64,
    /// Current phase on the Kuramoto ring.
    pub phase: f64,
    /// Number of tasks completed by this pane.
    pub tasks_completed: u64,
}

/// A completed or failed task record.
#[derive(Debug, Clone)]
pub struct TaskRecord {
    /// Task ID.
    pub task_id: String,
    /// Pane that executed the task.
    pub pane_id: PaneId,
    /// Brief task description.
    pub description: String,
    /// Outcome: "completed" or "failed".
    pub outcome: String,
    /// Unix timestamp when the task finished.
    pub finished_at: f64,
    /// Duration in seconds.
    pub duration_secs: f64,
}

/// An agent capability card (A2A-inspired).
#[derive(Debug, Clone)]
pub struct AgentCard {
    /// Pane identifier.
    pub pane_id: PaneId,
    /// List of capabilities/skills.
    pub capabilities: Vec<String>,
    /// Domain specialization (e.g. "rust", "frontend", "devops").
    pub domain: String,
    /// Model being used.
    pub model: String,
    /// Unix timestamp of card registration.
    pub registered_at: f64,
}

/// Ghost trace of a deregistered sphere, persisted to `SQLite`.
#[derive(Debug, Clone)]
pub struct GhostRecord {
    /// Sphere ID at time of departure.
    pub sphere_id: String,
    /// Persona string.
    pub persona: String,
    /// Epoch milliseconds when deregistered.
    pub deregistered_ms: u64,
    /// Kuramoto phase at departure.
    pub final_phase: f64,
    /// Total tool calls during the session.
    pub total_tools: u64,
    /// Session wall-clock duration in milliseconds.
    pub session_duration_ms: u64,
}

/// Consent declaration persisted to `SQLite`.
#[derive(Debug, Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ConsentRecord {
    /// Sphere ID this consent applies to.
    pub sphere_id: String,
    /// Allow SYNTHEX bridge writes.
    pub synthex_write: bool,
    /// Allow POVM bridge reads.
    pub povm_read: bool,
    /// Allow POVM bridge writes.
    pub povm_write: bool,
    /// Allow session hydration from POVM + RM.
    pub hydration: bool,
    /// Epoch milliseconds when last updated.
    pub updated_ms: u64,
}

/// Consent audit trail entry, persisted to `SQLite`.
#[derive(Debug, Clone)]
pub struct ConsentAuditEntry {
    /// Sphere ID whose consent was modified.
    pub sphere_id: String,
    /// Name of the field that changed (e.g. `"synthex_write"`).
    pub field_name: String,
    /// Previous value before the change.
    pub old_value: bool,
    /// New value after the change.
    pub new_value: bool,
    /// Epoch milliseconds when the change occurred.
    pub changed_ms: u64,
}

// ──────────────────────────────────────────────────────────────
// Blackboard (SQLite-backed)
// ──────────────────────────────────────────────────────────────

/// `SQLite`-backed shared fleet state.
///
/// Provides persistent storage for pane status, task history, and agent cards.
/// Use [`Blackboard::open`] for file-backed or [`Blackboard::in_memory`] for tests.
#[cfg(feature = "persistence")]
pub struct Blackboard {
    conn: Connection,
}

#[cfg(feature = "persistence")]
impl std::fmt::Debug for Blackboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Blackboard")
            .field("path", &"<sqlite>")
            .finish()
    }
}

#[cfg(feature = "persistence")]
impl Blackboard {
    /// Open or create a blackboard at the given path.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] if the database cannot be opened or migrations fail.
    pub fn open(path: &str) -> PvResult<Self> {
        let conn = Connection::open(path)
            .map_err(|e| PvError::Database(format!("open {path}: {e}")))?;
        let bb = Self { conn };
        bb.migrate()?;
        Ok(bb)
    }

    /// Create an in-memory blackboard (for tests).
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] if migrations fail.
    pub fn in_memory() -> PvResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| PvError::Database(format!("in-memory: {e}")))?;
        let bb = Self { conn };
        bb.migrate()?;
        Ok(bb)
    }

    /// Run schema migrations.
    fn migrate(&self) -> PvResult<()> {
        self.conn
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS pane_status (
                    pane_id     TEXT PRIMARY KEY,
                    status      TEXT NOT NULL DEFAULT 'Idle',
                    persona     TEXT NOT NULL DEFAULT '',
                    updated_at  REAL NOT NULL DEFAULT 0.0,
                    phase       REAL NOT NULL DEFAULT 0.0,
                    tasks_completed INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS task_history (
                    task_id      TEXT PRIMARY KEY,
                    pane_id      TEXT NOT NULL,
                    description  TEXT NOT NULL DEFAULT '',
                    outcome      TEXT NOT NULL DEFAULT 'completed',
                    finished_at  REAL NOT NULL DEFAULT 0.0,
                    duration_secs REAL NOT NULL DEFAULT 0.0
                );

                CREATE TABLE IF NOT EXISTS agent_cards (
                    pane_id       TEXT PRIMARY KEY,
                    capabilities  TEXT NOT NULL DEFAULT '[]',
                    domain        TEXT NOT NULL DEFAULT '',
                    model         TEXT NOT NULL DEFAULT '',
                    registered_at REAL NOT NULL DEFAULT 0.0
                );

                CREATE TABLE IF NOT EXISTS ghost_traces (
                    sphere_id         TEXT NOT NULL,
                    persona           TEXT NOT NULL DEFAULT '',
                    deregistered_ms   INTEGER NOT NULL,
                    final_phase       REAL NOT NULL DEFAULT 0.0,
                    total_tools       INTEGER NOT NULL DEFAULT 0,
                    session_duration_ms INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS consent_declarations (
                    sphere_id       TEXT PRIMARY KEY,
                    synthex_write   INTEGER NOT NULL DEFAULT 1,
                    povm_read       INTEGER NOT NULL DEFAULT 1,
                    povm_write      INTEGER NOT NULL DEFAULT 0,
                    hydration       INTEGER NOT NULL DEFAULT 1,
                    updated_ms      INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS consent_audit (
                    sphere_id   TEXT NOT NULL,
                    field_name  TEXT NOT NULL,
                    old_value   INTEGER NOT NULL,
                    new_value   INTEGER NOT NULL,
                    changed_ms  INTEGER NOT NULL
                );

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
                ",
            )
            .map_err(|e| PvError::Database(format!("migrate: {e}")))?;
        Ok(())
    }

    // ── Pane status ──

    /// Upsert a pane's status record.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn upsert_pane(&self, record: &PaneRecord) -> PvResult<()> {
        self.conn
            .execute(
                "INSERT INTO pane_status (pane_id, status, persona, updated_at, phase, tasks_completed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(pane_id) DO UPDATE SET
                     status = excluded.status,
                     persona = excluded.persona,
                     updated_at = excluded.updated_at,
                     phase = excluded.phase,
                     tasks_completed = excluded.tasks_completed",
                params![
                    record.pane_id.as_str(),
                    format!("{}", record.status),
                    record.persona,
                    record.updated_at,
                    record.phase,
                    record.tasks_completed,
                ],
            )
            .map_err(|e| PvError::Database(format!("upsert_pane: {e}")))?;
        Ok(())
    }

    /// Get a pane's status record.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn get_pane(&self, pane_id: &PaneId) -> PvResult<Option<PaneRecord>> {
        self.conn
            .query_row(
                "SELECT pane_id, status, persona, updated_at, phase, tasks_completed
                 FROM pane_status WHERE pane_id = ?1",
                params![pane_id.as_str()],
                |row| {
                    Ok(PaneRecord {
                        pane_id: PaneId::new(row.get::<_, String>(0)?),
                        status: parse_status(&row.get::<_, String>(1)?),
                        persona: row.get(2)?,
                        updated_at: row.get(3)?,
                        phase: row.get(4)?,
                        tasks_completed: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|e| PvError::Database(format!("get_pane: {e}")))
    }

    /// List all pane records.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn list_panes(&self) -> PvResult<Vec<PaneRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT pane_id, status, persona, updated_at, phase, tasks_completed
                 FROM pane_status ORDER BY pane_id",
            )
            .map_err(|e| PvError::Database(format!("list_panes prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(PaneRecord {
                    pane_id: PaneId::new(row.get::<_, String>(0)?),
                    status: parse_status(&row.get::<_, String>(1)?),
                    persona: row.get(2)?,
                    updated_at: row.get(3)?,
                    phase: row.get(4)?,
                    tasks_completed: row.get(5)?,
                })
            })
            .map_err(|e| PvError::Database(format!("list_panes query: {e}")))?;

        let mut panes = Vec::new();
        for row in rows {
            panes.push(row.map_err(|e| PvError::Database(format!("list_panes row: {e}")))?);
        }
        Ok(panes)
    }

    /// Remove a pane's status record (deregistration).
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn remove_pane(&self, pane_id: &PaneId) -> PvResult<bool> {
        let rows = self
            .conn
            .execute(
                "DELETE FROM pane_status WHERE pane_id = ?1",
                params![pane_id.as_str()],
            )
            .map_err(|e| PvError::Database(format!("remove_pane: {e}")))?;
        Ok(rows > 0)
    }

    /// Count registered panes.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn pane_count(&self) -> PvResult<usize> {
        self.conn
            .query_row("SELECT COUNT(*) FROM pane_status", [], |row| row.get(0))
            .map_err(|e| PvError::Database(format!("pane_count: {e}")))
    }

    /// Remove `pane_status` entries with `updated_at` older than `cutoff_secs` (Unix epoch seconds).
    ///
    /// Also removes associated `agent_cards` for pruned panes.
    /// Returns the number of pane records deleted.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn prune_stale_panes(&self, cutoff_secs: f64) -> PvResult<usize> {
        // Delete agent cards for stale panes first (no FK cascade in SQLite by default)
        self.conn
            .execute(
                "DELETE FROM agent_cards WHERE pane_id IN
                 (SELECT pane_id FROM pane_status WHERE updated_at < ?1)",
                params![cutoff_secs],
            )
            .map_err(|e| PvError::Database(format!("prune_stale_panes cards: {e}")))?;

        let deleted = self
            .conn
            .execute(
                "DELETE FROM pane_status WHERE updated_at < ?1",
                params![cutoff_secs],
            )
            .map_err(|e| PvError::Database(format!("prune_stale_panes: {e}")))?;
        Ok(deleted)
    }

    /// Remove only `Complete` panes with `updated_at` older than `cutoff_secs`.
    ///
    /// Unlike [`prune_stale_panes`], this preserves Idle/Working/Blocked panes
    /// regardless of age. Also removes associated `agent_cards` for pruned panes.
    /// Returns the number of pane records deleted.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn prune_complete_panes(&self, cutoff_secs: f64) -> PvResult<usize> {
        self.conn
            .execute(
                "DELETE FROM agent_cards WHERE pane_id IN
                 (SELECT pane_id FROM pane_status WHERE status = 'Complete' AND updated_at < ?1)",
                params![cutoff_secs],
            )
            .map_err(|e| PvError::Database(format!("prune_complete_panes cards: {e}")))?;

        let deleted = self
            .conn
            .execute(
                "DELETE FROM pane_status WHERE status = 'Complete' AND updated_at < ?1",
                params![cutoff_secs],
            )
            .map_err(|e| PvError::Database(format!("prune_complete_panes: {e}")))?;
        Ok(deleted)
    }

    /// Remove `task_history` entries with `finished_at` older than `cutoff_secs` (Unix epoch seconds).
    ///
    /// Returns the number of task records deleted.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn prune_old_tasks(&self, cutoff_secs: f64) -> PvResult<usize> {
        let deleted = self
            .conn
            .execute(
                "DELETE FROM task_history WHERE finished_at < ?1",
                params![cutoff_secs],
            )
            .map_err(|e| PvError::Database(format!("prune_old_tasks: {e}")))?;
        Ok(deleted)
    }

    // ── Task history ──

    /// Insert a task record.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn insert_task(&self, task: &TaskRecord) -> PvResult<()> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO task_history
                 (task_id, pane_id, description, outcome, finished_at, duration_secs)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    task.task_id,
                    task.pane_id.as_str(),
                    task.description,
                    task.outcome,
                    task.finished_at,
                    task.duration_secs,
                ],
            )
            .map_err(|e| PvError::Database(format!("insert_task: {e}")))?;
        Ok(())
    }

    /// Get recent tasks for a pane (most recent first).
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn recent_tasks(&self, pane_id: &PaneId, limit: usize) -> PvResult<Vec<TaskRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT task_id, pane_id, description, outcome, finished_at, duration_secs
                 FROM task_history WHERE pane_id = ?1
                 ORDER BY finished_at DESC LIMIT ?2",
            )
            .map_err(|e| PvError::Database(format!("recent_tasks prepare: {e}")))?;

        let rows = stmt
            .query_map(params![pane_id.as_str(), limit], |row| {
                Ok(TaskRecord {
                    task_id: row.get(0)?,
                    pane_id: PaneId::new(row.get::<_, String>(1)?),
                    description: row.get(2)?,
                    outcome: row.get(3)?,
                    finished_at: row.get(4)?,
                    duration_secs: row.get(5)?,
                })
            })
            .map_err(|e| PvError::Database(format!("recent_tasks query: {e}")))?;

        let mut tasks = Vec::new();
        for row in rows {
            tasks.push(row.map_err(|e| PvError::Database(format!("recent_tasks row: {e}")))?);
        }
        Ok(tasks)
    }

    /// Count total tasks in history.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn task_count(&self) -> PvResult<usize> {
        self.conn
            .query_row("SELECT COUNT(*) FROM task_history", [], |row| row.get(0))
            .map_err(|e| PvError::Database(format!("task_count: {e}")))
    }

    // ── Agent cards ──

    /// Upsert an agent capability card.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn upsert_card(&self, card: &AgentCard) -> PvResult<()> {
        let caps_json = serde_json::to_string(&card.capabilities)
            .map_err(|e| PvError::Database(format!("serialize capabilities: {e}")))?;
        self.conn
            .execute(
                "INSERT INTO agent_cards (pane_id, capabilities, domain, model, registered_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(pane_id) DO UPDATE SET
                     capabilities = excluded.capabilities,
                     domain = excluded.domain,
                     model = excluded.model,
                     registered_at = excluded.registered_at",
                params![
                    card.pane_id.as_str(),
                    caps_json,
                    card.domain,
                    card.model,
                    card.registered_at,
                ],
            )
            .map_err(|e| PvError::Database(format!("upsert_card: {e}")))?;
        Ok(())
    }

    /// Get an agent's capability card.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn get_card(&self, pane_id: &PaneId) -> PvResult<Option<AgentCard>> {
        self.conn
            .query_row(
                "SELECT pane_id, capabilities, domain, model, registered_at
                 FROM agent_cards WHERE pane_id = ?1",
                params![pane_id.as_str()],
                |row| {
                    let caps_str: String = row.get(1)?;
                    let capabilities: Vec<String> =
                        serde_json::from_str(&caps_str).unwrap_or_default();
                    Ok(AgentCard {
                        pane_id: PaneId::new(row.get::<_, String>(0)?),
                        capabilities,
                        domain: row.get(2)?,
                        model: row.get(3)?,
                        registered_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|e| PvError::Database(format!("get_card: {e}")))
    }

    /// List all agent cards.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn list_cards(&self) -> PvResult<Vec<AgentCard>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT pane_id, capabilities, domain, model, registered_at
                 FROM agent_cards ORDER BY pane_id",
            )
            .map_err(|e| PvError::Database(format!("list_cards prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let caps_str: String = row.get(1)?;
                let capabilities: Vec<String> =
                    serde_json::from_str(&caps_str).unwrap_or_default();
                Ok(AgentCard {
                    pane_id: PaneId::new(row.get::<_, String>(0)?),
                    capabilities,
                    domain: row.get(2)?,
                    model: row.get(3)?,
                    registered_at: row.get(4)?,
                })
            })
            .map_err(|e| PvError::Database(format!("list_cards query: {e}")))?;

        let mut cards = Vec::new();
        for row in rows {
            cards.push(row.map_err(|e| PvError::Database(format!("list_cards row: {e}")))?);
        }
        Ok(cards)
    }

    /// Remove an agent card (deregistration).
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn remove_card(&self, pane_id: &PaneId) -> PvResult<bool> {
        let rows = self
            .conn
            .execute(
                "DELETE FROM agent_cards WHERE pane_id = ?1",
                params![pane_id.as_str()],
            )
            .map_err(|e| PvError::Database(format!("remove_card: {e}")))?;
        Ok(rows > 0)
    }

    /// Count registered agent cards.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn card_count(&self) -> PvResult<usize> {
        self.conn
            .query_row("SELECT COUNT(*) FROM agent_cards", [], |row| row.get(0))
            .map_err(|e| PvError::Database(format!("card_count: {e}")))
    }

    // ── Ghost traces ──

    /// Insert a ghost trace record.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn insert_ghost(&self, ghost: &GhostRecord) -> PvResult<()> {
        self.conn
            .execute(
                "INSERT INTO ghost_traces
                 (sphere_id, persona, deregistered_ms, final_phase, total_tools, session_duration_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    ghost.sphere_id,
                    ghost.persona,
                    ghost.deregistered_ms,
                    ghost.final_phase,
                    ghost.total_tools,
                    ghost.session_duration_ms,
                ],
            )
            .map_err(|e| PvError::Database(format!("insert_ghost: {e}")))?;
        Ok(())
    }

    /// Get the most recent ghost traces (newest first).
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn recent_ghosts(&self, limit: usize) -> PvResult<Vec<GhostRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT sphere_id, persona, deregistered_ms, final_phase,
                        total_tools, session_duration_ms
                 FROM ghost_traces
                 ORDER BY deregistered_ms DESC LIMIT ?1",
            )
            .map_err(|e| PvError::Database(format!("recent_ghosts prepare: {e}")))?;

        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(GhostRecord {
                    sphere_id: row.get(0)?,
                    persona: row.get(1)?,
                    deregistered_ms: row.get(2)?,
                    final_phase: row.get(3)?,
                    total_tools: row.get(4)?,
                    session_duration_ms: row.get(5)?,
                })
            })
            .map_err(|e| PvError::Database(format!("recent_ghosts query: {e}")))?;

        let mut ghosts = Vec::new();
        for row in rows {
            ghosts.push(row.map_err(|e| PvError::Database(format!("recent_ghosts row: {e}")))?);
        }
        Ok(ghosts)
    }

    /// Count total ghost traces.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn ghost_count(&self) -> PvResult<usize> {
        self.conn
            .query_row("SELECT COUNT(*) FROM ghost_traces", [], |row| row.get(0))
            .map_err(|e| PvError::Database(format!("ghost_count: {e}")))
    }

    /// Prune old ghost traces, keeping only the most recent `keep` entries.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn prune_ghosts(&self, keep: usize) -> PvResult<usize> {
        let deleted = self
            .conn
            .execute(
                "DELETE FROM ghost_traces WHERE rowid NOT IN
                 (SELECT rowid FROM ghost_traces ORDER BY deregistered_ms DESC LIMIT ?1)",
                params![keep],
            )
            .map_err(|e| PvError::Database(format!("prune_ghosts: {e}")))?;
        Ok(deleted)
    }

    // ── Consent declarations ──

    /// Upsert a consent declaration for a sphere.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn upsert_consent(&self, record: &ConsentRecord) -> PvResult<()> {
        self.conn
            .execute(
                "INSERT INTO consent_declarations (sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(sphere_id) DO UPDATE SET
                     synthex_write = excluded.synthex_write,
                     povm_read = excluded.povm_read,
                     povm_write = excluded.povm_write,
                     hydration = excluded.hydration,
                     updated_ms = excluded.updated_ms",
                params![
                    record.sphere_id,
                    record.synthex_write,
                    record.povm_read,
                    record.povm_write,
                    record.hydration,
                    record.updated_ms,
                ],
            )
            .map_err(|e| PvError::Database(format!("upsert_consent: {e}")))?;
        Ok(())
    }

    /// Get a consent declaration for a sphere.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn get_consent_record(&self, sphere_id: &str) -> PvResult<Option<ConsentRecord>> {
        self.conn
            .query_row(
                "SELECT sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms
                 FROM consent_declarations WHERE sphere_id = ?1",
                params![sphere_id],
                |row| {
                    Ok(ConsentRecord {
                        sphere_id: row.get(0)?,
                        synthex_write: row.get(1)?,
                        povm_read: row.get(2)?,
                        povm_write: row.get(3)?,
                        hydration: row.get(4)?,
                        updated_ms: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(|e| PvError::Database(format!("get_consent_record: {e}")))
    }

    /// List all consent declarations.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn list_consents(&self) -> PvResult<Vec<ConsentRecord>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT sphere_id, synthex_write, povm_read, povm_write, hydration, updated_ms
                 FROM consent_declarations ORDER BY updated_ms DESC",
            )
            .map_err(|e| PvError::Database(format!("list_consents prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(ConsentRecord {
                    sphere_id: row.get(0)?,
                    synthex_write: row.get(1)?,
                    povm_read: row.get(2)?,
                    povm_write: row.get(3)?,
                    hydration: row.get(4)?,
                    updated_ms: row.get(5)?,
                })
            })
            .map_err(|e| PvError::Database(format!("list_consents query: {e}")))?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row.map_err(|e| PvError::Database(format!("list_consents row: {e}")))?);
        }
        Ok(records)
    }

    // ── Consent audit ──

    /// Record a consent field change in the audit trail.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn insert_consent_audit(&self, entry: &ConsentAuditEntry) -> PvResult<()> {
        self.conn
            .execute(
                "INSERT INTO consent_audit (sphere_id, field_name, old_value, new_value, changed_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    entry.sphere_id,
                    entry.field_name,
                    entry.old_value,
                    entry.new_value,
                    entry.changed_ms,
                ],
            )
            .map_err(|e| PvError::Database(format!("insert_consent_audit: {e}")))?;
        Ok(())
    }

    /// Get recent consent audit entries for a sphere.
    ///
    /// # Errors
    ///
    /// Returns [`PvError::Database`] on `SQLite` failure.
    pub fn recent_consent_audit(
        &self,
        sphere_id: &str,
        limit: usize,
    ) -> PvResult<Vec<ConsentAuditEntry>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT sphere_id, field_name, old_value, new_value, changed_ms
                 FROM consent_audit WHERE sphere_id = ?1
                 ORDER BY changed_ms DESC LIMIT ?2",
            )
            .map_err(|e| PvError::Database(format!("recent_consent_audit prepare: {e}")))?;

        let rows = stmt
            .query_map(params![sphere_id, limit], |row| {
                Ok(ConsentAuditEntry {
                    sphere_id: row.get(0)?,
                    field_name: row.get(1)?,
                    old_value: row.get(2)?,
                    new_value: row.get(3)?,
                    changed_ms: row.get(4)?,
                })
            })
            .map_err(|e| PvError::Database(format!("recent_consent_audit query: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            entries
                .push(row.map_err(|e| PvError::Database(format!("consent_audit row: {e}")))?);
        }
        Ok(entries)
    }
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Parse a status string back to `PaneStatus`.
fn parse_status(s: &str) -> PaneStatus {
    match s {
        "Working" => PaneStatus::Working,
        "Blocked" => PaneStatus::Blocked,
        "Complete" => PaneStatus::Complete,
        _ => PaneStatus::Idle,
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    // ── parse_status ──

    #[test]
    fn parse_status_idle() {
        assert_eq!(parse_status("Idle"), PaneStatus::Idle);
    }

    #[test]
    fn parse_status_working() {
        assert_eq!(parse_status("Working"), PaneStatus::Working);
    }

    #[test]
    fn parse_status_blocked() {
        assert_eq!(parse_status("Blocked"), PaneStatus::Blocked);
    }

    #[test]
    fn parse_status_complete() {
        assert_eq!(parse_status("Complete"), PaneStatus::Complete);
    }

    #[test]
    fn parse_status_unknown_defaults_idle() {
        assert_eq!(parse_status("xyz"), PaneStatus::Idle);
        assert_eq!(parse_status(""), PaneStatus::Idle);
    }

    // ── Data type construction ──

    #[test]
    fn pane_record_fields() {
        let r = PaneRecord {
            pane_id: pid("alpha"),
            status: PaneStatus::Working,
            persona: "test-agent".into(),
            updated_at: 1000.0,
            phase: 1.5,
            tasks_completed: 42,
        };
        assert_eq!(r.pane_id.as_str(), "alpha");
        assert_eq!(r.status, PaneStatus::Working);
        assert_eq!(r.tasks_completed, 42);
    }

    #[test]
    fn task_record_fields() {
        let t = TaskRecord {
            task_id: "task-001".into(),
            pane_id: pid("alpha"),
            description: "fix bug".into(),
            outcome: "completed".into(),
            finished_at: 2000.0,
            duration_secs: 30.0,
        };
        assert_eq!(t.task_id, "task-001");
        assert_eq!(t.outcome, "completed");
    }

    #[test]
    fn agent_card_fields() {
        let c = AgentCard {
            pane_id: pid("alpha"),
            capabilities: vec!["rust".into(), "testing".into()],
            domain: "backend".into(),
            model: "opus-4.6".into(),
            registered_at: 3000.0,
        };
        assert_eq!(c.capabilities.len(), 2);
        assert_eq!(c.domain, "backend");
    }

    // ── Shared test helpers (persistence-gated) ──

    #[cfg(feature = "persistence")]
    fn bb() -> Blackboard {
        Blackboard::in_memory().unwrap()
    }

    #[cfg(feature = "persistence")]
    fn sample_pane(id: &str, status: PaneStatus) -> PaneRecord {
        PaneRecord {
            pane_id: pid(id),
            status,
            persona: format!("agent-{id}"),
            updated_at: 1000.0,
            phase: 0.5,
            tasks_completed: 0,
        }
    }

    #[cfg(feature = "persistence")]
    fn sample_task(id: &str, pane: &str) -> TaskRecord {
        TaskRecord {
            task_id: id.into(),
            pane_id: pid(pane),
            description: format!("task {id}"),
            outcome: "completed".into(),
            finished_at: 2000.0,
            duration_secs: 10.0,
        }
    }

    #[cfg(feature = "persistence")]
    fn sample_card(id: &str) -> AgentCard {
        AgentCard {
            pane_id: pid(id),
            capabilities: vec!["rust".into(), "testing".into()],
            domain: "backend".into(),
            model: "opus-4.6".into(),
            registered_at: 3000.0,
        }
    }

    // ── SQLite tests (feature-gated) ──

    #[cfg(feature = "persistence")]
    mod sqlite_tests {
        use super::*;

        // ── Blackboard creation ──

        #[test]
        fn in_memory_creates_tables() {
            let b = bb();
            assert_eq!(b.pane_count().unwrap(), 0);
            assert_eq!(b.task_count().unwrap(), 0);
            assert_eq!(b.card_count().unwrap(), 0);
        }

        // ── Pane status ──

        #[test]
        fn upsert_and_get_pane() {
            let b = bb();
            let rec = sample_pane("alpha", PaneStatus::Working);
            b.upsert_pane(&rec).unwrap();
            let got = b.get_pane(&pid("alpha")).unwrap();
            assert!(got.is_some());
            let got = got.unwrap();
            assert_eq!(got.status, PaneStatus::Working);
            assert_eq!(got.persona, "agent-alpha");
        }

        #[test]
        fn upsert_pane_overwrites() {
            let b = bb();
            let mut rec = sample_pane("alpha", PaneStatus::Idle);
            b.upsert_pane(&rec).unwrap();
            rec.status = PaneStatus::Working;
            rec.tasks_completed = 5;
            b.upsert_pane(&rec).unwrap();
            let got = b.get_pane(&pid("alpha")).unwrap().unwrap();
            assert_eq!(got.status, PaneStatus::Working);
            assert_eq!(got.tasks_completed, 5);
            assert_eq!(b.pane_count().unwrap(), 1);
        }

        #[test]
        fn get_pane_not_found() {
            let b = bb();
            assert!(b.get_pane(&pid("nope")).unwrap().is_none());
        }

        #[test]
        fn list_panes_empty() {
            let b = bb();
            assert!(b.list_panes().unwrap().is_empty());
        }

        #[test]
        fn list_panes_multiple() {
            let b = bb();
            b.upsert_pane(&sample_pane("alpha", PaneStatus::Idle)).unwrap();
            b.upsert_pane(&sample_pane("beta", PaneStatus::Working)).unwrap();
            let panes = b.list_panes().unwrap();
            assert_eq!(panes.len(), 2);
        }

        #[test]
        fn list_panes_sorted_by_id() {
            let b = bb();
            b.upsert_pane(&sample_pane("charlie", PaneStatus::Idle)).unwrap();
            b.upsert_pane(&sample_pane("alpha", PaneStatus::Idle)).unwrap();
            let panes = b.list_panes().unwrap();
            assert_eq!(panes[0].pane_id.as_str(), "alpha");
            assert_eq!(panes[1].pane_id.as_str(), "charlie");
        }

        #[test]
        fn remove_pane_exists() {
            let b = bb();
            b.upsert_pane(&sample_pane("alpha", PaneStatus::Idle)).unwrap();
            assert!(b.remove_pane(&pid("alpha")).unwrap());
            assert_eq!(b.pane_count().unwrap(), 0);
        }

        #[test]
        fn remove_pane_not_found() {
            let b = bb();
            assert!(!b.remove_pane(&pid("nope")).unwrap());
        }

        #[test]
        fn pane_count() {
            let b = bb();
            assert_eq!(b.pane_count().unwrap(), 0);
            b.upsert_pane(&sample_pane("a", PaneStatus::Idle)).unwrap();
            assert_eq!(b.pane_count().unwrap(), 1);
            b.upsert_pane(&sample_pane("b", PaneStatus::Idle)).unwrap();
            assert_eq!(b.pane_count().unwrap(), 2);
        }

        // ── Task history ──

        #[test]
        fn insert_and_get_task() {
            let b = bb();
            let t = sample_task("t1", "alpha");
            b.insert_task(&t).unwrap();
            let tasks = b.recent_tasks(&pid("alpha"), 10).unwrap();
            assert_eq!(tasks.len(), 1);
            assert_eq!(tasks[0].task_id, "t1");
        }

        #[test]
        fn recent_tasks_ordered_desc() {
            let b = bb();
            let mut t1 = sample_task("t1", "alpha");
            t1.finished_at = 1000.0;
            let mut t2 = sample_task("t2", "alpha");
            t2.finished_at = 2000.0;
            b.insert_task(&t1).unwrap();
            b.insert_task(&t2).unwrap();
            let tasks = b.recent_tasks(&pid("alpha"), 10).unwrap();
            assert_eq!(tasks[0].task_id, "t2");
            assert_eq!(tasks[1].task_id, "t1");
        }

        #[test]
        fn recent_tasks_limited() {
            let b = bb();
            for i in 0..10 {
                let mut t = sample_task(&format!("t{i}"), "alpha");
                t.finished_at = f64::from(i);
                b.insert_task(&t).unwrap();
            }
            let tasks = b.recent_tasks(&pid("alpha"), 3).unwrap();
            assert_eq!(tasks.len(), 3);
        }

        #[test]
        fn recent_tasks_filters_by_pane() {
            let b = bb();
            b.insert_task(&sample_task("t1", "alpha")).unwrap();
            b.insert_task(&sample_task("t2", "beta")).unwrap();
            let alpha_tasks = b.recent_tasks(&pid("alpha"), 10).unwrap();
            assert_eq!(alpha_tasks.len(), 1);
            assert_eq!(alpha_tasks[0].task_id, "t1");
        }

        #[test]
        fn task_count() {
            let b = bb();
            assert_eq!(b.task_count().unwrap(), 0);
            b.insert_task(&sample_task("t1", "alpha")).unwrap();
            b.insert_task(&sample_task("t2", "beta")).unwrap();
            assert_eq!(b.task_count().unwrap(), 2);
        }

        #[test]
        fn insert_task_replace_on_conflict() {
            let b = bb();
            let mut t = sample_task("t1", "alpha");
            t.outcome = "completed".into();
            b.insert_task(&t).unwrap();
            t.outcome = "failed".into();
            b.insert_task(&t).unwrap();
            assert_eq!(b.task_count().unwrap(), 1);
            let tasks = b.recent_tasks(&pid("alpha"), 10).unwrap();
            assert_eq!(tasks[0].outcome, "failed");
        }

        // ── Agent cards ──

        #[test]
        fn upsert_and_get_card() {
            let b = bb();
            b.upsert_card(&sample_card("alpha")).unwrap();
            let card = b.get_card(&pid("alpha")).unwrap();
            assert!(card.is_some());
            let card = card.unwrap();
            assert_eq!(card.capabilities.len(), 2);
            assert_eq!(card.domain, "backend");
        }

        #[test]
        fn upsert_card_overwrites() {
            let b = bb();
            let mut c = sample_card("alpha");
            b.upsert_card(&c).unwrap();
            c.domain = "frontend".into();
            b.upsert_card(&c).unwrap();
            let got = b.get_card(&pid("alpha")).unwrap().unwrap();
            assert_eq!(got.domain, "frontend");
            assert_eq!(b.card_count().unwrap(), 1);
        }

        #[test]
        fn get_card_not_found() {
            let b = bb();
            assert!(b.get_card(&pid("nope")).unwrap().is_none());
        }

        #[test]
        fn list_cards_empty() {
            let b = bb();
            assert!(b.list_cards().unwrap().is_empty());
        }

        #[test]
        fn list_cards_multiple() {
            let b = bb();
            b.upsert_card(&sample_card("alpha")).unwrap();
            b.upsert_card(&sample_card("beta")).unwrap();
            let cards = b.list_cards().unwrap();
            assert_eq!(cards.len(), 2);
        }

        #[test]
        fn remove_card_exists() {
            let b = bb();
            b.upsert_card(&sample_card("alpha")).unwrap();
            assert!(b.remove_card(&pid("alpha")).unwrap());
            assert_eq!(b.card_count().unwrap(), 0);
        }

        #[test]
        fn remove_card_not_found() {
            let b = bb();
            assert!(!b.remove_card(&pid("nope")).unwrap());
        }

        #[test]
        fn card_count() {
            let b = bb();
            assert_eq!(b.card_count().unwrap(), 0);
            b.upsert_card(&sample_card("a")).unwrap();
            assert_eq!(b.card_count().unwrap(), 1);
        }

        // ── Cross-table ──

        #[test]
        fn full_lifecycle() {
            let b = bb();

            // Register pane
            b.upsert_pane(&sample_pane("alpha", PaneStatus::Idle)).unwrap();
            b.upsert_card(&sample_card("alpha")).unwrap();

            // Execute task
            let mut pane = b.get_pane(&pid("alpha")).unwrap().unwrap();
            pane.status = PaneStatus::Working;
            b.upsert_pane(&pane).unwrap();

            // Complete task
            b.insert_task(&sample_task("t1", "alpha")).unwrap();
            pane.status = PaneStatus::Idle;
            pane.tasks_completed = 1;
            b.upsert_pane(&pane).unwrap();

            // Verify
            let final_pane = b.get_pane(&pid("alpha")).unwrap().unwrap();
            assert_eq!(final_pane.status, PaneStatus::Idle);
            assert_eq!(final_pane.tasks_completed, 1);
            assert_eq!(b.task_count().unwrap(), 1);
            assert_eq!(b.card_count().unwrap(), 1);
        }

        // ── Deregistration ──

        #[test]
        fn deregister_removes_pane_and_card() {
            let b = bb();
            b.upsert_pane(&sample_pane("alpha", PaneStatus::Idle)).unwrap();
            b.upsert_card(&sample_card("alpha")).unwrap();
            b.remove_pane(&pid("alpha")).unwrap();
            b.remove_card(&pid("alpha")).unwrap();
            assert_eq!(b.pane_count().unwrap(), 0);
            assert_eq!(b.card_count().unwrap(), 0);
        }

        // ── Phase storage ──

        #[test]
        fn pane_phase_stored_correctly() {
            let b = bb();
            let mut rec = sample_pane("alpha", PaneStatus::Idle);
            rec.phase = 3.14159;
            b.upsert_pane(&rec).unwrap();
            let got = b.get_pane(&pid("alpha")).unwrap().unwrap();
            assert!((got.phase - 3.14159).abs() < 1e-5);
        }

        // ── Duration storage ──

        #[test]
        fn task_duration_stored_correctly() {
            let b = bb();
            let mut t = sample_task("t1", "alpha");
            t.duration_secs = 42.5;
            b.insert_task(&t).unwrap();
            let tasks = b.recent_tasks(&pid("alpha"), 10).unwrap();
            assert!((tasks[0].duration_secs - 42.5).abs() < 1e-5);
        }

        // ── Ghost traces ──

        fn sample_ghost(id: &str, tools: u64) -> GhostRecord {
            GhostRecord {
                sphere_id: id.into(),
                persona: format!("agent-{id}"),
                deregistered_ms: 1_000_000,
                final_phase: 1.57,
                total_tools: tools,
                session_duration_ms: 60_000,
            }
        }

        #[test]
        fn insert_and_get_ghost() {
            let b = bb();
            b.insert_ghost(&sample_ghost("alpha", 42)).unwrap();
            let ghosts = b.recent_ghosts(10).unwrap();
            assert_eq!(ghosts.len(), 1);
            assert_eq!(ghosts[0].sphere_id, "alpha");
            assert_eq!(ghosts[0].total_tools, 42);
        }

        #[test]
        fn recent_ghosts_ordered_desc() {
            let b = bb();
            let mut g1 = sample_ghost("first", 1);
            g1.deregistered_ms = 1000;
            let mut g2 = sample_ghost("second", 2);
            g2.deregistered_ms = 2000;
            b.insert_ghost(&g1).unwrap();
            b.insert_ghost(&g2).unwrap();
            let ghosts = b.recent_ghosts(10).unwrap();
            assert_eq!(ghosts[0].sphere_id, "second");
            assert_eq!(ghosts[1].sphere_id, "first");
        }

        #[test]
        fn recent_ghosts_limited() {
            let b = bb();
            for i in 0..10 {
                let mut g = sample_ghost(&format!("g{i}"), i);
                g.deregistered_ms = u64::from(i) * 1000;
                b.insert_ghost(&g).unwrap();
            }
            let ghosts = b.recent_ghosts(3).unwrap();
            assert_eq!(ghosts.len(), 3);
        }

        #[test]
        fn ghost_count() {
            let b = bb();
            assert_eq!(b.ghost_count().unwrap(), 0);
            b.insert_ghost(&sample_ghost("a", 1)).unwrap();
            b.insert_ghost(&sample_ghost("b", 2)).unwrap();
            assert_eq!(b.ghost_count().unwrap(), 2);
        }

        #[test]
        fn ghost_fields_preserved() {
            let b = bb();
            let g = GhostRecord {
                sphere_id: "test".into(),
                persona: "test-persona".into(),
                deregistered_ms: 9_999_999,
                final_phase: 3.14,
                total_tools: 87,
                session_duration_ms: 120_000,
            };
            b.insert_ghost(&g).unwrap();
            let ghosts = b.recent_ghosts(1).unwrap();
            assert_eq!(ghosts[0].persona, "test-persona");
            assert_eq!(ghosts[0].deregistered_ms, 9_999_999);
            assert!((ghosts[0].final_phase - 3.14).abs() < 1e-5);
            assert_eq!(ghosts[0].session_duration_ms, 120_000);
        }

        #[test]
        fn prune_ghosts_keeps_newest() {
            let b = bb();
            for i in 0..10 {
                let mut g = sample_ghost(&format!("g{i}"), i);
                g.deregistered_ms = u64::from(i) * 1000;
                b.insert_ghost(&g).unwrap();
            }
            let deleted = b.prune_ghosts(3).unwrap();
            assert_eq!(deleted, 7);
            assert_eq!(b.ghost_count().unwrap(), 3);
            let ghosts = b.recent_ghosts(10).unwrap();
            assert_eq!(ghosts[0].sphere_id, "g9");
        }

        #[test]
        fn prune_ghosts_no_op_when_under_limit() {
            let b = bb();
            b.insert_ghost(&sample_ghost("a", 1)).unwrap();
            let deleted = b.prune_ghosts(10).unwrap();
            assert_eq!(deleted, 0);
            assert_eq!(b.ghost_count().unwrap(), 1);
        }

        #[test]
        fn ghost_duplicate_sphere_ids_allowed() {
            let b = bb();
            b.insert_ghost(&sample_ghost("alpha", 1)).unwrap();
            b.insert_ghost(&sample_ghost("alpha", 2)).unwrap();
            assert_eq!(b.ghost_count().unwrap(), 2);
        }

        // ── Pruning ──

        #[test]
        fn prune_stale_panes_removes_old() {
            let b = bb();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("old"),
                status: PaneStatus::Complete,
                persona: "old-agent".into(),
                updated_at: 1000.0, // ancient
                phase: 0.0,
                tasks_completed: 5,
            })
            .unwrap();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("fresh"),
                status: PaneStatus::Working,
                persona: "fresh-agent".into(),
                updated_at: 9000.0, // recent
                phase: 0.5,
                tasks_completed: 1,
            })
            .unwrap();
            assert_eq!(b.pane_count().unwrap(), 2);

            let deleted = b.prune_stale_panes(5000.0).unwrap();
            assert_eq!(deleted, 1);
            assert_eq!(b.pane_count().unwrap(), 1);
            assert!(b.get_pane(&pid("old")).unwrap().is_none());
            assert!(b.get_pane(&pid("fresh")).unwrap().is_some());
        }

        #[test]
        fn prune_stale_panes_also_removes_cards() {
            let b = bb();
            b.upsert_pane(&sample_pane("old", PaneStatus::Idle)).unwrap();
            b.upsert_card(&sample_card("old")).unwrap();
            assert_eq!(b.card_count().unwrap(), 1);

            // Pane updated_at is 1000.0, cutoff at 5000.0 removes it
            b.prune_stale_panes(5000.0).unwrap();
            assert_eq!(b.pane_count().unwrap(), 0);
            assert_eq!(b.card_count().unwrap(), 0);
        }

        #[test]
        fn prune_stale_panes_no_op_when_all_fresh() {
            let b = bb();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("fresh"),
                status: PaneStatus::Working,
                persona: "agent".into(),
                updated_at: 9000.0,
                phase: 0.0,
                tasks_completed: 0,
            })
            .unwrap();
            let deleted = b.prune_stale_panes(5000.0).unwrap();
            assert_eq!(deleted, 0);
            assert_eq!(b.pane_count().unwrap(), 1);
        }

        #[test]
        fn prune_old_tasks_removes_old() {
            let b = bb();
            let mut old_task = sample_task("t-old", "alpha");
            old_task.finished_at = 1000.0;
            b.insert_task(&old_task).unwrap();

            let mut fresh_task = sample_task("t-fresh", "alpha");
            fresh_task.finished_at = 9000.0;
            b.insert_task(&fresh_task).unwrap();

            assert_eq!(b.task_count().unwrap(), 2);
            let deleted = b.prune_old_tasks(5000.0).unwrap();
            assert_eq!(deleted, 1);
            assert_eq!(b.task_count().unwrap(), 1);
        }

        #[test]
        fn prune_old_tasks_no_op_when_all_fresh() {
            let b = bb();
            let mut t = sample_task("t1", "alpha");
            t.finished_at = 9000.0;
            b.insert_task(&t).unwrap();
            let deleted = b.prune_old_tasks(5000.0).unwrap();
            assert_eq!(deleted, 0);
        }

        // ── prune_complete_panes ──

        #[test]
        fn prune_complete_panes_removes_old_complete() {
            let b = bb();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("done-old"),
                status: PaneStatus::Complete,
                persona: "agent".into(),
                updated_at: 1000.0,
                phase: 0.0,
                tasks_completed: 3,
            }).unwrap();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("done-fresh"),
                status: PaneStatus::Complete,
                persona: "agent".into(),
                updated_at: 9000.0,
                phase: 0.0,
                tasks_completed: 1,
            }).unwrap();
            let deleted = b.prune_complete_panes(5000.0).unwrap();
            assert_eq!(deleted, 1);
            assert!(b.get_pane(&pid("done-old")).unwrap().is_none());
            assert!(b.get_pane(&pid("done-fresh")).unwrap().is_some());
        }

        #[test]
        fn prune_complete_panes_preserves_working() {
            let b = bb();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("working-old"),
                status: PaneStatus::Working,
                persona: "agent".into(),
                updated_at: 1000.0,
                phase: 0.0,
                tasks_completed: 0,
            }).unwrap();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("idle-old"),
                status: PaneStatus::Idle,
                persona: "agent".into(),
                updated_at: 1000.0,
                phase: 0.0,
                tasks_completed: 0,
            }).unwrap();
            let deleted = b.prune_complete_panes(5000.0).unwrap();
            assert_eq!(deleted, 0);
            assert_eq!(b.pane_count().unwrap(), 2);
        }

        #[test]
        fn prune_complete_panes_also_removes_cards() {
            let b = bb();
            b.upsert_pane(&PaneRecord {
                pane_id: pid("done"),
                status: PaneStatus::Complete,
                persona: "agent".into(),
                updated_at: 1000.0,
                phase: 0.0,
                tasks_completed: 0,
            }).unwrap();
            b.upsert_card(&sample_card("done")).unwrap();
            assert_eq!(b.card_count().unwrap(), 1);
            b.prune_complete_panes(5000.0).unwrap();
            assert_eq!(b.pane_count().unwrap(), 0);
            assert_eq!(b.card_count().unwrap(), 0);
        }

        #[test]
        fn prune_complete_panes_no_op_when_none_complete() {
            let b = bb();
            b.upsert_pane(&sample_pane("alpha", PaneStatus::Working)).unwrap();
            b.upsert_pane(&sample_pane("beta", PaneStatus::Idle)).unwrap();
            let deleted = b.prune_complete_panes(5000.0).unwrap();
            assert_eq!(deleted, 0);
        }

        // ── Persistence across restart (file-backed) ──

        #[test]
        fn pane_survives_reopen() {
            let dir = std::env::temp_dir().join(format!("orac-bb-pane-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let p = dir.join("test.db");
            let ps = p.to_string_lossy().to_string();
            {
                let b = Blackboard::open(&ps).unwrap();
                b.upsert_pane(&sample_pane("alpha", PaneStatus::Working)).unwrap();
                b.upsert_pane(&sample_pane("beta", PaneStatus::Idle)).unwrap();
            }
            {
                let b = Blackboard::open(&ps).unwrap();
                assert_eq!(b.pane_count().unwrap(), 2);
                assert_eq!(b.get_pane(&pid("alpha")).unwrap().unwrap().status, PaneStatus::Working);
                assert_eq!(b.get_pane(&pid("beta")).unwrap().unwrap().status, PaneStatus::Idle);
            }
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn tasks_survive_reopen() {
            let dir = std::env::temp_dir().join(format!("orac-bb-tasks-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let p = dir.join("test.db");
            let ps = p.to_string_lossy().to_string();
            {
                let b = Blackboard::open(&ps).unwrap();
                let mut t1 = sample_task("t1", "alpha");
                t1.finished_at = 1000.0;
                let mut t2 = sample_task("t2", "alpha");
                t2.finished_at = 2000.0;
                b.insert_task(&t1).unwrap();
                b.insert_task(&t2).unwrap();
            }
            {
                let b = Blackboard::open(&ps).unwrap();
                assert_eq!(b.task_count().unwrap(), 2);
                let tasks = b.recent_tasks(&pid("alpha"), 10).unwrap();
                assert_eq!(tasks[0].task_id, "t2");
                assert_eq!(tasks[1].task_id, "t1");
            }
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn cards_survive_reopen() {
            let dir = std::env::temp_dir().join(format!("orac-bb-cards-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let p = dir.join("test.db");
            let ps = p.to_string_lossy().to_string();
            {
                let b = Blackboard::open(&ps).unwrap();
                b.upsert_card(&sample_card("alpha")).unwrap();
            }
            {
                let b = Blackboard::open(&ps).unwrap();
                assert_eq!(b.card_count().unwrap(), 1);
                let c = b.get_card(&pid("alpha")).unwrap().unwrap();
                assert_eq!(c.domain, "backend");
                assert_eq!(c.model, "opus-4.6");
            }
            let _ = std::fs::remove_dir_all(&dir);
        }

        #[test]
        fn full_lifecycle_survives_reopen() {
            let dir = std::env::temp_dir().join(format!("orac-bb-full-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&dir);
            let p = dir.join("test.db");
            let ps = p.to_string_lossy().to_string();
            {
                let b = Blackboard::open(&ps).unwrap();
                b.upsert_pane(&sample_pane("alpha", PaneStatus::Idle)).unwrap();
                b.upsert_card(&sample_card("alpha")).unwrap();
                let mut pane = b.get_pane(&pid("alpha")).unwrap().unwrap();
                pane.status = PaneStatus::Working;
                b.upsert_pane(&pane).unwrap();
                b.insert_task(&sample_task("t1", "alpha")).unwrap();
                pane.tasks_completed = 1;
                pane.status = PaneStatus::Idle;
                b.upsert_pane(&pane).unwrap();
            }
            {
                let b = Blackboard::open(&ps).unwrap();
                let pane = b.get_pane(&pid("alpha")).unwrap().unwrap();
                assert_eq!(pane.status, PaneStatus::Idle);
                assert_eq!(pane.tasks_completed, 1);
                assert_eq!(b.task_count().unwrap(), 1);
                assert_eq!(b.card_count().unwrap(), 1);
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }

    // ── Consent declarations ──

    mod consent_tests {
        use super::*;

        fn bb() -> Blackboard {
            Blackboard::in_memory().unwrap()
        }

        fn sample_consent(id: &str) -> ConsentRecord {
            ConsentRecord {
                sphere_id: id.into(),
                synthex_write: true,
                povm_read: true,
                povm_write: false,
                hydration: true,
                updated_ms: 1_742_600_000_000,
            }
        }

        #[test]
        fn upsert_and_get_consent() {
            let b = bb();
            b.upsert_consent(&sample_consent("sphere-a")).unwrap();
            let c = b.get_consent_record("sphere-a").unwrap();
            assert!(c.is_some());
            let c = c.unwrap();
            assert_eq!(c.sphere_id, "sphere-a");
            assert!(c.synthex_write);
            assert!(c.povm_read);
            assert!(!c.povm_write);
            assert!(c.hydration);
        }

        #[test]
        fn get_consent_missing() {
            let b = bb();
            assert!(b.get_consent_record("nonexistent").unwrap().is_none());
        }

        #[test]
        fn upsert_consent_overwrites() {
            let b = bb();
            b.upsert_consent(&sample_consent("sphere-a")).unwrap();
            let mut updated = sample_consent("sphere-a");
            updated.synthex_write = false;
            updated.povm_write = true;
            updated.updated_ms = 1_742_700_000_000;
            b.upsert_consent(&updated).unwrap();

            let c = b.get_consent_record("sphere-a").unwrap().unwrap();
            assert!(!c.synthex_write);
            assert!(c.povm_write);
            assert_eq!(c.updated_ms, 1_742_700_000_000);
        }

        #[test]
        fn list_consents_empty() {
            let b = bb();
            assert!(b.list_consents().unwrap().is_empty());
        }

        #[test]
        fn list_consents_multiple() {
            let b = bb();
            b.upsert_consent(&sample_consent("sphere-a")).unwrap();
            let mut second = sample_consent("sphere-b");
            second.updated_ms = 1_742_700_000_000;
            b.upsert_consent(&second).unwrap();
            let all = b.list_consents().unwrap();
            assert_eq!(all.len(), 2);
            // Ordered by updated_ms DESC
            assert_eq!(all[0].sphere_id, "sphere-b");
        }

        #[test]
        fn insert_and_query_audit() {
            let b = bb();
            let entry = ConsentAuditEntry {
                sphere_id: "sphere-a".into(),
                field_name: "synthex_write".into(),
                old_value: true,
                new_value: false,
                changed_ms: 1_742_600_000_000,
            };
            b.insert_consent_audit(&entry).unwrap();
            let audit = b.recent_consent_audit("sphere-a", 10).unwrap();
            assert_eq!(audit.len(), 1);
            assert_eq!(audit[0].field_name, "synthex_write");
            assert!(audit[0].old_value);
            assert!(!audit[0].new_value);
        }

        #[test]
        fn audit_empty_for_unknown_sphere() {
            let b = bb();
            assert!(b.recent_consent_audit("nope", 10).unwrap().is_empty());
        }

        #[test]
        fn audit_respects_limit() {
            let b = bb();
            for i in 0..5 {
                b.insert_consent_audit(&ConsentAuditEntry {
                    sphere_id: "sphere-a".into(),
                    field_name: format!("field_{i}"),
                    old_value: false,
                    new_value: true,
                    changed_ms: 1_742_600_000_000 + i,
                })
                .unwrap();
            }
            let audit = b.recent_consent_audit("sphere-a", 3).unwrap();
            assert_eq!(audit.len(), 3);
        }

        #[test]
        fn audit_ordered_by_time_desc() {
            let b = bb();
            b.insert_consent_audit(&ConsentAuditEntry {
                sphere_id: "sphere-a".into(),
                field_name: "first".into(),
                old_value: false,
                new_value: true,
                changed_ms: 100,
            })
            .unwrap();
            b.insert_consent_audit(&ConsentAuditEntry {
                sphere_id: "sphere-a".into(),
                field_name: "second".into(),
                old_value: true,
                new_value: false,
                changed_ms: 200,
            })
            .unwrap();
            let audit = b.recent_consent_audit("sphere-a", 10).unwrap();
            assert_eq!(audit[0].field_name, "second");
            assert_eq!(audit[1].field_name, "first");
        }

        #[test]
        fn audit_per_sphere_isolation() {
            let b = bb();
            b.insert_consent_audit(&ConsentAuditEntry {
                sphere_id: "sphere-a".into(),
                field_name: "hydration".into(),
                old_value: true,
                new_value: false,
                changed_ms: 100,
            })
            .unwrap();
            b.insert_consent_audit(&ConsentAuditEntry {
                sphere_id: "sphere-b".into(),
                field_name: "povm_write".into(),
                old_value: false,
                new_value: true,
                changed_ms: 200,
            })
            .unwrap();
            assert_eq!(b.recent_consent_audit("sphere-a", 10).unwrap().len(), 1);
            assert_eq!(b.recent_consent_audit("sphere-b", 10).unwrap().len(), 1);
        }
    }
}
