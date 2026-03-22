# Ecosystem Reference (ORAC Sidecar Edition)

## DevEnv Batches (5 layers + ORAC)
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

### ORAC DevEnv Entry (planned)
```toml
[services.orac-sidecar]
name = "ORAC Sidecar"
command = "./bin/orac-sidecar"
working_dir = "/home/louranicas/claude-code-workspace/orac-sidecar"
port = 8133
health_path = "/health"
batch = 5
depends_on = ["pane-vortex", "povm-engine"]
description = "Intelligent fleet coordination proxy"
```

## 58+ Custom Binaries (~/.local/bin/)

### ORAC Binaries (3 — deployed Session 054)
| Binary | Size | Purpose |
|--------|------|---------|
| `orac-sidecar` | 5.5MB | Main daemon: Axum HTTP :8133, IPC, graceful shutdown |
| `orac-probe` | 2.3MB | Diagnostics: probes 6 endpoints (ORAC, PV2, SX, ME, POVM, RM) |
| `orac-client` | 337KB | CLI: status, field, spheres, health, hooks, bridges |

### Habitat Probes (Rust)
| Binary | Purpose |
|--------|---------|
| `habitat-probe` | Fast system probes: pulse, sweep, field, spheres, bus, me, bridges, full |

### Fleet Scripts
| Binary | Purpose |
|--------|---------|
| fleet-ctl | Per-pane addressing, auto dispatch, status dashboard, exit, history, batch |
| fleet-vortex | Fleet coordination daemon |
| fleet-heartbeat | Dynamic tabs, verification queue, capacity reporting |
| fleet-inventory.sh | Hybrid L1+L2 structural/symptomatic scan |
| fleet-nav.sh | 6 shared functions: navigate, dump, write, return_home |

### Service Control
| Binary | Purpose |
|--------|---------|
| nvim-ctl | 26 commands (remote socket control) |
| pane-ctl | Pane management |
| pane-vortex-ctl | 22 PV2 HTTP routes |
| swarm-ctl | Swarm plugin management |

### Intelligence Tools
| Binary | Purpose |
|--------|---------|
| vault-search | Obsidian vault search CLI |
| evolution-metrics | Evolution chamber metrics |
| reasoning-memory | RM client (Rust) |

### Build Tools
| Binary | Purpose |
|--------|---------|
| quality-gate | 4-step quality gate runner |
| build-and-test | Combined build + test |
| shellcheck | Shell script linting |
| code-review | Code review helper |

## Nvim Integration (128L autocmds)
BufWritePost → PV /sphere/nvim/memory + status Working (5s debounce)
BufWritePost *.rs → RM diagnostics (10s debounce)
30s idle → PV /sphere/nvim/status Idle
VimEnter → register sphere | VimLeavePre → deregister

## Zellij Plugins (11)
harpoon(Alt+v) ghost(Alt+g) monocle(Alt+m) multitask(Alt+t) room(Ctrl+y)
swarm-orchestrator(Alt+w) autolock(auto) attention(auto) zjstatus sendkeys

**Never script plugin interactions** (zombie behaviour — keybind-only)

## Vault (Obsidian)

### Main Vault: ~/projects/claude_code/ (215+ notes)
CLI: `vault-search "query" 10 markdown`

**Key ORAC notes:**
- `[[Session 050 — ORAC Sidecar Architecture]]`
- `[[Session 051 — ORAC Sidecar .claude Scaffolding]]`
- `[[Session 052 — Phase 1 Hooks Deployed]]`
- `[[Session 053 — ORAC Phase 2 Intelligence + Gold Standard Audit]]`
- `[[Session 053b — ORAC Full Deploy Assessment]]`
- `[[ORAC — RALPH Multi-Parameter Mutation Fix]]`

**Key Habitat notes:**
- `[[Session 039 — ZSDE Nvim God-Tier Command Reference]]`
- `[[Session 039 — Lazygit God-Tier Command Reference]]`
- `[[Session 039 — Atuin and Yazi God-Tier Reference]]`
- `[[Session 039 — Architectural Schematics and Refactor Safety]]`
- `[[Session 042 — Habitat Skills Architecture and Progressive Disclosure]]`
- `[[ULTRAPLATE — Bugs and Known Issues]]`

### Shared Context: ~/projects/shared-context/
Subdirs: codebase, decisions, tasks, patterns, planning

### ORAC Documentation (in-repo)
- `ai_docs/` — 25 files: quickstart, gold standard, layer docs, module docs, schematics (Mermaid)
- `ai_specs/` — 11 files: API, hooks, bridges, wire protocol, evolution, patterns (builder, circuit breaker, Kuramoto, STDP)
- `.claude/schemas/` — 5 JSON schemas: hook_request, hook_response, permission_policy, bus_event, bus_frame
- `.claude/queries/` — 3 SQL files: blackboard, hook_events, fleet_state

## Cascade Handoff Protocol
1. Writer creates `tasks/handoff-{target}-{timestamp}.md`
2. Target reads, updates status: in-progress
3. On completion: status: completed
4. Tracked in `.claude/cascade-state.json`
5. ORAC's `m28_cascade` module can trigger cascade via `/cascade` endpoint or IPC bus

## ORAC Hook Deployment

### Current State (Session 054)
6 hooks migrated from bash to ORAC HTTP via `hooks/orac-hook.sh` forwarder.
**Rolled back** to bash hooks in settings.json for stability testing.

### Forwarder
```bash
hooks/orac-hook.sh <EventName> [timeout]
# Reads stdin → POST http://localhost:8133/hooks/<EventName> → stdout response
# Fails silently if ORAC unreachable (never blocks Claude Code)
```

### Re-apply ORAC hooks
```bash
# From orac-sidecar backup
\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json
# Then manually update hooks in settings.json to point to orac-hook.sh
```

### Hooks kept as bash
- SubagentStop (subagent_field_aggregate.sh — no ORAC endpoint)
- PreCompact (handoff-dispatch.sh — cascade system)
- Stop/check-cipher-messages.sh (non-PV2, cipher system)

## ORAC Build & Deploy

```bash
# Full build
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full

# Deploy (3 binaries)
\cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/
\cp -f /tmp/cargo-orac/release/orac-probe ~/.local/bin/
\cp -f /tmp/cargo-orac/release/orac-client ~/.local/bin/

# Start daemon
nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &

# Quality gate
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```
