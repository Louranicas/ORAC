//! # M13: Prompt Hooks
//!
//! Handler for `UserPromptSubmit` hook events from Claude Code.
//!
//! Injects field state (r, tick, spheres, thermal) and pending bus tasks
//! into every user prompt. Skips short prompts (<20 chars) to avoid
//! noise on simple commands.
//!
//! ## Layer: L3 (Hooks) | Module: M13
//! ## Dependencies: `m10_hook_server` (`OracState`, `HookEvent`, `HookResponse`)

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use super::m10_hook_server::{http_get, HookEvent, HookResponse, OracState};

/// Minimum prompt length to trigger field state injection.
const MIN_PROMPT_LENGTH: usize = 20;

// ──────────────────────────────────────────────────────────────
// UserPromptSubmit handler
// ──────────────────────────────────────────────────────────────

/// Handle `UserPromptSubmit` hook from Claude Code.
///
/// 1. Skips short prompts (< 20 chars)
/// 2. Fetches field state from PV2 (r, tick, spheres)
/// 3. Fetches thermal state from SYNTHEX
/// 4. Checks for pending bus tasks
/// 5. Returns field state + task info as `systemMessage`
pub async fn handle_user_prompt_submit(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse> {
    // Skip short prompts
    let prompt = event.prompt.as_deref().unwrap_or("");
    if prompt.len() < MIN_PROMPT_LENGTH {
        return Json(HookResponse::empty());
    }

    // Advance breaker tick
    #[cfg(feature = "intelligence")]
    state.breaker_tick();

    // Parallel data collection (breaker-gated)
    let pv2_health_url = format!("{}/health", state.pv2_url);
    let thermal_url = format!("{}/v3/thermal", state.synthex_url);
    let tasks_url = format!("{}/bus/tasks", state.pv2_url);

    #[cfg(feature = "intelligence")]
    let pv2_open = !state.breaker_allows("pv2");
    #[cfg(not(feature = "intelligence"))]
    let pv2_open = false;

    let (pv_data, thermal_data, tasks_data) = if pv2_open {
        // PV2 breaker open — skip all PV2 calls, fall back to cached/unknown
        let thermal = http_get(&thermal_url, 1000).await;
        (None, thermal, None)
    } else {
        let (pv, th, tk) = tokio::join!(
            http_get(&pv2_health_url, 1000),
            http_get(&thermal_url, 1000),
            http_get(&tasks_url, 1000),
        );
        // Record PV2 breaker outcome
        #[cfg(feature = "intelligence")]
        if pv.is_some() {
            state.breaker_success("pv2");
        } else {
            state.breaker_failure("pv2");
        }
        (pv, th, tk)
    };

    // Parse field state
    let (r, tick, spheres) = parse_field_state(pv_data.as_deref());
    let thermal = parse_temperature(thermal_data.as_deref());

    // Check for pending tasks
    let (pending_count, first_task, first_task_id) = parse_pending_tasks(tasks_data.as_deref());

    // Build system message
    let message = if pending_count > 0 {
        format!(
            "[FIELD] r={r} tick={tick} spheres={spheres} T={thermal}\n\
             [FLEET TASK AVAILABLE] {pending_count} pending. First: {first_task}\n\
             To claim: pane-vortex-client claim {first_task_id} — then work on it. Include TASK_COMPLETE when done."
        )
    } else {
        format!(
            "[FIELD] r={r} tick={tick} spheres={spheres} T={thermal} | No pending fleet tasks"
        )
    };

    Json(HookResponse::with_message(message))
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Parse PV2 health response for field state values.
///
/// Returns `(r, tick, spheres)` as display strings.
fn parse_field_state(data: Option<&str>) -> (String, String, String) {
    let Some(data) = data else {
        return ("?".into(), "?".into(), "?".into());
    };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return ("?".into(), "?".into(), "?".into()),
    };
    let r = parsed
        .get("r")
        .map_or("?".to_owned(), |v| format!("{:.4}", v.as_f64().unwrap_or(0.0)));
    let tick = parsed
        .get("tick")
        .map_or("?".to_owned(), serde_json::Value::to_string);
    let spheres = parsed
        .get("spheres")
        .map_or("?".to_owned(), serde_json::Value::to_string);
    (r, tick, spheres)
}

/// Parse SYNTHEX thermal response for temperature display value.
fn parse_temperature(data: Option<&str>) -> String {
    let Some(data) = data else {
        return "?".into();
    };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return "?".into(),
    };
    parsed
        .get("temperature")
        .map_or("?".to_owned(), |v| format!("{:.3}", v.as_f64().unwrap_or(0.0)))
}

