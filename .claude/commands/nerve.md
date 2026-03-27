# /nerve — Habitat Nerve Center (Continuous Dashboard)

Live dashboard polling all critical subsystems every 10 seconds. Replaces 24+ ad-hoc watch loop reinventions discovered in atuin history.

Run this in a dedicated pane (Tab 2 Workspace or fleet pane).

```bash
watch -n 10 -t bash -c '
echo "═══ HABITAT NERVE CENTER ═══"
echo ""

# ORAC
O=$(curl -s -m 2 localhost:8133/health 2>/dev/null)
echo "ORAC: gen=$(echo $O | jq -r ".ralph_gen") fit=$(echo $O | jq -r ".ralph_fitness" | head -c 6) phase=$(echo $O | jq -r ".ralph_phase") sessions=$(echo $O | jq -r ".sessions")"
echo "STDP: LTP=$(echo $O | jq -r ".hebbian_ltp_total") LTD=$(echo $O | jq -r ".hebbian_ltd_total") coupling=$(echo $O | jq -r ".coupling_connections")"
echo "Emergence: $(echo $O | jq -r ".emergence_events") events | IPC: $(echo $O | jq -r ".ipc_state")"

# Field
P=$(curl -s -m 2 localhost:8132/health 2>/dev/null)
echo ""
echo "Field: r=$(echo $P | jq -r ".r" | head -c 6) spheres=$(echo $P | jq -r ".spheres") K=$(echo $P | jq -r ".k" | head -c 5) mode=$(echo $P | jq -r ".fleet_mode")"

# Thermal
T=$(curl -s -m 2 localhost:8090/v3/thermal 2>/dev/null)
echo "Thermal: temp=$(echo $T | jq -r ".temperature" | head -c 6) target=$(echo $T | jq -r ".target") pid=$(echo $T | jq -r ".pid_output" | head -c 6)"

# ME
echo "ME: fitness=$(curl -s -m 2 localhost:8080/api/health 2>/dev/null | jq -r ".last_fitness // .fitness // \"?\"" | head -c 6)"

# Services
OK=0; for p in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
  hp="/health"; [ "$p" = "8080" ] || [ "$p" = "8090" ] && hp="/api/health"
  [ "$(curl -s -o /dev/null -w "%{http_code}" -m 1 localhost:$p$hp 2>/dev/null)" = "200" ] && OK=$((OK+1))
done
echo ""
echo "Services: $OK/17 | $(date +%H:%M:%S)"
'
```
