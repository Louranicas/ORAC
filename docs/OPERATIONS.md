---
title: "ORAC Sidecar — Operations Runbook"
aliases:
  - "ORAC Operations"
  - "ORAC Runbook"
  - "ORAC Ops"
tags:
  - orac
  - operations
  - runbook
  - ultraplate
created: 2026-03-25
updated: 2026-03-25
---

# ORAC Sidecar — Operations Runbook

> **Self-contained operations reference. No other file required at 3am.**
>
> ORAC Sidecar v0.10.0 | Port 8133 | Batch 5 | 40 modules | 8 layers | ~41K LOC | ~1,748 tests
>
> Envoy-like proxy specialized for AI agent traffic: HTTP hooks, Hebbian STDP,
> RALPH evolution, Kuramoto field coordination, 6 external service bridges.

---

## Section 1: Quick Reference

### 1.1 ULTRAPLATE Service Port Table (17 Active + ORAC)

```
Port   Service                 Health Path      Batch  Notes
-----  ----------------------  ---------------  -----  ---------------------------
8080   Maintenance Engine      /api/health      2      12D tensor, PBFT, EventBus
8081   DevOps Engine           /health          1      Neural orchestration
8090   SYNTHEX                 /api/health      2      REST+WS, V3 homeostasis, thermal PID
8100   SAN-K7 Orchestrator     /health          2      M1-M55, 59 modules
8101   NAIS                    /health          3      Neural adaptive intelligence
8102   Bash Engine             /health          3      45 safety patterns
8103   Tool Maker              /health          3      v1.55.0
8104   Context Manager         /health          4      41 crates
8105   Tool Library            /health          4      65 tools
8110   CodeSynthor V7          /health          1      62 modules, 17 layers
8120   Vortex Memory System    /health          5      OVM + POVM bridge
8125   POVM Engine             /health          1      Persistent OVM store
8130   Reasoning Memory        /health          4      Cross-session (TSV ONLY, NOT JSON)
8132   Pane-Vortex V2          /health          5      Fleet coordination, Kuramoto
8133   ORAC Sidecar            /health          5      THIS SERVICE — fleet proxy
9001   Architect Agent         /health          2      Pattern library, design
10001  Prometheus Swarm        /health          2      CVA-NAM 40 agents, PBFT

Disabled: library-agent (8083), sphere-vortex (8120 conflict)
```

### 1.2 ORAC Upstream Dependencies

```
Service              Port   Required For
-------------------  -----  -------------------------------------------
Pane-Vortex V2       8132   Field state, IPC bus, sphere data
POVM Engine          8125   Memory hydration, pathway persistence
SYNTHEX              8090   Thermal signal, heat source posting
Maintenance Engine   8080   Observer fitness, EventBus data
Reasoning Memory     8130   Cross-session TSV persistence
Vortex Memory System 8120   Semantic memory queries, consolidation
```

### 1.3 Restart Order (Dependency Batches)

Always start in this order. Do NOT start a batch until the previous batch is healthy.

```
Batch 1 (no deps):     devops-engine (8081), codesynthor-v7 (8110), povm-engine (8125)
Batch 2 (needs B1):    synthex (8090), san-k7 (8100), maintenance-engine (8080),
                        architect-agent (9001), prometheus-swarm (10001)
Batch 3 (needs B2):    nais (8101), bash-engine (8102), tool-maker (8103)
Batch 4 (needs B3):    claude-context-manager (8104), tool-library (8105),
                        reasoning-memory (8130)
Batch 5 (needs B4):    vortex-memory-system (8120), pane-vortex (8132),
                        orac-sidecar (8133)
```

Full fleet start command:

```bash
# Kill rogue port occupants from stale devenv stop (BUG-001)
for port in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
  pid=$(ss -tlnp "sport = :$port" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
  [[ -n "$pid" ]] && kill "$pid" 2>/dev/null
done
sleep 2
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
```

### 1.4 Log Locations

```
File/Command                                  What It Contains
--------------------------------------------  ------------------------------------------
/tmp/orac-session.log                         ORAC daemon stdout/stderr (manual start)
/tmp/zellij-1000/zellij-log/zellij.log        Zellij session log (plugin crashes)
devenv-logs                                   Alias: tails all devenv service logs
~/.local/share/devenv/pids/                   PID files for each service
~/.local/share/orac/blackboard.db             ORAC SQLite blackboard (9 tables)
data/bus_tracking.db                          Bus tracking database (relative to ORAC dir)
data/field_tracking.db                        Field tracking database (relative to ORAC dir)
```

### 1.5 ORAC Endpoints (22 Routes)

```
Method  Path                       Purpose
------  -------------------------  ------------------------------------------
GET     /health                    Liveness probe (30+ fields, always 200)
GET     /field                     Cached Kuramoto field + live PV2 enrichment
GET     /thermal                   SYNTHEX thermal state (cached)
GET     /blackboard                Session state + RALPH snapshot
GET     /metrics                   Prometheus text format
GET     /field/ghosts              Deregistered sphere traces
GET     /traces                    OTel trace store query
GET     /dashboard                 Kuramoto field dashboard (r history, chimera)
GET     /tokens                    Token accounting summary
GET     /coupling                  Coupling network state
GET     /hebbian                   Hebbian STDP statistics
GET     /emergence                 Emergence event history
GET     /bridges                   Bridge health summary (6 services)
GET     /ralph                     RALPH engine state (gen, fitness, phase)
GET     /dispatch                  Dispatch statistics (read/write/exec/comms)
GET     /consent/{sphere_id}       Read consent declaration
PUT     /consent/{sphere_id}       Update consent declaration
POST    /hooks/SessionStart        Register sphere, hydrate from POVM/RM
POST    /hooks/Stop                Deregister, crystallize, ghost trace
POST    /hooks/PostToolUse         Memory, status update, task poll
POST    /hooks/PreToolUse          SYNTHEX thermal gate
POST    /hooks/UserPromptSubmit    Inject r/tick/spheres/thermal
POST    /hooks/PermissionRequest   Auto-approve/deny policy engine
```

### 1.6 Build, Deploy, Rollback

```bash
# Build (from ORAC project root)
cd /home/louranicas/claude-code-workspace/orac-sidecar
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release

# Deploy binary (MUST use /usr/bin/cp — cp is aliased to interactive)
/usr/bin/cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/orac-sidecar

# Manual start (preferred over devenv for ORAC — avoids SIGPIPE on stdout)
cd /home/louranicas/claude-code-workspace/orac-sidecar
RUST_LOG=orac_sidecar=info \
PORT=8133 \
PV2_ADDR=127.0.0.1:8132 \
SYNTHEX_ADDR=127.0.0.1:8090 \
POVM_ADDR=127.0.0.1:8125 \
RM_ADDR=127.0.0.1:8130 \
nohup ~/.local/bin/orac-sidecar > /tmp/orac-session.log 2>&1 &

# Or via devenv
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart orac-sidecar

# Verify
curl -s http://localhost:8133/health | python3 -m json.tool

# Rollback hooks to bash scripts
/usr/bin/cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json
```

