//! # M12: Tool Hooks
//!
//! Handlers for `PostToolUse` and `PreToolUse` hook events from Claude Code.
//!
//! - **`PostToolUse`**: Records tool memory on PV2, updates sphere status,
//!   polls for pending bus tasks (1-in-5 throttled), claims and injects tasks.
//! - **`PreToolUse`**: Checks SYNTHEX thermal state, warns if system is hot
//!   (>30% over target for write operations).
//!
//! ## Layer: L3 (Hooks) | Module: M12
//! ## Dependencies: `m10_hook_server` (`OracState`, `HookEvent`, `HookResponse`)

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use super::m10_hook_server::{
    fire_and_forget_post, http_get, http_post, HookEvent, HookResponse, OracState,
};

/// Throttle divisor — poll tasks every N-th `PostToolUse` call.
const POLL_EVERY_N: u64 = 5;

// ──────────────────────────────────────────────────────────────
// PostToolUse handler
// ──────────────────────────────────────────────────────────────

/// Handle `PostToolUse` hook from Claude Code.
///
/// 1. Records tool use as sphere memory (fire-and-forget)
/// 2. Updates sphere status to "working" (fire-and-forget)
/// 3. Checks for `TASK_COMPLETE` in tool output (completes active task)
/// 4. Throttled task polling (1-in-5 calls)
/// 5. Claims first pending task if found, returns as `systemMessage`
pub async fn handle_post_tool_use(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse> {
    let tool_name = event.tool_name.as_deref().unwrap_or("unknown");
    let tool_output = event.tool_output.as_deref().unwrap_or("");
    let summary = event
        .tool_input
        .as_ref()
        .map(|v| truncate_string(&v.to_string(), 200))
        .unwrap_or_default();

    // Find session for this hook call
    let (session_id, pane_id_str) = find_session_pane(&state, &event);

    // 1. Record memory (fire-and-forget)
    let mem_url = format!("{}/sphere/{}/memory", state.pv2_url, pane_id_str);
    let mem_body = serde_json::json!({
        "tool_name": tool_name,
        "summary": summary,
    })
    .to_string();
    fire_and_forget_post(mem_url, mem_body);

    // 2. Update status (fire-and-forget)
    let status_url = format!("{}/sphere/{}/status", state.pv2_url, pane_id_str);
    let status_body = serde_json::json!({
        "status": "working",
        "last_tool": tool_name,
    })
    .to_string();
    fire_and_forget_post(status_url, status_body);

    // 3. Check for TASK_COMPLETE in tool output
    if tool_output.contains("TASK_COMPLETE") {
        if let Some(task_id) = get_active_task(&state, &session_id) {
            let complete_url = format!("{}/bus/complete/{}", state.pv2_url, task_id);
            fire_and_forget_post(complete_url, "{}".into());
            clear_active_task(&state, &session_id);
        }
    }

    // If we have an active task, skip polling — we're working on it
    if has_active_task(&state, &session_id) {
        return Json(HookResponse::empty());
    }

    // 4. Throttled task polling (1-in-5)
    let poll_count = increment_poll_counter(&state, &session_id);
    if poll_count % POLL_EVERY_N != 0 {
        return Json(HookResponse::empty());
    }

    // 5. Poll for pending tasks
    let tasks_url = format!("{}/bus/tasks", state.pv2_url);
    let tasks_data = http_get(&tasks_url, 1000).await;

    if let Some(task) = find_first_pending_task(tasks_data.as_deref()) {
        // Attempt atomic claim
        let claim_url = format!("{}/bus/claim/{}", state.pv2_url, task.id);
        let claim_body = serde_json::json!({"claimer": pane_id_str}).to_string();
        let claim_result = http_post(&claim_url, &claim_body, 2000).await;

        if is_claim_successful(claim_result.as_deref()) {
            set_active_task(&state, &session_id, &task.id);
            let message = format!(
                "[FLEET TASK] Claimed {}: {}. When done, include TASK_COMPLETE in your response.",
                task.id, task.description,
            );
            return Json(HookResponse::with_message(message));
        }
    }

    Json(HookResponse::empty())
}

// ──────────────────────────────────────────────────────────────
// PreToolUse handler
// ──────────────────────────────────────────────────────────────

/// Handle `PreToolUse` hook from Claude Code.
///
/// Only gates write operations (`Write`, `Edit`, `Bash`).
/// Checks SYNTHEX thermal state and warns if >30% over target.
/// Does NOT block — advisory only.
pub async fn handle_pre_tool_use(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse> {
    let tool_name = event.tool_name.as_deref().unwrap_or("");

    // Only gate write operations
    if !is_write_operation(tool_name) {
        return Json(HookResponse::empty());
    }

    // Check thermal state from SYNTHEX
    let thermal_url = format!("{}/v3/thermal", state.synthex_url);
    let thermal_data = http_get(&thermal_url, 1000).await;

    if let Some((temp, target)) = parse_thermal(thermal_data.as_deref()) {
        if target > 0.0 && (temp - target) / target > 0.3 {
            let message = format!(
                "[THERMAL] System HOT: T={temp:.3} target={target:.3}. Consider reducing write frequency."
            );
            return Json(HookResponse::with_message(message));
        }
    }

    Json(HookResponse::empty())
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Whether a tool name corresponds to a write operation.
#[must_use]
fn is_write_operation(tool_name: &str) -> bool {
    matches!(tool_name, "Write" | "Edit" | "Bash")
}

/// Truncate a string to at most `max_len` characters.
#[must_use]
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_owned()
    } else {
        format!("{}...", &s[..max_len.min(s.len())])
    }
}

/// Parsed pending task from bus response.
struct PendingTask {
    /// Task ID for claiming.
    id: String,
    /// Task description for the system message.
    description: String,
}

/// Find the first pending task in the bus tasks response.
fn find_first_pending_task(data: Option<&str>) -> Option<PendingTask> {
    let data = data?;
    let parsed: serde_json::Value = serde_json::from_str(data).ok()?;
    let tasks = parsed.get("tasks")?.as_array()?;
    tasks.iter().find_map(|t| {
        let status = t.get("status")?.as_str()?;
        if status != "Pending" {
            return None;
        }
        Some(PendingTask {
            id: t.get("id")?.as_str()?.to_owned(),
            description: t.get("description")?.as_str()?.to_owned(),
        })
    })
}

/// Check if a claim response indicates success.
fn is_claim_successful(data: Option<&str>) -> bool {
    let Some(data) = data else {
        return false;
    };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return false,
    };
    parsed
        .get("status")
        .and_then(serde_json::Value::as_str)
        == Some("Claimed")
}

