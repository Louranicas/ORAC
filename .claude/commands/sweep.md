# /sweep — Habitat Health Sweep (17 Services + ORAC + Field + Thermal)

Probe every service and subsystem in parallel. Report health, RALPH state, field coherence, thermal, STDP, and emergence.

topology: convergent

```bash
python3 -c "
import json, urllib.request as u, time
from concurrent.futures import ThreadPoolExecutor

start = time.monotonic()

ports = {
    8080: ('/api/health', 'ME'),
    8081: ('/health', 'DevOps'),
    8090: ('/api/health', 'SYNTHEX'),
    8100: ('/health', 'K7'),
    8101: ('/health', 'NAIS'),
    8102: ('/health', 'Bash'),
    8103: ('/health', 'TM'),
    8104: ('/health', 'CCM'),
    8105: ('/health', 'TL'),
    8110: ('/health', 'CSV7'),
    8120: ('/health', 'VMS'),
    8125: ('/health', 'POVM'),
    8130: ('/health', 'RM'),
    8132: ('/health', 'PV2'),
    8133: ('/health', 'ORAC'),
    9001: ('/health', 'Arch'),
    10001: ('/health', 'Prom'),
}

def check(args):
    port, path, name = args
    try:
        r = u.urlopen(f'http://localhost:{port}{path}', timeout=3)
        return (port, name, r.status, json.loads(r.read()))
    except Exception as e:
        return (port, name, 0, {})

with ThreadPoolExecutor(max_workers=17) as pool:
    results = list(pool.map(check, [(p, path, name) for p, (path, name) in ports.items()]))

svc_ms = (time.monotonic() - start) * 1000

print(f'━━━ SERVICES ({svc_ms:.0f}ms parallel) ━━━')
ok = 0
for port, name, status, data in sorted(results):
    if status == 200:
        ok += 1
        print(f'  {name}:{port} ✓')
    else:
        print(f'  {name}:{port} ✗ ({status})')
print(f'  TOTAL: {ok}/17')

# Extract ORAC data from results
orac = next((d for p, n, s, d in results if p == 8133 and s == 200), {})
pv2 = next((d for p, n, s, d in results if p == 8132 and s == 200), {})

print()
print('━━━ ORAC STATE ━━━')
for k in ['ralph_gen','ralph_fitness','ralph_phase','field_r','emergence_events','coupling_connections','coupling_weight_mean','hebbian_ltp_total','hebbian_ltd_total','sessions','ipc_state']:
    print(f'  {k}: {orac.get(k, \"?\")}')

print()
print('━━━ THERMAL ━━━')
sx = next((d for p, n, s, d in results if p == 8090 and s == 200), {})
if sx:
    print(f'  temp={sx.get(\"temperature\",0):.4f} target={sx.get(\"target\",0):.4f}')

print()
print('━━━ FIELD ━━━')
if pv2:
    print(f'  r={pv2.get(\"r\",0):.4f} spheres={pv2.get(\"spheres\",\"?\")} K={pv2.get(\"k\",0):.3f} mode={pv2.get(\"fleet_mode\",\"?\")}')

total_ms = (time.monotonic() - start) * 1000
print(f'\n━━━ {total_ms:.0f}ms total ━━━')
"
```