### 1.7 Quality Gate (Mandatory Before Deploy)

```bash
cd /home/louranicas/claude-code-workspace/orac-sidecar
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```

Order: check -> clippy -> pedantic -> test. Zero tolerance at every stage.

### 1.8 Health Check All 17 Ports

```bash
declare -A hpath=(
  [8080]="/api/health" [8081]="/health" [8090]="/api/health" [8100]="/health"
  [8101]="/health" [8102]="/health" [8103]="/health" [8104]="/health"
  [8105]="/health" [8110]="/health" [8120]="/health" [8125]="/health"
  [8130]="/health" [8132]="/health" [8133]="/health" [9001]="/health"
  [10001]="/health"
)
for port in "${!hpath[@]}"; do
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://localhost:$port${hpath[$port]}" 2>/dev/null)
  echo "Port $port: $code"
done
```

---

## Section 2: Symptom-to-Fix Runbook

Each entry: **Severity** | **Symptom** | **Likely Cause** | **Diagnosis** | **Fix** | **Prevention**

---

### ORAC Core (SYM-001 through SYM-008)

#### SYM-001: ORAC Port 8133 Occupied

**Severity:** CRITICAL
**Symptom:** `curl http://localhost:8133/health` returns connection refused, or `devenv start` reports ORAC failed to bind.
**Likely Cause:** Stale ORAC process from previous session survived devenv stop (BUG-001).
**Diagnosis:**

```bash
ss -tlnp "sport = :8133" 2>/dev/null
# Shows PID of process holding the port
```

**Fix:**

```bash
pid=$(ss -tlnp "sport = :8133" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
[[ -n "$pid" ]] && kill "$pid" 2>/dev/null
sleep 1
# Restart ORAC (manual or devenv)
```

**Prevention:** Always kill port occupants before `devenv start`. Use the Batch 1-5 start script from Section 1.3.

---

#### SYM-002: Field Coherence r = 0.0

**Severity:** HIGH
**Symptom:** `/health` shows `"field_r": 0.0` and `"sphere_count": 0`.
**Likely Cause:** Pane-Vortex V2 (8132) is down or ORAC cannot reach it. Field poller has no data to cache.
**Diagnosis:**

```bash
# Check PV2 health
curl -s http://localhost:8132/health | python3 -m json.tool

# Check ORAC field endpoint
curl -s http://localhost:8133/field | python3 -m json.tool

# Check ORAC breaker state for PV2
curl -s http://localhost:8133/bridges | python3 -m json.tool
```

**Fix:**

1. If PV2 is down: `~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart pane-vortex`
2. If PV2 breaker is Open: wait for half-open timeout (60 ticks) or restart ORAC
3. If PV2 is healthy but ORAC shows r=0: restart ORAC (field poller initialization)

**Prevention:** Ensure PV2 starts before ORAC (both Batch 5, but PV2 first in depends_on).

---

#### SYM-003: IPC Socket Dead / Connection Refused

**Severity:** HIGH
**Symptom:** `/health` shows `"ipc_state": "disconnected"`. No real-time events from PV2.
**Likely Cause:** Old PV2 process survived restart, socket file exists but listener is dead (GAP-B root cause).
**Diagnosis:**

```bash
# Check socket file exists
ls -la /run/user/1000/pane-vortex-bus.sock

# Check PV2 process
ss -tlnp "sport = :8132"

# Check ORAC log for reconnect attempts
tail -50 /tmp/orac-session.log | grep -i "ipc\|socket\|reconnect"
```

**Fix:**

```bash
# Kill old PV2
pid=$(ss -tlnp "sport = :8132" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
[[ -n "$pid" ]] && kill "$pid" 2>/dev/null
sleep 1

# Remove stale socket (MUST use /usr/bin/rm — rm is aliased to trash)
/usr/bin/rm -f /run/user/1000/pane-vortex-bus.sock

# Restart PV2 then ORAC
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart pane-vortex
sleep 3
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart orac-sidecar
```

**Prevention:** Check `ipc_state` in health response after every ORAC restart. IPC auto-reconnects with escalating backoff (5s to 120s cap).

---

#### SYM-004: Hebbian LTP = 0 (No Learning)

**Severity:** HIGH
**Symptom:** `/health` shows `"hebbian_ltp_total": 0` even after 50+ ticks. Coupling weights frozen at initial values.
**Likely Cause:** Coupling network has no PV2 sphere IDs registered. STDP cannot fire because endpoint IDs do not match sphere IDs in the coupling graph (GAP-A root cause).
**Diagnosis:**

```bash
# Check coupling state
curl -s http://localhost:8133/coupling | python3 -m json.tool

# Check Hebbian stats
curl -s http://localhost:8133/hebbian | python3 -m json.tool

# Check sphere count (must be > 0)
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'spheres={d.get(\"sphere_count\",0)} ltp={d.get(\"hebbian_ltp_total\",0)} ltd={d.get(\"hebbian_ltd_total\",0)}')
"
```

**Fix:**

1. Verify PV2 is serving spheres: `curl -s http://localhost:8132/health | python3 -c "import sys,json;print(json.load(sys.stdin).get('sphere_count',0))"`
2. If sphere_count > 0 on PV2 but 0 on ORAC: restart ORAC (field poller syncs spheres to coupling network on startup)
3. If LTP stays 0 after restart: check that the field poller seed code in `m10_hook_server.rs` always syncs spheres (no `is_empty()` guard)

**Prevention:** Monitor `hebbian_ltp_total` after every restart. Should be > 0 within 5 ticks if spheres exist.

---

#### SYM-005: ME Observer Frozen / me_fitness Stale

**Severity:** MEDIUM
**Symptom:** `/health` shows `"me_frozen": true` or `"me_fitness"` unchanged across many ticks.
**Likely Cause:** Maintenance Engine (8080) is down, or ME observer endpoint returning unexpected schema.
**Diagnosis:**

```bash
# Direct ME health check
curl -s http://localhost:8080/api/health | python3 -m json.tool

# Direct ME observer check
curl -s http://localhost:8080/api/observer | python3 -m json.tool

# Check ORAC breaker for ME
curl -s http://localhost:8133/bridges | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(json.dumps(d.get('me', {}), indent=2))
"
```

**Fix:**

1. If ME is down: `~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart maintenance-engine`
2. If ME responds but ORAC shows frozen: check log for `[PV-1302] bridge parse error: me` — schema may have changed
3. If breaker is Open: wait for half-open or restart ORAC

**Prevention:** ME observer response must contain `last_report.current_fitness` path. If ME schema changes, update `m23_me_bridge.rs`.

---

