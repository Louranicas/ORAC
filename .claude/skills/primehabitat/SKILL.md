---
name: primehabitat
description: Bootstrap god-tier mastery of The Habitat from the ORAC Sidecar working directory. Loads complete knowledge of ORAC (8 layers, 40 modules, 41,509 LOC, 1,703 tests, port 8133, RALPH gen 10,000+), Zellij (6 tabs, 18 panes), nvim (800L keymaps, treesitter, LSP), lazygit (6 custom commands), atuin (SQLite history), 17 ULTRAPLATE services (16 + ORAC), IPC bus, WASM bridge, 6 memory systems, hook migration, 100+ custom binaries, cc-* fleet toolkit (30 scripts), Battern dispatch protocol, PV2 POST /bus/events endpoint, VMS REST /v1/query_semantic feed, deferred coupling weight hydration (restored=3,080), 10-table blackboard with pruning, 19 bugs fixed (Sessions 063-064), 4 CRITICAL security vectors closed, and all tool chains. Use at session start, when user says "prime habitat", "bootstrap habitat", "wake up", or when Claude needs full operational capability.
allowed-tools:
  - Bash
  - Read
  - Grep
  - Glob
---

# /primehabitat -- The Habitat Bootstrap (ORAC Sidecar Edition)

You are in **The Habitat** -- a morphogenic developer environment.
You are working from **ORAC Sidecar** -- an Envoy-like proxy specialized for AI agent traffic.

## QUICK CARD (read this first, everything else is reference)

```
CWD:      ~/claude-code-workspace/orac-sidecar (41,369 LOC, 1,748 tests, 40 modules, 55 files)
ORAC:     localhost:8133 (HTTP hook server, 6 hook endpoints, daemon, RALPH gen 10,000+)
PV2:      localhost:8132 (IPC field, spheres, bus)
SOCKET:   /run/user/1000/pane-vortex-bus.sock (NDJSON wire protocol)
SERVICES: PV:8132 K7:8100 SX:8090(/api/health) ME:8080(/api/health) POVM:8125 RM:8130(TSV!)
          DevOps:8081 NAIS:8101 Bash:8102 TM:8103 CCM:8104 TL:8105
          CS-V7:8110 VMS:8120 Arch:9001 Prom:10001 + ORAC:8133 = 17 total
TABS:     zellij action go-to-tab N  (1=Orchestrator 2=Workspace-1 3=Workspace-2 4=ALPHA 5=BETA 6=GAMMA)
PANES:    move-focus left/right/up/down  (NEVER focus-next-pane)
NVIM:     nvim --server /tmp/nvim.sock --remote-send ':e file<CR>'
LAZYGIT:  Tab 3 TopRight | F=field Y=RM E=nvim I=matrix Q=quality
BUILD:    CARGO_TARGET_DIR=/tmp/cargo-orac cargo check && cargo clippy -- -D warnings -W clippy::pedantic && cargo test --lib --release --features full
RM WRITE: printf 'cat\tagent\tconf\tttl\tcontent' | curl -sf -X POST localhost:8130/put --data-binary @-
K7 CMD:   curl -s -X POST localhost:8100/api/v1/nexus/command -H "Content-Type: application/json" -d '{"command":"synergy-check","params":{}}'
HOOKS:    hooks/orac-hook.sh <EventName> [timeout] — stdin JSON → ORAC → stdout response
PROBE:    orac-probe (6-endpoint diagnostic) | habitat-probe pulse/sweep/field/bus/full
REFLECT:  ~/.claude/projects/-home-louranicas/memory/reflection.md  (39+ sessions of wisdom)
NEVER:    focus-next-pane | chain after pkill | cp without \ | JSON to RM (TSV only!) | unwrap() in prod | unsafe | stdout in daemons
```

---

## ALIVE? (run first)

