# /battern — Patterned Batch Dispatch for Fleet Panes

Structured multi-pane work with unique roles, gate checking, and collection.

## Arguments
- `$ARGUMENTS` — The task to distribute across fleet panes

## Protocol

### Step 1: Design
Choose topology, assign unique roles per pane, define output paths and gate criteria.

### Step 2: Dispatch
Each pane gets a unique role + output path:
```bash
# Example: 3-pane investigation
zellij action go-to-tab 4; zellij action move-focus left; zellij action move-focus left
zellij action write-chars "Role: Investigator — explore X, write to ~/projects/shared-context/tasks/battern-run1-investigator.md"
zellij action write 13
```

### Step 3: Gate
Poll for completion. Do NOT proceed until N sources deliver:
```bash
found=0
for f in ~/projects/shared-context/tasks/battern-run1-*.md; do
  [ -f "$f" ] && [ "$(wc -l < "$f")" -gt 10 ] && found=$((found+1))
done
echo "Gate: $found/N delivered"
```

### Step 4: Collect
Gather all sources into single document for synthesis.

### Step 5: Synthesize
Orchestrator reads collection, produces final output.

## 5 Battern Types
- **Investigation** — explore a domain
- **Adversarial** — attack findings
- **Verification** — check assumptions against source
- **Monitoring** — star tracker, health probes
- **Implementation** — parallel deploy across modules

## Fleet Pane Map
- Tab 4 (ALPHA): Left + TopRight + BotRight
- Tab 5 (BETA): Left + TopRight + BotRight
- Tab 6 (GAMMA): Left + TopRight + BotRight
- Navigate: `move-focus left/right/up/down` (NEVER focus-next-pane)
- Always return to Tab 1 after dispatch

## Reference
- [[Battern — Patterned Batch Dispatch for Claude Code Fleets]] in Obsidian
