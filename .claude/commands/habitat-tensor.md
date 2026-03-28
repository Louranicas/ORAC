# /habitat-tensor -- 125ms 6-Layer Parallel Chain

Captures 24 data points across 6 semantic layers simultaneously via ThreadPoolExecutor.
The most powerful monitoring pattern discovered in Session 066.

```bash
python3 -c "
import json, urllib.request as u, subprocess, time
from concurrent.futures import ThreadPoolExecutor

start = time.monotonic()

def http(url):
    try: return json.loads(u.urlopen(url, timeout=2).read())
    except: return {}

def rg_count(pattern, path):
    try:
        r = subprocess.run(['rg', '--count', pattern, path], capture_output=True, text=True, timeout=5)
        return sum(int(l.split(':')[-1]) for l in r.stdout.strip().split('\n') if ':' in l)
    except: return 0

# Layer 1: Substrate (11 HTTP endpoints)
endpoints = {
    'pv2': 'http://localhost:8132/health',
    'orac': 'http://localhost:8133/health',
    'me': 'http://localhost:8080/api/observer',
    'sx_thermal': 'http://localhost:8090/v3/thermal',
    'povm': 'http://localhost:8125/memories?limit=5',
    'rm': 'http://localhost:8130/health',
    'k7': 'http://localhost:8100/health',
    'vms': 'http://localhost:8120/health',
}

with ThreadPoolExecutor(max_workers=12) as pool:
    futures = {k: pool.submit(http, v) for k, v in endpoints.items()}
    results = {k: f.result() for k, f in futures.items()}

# Extract key metrics
pv = results.get('pv2', {})
oc = results.get('orac', {})
me = results.get('me', {})
sx = results.get('sx_thermal', {})

r = pv.get('r', 0)
gen = oc.get('ralph_gen', 0)
fit = oc.get('ralph_fitness', 0)
ltp = oc.get('hebbian_ltp_total', 0)
ltd = oc.get('hebbian_ltd_total', 0)
em = oc.get('emergence_events', 0)
me_fit = me.get('last_report', me).get('current_fitness', 0)
temp = sx.get('temperature', 0)
sph = pv.get('spheres', 0)

# Composites
metabolic = me_fit * fit * r if (me_fit and fit and r) else 0
ratio = ltp / max(ltd, 1)

ms = (time.monotonic() - start) * 1000

print(f'=== Habitat Tensor ({ms:.0f}ms) ===')
print(f'Field:     r={r:.4f} spheres={sph} K={pv.get(\"k\",0):.3f}')
print(f'RALPH:     gen={gen} fitness={fit:.4f} emergence={em}')
print(f'Hebbian:   LTP={ltp} LTD={ltd} ratio={ratio:.4f}')
print(f'ME:        fitness={me_fit:.4f}')
print(f'Thermal:   temp={temp:.4f} target={sx.get(\"target\",0):.4f}')
print(f'Metabolic: {metabolic:.4f} (ME*ORAC*PV2)')
print(f'Services:  {sum(1 for v in results.values() if v)}/{len(endpoints)} responding')
"
```