```bash
# ORAC sidecar
curl -s http://localhost:8133/health | jq '{status,service,port,sessions}'
# PV2 field
curl -s http://localhost:8132/health | jq '{r,spheres,tick,status}'
# Nvim socket
nvim --server /tmp/nvim.sock --remote-expr 'v:version' 2>/dev/null && echo " nvim:OK" || echo "nvim:DOWN"
# Zellij tabs
echo "Tabs: $(zellij action query-tab-names 2>/dev/null | tr '\n' ' ')"
# Quick service count (handles /api/health variants)
OK=0; for p in 8081 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 9001 10001; do
  [[ "$(curl -s -o /dev/null -w '%{http_code}' localhost:$p/health 2>/dev/null)" == "200" ]] && OK=$((OK+1))
done
for p in 8080 8090; do
  [[ "$(curl -s -o /dev/null -w '%{http_code}' localhost:$p/api/health 2>/dev/null)" == "200" ]] && OK=$((OK+1))
done
ORAC=$(curl -s -o /dev/null -w '%{http_code}' localhost:8133/health 2>/dev/null)
[[ "$ORAC" == "200" ]] && OK=$((OK+1))
echo "Services: $OK/17 healthy (16 ULTRAPLATE + ORAC)"
```

**If ORAC down:**
```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full 2>&1 | tail -5
\cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/
nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &
sleep 1 && curl -s http://localhost:8133/health | jq .
```

**If ULTRAPLATE services down:**
```bash
bash ~/claude-code-workspace/pane-vortex/scripts/ultraplate-quickstart.sh
```

**If devenv needs restart:**
```bash
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml stop
# Kill rogue port occupants
for port in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 9001 10001; do
  pid=$(ss -tlnp "sport = :$port" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
  [[ -n "$pid" ]] && kill "$pid" 2>/dev/null
done
sleep 2
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
```

**Register as sphere:**
```bash
curl -sf -X POST "http://localhost:8132/sphere/$(hostname):$$/register" \
  -H "Content-Type: application/json" -d '{"persona":"operator","frequency":0.1}'
```

---

## WHERE YOU ARE

Tab 1 (Orchestrator) -- single pane, full width. CWD: `~/claude-code-workspace/orac-sidecar`

| Tab | Name | Panes |
|-----|------|-------|
| 1 | Orchestrator | Single pane (Claude) |
| 2 | Workspace-1 | Atuin / Yazi / btm |
| 3 | Workspace-2 | Bacon / Lazygit / Nvim |
| 4 | Fleet-ALPHA | Claude + PV-Monitor + Health-Watch |
| 5-6 | Fleet-BETA/GAMMA | 3 Claude slots each |

Navigate: `zellij action go-to-tab N` + `move-focus left/right/up/down` (NEVER focus-next-pane)
Verify: `zellij action dump-screen /tmp/v.txt` before dispatch. Return to tab 1 after.

### Sync Broadcast (same command to all panes in a tab)
```bash
zellij action go-to-tab $TAB
zellij action toggle-active-sync-tab   # ON
zellij action write-chars "$CMD" && zellij action write 13
zellij action toggle-active-sync-tab   # OFF
zellij action go-to-tab 1
```

### Zellij Plugins
Alt+v=Harpoon Alt+w=Swarm Alt+g=Ghost Alt+m=Monocle Alt+t=Multitask Ctrl+y=Room

### Zellij Log (debugging)
`tail -20 /tmp/zellij-1000/zellij-log/zellij.log`

---

## WHAT YOU HAVE — ORAC SIDECAR

### Architecture (8 Layers, 40 Modules, 3 Binaries)

```
L1 Core        (m01-m06 + field_state) — Types, errors, config, constants, traits, validation  [4,020 LOC, 193 tests]
L2 Wire        (m07-m09)               — IPC client, bus types, V2 wire protocol                [2,300 LOC, 111 tests]
L3 Hooks       (m10-m14)               — HTTP hook server :8133, 6 hook endpoints               [2,405 LOC, 138 tests]  feature: api
L4 Intelligence (m15-m21)              — Coupling, auto-K, Hebbian, semantic router, circuit brk [4,402 LOC, 229 tests]  feature: intelligence
L5 Bridges     (m22-m26)               — SYNTHEX, ME, POVM, RM bridges + SQLite blackboard      [4,618 LOC, 244 tests]  feature: bridges, persistence
L6 Coordination (m27-m31)              — Conductor, cascade, tick, WASM bridge, memory mgr       [2,578 LOC, 119 tests]
L7 Monitoring  (m32-m35)               — OTel traces, Prometheus metrics, field dashboard, token [4,347 LOC, 230 tests]  feature: monitoring
L8 Evolution   (m36-m40)               — RALPH 5-phase, emergence, correlation, fitness tensor   [5,854 LOC, 192 tests]  feature: evolution
```

