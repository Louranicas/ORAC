# Plugin Ecosystem (11 WASM Plugins)

**Directory:** `~/.config/zellij/plugins/` (18MB total)
**Binary format:** wasm32-wasip1 (Rust → WASM)

## Floating Plugins (6, keybind-activated)

### Harpoon — Pane Bookmarks (`Alt+v`)
**File:** harpoon.wasm (1.3MB) | **Source:** Nacho114/harpoon (ThePrimeagen port)
**Purpose:** Bookmark panes, jump across tabs instantly

| Key | Action |
|-----|--------|
| `a` | Add current pane to bookmark list |
| `A` | Add ALL session panes to list |
| `j/k` or arrows | Navigate bookmark list |
| `Enter` or `l` | Jump to selected pane (cross-tab) |
| `d` | Remove pane from list |
| `Esc` / `Ctrl c` | Close Harpoon |

**Behavior:** Panes auto-remove when closed. Name changes propagate. Cross-tab jumping switches both tab AND pane focus in one action.

**When to use:** Repeatedly jumping between specific panes across tabs (e.g., editor ↔ fleet pane ↔ monitor).

### Swarm Orchestrator — Fleet Dashboard (`Alt+w`)
**File:** swarm-orchestrator.wasm (1.1MB) | **Source:** gitlab:lukeomahoney/swarm-orchestrator
**Config:** quality_threshold=0.80, max_iterations=5, floating, move_to_focused_tab
**Purpose:** Fleet coordination UI, RALPH iteration loop

**Pipe protocol:**
```bash
# All commands via zellij pipe
zellij pipe -p "file:~/.config/zellij/plugins/swarm-orchestrator.wasm" -n cmd -- '<JSON>'

# JSON commands:
{"Spawn":{"persona":"reviewer","task":"Review auth module"}}
{"Dispatch":{"agent_id":"A1","task":"Run tests"}}
{"DispatchAll":{"task":"git pull"}}
{"Ralph":{"task":"Optimize API","iterations":5,"threshold":0.80}}
{"Status":{}}
{"Kill":{"agent_id":"A1"}}
{"Threshold":{"value":0.90}}
```

**CLI wrapper:** `swarm-ctl` (see dispatch-patterns.md)

### Ghost — Command Palette (`Alt+g`)
**File:** ghost.wasm (2.4MB)
**Config:** shell=bash, shell_flag=-ic (interactive, loads aliases)
**Purpose:** Fuzzy-filter command launcher

Pre-configured completions:
- lazygit, btm, litecli (service_tracking.db)
- nvim --listen /tmp/nvim.sock
- swarm-ctl status/ralph, claude, yazi
- w3m (web docs), pane-ctl scan 15
- curl to RM (8130) and VMS (8120)

**Usage:** Open with Alt+g → type to fuzzy filter → Enter launches in new pane.

### Monocle — Fullscreen Focus (`Alt+m`)
**File:** monocle.wasm (2.6MB)
**Purpose:** Zoom single pane to fullscreen

Toggle on/off. Focus on one pane, everything else hidden. Useful for code review or reading long output.

### Multitask — Parallel Task Viewer (`Alt+t`)
**File:** multitask.wasm (1.3MB)
**Purpose:** Multi-pane task coordination

View and manage multiple concurrent tasks across panes.

### Room — Fuzzy Tab/Pane Switcher (`Ctrl+y`)
**File:** room.wasm (1.1MB)
**Config:** floating, ignore_case=true, quick_jump=true

| Key | Action |
|-----|--------|
| Type text | Fuzzy filter tabs/panes (e.g., "Mon" → Monitor) |
| Number key | Quick jump to tab N (with quick_jump=true) |
| `Enter` | Jump to selected item |
| `Tab/Up/Down` | Cycle through filtered list |
| `Esc` / `Ctrl c` | Close without jumping |

**Pipe command:** `zellij pipe --plugin file:~/.config/zellij/plugins/room.wasm --name focus-pane -- <pane_id>`

**When to use:** Quick tab switching when you know partial name. Faster than go-to-tab for discovery.

## Background Plugins (2, auto-loaded)

### Autolock — TUI App Protection
**File:** zellij-autolock.wasm (1.1MB)
**Config:**
```kdl
triggers "nvim|vim|pi|git|lazygit"   // Apps that trigger lock
reaction "lock"                       // Lock Zellij when triggered
watch_triggers "nvim|vim|pi"          // Subset to watch continuously
watch_interval "1.0"                  // Check every 1 second
```
**Purpose:** Prevents accidental Zellij keybinds when editing in nvim or using lazygit. Locks mode automatically; Ctrl+g to unlock manually if needed.

### Attention — Tab Activity Indicators
**File:** zellij-attention.wasm (1.1MB)
**Config:** default (no custom params)
**Purpose:** Highlights tabs that have new output since you last looked. Visual cue for fleet monitoring.

## Utility Plugins (3, no keybind)

### zjstatus — Custom Status Bar
**File:** zjstatus.wasm (3.7MB) — largest plugin
**Status:** Available but not actively configured in current setup (using built-in status-bar)
**Purpose:** Highly customizable status bar with segments, colors, conditional display

### zellij-send-keys — Key Injection
**File:** zellij-send-keys.wasm (980KB)
**Purpose:** Programmatically send keystrokes to panes. Used by automation scripts.

### Swarm Orchestrator V2
**File:** swarm-orchestrator-v2.wasm (992KB)
**Status:** Alternative binary, not bound to keybind
**Purpose:** Updated orchestrator (may replace V1)

## CLI Launch Commands (All Plugins)

```bash
# Launch any plugin as floating
zellij action launch-or-focus-plugin --floating "file:~/.config/zellij/plugins/NAME.wasm"

# With config
zellij action launch-or-focus-plugin --floating \
  "file:~/.config/zellij/plugins/room.wasm" \
  -c "ignore_case=true,quick_jump=true"

# GOTCHA: launch-or-focus-plugin is a TOGGLE
# If plugin is already open, calling this CLOSES it
```

## Plugin Decision Tree

```
Need to jump to a specific pane repeatedly? → Harpoon (Alt+v)
Need to find a tab by name?                 → Room (Ctrl+y)
Need to launch a tool quickly?              → Ghost (Alt+g)
Need to focus on one pane fullscreen?       → Monocle (Alt+m)
Need fleet coordination dashboard?          → Swarm (Alt+w)
Need to see multiple tasks at once?         → Multitask (Alt+t)
```