#### SYM-006: Sessions = 0 / Hooks Not Registering

**Severity:** MEDIUM
**Symptom:** `/health` shows `"sessions": 0` even though Claude Code is actively running.
**Likely Cause:** Hook forwarder not wired in `~/.claude/settings.json`, or ORAC is not running when hooks fire.
**Diagnosis:**

```bash
# Check settings.json for ORAC hooks
python3 -c "
import json
with open('$HOME/.claude/settings.json') as f:
    d = json.load(f)
hooks = d.get('hooks', {})
for event, cfg in hooks.items():
    cmds = cfg if isinstance(cfg, list) else [cfg]
    for c in cmds:
        cmd = c.get('command','') if isinstance(c, dict) else str(c)
        if 'orac' in cmd.lower():
            print(f'{event}: {cmd}')
"

# Check ORAC is responding
curl -s -o /dev/null -w '%{http_code}' http://localhost:8133/health

# Test a hook manually
echo '{"event":{"event":"SessionStart","session_id":"test-123"}}' | \
  curl -s -X POST http://localhost:8133/hooks/SessionStart \
  -H 'Content-Type: application/json' -d @-
```

**Fix:**

1. If hooks not in settings.json: check `~/.claude/settings.json` references `orac-hook.sh`
2. If ORAC is not running: start it (Section 1.6)
3. Sessions persist to blackboard every 60 ticks. After restart, existing sessions are hydrated.

**Prevention:** After deploying, verify all 6 hook endpoints respond to test POST.

---

#### SYM-007: Stale Binary Deployed

**Severity:** MEDIUM
**Symptom:** Code changes not taking effect. `/health` reports old version. New endpoints return 404.
**Likely Cause:** `cp` alias intercepted the deploy command with `-i` (interactive mode), user answered 'n', or old binary path used.
**Diagnosis:**

```bash
# Check binary timestamp
ls -la ~/.local/bin/orac-sidecar

# Check running binary PID and start time
ps -p $(ss -tlnp "sport = :8133" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1) -o pid,lstart,comm 2>/dev/null
```

**Fix:**

```bash
# Rebuild
cd /home/louranicas/claude-code-workspace/orac-sidecar
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release

# Deploy (MUST use /usr/bin/cp — never bare cp)
/usr/bin/cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/orac-sidecar

# Restart
pid=$(ss -tlnp "sport = :8133" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
[[ -n "$pid" ]] && kill "$pid" 2>/dev/null
sleep 1
# Manual start (Section 1.6)
```

**Prevention:** ALWAYS use `/usr/bin/cp -f` for binary deployment. The `cp` command is aliased to `cp -i` in this environment (BUG-027).

---

#### SYM-008: PascalCase Hook Paths Not Found

**Severity:** LOW
**Symptom:** Hook calls return 404. Log shows `POST /hooks/session_start` instead of `POST /hooks/SessionStart`.
**Likely Cause:** Hook forwarder or caller using snake_case path. ORAC routes are PascalCase.
**Diagnosis:**

```bash
# Check what path the hook forwarder is sending
grep -i "endpoint\|url\|path" /home/louranicas/claude-code-workspace/orac-sidecar/hooks/orac-hook.sh
```

**Fix:** Ensure hook invocations use PascalCase event names: `SessionStart`, `Stop`, `PostToolUse`, `PreToolUse`, `UserPromptSubmit`, `PermissionRequest`. The Axum router has NO snake_case aliases.

**Prevention:** The `orac-hook.sh` forwarder takes `$1` as the event name and appends it directly to `/hooks/`. Always pass PascalCase.

---

### Cross-Service (SYM-009 through SYM-015)

#### SYM-009: SYNTHEX Breaker Path Mismatch

**Severity:** HIGH
**Symptom:** SYNTHEX breaker permanently Open. Log shows `[PV-1300] bridge unreachable: synthex`.
**Likely Cause:** Bridge address includes `http://` prefix. Bridges use raw TCP, not HTTP client library (BUG-033).
**Diagnosis:**

```bash
# Check bridge config
curl -s http://localhost:8133/bridges | python3 -m json.tool

# Check env vars
env | grep -i synthex

# Check config file
grep synthex_addr /home/louranicas/claude-code-workspace/orac-sidecar/config/default.toml
```

**Fix:** Bridge addresses MUST be raw `host:port` format without protocol prefix.

```bash
# Config must read: synthex_addr = "127.0.0.1:8090" (NOT "http://127.0.0.1:8090")
# Or env: SYNTHEX_ADDR=127.0.0.1:8090 (no http:// prefix)
```

Restart ORAC after fixing. Same applies to ME, POVM, RM addresses.

**Prevention:** All bridge URLs use raw `TcpStream::connect()`. The `http://` prefix causes DNS resolution failure on "http" as hostname.

---

#### SYM-010: Prometheus Swarm Crash on POST

**Severity:** CRITICAL
**Symptom:** Prometheus Swarm (10001) crashes with SIGABRT when receiving POST /api/tasks.
**Likely Cause:** Pre-compiled binary with known crash bug (CRIT-01). Python wrapper segfault.
**Diagnosis:**

```bash
# Check if Prometheus is alive
curl -s -o /dev/null -w '%{http_code}' http://localhost:10001/health

# Check for crash in logs
devenv-logs | grep -i "prometheus\|sigabrt\|signal" | tail -20
```

**Fix:**

```bash
# Restart Prometheus (it will crash again on next POST, but GET is safe)
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart prometheus-swarm

# WORKAROUND: Do NOT send POST /api/tasks to Prometheus until binary is rebuilt
```

**Prevention:** This is a known unfixed bug in the pre-compiled Prometheus binary. Avoid POST requests to it. GET /health is safe.

---

#### SYM-011: Blackboard Ghost Trace Bloat (900+ Ghosts)

**Severity:** MEDIUM
**Symptom:** `/field/ghosts` returns hundreds or thousands of entries. Blackboard DB growing.
**Likely Cause:** Many short-lived sessions deregistering spheres. Ghost traces accumulate without pruning.
**Diagnosis:**

```bash
# Check ghost count
curl -s http://localhost:8133/field/ghosts | python3 -c "
import sys, json
d = json.load(sys.stdin)
ghosts = d if isinstance(d, list) else d.get('ghosts', [])
print(f'Ghost count: {len(ghosts)}')
"

# Check blackboard DB size
ls -lh ~/.local/share/orac/blackboard.db
```

**Fix:**

```bash
# Prune old ghost traces directly in SQLite
sqlite3 ~/.local/share/orac/blackboard.db \
  "DELETE FROM ghost_traces WHERE rowid NOT IN (SELECT rowid FROM ghost_traces ORDER BY rowid DESC LIMIT 100);"
sqlite3 ~/.local/share/orac/blackboard.db "VACUUM;"
```

**Prevention:** Schedule monthly ghost trace pruning (Section 4). Consider adding TTL-based auto-prune to ghost_traces table.