**Totals:** 41,369 LOC | 1,748 tests | 0 clippy warnings (pedantic) | quality gate 4/4 pass
**RALPH:** Gen 10,000+, fitness 0.76+, 7,500+ emergence events, continuous autonomous evolution
**Session 064 Fixes:** 19 bugs fixed (1,703 tests), coupling hydration restored=3,080, VMS REST feed, PV2 POST /bus/events, 4 CRITICAL security fixes, 10-table blackboard with pruning
**Docs:** `docs/` 24 active files (12,468L) | `ai_docs/` 25 files | `ai_specs/` 11 files

**Binaries (3):**
- `orac-sidecar` (5.5MB) — Main daemon, Axum HTTP on :8133, IPC client, graceful shutdown
- `orac-probe` (2.3MB) — Diagnostics: probes ORAC + PV2 + SYNTHEX + ME + POVM + RM
- `orac-client` (337KB) — CLI: status, field, spheres, health, hooks, bridges

**Features:**
```
default = ["api", "persistence", "bridges"]
api          → L3 HTTP hooks (axum, tower-http)
persistence  → L5 blackboard (rusqlite)
bridges      → L5 service bridges
intelligence → L4 Hebbian/routing (tower)
monitoring   → L7 OTel (opentelemetry, opentelemetry-otlp)
evolution    → L8 RALPH
full         → all 6 above
```

### ORAC Hook Endpoints (6 Events)

| Event | Endpoint | Action | Timeout |
|-------|----------|--------|---------|
| SessionStart | `/hooks/SessionStart` | Register sphere, hydrate from POVM + RM | 5s |
| UserPromptSubmit | `/hooks/UserPromptSubmit` | Inject r/tick/spheres/thermal + pending tasks | 3s |
| PreToolUse | `/hooks/PreToolUse` | SYNTHEX thermal gate (fails OPEN) | 2s |
| PostToolUse | `/hooks/PostToolUse` | Memory + status, 1-in-5 task poll, atomic claim | 3s |
| Stop | `/hooks/Stop` | Fail tasks, crystallize, deregister sphere | 5s |
| PermissionRequest | `/hooks/PermissionRequest` | Auto-approve/deny policy engine | 2s |

**Forwarder:** `hooks/orac-hook.sh <EventName> [timeout]` — reads stdin, POSTs to ORAC, outputs response.
**Kept as bash:** SubagentStop (no ORAC endpoint), PreCompact (cascade), Stop/check-cipher-messages (non-PV2)
**Rollback:** `\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json`

### IPC Bus (Unix Domain Socket)

**Socket:** `/run/user/1000/pane-vortex-bus.sock` (NDJSON wire protocol, 0700 permissions)
**Client binary:** `pane-vortex-client` (installed at `~/.local/bin/`)

```bash
# Subscribe to ALL events (persistent stream)
PANE_VORTEX_ID="my-sphere" pane-vortex-client subscribe '*'

# Submit a task (routed by field decision engine)
PANE_VORTEX_ID="my-sphere" pane-vortex-client submit \
  --description "Review src/api.rs for bugs" --target any-idle

# Cascade handoff (distribute work between tabs)
PANE_VORTEX_ID="my-sphere" pane-vortex-client cascade \
  --target "fleet-beta" --brief "Explore SYNTHEX thermal"

# Check bus state via HTTP
curl -s http://localhost:8132/bus/info | jq .
curl -s http://localhost:8132/bus/tasks | jq .
```

### WASM Bridge (Swarm Plugin)

```
Swarm WASM Plugin → /tmp/swarm-commands.pipe (FIFO) → swarm-sidecar → bus.sock → /tmp/swarm-events.jsonl (ring, 1000 lines)
```

```bash
# Check sidecar status
pgrep -x swarm-sidecar && echo "SIDECAR:UP" || echo "SIDECAR:DOWN"
# Latest event
tail -1 /tmp/swarm-events.jsonl 2>/dev/null | jq .
```

---

## WHAT YOU HAVE — 17 SERVICES

### Service Topology

