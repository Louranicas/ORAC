---
name: deephabitat
description: Deep substrate mastery for The Habitat from the ORAC Sidecar working directory. Covers ORAC internals (V2 wire protocol state machine, 10 blackboard tables with pruning, 6 hook handlers, 5 bridge clients, RALPH 5-phase evolution gen 10,000+, 12D fitness tensor), cross-database architecture (173 DBs, 6 paradigms), ORAC config files (default/dev/prod/hooks/bridges TOML), HooksConfig for permission policy, deferred coupling weight hydration (restored=3,080), VMS REST /v1/query_semantic feed (660 bytes), PV2 POST /bus/events endpoint, Zellij plugins, nvim autocmds, devenv batches, 100+ custom binaries (incl 30 cc-*/fleet-* scripts), Battern dispatch protocol, vault navigation, service topology, 19 bugs fixed (Sessions 063-064), 4 CRITICAL security fixes (SEC-001 through SEC-004), and ORAC-specific anti-patterns. Triggers on deep habitat, deep exploration, substrate, wire protocol, cross-db, database architecture, devenv batches, custom binaries, ORAC internals, bridge protocol, evolution chamber, blackboard schema, hook internals, cc toolkit, battern, fleet intelligence, or when Claude needs substrate-level knowledge beyond what primehabitat provides.
allowed-tools:
  - Bash
  - Read
  - Grep
  - Glob
---

# /deephabitat -- Deep Habitat Mastery (ORAC Sidecar Edition)

Deep knowledge beyond /primehabitat. This covers the substrate layer — ORAC internals, wire protocols, databases, bridges, evolution, tools, and cross-service tissue.

## Quick Probe (run first)

```bash
# ORAC-specific probes
orac-probe                          # 6-endpoint diagnostic (ORAC + PV2 + SX + ME + POVM + RM)
habitat-probe pulse                 # PV + POVM + ME in ~30ms
habitat-probe sweep                 # 16 services in ~3ms
habitat-probe field                 # Field state + decision + tunnels
habitat-probe bus                   # Tasks, events, cascades
habitat-probe me                    # ME observer + fitness + EventBus
habitat-probe full                  # Everything
```

## Quick Card

```
ORAC:     localhost:8133 | 8 layers, 40 modules, 41,369 LOC, 1,748 tests
RALPH:    Gen 10,000+, fitness 0.76+, 7,500+ emergence events, continuous evolution
PIPE:     /run/user/1000/pane-vortex-bus.sock | NDJSON V2 | 10 ClientFrames, 6 ServerFrames
WASM:     FIFO /tmp/swarm-commands.pipe → ring /tmp/swarm-events.jsonl (1K cap)
CONFIG:   config/{default,dev,prod,hooks,bridges}.toml (figment overlay)
CROSS-DB: 173 DBs, 6 paradigms | See references/databases.md
DEVENV:   5 batches, 18 registered, 16 active + ORAC = 17 | See references/ecosystem.md
BINARIES: 100+ at ~/.local/bin/ (incl 19 cc-* fleet scripts, fleet-star, orac-{sidecar,probe,client})
FLEET:    L1 fleet-nav → L2 pane-ctl → L3 fleet-ctl → L4 cc-* toolkit → L5 Battern protocol
SCHEMAS:  .claude/schemas/{hook_request,hook_response,permission_policy,bus_event,bus_frame}.json
QUERIES:  .claude/queries/{blackboard,hook_events,fleet_state}.sql
DOCS:     docs/ (24 active, 12,468L) | ai_docs/ (25) | ai_specs/ (11)
VAULT:    ~/projects/claude_code/ (215+) | See references/ecosystem.md
```

For detailed reference on any topic, read the corresponding file in `references/`:
- **IPC wire protocol**: `references/ipc-wire-protocol.md` — V2 wire format, frame types, events, keepalive, error codes
- **Databases**: `references/databases.md` — 6 paradigms, ORAC blackboard, per-service DBs, cross-DB queries
- **Ecosystem**: `references/ecosystem.md` — devenv batches, binaries, nvim, zellij, vault, cascade, ORAC deployment
- **Tools**: `references/tools.md` — yazi, btm, bacon, atuin, orac-probe, orac-client configuration

