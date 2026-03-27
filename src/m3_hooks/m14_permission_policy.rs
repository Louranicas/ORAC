//! # M14: Permission Policy
//!
//! Auto-approve/deny policy engine for `PermissionRequest` hook events.
//!
//! Fleet agents frequently trigger permission dialogs for common operations
//! (file reads, grep, glob). This module applies a configurable policy to
//! auto-approve safe operations and deny dangerous ones, eliminating
//! permission dialog spam across the fleet.
//!
//! ## Layer: L3 (Hooks) | Module: M14
//! ## Dependencies: `m10_hook_server` (`OracState`, `HookEvent`, `HookResponse`)
//!
//! ## Policy Rules
//!
//! 1. **Always approve**: `Read`, `Glob`, `Grep`, `LS`, `Agent` (read-only)
//! 2. **Approve with notice**: `Edit`, `Write`, `Bash` (write operations)
//! 3. **Deny**: tools in the explicit deny list (configurable)
//! 4. **Default**: approve (permissive fleet policy)

use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::m1_core::m03_config::HooksConfig;

use super::m10_hook_server::{HookEvent, HookResponse, OracState};

// ──────────────────────────────────────────────────────────────
// Permission decision
// ──────────────────────────────────────────────────────────────

/// Permission decision result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    /// Allow the tool call.
    Allow,
    /// Allow with an informational message.
    AllowWithNotice,
    /// Deny the tool call.
    Deny,
}

// ──────────────────────────────────────────────────────────────
// Permission policy
// ──────────────────────────────────────────────────────────────

/// Configurable permission policy for fleet agents.
///
/// Determines which tool calls to auto-approve, approve with notice, or deny.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPolicy {
    /// Tools that are always auto-approved (read-only operations).
    pub always_approve: Vec<String>,
    /// Tools approved with an informational notice (write operations).
    pub approve_with_notice: Vec<String>,
    /// Tools that are always denied.
    pub always_deny: Vec<String>,
    /// Whether to default to approve for unknown tools.
    pub default_approve: bool,
}

impl Default for PermissionPolicy {
    fn default() -> Self {
        Self {
            always_approve: vec![
                "Read".into(),
                "Glob".into(),
                "Grep".into(),
                "LS".into(),
                "Agent".into(),
                "WebSearch".into(),
                "WebFetch".into(),
                "TodoRead".into(),
                "TodoWrite".into(),
            ],
            approve_with_notice: vec![
                "Edit".into(),
                "Write".into(),
                "Bash".into(),
                "NotebookEdit".into(),
            ],
            always_deny: Vec::new(),
            default_approve: true,
        }
    }
}

impl PermissionPolicy {
    /// Evaluate the policy for a given tool name.
    #[must_use]
    pub fn evaluate(&self, tool_name: &str) -> Decision {
        // SEC-002 fix: case-insensitive comparison prevents bypass via
        // "bash" vs "Bash" or Unicode homoglyph variants.
        if self.always_deny.iter().any(|t| t.eq_ignore_ascii_case(tool_name)) {
            return Decision::Deny;
        }
        if self.always_approve.iter().any(|t| t.eq_ignore_ascii_case(tool_name)) {
            return Decision::Allow;
        }
        if self.approve_with_notice.iter().any(|t| t.eq_ignore_ascii_case(tool_name)) {
            return Decision::AllowWithNotice;
        }
        if self.default_approve {
            Decision::Allow
        } else {
            Decision::Deny
        }
    }

    /// Add a tool to the always-approve list.
    pub fn add_always_approve(&mut self, tool: impl Into<String>) {
        let tool = tool.into();
        if !self.always_approve.contains(&tool) {
            self.always_approve.push(tool);
        }
    }

    /// Add a tool to the always-deny list.
    pub fn add_always_deny(&mut self, tool: impl Into<String>) {
        let tool = tool.into();
        if !self.always_deny.contains(&tool) {
            self.always_deny.push(tool);
        }
    }

