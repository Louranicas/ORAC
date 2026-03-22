---
name: hook-debug
user-invocable: true
description: Debug and test ORAC HTTP hook endpoints. Sends test payloads to all 6 hook events, inspects responses, checks latency, and verifies hook-to-PV2 wiring. Use when hook server is misbehaving, testing new hook logic, verifying permission policy, or checking thermal gate behavior.
argument-hint: [event-name|all|latency]
---

# /hook-debug — ORAC Hook Endpoint Debugger

Test and debug all 6 ORAC HTTP hook endpoints.

## Quick Test (all endpoints)

```bash
ORAC=http://localhost:8133
SESSION="debug-$(date +%s)"

# SessionStart — should register sphere
curl -s -X POST "$ORAC/hooks/SessionStart" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION\",\"event\":\"SessionStart\",\"hook_id\":\"h1\"}" | jq .

# PreToolUse — thermal gate (should approve if SYNTHEX healthy or down)
curl -s -X POST "$ORAC/hooks/PreToolUse" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION\",\"event\":\"PreToolUse\",\"hook_id\":\"h2\",\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"ls\"}}" | jq .

# PostToolUse — Hebbian + memory
curl -s -X POST "$ORAC/hooks/PostToolUse" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION\",\"event\":\"PostToolUse\",\"hook_id\":\"h3\",\"tool_name\":\"Read\",\"tool_input\":{\"file_path\":\"/tmp/test\"},\"tool_output\":{}}" | jq .

# UserPromptSubmit — field state injection
curl -s -X POST "$ORAC/hooks/UserPromptSubmit" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION\",\"event\":\"UserPromptSubmit\",\"hook_id\":\"h4\",\"prompt\":\"test prompt\"}" | jq .

# PermissionRequest — auto-approve policy
curl -s -X POST "$ORAC/hooks/PermissionRequest" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION\",\"event\":\"PermissionRequest\",\"hook_id\":\"h5\",\"permission\":{\"tool\":\"Bash\",\"action\":\"execute\",\"target\":\"ls -la\"}}" | jq .

# Stop — deregister + quality gate
curl -s -X POST "$ORAC/hooks/Stop" \
  -H "Content-Type: application/json" \
  -d "{\"session_id\":\"$SESSION\",\"event\":\"Stop\",\"hook_id\":\"h6\",\"stop_reason\":\"completed\"}" | jq .
```

## Latency Check

```bash
ORAC=http://localhost:8133
for event in SessionStart PreToolUse PostToolUse UserPromptSubmit Stop PermissionRequest; do
  TIME=$(curl -s -o /dev/null -w '%{time_total}' -X POST "$ORAC/hooks/$event" \
    -H "Content-Type: application/json" \
    -d "{\"session_id\":\"latency-test\",\"event\":\"$event\",\"hook_id\":\"lt-$event\"}" 2>/dev/null)
  echo "$event: ${TIME}s"
done
```

**Target:** All responses <1ms (0.001s). If >5ms, check for blocking calls in handler.

## Hook Event DB (after events fire)

```bash
sqlite3 -header -column data/hook_events.db \
  "SELECT event_type, COUNT(*) as count, AVG(latency_us) as avg_us FROM hook_events GROUP BY event_type;"
```

## Verify PV2 Wiring

```bash
# Check sphere was registered by SessionStart hook
curl -s localhost:8132/spheres | jq '.[] | select(.sphere_id | contains("debug"))'
# Check memory was recorded by PostToolUse hook
curl -s localhost:8132/sphere/debug-session/memories | jq '.[0]'
```

## Expected Responses

| Event | decision | inject | notes |
|-------|----------|--------|-------|
| SessionStart | approve | field state | Registers sphere on PV2 |
| PreToolUse | approve/deny | — | Deny only if thermal threshold exceeded |
| PostToolUse | approve | — | Records Hebbian co-activation |
| UserPromptSubmit | approve | field context | Injects r, spheres, K into prompt |
| Stop | approve | — | Deregisters sphere, runs quality gate |
| PermissionRequest | approve/deny | reason | Policy cascade: sphere→fleet→default |