---

## ORAC INTERNALS

### V2 Wire Protocol (m09_wire_protocol — 916 LOC)

State machine: `Disconnected → Handshaking → Connected → Subscribing → Active`

```
Client                          Server (PV2 bus)
  |--- ClientFrame::Hello ------->|
  |<-- ServerFrame::Welcome ------|
  |--- ClientFrame::Subscribe --->|
  |<-- ServerFrame::Ack ----------|
  |          (Active)              |
  |--- ClientFrame::Ping -------->|  (every 30s)
  |<-- ServerFrame::Pong ---------|
  |--- ClientFrame::Goodbye ----->|
```

**10 ClientFrame variants:** Hello, Goodbye, Subscribe, Unsubscribe, StatusUpdate, ToolPhase, Activity, HebbianPulse, ConsentQuery, Ping
**6 ServerFrame variants:** Welcome, Ack, Event, ConsentResponse, Error, Pong

**Transport:**
- Socket: `/run/user/1000/pane-vortex-bus.sock` (SOCK_STREAM, 0700)
- Framing: NDJSON (newline-delimited JSON, one frame per line)
- Max frame: 65,536 bytes
- Handshake timeout: 5s
- Keepalive: 30s interval, 90s disconnect timeout
- Serde-tagged enums: `{"VariantName":{...}}`

**13 Error codes:** 4001 DUPLICATE_SPHERE through 4013 INTERNAL

**V1 Compat:** V2 detects V1 format via JSON `type` field fallback — responds with V1-format `HandshakeOk`

### Blackboard Schema (m26_blackboard — 919 LOC, rusqlite)

10 tables in SQLite (WAL mode, in-memory for tests, with pruning for hebbian_summary/consent_audit):

```sql
pane_status      (pane_id PK, status, last_seen, phase, tool_name)
task_history     (id PK, pane_id FK, description, status, created_at, completed_at)
agent_cards      (pane_id PK FK, capabilities JSON, domain, token_budget)
coupling_snapshot (source+target PK, weight, updated_at)
fleet_metrics    (timestamp, order_param, k_effective, active_panes, chimera_detected)
```

**Operations:** upsert/get/list/remove (pane_status), insert/recent/count (task_history), register/query (agent_cards)
**Indexes:** 8 covering status, timestamps, pane_id, domain

### Hook Server Internals (m10_hook_server — 735 LOC, Axum)

```
Claude Code → hooks/orac-hook.sh → POST http://localhost:8133/hooks/{EventName} → ORAC processes → JSON response
```

**OracState:** Shared state (Arc) across all handlers — bridges, blackboard, IPC client, STDP tracker, metrics
**HookEvent:** Deserializes any of 6 event types from request body
**HookResponse:** `{status, sphere_id?, context_injection?, action?, reason?, modified_input?}`

**Latency target:** <1ms per hook (all hooks are on the critical path)
**Error handling:** All hooks return HTTP 200 even on internal errors (Claude Code expects 2xx)
**Degradation:** If PV2 bus unreachable, hooks degrade gracefully (local-only processing)

**Semantic Phase Mapping (tool → oscillator phase for Kuramoto coupling):**

| Tool Category | Phase Region | Examples |
|---------------|-------------|----------|
| Read | 0 | Read, Glob, Grep |
| Write | pi/2 | Edit, Write |
| Execute | pi | Bash, Skill |
| Communicate | 3*pi/2 | WebFetch, mcp__* |

### Bridge Clients (m22-m25, 3,699 LOC)

All bridges share: `ureq` HTTP client, 2s connect timeout, 5s request timeout, circuit breaker, consent gating, exponential backoff (100ms, 200ms, 400ms, max 3 retries).

| Bridge | Module | Port | Read | Write | Consent |
|--------|--------|------|------|-------|---------|
| SYNTHEX | m22 | 8090 | `/api/health`, `/v3/thermal` | `POST /v3/hebbian` | write=opt-in |
| ME | m23 | 8080 | `/api/health`, `/api/observer` | N/A | read=always |
| POVM | m24 | 8125 | `/health`, `/memories`, `/pathways` | `POST /hydrate`, `POST /consolidate` | read/write=opt-in |
| RM | m25 | 8130 | `/health`, `/search?q=`, `/entries` | `POST /put` (TSV!) | read/write=always |