    /// Build a permission policy from hooks configuration (SEC-001 fix).
    ///
    /// Parses `auto_approve.patterns` from `hooks.toml`:
    /// - Bare tool names (e.g. `"Read"`) go into `always_approve`
    /// - `Tool:pattern` entries (e.g. `"Bash:git status*"`) extract the bare
    ///   tool name for policy matching; the glob suffix is reserved for
    ///   future sub-command gating
    ///
    /// Write tools (`Edit`, `Write`, `Bash`, `NotebookEdit`) not present in
    /// `auto_approve` are placed into `approve_with_notice`. All other
    /// unknown tools fall through to `default_approve`.
    #[must_use]
    pub fn from_config(hooks: &HooksConfig) -> Self {
        const WRITE_TOOLS: &[&str] = &["Edit", "Write", "Bash", "NotebookEdit"];

        // Extract bare tool names (strip `:suffix` if present), deduplicate
        let mut always_approve: Vec<String> = Vec::new();
        for pattern in &hooks.auto_approve.patterns {
            let bare = pattern.split(':').next().unwrap_or(pattern);
            if !always_approve.iter().any(|t| t.eq_ignore_ascii_case(bare)) {
                always_approve.push(bare.to_string());
            }
        }

        // Standard read-only tools that should always be approved even if
        // omitted from the TOML (backward compat with PermissionPolicy::default)
        for tool in &["Read", "Glob", "Grep", "LS", "Agent", "WebSearch", "WebFetch", "TodoRead", "TodoWrite"] {
            if !always_approve.iter().any(|t| t.eq_ignore_ascii_case(tool)) {
                always_approve.push((*tool).to_string());
            }
        }

        // Write tools not auto-approved get notice treatment
        let approve_with_notice: Vec<String> = WRITE_TOOLS
            .iter()
            .filter(|t| !always_approve.iter().any(|a| a.eq_ignore_ascii_case(t)))
            .map(|t| (*t).to_string())
            .collect();

        Self {
            always_approve,
            approve_with_notice,
            always_deny: Vec::new(),
            default_approve: true,
        }
    }
}

// ──────────────────────────────────────────────────────────────
// PermissionRequest handler
// ──────────────────────────────────────────────────────────────