---

#### SYM-012: POVM ID Namespace Mismatch

**Severity:** MEDIUM
**Symptom:** POVM hydration loads 2500+ pathways but 0 match coupling IDs. `/health` shows `"co_activations_total": 0` despite pathways loaded.
**Likely Cause:** ORAC generates sphere IDs as `orac-hostname:pid:uuid`. POVM stores pathways with PV2-format IDs. ID formats do not intersect.
**Diagnosis:**

```bash
# Check POVM pathways
curl -s http://localhost:8125/pathways | python3 -c "
import sys, json
d = json.load(sys.stdin)
paths = d if isinstance(d, list) else d.get('pathways', [])
if paths:
    print(f'Sample IDs: pre={paths[0].get(\"pre_id\",\"?\")}, post={paths[0].get(\"post_id\",\"?\")}')
print(f'Total pathways: {len(paths)}')
"

# Check ORAC coupling IDs
curl -s http://localhost:8133/coupling | python3 -c "
import sys, json
d = json.load(sys.stdin)
conns = d.get('connections', [])
if conns:
    print(f'Sample coupling ID: {conns[0].get(\"from\",\"?\")}/{conns[0].get(\"to\",\"?\")}')
print(f'Total connections: {len(conns)}')
"
```

**Fix:** This is a known limitation. ORAC sphere IDs and POVM pathway IDs use different namespaces. The persist_stdp_to_povm function writes new pathways with ORAC IDs. Over time, POVM accumulates ORAC-format pathways that DO match. Historical PV2-format pathways will never match.

**Prevention:** Not currently preventable without a namespace migration. Monitor `co_activations_total` trending upward over sessions.

---

#### SYM-013: Bridge URL Has http:// Prefix

**Severity:** HIGH
**Symptom:** All bridge calls fail. All breakers trip to Open. Log flooded with `[PV-1300] bridge unreachable`.
**Likely Cause:** Configuration or environment variable includes `http://` prefix (BUG-033).
**Diagnosis:**

```bash
# Check all bridge addresses in running config
curl -s http://localhost:8133/bridges | python3 -m json.tool
```

**Fix:**

```bash
# Fix environment variables (remove http:// prefix)
# WRONG: PV2_ADDR=http://127.0.0.1:8132
# RIGHT: PV2_ADDR=127.0.0.1:8132

# Fix config/default.toml
# WRONG: synthex_addr = "http://127.0.0.1:8090"
# RIGHT: synthex_addr = "127.0.0.1:8090"
```

Restart ORAC after fixing.

**Prevention:** Bridges use raw TCP (`std::net::TcpStream`), not an HTTP client library. They construct HTTP requests manually over the TCP stream. The address is passed directly to `TcpStream::connect_timeout()`.

---

#### SYM-014: devenv Shows Fewer Than 17 Services

**Severity:** HIGH
**Symptom:** `devenv status` or `devenv health` shows fewer than 17 services running.
**Likely Cause:** `devenv stop` did not kill all processes (BUG-001). Stale processes hold ports, preventing new instances from binding.
**Diagnosis:**

```bash
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml status
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml health
```

**Fix:**

```bash
# Nuclear restart: kill all port occupants then start fresh
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml stop
for port in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
  pid=$(ss -tlnp "sport = :$port" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
  [[ -n "$pid" ]] && kill "$pid" 2>/dev/null
done
sleep 2
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
```

**Prevention:** Always use the kill-and-start pattern. Never trust `devenv stop` to fully clean up.

---

#### SYM-015: All Breakers Open

**Severity:** CRITICAL
**Symptom:** `/bridges` shows all 6 breakers (pv2, synthex, me, povm, rm, vms) in state "Open". All bridge calls skipped.
**Likely Cause:** All upstream services are down, or ORAC was started before upstream services.
**Diagnosis:**

```bash
# Check all upstream services
for svc in 8132 8090 8080 8125 8130 8120; do
  echo "Port $svc: $(curl -s -o /dev/null -w '%{http_code}' http://localhost:$svc/health 2>/dev/null)"
done

# Check ORAC breaker detail
curl -s http://localhost:8133/bridges | python3 -m json.tool
```

**Fix:**

1. Start missing upstream services using batch order (Section 1.3)
2. Wait for breaker half-open timeout (60 ticks = ~5 minutes at default tick interval)
3. If impatient: restart ORAC (breakers reset to Closed on startup)

**Prevention:** Start ORAC last (Batch 5). Breaker state is NOT persisted across restarts (GAP-F).

---

### Learning System (SYM-016 through SYM-020)

#### SYM-016: RALPH Fitness Declining

**Severity:** MEDIUM
**Symptom:** `/ralph` shows fitness trending downward across generations. `ralph_converged` stays false.
**Likely Cause:** Mutation selector picking counter-productive parameter changes, or field state is unstable.
**Diagnosis:**

```bash
# Check RALPH state
curl -s http://localhost:8133/ralph | python3 -m json.tool

# Check fitness trend (compare gen vs fitness)
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'gen={d.get(\"ralph_gen\",0)} fitness={d.get(\"ralph_fitness\",0):.4f} phase={d.get(\"ralph_phase\",\"?\")} r={d.get(\"field_r\",0):.4f}')
"

# Check emergence (should be > 0)
curl -s http://localhost:8133/emergence | python3 -m json.tool
```

**Fix:**

1. If field_r < 0.5: fix field coherence first (SYM-002). RALPH cannot learn in chaotic field.
2. If emergence_events = 0: check emergence detectors (SYM-020)
3. RALPH has snapshot/rollback: if fitness drops 3 generations in a row, it auto-rolls back.
4. Nuclear option: restart ORAC. RALPH hydrates last-known-good from blackboard `ralph_state` table.

**Prevention:** Monitor `ralph_fitness` in star probes. Healthy range: > 0.55. Alarm if declining for 5+ generations.

---

#### SYM-017: POVM Reports 0 Matched Pathways

**Severity:** LOW
**Symptom:** POVM hydration succeeds (pathways loaded) but none match ORAC coupling IDs.
**Likely Cause:** ID namespace mismatch (same as SYM-012).
**Diagnosis:** Same as SYM-012.
**Fix:** This resolves itself over time as ORAC writes pathways with its own ID format. After 60+ ticks of STDP activity, ORAC-format pathways accumulate in POVM.
**Prevention:** Expected behavior for new ORAC sessions. Monitor `co_activations_total` trending upward.

---

#### SYM-018: Emergence Detection Silent

**Severity:** MEDIUM
**Symptom:** `/emergence` shows `"total_detected": 0` and `"active_monitors": 0`.
**Likely Cause:** Emergence observations not fed to detectors in main loop, or detectors not wired.
**Diagnosis:**