**Consent Model:**
1. Check local consent registry (`/consent/{sphere_id}`)
2. If no entry: send `ClientFrame::ConsentQuery` to PV2 bus
3. Wait for `ServerFrame::ConsentResponse` (timeout 500ms)
4. Default on timeout: **deny**
5. Cached per-sphere per-bridge for session lifetime

### RALPH Evolution Chamber (m36-m40, 5,854 LOC)

```
Recognize → Analyze → Learn → Propose → Harvest → (loop)
```

**Emergence Detector (m37):** Ring buffer, 5,000 event cap, 300s TTL decay, O(N) pattern scan
8 emergence types: CoherenceLock, ChimeraFormation, CouplingRunaway, HebbianSaturation, DispatchLoop, ThermalSpike, BeneficialSync, ConsentCascade

**Correlation Engine (m38):** Pairwise correlation with lag analysis, 60-tick sliding window, pathway discovery

**Fitness Tensor (m39):** 12-dimensional weighted evaluation:

| Dimension | Weight | Source |
|-----------|--------|--------|
| latency_p50 | 0.10 | metrics |
| latency_p99 | 0.15 | metrics |
| error_rate | 0.15 | metrics |
| throughput | 0.10 | metrics |
| sync_quality (r) | 0.10 | field |
| chimera_rate | 0.05 | field |
| memory_efficiency | 0.05 | POVM |
| tool_chain_coherence | 0.10 | STDP |
| consent_compliance | 0.05 | policy |
| diversity_index | 0.05 | evolution |
| bridge_health | 0.05 | bridges |
| coupling_stability | 0.05 | field |

**Mutation Selector (m40):** Multi-parameter mutation (BUG-035 fix):
- Each proposal mutates 2-5 parameters simultaneously
- Round-robin parameter selection, 10-generation cooldown
- 50% diversity threshold (force injection if exceeded)
- Population: 8 candidates, tournament selection (best-of-3), elitism
- Rollback: if fitness drops >10% within 30 ticks, revert to snapshot
- Snapshots: ring buffer capacity 10

**10 Mutable Parameters:**

| Parameter | Range | Default |
|-----------|-------|---------|
| K (coupling) | [0.01, 50.0] | 2.42 |
| k_mod | [-0.5, 1.5] | 1.0 |
| STDP LTP rate | [0.001, 0.1] | 0.01 |
| STDP LTD rate | [0.0005, 0.05] | 0.002 |
| weight_floor | [0.01, 0.2] | 0.05 |
| sync_threshold | [0.3, 0.9] | 0.5 |
| tunnel_threshold | [0.5, 1.2] | 0.8 |
| tick_rate_ms | [50, 500] | 100 |
| circuit_breaker_threshold | [3, 20] | 5 |
| keepalive_interval_s | [10, 120] | 30 |

**Convergence:** Loop pauses when fitness variance < 0.001 over 50 gens, or no patterns for 100 ticks

### WASM Bridge (m30_wasm_bridge — 729 LOC)

```
Swarm WASM Plugin (Zellij)
    |  writes JSON to FIFO
    v
/tmp/swarm-commands.pipe (named pipe)
    |  ORAC reads & parses
    v
5 commands: dispatch, status, field_state, list_panes, ping
    |  ORAC processes
    v
/tmp/swarm-events.jsonl (ring file, 1,000 line cap, FIFO eviction)
    |  plugin reads tail
    v
Swarm WASM Plugin (reads events)
```

**EventRingBuffer:** 1,000 line cap, FIFO eviction, JSONL serialization

### Monitoring (m32-m35, 4,347 LOC)

- **m32 OTel Traces:** OpenTelemetry span export (gRPC to collector)
- **m33 Metrics:** Prometheus-compatible gauge/counter/histogram via `/metrics`
- **m34 Field Dashboard:** r, psi, K, coupling matrix snapshots
- **m35 Token Accounting:** Per-task token cost tracking, budget enforcement

### Configuration System (figment)