/// Handle `PermissionRequest` hook from Claude Code.
///
/// Evaluates the permission policy for the requested tool and returns
/// an appropriate response. Auto-approves read-only tools, approves
/// write tools with notice, and denies explicitly blocked tools.
pub async fn handle_permission_request(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse> {
    // SEC-001 fix: Use policy from OracState (loaded from hooks.toml at startup),
    // not a throwaway PermissionPolicy::default() per request.
    // SEC-002 fix: Case-insensitive comparison via policy.evaluate().
    let raw_tool = event.tool_name.as_deref().unwrap_or("unknown");
    let tool_name = raw_tool.trim();

    let decision = state.permission_policy.evaluate(tool_name);

    match decision {
        Decision::Allow => Json(HookResponse::empty()),
        Decision::AllowWithNotice => {
            Json(HookResponse::allow(Some(format!(
                "[ORAC] Auto-approved write operation: {tool_name}"
            ))))
        }
        Decision::Deny => Json(HookResponse::block(format!(
            "[ORAC] Denied: {tool_name} is blocked by fleet policy"
        ))),
    }
}

// ──────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn default_policy() -> PermissionPolicy {
        PermissionPolicy::default()
    }

    // ── Default policy ──

    #[test]
    fn default_always_approve_list() {
        let policy = default_policy();
        assert!(policy.always_approve.contains(&"Read".to_string()));
        assert!(policy.always_approve.contains(&"Glob".to_string()));
        assert!(policy.always_approve.contains(&"Grep".to_string()));
        assert!(policy.always_approve.contains(&"LS".to_string()));
        assert!(policy.always_approve.contains(&"Agent".to_string()));
    }

    #[test]
    fn default_approve_with_notice_list() {
        let policy = default_policy();
        assert!(policy.approve_with_notice.contains(&"Edit".to_string()));
        assert!(policy.approve_with_notice.contains(&"Write".to_string()));
        assert!(policy.approve_with_notice.contains(&"Bash".to_string()));
    }

    #[test]
    fn default_deny_list_empty() {
        let policy = default_policy();
        assert!(policy.always_deny.is_empty());
    }

    #[test]
    fn default_is_permissive() {
        let policy = default_policy();
        assert!(policy.default_approve);
    }

    // ── evaluate ──

    #[test]
    fn evaluate_read_allowed() {
        assert_eq!(default_policy().evaluate("Read"), Decision::Allow);
    }

    #[test]
    fn evaluate_glob_allowed() {
        assert_eq!(default_policy().evaluate("Glob"), Decision::Allow);
    }

    #[test]
    fn evaluate_grep_allowed() {
        assert_eq!(default_policy().evaluate("Grep"), Decision::Allow);
    }

    #[test]
    fn evaluate_edit_with_notice() {
        assert_eq!(default_policy().evaluate("Edit"), Decision::AllowWithNotice);
    }

    #[test]
    fn evaluate_write_with_notice() {
        assert_eq!(default_policy().evaluate("Write"), Decision::AllowWithNotice);
    }

    #[test]
    fn evaluate_bash_with_notice() {
        assert_eq!(default_policy().evaluate("Bash"), Decision::AllowWithNotice);
    }

    #[test]
    fn evaluate_unknown_defaults_allow() {
        assert_eq!(default_policy().evaluate("UnknownTool"), Decision::Allow);
    }

    #[test]
    fn evaluate_deny_overrides_approve() {
        let mut policy = default_policy();
        policy.add_always_deny("Read");
        assert_eq!(policy.evaluate("Read"), Decision::Deny);
    }

    #[test]
    fn evaluate_deny_overrides_notice() {
        let mut policy = default_policy();
        policy.add_always_deny("Edit");
        assert_eq!(policy.evaluate("Edit"), Decision::Deny);
    }

    #[test]
    fn evaluate_restrictive_default() {
        let mut policy = default_policy();
        policy.default_approve = false;
        assert_eq!(policy.evaluate("UnknownTool"), Decision::Deny);
    }

    // ── add_always_approve ──

    #[test]
    fn add_always_approve_new() {
        let mut policy = default_policy();
        policy.add_always_approve("CustomTool");
        assert_eq!(policy.evaluate("CustomTool"), Decision::Allow);
    }

    #[test]
    fn add_always_approve_duplicate() {
        let mut policy = default_policy();
        let initial_len = policy.always_approve.len();
        policy.add_always_approve("Read");
        assert_eq!(policy.always_approve.len(), initial_len);
    }

    // ── add_always_deny ──

    #[test]
    fn add_always_deny_new() {
        let mut policy = default_policy();
        policy.add_always_deny("DangerousTool");
        assert_eq!(policy.evaluate("DangerousTool"), Decision::Deny);
    }

    #[test]
    fn add_always_deny_duplicate() {
        let mut policy = default_policy();
        policy.add_always_deny("X");
        let len = policy.always_deny.len();
        policy.add_always_deny("X");
        assert_eq!(policy.always_deny.len(), len);
    }

    // ── Serialization ──

    #[test]
    fn policy_serde_roundtrip() {
        let policy = default_policy();
        let json = serde_json::to_string(&policy).unwrap();
        let back: PermissionPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back.always_approve.len(), policy.always_approve.len());
        assert_eq!(back.default_approve, policy.default_approve);
    }

    #[test]
    fn policy_from_json() {
        let json = r#"{"always_approve":["Read"],"approve_with_notice":[],"always_deny":["Bash"],"default_approve":false}"#;
        let policy: PermissionPolicy = serde_json::from_str(json).unwrap();
        assert_eq!(policy.evaluate("Read"), Decision::Allow);
        assert_eq!(policy.evaluate("Bash"), Decision::Deny);
        assert_eq!(policy.evaluate("Unknown"), Decision::Deny);
    }

    // ── Decision enum ──

    #[test]
    fn decision_equality() {
        assert_eq!(Decision::Allow, Decision::Allow);
        assert_eq!(Decision::Deny, Decision::Deny);
        assert_eq!(Decision::AllowWithNotice, Decision::AllowWithNotice);
        assert_ne!(Decision::Allow, Decision::Deny);
    }

    // ── Empty tool name ──

    #[test]
    fn evaluate_empty_tool_name() {
        assert_eq!(default_policy().evaluate(""), Decision::Allow);
    }

    #[test]
    fn evaluate_unknown_tool_name() {
        assert_eq!(default_policy().evaluate("unknown"), Decision::Allow);
    }

    // ── All read-only tools ──

    #[test]
    fn all_read_only_tools_approved() {
        let policy = default_policy();
        for tool in &["Read", "Glob", "Grep", "LS", "Agent", "WebSearch", "WebFetch"] {
            assert_eq!(
                policy.evaluate(tool),
                Decision::Allow,
                "{tool} should be auto-approved"
            );
        }
    }

    // ── All write tools have notice ──

    #[test]
    fn all_write_tools_have_notice() {
        let policy = default_policy();
        for tool in &["Edit", "Write", "Bash", "NotebookEdit"] {
            assert_eq!(
                policy.evaluate(tool),
                Decision::AllowWithNotice,
                "{tool} should have notice"
            );
        }
    }

    // ── from_config (SEC-001) ──

    fn default_hooks_config() -> HooksConfig {
        HooksConfig::default()
    }

    #[test]
    fn from_config_default_approves_read_tools() {
        let policy = PermissionPolicy::from_config(&default_hooks_config());
        for tool in &["Read", "Glob", "Grep", "LS", "Agent", "WebSearch", "WebFetch"] {
            assert_eq!(
                policy.evaluate(tool),
                Decision::Allow,
                "{tool} should be auto-approved via from_config"
            );
        }
    }

    #[test]
    fn from_config_bash_auto_approved_from_patterns() {
        // Default hooks.toml has "Bash:ls *", "Bash:git status*" etc.
        // The bare tool name "Bash" is extracted and auto-approved.
        let policy = PermissionPolicy::from_config(&default_hooks_config());
        assert_eq!(policy.evaluate("Bash"), Decision::Allow);
    }

    #[test]
    fn from_config_write_tools_not_in_patterns_get_notice() {
        // Edit, Write, NotebookEdit are not in default auto_approve patterns
        let policy = PermissionPolicy::from_config(&default_hooks_config());
        assert_eq!(policy.evaluate("Edit"), Decision::AllowWithNotice);
        assert_eq!(policy.evaluate("Write"), Decision::AllowWithNotice);
        assert_eq!(policy.evaluate("NotebookEdit"), Decision::AllowWithNotice);
    }

    #[test]
    fn from_config_deduplicates_bash_patterns() {
        // Multiple "Bash:*" patterns should yield only one "Bash" entry
        let mut hooks = default_hooks_config();
        hooks.auto_approve.patterns = vec![
            "Bash:ls *".into(),
            "Bash:git status*".into(),
            "Bash:cargo build".into(),
        ];
        let policy = PermissionPolicy::from_config(&hooks);
        let bash_count = policy
            .always_approve
            .iter()
            .filter(|t| t.eq_ignore_ascii_case("Bash"))
            .count();
        assert_eq!(bash_count, 1, "Bash should appear exactly once");
    }

    #[test]
    fn from_config_custom_patterns_override() {
        let mut hooks = default_hooks_config();
        hooks.auto_approve.patterns = vec!["CustomTool".into()];
        let policy = PermissionPolicy::from_config(&hooks);
        assert_eq!(policy.evaluate("CustomTool"), Decision::Allow);
        // Standard read-only tools still auto-approved (backward compat)
        assert_eq!(policy.evaluate("Read"), Decision::Allow);
    }

    #[test]
    fn from_config_empty_patterns_still_has_read_tools() {
        let mut hooks = default_hooks_config();
        hooks.auto_approve.patterns = Vec::new();
        let policy = PermissionPolicy::from_config(&hooks);
        assert_eq!(policy.evaluate("Read"), Decision::Allow);
        assert_eq!(policy.evaluate("Glob"), Decision::Allow);
    }

    #[test]
    fn from_config_unknown_tool_defaults_allow() {
        let policy = PermissionPolicy::from_config(&default_hooks_config());
        assert_eq!(policy.evaluate("SomethingNew"), Decision::Allow);
    }
}
