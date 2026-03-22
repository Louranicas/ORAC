# Troubleshooting

## Log Location

```bash
tail -50 /tmp/zellij-1000/zellij-log/zellij.log
```

## Common Failures

### SIGABRT on Rapid IPC (0.43.x)
**Symptom:** Zellij crashes when running many `zellij action` commands in quick succession.
**Cause:** Race condition in 0.43.x IPC handling.
**Fix:** Use fleet-nav.sh which enforces 150ms pacing between calls.
```bash
source ~/.local/bin/fleet-nav.sh
zj_action "go-to-tab" "3"    # Safe (150ms paced)
# NOT: zellij action go-to-tab 3 && zellij action move-focus left  (too fast)
```

### Plugin Crash on Unload
**Symptom:** Error flood in logs after plugin closes, possible stale state.
**Cause:** Stale cached state not cleaned up; mutex poisoning.
**Fix:** Running patched 0.43.1 binary that handles this. If unpatched:
```bash
# Rollback details: ~/projects/shared-context/Zellij 0.43.1 Patched Binary.md
```

### Session Won't Start (Port Conflict)
**Symptom:** `zellij` hangs or errors about socket in use.
**Fix:**
```bash
# Check for existing sessions
zellij list-sessions
# Kill orphaned servers
zellij kill-all-sessions
# Or force socket cleanup
rm -f /run/user/1000/zellij/0.43.1/*
```

### Layout Syntax Error
**Symptom:** `Error parsing layout` on session start.
**Common causes:**
- Plugin location must be on separate line from pane node (0.43.1 requirement)
- String values must be quoted: `size="50%"` not `size=50%`
- `size=1` (number) for fixed rows, `size="50%"` (string) for percentage
- Missing `children` node in `default_tab_template`
**Debug:** Load layout directly: `zellij --layout /path/to/layout.kdl --session test`

### Pane Focus Lost After Plugin Dismiss
**Symptom:** After closing floating plugin, focus is on wrong pane.
**Fix:** Use directional move-focus to re-target:
```bash
zellij action move-focus left; zellij action move-focus left  # Back to left pane
```

### Tab Sync Stuck On
**Symptom:** Everything you type goes to ALL panes in the tab.
**Cause:** `toggle-active-sync-tab` was turned on but never turned off.
**Fix:**
```bash
zellij action toggle-active-sync-tab    # Toggle OFF
# Or switch to Tab mode (Ctrl+t) and press 's' to toggle sync
```

### Autolock Won't Release
**Symptom:** Zellij appears locked even after exiting nvim/lazygit.
**Cause:** Autolock plugin watch_interval may not have caught the exit yet.
**Fix:** Press `Ctrl+g` twice (lock → unlock cycle).

### Fleet Pane State Stale
**Symptom:** `fleet-ctl status` shows pane as idle but it's actually active.
**Cause:** Cache TTL is 300s (5 minutes).
**Fix:**
```bash
fleet-ctl status --live    # Force fresh scan
# Or bypass cache entirely:
pane-ctl read 5 10         # Direct pane content read
# Or dump screen:
zellij action go-to-tab 5
zellij action dump-screen /tmp/check.txt
cat /tmp/check.txt
```

### Sidecar Disconnects (BUG-028)
**Symptom:** `/tmp/swarm-sidecar.log` shows "bus disconnected" repeatedly.
**Cause:** V1 binary doesn't have V2 wire compat fix. Sidecar handshake fails.
**Fix:** Deploy V2 binary (has V1 compat layer for subscribe/event responses).
```bash
# Check sidecar
pgrep -x swarm-sidecar && echo "UP" || echo "DOWN"
tail -5 /tmp/swarm-sidecar.log

# Restart sidecar
pkill swarm-sidecar; sleep 1
PANE_VORTEX_ID="swarm-sidecar" nohup ~/.local/bin/swarm-sidecar > /tmp/swarm-sidecar.log 2>&1 &
```

### Plugin Not Loading
**Symptom:** Plugin doesn't appear when pressing keybind.
**Causes:**
1. WASM file missing or corrupted: `ls -la ~/.config/zellij/plugins/NAME.wasm`
2. Keybind in wrong mode: plugin keybinds must be in `shared_except "locked"` block
3. Path must use `file:` protocol: `file:~/.config/zellij/plugins/NAME.wasm`
4. Plugin already loaded (toggle behavior): pressing keybind again CLOSES it

### Services Not Starting After Zellij Restart
**Symptom:** devenv shows fewer than 16 services after session restart.
**Cause:** Session serialization restored layout but not background processes.
**Fix:**
```bash
# Kill rogue port occupants
for port in 8080 8081 8090 8100 8101 8102 8103 8104 8105 8110 8120 8125 8130 8132 9001 10001; do
  pid=$(ss -tlnp "sport = :$port" 2>/dev/null | grep -oP 'pid=\K[0-9]+' | head -1)
  [[ -n "$pid" ]] && kill "$pid" 2>/dev/null
done
sleep 2
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start
```

## Recovery Procedures

### Full Session Recovery
```bash
# 1. Kill everything cleanly
zellij kill-all-sessions
pkill -f pane-vortex || true
pkill -f swarm-sidecar || true

# 2. Clear stale state
rm -f /run/user/1000/zellij/0.43.1/*
rm -f /tmp/fleet-state.json
rm -f /tmp/swarm-*.jsonl /tmp/swarm-*.pipe /tmp/swarm-*.pid

# 3. Restart services
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml stop
sleep 2
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start

# 4. Launch fresh session with gold standard layout
zellij --layout synth-orchestrator --session habitat
```

### Patched Binary Details
Running Zellij 0.43.1 with two patches:
1. **Plugin Crash Resilience:** Stale cached state cleanup on unload, mutex poisoning recovery
2. **IPC Error Log Suppression:** Rate-limiting error floods (first 3 logged, then suppressed)

Rollback procedure and build-from-source instructions:
`~/projects/shared-context/Zellij 0.43.1 Patched Binary.md`
