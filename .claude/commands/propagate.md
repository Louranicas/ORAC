# /propagate — Push Habitat optimisations across all services

Updates every service's CLAUDE.md and CLAUDE.local.md with the current slash command table. Run this after creating or modifying any slash command.

```bash
BLOCK='
## Habitat Slash Commands (Session 064)

> These commands work from ANY service directory. They are defined at `orac-sidecar/.claude/commands/`.

| Command | What It Does |
|---------|-------------|
| `/gate` | 4-stage quality gate: check → clippy → pedantic → test |
| `/sweep` | Probe 17 services + ORAC + thermal + field |
| `/deploy-orac` | Build → deploy → verify (encodes all traps) |
| `/acp` | Adversarial Convergence Protocol (3 rounds) |
| `/battern` | Fleet batch dispatch: roles → gate → collect |
| `/nerve` | Continuous Nerve Center dashboard (10s refresh) |
| `/propagate` | Push command table to all service CLAUDE.md files |
'

UPDATED=0
for dir in \
  /home/louranicas/claude-code-workspace/pane-vortex-v2 \
  /home/louranicas/claude-code-workspace/pane-vortex \
  /home/louranicas/claude-code-workspace/the_maintenance_engine \
  /home/louranicas/claude-code-workspace/the_maintenance_engine_v2 \
  /home/louranicas/claude-code-workspace/developer_environment_manager \
  /home/louranicas/claude-code-workspace/developer_environment_manager/synthex \
  /home/louranicas/claude-code-workspace/the-orchestrator \
  /home/louranicas/claude-code-workspace/vortex-memory-system \
  /home/louranicas/claude-code-workspace/devops_engine_v2; do
  for target in "$dir/CLAUDE.md" "$dir/CLAUDE.local.md"; do
    if [ -f "$target" ]; then
      # Remove old block if present, append fresh
      sed -i '/^## Habitat Slash Commands/,/^$/d' "$target" 2>/dev/null
      echo "$BLOCK" >> "$target"
      UPDATED=$((UPDATED+1))
    fi
  done
done

echo "Propagated to $UPDATED files across $(echo /home/louranicas/claude-code-workspace/*/CLAUDE.md | wc -w) services"
```