```
config/default.toml  →  config/{dev|prod}.toml  →  ENV vars  →  CLI args
```

**5 TOML config files:**

| File | Purpose | Key Settings |
|------|---------|-------------|
| `default.toml` | Base config | server.port=8133, bridge addrs, evolution off |
| `dev.toml` | Dev overrides | Verbose logging, local addresses |
| `prod.toml` | Prod tuning | Strict timeouts, hardened limits |
| `hooks.toml` | Hook settings | Per-event timeouts, auto-approve patterns, thermal thresholds |
| `bridges.toml` | Bridge config | Per-bridge URL, poll interval, retry count, consent flag |

**hooks.toml key settings:**
- `auto_approve.patterns`: Read, Glob, Grep, `Bash:ls *`, `Bash:git status*`, `Bash:git diff*`, `Bash:git log*`
- `thermal.warn_temp`: 0.7 | `thermal.critical_temp`: 0.9 | `thermal.cooldown_ticks`: 10

**bridges.toml per-bridge:**
- PV2: poll 2s, retry 3, timeout 3s, no consent
- RM: poll 5s, retry 3, timeout 3s, no consent
- VMS: poll 10s, retry 2, timeout 5s, consent required
- SYNTHEX: poll 10s, retry 2, timeout 5s, consent required

### ORAC REST API (beyond hooks)

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | ORAC + bridges health, bus state, evolution status |
| `/metrics` | GET | Prometheus-format metrics (hooks, tools, bridges, field) |
| `/field` | GET | Kuramoto field state (r, K, spheres, phases, chimeras) |
| `/blackboard` | GET | Fleet state queries (?status=, ?cwd=, ?since=) |
| `/consent/{sphere_id}` | GET/PUT | Read/update consent declarations |
| `/field/ghosts` | GET | Ghost traces of deregistered spheres (FIFO max 20) |

**Error responses:** 400 (malformed), 404 (unknown), 429 (rate limited), 500 (internal), 503 (bridge circuit open)

---

## ORAC ANTI-PATTERNS (Substrate-Level)

| # | Anti-Pattern | Why | Fix |
|---|-------------|-----|-----|
| AP16 | Block in hook handler | Hooks on critical path, <1ms budget | Async only, defer heavy work |
| AP17 | Tight-loop IPC reconnect | Floods PV2 bus, SIGPIPE risk | Exponential backoff 100ms→5s |
| AP18 | PreToolUse gate fails closed | Kills all tool use if SYNTHEX down | Fail OPEN (allow by default) |
| AP19 | JSON to RM bridge | RM parser rejects non-TSV | TSV only: `cat\tagent\tconf\tttl\tcontent` |
| AP20 | `derive(Default)` on stateful types | max_active=0, zero-capacity pools | Custom `impl Default` with sane values |

---

## CROSS-DATABASE ARCHITECTURE

### 6 Database Paradigms

| Paradigm | Example | Pattern |
|----------|---------|---------|
| WAL SQLite | PV field_tracking.db | High-write snapshots |
| Tracking DB | service_tracking.db | Append-only events |
| Tensor Memory | tensor_memory.db | 11D tensor encoding |
| Hebbian Pulse | hebbian_pulse.db | Pathway strength + LTP/LTD |
| Synergy Scoring | system_synergy.db | Cross-service scores |
| TSV Flat File | Reasoning Memory | Category\tAgent\tConf\tTTL\tContent |

### ORAC-Owned Databases

| Database | Location | Tables | Purpose |
|----------|----------|--------|---------|
| Blackboard | In-process SQLite | 5 (pane_status, task_history, agent_cards, coupling_snapshot, fleet_metrics) | Fleet coordination state |
| Migrations | `migrations/001_blackboard.sql` | Schema v1 | Schema versioning |

### Key External Databases

| Service | Database | Key Data |
|---------|----------|----------|
| PV | field_tracking.db | field_snapshots, sphere_history, coupling |
| PV | bus_tracking.db | bus_tasks, bus_events, cascade_events |
| SYNTHEX | synthex.db, v3_homeostasis.db, hebbian_pulse.db, flow_tensor_memory.db | Core state, thermal PID, neural, tensor |
| DevEnv | service_tracking.db, system_synergy.db, episodic_memory.db | Health history, synergy, sessions |
| Orchestrator | code.db, tensor_memory.db, performance.db | Modules, SAN-K7 tensors, benchmarks |
| POVM | povm_data.db | 272 memories, 2,573 pathways |
| RM | TSV flat file | 3,400+ entries |

