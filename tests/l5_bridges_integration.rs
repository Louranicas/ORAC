//! L5 Bridges integration tests — blackboard lifecycle with in-memory SQLite.
//!
//! Exercises the full SessionStart → PostToolUse → Stop blackboard flow
//! using `Blackboard::in_memory()` — no disk I/O, fast, deterministic.

mod common;

#[cfg(feature = "persistence")]
mod blackboard_lifecycle {
    use orac_sidecar::m1_core::m01_core_types::{PaneId, PaneStatus};
    use orac_sidecar::m5_bridges::m26_blackboard::{
        AgentCard, Blackboard, ConsentAuditEntry, GhostRecord, PaneRecord, TaskRecord,
    };

    fn bb() -> Blackboard {
        Blackboard::in_memory().unwrap()
    }

    fn pid(s: &str) -> PaneId {
        PaneId::new(s)
    }

    // ── Full lifecycle: register → work → complete task → stop ──

    #[test]
    fn full_session_lifecycle() {
        let b = bb();
        let pane = pid("alpha-left");

        // 1. SessionStart: register pane + agent card
        b.upsert_pane(&PaneRecord {
            pane_id: pane.clone(),
            status: PaneStatus::Idle,
            persona: "orac-agent".into(),
            updated_at: 1000.0,
            phase: 0.0,
            tasks_completed: 0,
        })
        .unwrap();

        b.upsert_card(&AgentCard {
            pane_id: pane.clone(),
            capabilities: vec!["read".into(), "write".into()],
            domain: "general".into(),
            model: "opus-4.6".into(),
            registered_at: 1000.0,
        })
        .unwrap();

        assert_eq!(b.pane_count().unwrap(), 1);
        assert_eq!(b.card_count().unwrap(), 1);

        // 2. PostToolUse: status → Working
        let mut rec = b.get_pane(&pane).unwrap().unwrap();
        assert_eq!(rec.status, PaneStatus::Idle);
        rec.status = PaneStatus::Working;
        rec.updated_at = 2000.0;
        b.upsert_pane(&rec).unwrap();

        let working = b.get_pane(&pane).unwrap().unwrap();
        assert_eq!(working.status, PaneStatus::Working);
        assert_eq!(working.persona, "orac-agent"); // preserved

        // 3. Task claimed → insert task history
        b.insert_task(&TaskRecord {
            task_id: "task-001".into(),
            pane_id: pane.clone(),
            description: "Fix authentication bug".into(),
            outcome: "claimed".into(),
            finished_at: 2500.0,
            duration_secs: 0.0,
        })
        .unwrap();
        assert_eq!(b.task_count().unwrap(), 1);

        // 4. TASK_COMPLETE → update task + increment counter
        b.insert_task(&TaskRecord {
            task_id: "task-001".into(),
            pane_id: pane.clone(),
            description: "Fix authentication bug".into(),
            outcome: "completed".into(),
            finished_at: 3000.0,
            duration_secs: 500.0,
        })
        .unwrap();

        let mut done = b.get_pane(&pane).unwrap().unwrap();
        done.tasks_completed += 1;
        done.status = PaneStatus::Idle;
        done.updated_at = 3000.0;
        b.upsert_pane(&done).unwrap();

        let final_pane = b.get_pane(&pane).unwrap().unwrap();
        assert_eq!(final_pane.tasks_completed, 1);
        assert_eq!(final_pane.status, PaneStatus::Idle);

        // 5. Stop: mark Complete + record ghost
        let mut stop = b.get_pane(&pane).unwrap().unwrap();
        stop.status = PaneStatus::Complete;
        stop.updated_at = 4000.0;
        b.upsert_pane(&stop).unwrap();

        b.insert_ghost(&GhostRecord {
            sphere_id: pane.as_str().into(),
            persona: "orac-agent".into(),
            deregistered_ms: 4_000_000,
            final_phase: 1.57,
            total_tools: 42,
            session_duration_ms: 3_000_000,
        })
        .unwrap();

        // Verify final state
        let completed = b.get_pane(&pane).unwrap().unwrap();
        assert_eq!(completed.status, PaneStatus::Complete);
        assert_eq!(b.ghost_count().unwrap(), 1);
        assert_eq!(b.task_count().unwrap(), 1); // task-001 replaced (PK)
        let tasks = b.recent_tasks(&pane, 10).unwrap();
        assert_eq!(tasks[0].outcome, "completed");
        assert!((tasks[0].duration_secs - 500.0).abs() < f64::EPSILON);
    }

    // ── Multi-pane isolation ──