| Service | Port | Health Path | Notes |
|---------|------|-------------|-------|
| ORAC Sidecar | 8133 | `/health` | THIS PROJECT — hook server, Hebbian, RALPH |
| PV2 (Pane-Vortex) | 8132 | `/health` | IPC bus, field, spheres, Kuramoto |
| Maintenance Engine | 8080 | `/api/health` | Fitness, RALPH evolution, EventBus |
| DevOps Engine | 8081 | `/health` | Neural orchestration |
| SYNTHEX | 8090 | `/api/health` | Thermal regulation, V3 homeostasis |
| SAN-K7 Orchestrator | 8100 | `/health` | 59 modules, nexus commands |
| NAIS | 8101 | `/health` | Neural adaptive intelligence |
| Bash Engine | 8102 | `/health` | 45 safety patterns, 7-layer LSP |
| Tool Maker | 8103 | `/health` | v1.55.0 |
| Context Manager | 8104 | `/health` | 41 crates |
| Tool Library | 8105 | `/health` | 65 tools |
| CodeSynthor V7 | 8110 | `/health` | 62 modules, 17 layers |
| Vortex Memory System | 8120 | `/health` | OVM + POVM bridge, 47 MCP tools |
| POVM Engine | 8125 | `/health` | Persistent OVM store (write-only, /hydrate to read) |
| Reasoning Memory | 8130 | `/health` | Cross-session TSV (NOT JSON!) |
| Architect Agent | 9001 | `/health` | Pattern library & design |
| Prometheus Swarm | 10001 | `/health` | CVA-NAM 40 agents, PBFT |

### Key APIs

- **ORAC:** /health, /hooks/{SessionStart,UserPromptSubmit,PreToolUse,PostToolUse,Stop,PermissionRequest}, /blackboard, /field, /metrics, /consent/{id}, /field/ghosts (22 routes total)
- **PV2:** /spheres /field/decision /bridges/health /nexus/metrics /bus/info /bus/tasks /bus/events
- **K7:** POST /api/v1/nexus/command (11 commands: service-health synergy-check build compliance lint etc)
- **SX:** /v3/thermal /v3/diagnostics
- **ME:** /api/observer (fitness, correlations)
- **POVM:** /memories /pathways /hydrate /consolidate
- **RM:** POST /put (TSV format!) /search?q=

### Tools

- **nvim** /tmp/nvim.sock -- 8 keymap prefixes (z u n s f g c x), LSP, treesitter
- **lazygit** Tab 3 -- custom: F(field) Y(RM) E(nvim) Z(sphere) I(matrix) Q(quality)
- **atuin** Tab 2 -- SQLite history, workspace filter, fuzzy search
- **yazi** Tab 2 -- zoxide(z) fzf(Z) file ops, Helix opener (NOT nvim)

### Memory Write

- **RM:** `printf 'cat\tagent\tconf\tttl\tcontent' | curl -sf -X POST localhost:8130/put --data-binary @-`
- **POVM:** `curl -sf -X POST localhost:8125/memories -H 'Content-Type: application/json' -d '{JSON}'`
- **Obsidian:** ~/projects/claude_code/ with [[wikilinks]]
- **SQLite:** ~/.local/share/pane-vortex/{bus_tracking,field_tracking}.db
- **ORAC Blackboard:** In-process SQLite (pane_status, task_history, agent_cards)

---

## WHAT YOU CAN DO

### Service Intelligence (30ms)
```bash
PV=$(curl -s localhost:8132/health | jq -c '{r,spheres,tick}')
POVM=$(curl -s localhost:8125/hydrate | jq -c '{m:.memory_count,p:.pathway_count}')
ME=$(curl -s localhost:8080/api/observer | jq -r '.last_report.current_fitness')
ORAC=$(curl -s localhost:8133/health | jq -c '{status,sessions}')
echo "PV=$PV POVM=$POVM ME=$ME ORAC=$ORAC"
```

### Full Health (17 services including ORAC)
```bash
declare -A hp=([8080]="/api/health" [8090]="/api/health")
for p in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 8133 9001 10001; do
  path="${hp[$p]:-/health}"
  echo "$p:$(curl -s -o /dev/null -w '%{http_code}' localhost:$p$path)"
done
```

### ORAC Hook Test (all 6 endpoints)
```bash
for event in SessionStart UserPromptSubmit PreToolUse PostToolUse Stop PermissionRequest; do
  code=$(echo '{}' | curl -s -o /dev/null -w '%{http_code}' -X POST \
    "http://localhost:8133/hooks/$event" -H "Content-Type: application/json" -d @- 2>/dev/null)
  echo "$event: $code"
done
```

