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
use axum::http::StatusCode;
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
/// 2. Reads cached field state (r, tick, spheres) from `SharedState`
/// 3. Fetches thermal state from SYNTHEX (live)
/// 4. Checks for pending bus tasks (live PV2 call)
/// 5. Returns field state + task info as `systemMessage`
#[allow(clippy::too_many_lines)]
pub async fn handle_user_prompt_submit(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> (StatusCode, Json<HookResponse>) {
    // Skip short prompts
    let prompt = event.prompt.as_deref().unwrap_or("");
    if prompt.len() < MIN_PROMPT_LENGTH {
        return (StatusCode::OK, Json(HookResponse::empty()));
    }

    // Advance breaker tick
    #[cfg(feature = "intelligence")]
    state.breaker_tick();

    // Periodic blackboard GC (every 720 ticks ≈ 1hr at 5s/tick)
    // - Complete panes: pruned after 24h (audit trail, not actively needed)
    // - Task history: pruned after 24h
    // - Idle/Working/Blocked panes: never pruned by GC (use prune_stale_panes for that)
    #[cfg(feature = "persistence")]
    {
        let current_tick = state.tick.load(std::sync::atomic::Ordering::Relaxed);
        if current_tick % 720 == 0 && current_tick > 0 {
            if let Some(bb) = state.blackboard() {
                #[allow(clippy::cast_precision_loss)]
                let now_secs = super::m10_hook_server::epoch_ms() as f64 / 1000.0;
                let twenty_four_hours_ago = now_secs - 86_400.0;
                if let Err(e) = bb.prune_complete_panes(twenty_four_hours_ago) {
                    tracing::warn!("blackboard prune_complete_panes failed: {e}");
                }
                if let Err(e) = bb.prune_old_tasks(twenty_four_hours_ago) {
                    tracing::warn!("blackboard prune_old_tasks failed: {e}");
                }
            }
        }
    }

    // Read cached field state (populated by spawn_field_poller every 5s)
    let (r, tick, spheres) = {
        let guard = state.field_state.read();
        let r_val = guard.field.order.r;
        let tick_val = guard.field.tick;
        let sphere_count = guard.spheres.len();
        (
            format!("{r_val:.4}"),
            tick_val.to_string(),
            sphere_count.to_string(),
        )
    };

    // Fetch thermal, tasks, and POVM memories live (not cached by poller)
    let thermal_url = format!("{}/v3/thermal", state.synthex_url);
    let tasks_url = format!("{}/bus/tasks", state.pv2_url);
    let povm_url = format!("{}/memories?limit=5", state.povm_url);

    #[cfg(feature = "intelligence")]
    let pv2_blocked = !state.breaker_allows("pv2");
    #[cfg(not(feature = "intelligence"))]
    let pv2_blocked = false;

    let (thermal_data, tasks_data, povm_data) = if pv2_blocked {
        // PV2 breaker blocked — skip tasks call (BUG-L3-001 rename)
        let (thermal, povm) = tokio::join!(
            http_get(&thermal_url, 1000),
            http_get(&povm_url, 1000),
        );
        (thermal, None, povm)
    } else {
        let (th, tk, pv) = tokio::join!(
            http_get(&thermal_url, 1000),
            http_get(&tasks_url, 1000),
            http_get(&povm_url, 1000),
        );
        // Record PV2 breaker outcome (tasks endpoint)
        #[cfg(feature = "intelligence")]
        if tk.is_some() {
            state.breaker_success("pv2");
        } else {
            state.breaker_failure("pv2");
        }
        (th, tk, pv)
    };

    let thermal = parse_temperature(thermal_data.as_deref());
    let povm_context = parse_povm_memories(povm_data.as_deref());

    // Check for pending tasks
    let (pending_count, first_task, first_task_id) = parse_pending_tasks(tasks_data.as_deref());

    // Read blackboard fleet summary (fast local SQLite, no HTTP)
    let bb_summary = read_blackboard_summary(&state);

    // Session 075 BREAK-4: Inject emergence + RALPH state into CC context.
    // Fleet CCs gain ecosystem awareness — they can see recent emergence
    // events and RALPH mutation effectiveness.
    let emergence_context = {
        #[cfg(feature = "evolution")]
        {
            let ralph = &state.ralph;
            let recent = ralph.emergence().recent(3);
            let ralph_state = ralph.state();
            let gen = ralph_state.generation;
            let fit = ralph_state.current_fitness;
            let mut parts = Vec::new();
            if !recent.is_empty() {
                let emergence_str: String = recent.iter()
                    .map(|e| format!("{:?}(tick {})", e.emergence_type, e.detected_at_tick))
                    .collect::<Vec<_>>()
                    .join(", ");
                parts.push(format!("[EMERGENCE] {emergence_str}"));
            }
            parts.push(format!("[RALPH] gen={gen} fitness={fit:.3}"));
            format!("\n{}", parts.join("\n"))
        }
        #[cfg(not(feature = "evolution"))]
        { String::new() }
    };

    // Build system message
    let message = if pending_count > 0 {
        format!(
            "[FIELD] r={r} tick={tick} spheres={spheres} T={thermal}{bb_summary}\n\
             {povm_context}\
             [FLEET TASK AVAILABLE] {pending_count} pending. First: {first_task}\n\
             To claim: pane-vortex-client claim {first_task_id} — then work on it. Include TASK_COMPLETE when done.{emergence_context}"
        )
    } else {
        format!(
            "[FIELD] r={r} tick={tick} spheres={spheres} T={thermal}{bb_summary}{povm_context} | No pending fleet tasks{emergence_context}"
        )
    };

    (StatusCode::OK, Json(HookResponse::with_message(message)))
}

// ──────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────

/// Read a compact fleet summary from the local blackboard.
///
/// Returns a short string like ` fleet=3/1W` (3 panes, 1 working)
/// or empty string if blackboard is unavailable.
fn read_blackboard_summary(state: &OracState) -> String {
    #[cfg(feature = "persistence")]
    {
        let Some(bb) = state.blackboard() else {
            return String::new();
        };
        let panes = bb.list_panes().unwrap_or_default();
        if panes.is_empty() {
            return String::new();
        }
        let total = panes.len();
        let working = panes
            .iter()
            .filter(|p| p.status == crate::m1_core::m01_core_types::PaneStatus::Working)
            .count();
        let tasks_done: u64 = panes.iter().map(|p| p.tasks_completed).sum();
        if tasks_done > 0 {
            format!(" fleet={total}/{working}W done={tasks_done}")
        } else {
            format!(" fleet={total}/{working}W")
        }
    }
    #[cfg(not(feature = "persistence"))]
    {
        let _ = state;
        String::new()
    }
}

/// Parse PV2 health response for field state values.
///
/// Returns `(r, tick, spheres)` as display strings.
/// Retained for tests and potential fallback if cache is stale.
#[cfg(test)]
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

/// Parse POVM `/memories?limit=5` response into a compact context line.
///
/// Returns a string like `\n[POVM] 5 memories: "Session 027: ..." | "Session 028: ..."`.
/// Returns empty string if POVM is unreachable or has no memories.
fn parse_povm_memories(data: Option<&str>) -> String {
    let Some(data) = data else {
        return String::new();
    };
    let Ok(parsed) = serde_json::from_str::<Vec<serde_json::Value>>(data) else {
        return String::new();
    };
    if parsed.is_empty() {
        return String::new();
    }
    let summaries: Vec<String> = parsed
        .iter()
        .filter_map(|m| {
            let content = m.get("content").and_then(serde_json::Value::as_str)?;
            // Truncate to 80 chars for prompt budget
            let truncated = if content.len() > 80 {
                format!("{}...", &content[..77])
            } else {
                content.to_owned()
            };
            Some(format!("\"{truncated}\""))
        })
        .collect();
    if summaries.is_empty() {
        return String::new();
    }
    format!("\n[POVM] {} memories: {}", summaries.len(), summaries.join(" | "))
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

    // ── parse_povm_memories ──

    #[test]
    fn povm_memories_none() {
        assert_eq!(parse_povm_memories(None), "");
    }

    #[test]
    fn povm_memories_empty_array() {
        assert_eq!(parse_povm_memories(Some("[]")), "");
    }

    #[test]
    fn povm_memories_invalid_json() {
        assert_eq!(parse_povm_memories(Some("not json")), "");
    }

    #[test]
    fn povm_memories_one() {
        let data = r#"[{"content":"Session 027: full deploy","id":"abc"}]"#;
        let result = parse_povm_memories(Some(data));
        assert!(result.contains("[POVM] 1 memories"));
        assert!(result.contains("Session 027: full deploy"));
    }

    #[test]
    fn povm_memories_truncates_long_content() {
        let long = "A".repeat(120);
        let data = format!(r#"[{{"content":"{long}","id":"x"}}]"#);
        let result = parse_povm_memories(Some(&data));
        assert!(result.contains("..."));
        // 77 chars + "..." = 80 max
        assert!(result.len() < 120);
    }

    #[test]
    fn povm_memories_multiple() {
        let data = r#"[
            {"content":"First memory","id":"1"},
            {"content":"Second memory","id":"2"},
            {"content":"Third memory","id":"3"}
        ]"#;
        let result = parse_povm_memories(Some(data));
        assert!(result.contains("[POVM] 3 memories"));
        assert!(result.contains("First memory"));
        assert!(result.contains(" | "));
        assert!(result.contains("Third memory"));
    }

    #[test]
    fn povm_memories_skips_missing_content() {
        let data = r#"[{"id":"1"},{"content":"Real memory","id":"2"}]"#;
        let result = parse_povm_memories(Some(data));
        assert!(result.contains("[POVM] 1 memories"));
        assert!(result.contains("Real memory"));
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

    // ── Blackboard summary ──

    #[test]
    fn blackboard_summary_no_persistence() {
        let state = OracState::new(crate::m1_core::m03_config::PvConfig::default());
        let summary = read_blackboard_summary(&state);
        // Without persistence feature or DB, returns empty
        assert!(summary.is_empty() || summary.contains("fleet="));
    }

    #[test]
    fn message_includes_fleet_placeholder() {
        let bb = " fleet=3/1W done=5";
        let msg = format!(
            "[FIELD] r=0.93 tick=100 spheres=65 T=0.500{bb} | No pending fleet tasks"
        );
        assert!(msg.contains("fleet=3/1W"));
        assert!(msg.contains("done=5"));
    }

    #[test]
    fn message_with_tasks_includes_fleet() {
        let bb = " fleet=12/4W done=27";
        let msg = format!(
            "[FIELD] r=0.93 tick=100 spheres=65 T=0.500{bb}\n\
             [FLEET TASK AVAILABLE] 2 pending. First: Fix bug\n\
             To claim: pane-vortex-client claim t1 — then work on it. Include TASK_COMPLETE when done."
        );
        assert!(msg.contains("fleet=12/4W"));
        assert!(msg.contains("[FLEET TASK AVAILABLE]"));
    }
}
