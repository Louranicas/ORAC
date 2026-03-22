# L3 Hooks — HTTP Hook Server (Keystone Layer)

> THE KEYSTONE. Replaces all 8 bash hook scripts with sub-ms HTTP endpoints.
> Claude Code sends HTTP POST to `http://localhost:8133/hooks/{event}`.

## Feature Gate

`api`

## Modules

| Module | File | Description | Test Kind |
|--------|------|-------------|-----------|
| m10_hook_server | `src/m3_hooks/m10_hook_server.rs` | Axum HTTP server on `:8133`, route registration for 6 hook endpoints, graceful shutdown | integration |
| m11_session_hooks | `src/m3_hooks/m11_session_hooks.rs` | `SessionStart` (register sphere on PV2) and `Stop` (quality gate + deregister) handlers | unit + integration |
| m12_tool_hooks | `src/m3_hooks/m12_tool_hooks.rs` | `PostToolUse` (Hebbian STDP + task poll + memory) and `PreToolUse` (thermal gate) handlers | unit + integration |
| m13_prompt_hooks | `src/m3_hooks/m13_prompt_hooks.rs` | `UserPromptSubmit` (inject field state r/spheres/K into prompt context) handler | unit |
| m14_permission_policy | `src/m3_hooks/m14_permission_policy.rs` | `PermissionRequest` auto-approve/deny policy engine (sphere→fleet→default cascade) | unit + integration |

## Hook Endpoints (6)

| Event | Endpoint | Action | Response |
|-------|----------|--------|----------|
| SessionStart | `POST /hooks/SessionStart` | Register sphere on PV2 | `{decision: "approve", inject: field_state}` |
| PreToolUse | `POST /hooks/PreToolUse` | Thermal gate via SYNTHEX | `{decision: "approve"/"deny"}` — fails OPEN |
| PostToolUse | `POST /hooks/PostToolUse` | Hebbian STDP + task poll | `{decision: "approve"}` |
| UserPromptSubmit | `POST /hooks/UserPromptSubmit` | Inject field state | `{decision: "approve", inject: context}` |
| Stop | `POST /hooks/Stop` | Quality gate + deregister | `{decision: "approve"}` |
| PermissionRequest | `POST /hooks/PermissionRequest` | Auto-approve/deny policy | `{decision: "approve"/"deny", reason}` |

## Dependencies

- **L1 Core** — `OracError`, `PvConfig`, types, validation
- **L2 Wire** — IPC client for PV2 sphere registration, status updates, task polling

## Design Constraints

- All handlers return within 1ms (local memory, no external I/O in hot path)
- PreToolUse thermal gate fails OPEN if SYNTHEX bridge is down (AP18)
- Permission policy cascade: sphere-specific → fleet-wide → default (P15)
- Hook handlers must be idempotent — same `hook_id` twice = same result (P16)
- Body size limit: 64KB per request

## Hot-Swap Source

- ALL NEW (no PV2 equivalent — PV2 uses bash hook scripts)

## Cross-References

- [[Session 050 — ORAC Sidecar Architecture]] §HTTP Hook Server
- [[Session 045 Arena — 02-api-wiring-map]]
- [[Consent Flow Analysis]]
- `.claude/schemas/hook_request.json` (6 events)
- `.claude/schemas/hook_response.json` (approve/deny/skip)
- `.claude/schemas/permission_policy.schema.json`
- ORAC_PLAN.md §Phase 1 Detail (steps 4-12)
- ORAC_MINDMAP.md §Branch 1 (HTTP Hook Server)