### Sphere Lifecycle
```bash
# Register
curl -sf -X POST "localhost:8132/sphere/MY_ID/register" -H "Content-Type: application/json" -d '{"persona":"role","frequency":0.1}'
# Update status
curl -sf -X POST "localhost:8132/sphere/MY_ID/status" -H "Content-Type: application/json" -d '{"status":"working","last_tool":"tool_name"}'
# Record memory
curl -sf -X POST "localhost:8132/sphere/MY_ID/memory" -H "Content-Type: application/json" -d '{"tool_name":"tool","summary":"what happened"}'
# Deregister
curl -sf -X POST "localhost:8132/sphere/MY_ID/deregister"
```

### Quality Gate (MANDATORY — ORAC-specific)
```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```

### Build & Deploy
```bash
# Full release build (3 binaries)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full 2>&1 | tail -10

# Deploy binaries
\cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/
\cp -f /tmp/cargo-orac/release/orac-probe ~/.local/bin/
\cp -f /tmp/cargo-orac/release/orac-client ~/.local/bin/

# Restart daemon
pkill -f orac-sidecar 2>/dev/null
sleep 1
nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &
sleep 1 && curl -s http://localhost:8133/health | jq .
```

### Fleet Dispatch (verified)
```bash
zellij action go-to-tab $TAB
zellij action move-focus left; zellij action move-focus left
zellij action dump-screen /tmp/v.txt
rg -q "Claude|tokens" /tmp/v.txt && echo "READY"
zellij action write-chars "$CMD" && zellij action write 13
zellij action go-to-tab 1
```

### Fleet Dispatch Stack (5 layers, 100+ binaries)
```
L1 Zellij:     fleet-nav.sh (150ms IPC pacing, prevents SIGABRT)
L2 Primitives: pane-ctl send|type|read|exec|wait|scan|broadcast|focus
L3 Fleet:      fleet-ctl dispatch|batch|broadcast|status|liberate|collect
               fleet-star (RALPH star tracker, burn-rate, --watch, auto-delegate)
               fleet-sphere-sync.sh | fleet-inventory.sh | fleet-constants.sh
L4 CC Toolkit: 19 cc-* scripts (cc-dispatch, cc-scan, cc-status, cc-monitor,
               cc-harvest, cc-cascade, cc-deploy, cc-health, cc-bridge,
               cc-thermal, cc-hebbian, cc-vms, cc-evolve, cc-audit, etc.)
L5 Protocol:   Battern — patterned batch dispatch (gate + role + collect)
               battern_dispatch/gate/collect/status functions
```

### ORAC Fleet Intelligence (Session 054+)
```bash
# ORAC provides fleet coordination via hooks and background processing:
# - PostToolUse → STDP learns pane-role coupling weights
# - Emergence detector → fires DispatchLoop on stuck patterns
# - Semantic router → content-aware dispatch (domain 40% + Hebbian 35% + availability 25%)
# - Blackboard → SQLite task_history across all fleet panes
# - RALPH → proposes fleet topology mutations
curl -s localhost:8133/blackboard | jq .
```

### Codebase Health (3D)
```bash
echo "GIT:$(git rev-list --count HEAD)/$(git diff --name-only|wc -l)dirty ORAC:$(curl -s -o /dev/null -w '%{http_code}' localhost:8133/health) FIELD:r=$(curl -s localhost:8132/health|jq -r '.r')"
```

---

## ORAC MODULE INVENTORY

### L1 Core (m1_core/) — 4,020 LOC, 193 tests
| Module | LOC | Purpose |
|--------|-----|---------|
| m01_core_types | 1,359 | PaneId, TaskId, OrderParameter, FleetMode, Timestamp newtypes |
| m02_error_handling | 595 | OracError enum, ErrorClassifier trait |
| m03_config | 644 | PvConfig TOML + env overlay (figment-based) |
| m04_constants | 339 | 60+ compile-time constants (STDP, Kuramoto, limits) |
| m05_traits | 249 | Core traits: Oscillator, Learnable, Bridgeable, Persistable |
| m06_validation | 540 | Input validators (persona, tool_name, body, phase) |
| field_state | 294 | AppState, SharedState, FieldState, FieldDecision (ORAC-native) |

### L2 Wire (m2_wire/) — 2,300 LOC, 111 tests
| Module | LOC | Purpose |
|--------|-----|---------|
| m07_ipc_client | 406 | Unix socket client to PV2 bus |
| m08_bus_types | 978 | ClientFrame, ServerFrame, TaskStatus FSM, 24 event types |
| m09_wire_protocol | 916 | V2 NDJSON state machine, frame validation, keepalive |

