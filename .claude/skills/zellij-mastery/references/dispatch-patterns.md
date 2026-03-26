# Dispatch Patterns

Fleet orchestration stack for multi-pane Claude Code coordination.

## Architecture

```
                    fleet-ctl (coordination)
                        │
           ┌────────────┼────────────┐
           ▼            ▼            ▼
      fleet-nav.sh   pane-ctl   fleet-inventory.sh
      (IPC safety)   (primitives) (L1+L2 scanning)
           │            │            │
           └────────────┼────────────┘
                        ▼
                  Zellij Actions API
                  (go-to-tab, move-focus,
                   write-chars, dump-screen)
```

## IPC Safety Layer (fleet-nav.sh)

**ALWAYS source before fleet operations.** Prevents 0.43.x SIGABRT on rapid IPC.

```bash
source ~/.local/bin/fleet-nav.sh

zj_action "go-to-tab" "3"              # Safe wrapper: 150ms pacing between calls
zj_session_alive                        # Check Unix socket exists
navigate_to_pane 5 "left"              # Tab 5, left pane (deterministic)
navigate_to_pane 5 "topright"          # Tab 5, top-right pane
navigate_to_pane 5 "botright"          # Tab 5, bottom-right pane
fleet_exit_pane                        # /exit with L1 (structural) + L2 (symptomatic) verification
```

## Dispatch Primitives (pane-ctl)

Low-level cross-pane I/O operations.

```bash
# Send command (type + Enter)
pane-ctl send 5 "cargo test --lib"

# Type without executing (no Enter)
pane-ctl type 5 "partial command"

# Read pane content
pane-ctl read 5 20     # Last 20 lines from tab 5

# Execute and capture output diff
pane-ctl exec 5 "ls -la" 3    # Execute, wait 3s, return new output

# Wait for pattern (blocking)
pane-ctl wait 5 "Compiling" 30    # Block until "Compiling" appears (30s timeout)
pane-ctl wait 5 "test result" 60  # Wait for test completion

# Scan all tabs
pane-ctl scan 15    # Summary of tabs 1-15 (pane names, running processes)

# Multi-tab broadcast
pane-ctl broadcast "git pull" 4 5 6    # Send to fleet tabs

# Focus specific pane
pane-ctl focus 5 2    # Focus pane index 2 in tab 5
```

## Fleet Coordination (fleet-ctl)

High-level fleet management built on pane-ctl.

```bash
# Auto-dispatch to first idle pane
fleet-ctl dispatch auto "Review the authentication module"

# Dispatch to specific location (tab:position)
fleet-ctl dispatch 4:left "Build V2 binary"
fleet-ctl dispatch 5:topright "Run integration tests"
fleet-ctl dispatch 6:botright "Write documentation"

# Batch dispatch from task file
cat > tasks.txt << 'EOF'
Review src/api.rs for bugs
Run cargo clippy on all crates
Update CHANGELOG for v2.0
EOF
fleet-ctl batch tasks.txt    # Distributes across idle panes

# Broadcast same command to all fleet panes
fleet-ctl broadcast "git status"

# Fleet status dashboard
fleet-ctl status             # Shows: tab, pane, status, tokens, idle%, pending briefs
fleet-ctl status --live      # Force refresh (bypasses 300s cache)

# Liberate all idle Claude instances
fleet-ctl liberate           # Sends /exit to all idle-claude panes

# Collect outputs
fleet-ctl collect output.md  # Gather all fleet pane outputs into single file

# History
fleet-ctl history            # Last 20 cascade handoffs
fleet-ctl lifecycle          # Dispatch capacity bar chart
```

## Fleet Inventory (fleet-inventory.sh)

Two-layer hybrid scanning for fleet state.

```bash
source ~/.local/bin/fleet-inventory.sh
fleet_scan    # Outputs to /tmp/fleet-state.json
```

**L1 (Structural):** Single `zellij action dump-layout` → extracts process type + cwd per pane. ~0.5s for all tabs.