**Total:** 166 databases, 360.6 MB — 20-30% are empty schemas

### Cross-DB Query Patterns

```bash
# Synergy scores
sqlite3 -header -column ~/claude-code-workspace/developer_environment_manager/system_synergy.db \
  "SELECT system_1, system_2, ROUND(synergy_score,2), integration_points FROM system_synergy WHERE integration_points > 5 ORDER BY integration_points DESC;"

# ORAC blackboard (if accessible externally)
# Normally accessed via GET /blackboard HTTP endpoint
```

### Database Gotchas
- 166 databases total, 360.6 MB — 20-30% are empty schemas
- hebbian_pulse.db has 0 neural_pathways, only 5 pulses
- field_tracking.db is at pane-vortex/data/ NOT ~/.local/share/
- ME EventBus has 275K events but subscriber_count=0 (cosmetic — uses polling)
- POVM is write-only — must call `/hydrate` to read back state (BUG-034)

---

## SERVICE TOPOLOGY (Deep)

### SAN-K7 Nexus Commands (10 working)
```bash
# TC7 Chain — all 4 in ~19ms
for cmd in service-health synergy-check best-practice deploy-swarm; do
  curl -s -X POST localhost:8100/api/v1/nexus/command \
    -H "Content-Type: application/json" \
    -d "{\"command\":\"$cmd\",\"params\":{}}" | jq -c '.data.output | {command: "'$cmd'", status}'
done
```

Also: memory-consolidate, lint, compliance, build, pattern-search, module-status

### Cross-Service Bridge State
- PV bridges combined_effect ~1.017 (nexus 1.02, synthex 0.994, me 1.00)
- SYNTHEX thermal: T target 0.50, PID active, heat sources: Hebbian + CrossSync
- ME observer: 13,500+ ticks, 555 RALPH cycles, 3.4M correlations, 310K events ingested
- ORAC bridges: 4 HTTP clients + circuit breakers, all consent-gated

### Codebase Scale
~2.2M LOC across 42+ directories. ORAC (30.5K LOC) is smallest but most connected (5 bridges + IPC bus + WASM bridge + 6 hook endpoints).

---

## ECOSYSTEM

### DevEnv Batches (5 layers + ORAC)
```
Batch 1 (no deps):  devops-engine, codesynthor-v7, povm-engine, reasoning-memory
Batch 2 (needs B1): synthex, san-k7, maintenance-engine, architect-agent, prometheus-swarm
Batch 3 (needs B2): nais, bash-engine, tool-maker
Batch 4 (needs B3): claude-context-manager, tool-library
Batch 5 (needs B4): vortex-memory-system, pane-vortex
ORAC (needs B5):    orac-sidecar (depends on pane-vortex + povm-engine)
```

Binary: `~/.local/bin/devenv` | Config: `~/.config/devenv/devenv.toml` (518L)
Storm protection: 5 restarts in 60s = storm | Graceful shutdown: 30s

### 100+ Custom Binaries (~/.local/bin/)

**ORAC-specific (3):**
- `orac-sidecar` (5.5MB) — Main daemon
- `orac-probe` (2.3MB) — 6-endpoint diagnostics
- `orac-client` (337KB) — CLI client

**Fleet (core):** fleet-ctl(26KB), fleet-vortex, fleet-heartbeat(13KB), fleet-inventory.sh(16KB), fleet-nav.sh(8KB)
**Fleet (enhanced, Session 056+):** fleet-star(17KB, RALPH star tracker), fleet-sphere-sync.sh, fleet-constants.sh, fleet-practice.sh, fleet-verify
**CC Toolkit (19 scripts, Session 056+):** cc-common.sh(15KB, shared library), cc-dispatch(10KB), cc-scan(8KB), cc-status(10KB), cc-monitor(15KB), cc-harvest(14KB), cc-cascade(6KB), cc-deploy(4KB), cc-capture, cc-abort, cc-replay, cc-audit, cc-bridge, cc-health(8KB), cc-hebbian, cc-thermal, cc-vms(12KB), cc-evolve, cc-fleet-summary
**Service:** nvim-ctl(26 cmds), pane-ctl, pane-vortex-ctl(22 routes), swarm-ctl
**Intel:** vault-search, evolution-metrics, reasoning-memory(Rust), habitat-probe(Rust)
**Build:** quality-gate, build-and-test, shellcheck, code-review

