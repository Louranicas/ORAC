---
name: bridge-probe
description: Probe ORAC bridge health and connectivity to SYNTHEX, ME, POVM, RM, and PV2. Checks port availability, endpoint responses, data format compliance, and circuit breaker state. Use when bridges are failing, verifying bridge wiring, or diagnosing service connectivity issues.
allowed-tools:
  - Bash
  - Read
---

# /bridge-probe — ORAC Bridge Health Probe

Diagnose connectivity between ORAC and its 5 upstream services.

## Quick Probe (all bridges)

```bash
echo "=== ORAC Bridge Probe ==="

# PV2 (primary — IPC bus)
PV2=$(curl -s localhost:8132/health 2>/dev/null)
echo "PV2:8132 — $(echo $PV2 | jq -c '{status,r:.r,tick,spheres}' 2>/dev/null || echo 'DOWN')"
echo "  Socket: $(test -S /run/user/1000/pane-vortex-bus.sock && echo 'EXISTS' || echo 'MISSING')"

# SYNTHEX (thermal gate)
SX=$(curl -s -o /dev/null -w '%{http_code}' localhost:8090/api/health 2>/dev/null)
echo "SYNTHEX:8090 — HTTP $SX"
test "$SX" = "200" && curl -s localhost:8090/v3/thermal 2>/dev/null | jq -c '{temperature,target,pid_active}' 2>/dev/null

# ME (fitness signal)
ME=$(curl -s -o /dev/null -w '%{http_code}' localhost:8080/api/health 2>/dev/null)
echo "ME:8080 — HTTP $ME"
test "$ME" = "200" && curl -s localhost:8080/api/observer 2>/dev/null | jq -c '{fitness:.last_report.current_fitness,ralph_cycles:.ralph_cycles}' 2>/dev/null

# POVM (memory hydration)
POVM=$(curl -s -o /dev/null -w '%{http_code}' localhost:8125/health 2>/dev/null)
echo "POVM:8125 — HTTP $POVM"
test "$POVM" = "200" && curl -s localhost:8125/hydrate 2>/dev/null | jq -c '{memory_count,pathway_count}' 2>/dev/null

# RM (TSV persistence — NOT JSON!)
RM=$(curl -s -o /dev/null -w '%{http_code}' localhost:8130/health 2>/dev/null)
echo "RM:8130 — HTTP $RM (TSV format!)"
test "$RM" = "200" && echo "  Entries: $(curl -s localhost:8130/entries 2>/dev/null | wc -l)"
```

## Deep Probe: SYNTHEX Thermal

```bash
# Thermal state (critical for PreToolUse gate)
curl -s localhost:8090/v3/thermal | jq '{
  temperature: .temperature,
  target: .target,
  pid_active: .pid_active,
  heat_sources: .heat_sources,
  cool_down_rate: .cool_down_rate
}'

# Diagnostics
curl -s localhost:8090/v3/diagnostics | jq '{
  breaker_state: .circuit_breaker.state,
  cascade_amp: .cascade.amplification,
  homeostasis: .homeostasis.active
}'
```

## Deep Probe: PV2 IPC Bus

```bash
# Bus info
curl -s localhost:8132/bus/info | jq .
# Active tasks
curl -s localhost:8132/bus/tasks | jq '.[] | {id,status,source:.source_sphere,target:.target_type}'
# Recent events
curl -s localhost:8132/bus/events | jq '.[0:5]'
# Coupling matrix (Hebbian differentiation)
curl -s localhost:8132/coupling/matrix | jq '[.matrix[].weight] | {min: min, max: max, unique: unique | length}'
```

## Circuit Breaker State

```bash
# ORAC circuit breaker state (when implemented)
curl -s localhost:8133/circuit-breakers | jq '.[] | {bridge,state,failures,next_retry}'
```

## Bridge Staleness Check

```bash
# Check each bridge hasn't gone stale
for service in "8090:SYNTHEX:/api/health" "8080:ME:/api/health" "8125:POVM:/health" "8130:RM:/health" "8132:PV2:/health"; do
  IFS=: read -r port name path <<< "$service"
  CODE=$(curl -s -o /dev/null -w '%{http_code}' -m 2 "localhost:$port$path" 2>/dev/null)
  if [ "$CODE" = "200" ]; then
    echo "$name:$port — HEALTHY"
  elif [ "$CODE" = "000" ]; then
    echo "$name:$port — UNREACHABLE (service down?)"
  else
    echo "$name:$port — HTTP $CODE (unexpected)"
  fi
done
```

## TRAPS

- **BUG-033**: Bridge URLs must NOT include `http://` prefix — raw SocketAddr only
- **BUG-034**: POVM is write-only — must call `/hydrate` to read state
- **RM**: TSV format ONLY — never send JSON
- **SYNTHEX**: Uses `/api/health` not `/health`
- **ME**: Uses `/api/health` not `/health`
- **Thermal gate**: Fails OPEN if SYNTHEX down (AP18)
