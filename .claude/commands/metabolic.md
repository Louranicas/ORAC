# /metabolic -- Cross-Service Health Composite

Computes the metabolic product: ME fitness x ORAC fitness x PV2 r.
A single number capturing whether the system is learning, not just alive.

```bash
python3 -c "
import json, urllib.request as u

def f(url):
    try: return json.loads(u.urlopen(url, timeout=2).read())
    except: return {}

oc = f('http://localhost:8133/health')
pv = f('http://localhost:8132/health')
me = f('http://localhost:8080/api/observer')

me_fit = me.get('last_report', me).get('current_fitness', 0)
oc_fit = oc.get('ralph_fitness', 0)
pv_r = pv.get('r', 0)
metabolic = me_fit * oc_fit * pv_r

ltp = oc.get('hebbian_ltp_total', 0)
ltd = oc.get('hebbian_ltd_total', 0)
ratio = ltp / max(ltd, 1)

print(f'=== Metabolic Product ===')
print(f'  ME fitness:    {me_fit:.4f}')
print(f'  ORAC fitness:  {oc_fit:.4f}')
print(f'  PV2 r:         {pv_r:.4f}')
print(f'  Product:       {metabolic:.4f} (target > 0.55)')
print(f'  LTP/LTD:       {ratio:.4f} (target > 0.10)')
print()
if metabolic > 0.55:
    print('METABOLIC: HEALTHY')
elif metabolic > 0.45:
    print('METABOLIC: RECOVERING')
else:
    print('METABOLIC: DORMANT')
"
```
