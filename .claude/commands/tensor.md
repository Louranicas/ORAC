# /tensor — 6-Dimensional Habitat Tensor (~65ms)

Fuses all 6 dimensions in parallel. Run at the start of every context window after bootstrap.

topology: convergent

```bash
python3 -c "
import json, urllib.request as u, subprocess, sqlite3, time, os
from concurrent.futures import ThreadPoolExecutor

S = time.monotonic()

def d1():
    r = subprocess.run(['atuin', 'history', 'list', '--cmd-only'], capture_output=True, text=True, timeout=5)
    h = len(r.stdout.strip().split('\n'))
    oc = json.loads(u.urlopen('http://localhost:8133/health', timeout=2).read())
    pv = json.loads(u.urlopen('http://localhost:8132/health', timeout=2).read())
    return f'D1 TEMPORAL  hist={h} gen={oc.get(\"ralph_gen\",0)} fit={oc.get(\"ralph_fitness\",0):.3f} r={pv.get(\"r\",0):.3f} LTP={oc.get(\"hebbian_ltp_total\",0)} sph={pv.get(\"spheres\",0)}'

def d2():
    t_loc=0; t_test=0
    for repo in ['orac-sidecar','pane-vortex-v2','the_maintenance_engine','vortex-memory-system']:
        src = os.path.expanduser(f'~/claude-code-workspace/{repo}/src')
        try:
            r = subprocess.run(['rg','--hidden','-c','.','--type','rust',src], capture_output=True, text=True, timeout=5)
            t_loc += sum(int(l.split(':')[-1]) for l in r.stdout.strip().split('\n') if ':' in l)
            r2 = subprocess.run(['rg','--hidden','-c','#\\\\[test\\\\]',src], capture_output=True, text=True, timeout=5)
            t_test += sum(int(l.split(':')[-1]) for l in r2.stdout.strip().split('\n') if ':' in l)
        except: pass
    return f'D2 STRUCTURAL  {t_loc:,} LOC  {t_test} tests'

def d3():
    ok=0
    for p in [8080,8081,8090,8100,8101,8102,8103,8104,8105,8110,8120,8125,8130,8132,8133,9001,10001]:
        path='/api/health' if p in (8080,8090) else '/health'
        try:
            if u.urlopen(f'http://localhost:{p}{path}',timeout=2).status==200: ok+=1
        except: pass
    return f'D3 SERVICES  {ok}/17 healthy'

def d4():
    pm = json.loads(u.urlopen('http://localhost:8125/memories?limit=500',timeout=3).read())
    rm = json.loads(u.urlopen('http://localhost:8130/health',timeout=2).read())
    db = os.path.expanduser('~/claude-code-workspace/developer_environment_manager/service_tracking.db')
    try:
        c = sqlite3.connect(db,timeout=2); pt = c.execute('SELECT COUNT(*) FROM learned_patterns').fetchone()[0]; c.close()
    except: pt=0
    return f'D4 MEMORY  POVM={len(pm)} RM={rm.get(\"active_entries\",0)} patterns={pt} scripts=9'

def d5():
    tabs = subprocess.run(['zellij','action','query-tab-names'], capture_output=True, text=True, timeout=3)
    tc = len([l for l in tabs.stdout.strip().split('\n') if l])
    wt = subprocess.run(['git','-C',os.path.expanduser('~/claude-code-workspace/orac-sidecar'),'worktree','list'], capture_output=True, text=True, timeout=3)
    wc = len([l for l in wt.stdout.strip().split('\n') if l])
    return f'D5 COORDINATION  {tc} tabs  {wc} worktrees'

def d6():
    oc = json.loads(u.urlopen('http://localhost:8133/health',timeout=2).read())
    pv = json.loads(u.urlopen('http://localhost:8132/health',timeout=2).read())
    me = json.loads(u.urlopen('http://localhost:8080/api/observer',timeout=2).read())
    sx = json.loads(u.urlopen('http://localhost:8090/v3/thermal',timeout=2).read())
    mf=me.get('last_report',me).get('current_fitness',0);of=oc.get('ralph_fitness',0);pr=pv.get('r',0)
    return f'D6 SYNTHESIS  metabolic={mf*of*pr:.4f} ME={mf:.3f} ORAC={of:.3f} PV={pr:.3f} T={sx.get(\"temperature\",0):.3f}'

with ThreadPoolExecutor(6) as pool:
    results = [pool.submit(f).result() for f in [d1,d2,d3,d4,d5,d6]]

ms = (time.monotonic() - S) * 1000
print(f'=== Habitat Tensor ({ms:.0f}ms) ===')
for r in results: print(f'  {r}')
"
```
