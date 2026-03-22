# ORAC HTTP API Specification

> REST API on port 8133. All responses are JSON. All timestamps are `u64` milliseconds.

## Base URL

```
http://localhost:8133
```

## Endpoints

### GET /health

Health check for ORAC and all bridges.

```json
// Response 200
{
  "status": "ok",
  "uptime_s": 3600,
  "version": "0.1.0",
  "bridges": {
    "synthex": {"status": "ok", "latency_ms": 2},
    "me": {"status": "ok", "latency_ms": 1},
    "povm": {"status": "ok", "latency_ms": 3},
    "rm": {"status": "ok", "latency_ms": 1}
  },
  "pv2_bus": {"connected": true, "sphere_count": 4},
  "evolution": {"active": true, "generation": 42}
}
```

---

### POST /hooks/{event}

Claude Code hook handlers. See [`HOOKS_SPEC.md`](HOOKS_SPEC.md) for full schemas.

| Event | Path |
|-------|------|
| SessionStart | `/hooks/session_start` |
| PreToolUse | `/hooks/pre_tool_use` |
| PostToolUse | `/hooks/post_tool_use` |
| UserPromptSubmit | `/hooks/user_prompt_submit` |
| PermissionRequest | `/hooks/permission_request` |
| Stop | `/hooks/stop` |

---

### GET /metrics

Prometheus-format metrics.

```
Content-Type: text/plain; version=0.0.4

# HELP orac_hook_duration_ms Hook processing latency
# TYPE orac_hook_duration_ms histogram
orac_hook_duration_ms_bucket{hook="pre_tool_use",le="0.5"} 142
orac_hook_duration_ms_bucket{hook="pre_tool_use",le="1.0"} 198
orac_hook_duration_ms_bucket{hook="pre_tool_use",le="5.0"} 200

# HELP orac_sessions_active Active session count
# TYPE orac_sessions_active gauge
orac_sessions_active 4

# HELP orac_tool_calls_total Tool calls processed
# TYPE orac_tool_calls_total counter
orac_tool_calls_total{tool="Read"} 342
orac_tool_calls_total{tool="Bash"} 128

# HELP orac_bridge_errors_total Bridge request failures
# TYPE orac_bridge_errors_total counter
orac_bridge_errors_total{bridge="synthex"} 0
orac_bridge_errors_total{bridge="rm"} 2

# HELP orac_field_order_parameter Kuramoto order parameter r
# TYPE orac_field_order_parameter gauge
orac_field_order_parameter 0.997

# HELP orac_circuit_breaker_state Circuit breaker state (0=closed,1=open,2=half_open)
# TYPE orac_circuit_breaker_state gauge
orac_circuit_breaker_state{bridge="synthex"} 0
```

---

### GET /field

Current Kuramoto field state.

```json
// Response 200
{
  "r": 0.997,
  "K": 2.42,
  "effective_K": 1.11,
  "tick": 14200,
  "sphere_count": 4,
  "spheres": [
    {
      "id": "uuid",
      "phase": 1.57,
      "natural_freq": 0.1,
      "status": "working",
      "k_mod": 1.2,
      "activation": 0.85
    }
  ],
  "chimera_detected": false,
  "ghosts": 2
}
```

---

### GET /blackboard

Fleet state queries. Read-only view of the shared blackboard.

```json
// Response 200
{
  "sessions": [
    {
      "session_id": "uuid",
      "sphere_id": "uuid",
      "cwd": "/home/user/project",
      "status": "working",
      "started_ms": 1742600000000,
      "tool_count": 42,
      "last_tool": "Edit",
      "last_active_ms": 1742600300000
    }
  ],
  "fleet_size": 4,
  "sync_quality": "high"
}
```

Query parameters:
- `?status=working` — filter by status
- `?cwd=/path` — filter by working directory
- `?since=1742600000000` — sessions started after timestamp

---

### GET /consent/{sphere_id}

Get consent declarations for a sphere.

```json
// Response 200
{
  "sphere_id": "uuid",
  "consents": {
    "synthex_write": true,
    "povm_read": true,
    "povm_write": false,
    "hydration": true
  },
  "updated_ms": 1742600000000
}
```

### PUT /consent/{sphere_id}

Update consent declarations.

```json
// Request
{
  "synthex_write": true,
  "povm_write": true
}

// Response 200
{
  "status": "ok",
  "updated": ["synthex_write", "povm_write"]
}
```

---

### GET /field/ghosts

Ghost traces of deregistered spheres (FIFO, max 20).

```json
// Response 200
{
  "ghosts": [
    {
      "sphere_id": "uuid",
      "persona": "/home/user/project",
      "deregistered_ms": 1742600000000,
      "final_phase": 3.14,
      "total_tools": 87,
      "session_duration_ms": 1800000
    }
  ]
}
```

## Error Responses

All errors use standard format:

```json
{
  "error": "description",
  "code": 4005
}
```

| HTTP Status | When |
|-------------|------|
| 400 | Malformed request body |
| 404 | Unknown sphere_id or endpoint |
| 429 | Rate limited |
| 500 | Internal error |
| 503 | Bridge unavailable (circuit breaker open) |