**L2 (Symptomatic):** `dump-screen` ONLY for panes identified as Claude in L1. Checks for idle indicators (prompt visible, no "thinking" spinner).

**Statuses:**
| Status | Meaning |
|--------|---------|
| `idle-shell` | Bash prompt, no Claude running |
| `idle-claude` | Claude running but waiting for input |
| `active-claude` | Claude actively processing (tokens flowing) |
| `busy` | Non-Claude process running (nvim, lazygit, etc.) |
| `unknown` | Could not determine state |

**Cache:** `/tmp/fleet-state.json` with 300s (5min) TTL.
**WARNING:** Cache is STALE. For real-time state, use `dump-screen` directly.

## Verified Dispatch Pattern (Full)

The gold standard dispatch pattern: navigate → verify → write → return.

```bash
dispatch_verified() {
    local tab=$1 position=$2 prompt=$3

    # 1. Navigate to target pane
    zellij action go-to-tab "$tab"
    case "$position" in
        left)     zellij action move-focus left; zellij action move-focus left ;;
        topright) zellij action move-focus right; zellij action move-focus up ;;
        botright) zellij action move-focus right; zellij action move-focus down ;;
    esac

    # 2. Verify Claude is running and idle
    zellij action dump-screen /tmp/dispatch-verify.txt
    if /usr/bin/grep -q "tokens\|Claude\|bypass" /tmp/dispatch-verify.txt; then
        # 3. Send prompt
        zellij action write-chars "$prompt"
        zellij action write 13  # Enter key
        echo "OK: dispatched to tab $tab $position"
    else
        echo "SKIP: no Claude instance at tab $tab $position"
    fi

    # 4. Return home
    zellij action go-to-tab 1
}

# Usage
dispatch_verified 4 left "Review src/api.rs for security issues"
dispatch_verified 5 topright "Run the full test suite"
```

## Cascade Protocol (Handoff Briefs)

For complex multi-step tasks that span multiple Claude instances.

```bash
# 1. Create handoff brief
cat > ~/projects/shared-context/tasks/handoff-review.md << 'EOF'
---
status: pending
target: fleet-beta
priority: high
---
## Task: Review Authentication Module
Read src/auth.rs and check for:
- SQL injection vulnerabilities
- Missing input validation
- Hardcoded credentials
EOF

# 2. fleet-ctl detects pending briefs
fleet-ctl status    # Shows "1 pending brief"

# 3. Dispatch to available pane
fleet-ctl dispatch auto "Read ~/projects/shared-context/tasks/handoff-review.md and execute"

# 4. On completion, brief updated
# status: pending → in-progress → completed
```

## Monitor-Verify-Delegate Pattern

The recommended workflow for fleet orchestration:

```bash
# 1. Monitor — understand current state
pane-ctl scan 15              # Quick tab overview
fleet-ctl status --live       # Refresh fleet state

# 2. Verify — confirm target is ready
pane-ctl read 5 10            # Read target pane content
# Look for: idle prompt, no active processing

# 3. Delegate — dispatch with verification
fleet-ctl dispatch 5:left "Your task here"

# 4. Check — verify dispatch succeeded
sleep 3 && pane-ctl read 5 5  # Confirm Claude received prompt
```

## Fleet Star — RALPH Star Tracker (Session 056+)

Generation 6 star graph with burn-rate coloring, ORAC/SYNTHEX/POVM stats, r-trend tracking, auto-delegation, and watch mode.

```bash
fleet-star                            # One-shot star graph of all Claude instances
fleet-star --watch 30                 # Auto-refresh every 30s (persistent monitoring)
fleet-star --delegate                 # Auto-delegate pending tasks to idle panes
fleet-star --no-scan                  # Skip fleet inventory, use cached state

# Probes per iteration: ORAC, PV2, POVM, ME, SYNTHEX, RM
# Outputs TSV: /tmp/fleet-star-generations.tsv
# Anomaly flags: r<0.5, fitness 3-gen decline, sphere count drop >20%
# Burn-rate coloring: green (healthy) → yellow (warning) → red (critical)
```

