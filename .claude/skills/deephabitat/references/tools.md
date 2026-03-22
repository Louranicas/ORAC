# Workspace Tools Reference (ORAC Sidecar Edition)

## ORAC Binaries (primary tools for this CWD)

### orac-sidecar (daemon, 5.5MB)
Main daemon on :8133. Axum HTTP, IPC bus client, graceful shutdown on SIGINT.
```bash
# Check if running
pgrep -f orac-sidecar && echo "UP" || echo "DOWN"
curl -s localhost:8133/health | jq .

# Start
nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &

# Restart (never chain after pkill!)
pkill -f orac-sidecar 2>/dev/null
sleep 1
nohup orac-sidecar > /tmp/orac-sidecar.log 2>&1 &

# Logs
tail -f /tmp/orac-sidecar.log
```

### orac-probe (diagnostics, 2.3MB)
Probes 6 endpoints: ORAC, PV2, SYNTHEX, ME, POVM, RM.
Returns exit code 0 (all reachable) or 1 (some fail).
```bash
orac-probe
```

### orac-client (CLI, 337KB)
CLI client scaffold: status, field, spheres, health, hooks, bridges.
```bash
orac-client status
orac-client health
```

### habitat-probe (Rust, fast system intelligence)
```bash
habitat-probe pulse     # PV + POVM + ME in ~30ms
habitat-probe sweep     # 16 services health
habitat-probe field     # Field state + decision + tunnels
habitat-probe spheres   # Active sphere listing
habitat-probe bus       # Tasks, events, cascades
habitat-probe me        # ME observer + fitness + EventBus
habitat-probe bridges   # Bridge staleness check
habitat-probe full      # Everything above
```

## YAZI (File Navigator)
Tab 2 TopRight | `~/.config/yazi/yazi.toml` | **Helix** default opener (NOT nvim)

| Key | Action |
|-----|--------|
| z/Z | Zoxide/fzf jump |
| - | Parent dir |
| CR | Open file |
| gs/g. | Sort/toggle hidden |
| Space/v | Select/visual |
| d/y/p/r | Trash/yank/paste/rename |

## BTM (Process Monitor)
Tab 2 Bottom | `btm --regex_filter "pane-vortex|synthex|povm|orac"`
Tab=cycle | /=search | t=tree | dd=kill | s=sort

**ORAC monitoring:** Filter for `orac` to see sidecar memory/CPU.

## BACON (Continuous Compiler)
Tab 3 Left | `bacon.toml` in project root

**ORAC bacon.toml uses** `CARGO_TARGET_DIR=/tmp/cargo-orac` to avoid lock conflicts.
Jobs: check, clippy, pedantic, test, gate

## ATUIN (Shell History)
SQLite: `~/.local/share/atuin/history.db`

```bash
# ORAC-relevant history
atuin search --cwd ~/claude-code-workspace/orac-sidecar --limit 20

# Find build commands
atuin search "cargo-orac" --limit 10

# Search curl commands to ORAC
atuin search "localhost:8133" --limit 10
```

## LAZYGIT (Git TUI)
Tab 3 TopRight | `~/.config/lazygit/config.yml`

| Custom Key | Action |
|------------|--------|
| F | PV2 field state (curl) |
| Y | RM write (last commit) |
| E | Open in nvim |
| Z | Sphere status |
| I | Integration matrix |
| Q | Quality gate |

**ORAC Git:**
```
Remote: git@gitlab.com:lukeomahoney/orac-sidecar.git
Branch: main
```

## NVIM (Remote Socket)
Socket: `/tmp/nvim.sock`

```bash
# Open ORAC file remotely
nvim --server /tmp/nvim.sock --remote-send ':e ~/claude-code-workspace/orac-sidecar/src/lib.rs<CR>'

# Check version
nvim --server /tmp/nvim.sock --remote-expr 'v:version'

# Get LSP error count
nvim --server /tmp/nvim.sock --remote-expr 'luaeval("vim.tbl_count(vim.diagnostic.get(nil,{severity=1}))")'
```

**8 keymap prefixes:** z u n s f g c x
**ORAC-relevant:** rust-analyzer LSP, treesitter syntax highlighting
