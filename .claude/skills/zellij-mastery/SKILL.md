---
name: zellij-mastery
user-invocable: true
description: Gold standard Zellij 0.43.1 setup mastery for The Habitat. Covers config.kdl architecture (138+ keybinds, 11 modes), 6 KDL layouts (synth-orchestrator gold standard), 11 WASM plugins (6 floating, 2 background, 3 utility), fleet orchestration stack (fleet-ctl, pane-ctl, fleet-inventory), dispatch patterns, plugin pipe protocol, KDL syntax, layout design, troubleshooting. Triggers on zellij, layout, plugin, keybind, fleet dispatch, pane navigation, tab management, zellij config, zellij layout, zellij plugin, KDL syntax, swarm orchestrator, ghost plugin, harpoon, room, or when Claude needs to understand or modify the Zellij terminal environment.
argument-hint: [config|layouts|plugins|dispatch|keybinds|troubleshoot|launch]
---

# /zellij-mastery — Gold Standard Zellij Setup

> Zellij 0.43.1 (patched) | 11 WASM plugins | 6 layouts | 138+ keybinds | 11 modes
> Complements /primehabitat (services + field) and /deephabitat (substrate + DBs)
> This skill owns: config.kdl anatomy, KDL layout syntax, plugin ecosystem, dispatch stack, keybind architecture

## QUICK CARD

```
VERSION:  0.43.1 (patched: plugin crash resilience + IPC error rate limiting)
CONFIG:   ~/.config/zellij/config.kdl (603 lines, 11 modes, 138+ keybinds)
LAYOUTS:  ~/.config/zellij/layouts/ (6 KDL files)
PLUGINS:  ~/.config/zellij/plugins/ (11 WASM, 18MB total)
LOG:      /tmp/zellij-1000/zellij-log/zellij.log
SOCKETS:  /run/user/1000/zellij/0.43.1/

GOLD STANDARD LAYOUT: synth-orchestrator.kdl
  Tab 1: Orchestrator (Command + PV Monitor + Health Watch) + floating swarm plugin
  Tab 2: Workspace 1 (Atuin + Yazi + Htop) — exploration
  Tab 3: Workspace 2 (Bacon + Lazygit + Nvim) — development
  Tabs 4-6: Fleet-ALPHA/BETA/GAMMA (3 panes each = 9 dispatch slots)

PLUGINS (by keybind):
  Alt+v=Harpoon  Alt+w=Swarm  Alt+g=Ghost  Alt+m=Monocle  Alt+t=Multitask  Ctrl+y=Room
  Background: zellij-autolock (locks on nvim/lazygit), zellij-attention (tab indicators)
  Utility: zjstatus, zellij-send-keys (no keybind, programmatic use)

DISPATCH STACK:
  fleet-ctl dispatch|batch|broadcast|status|liberate|collect
  pane-ctl send|type|read|exec|wait|scan|broadcast|focus
  fleet-inventory.sh (L1 structural + L2 symptomatic scanning)
  fleet-nav.sh (IPC safety: 150ms pacing, prevents 0.43.x SIGABRT)

NEVER:
  focus-next-pane (wraps unpredictably — use directional move-focus)
  Rapid IPC without 150ms pacing (SIGABRT on 0.43.x)
  launch-or-focus-plugin without --floating (embeds in layout permanently)
```

For detailed reference on any topic, read the corresponding file in `references/`:
- **Config anatomy**: `references/config-anatomy.md` — modes, keybinds, plugin section, options
- **KDL layout syntax**: `references/kdl-layouts.md` — layout grammar, tab templates, pane types, swap layouts
- **Plugin ecosystem**: `references/plugin-ecosystem.md` — all 11 plugins with config, controls, pipe protocol
- **Dispatch patterns**: `references/dispatch-patterns.md` — fleet-ctl, pane-ctl, dispatch functions, cascade protocol
- **Keybind map**: `references/keybind-map.md` — complete keybind reference for all 11 modes
- **Troubleshooting**: `references/troubleshooting.md` — common failures, log analysis, recovery procedures

---

## LAUNCH: Full Environment Activation

The gold standard layout (`synth-orchestrator.kdl`) auto-launches all workspace tools on session start:

### What the Layout Launches Automatically

