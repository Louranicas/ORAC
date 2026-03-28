# /intel -- 17ms Habitat Pulse

Fuse PV2 + ORAC + SYNTHEX + POVM into a single status line. S-tier speed.

```bash
python3 -c "
import json, urllib.request as u, time
start = time.monotonic()
def f(url):
    try: return json.loads(u.urlopen(url, timeout=2).read())
    except: return {}
pv = f('http://localhost:8132/health')
oc = f('http://localhost:8133/health')
sx = f('http://localhost:8090/v3/thermal')
pm = f('http://localhost:8125/memories?limit=1')
ms = (time.monotonic() - start) * 1000
r = pv.get('r', 0)
gen = oc.get('ralph_gen', 0)
fit = oc.get('ralph_fitness', 0)
ltp = oc.get('hebbian_ltp_total', 0)
ltd = oc.get('hebbian_ltd_total', 0)
ratio = ltp / max(ltd, 1)
temp = sx.get('temperature', 0)
sph = pv.get('spheres', 0)
mem = len(pm) if isinstance(pm, list) else 0
print(f'r={r:.3f} gen={gen} fit={fit:.3f} LTP/LTD={ltp}/{ltd}({ratio:.2f}) T={temp:.3f} sph={sph} POVM={mem} [{ms:.0f}ms]')
"
```
