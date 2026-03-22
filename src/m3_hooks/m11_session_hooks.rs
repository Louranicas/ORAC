//! # M11: Session Hooks
//!
//! Handlers for `SessionStart` and `Stop` hook events from Claude Code.
//!
//! - **`SessionStart`**: Registers sphere on PV2, hydrates from POVM + RM,
//!   returns hydration context as `systemMessage`.
//! - **`Stop`**: Fails active tasks, crystallizes to POVM + RM, deregisters
//!   sphere, cleans up session tracking.
//!
//! ## Layer: L3 (Hooks) | Module: M11
//! ## Dependencies: `m10_hook_server` (`OracState`, `HookEvent`, `HookResponse`)

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use super::m10_hook_server::{
    fire_and_forget_post, generate_pane_id, http_get, HookEvent, HookResponse, OracState,
};

// ──────────────────────────────────────────────────────────────
// SessionStart handler
// ──────────────────────────────────────────────────────────────

/// Handle `SessionStart` hook from Claude Code.
///
/// 1. Generates a unique pane ID for this session
/// 2. Registers sphere on PV2 daemon
/// 3. Hydrates from POVM (memories, pathways) and RM (discoveries)
/// 4. Tracks the session in `OracState`
/// 5. Returns hydration context as `systemMessage`
pub async fn handle_session_start(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse> {
    let session_id = event
        .session_id
        .unwrap_or_else(|| format!("sess-{}", uuid::Uuid::new_v4()));

    let pane_id = generate_pane_id();
    let pane_id_str = pane_id.as_str().to_owned();

    // Register sphere on PV2 (fire-and-forget)
    let register_url = format!("{}/sphere/{}/register", state.pv2_url, pane_id_str);
    let register_body = serde_json::json!({
        "persona": "orac-agent",
        "frequency": 0.1
    })
    .to_string();
    fire_and_forget_post(register_url, register_body);

    // Hydrate from POVM + RM (parallel)
    let povm_url = format!("{}/hydrate", state.povm_url);
    let rm_url = format!("{}/search?q=discovery", state.rm_url);

    let (povm_data, rm_data) = tokio::join!(
        http_get(&povm_url, 2000),
        http_get(&rm_url, 2000),
    );

    let (povm_memories, povm_pathways) = parse_povm_hydration(povm_data.as_deref());
    let rm_count = parse_rm_count(rm_data.as_deref());

    // Track session
    state.register_session(session_id, pane_id.clone());

    // Register in coupling network for semantic routing
    {
        let mut network = state.coupling.write();
        network.phases.insert(pane_id.clone(), 0.0);
        network.frequencies.insert(pane_id, 0.1);
    }

    let message = format!(
        "[HABITAT] Hydrated: POVM {povm_memories} memories, {povm_pathways} pathways | RM {rm_count} discoveries",
    );

    Json(HookResponse::with_message(message))
}

// ──────────────────────────────────────────────────────────────
// Stop handler
// ──────────────────────────────────────────────────────────────

/// Handle `Stop` hook from Claude Code.
///
/// 1. Fails any active claimed task on the bus
/// 2. Marks sphere status as "complete"
/// 3. Crystallizes session summary to POVM + RM
/// 4. Deregisters sphere (creates ghost trace)
/// 5. Removes session from tracking
pub async fn handle_stop(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse> {
    let session_id = event.session_id.unwrap_or_default();

    let tracker = state.remove_session(&session_id);
    let pane_id_str = tracker.as_ref().map_or_else(
        || generate_pane_id().as_str().to_owned(),
        |t| t.pane_id.as_str().to_owned(),
    );

    if let Some(ref t) = tracker {
        if let Some(ref task_id) = t.active_task_id {
            let fail_url = format!("{}/bus/fail/{}", state.pv2_url, task_id);
            fire_and_forget_post(fail_url, "{}".into());
        }
    }

    let status_url = format!("{}/sphere/{}/status", state.pv2_url, pane_id_str);
    let status_body = serde_json::json!({"status": "complete"}).to_string();
    fire_and_forget_post(status_url, status_body);

    let r_value = fetch_current_r(&state.pv2_url).await;
    let povm_url = format!("{}/snapshots", state.povm_url);
    let povm_body = serde_json::json!({
        "sphere_id": pane_id_str,
        "r": r_value,
        "event": "session_end"
    })
    .to_string();
    fire_and_forget_post(povm_url, povm_body);

    let rm_put_url = format!("{}/put", state.rm_url);
    let rm_tsv = format!(
        "session\t{pane_id_str}\tsession-end\t3600\tsession-end r={r_value}"
    );
    tokio::spawn(async move {
        let _ = tokio::task::spawn_blocking(move || {
            let _ = ureq::post(&rm_put_url)
                .timeout(std::time::Duration::from_millis(2000))
                .send_string(&rm_tsv);
        })
        .await;
    });

    // 5b. Update blackboard: record failed task (if any), mark Complete
    #[cfg(feature = "persistence")]
    if let Some(bb) = state.blackboard() {
        use crate::m1_core::m01_core_types::PaneStatus;

        #[allow(clippy::cast_precision_loss)]
        let now = super::m10_hook_server::epoch_ms() as f64 / 1000.0;
        let pid = crate::m1_core::m01_core_types::PaneId::new(&pane_id_str);

        if let Some(ref t) = tracker {
            if let Some(ref task_id) = t.active_task_id {
                use crate::m5_bridges::m26_blackboard::TaskRecord;
                let _ = bb.insert_task(&TaskRecord {
                    task_id: task_id.clone(),
                    pane_id: pid.clone(),
                    description: "session terminated".into(),
                    outcome: "failed".into(),
                    finished_at: now,
                    duration_secs: 0.0,
                });
            }
        }

        if let Ok(Some(mut pane)) = bb.get_pane(&pid) {
            pane.status = PaneStatus::Complete;
            pane.updated_at = now;
            let _ = bb.upsert_pane(&pane);
        }
    }

    // 6. Deregister sphere (creates ghost trace)
    let dereg_url = format!("{}/sphere/{}/deregister", state.pv2_url, pane_id_str);
    fire_and_forget_post(dereg_url, String::new());

    Json(HookResponse::empty())
}

fn parse_povm_hydration(data: Option<&str>) -> (u64, u64) {
    let Some(data) = data else { return (0, 0) };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return (0, 0),
    };
    let memories = parsed.get("memory_count").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let pathways = parsed.get("pathway_count").and_then(serde_json::Value::as_u64).unwrap_or(0);
    (memories, pathways)
}

fn parse_rm_count(data: Option<&str>) -> usize {
    let Some(data) = data else { return 0 };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    parsed.as_array().map_or(0, Vec::len)
}

async fn fetch_current_r(pv2_url: &str) -> f64 {
    let url = format!("{pv2_url}/health");
    let data = http_get(&url, 1000).await;
    data.and_then(|s| {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        v.get("r").and_then(serde_json::Value::as_f64)
    })
    .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn parse_povm_none() { assert_eq!(parse_povm_hydration(None), (0, 0)); }
    #[test] fn parse_povm_empty() { assert_eq!(parse_povm_hydration(Some("{}")), (0, 0)); }
    #[test] fn parse_povm_valid() { assert_eq!(parse_povm_hydration(Some(r#"{"memory_count":42,"pathway_count":100}"#)), (42, 100)); }
    #[test] fn parse_povm_invalid() { assert_eq!(parse_povm_hydration(Some("bad")), (0, 0)); }
    #[test] fn parse_povm_partial() { assert_eq!(parse_povm_hydration(Some(r#"{"memory_count":5}"#)), (5, 0)); }
    #[test] fn parse_povm_null() { assert_eq!(parse_povm_hydration(Some(r#"{"memory_count":null}"#)), (0, 0)); }
    #[test] fn parse_povm_string() { assert_eq!(parse_povm_hydration(Some(r#"{"memory_count":"5"}"#)), (0, 0)); }
    #[test] fn parse_povm_large() { assert_eq!(parse_povm_hydration(Some(r#"{"memory_count":999999,"pathway_count":888888}"#)), (999_999, 888_888)); }
    #[test] fn parse_povm_zero() { assert_eq!(parse_povm_hydration(Some(r#"{"memory_count":0,"pathway_count":0}"#)), (0, 0)); }

    #[test] fn parse_rm_none() { assert_eq!(parse_rm_count(None), 0); }
    #[test] fn parse_rm_empty() { assert_eq!(parse_rm_count(Some("[]")), 0); }
    #[test] fn parse_rm_valid() { assert_eq!(parse_rm_count(Some(r#"[{"e":1},{"e":2},{"e":3}]"#)), 3); }
    #[test] fn parse_rm_not_array() { assert_eq!(parse_rm_count(Some(r#"{"count":5}"#)), 0); }
    #[test] fn parse_rm_invalid() { assert_eq!(parse_rm_count(Some("bad")), 0); }
    #[test] fn parse_rm_single() { assert_eq!(parse_rm_count(Some("[1]")), 1); }
    #[test] fn parse_rm_large() { let d = serde_json::to_string(&(0..100).map(|i| serde_json::json!({"id":i})).collect::<Vec<_>>()).unwrap(); assert_eq!(parse_rm_count(Some(&d)), 100); }

    #[test]
    fn session_start_message_format() {
        let msg = format!("[HABITAT] Hydrated: POVM {} memories, {} pathways | RM {} discoveries", 42, 100, 5);
        assert!(msg.contains("POVM 42 memories"));
        assert!(msg.contains("100 pathways"));
        assert!(msg.contains("RM 5 discoveries"));
    }

    #[test]
    fn stop_rm_tsv_format() {
        let tsv = format!("session\ttest-pane\tsession-end\t3600\tsession-end r=0.995");
        assert_eq!(tsv.matches('\t').count(), 4);
        assert!(tsv.contains("test-pane"));
    }

    #[test]
    fn session_id_generation_fallback() {
        let event = HookEvent::default();
        let sid = event.session_id.unwrap_or_else(|| format!("sess-{}", uuid::Uuid::new_v4()));
        assert!(sid.starts_with("sess-"));
    }

    #[test] fn stop_no_tracker() { let s = OracState::new(crate::m1_core::m03_config::PvConfig::default()); assert!(s.remove_session("x").is_none()); }

    #[test]
    fn stop_tracker_no_task() {
        let s = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        s.register_session("s".into(), crate::m1_core::m01_core_types::PaneId::new("p"));
        let t = s.remove_session("s");
        assert!(t.is_some());
        assert!(t.unwrap().active_task_id.is_none());
    }

    #[test]
    fn stop_tracker_with_task() {
        let s = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        s.register_session("s".into(), crate::m1_core::m01_core_types::PaneId::new("p"));
        { let mut ss = s.sessions.write(); if let Some(t) = ss.get_mut("s") { t.active_task_id = Some("task-42".into()); } }
        let t = s.remove_session("s");
        assert_eq!(t.unwrap().active_task_id.as_deref(), Some("task-42"));
    }

    #[test] fn pane_id_has_colon() { assert!(generate_pane_id().as_str().contains(':')); }
    #[test] fn hook_response_empty() { assert_eq!(serde_json::to_string(&HookResponse::empty()).unwrap(), "{}"); }
}