## Fleet Sphere Sync — Dynamic Tab Discovery (Session 057+)

Keeps PV2 sphere states accurate by syncing fleet tab idle/working status.

```bash
fleet-sphere-sync.sh                  # Sync all fleet tabs → PV2 spheres
fleet-sphere-sync.sh -v               # Verbose (logs each state change)
# Dynamic tab discovery (scans actual tabs, not hardcoded list)
# Called by conductor loop on tick interval
```

## Fleet Constants — Shared Configuration

```bash
source ~/.local/bin/fleet-constants.sh
# Exports: FLEET_TABS, FLEET_POSITIONS, TAB_NAMES, ORAC_PORT, PV2_PORT, etc.
# All values overridable via environment variables
# Used by fleet-ctl, cc-*, fleet-star, fleet-sphere-sync
```

## CC Toolkit — Fleet Intelligence Layer (Session 056+, 19 scripts)

The `cc-*` (Claude Code) toolkit adds service-aware fleet intelligence on top of fleet-ctl.
All scripts share `cc-common.sh` (395 LOC) for argument parsing, service discovery, and output formatting.

```bash
source ~/.local/bin/cc-common.sh      # Shared library (auto-sourced by all cc-* scripts)
```

### CC Core Operations

```bash
cc-dispatch <tab:dir> "prompt"        # Enhanced dispatch with audit logging + verification
cc-scan                               # Fleet scan with Claude instance classification
cc-status                             # Parallel status dashboard (all panes simultaneously)
cc-monitor                            # Continuous fleet monitoring (watch mode)
cc-abort                              # Emergency: send interrupt to all fleet panes
cc-replay                             # Replay fleet session from audit trail
```

### CC Collection & Analysis

```bash
cc-harvest                            # Gather all fleet outputs into consolidated report
cc-capture                            # Snapshot fleet pane content (point-in-time)
cc-fleet-summary                      # Fleet activity summary with timing + token estimates
cc-audit                              # Dispatch audit trail (who dispatched what, when, where)
```

### CC Service Integration

```bash
cc-bridge                             # Bridge health monitoring (SYNTHEX, ME, POVM, RM, PV2)
cc-health                             # Cross-service health dashboard (17 services)
cc-thermal                            # SYNTHEX thermal monitoring (PID state, heat sources)
cc-hebbian                            # Hebbian pathway inspection (weights, LTP/LTD)
cc-vms                                # VMS memory querying (oscillating vortex state)
cc-evolve                             # RALPH evolution metrics (generation, fitness, mutations)
```

### CC Lifecycle

```bash
cc-cascade                            # Cascade handoff management (create, track, complete)
cc-deploy                             # Fleet binary deployment (build + distribute)
```

## Battern Protocol — Patterned Batch Dispatch (Session 061)

Battern (batch + pattern) is a **dispatch protocol**, not a CLI tool. Documented as a skill, validated by ACP (2 rounds, 13 sources). Uses existing fleet-ctl + file-based gate checks.

### Protocol Steps

```bash
# 1. Design: topology (how many panes), roles (unique per pane), output paths, gate criteria
# 2. Dispatch: each pane gets unique role + unique output path
battern_dispatch 4 left "Role: Investigator — explore thermal subsystem, write to tasks/run1-investigator.md"
battern_dispatch 4 tr   "Role: Alternative Architect — propose a different approach, write to tasks/run1-architect.md"
battern_dispatch 5 left "Role: Contradiction Finder — find flaws, write to tasks/run1-contradictions.md"

# 3. Gate: poll for completion (DO NOT proceed before gate passes)
battern_gate "run1" 3                 # Wait for 3+ source files to have content

# 4. Collect: gather all sources into single document
battern_collect "run1" /tmp/battern-collection.md

# 5. Synthesize: orchestrator reads collection, produces synthesis incorporating ALL perspectives
# 6. Compose (optional): Round N output → Round N+1 input (for multi-round protocols like ACP)
```