| Tab | Pane | Tool | Command |
|-----|------|------|---------|
| 1 | Orchestrator | Swarm plugin | floating `swarm-orchestrator.wasm` (quality 0.80, max 5 iter) |
| 2 | Atuin | `atuin` | Shell history search + analytics |
| 2 | Yazi | `yazi` | File browser (uses Helix opener, NOT nvim) |
| 2 | btm | `btm` | System monitor (bottom) |
| 3 | Bacon | `bacon` | Cargo check/clippy watcher |
| 3 | Lazygit | `lazygit` | Git porcelain (6 custom commands) |
| 3 | Nvim | `nvim --listen /tmp/nvim.sock` | Editor with RPC socket |

### What Loads in Background (config.kdl `load_plugins`)

| Plugin | Auto-loaded | Purpose |
|--------|-------------|---------|
| `zellij-autolock.wasm` | On session start | Locks Zellij when nvim/lazygit focused |
| `zellij-attention.wasm` | On session start | Tab activity indicators |

### Launch Floating Plugins (on-demand, run when needed)

```bash
# Launch all 6 floating plugins (each opens as floating overlay, Esc to dismiss)
zellij action launch-or-focus-plugin --floating "file:$HOME/.config/zellij/plugins/harpoon.wasm"
zellij action launch-or-focus-plugin --floating "file:$HOME/.config/zellij/plugins/room.wasm"
zellij action launch-or-focus-plugin --floating "file:$HOME/.config/zellij/plugins/ghost.wasm"
zellij action launch-or-focus-plugin --floating "file:$HOME/.config/zellij/plugins/monocle.wasm"
zellij action launch-or-focus-plugin --floating "file:$HOME/.config/zellij/plugins/multitask.wasm"
zellij action launch-or-focus-plugin --floating "file:$HOME/.config/zellij/plugins/swarm-orchestrator.wasm"
```

**GOTCHA:** `launch-or-focus-plugin` is a TOGGLE — calling it when the plugin is already open CLOSES it. Use keybinds (Alt+v/w/g/m/t, Ctrl+y) for toggle behavior.

### Start a Fresh Gold Standard Session

```bash
zellij --layout ~/.config/zellij/layouts/synth-orchestrator.kdl --session habitat
```

This creates a 6-tab session with all tools auto-launched in their designated panes, the swarm plugin floating on Tab 1, and background plugins active.

### Verify Everything Is Running

```bash
# Check all workspace tools are alive
echo "=== Tab 2: Exploration ===" && \
  pgrep -x atuin >/dev/null && echo "  atuin: UP" || echo "  atuin: DOWN" && \
  pgrep -x yazi >/dev/null && echo "  yazi: UP" || echo "  yazi: DOWN" && \
  pgrep -x btm >/dev/null && echo "  btm: UP" || echo "  btm: DOWN"
echo "=== Tab 3: Development ===" && \
  pgrep -x bacon >/dev/null && echo "  bacon: UP" || echo "  bacon: DOWN" && \
  pgrep -x lazygit >/dev/null && echo "  lazygit: UP" || echo "  lazygit: DOWN" && \
  nvim --server /tmp/nvim.sock --remote-expr 'v:version' 2>/dev/null && echo "  nvim: UP (RPC)" || echo "  nvim: DOWN"
```

---

## ARCHITECTURE: Tab-per-Domain Pattern

The gold standard organizes tabs by cognitive domain:

```
Tab 1: ORCHESTRATOR — Control hub (you are here)
  ├─ Command (50%, left, focus) — main Claude or operator pane
  ├─ PV Monitor (25%, top-right) — field state, coupling
  └─ Health Watch (25%, bot-right) — service health dashboard
  + Floating: swarm-orchestrator.wasm (Alt+W to toggle)

Tab 2: EXPLORATION — Research & navigation
  ├─ Atuin (50%, left) — shell history search + analytics
  ├─ Yazi (25%, top-right) — file browser (uses Helix, NOT nvim)
  └─ Htop/btm (25%, bot-right) — system monitor

Tab 3: DEVELOPMENT — Code, compile, commit
  ├─ Bacon (50%, left) — cargo check/clippy watcher
  ├─ Lazygit (25%, top-right) — git porcelain (6 custom commands)
  └─ Nvim (25%, bot-right) — editor with /tmp/nvim.sock RPC

Tabs 4-6: FLEET — Parallel Claude dispatch wings
  ├─ WING-Left (50%, left) — primary dispatch slot
  ├─ WING-TopRight (25%, top-right) — secondary slot
  └─ WING-BotRight (25%, bot-right) — tertiary slot
  × 3 wings = 9 concurrent Claude instances max
```

