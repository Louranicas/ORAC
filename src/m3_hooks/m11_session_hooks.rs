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

    // Parse hydration data
    let (povm_memories, povm_pathways) = parse_povm_hydration(povm_data.as_deref());
    let rm_count = parse_rm_count(rm_data.as_deref());

    // Track session
    state.register_session(session_id, pane_id);

    // Build system message
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

    // Look up tracked session
    let tracker = state.remove_session(&session_id);
    let pane_id_str = tracker.as_ref().map_or_else(
        || generate_pane_id().as_str().to_owned(),
        |t| t.pane_id.as_str().to_owned(),
    );

    // 1. Fail active task (if any)
    if let Some(ref t) = tracker {
        if let Some(ref task_id) = t.active_task_id {
            let fail_url = format!("{}/bus/fail/{}", state.pv2_url, task_id);
            fire_and_forget_post(fail_url, "{}".into());
        }
    }

    // 2. Mark complete status
    let status_url = format!("{}/sphere/{}/status", state.pv2_url, pane_id_str);
    let status_body = serde_json::json!({"status": "complete"}).to_string();
    fire_and_forget_post(status_url, status_body);

    // 3. Crystallize to POVM
    let r_value = fetch_current_r(&state.pv2_url).await;
    let povm_url = format!("{}/snapshots", state.povm_url);
    let povm_body = serde_json::json!({
        "sphere_id": pane_id_str,
        "r": r_value,
        "event": "session_end"
    })
    .to_string();
    fire_and_forget_post(povm_url, povm_body);

    // 4. Crystallize to RM (TSV format — NOT JSON!)
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

    // 5. Deregister sphere (creates ghost trace)
    let dereg_url = format!("{}/sphere/{}/deregister", state.pv2_url, pane_id_str);
    fire_and_forget_post(dereg_url, String::new());

    Json(HookResponse::empty())
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Parse POVM hydration response for memory and pathway counts.
fn parse_povm_hydration(data: Option<&str>) -> (u64, u64) {
    let Some(data) = data else {
        return (0, 0);
    };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return (0, 0),
    };
    let memories = parsed
        .get("memory_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let pathways = parsed
        .get("pathway_count")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    (memories, pathways)
}

/// Parse RM search response for entry count.
fn parse_rm_count(data: Option<&str>) -> usize {
    let Some(data) = data else {
        return 0;
    };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return 0,
    };
    parsed.as_array().map_or(0, Vec::len)
}