    #[test]
    fn multi_pane_independent_state() {
        let b = bb();
        let alpha = pid("alpha");
        let beta = pid("beta");

        b.upsert_pane(&PaneRecord {
            pane_id: alpha.clone(),
            status: PaneStatus::Working,
            persona: "alpha-agent".into(),
            updated_at: 1000.0,
            phase: 0.5,
            tasks_completed: 3,
        })
        .unwrap();

        b.upsert_pane(&PaneRecord {
            pane_id: beta.clone(),
            status: PaneStatus::Idle,
            persona: "beta-agent".into(),
            updated_at: 1000.0,
            phase: 1.0,
            tasks_completed: 0,
        })
        .unwrap();

        // Modify alpha without affecting beta
        let mut alpha_rec = b.get_pane(&alpha).unwrap().unwrap();
        alpha_rec.tasks_completed = 10;
        b.upsert_pane(&alpha_rec).unwrap();

        let beta_rec = b.get_pane(&beta).unwrap().unwrap();
        assert_eq!(beta_rec.tasks_completed, 0);
        assert_eq!(beta_rec.status, PaneStatus::Idle);
    }

    // ── Consent audit persistence ──

    #[test]
    fn consent_audit_persists_changes() {
        let b = bb();

        b.insert_consent_audit(&ConsentAuditEntry {
            sphere_id: "sphere-x".into(),
            field_name: "povm_write".into(),
            old_value: false,
            new_value: true,
            changed_ms: 5_000_000,
        })
        .unwrap();

        b.insert_consent_audit(&ConsentAuditEntry {
            sphere_id: "sphere-x".into(),
            field_name: "hydration".into(),
            old_value: true,
            new_value: false,
            changed_ms: 6_000_000,
        })
        .unwrap();

        let audit = b.recent_consent_audit("sphere-x", 10).unwrap();
        assert_eq!(audit.len(), 2);
        // Newest first
        assert_eq!(audit[0].field_name, "hydration");
        assert_eq!(audit[1].field_name, "povm_write");
    }

    // ── Ghost persistence across "restarts" ──

    #[test]
    fn ghosts_survive_across_reads() {
        let b = bb();
        for i in 0..5 {
            b.insert_ghost(&GhostRecord {
                sphere_id: format!("ghost-{i}"),
                persona: "agent".into(),
                deregistered_ms: i as u64 * 1000,
                final_phase: 0.0,
                total_tools: i as u64 * 10,
                session_duration_ms: 60_000,
            })
            .unwrap();
        }

        // Read back — simulates restart + reload
        let ghosts = b.recent_ghosts(20).unwrap();
        assert_eq!(ghosts.len(), 5);
        assert_eq!(ghosts[0].sphere_id, "ghost-4"); // newest first
        assert_eq!(ghosts[4].sphere_id, "ghost-0");
    }

    // ── Task history across panes ──

    #[test]
    fn task_history_cross_pane() {
        let b = bb();

        b.insert_task(&TaskRecord {
            task_id: "t1".into(),
            pane_id: pid("alpha"),
            description: "task for alpha".into(),
            outcome: "completed".into(),
            finished_at: 1000.0,
            duration_secs: 30.0,
        })
        .unwrap();

        b.insert_task(&TaskRecord {
            task_id: "t2".into(),
            pane_id: pid("beta"),
            description: "task for beta".into(),
            outcome: "failed".into(),
            finished_at: 2000.0,
            duration_secs: 15.0,
        })
        .unwrap();

        assert_eq!(b.task_count().unwrap(), 2);
        assert_eq!(b.recent_tasks(&pid("alpha"), 10).unwrap().len(), 1);
        assert_eq!(b.recent_tasks(&pid("beta"), 10).unwrap().len(), 1);
        assert!(b.recent_tasks(&pid("gamma"), 10).unwrap().is_empty());
    }

    // ── Edge: empty blackboard queries ──

    #[test]
    fn empty_blackboard_all_queries() {
        let b = bb();
        assert_eq!(b.pane_count().unwrap(), 0);
        assert_eq!(b.task_count().unwrap(), 0);
        assert_eq!(b.card_count().unwrap(), 0);
        assert_eq!(b.ghost_count().unwrap(), 0);
        assert!(b.recent_consent_audit("any", 10).unwrap().is_empty());
        assert!(b.list_panes().unwrap().is_empty());
        assert!(b.list_cards().unwrap().is_empty());
        assert!(b.recent_ghosts(10).unwrap().is_empty());
        assert!(b.recent_consent_audit("x", 10).unwrap().is_empty());
    }

    // ── Prune ghosts under load ──

    #[test]
    fn prune_ghosts_under_load() {
        let b = bb();
        for i in 0..200 {
            b.insert_ghost(&GhostRecord {
                sphere_id: format!("g{i}"),
                persona: "load-test".into(),
                deregistered_ms: i as u64,
                final_phase: 0.0,
                total_tools: 1,
                session_duration_ms: 1000,
            })
            .unwrap();
        }
        assert_eq!(b.ghost_count().unwrap(), 200);
        let deleted = b.prune_ghosts(100).unwrap();
        assert_eq!(deleted, 100);
        assert_eq!(b.ghost_count().unwrap(), 100);
        // Newest 100 remain
        let ghosts = b.recent_ghosts(1).unwrap();
        assert_eq!(ghosts[0].sphere_id, "g199");
    }
}