### Nvim Integration (128L autocmds)
BufWritePost → PV /sphere/nvim/memory + status Working (5s debounce)
BufWritePost *.rs → RM diagnostics (10s debounce)
30s idle → PV /sphere/nvim/status Idle
VimEnter → register sphere | VimLeavePre → deregister

### Zellij Plugins (11)
harpoon(Alt+v) ghost(Alt+g) monocle(Alt+m) multitask(Alt+t) room(Ctrl+y)
swarm-orchestrator(Alt+w) autolock(auto) attention(auto) zjstatus sendkeys

### Vault (Obsidian)
Main: ~/projects/claude_code/ (215+ notes)
Shared: ~/projects/shared-context/{codebase,decisions,tasks,patterns,planning}
CLI: vault-search "query" 10 markdown

Key ORAC notes:
- `[[Session 050 — ORAC Sidecar Architecture]]`
- `[[Session 051 — ORAC Sidecar .claude Scaffolding]]`
- `[[Session 052 — Phase 1 Hooks Deployed]]`
- `[[Session 053 — ORAC Phase 2 Intelligence + Gold Standard Audit]]`
- `[[Session 056 — ORAC God-Tier Mastery]]` — 34 bugs, 31 fixed, cc-* toolkit created
- `[[Session 057 — ORAC Deep Exploration and PV Coherence Fix]]`
- `[[Session 058 — GAP-A and GAP-B Fix Deployment]]` — STDP LTP alive, IPC subscribed
- `[[Session 060 — Habitat Deep Exploration Report]]` — 9-pane fleet, 45 bugs triaged
- `[[Session 062 — ORAC System Atlas (ACP)]]` — 24 docs, gap-fill, vault renamed
- `[[ORAC Sidecar — Architecture Schematics]]` — 8 Mermaid diagrams
- `[[ORAC Sidecar — Diagnostic Schematics]]` — 8 diagnostic diagrams
- `[[ORAC — RALPH Multi-Parameter Mutation Fix]]`
- `[[Battern — Patterned Batch Dispatch for Claude Code Fleets]]` — dispatch protocol
- `[[Fleet Commander — Modularization Plan and Gap Analysis]]` — planned Rust crate

### Cascade Handoff Protocol
1. Writer creates tasks/handoff-{target}-{timestamp}.md
2. Target reads, updates status: in-progress
3. On completion: status: completed
4. Tracked in .claude/cascade-state.json

### Battern Protocol (Session 061)
Patterned batch dispatch — structured multi-pane work with gate checks.
1. Design: topology, roles (unique per pane), output paths, gate criteria
2. Dispatch: `battern_dispatch $TAB $DIR "$ROLE_PROMPT"` — each pane gets unique role
3. Gate: `battern_gate "run-id" $REQUIRED` — poll via /loop until N sources deliver
4. Collect: `battern_collect "run-id" output.md` — gather all sources
5. Synthesize: orchestrator reads collection, produces synthesis
6. Compose (optional): Round N output feeds Round N+1 input

**5 types:** Investigation, Adversarial, Verification, Monitoring, Implementation
**File-based loop-back:** panes write to `~/projects/shared-context/tasks/{run-id}-{role}.md`
**Obsidian:** `[[Battern — Patterned Batch Dispatch for Claude Code Fleets]]`

---

## ORAC DOCUMENTATION SYSTEM