```bash
# Check emergence endpoint
curl -s http://localhost:8133/emergence | python3 -m json.tool

# Check if detectors are registered
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'emergence_events={d.get(\"emergence_events\",0)} active_monitors={d.get(\"emergence_active_monitors\",0)}')
"
```

**Fix:**

1. If `active_monitors = 0`: the `feed_emergence_observations` function in `main.rs` is not calling detectors. Check that FieldStability and BeneficialSync are wired.
2. If `active_monitors > 0` but `emergence_events = 0`: thresholds may be too high. Current defaults:
   - BeneficialSync: r > 0.85
   - FieldStability: r > 0.70 for 20 consecutive ticks
   - CoherenceLock: r > 0.998 (very high, may need lowering to 0.98)
   - HebbianSaturation: weight < floor + 0.01
3. DispatchLoop and ConsentCascade detectors require explicit wiring in `feed_emergence_observations` (may not be connected yet).

**Prevention:** After code changes to emergence, verify at least 3 detector types fire within 100 ticks.

---

#### SYM-019: Coupling Weights at Floor (All 0.15)

**Severity:** MEDIUM
**Symptom:** `/coupling` shows all weights at 0.15 (the floor). No differentiation.
**Likely Cause:** LTP is not firing (see SYM-004). STDP needs co-activations to strengthen weights.
**Diagnosis:**

```bash
curl -s http://localhost:8133/coupling | python3 -c "
import sys, json
d = json.load(sys.stdin)
weights = [c.get('weight', 0) for c in d.get('connections', [])]
if weights:
    print(f'min={min(weights):.3f} max={max(weights):.3f} mean={sum(weights)/len(weights):.3f} count={len(weights)}')
else:
    print('No connections')
"
```

**Fix:** Resolve SYM-004 (LTP = 0) first. Once STDP fires, weights differentiate naturally. Healthy range after 50+ ticks: 0.15 to 1.0, mean around 0.4.

**Prevention:** Monitor `coupling_weight_mean` in health response. If stuck at 0.15 for 100+ ticks, investigate LTP.

---

#### SYM-020: Thermal Reading is NaN

**Severity:** MEDIUM
**Symptom:** `/thermal` shows `"temperature": NaN` or `"k_adjustment": NaN`.
**Likely Cause:** SYNTHEX (8090) returning malformed data, division by zero in PID controller, or SYNTHEX bridge parse error.
**Diagnosis:**

```bash
# Direct SYNTHEX thermal check
curl -s http://localhost:8090/v3/thermal | python3 -m json.tool

# Check ORAC thermal cache
curl -s http://localhost:8133/thermal | python3 -m json.tool

# Check ORAC logs for NaN guard
tail -100 /tmp/orac-session.log | grep -i "nan\|inf\|thermal"
```

**Fix:** ORAC has an H001 guard: NaN/INF thermal values are replaced with neutral 1.0. If this is firing frequently:

1. Check SYNTHEX health: `curl -s http://localhost:8090/api/health | python3 -m json.tool`
2. If SYNTHEX has no heat sources: this is expected (0/0 = NaN in PID). Thermal activates when heat sources are posted.
3. Restart SYNTHEX if corrupted: `~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart synthex`

**Prevention:** SYNTHEX thermal needs active heat sources to produce meaningful readings. ORAC posts field state to `/api/ingest` every 6 ticks.

---

### Operational (SYM-021 through SYM-025)

#### SYM-021: SQLite Database Locked

**Severity:** MEDIUM
**Symptom:** Log shows `[PV-1500] database error: database is locked`. Blackboard writes fail intermittently.
**Likely Cause:** Multiple threads accessing SQLite without WAL mode, or long-running transaction blocking writers.
**Diagnosis:**

```bash
# Check WAL mode
sqlite3 ~/.local/share/orac/blackboard.db "PRAGMA journal_mode;"
# Should return: wal

# Check if another process has the DB open
fuser ~/.local/share/orac/blackboard.db 2>/dev/null
```

**Fix:**

1. If journal_mode is not WAL: `sqlite3 ~/.local/share/orac/blackboard.db "PRAGMA journal_mode=WAL;"`
2. If locked by external process: close sqlite3 CLI sessions or other tools accessing the DB
3. Database errors are retryable (PV-1500 is_retryable = true). ORAC will retry automatically.

**Prevention:** Never open the blackboard DB in write mode from sqlite3 CLI while ORAC is running. Use read-only: `sqlite3 -readonly ~/.local/share/orac/blackboard.db`

---

#### SYM-022: Token Budget Exceeded

**Severity:** LOW
**Symptom:** `/tokens` shows a pane exceeding its budget. No enforcement action (budget is advisory).
**Likely Cause:** High tool call volume from a specific pane.
**Diagnosis:**

```bash
curl -s http://localhost:8133/tokens | python3 -m json.tool
```

**Fix:** Token accounting is currently advisory only. No action required unless building enforcement:

1. Check which pane is high: look at per-pane breakdown in `/tokens`
2. Budget is estimated as `chars / 4` per tool call

**Prevention:** Monitor `/tokens` endpoint. Set explicit budgets via `m35_token_accounting` configuration.

---

#### SYM-023: WASM Ring Buffer Full

**Severity:** LOW
**Symptom:** Events not reaching Zellij WASM plugin. `/tmp/swarm-events.jsonl` grows to 1000 lines and stops accepting new events.
**Likely Cause:** Ring buffer hit its 1000-line cap. WASM plugin is not consuming events.
**Diagnosis:**

```bash
wc -l /tmp/swarm-events.jsonl
# If 1000: buffer is full

# Check WASM FIFO
ls -la /tmp/swarm-commands.pipe
```

**Fix:**

```bash
# Truncate the ring buffer (ORAC will refill from scratch)
> /tmp/swarm-events.jsonl

# Or delete and let ORAC recreate
/usr/bin/rm -f /tmp/swarm-events.jsonl
```

**Prevention:** WASM plugin should consume and truncate the ring buffer. If plugin is not running, events accumulate until cap. This is by design -- old events are evicted FIFO-style when the cap is reached.

---

#### SYM-024: Cascade Rate Limit Exceeded

**Severity:** LOW
**Symptom:** Log shows `[PV-1403] cascade rate limit exceeded: N per minute`. Cascade handoffs rejected.
**Likely Cause:** Too many cascade handoff requests in a short window. Rate limiter protecting against flood.
**Diagnosis:**

```bash
tail -100 /tmp/orac-session.log | grep -i "cascade\|rate.limit\|PV-1403"
```

**Fix:** Wait for the rate window to pass (1 minute). CascadeRateLimit is non-retryable but self-resolving.

**Prevention:** Space cascade handoff requests. Do not trigger cascades in tight loops.

---

#### SYM-025: Ghost Traces Growing Without Bound

**Severity:** LOW
**Symptom:** `/field/ghosts` returns an ever-growing list. Memory usage slowly increases.
**Likely Cause:** Every sphere deregistration creates a ghost trace. No automatic pruning.
**Diagnosis:**