/// Fetch the current order parameter `r` from PV2 health endpoint.
async fn fetch_current_r(pv2_url: &str) -> f64 {
    let url = format!("{pv2_url}/health");
    let data = http_get(&url, 1000).await;
    data.and_then(|s| {
        let v: serde_json::Value = serde_json::from_str(&s).ok()?;
        v.get("r").and_then(serde_json::Value::as_f64)
    })
    .unwrap_or(0.0)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── POVM parsing ──

    #[test]
    fn parse_povm_none() {
        let (m, p) = parse_povm_hydration(None);
        assert_eq!(m, 0);
        assert_eq!(p, 0);
    }

    #[test]
    fn parse_povm_empty_json() {
        let (m, p) = parse_povm_hydration(Some("{}"));
        assert_eq!(m, 0);
        assert_eq!(p, 0);
    }

    #[test]
    fn parse_povm_valid() {
        let data = r#"{"memory_count": 42, "pathway_count": 100}"#;
        let (m, p) = parse_povm_hydration(Some(data));
        assert_eq!(m, 42);
        assert_eq!(p, 100);
    }

    #[test]
    fn parse_povm_invalid_json() {
        let (m, p) = parse_povm_hydration(Some("not json"));
        assert_eq!(m, 0);
        assert_eq!(p, 0);
    }

    #[test]
    fn parse_povm_missing_fields() {
        let data = r#"{"memory_count": 5}"#;
        let (m, p) = parse_povm_hydration(Some(data));
        assert_eq!(m, 5);
        assert_eq!(p, 0);
    }

    #[test]
    fn parse_povm_null_values() {
        let data = r#"{"memory_count": null, "pathway_count": null}"#;
        let (m, p) = parse_povm_hydration(Some(data));
        assert_eq!(m, 0);
        assert_eq!(p, 0);
    }

    #[test]
    fn parse_povm_string_values_not_parsed() {
        let data = r#"{"memory_count": "5", "pathway_count": "10"}"#;
        let (m, p) = parse_povm_hydration(Some(data));
        assert_eq!(m, 0);
        assert_eq!(p, 0);
    }

    #[test]
    fn parse_povm_large_values() {
        let data = r#"{"memory_count": 999999, "pathway_count": 888888}"#;
        let (m, p) = parse_povm_hydration(Some(data));
        assert_eq!(m, 999_999);
        assert_eq!(p, 888_888);
    }

    #[test]
    fn parse_povm_zero_values() {
        let data = r#"{"memory_count": 0, "pathway_count": 0}"#;
        let (m, p) = parse_povm_hydration(Some(data));
        assert_eq!(m, 0);
        assert_eq!(p, 0);
    }

    // ── RM parsing ──

    #[test]
    fn parse_rm_none() {
        assert_eq!(parse_rm_count(None), 0);
    }

    #[test]
    fn parse_rm_empty_array() {
        assert_eq!(parse_rm_count(Some("[]")), 0);
    }

    #[test]
    fn parse_rm_valid_array() {
        let data = r#"[{"entry": 1}, {"entry": 2}, {"entry": 3}]"#;
        assert_eq!(parse_rm_count(Some(data)), 3);
    }

    #[test]
    fn parse_rm_not_array() {
        assert_eq!(parse_rm_count(Some(r#"{"count": 5}"#)), 0);
    }

    #[test]
    fn parse_rm_invalid_json() {
        assert_eq!(parse_rm_count(Some("not json")), 0);
    }

    #[test]
    fn parse_rm_single_element() {
        assert_eq!(parse_rm_count(Some("[1]")), 1);
    }

    #[test]
    fn parse_rm_large_array() {
        let entries: Vec<serde_json::Value> =
            (0..100).map(|i| serde_json::json!({"id": i})).collect();
        let data = serde_json::to_string(&entries).unwrap();
        assert_eq!(parse_rm_count(Some(&data)), 100);
    }

    // ── Message formatting ──

    #[test]
    fn session_start_message_format() {
        let msg = format!(
            "[HABITAT] Hydrated: POVM {} memories, {} pathways | RM {} discoveries",
            42, 100, 5,
        );
        assert!(msg.contains("POVM 42 memories"));
        assert!(msg.contains("100 pathways"));
        assert!(msg.contains("RM 5 discoveries"));
    }

    #[test]
    fn stop_rm_tsv_format() {
        let pane_id = "test-pane";
        let r = 0.995;
        let tsv = format!("session\t{pane_id}\tsession-end\t3600\tsession-end r={r}");
        assert!(tsv.contains('\t'));
        assert!(tsv.contains("test-pane"));
        assert!(tsv.contains("session-end r=0.995"));
        // Verify TSV has exactly 4 tabs (5 fields)
        assert_eq!(tsv.matches('\t').count(), 4);
    }

    // ── Session lifecycle ──

    #[test]
    fn session_id_generation_fallback() {
        let event = HookEvent::default();
        let session_id = event
            .session_id
            .unwrap_or_else(|| format!("sess-{}", uuid::Uuid::new_v4()));
        assert!(session_id.starts_with("sess-"));
    }

    #[test]
    fn stop_with_no_tracker() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        let tracker = state.remove_session("nonexistent");
        assert!(tracker.is_none());
    }

    #[test]
    fn stop_with_tracker_no_task() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        state.register_session(
            "test-session".into(),
            crate::m1_core::m01_core_types::PaneId::new("test"),
        );
        let tracker = state.remove_session("test-session");
        assert!(tracker.is_some());
        assert!(tracker.unwrap().active_task_id.is_none());
    }

    #[test]
    fn stop_with_tracker_with_task() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        state.register_session(
            "test-session".into(),
            crate::m1_core::m01_core_types::PaneId::new("test"),
        );
        {
            let mut sessions = state.sessions.write();
            if let Some(tracker) = sessions.get_mut("test-session") {
                tracker.active_task_id = Some("task-42".into());
            }
        }
        let tracker = state.remove_session("test-session");
        assert!(tracker.is_some());
        assert_eq!(tracker.unwrap().active_task_id.as_deref(), Some("task-42"));
    }

    #[test]
    fn generate_pane_id_has_colon() {
        let id = generate_pane_id();
        assert!(id.as_str().contains(':'));
    }

    #[test]
    fn hook_response_empty_serializes() {
        let resp = HookResponse::empty();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, "{}");
    }
}