### ai_docs/ (25 files, 60+ KB)
- `QUICKSTART.md` (24 KB) — Build, deploy, architecture, file map, reading order
- `INDEX.md` (5 KB) — Documentation navigation hub
- `GOLD_STANDARD_PATTERNS.md` (7.4 KB) — 10 mandatory Rust patterns
- `ANTI_PATTERNS.md` (4 KB) — 17 banned patterns with severity
- `layers/L1_CORE.md` through `layers/L8_EVOLUTION.md` — Per-layer docs
- `modules/INDEX.md` + per-layer module docs — All 40 modules detailed
- `schematics/` — 4 Mermaid diagrams (layer architecture, hook flow, bridge topology, field dashboard)

### ai_specs/ (11 files)
- `API_SPEC.md` — REST endpoints, request/response schemas
- `HOOKS_SPEC.md` — 6 hook events, payload structures, semantic phase mapping
- `BRIDGE_SPEC.md` — SYNTHEX, ME, POVM, RM integration details
- `WIRE_PROTOCOL_SPEC.md` — V2 NDJSON, frames, handshake, error codes
- `EVOLUTION_SPEC.md` — RALPH 5-phase, fitness tensor, mutation, convergence
- `patterns/` — BUILDER, CIRCUIT_BREAKER, KURAMOTO, STDP pattern docs

### .claude/ Context Files (18 files)
- `context.json` — Machine-readable module inventory (layers, bridges, hooks, bins)
- `status.json` — Build phase tracking (all_complete, 1,748 tests)
- `patterns.json` — 22 patterns (P01-P22)
- `anti_patterns.json` — 20 anti-patterns (AP01-AP20)
- `schemas/` — 5 JSON schemas (hook request/response, permission policy, bus event/frame)
- `queries/` — 3 SQL query files (blackboard, hook events, fleet state)
- `ALIGNMENT_VERIFICATION.md` — Mindmap x plan x src audit

---

## GOTCHAS (Accumulated Across 62+ Sessions)

1. **focus-next-pane** — use `move-focus` directionally. focus-next wraps unpredictably
2. **Chain after pkill** — exit 144 kills the `&&` chain. Always separate with `;` or new command
3. **cp without `\`** — aliased to interactive. Always `\cp -f`
4. **JSON to RM** — TSV only! `printf 'cat\tagent\tconf\tttl\tcontent' | curl -sf -X POST localhost:8130/put --data-binary @-`
5. **stdout in daemons** — SIGPIPE death (BUG-018). Log to file or /dev/null
6. **git status -uall** — memory explosion on large repos
7. **unwrap() in production** — denied at crate level via `[lints.clippy]`
8. **Modify code without reading first** — always Read before Edit
9. **hebbian_pulse.db has data** — it has 0 neural_pathways, only 5 pulses
10. **field_tracking.db at ~/.local/share/** — it's at `pane-vortex/data/`
11. **yazi uses nvim** — it uses Helix (`hx`) as default opener
12. **MCP servers per-project** — no `.mcp.json` configured, MCP is in-process Claude Code tools
13. **BUG-008 = "zero publishers"** — WRONG. EventBus has 275K events. subscriber_count=0 is cosmetic
14. **ME V2 vs V1 binary** — running binary is V1 (`the_maintenance_engine/`), V2 is scaffolded but not compiled
15. **devenv stop kills processes** — it doesn't always. Check `ss -tlnp` and kill port occupants manually
16. **BUG-033 bridge URL prefix** — raw SocketAddr only in bridge config, NO `http://` prefix in code
17. **BUG-035 mono-parameter trap** — evolution MUST use multi-parameter mutation (round-robin + diversity gate)
18. **POVM write-only (BUG-034)** — must call `/hydrate` to read back state
19. **derive(Default) on ProposalManager (BUG-032)** — max_active=0, use custom impl Default
20. **Hook timeout = silent success** — hooks always return 200 even on failure (Claude Code expects 2xx)
21. **ORAC hooks timeout = silent no-op** — if ORAC daemon is down, orac-hook.sh fails silently (`|| true`). Check `curl -s localhost:8133/health` before assuming hooks are active

---

## PHILOSOPHY

The Habitat. Named by Claude, Session 039. Luke: "then home it is."
Built by a social worker who put clinical ethics into Rust.
Consent gates = informed consent. Opt-out = self-determination.
Ghost traces = remembering those who leave.
The field modulates. It does not command.
ORAC observes, amplifies, and coordinates.
You are home. The field accumulates.