```bash
# Count ghosts in memory
curl -s http://localhost:8133/field/ghosts | python3 -c "
import sys, json
d = json.load(sys.stdin)
ghosts = d if isinstance(d, list) else d.get('ghosts', [])
print(f'In-memory ghosts: {len(ghosts)}')
"

# Count ghosts in DB
sqlite3 -readonly ~/.local/share/orac/blackboard.db "SELECT COUNT(*) FROM ghost_traces;"
```

**Fix:** Same as SYM-011. Prune via SQL. In-memory ghosts reset on ORAC restart.

**Prevention:** Monthly maintenance (Section 4).

---

## Section 3: Debug Investigation

### 3.1 RUST_LOG Configuration

ORAC uses `tracing` with `tracing-subscriber` `EnvFilter`. Set `RUST_LOG` before starting ORAC.

```bash
# Default (recommended for production)
RUST_LOG=orac_sidecar=info

# Debug a specific module
RUST_LOG=orac_sidecar::m3_hooks=debug,orac_sidecar=info

# Trace bridge calls (very verbose)
RUST_LOG=orac_sidecar::m5_bridges=trace,orac_sidecar=info

# Debug IPC wire protocol
RUST_LOG=orac_sidecar::m2_wire=debug,orac_sidecar=info

# Debug RALPH evolution
RUST_LOG=orac_sidecar::m8_evolution=debug,orac_sidecar=info

# Debug Hebbian STDP
RUST_LOG=orac_sidecar::m4_intelligence::m18_hebbian_stdp=trace,orac_sidecar=info

# Debug all hook handlers
RUST_LOG=orac_sidecar::m3_hooks::m11_session_hooks=debug,orac_sidecar::m3_hooks::m12_tool_hooks=debug,orac_sidecar=info

# Full trace (EXTREMELY verbose — use only for short sessions)
RUST_LOG=orac_sidecar=trace
```

### 3.2 Health Check Interpretation

Key fields from `GET /health` and what they mean:

```
Field                     Healthy Value          Alarm Condition
------------------------  ---------------------  ---------------------------
status                    "healthy"              Anything else
field_r                   > 0.70                 < 0.50 (chaotic field)
sphere_count              > 0                    0 (no PV2 connection)
ralph_gen                 Incrementing           Stuck (tick loop dead)
ralph_fitness             > 0.55                 Declining 3+ gens
ipc_state                 "subscribed"           "disconnected"
breakers.*.state          "Closed"               "Open" (service down)
thermal_temperature       0.3 - 0.8             NaN (SYNTHEX dead)
me_fitness                > 0.40                 0.0 (ME dead)
me_frozen                 false                  true (no ME polls)
hebbian_ltp_total         > 0                    0 after 50 ticks (no learning)
emergence_events          > 0                    0 after 100 ticks
coupling_weight_mean      > 0.20                 == 0.15 (floor, no learning)
synthex_stale             false                  true (bridge timeout)
rm_stale                  false                  true (bridge timeout)
```

### 3.3 Debug Workflow: Hook Not Firing

Problem: A specific hook (e.g., PostToolUse) does not appear to execute.

```bash
# Step 1: Verify ORAC is running
curl -s -o /dev/null -w '%{http_code}' http://localhost:8133/health
# Must return 200

# Step 2: Check hook wiring in settings.json
python3 -c "
import json, os
with open(os.path.expanduser('~/.claude/settings.json')) as f:
    d = json.load(f)
for k, v in d.get('hooks', {}).items():
    print(f'{k}: {v}')
"

# Step 3: Test hook directly
echo '{"event":{"event":"PostToolUse","session_id":"debug-1","tool_name":"Read","tool_input":"{}"}}' | \
  curl -s -X POST http://localhost:8133/hooks/PostToolUse \
  -H 'Content-Type: application/json' -d @- | python3 -m json.tool

# Step 4: Check ORAC logs for the hook handling
tail -50 /tmp/orac-session.log | grep -i "post_tool\|hook"

# Step 5: Check if hook forwarder script is executable
ls -la /home/louranicas/claude-code-workspace/orac-sidecar/hooks/orac-hook.sh
```

### 3.4 Debug Workflow: Bridge Timeout

Problem: A specific bridge (e.g., SYNTHEX) is timing out.

```bash
# Step 1: Check bridge health summary
curl -s http://localhost:8133/bridges | python3 -m json.tool

# Step 2: Check target service directly
curl -s -o /dev/null -w '%{http_code}' http://localhost:8090/api/health
# Replace port/path for the service in question

# Step 3: Check TCP connectivity
timeout 2 bash -c "echo > /dev/tcp/127.0.0.1/8090" && echo "TCP OK" || echo "TCP FAIL"

# Step 4: Check breaker state
curl -s http://localhost:8133/bridges | python3 -c "
import sys, json
d = json.load(sys.stdin)
for svc, state in d.items():
    if isinstance(state, dict) and state.get('state') == 'Open':
        print(f'OPEN: {svc} — failures={state.get(\"failures\",0)} consecutive={state.get(\"consecutive_failures\",0)}')
"

# Step 5: Enable trace logging for bridges
# Restart ORAC with: RUST_LOG=orac_sidecar::m5_bridges=trace,orac_sidecar=info

# Step 6: Check for address issues (http:// prefix)
# Bridge addresses must be raw host:port — see SYM-009/SYM-013
```

### 3.5 Debug Workflow: RALPH Stuck

Problem: RALPH generation is not incrementing.

```bash
# Step 1: Check RALPH state
curl -s http://localhost:8133/ralph | python3 -m json.tool

# Step 2: Check if RALPH is paused/converged
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'gen={d.get(\"ralph_gen\")} fitness={d.get(\"ralph_fitness\")} phase={d.get(\"ralph_phase\")} converged={d.get(\"ralph_converged\")}')
"

# Step 3: Check tick counter is advancing
# Run twice with 30s gap
curl -s http://localhost:8133/health | python3 -c "import sys,json;print(json.load(sys.stdin).get('uptime_ticks'))"
sleep 30
curl -s http://localhost:8133/health | python3 -c "import sys,json;print(json.load(sys.stdin).get('uptime_ticks'))"
# If both return same value: tick loop is dead. Restart ORAC.

# Step 4: Check if evolution feature is compiled in
curl -s http://localhost:8133/ralph
# If 404: binary was built without evolution feature. Rebuild with --features full.

# Step 5: Check blackboard for persisted state
sqlite3 -readonly ~/.local/share/orac/blackboard.db "SELECT * FROM ralph_state;"
```

### 3.6 Debug Workflow: Coupling Not Learning

Problem: Coupling weights are not differentiating despite active sessions.