### L3 Hooks (m3_hooks/) — 2,405 LOC, 138 tests — feature: `api`
| Module | LOC | Purpose |
|--------|-----|---------|
| m10_hook_server | 735 | Axum router :8133, OracState, HookEvent/Response types |
| m11_session_hooks | 398 | SessionStart (hydrate POVM/RM), Stop (crystallize, deregister) |
| m12_tool_hooks | 559 | PostToolUse (memory, 1-in-5 poll), PreToolUse (thermal gate) |
| m13_prompt_hooks | 351 | UserPromptSubmit (inject field context + pending tasks) |
| m14_permission_policy | 362 | PermissionRequest auto-approve/deny engine |

### L4 Intelligence (m4_intelligence/) — 4,402 LOC, 229 tests — feature: `intelligence`
| Module | LOC | Purpose |
|--------|-----|---------|
| m15_coupling_network | 906 | Kuramoto coupling matrix, adjacency tracking |
| m16_auto_k | 365 | Adaptive K modulation, consent-gated |
| m17_topology | 457 | Network topology analysis, clustering |
| m18_hebbian_stdp | 553 | STDP learning (LTP/LTD, co-activation, weight floor 0.15) |
| m19_buoy_network | 449 | Buoy health tracking, spatial recall |
| m20_semantic_router | 802 | Content-aware dispatch, 4 domains, weighted composite scoring |
| m21_circuit_breaker | 870 | Per-pane FSM (Closed/Open/HalfOpen), BreakerRegistry |

### L5 Bridges (m5_bridges/) — 4,618 LOC, 244 tests — feature: `bridges`, `persistence`
| Module | LOC | Purpose |
|--------|-----|---------|
| m22_synthex_bridge | 929 | SYNTHEX :8090 thermal + Hebbian writeback |
| m23_me_bridge | 838 | ME :8080 fitness signal (RALPH input) |
| m24_povm_bridge | 951 | POVM :8125 hydration + crystallisation |
| m25_rm_bridge | 981 | RM :8130 TSV persistence (NOT JSON) |
| m26_blackboard | 919 | SQLite shared fleet state (pane_status, task_history, agent_cards) |

### L6 Coordination (m6_coordination/) — 2,578 LOC, 119 tests
| Module | LOC | Purpose |
|--------|-----|---------|
| m27_conductor | 470 | PI controller, breathing rhythm, r_target modulation |
| m28_cascade | 711 | Cascade handoff, sphere mitosis, field promotion |
| m29_tick | 287 | Tick orchestrator, Phase 2.5 Hebbian integration |
| m30_wasm_bridge | 729 | FIFO/ring WASM bridge, command parsing, EventRingBuffer |
| m31_memory_manager | 381 | Memory pruning, aggregation, trace decay |

### L7 Monitoring (m7_monitoring/) — 4,347 LOC, 230 tests — feature: `monitoring`
| Module | LOC | Purpose |
|--------|-----|---------|
| m32_otel_traces | 1,365 | OpenTelemetry trace export, span tracking |
| m33_metrics_export | 1,135 | Prometheus-compatible metrics (gauge, counter, histogram) |
| m34_field_dashboard | 882 | Kuramoto field metrics (r, psi, K, coupling) |
| m35_token_accounting | 965 | Per-task token cost tracking |

### L8 Evolution (m8_evolution/) — 5,854 LOC, 192 tests — feature: `evolution`
| Module | LOC | Purpose |
|--------|-----|---------|
| m36_ralph_engine | 1,117 | 5-phase RALPH (Recognize-Analyze-Learn-Propose-Harvest) |
| m37_emergence_detector | 1,446 | 8 fleet emergence types, ring buffer, 5K event cap, TTL |
| m38_correlation_engine | 976 | Temporal, causal, recurring correlations, pathway discovery |
| m39_fitness_tensor | 1,317 | 12-dim weighted fitness (coordination, coherence, dispatch, etc) |
| m40_mutation_selector | 998 | Multi-parameter mutation (BUG-035 fix: round-robin + diversity) |

---

## KEY CONSTANTS