/// Parse SYNTHEX thermal response for temperature and target.
fn parse_thermal(data: Option<&str>) -> Option<(f64, f64)> {
    let data = data?;
    let parsed: serde_json::Value = serde_json::from_str(data).ok()?;
    let temp = parsed.get("temperature")?.as_f64()?;
    let target = parsed.get("target").and_then(serde_json::Value::as_f64).unwrap_or(0.5);
    Some((temp, target))
}

/// Find the session and pane ID for this hook call.
fn find_session_pane(state: &OracState, event: &HookEvent) -> (String, String) {
    let session_id = event.session_id.clone().unwrap_or_default();
    let sessions = state.sessions.read();
    let pane_id = sessions
        .get(&session_id)
        .map_or_else(
            || {
                // Fall back to first session or generate
                sessions
                    .values()
                    .next()
                    .map_or_else(
                        || super::m10_hook_server::generate_pane_id().as_str().to_owned(),
                        |t| t.pane_id.as_str().to_owned(),
                    )
            },
            |t| t.pane_id.as_str().to_owned(),
        );
    (session_id, pane_id)
}

/// Get the active task ID for a session.
fn get_active_task(state: &OracState, session_id: &str) -> Option<String> {
    state
        .sessions
        .read()
        .get(session_id)
        .and_then(|t| t.active_task_id.clone())
}

/// Whether a session has an active task.
fn has_active_task(state: &OracState, session_id: &str) -> bool {
    state
        .sessions
        .read()
        .get(session_id)
        .is_some_and(|t| t.active_task_id.is_some())
}