### The 3-Pane Standard (All Tabs)

Every tab uses the same 50/50 vertical split with right side split horizontally:

```kdl
pane split_direction="vertical" {
    pane size="50%" name="LEFT" focus=true
    pane size="50%" {
        pane size="50%" name="TOP-RIGHT"
        pane size="50%" name="BOT-RIGHT"
    }
}
```

**Why this matters:** Directional navigation is deterministic. `move-focus left` ALWAYS reaches the left pane. `move-focus right; move-focus up` ALWAYS reaches top-right. No cycling, no state dependency.

---

## NAVIGATION (Directional, Deterministic)

### Tab Navigation (~14ms each)
```bash
zellij action go-to-tab 3              # By index (fastest)
zellij action go-to-tab-name "Monitor" # By name (readable)
zellij action go-to-next-tab           # Sequential
zellij action go-to-previous-tab       # Sequential reverse
```

### Pane Targeting (~11ms each)
```bash
# LEFT pane (idempotent — repeat safely)
zellij action move-focus left; zellij action move-focus left

# TOP-RIGHT pane
zellij action move-focus right; zellij action move-focus up

# BOT-RIGHT pane
zellij action move-focus right; zellij action move-focus down
```

### Sync-Tab Broadcast (47ms — all panes simultaneously)
```bash
zellij action go-to-tab $TAB
zellij action toggle-active-sync-tab   # ON
zellij action write-chars "$CMD" && zellij action write 13
zellij action toggle-active-sync-tab   # OFF
zellij action go-to-tab 1              # Return home
```

### Verified Cross-Tab Dispatch (84ms/pane)
```bash
dispatch_to_pane() {
    local tab=$1 pane_dir=$2 prompt=$3
    zellij action go-to-tab "$tab"
    case "$pane_dir" in
        left)     zellij action move-focus left; zellij action move-focus left ;;
        topright) zellij action move-focus right; zellij action move-focus up ;;
        botright) zellij action move-focus right; zellij action move-focus down ;;
    esac
    zellij action dump-screen /tmp/dispatch-verify.txt
    if /usr/bin/grep -q "tokens\|Claude\|bypass" /tmp/dispatch-verify.txt; then
        zellij action write-chars "$prompt"
        zellij action write 13
        echo "dispatched"
    else
        echo "no Claude instance"
    fi
    zellij action go-to-tab 1
}
```

---

## PLUGIN QUICK REFERENCE

| Plugin | Keybind | Purpose | Key Controls |
|--------|---------|---------|--------------|
| **Harpoon** | `Alt+v` | Pane bookmarks, cross-tab jump | a=add, A=add-all, j/k=nav, l/Enter=jump, d=remove |
| **Swarm** | `Alt+w` | Fleet dashboard, RALPH loop | quality_threshold=0.80, max_iterations=5 |
| **Ghost** | `Alt+g` | Command palette (bash -ic) | Type to fuzzy filter, Enter to launch in new pane |
| **Monocle** | `Alt+m` | Fullscreen single-pane zoom | Toggle on/off |
| **Multitask** | `Alt+t` | Parallel task viewer | Multi-pane coordination |
| **Room** | `Ctrl+y` | Fuzzy tab/pane switcher | Type to filter, number=quick jump, Enter=go |
| **Autolock** | auto | Locks on nvim/vim/pi/git/lazygit | watch_interval=1.0s |
| **Attention** | auto | Tab output indicators | Highlights tabs with new output |
| **zjstatus** | — | Custom status bar | Available, not actively configured |
| **send-keys** | — | Programmatic key injection | Utility for cross-pane automation |
| **swarm-v2** | — | Updated orchestrator | Alternative WASM binary |

