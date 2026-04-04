# /pswarm — Prometheus Swarm Orchestration Dashboard

Quick dashboard + orchestration interface for Prometheus Swarm v2.0 on port 10002.

```bash
echo "━━━ PROMETHEUS SWARM v2.0 ━━━"
pswarm-ctl status 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  Status: {d[\"status\"]} | v{d[\"version\"]} | up {d[\"uptime_secs\"]}s')
print(f'  Agents: {d[\"agents\"]} | Tasks: {d[\"active_tasks\"]} | Requests: {d[\"requests\"]}')
"

echo ""
echo "━━━ ORAC ━━━"
curl -s localhost:8133/health 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  gen={d.get(\"ralph_gen\",\"?\")} fit={d.get(\"ralph_fitness\",0):.3f} LTP={d.get(\"hebbian_ltp_total\",0)} grade={d.get(\"system_grade\",\"?\")}')
"

echo ""
echo "━━━ FIELD ━━━"
curl -s localhost:8132/health 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  r={d.get(\"r\",0):.4f} spheres={d.get(\"spheres\",0)} tick={d.get(\"tick\",0)}')
"

echo ""
echo "━━━ PERSONAS ━━━"
pswarm-ctl personas 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
for p in d.get('personas',[]):
    print(f'  {p[\"name\"]:20} model={p[\"model\"]:6} pos={p[\"workflow_position\"]}')
"

echo ""
echo "━━━ EVOLUTION ━━━"
pswarm-ctl evolution 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  gen={d.get(\"generation\",0)} fit={d.get(\"fitness\",0):.3f} phase={d.get(\"phase\",\"?\")}')
"
```
