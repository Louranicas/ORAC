# /sweep — Habitat Health Sweep (17 Services + ORAC + Field + Thermal)

Probe every service and subsystem. Report health, RALPH state, field coherence, thermal, STDP, and emergence.

```bash
echo "━━━ SERVICES (17 ports) ━━━"
declare -A hp=([8080]="/api/health" [8090]="/api/health")
declare -A svc=([8080]="ME" [8081]="DevOps" [8090]="SYNTHEX" [8100]="K7" [8101]="NAIS" [8102]="Bash" [8103]="TM" [8104]="CCM" [8105]="TL" [8110]="CSV7" [8120]="VMS" [8125]="POVM" [8130]="RM" [8132]="PV2" [8133]="ORAC" [9001]="Arch" [10001]="Prom")
OK=0
for p in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
  path="${hp[$p]:-/health}"
  code=$(curl -s -o /dev/null -w '%{http_code}' -m 3 "localhost:$p$path" 2>/dev/null)
  [ "$code" = "200" ] && OK=$((OK+1)) && echo "  ${svc[$p]}:$p ✓" || echo "  ${svc[$p]}:$p ✗ ($code)"
done
echo "  TOTAL: $OK/17"

echo ""
echo "━━━ ORAC STATE ━━━"
curl -s localhost:8133/health 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
for k in ['ralph_gen','ralph_fitness','ralph_phase','field_r','emergence_events','coupling_connections','coupling_weight_mean','hebbian_ltp_total','hebbian_ltd_total','sessions','ipc_state']:
    v=d.get(k,'?'); print(f'  {k}: {v}')
"

echo ""
echo "━━━ THERMAL ━━━"
curl -s localhost:8090/v3/thermal 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  temp={d.get(\"temperature\",\"?\"):.4f} target={d.get(\"target\",\"?\"):.4f} pid={d.get(\"pid_output\",\"?\"):.4f}')
"

echo ""
echo "━━━ FIELD ━━━"
curl -s localhost:8132/health 2>/dev/null | python3 -c "
import sys,json;d=json.load(sys.stdin)
print(f'  r={d.get(\"r\",\"?\"):.4f} spheres={d.get(\"spheres\",\"?\")} K={d.get(\"k\",\"?\"):.3f} mode={d.get(\"fleet_mode\",\"?\")}')
"
```