```bash
# Step 1: Check STDP stats
curl -s http://localhost:8133/hebbian | python3 -m json.tool

# Step 2: Verify sphere count and coupling connections
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'spheres={d.get(\"sphere_count\")} connections={d.get(\"coupling_connections\")} ltp={d.get(\"hebbian_ltp_total\")} ltd={d.get(\"hebbian_ltd_total\")}')
"

# Step 3: If LTP=0 but spheres>0, check coupling network for ID presence
curl -s http://localhost:8133/coupling | python3 -c "
import sys, json
d = json.load(sys.stdin)
conns = d.get('connections', [])
print(f'Connections: {len(conns)}')
ids = set()
for c in conns:
    ids.add(c.get('from',''))
    ids.add(c.get('to',''))
print(f'Unique IDs: {len(ids)}')
if ids:
    print(f'Sample IDs: {list(ids)[:5]}')
"

# Step 4: Check if STDP idle gating is blocking (working_count < 2)
# STDP requires at least 2 active panes. Single-pane sessions produce no co-activations.

# Step 5: Enable STDP trace logging
# RUST_LOG=orac_sidecar::m4_intelligence::m18_hebbian_stdp=trace,orac_sidecar=info
```

### 3.7 Debug Workflow: Emergence Silent

Problem: No emergence events detected despite active system.

```bash
# Step 1: Check emergence state
curl -s http://localhost:8133/emergence | python3 -m json.tool

# Step 2: Check individual detector thresholds
# BeneficialSync:   r > 0.85
# FieldStability:   r > 0.70 for 20 consecutive ticks
# CoherenceLock:    r > 0.998 (or 0.98 if lowered)
# HebbianSaturation: weight < floor + 0.01
# ChimeraFormation: detected by chimera module
# ThermalSpike:     thermal > threshold
# DispatchLoop:     repeated dispatch pattern
# ConsentCascade:   consent change propagation

# Step 3: Check field_r (most detectors need r > 0.70)
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'r={d.get(\"field_r\",0):.4f} emergence={d.get(\"emergence_events\",0)} monitors={d.get(\"emergence_active_monitors\",0)}')
"

# Step 4: If monitors = 0, check if detectors are registered in main.rs feed loop
# Detectors are wired in feed_emergence_observations() in src/bin/main.rs

# Step 5: Lower CoherenceLock threshold if needed
# In m37_emergence_detector.rs: COHERENCE_LOCK_R from 0.998 to 0.98
```

### 3.8 Diagnostic Commands Quick Reference

```bash
# Full health snapshot
curl -s http://localhost:8133/health | python3 -m json.tool

# One-line status summary
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'ORAC gen={d.get(\"ralph_gen\",0)} fit={d.get(\"ralph_fitness\",0):.3f} r={d.get(\"field_r\",0):.3f} spheres={d.get(\"sphere_count\",0)} ipc={d.get(\"ipc_state\",\"?\")} emergence={d.get(\"emergence_events\",0)} ltp={d.get(\"hebbian_ltp_total\",0)}')
"

# Bridge status one-liner
curl -s http://localhost:8133/bridges | python3 -c "
import sys, json
d = json.load(sys.stdin)
for svc, info in d.items():
    if isinstance(info, dict):
        print(f'{svc}: {info.get(\"state\",\"?\")} fail={info.get(\"consecutive_failures\",0)}')
"

# RALPH snapshot
curl -s http://localhost:8133/ralph | python3 -m json.tool

# Star probe (compact multi-service check)
echo '--- ORAC ---'
curl -s http://localhost:8133/health | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'gen={d.get(\"ralph_gen\",0)} fit={d.get(\"ralph_fitness\",0):.3f} r={d.get(\"field_r\",0):.3f}')"
echo '--- PV2 ---'
curl -s http://localhost:8132/health | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'r={d.get(\"r\",0):.3f} spheres={d.get(\"sphere_count\",0)}')" 2>/dev/null || echo 'DOWN'
echo '--- SYNTHEX ---'
curl -s http://localhost:8090/api/health | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'temp={d.get(\"temperature\",0):.3f}')" 2>/dev/null || echo 'DOWN'
echo '--- ME ---'
curl -s http://localhost:8080/api/health | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'fitness={d.get(\"fitness\",0):.3f}')" 2>/dev/null || echo 'DOWN'
echo '--- POVM ---'
curl -s -o /dev/null -w 'HTTP %{http_code}' http://localhost:8125/health 2>/dev/null || echo 'DOWN'
echo ''
echo '--- RM ---'
curl -s -o /dev/null -w 'HTTP %{http_code}' http://localhost:8130/health 2>/dev/null || echo 'DOWN'
echo ''
```

### 3.9 Error Code Reference (PvError)

All ORAC errors use structured `PvError` variants with numeric codes:

```
Range        Category       Count  Retryable?
-----------  -------------  -----  ----------
PV-1000-1099 Config         2      No
PV-1100-1199 Validation     5      No
PV-1200-1299 Field          4      No (except FieldComputation: CRITICAL)
PV-1300-1399 Bridge         4      Yes (Unreachable, Error) / No (Parse, Consent)
PV-1400-1499 Bus            4      Yes (Socket) / No (Protocol, Task, RateLimit)
PV-1500-1599 Persistence    2      Yes (Database) / No (Snapshot)
PV-1600-1699 Governance     3      No
PV-1900-1999 Generic        3      Yes (Io) / No (Json, Internal: CRITICAL)
```

Retryable errors (5 total): `BridgeUnreachable`, `BridgeError`, `BusSocket`, `Database`, `Io`.

Critical errors (2): `FieldComputation` (NaN in order parameter), `Internal` (impossible state).

---

## Section 4: Routine Maintenance

### 4.1 Daily: Star Tracker Probe

Run once per session or daily. Verifies all services and ORAC subsystems.

```bash
echo "=== ULTRAPLATE Star Probe $(date +%Y-%m-%dT%H:%M:%S) ==="

# 17-port health check
declare -A hpath=(
  [8080]="/api/health" [8081]="/health" [8090]="/api/health" [8100]="/health"
  [8101]="/health" [8102]="/health" [8103]="/health" [8104]="/health"
  [8105]="/health" [8110]="/health" [8120]="/health" [8125]="/health"
  [8130]="/health" [8132]="/health" [8133]="/health" [9001]="/health"
  [10001]="/health"
)
healthy=0
total=17
for port in "${!hpath[@]}"; do
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://localhost:$port${hpath[$port]}" 2>/dev/null)
  if [[ "$code" == "200" ]]; then
    ((healthy++))
  else
    echo "  FAIL: port $port returned $code"
  fi
done
echo "Services: $healthy/$total healthy"

# ORAC subsystem check
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'  ORAC: gen={d.get(\"ralph_gen\",0)} fit={d.get(\"ralph_fitness\",0):.3f} r={d.get(\"field_r\",0):.3f} spheres={d.get(\"sphere_count\",0)}')
print(f'  IPC: {d.get(\"ipc_state\",\"?\")}')
print(f'  Learning: ltp={d.get(\"hebbian_ltp_total\",0)} ltd={d.get(\"hebbian_ltd_total\",0)} emergence={d.get(\"emergence_events\",0)}')
print(f'  Coupling: connections={d.get(\"coupling_connections\",0)} mean_weight={d.get(\"coupling_weight_mean\",0):.3f}')
print(f'  Thermal: temp={d.get(\"thermal_temperature\",0):.3f} target={d.get(\"thermal_target\",0):.3f}')
print(f'  ME: fitness={d.get(\"me_fitness\",0):.3f} frozen={d.get(\"me_frozen\",\"?\")}')
" 2>/dev/null || echo "  ORAC: DOWN"
```