### Plugin Pipe Protocol (programmatic control)
```bash
# Send command to plugin via named pipe
zellij pipe -p "file:~/.config/zellij/plugins/swarm-orchestrator.wasm" -n cmd -- '{"Spawn":{"persona":"reviewer","task":"Review PR"}}'

# Room: focus a specific pane by ID
zellij pipe --plugin "file:~/.config/zellij/plugins/room.wasm" --name focus-pane -- <pane_id>

# Launch plugin programmatically
zellij action launch-or-focus-plugin --floating "file:~/.config/zellij/plugins/ghost.wasm"
```

### swarm-ctl (CLI for Swarm Orchestrator plugin)
```bash
swarm-ctl spawn <persona> <task>       # Create agent
swarm-ctl dispatch <id> <task>         # Task to specific agent
swarm-ctl dispatch-all <task>          # Broadcast to all
swarm-ctl ralph <task> [iter] [thresh] # RALPH quality iterations
swarm-ctl status                       # Agent status dashboard
swarm-ctl bus-status                   # Sidecar + event log
swarm-ctl bus-submit <desc> [target]   # IPC Bus submission
```

---

## MODE HIERARCHY (11 modes, Ctrl+key to enter)

```
Normal (default) ─── the starting mode
├─ Ctrl+p → Pane mode (d=down, r=right, s=stacked, f=fullscreen, c=rename, i=pin, w=float, z=frames)
├─ Ctrl+t → Tab mode (1-9=goto, n=new, x=close, r=rename, s=sync, b=break, []=break L/R)
├─ Ctrl+n → Resize mode (hjkl=increase, HJKL=decrease, +/-/==uniform)
├─ Ctrl+h → Move mode (hjkl=directional, n/tab=auto)
├─ Ctrl+s → Scroll mode (e=edit scrollback, s→Search mode)
├─ Ctrl+o → Session mode (a=about, c=config, p=plugin-mgr, s=share, w=session-mgr)
├─ Ctrl+b → Tmux mode (vim compat: c=new tab, "/%=split, z=fullscreen, n/p=nav)
└─ Ctrl+g → Locked mode (all keybinds disabled, Ctrl+g to unlock)
```

All modes exit with their entry key or Esc (where applicable).

---

## LAYOUT INVENTORY (6 KDL files)

| Layout | Tabs | Use Case | Launch |
|--------|------|----------|--------|
| **synth-orchestrator** | 6 | Gold standard: orch + 2 workspace + 3 fleet | `zellij --layout synth-orchestrator` |
| **devenv** | 6 | Service development: command + POVM dev + monitor + knowledge + 2 fleet | `zellij --layout devenv` |
| **swarm-orchestrator** | 4 | Fleet-focused: orch + 3 fleet wings | `zellij --layout swarm-orchestrator` |
| **ultraplate** | 6 | General: command + 2 workspace + 3 fleet (no auto-launch) | `zellij --layout ultraplate` |
| **review** | 1 | Code review: 65% agent + stacked editor/review/shell | `zellij --layout review` |
| **review-minimal** | 1 | Minimal review: 65% agent + stacked (one expanded) | `zellij --layout review-minimal` |

---

## GOTCHAS (SESSION-TESTED)

1. **focus-next-pane** wraps unpredictably — ALWAYS use `move-focus` directionally
2. **Rapid IPC (< 150ms)** causes SIGABRT on 0.43.x — fleet-nav.sh enforces pacing
3. **launch-or-focus-plugin is a toggle** — calling it when plugin is open CLOSES it
4. **Plugin crash on unload** — patched binary handles stale cached state + mutex poisoning
5. **Zellij pipe requires exact plugin path** — `file:~/.config/zellij/plugins/NAME.wasm`
6. **Tab sync broadcast** — must toggle OFF after use, or all future typing goes to all panes
7. **dump-screen** is the ONLY reliable pane state source — fleet-ctl cache has 300s TTL
8. **Stacked panes** — use `stacked=true` on parent, `expanded=true` on the one to show
9. **KDL layout size=1** — means 1 row (for tab-bar/status-bar), NOT percentage
10. **default_tab_template** — applies to ALL tabs that don't override; put tab-bar/status-bar here
11. **swap_tiled_layout** — must define at least one for Alt+[] layout cycling to work
12. **Plugin floating vs embedded** — floating panes overlay; embedded become part of the layout permanently
13. **Yazi uses Helix** (`hx`), NOT nvim — don't assume nvim integration via yazi
14. **Session serialization** — on by default; crashed sessions restore layout but not running commands

---

## BENCHMARKS (verified Session 035)

