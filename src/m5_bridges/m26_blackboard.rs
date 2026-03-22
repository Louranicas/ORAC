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

                CREATE INDEX IF NOT EXISTS idx_task_history_pane
                    ON task_history(pane_id);
                CREATE INDEX IF NOT EXISTS idx_task_history_finished
                    ON task_history(finished_at);
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

    // ── SQLite tests (feature-gated) ──

    #[cfg(feature = "persistence")]
    mod sqlite_tests {
        use super::*;

        fn bb() -> Blackboard {
            Blackboard::in_memory().unwrap()
        }

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

        fn sample_card(id: &str) -> AgentCard {
            AgentCard {
                pane_id: pid(id),
                capabilities: vec!["rust".into(), "testing".into()],
                domain: "backend".into(),
                model: "opus-4.6".into(),
                registered_at: 3000.0,
            }
        }

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
                #[allow(clippy::cast_precision_loss)]
                { t.finished_at = i as f64; }
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
    }
}