/// Clear the active task for a session.
fn clear_active_task(state: &OracState, session_id: &str) {
    if let Some(tracker) = state.sessions.write().get_mut(session_id) {
        tracker.active_task_id = None;
    }
}

/// Set the active task for a session.
fn set_active_task(state: &OracState, session_id: &str, task_id: &str) {
    if let Some(tracker) = state.sessions.write().get_mut(session_id) {
        tracker.active_task_id = Some(task_id.to_owned());
    }
}

/// Increment and return the poll counter for a session.
fn increment_poll_counter(state: &OracState, session_id: &str) -> u64 {
    let mut sessions = state.sessions.write();
    if let Some(tracker) = sessions.get_mut(session_id) {
        tracker.poll_counter += 1;
        tracker.poll_counter
    } else {
        1
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_write_operation ──

    #[test]
    fn write_operation_write() {
        assert!(is_write_operation("Write"));
    }

    #[test]
    fn write_operation_edit() {
        assert!(is_write_operation("Edit"));
    }

    #[test]
    fn write_operation_bash() {
        assert!(is_write_operation("Bash"));
    }

    #[test]
    fn not_write_operation_read() {
        assert!(!is_write_operation("Read"));
    }

    #[test]
    fn not_write_operation_glob() {
        assert!(!is_write_operation("Glob"));
    }

    #[test]
    fn not_write_operation_grep() {
        assert!(!is_write_operation("Grep"));
    }

    #[test]
    fn not_write_operation_empty() {
        assert!(!is_write_operation(""));
    }

    // ── truncate_string ──

    #[test]
    fn truncate_short() {
        assert_eq!(truncate_string("hello", 10), "hello");
    }

    #[test]
    fn truncate_exact() {
        assert_eq!(truncate_string("hello", 5), "hello");
    }

    #[test]
    fn truncate_long() {
        let result = truncate_string("hello world", 5);
        assert!(result.len() <= 8); // 5 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate_string("", 10), "");
    }

    // ── find_first_pending_task ──

    #[test]
    fn pending_task_none_input() {
        assert!(find_first_pending_task(None).is_none());
    }

    #[test]
    fn pending_task_empty_array() {
        assert!(find_first_pending_task(Some(r#"{"tasks":[]}"#)).is_none());
    }

    #[test]
    fn pending_task_found() {
        let data = r#"{"tasks":[
            {"id":"t1","description":"Fix bug","status":"Pending","target":{"type":"AnyIdle"},"submitted_by":"alpha","submitted_at":0.0}
        ]}"#;
        let task = find_first_pending_task(Some(data));
        assert!(task.is_some());
        let t = task.unwrap();
        assert_eq!(t.id, "t1");
        assert_eq!(t.description, "Fix bug");
    }

    #[test]
    fn pending_task_skips_claimed() {
        let data = r#"{"tasks":[
            {"id":"t1","description":"Claimed one","status":"Claimed"},
            {"id":"t2","description":"Pending one","status":"Pending"}
        ]}"#;
        let task = find_first_pending_task(Some(data));
        assert!(task.is_some());
        assert_eq!(task.unwrap().id, "t2");
    }

    #[test]
    fn pending_task_all_completed() {
        let data = r#"{"tasks":[
            {"id":"t1","description":"Done","status":"Completed"},
            {"id":"t2","description":"Failed","status":"Failed"}
        ]}"#;
        assert!(find_first_pending_task(Some(data)).is_none());
    }

    #[test]
    fn pending_task_invalid_json() {
        assert!(find_first_pending_task(Some("not json")).is_none());
    }

    #[test]
    fn pending_task_no_tasks_key() {
        assert!(find_first_pending_task(Some("{}")).is_none());
    }

    // ── is_claim_successful ──

    #[test]
    fn claim_success() {
        assert!(is_claim_successful(Some(r#"{"status":"Claimed"}"#)));
    }

    #[test]
    fn claim_failure_pending() {
        assert!(!is_claim_successful(Some(r#"{"status":"Pending"}"#)));
    }

    #[test]
    fn claim_failure_none() {
        assert!(!is_claim_successful(None));
    }

    #[test]
    fn claim_failure_invalid() {
        assert!(!is_claim_successful(Some("not json")));
    }

    #[test]
    fn claim_failure_no_status() {
        assert!(!is_claim_successful(Some("{}")));
    }

    // ── parse_thermal ──

    #[test]
    fn thermal_valid() {
        let data = r#"{"temperature": 0.7, "target": 0.5}"#;
        let result = parse_thermal(Some(data));
        assert!(result.is_some());
        let (temp, target) = result.unwrap();
        assert!((temp - 0.7).abs() < f64::EPSILON);
        assert!((target - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn thermal_no_target_defaults() {
        let data = r#"{"temperature": 0.6}"#;
        let result = parse_thermal(Some(data));
        assert!(result.is_some());
        let (_, target) = result.unwrap();
        assert!((target - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn thermal_none() {
        assert!(parse_thermal(None).is_none());
    }

    #[test]
    fn thermal_invalid_json() {
        assert!(parse_thermal(Some("nope")).is_none());
    }

    #[test]
    fn thermal_no_temperature() {
        assert!(parse_thermal(Some(r#"{"target": 0.5}"#)).is_none());
    }

    #[test]
    fn thermal_hot_detection() {
        let (temp, target) = (0.75, 0.5);
        let ratio = (temp - target) / target;
        assert!(ratio > 0.3, "0.5 * 1.3 = 0.65, 0.75 > 0.65 → hot");
    }

    #[test]
    fn thermal_not_hot() {
        let (temp, target) = (0.55, 0.5);
        let ratio = (temp - target) / target;
        assert!(ratio <= 0.3, "0.5 * 1.3 = 0.65, 0.55 < 0.65 → not hot");
    }

    // ── Session helpers ──

    #[test]
    fn active_task_lifecycle() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        let session_id = "test";
        state.register_session(
            session_id.into(),
            crate::m1_core::m01_core_types::PaneId::new("pane"),
        );

        assert!(!has_active_task(&state, session_id));
        assert!(get_active_task(&state, session_id).is_none());

        set_active_task(&state, session_id, "task-1");
        assert!(has_active_task(&state, session_id));
        assert_eq!(get_active_task(&state, session_id).as_deref(), Some("task-1"));

        clear_active_task(&state, session_id);
        assert!(!has_active_task(&state, session_id));
    }

    #[test]
    fn poll_counter_increments() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        let session_id = "test";
        state.register_session(
            session_id.into(),
            crate::m1_core::m01_core_types::PaneId::new("pane"),
        );

        assert_eq!(increment_poll_counter(&state, session_id), 1);
        assert_eq!(increment_poll_counter(&state, session_id), 2);
        assert_eq!(increment_poll_counter(&state, session_id), 3);
        assert_eq!(increment_poll_counter(&state, session_id), 4);
        assert_eq!(increment_poll_counter(&state, session_id), 5);
    }

    #[test]
    fn poll_counter_throttle() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        let session_id = "test";
        state.register_session(
            session_id.into(),
            crate::m1_core::m01_core_types::PaneId::new("pane"),
        );

        // Only every 5th call should pass throttle
        for i in 1..=10 {
            let count = increment_poll_counter(&state, session_id);
            if i % 5 == 0 {
                assert_eq!(count % POLL_EVERY_N, 0, "call {i} should pass throttle");
            } else {
                assert_ne!(count % POLL_EVERY_N, 0, "call {i} should be throttled");
            }
        }
    }

    #[test]
    fn poll_counter_no_session_returns_one() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        // No session registered — returns 1 as fallback
        assert_eq!(increment_poll_counter(&state, "none"), 1);
    }

    // ── Task message formatting ──

    #[test]
    fn task_message_format() {
        let msg = format!(
            "[FLEET TASK] Claimed {}: {}. When done, include TASK_COMPLETE in your response.",
            "task-42", "Fix the bug",
        );
        assert!(msg.contains("task-42"));
        assert!(msg.contains("Fix the bug"));
        assert!(msg.contains("TASK_COMPLETE"));
    }
}