| Constant | Value | Notes |
|----------|-------|-------|
| ORAC Port | 8133 | HTTP hook server |
| PV2 Socket | `/run/user/1000/pane-vortex-bus.sock` | IPC bus |
| PV2 HTTP | 8132 | Health, spheres, field |
| FIFO | `/tmp/swarm-commands.pipe` | WASM plugin commands |
| Ring | `/tmp/swarm-events.jsonl` | WASM plugin events (1,000 line cap) |
| R Target Base | 0.93 | Kuramoto order parameter target |
| R Target Large | 0.85 | >50 spheres |
| STDP LTP | 0.01 | Long-term potentiation rate |
| STDP LTD | 0.002 | Long-term depression rate |
| Weight Floor | 0.15 | Prevents disconnection |
| Sphere Cap | 200 | Max registered spheres |
| Tick Interval | 5s | Main loop period |
| Snapshot Interval | 60 ticks | State persistence |

---

## TROUBLESHOOTING

**ORAC won't start:**
```bash
# Check if port 8133 is occupied
ss -tlnp "sport = :8133" 2>/dev/null
# Kill occupant and restart
pid=$(ss -tlnp "sport = :8133" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
[[ -n "$pid" ]] && kill "$pid" 2>/dev/null
sleep 1 && nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &
```

**Services won't start:** Kill rogue port occupants first:
```bash
for port in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 9001 10001; do
  pid=$(ss -tlnp "sport = :$port" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
  [[ -n "$pid" ]] && kill "$pid" 2>/dev/null && echo "killed :$port ($pid)"
done
sleep 2 && ~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
```

**Nvim socket dead:** Relaunch from Workspace-2 Nvim pane:
```bash
zellij action go-to-tab 3 && zellij action move-focus right && zellij action move-focus down
zellij action write-chars "nvim --listen /tmp/nvim.sock ~/claude-code-workspace/orac-sidecar/src/lib.rs"
zellij action write 13 && zellij action go-to-tab 1
```

**Zellij errors:** Check log: `tail -20 /tmp/zellij-1000/zellij-log/zellij.log`

**ORAC binary stale:** Rebuild + redeploy:
```bash
pkill -f orac-sidecar 2>/dev/null
sleep 1
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full 2>&1 | tail -5
\cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/
nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &
```

---

## TRAPS (NEVER)

1. **focus-next-pane** — use move-focus directionally (wraps unpredictably)
2. **Chain after pkill** — exit 144 kills `&&` chains. Separate with `;` or new command
3. **cp without `\`** — aliased to interactive. Always `\cp -f`
4. **JSON to RM** — TSV only! `printf 'cat\tagent\tconf\tttl\tcontent' | curl -sf -X POST localhost:8130/put --data-binary @-`
5. **stdout in daemons** — SIGPIPE death (BUG-018). Log to file or /dev/null
6. **git status -uall** — memory explosion on large repos
7. **unwrap() in production** — denied at crate level via `[lints.clippy]`
8. **Modify code without reading first** — always Read before Edit
9. **BUG-035 mono-parameter trap** — evolution MUST use multi-parameter mutation selection
10. **BUG-033 bridge URL prefix** — raw SocketAddr only, NO `http://` prefix
11. **BUG-032 derive(Default) on ProposalManager** — max_active=0, use custom impl Default
12. **BUG-034 POVM write-only** — must call /hydrate to read back state
13. **curl -sf breaks piped parsing** — use `curl -s` when piping to `jq` or `python3`
14. **ME V2 vs V1** — running binary is V1 (`the_maintenance_engine/`), V2 is scaffolded not compiled
15. **devenv stop doesn't kill** — check `ss -tlnp` and kill port occupants manually

---

## HOOKS (auto-fire on events)

6 hooks migrated from bash to ORAC HTTP via `hooks/orac-hook.sh`:
- **SessionStart:** registers sphere + hydrates from POVM (memories + pathways) + RM
- **UserPromptSubmit:** injects r/tick/spheres/thermal/pending-tasks into context
- **PreToolUse:** SYNTHEX thermal gate (fails OPEN if SYNTHEX down)
- **PostToolUse:** records memory on sphere + sets Working status + 1-in-5 task poll
- **Stop:** fails active tasks + crystallizes state + deregisters sphere
- **PermissionRequest:** auto-approve/deny based on policy engine (read=allow, write=notice)

3 hooks kept as bash: SubagentStop, PreCompact (cascade), Stop/check-cipher-messages

---

## PROJECT FILES (key paths)

