# ORAC Hook Server Specification

> Claude Code hook endpoints. Each hook intercepts a lifecycle event, applies filters/transforms, and fires IPC actions on the PV2 bus.

## Overview

ORAC registers as a Claude Code hook server. Claude Code sends HTTP POST requests to ORAC on each lifecycle event. ORAC processes the hook synchronously and returns a response within the latency budget.

**Latency target**: <1ms per hook (all hooks are on the critical path).

## Hook Endpoints

### 1. SessionStart

| Field | Value |
|-------|-------|
| Endpoint | `POST /hooks/session_start` |
| Trigger | Claude Code session begins |
| Latency | <1ms |

**Request schema:**
```json
{
  "session_id": "string (UUID)",
  "cwd": "string (absolute path)",
  "timestamp_ms": "u64",
  "model": "string (e.g. claude-opus-4-6)"
}
```

**Response schema:**
```json
{
  "status": "ok",
  "sphere_id": "string (assigned sphere UUID)"
}
```

**Side effects:**
- Register sphere on PV2 bus (`ClientFrame::Hello`)
- Initialize session entry in blackboard
- Start metrics collection for this session

**IPC wiring:**
- Sends `ClientFrame::Hello { sphere_id, persona: cwd }` to PV2 bus
- Subscribes to `field.*` and `task.*` events for this sphere

---

### 2. PreToolUse

| Field | Value |
|-------|-------|
| Endpoint | `POST /hooks/pre_tool_use` |
| Trigger | Before Claude executes a tool call |
| Latency | <1ms |

**Request schema:**
```json
{
  "session_id": "string",
  "tool_name": "string (max 128 chars)",
  "tool_input": "object (tool parameters)",
  "timestamp_ms": "u64"
}
```

**Response schema:**
```json
{
  "action": "allow | deny | modify",
  "modified_input": "object | null",
  "reason": "string | null"
}
```

**Side effects:**
- Update sphere phase (tool_name -> semantic phase mapping)
- Check consent flags for this tool
- Increment tool use counter in Hebbian STDP tracker

**IPC wiring:**
- Sends `ClientFrame::ToolPhase { sphere_id, tool_name, phase }` to PV2 bus
- If consent check fails: returns `deny` with reason, sends `ClientFrame::ConsentDenied`

---

### 3. PostToolUse

| Field | Value |
|-------|-------|
| Endpoint | `POST /hooks/post_tool_use` |
| Trigger | After Claude tool call completes |
| Latency | <1ms |

**Request schema:**
```json
{
  "session_id": "string",
  "tool_name": "string",
  "tool_result": "string (truncated to 4096 chars)",
  "duration_ms": "u64",
  "success": "bool",
  "timestamp_ms": "u64"
}
```

**Response schema:**
```json
{
  "status": "ok"
}
```

**Side effects:**
- Update Hebbian weight for tool chain (previous_tool -> current_tool)
- Record duration in metrics histogram
- If `!success`: increment circuit breaker failure count for upstream
- Write tool event to reasoning memory (RM bridge, TSV format)

**IPC wiring:**
- Sends `ClientFrame::Activity { sphere_id, tool_name, duration_ms }` to PV2 bus
- Triggers STDP weight update via `ClientFrame::HebbianPulse { pre, post, delta_t }`

---

### 4. UserPromptSubmit

| Field | Value |
|-------|-------|
| Endpoint | `POST /hooks/user_prompt_submit` |
| Trigger | User submits a prompt |
| Latency | <1ms |

**Request schema:**
```json
{
  "session_id": "string",
  "prompt_length": "usize",
  "timestamp_ms": "u64"
}
```

**Response schema:**
```json
{
  "status": "ok",
  "context_injection": "string | null"
}
```

**Side effects:**
- Reset idle timer for this sphere
- Optionally inject field context (if sphere is drifting or chimera detected)
- Log prompt event to session timeline

**IPC wiring:**
- Sends `ClientFrame::StatusUpdate { sphere_id, status: "working" }` to PV2 bus
- If chimera detected: includes field summary in `context_injection`

---

### 5. PermissionRequest

| Field | Value |
|-------|-------|
| Endpoint | `POST /hooks/permission_request` |
| Trigger | Claude requests permission for a sensitive action |
| Latency | <1ms |

**Request schema:**
```json
{
  "session_id": "string",
  "permission_type": "string (e.g. file_write, bash_exec, network)",
  "resource": "string (path or URL)",
  "timestamp_ms": "u64"
}
```

**Response schema:**
```json
{
  "action": "allow | deny",
  "reason": "string | null"
}
```

**Side effects:**
- Check consent registry for this sphere + permission type
- Log permission decision to audit trail
- If denied: increment policy violation counter

**IPC wiring:**
- Sends `ClientFrame::ConsentQuery { sphere_id, permission_type, resource }` to PV2 bus
- Waits for `ServerFrame::ConsentResponse` (timeout: 500ms, default: deny)

---

### 6. Stop

| Field | Value |
|-------|-------|
| Endpoint | `POST /hooks/stop` |
| Trigger | Claude Code session ends |
| Latency | <1ms |

**Request schema:**
```json
{
  "session_id": "string",
  "reason": "string (user_exit | error | timeout | compact)",
  "timestamp_ms": "u64"
}
```

**Response schema:**
```json
{
  "status": "ok"
}
```

**Side effects:**
- Deregister sphere from PV2 bus (leave ghost trace)
- Flush session metrics to RM bridge
- Snapshot Hebbian weights for this sphere
- If reason is `compact`: trigger cascade handoff (write brief to shared-context)

**IPC wiring:**
- Sends `ClientFrame::Goodbye { sphere_id }` to PV2 bus
- Ghost trace created automatically by PV2 (FIFO max 20)

## Semantic Phase Mapping

Tool names map to oscillator phases for Kuramoto coupling:

| Tool Category | Phase Region | Examples |
|---------------|-------------|----------|
| Read | 0 | Read, Glob, Grep |
| Write | pi/2 | Edit, Write |
| Execute | pi | Bash, Skill |
| Communicate | 3*pi/2 | WebFetch, mcp__* |

## Error Handling

- All hooks return HTTP 200 even on internal errors (Claude Code expects 2xx)
- Errors are logged and surfaced via `/metrics` endpoint
- If PV2 bus is unreachable: hooks degrade gracefully (no IPC, local-only processing)
- Hook timeout: if processing exceeds 5ms, return default response and log warning