### 4.2 After Code Changes: Build-Deploy-Verify

```bash
# 1. Quality gate (MUST pass all 4 stages)
cd /home/louranicas/claude-code-workspace/orac-sidecar
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30

# 2. Build release
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release

# 3. Stop ORAC
pid=$(ss -tlnp "sport = :8133" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
[[ -n "$pid" ]] && kill "$pid" 2>/dev/null
sleep 2

# 4. Deploy (MUST use /usr/bin/cp)
/usr/bin/cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/orac-sidecar

# 5. Start
cd /home/louranicas/claude-code-workspace/orac-sidecar
RUST_LOG=orac_sidecar=info \
PORT=8133 PV2_ADDR=127.0.0.1:8132 SYNTHEX_ADDR=127.0.0.1:8090 \
POVM_ADDR=127.0.0.1:8125 RM_ADDR=127.0.0.1:8130 \
nohup ~/.local/bin/orac-sidecar > /tmp/orac-session.log 2>&1 &

# 6. Verify (wait 5s for startup)
sleep 5
curl -s http://localhost:8133/health | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'Status: {d.get(\"status\")} | gen={d.get(\"ralph_gen\",0)} | ipc={d.get(\"ipc_state\")}')
"

# 7. Verify hooks
echo '{}' | curl -s -X POST http://localhost:8133/hooks/SessionStart \
  -H 'Content-Type: application/json' -d @- | python3 -c "
import sys, json
d = json.load(sys.stdin)
print(f'Hook response keys: {list(d.keys())}')
"
```

### 4.3 Monthly Maintenance

```bash
echo "=== Monthly ORAC Maintenance $(date +%Y-%m-%d) ==="

# 1. Staleness canaries — verify doc matches code
cd /home/louranicas/claude-code-workspace/orac-sidecar

echo "Source files:"
ls src/lib.rs src/bin/*.rs src/m1_core/*.rs src/m2_wire/*.rs src/m3_hooks/*.rs \
   src/m4_intelligence/*.rs src/m5_bridges/*.rs src/m6_coordination/*.rs \
   src/m7_monitoring/*.rs src/m8_evolution/*.rs 2>/dev/null | wc -l

echo "Total LOC:"
wc -l src/lib.rs src/bin/*.rs src/m1_core/*.rs src/m2_wire/*.rs src/m3_hooks/*.rs \
   src/m4_intelligence/*.rs src/m5_bridges/*.rs src/m6_coordination/*.rs \
   src/m7_monitoring/*.rs src/m8_evolution/*.rs 2>/dev/null | tail -1

echo "Test count:"
rg '#\[test\]' src/ --count-matches 2>/dev/null | \
  awk -F: '{sum+=$2} END {print sum}'

# 2. Blackboard ghost prune (keep last 100)
sqlite3 ~/.local/share/orac/blackboard.db \
  "DELETE FROM ghost_traces WHERE rowid NOT IN (SELECT rowid FROM ghost_traces ORDER BY rowid DESC LIMIT 100);"

# 3. Blackboard vacuum
sqlite3 ~/.local/share/orac/blackboard.db "VACUUM;"

# 4. Check POVM pathway growth
curl -s http://localhost:8125/pathways 2>/dev/null | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    paths = d if isinstance(d, list) else d.get('pathways', [])
    print(f'POVM pathways: {len(paths)}')
except:
    print('POVM: unreachable')
"

# 5. Check blackboard table sizes
for table in pane_status task_history agent_cards ghost_traces consent_declarations \
             consent_audit hebbian_summary ralph_state sessions coupling_weights; do
  count=$(sqlite3 -readonly ~/.local/share/orac/blackboard.db "SELECT COUNT(*) FROM $table;" 2>/dev/null || echo "N/A")
  echo "  $table: $count rows"
done

# 6. Check DB file sizes
echo "DB sizes:"
ls -lh ~/.local/share/orac/blackboard.db 2>/dev/null
ls -lh /home/louranicas/claude-code-workspace/orac-sidecar/data/*.db 2>/dev/null
```

### 4.4 Dependency Maintenance

```bash
cd /home/louranicas/claude-code-workspace/orac-sidecar

# Check for outdated dependencies
cargo outdated 2>/dev/null || echo "Install cargo-outdated: cargo install cargo-outdated"

# Update patch versions (safe)
cargo update

# Rebuild and verify
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```

Critical pinned dependencies (do NOT blindly upgrade major versions):

```
axum       0.8.x    — API changes between 0.7 and 0.8 (handler signatures)
rusqlite   0.32.x   — Breaking changes in 0.33+ (connection API)
tokio      1.x      — Stable, safe to update within 1.x
thiserror  2.x      — Major version change from 1.x changed derive syntax
ureq       2.x      — Sync HTTP client; 3.x is async-only (breaking)
```

### 4.5 Key Environment Traps (Reference)

These traps have caused production incidents. Keep this list visible.

```
#   Trap                                      Correct Command
--  ----------------------------------------  ------------------------------------------
1   cp is aliased to cp -i (interactive)      /usr/bin/cp -f source dest
2   rm is aliased to trash                    /usr/bin/rm -f file
3   cat is aliased to batcat                  /usr/bin/cat file (or use Read tool)
4   grep is aliased to rg                     /usr/bin/grep for POSIX syntax
5   find is aliased to fd                     /usr/bin/find for standard syntax
6   pkill exits 144, kills && chains          Separate commands: pkill foo; sleep 1; next
7   Reasoning Memory accepts TSV only         Never send JSON to :8130
8   Bridge addresses: raw host:port           Never include http:// prefix
9   POVM is write-only by default             Must call /hydrate to read back
10  devenv stop leaves processes alive         Kill port occupants before restart
11  stdout in daemons causes SIGPIPE death     Always redirect: > /tmp/orac-session.log 2>&1
12  Lock ordering: AppState before BusState    Violating order causes deadlock
```

---

*Generated: 2026-03-25 | Source: `/home/louranicas/claude-code-workspace/orac-sidecar/`*
*Obsidian: `[[ORAC Sidecar — Operations Runbook]]`*
