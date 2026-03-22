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