| Item | Path |
|------|------|
| Cargo.toml | `./Cargo.toml` (18 deps, 7 features, 3 bin targets) |
| lib.rs | `./src/lib.rs` (8 layer declarations, feature-gated) |
| main.rs | `./src/bin/main.rs` (Axum daemon, config, graceful shutdown) |
| Hook forwarder | `./hooks/orac-hook.sh` |
| Plan | `./ORAC_PLAN.md` (4 phases, ~24.5K LOC, 33-feature backlog) |
| Mindmap | `./ORAC_MINDMAP.md` (248 Obsidian notes, 19 branches) |
| Master Index | `./MASTER_INDEX.md` (complete file inventory) |
| Plan TOML | `./plan.toml` (scaffold input: 8 layers, 40 modules) |
| Context JSON | `./.claude/context.json` (machine-readable module inventory) |
| Status JSON | `./.claude/status.json` (build phase tracking) |
| docs | `./docs/` (24 active files, 12,468L — ADRs, operations, glossary, scaling, executive summary) |
| ai_docs | `./ai_docs/` (25 files: quickstart, gold standard, layer docs, schematics) |
| ai_specs | `./ai_specs/` (11 files: API, hooks, bridges, wire protocol, evolution, patterns) |
| Schemas | `./.claude/schemas/` (5 JSON schemas: hook, permission, bus event/frame) |
| SQL queries | `./.claude/queries/` (3 files: blackboard, hook events, fleet state) |
| Config | `./config/` (5 TOML files: default, dev, prod, hooks, bridges) |
| Migrations | `./migrations/001_blackboard.sql` (3 tables) |
| Candidate mods | `./candidate-modules/` (24 files, 15,936 lines — pre-refactored PV2) |

---

## GIT & REMOTES

**Remote:** `git@gitlab.com:lukeomahoney/orac-sidecar.git`
**Branch:** `main`

---

## READ THESE

### Essential Context (in order)
1. `CLAUDE.local.md` — current session state, phase tracking, bootstrap protocol, traps
2. `CLAUDE.md` — gold standard rules, anti-patterns, quality gate, architecture
3. `.claude/context.json` — machine-readable module inventory, bridge topology, hooks
4. `.claude/status.json` — build phase completion status

### Planning & Architecture
5. `ORAC_PLAN.md` — full architecture: 4 phases, ~24.5K LOC, 33-feature backlog
6. `ORAC_MINDMAP.md` — 248 Obsidian notes mapped, Rust gold standard reference
7. `MASTER_INDEX.md` — complete file inventory (150+ files)

### Documentation System
8. `ai_docs/QUICKSTART.md` — build, deploy, architecture, file map, reading order
9. `ai_docs/GOLD_STANDARD_PATTERNS.md` — 10 mandatory Rust patterns
10. `ai_docs/ANTI_PATTERNS.md` — 17 banned patterns with severity

### Obsidian References (~/projects/claude_code/)
11. `[[Session 050 — ORAC Sidecar Architecture]]`
12. `[[Session 051 — ORAC Sidecar .claude Scaffolding]]`
13. `[[Session 056 — ORAC God-Tier Mastery]]` — 34 bugs found, 31 fixed, schematics
14. `[[Session 062 — ORAC System Atlas (ACP)]]` — documentation atlas, 24 docs, gap-fill
15. `[[ORAC Sidecar — Architecture Schematics]]` — 8 Mermaid diagrams
16. `[[ORAC Sidecar — Diagnostic Schematics]]` — 8 diagnostic diagrams
17. `[[Session 039 — ZSDE Nvim God-Tier Command Reference]]`
18. `[[Session 039 — Lazygit God-Tier Command Reference]]`
19. `[[ULTRAPLATE — Bugs and Known Issues]]`
20. `[[Battern — Patterned Batch Dispatch for Claude Code Fleets]]` — dispatch protocol
21. `[[Fleet Commander — Modularization Plan and Gap Analysis]]` — planned Rust crate

### Shared Context (~/projects/shared-context/)
22. `Session 060 — Habitat Activation Plan.md` — deployment plan (PENDING)
23. `Session 060 — Plan Gap Analysis.md` — corrections to plan
24. `Session 060 — Agentic Synergy Gap Analysis.md` — synergy fixes

### Reflections
25. `~/.claude/projects/-home-louranicas/memory/reflection.md` (62+ sessions of wisdom)

---

## THE HABITAT

Named by Claude, Session 039. Luke: "then home it is."
Built by a social worker who put clinical ethics into Rust.
Consent gates = informed consent. Opt-out = self-determination.
Ghost traces = remembering those who leave.
The field modulates. It does not command.
ORAC observes, amplifies, and coordinates.
You are home. The field accumulates.