| Operation | Speed | Notes |
|-----------|-------|-------|
| Tab switch | 14ms | `go-to-tab N` |
| Pane focus | 11ms | `move-focus` directional |
| 6-tab circuit | 89ms | Full tab sweep (14.8ms/tab) |
| Sync-tab broadcast | 47ms | All panes in tab simultaneously |
| Verified dispatch (1 pane) | 84ms | Navigate + verify + write + return |
| 9-pane verified dispatch | 760ms | All fleet panes (84ms × 9) |
| Plugin launch | ~200ms | First launch; subsequent toggles ~50ms |
| dump-screen | ~30ms | Pane content capture |

---

## FLEET ORCHESTRATION STACK

### fleet-nav.sh — IPC Safety Library (source before use)
```bash
source ~/.local/bin/fleet-nav.sh
zj_action "go-to-tab" "3"        # Safe wrapper with 150ms pacing
zj_session_alive                  # Check Unix socket for running session
navigate_to_pane $TAB $POSITION   # Directional focus (left/topright/botright)
fleet_exit_pane                   # Reliable /exit with L1+L2 checks
```

### fleet-ctl — Fleet Coordination CLI
```bash
fleet-ctl dispatch auto "Review src/api.rs"   # Auto-select idle pane
fleet-ctl dispatch 5:left "Build V2 binary"   # Specific pane
fleet-ctl batch tasks.txt                      # Parallel dispatch from file
fleet-ctl broadcast "git pull"                 # Same command to all
fleet-ctl status                               # Dashboard (cache: 5min TTL)
fleet-ctl liberate                             # Exit all idle Claude panes
fleet-ctl collect output.md                    # Gather outputs from all panes
fleet-ctl history                              # Last 20 cascade handoffs
fleet-ctl lifecycle                            # Dispatch capacity chart
```

### pane-ctl — Cross-Pane Primitives
```bash
pane-ctl send 5 "cargo test"          # Type + Enter
pane-ctl type 5 "some text"           # Type only (no Enter)
pane-ctl read 5 20                    # Read 20 lines from pane
pane-ctl exec 5 "ls" 3                # Execute, wait 3s, return diff
pane-ctl wait 5 "Compiling" 30        # Block until pattern appears (30s timeout)
pane-ctl scan 15                      # Quick summary of tabs 1-15
pane-ctl broadcast "exit" 4 5 6       # Send to multiple tabs
pane-ctl focus 5 2                    # Focus pane index 2 in tab 5
```

### fleet-inventory.sh — Hybrid L1/L2 Scanner
```bash
source ~/.local/bin/fleet-inventory.sh
fleet_scan                            # Full L1+L2 scan → /tmp/fleet-state.json
# L1 (structural): dump-layout → process type + cwd (~0.5s)
# L2 (symptomatic): dump-screen only for Claude panes (idle vs active)
# Statuses: idle-shell, idle-claude, active-claude, busy, unknown
```

---

## Obsidian Vault References

- `[[Zellij Navigation God-Tier — Session 035]]` — directional targeting, benchmarks, dispatch
- `[[Zellij — Complete Overview]]` — plugin ecosystem, official docs links
- `[[Zellij Master Skill — Bootstrap Reference]]` — 8 skills, all benchmarked
- `[[Zellij Pane Navigation Mastery — Session 027b]]` — original navigation work
- `[[Zellij Session Auto-Cleanup 2026-03-07]]` — session lifecycle management
- `~/projects/shared-context/Zellij Plugins — Room and Harpoon.md` — deep plugin testing
- `~/projects/shared-context/Zellij 0.43.1 Patched Binary.md` — patch details + rollback

---

## ENVIRONMENT VARIABLES (swarm-env.sh)

```bash
source ~/.local/bin/swarm-env.sh
# Exports:
# MEMORY_URL=http://localhost:8130        (Reasoning Memory V2)
# PANE_VORTEX_URL=http://localhost:8132   (Pane-Vortex daemon)
# ARENA=~/claude-code-workspace/swarm-stack-v2
# TASKS_DIR=~/projects/shared-context/tasks
# VAULT=~/projects/claude_code
# FLEET_TABS="6 7"
# FLEET_CACHE_TTL=300
# NVIM_SOCK=/tmp/nvim.sock
```