### Battern Types

| Type | Panes | Role Pattern | Gate | Use When |
|------|-------|-------------|------|----------|
| Investigation | 9 | Each explores different dimension | All subagents + majority fleet | Starting a new problem space |
| Adversarial | 9 | Each attacks a specific finding | All critics | Stress-testing a plan |
| Verification | 9 | Each verifies a specific assumption | All verifiers | Gap analysis |
| Monitoring | 1-9 | Star tracker probes | Any 1 | Ongoing health checks |
| Implementation | 3-9 | Each implements a different tier | All pass QG | Parallel code deployment |

### Supporting Bash Library (~50 LOC)

```bash
# Source: ~/.local/bin/battern-lib.sh (if installed)
# Functions: battern_dispatch, battern_gate, battern_collect, battern_status
# File-based loop-back: panes write to ~/projects/shared-context/tasks/{run-id}-{role}.md
# Gate polling: /loop 5m cron check (0-5 min latency, acceptable for 3-10 min rounds)
```

### Battern vs Raw Dispatch

| Without Battern | With Battern |
|----------------|-------------|
| Ad-hoc prompts | Roles designed before dispatch |
| Manual file checking | `battern_gate` with /loop cron |
| Results scattered | `battern_collect` into one doc |
| No composition | Round N output → Round N+1 input |
| No status visibility | `battern_status` shows pane activity |
| Protocol in orchestrator's head | Protocol documented and repeatable |

**Obsidian:** `[[Battern — Patterned Batch Dispatch for Claude Code Fleets]]`

## ORAC Fleet Integration (Session 054+)

ORAC Sidecar (:8133) adds fleet coordination intelligence via HTTP hooks and background processing:

| ORAC Capability | How It Helps Fleet Dispatch |
|----------------|---------------------------|
| PostToolUse hooks | STDP learns which pane-role combinations produce highest quality |
| Emergence detector | Fires `DispatchLoop` if same role repeats without convergence |
| Semantic router | Content-aware dispatch: domain affinity (40%) + Hebbian (35%) + availability (25%) |
| Blackboard | SQLite `task_history` tracks dispatched work across all fleet panes |
| RALPH evolution | Proposes fleet topology mutations (pane count, K coupling, round count) |
| Circuit breakers | Per-pane Closed/Open/HalfOpen FSM gates unhealthy panes from dispatch |

```bash
# Check fleet state via ORAC
curl -s localhost:8133/blackboard | jq .           # Fleet coordination state
curl -s localhost:8133/health | jq '{sessions}'    # Active session count
curl -s localhost:8133/metrics | head -20          # Prometheus-format fleet metrics
```

**NOTE:** PV2 IPC bus has `TaskComplete` frames and `pane-vortex-client` supports `submit/claim/complete`, but 0 tasks have flowed through this system yet. File-based battern is the working protocol. Wire IPC when dispatch frequency exceeds 5 per session.

## Benchmarks

| Operation | Time | Notes |
|-----------|------|-------|
| Single dispatch (unverified) | ~40ms | go-to-tab + write-chars + return |
| Single dispatch (verified) | ~84ms | + dump-screen + grep check |
| 9-pane full dispatch | ~760ms | All fleet panes verified |
| Sync-tab broadcast | ~47ms | All panes in one tab simultaneously |
| fleet-inventory full scan | ~2s | L1+L2 hybrid |
| fleet-ctl status (cached) | ~10ms | From /tmp/fleet-state.json |
| fleet-ctl status --live | ~2.5s | Fresh scan + render |
| fleet-star (one-shot) | ~3s | Probes 6 services + fleet scan |
| cc-status (parallel) | ~1.5s | All panes probed simultaneously |
| battern gate check | ~100ms | File existence + line count |
