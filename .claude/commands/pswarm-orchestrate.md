# /pswarm-orchestrate — Submit Orchestration Task

Submit a task to Prometheus Swarm for multi-agent orchestration.

## Arguments
- `$ARGUMENTS` — The task description to orchestrate

## Usage
```
/pswarm-orchestrate Review the authentication module for security vulnerabilities
/pswarm-orchestrate Full code audit of orac-sidecar src/m3_hooks/
```

## Action

1. Parse the task description from arguments
2. Register a temporary agent if none exist
3. Submit via `pswarm-ctl orchestrate`
4. Report task ID and dispatch status

```bash
TASK="$ARGUMENTS"
if [ -z "$TASK" ]; then
  echo "Usage: /pswarm-orchestrate <task description>"
  exit 0
fi

# Ensure at least one agent exists
AGENT_COUNT=$(pswarm-ctl status 2>/dev/null | python3 -c "import sys,json;print(json.load(sys.stdin).get('agents',0))")
if [ "$AGENT_COUNT" = "0" ]; then
  pswarm-ctl register auto-agent cc_subagent rust testing security 2>/dev/null
  echo "Auto-registered agent: auto-agent"
fi

echo "Orchestrating: $TASK"
pswarm-ctl orchestrate "$TASK" 2>/dev/null | python3 -m json.tool
```
