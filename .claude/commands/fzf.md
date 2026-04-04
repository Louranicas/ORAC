# /fzf — Fuzzy Intelligence as CLI Primitive

Non-interactive fuzzy matching across files, services, agents, skills, and endpoints.
Uses `--filter` mode for deterministic pipeline output — no TTY needed.

## Arguments
- `$ARGUMENTS` — subcommand + pattern

## Usage
```
/fzf files server src/             — find files matching "server"
/fzf code resonance src/           — grep code for "resonance"
/fzf services orac                 — match service by name
/fzf agents explorer               — match PSwarm agents
/fzf scripts pswarm                — match Atuin scripts
/fzf skills sweep                  — match CC skills
/fzf modules evolution src/        — match Rust module names
/fzf personas debug                — match PSwarm personas
/fzf endpoints health              — match API endpoints
/fzf history cargo                 — search command history
```

## Action

```bash
CMD="${1:-help}"
shift 2>/dev/null
fzf-exec "$CMD" "$@"
```

## Why Use This

Fuzzy matching is faster than exact grep for discovery. `fzf-exec agents explorer` finds all 10 explorer agents instantly. `fzf-exec endpoints agent` shows every agent-related API route. `fzf-exec services thermal` maps to `SYNTHEX:8090`. The `--filter` mode makes fzf a pipeline tool — composable with other primitives.
