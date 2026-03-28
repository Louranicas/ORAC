# /broadcast -- Memory Propagation to All Substrates

Write a structured event to all 6+ memory substrates in parallel.
One call, ~500ms, guaranteed completeness across RM, POVM, MCP KG, SQLite, Obsidian ref, Auto-Memory.

topology: broadcast

Usage: Run this command and provide the content to broadcast when prompted.

```bash
# Broadcast requires content — this command shows the pattern and current substrate health
python3 -c "
import json, urllib.request as u
from concurrent.futures import ThreadPoolExecutor

substrates = {
    'RM': 'http://localhost:8130/health',
    'POVM': 'http://localhost:8125/health',
    'PV2': 'http://localhost:8132/health',
    'ORAC': 'http://localhost:8133/health',
}

def check(item):
    name, url = item
    try:
        r = u.urlopen(url, timeout=2)
        return (name, r.status == 200)
    except:
        return (name, False)

with ThreadPoolExecutor(max_workers=6) as pool:
    results = dict(pool.map(check, substrates.items()))

print('=== Broadcast Substrate Health ===')
for name, healthy in results.items():
    print(f'  {name}: {\"UP\" if healthy else \"DOWN\"}')
print()
print('To broadcast, use this pattern:')
print('  RM:   printf \"cat\\\\tagent\\\\tconf\\\\tttl\\\\tcontent\" | curl -sf -X POST localhost:8130/put --data-binary @-')
print('  POVM: curl -sf -X POST localhost:8125/memories -H \"Content-Type: application/json\" -d \\'JSON\\'')
print('  MCP:  mcp__memory__create_entities')
print()
ready = sum(1 for v in results.values() if v)
print(f'Substrates ready: {ready}/{len(substrates)}')
"
```