/// Parse bus tasks response for pending task info.
///
/// Returns `(pending_count, first_description, first_id)`.
fn parse_pending_tasks(data: Option<&str>) -> (usize, String, String) {
    let Some(data) = data else {
        return (0, String::new(), String::new());
    };
    let parsed: serde_json::Value = match serde_json::from_str(data) {
        Ok(v) => v,
        Err(_) => return (0, String::new(), String::new()),
    };
    let Some(tasks) = parsed.get("tasks").and_then(serde_json::Value::as_array) else {
        return (0, String::new(), String::new());
    };

    let pending: Vec<&serde_json::Value> = tasks
        .iter()
        .filter(|t| {
            t.get("status")
                .and_then(serde_json::Value::as_str)
                == Some("Pending")
        })
        .collect();

    let count = pending.len();
    if count == 0 {
        return (0, String::new(), String::new());
    }

    let first = pending[0];
    let desc = first
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();
    let id = first
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .to_owned();

    (count, desc, id)
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_field_state ──

    #[test]
    fn field_state_none() {
        let (r, tick, spheres) = parse_field_state(None);
        assert_eq!(r, "?");
        assert_eq!(tick, "?");
        assert_eq!(spheres, "?");
    }

    #[test]
    fn field_state_valid() {
        let data = r#"{"r": 0.9876, "tick": 12345, "spheres": 8}"#;
        let (r, tick, spheres) = parse_field_state(Some(data));
        assert_eq!(r, "0.9876");
        assert_eq!(tick, "12345");
        assert_eq!(spheres, "8");
    }

    #[test]
    fn field_state_partial() {
        let data = r#"{"r": 0.5}"#;
        let (r, tick, spheres) = parse_field_state(Some(data));
        assert_eq!(r, "0.5000");
        assert_eq!(tick, "?");
        assert_eq!(spheres, "?");
    }

    #[test]
    fn field_state_invalid() {
        let (r, tick, spheres) = parse_field_state(Some("not json"));
        assert_eq!(r, "?");
        assert_eq!(tick, "?");
        assert_eq!(spheres, "?");
    }

    #[test]
    fn field_state_zero_r() {
        let data = r#"{"r": 0.0, "tick": 0, "spheres": 0}"#;
        let (r, tick, spheres) = parse_field_state(Some(data));
        assert_eq!(r, "0.0000");
        assert_eq!(tick, "0");
        assert_eq!(spheres, "0");
    }

    // ── parse_temperature ──

    #[test]
    fn temperature_none() {
        assert_eq!(parse_temperature(None), "?");
    }

    #[test]
    fn temperature_valid() {
        let data = r#"{"temperature": 0.456}"#;
        assert_eq!(parse_temperature(Some(data)), "0.456");
    }

    #[test]
    fn temperature_invalid() {
        assert_eq!(parse_temperature(Some("nope")), "?");
    }

    #[test]
    fn temperature_missing_field() {
        assert_eq!(parse_temperature(Some("{}")), "?");
    }

    #[test]
    fn temperature_zero() {
        let data = r#"{"temperature": 0.0}"#;
        assert_eq!(parse_temperature(Some(data)), "0.000");
    }

    // ── parse_pending_tasks ──

    #[test]
    fn pending_tasks_none() {
        let (c, _, _) = parse_pending_tasks(None);
        assert_eq!(c, 0);
    }

    #[test]
    fn pending_tasks_empty() {
        let (c, _, _) = parse_pending_tasks(Some(r#"{"tasks":[]}"#));
        assert_eq!(c, 0);
    }

    #[test]
    fn pending_tasks_one() {
        let data = r#"{"tasks":[
            {"id":"t1","description":"Fix bug","status":"Pending"}
        ]}"#;
        let (c, desc, id) = parse_pending_tasks(Some(data));
        assert_eq!(c, 1);
        assert_eq!(desc, "Fix bug");
        assert_eq!(id, "t1");
    }

    #[test]
    fn pending_tasks_multiple() {
        let data = r#"{"tasks":[
            {"id":"t1","description":"First","status":"Pending"},
            {"id":"t2","description":"Second","status":"Pending"}
        ]}"#;
        let (c, desc, id) = parse_pending_tasks(Some(data));
        assert_eq!(c, 2);
        assert_eq!(desc, "First");
        assert_eq!(id, "t1");
    }

    #[test]
    fn pending_tasks_filters_non_pending() {
        let data = r#"{"tasks":[
            {"id":"t1","description":"Done","status":"Completed"},
            {"id":"t2","description":"Active","status":"Pending"}
        ]}"#;
        let (c, desc, id) = parse_pending_tasks(Some(data));
        assert_eq!(c, 1);
        assert_eq!(desc, "Active");
        assert_eq!(id, "t2");
    }

    #[test]
    fn pending_tasks_no_tasks_key() {
        let (c, _, _) = parse_pending_tasks(Some("{}"));
        assert_eq!(c, 0);
    }

    #[test]
    fn pending_tasks_invalid_json() {
        let (c, _, _) = parse_pending_tasks(Some("not json"));
        assert_eq!(c, 0);
    }

    // ── MIN_PROMPT_LENGTH ──

    #[test]
    fn min_prompt_length() {
        assert_eq!(MIN_PROMPT_LENGTH, 20);
    }

    // ── Message formatting ──

    #[test]
    fn message_no_tasks() {
        let msg = format!(
            "[FIELD] r={} tick={} spheres={} T={} | No pending fleet tasks",
            "0.9876", "12345", "8", "0.456",
        );
        assert!(msg.contains("r=0.9876"));
        assert!(msg.contains("tick=12345"));
        assert!(msg.contains("No pending fleet tasks"));
    }

    #[test]
    fn message_with_tasks() {
        let msg = format!(
            "[FIELD] r={} tick={} spheres={} T={}\n\
             [FLEET TASK AVAILABLE] {} pending. First: {}\n\
             To claim: pane-vortex-client claim {} — then work on it. Include TASK_COMPLETE when done.",
            "0.99", "100", "5", "0.5", 2, "Fix bug", "t1",
        );
        assert!(msg.contains("[FLEET TASK AVAILABLE]"));
        assert!(msg.contains("Fix bug"));
        assert!(msg.contains("TASK_COMPLETE"));
    }

    // ── Short prompt skip ──

    #[test]
    fn short_prompt_detected() {
        let prompt = "Fix it";
        assert!(prompt.len() < MIN_PROMPT_LENGTH);
    }

    #[test]
    fn long_prompt_passes() {
        let prompt = "Please fix the authentication bug in the login handler";
        assert!(prompt.len() >= MIN_PROMPT_LENGTH);
    }
}
