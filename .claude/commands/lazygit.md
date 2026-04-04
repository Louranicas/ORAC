# /lazygit — Git Intelligence as CLI Primitive

Access git operations without entering lazygit TUI. Status, log, diff, blame, branches — all as one-liners.

## Arguments
- `$ARGUMENTS` — subcommand: status, log, diff, changed, blame, branches, stats, remotes

## Usage
```
/lazygit status                     — branch, dirty count, ahead/behind
/lazygit log 10                     — last 10 commits with graph
/lazygit diff                       — staged + unstaged + untracked summary
/lazygit changed                    — list all changed files
/lazygit blame src/server.rs 1,20   — blame lines 1-20
/lazygit branches                   — branches sorted by date
/lazygit stats                      — commits, files, date range
/lazygit remotes                    — remote URLs
/lazygit worktrees                  — list worktrees
/lazygit authors 5                  — top 5 contributors
```

## Action

```bash
CMD="${1:-status}"
shift 2>/dev/null
lazygit-exec "$CMD" "$@"
```

## Why Use This

Git intelligence without context switching. `lazygit-exec status` gives branch + dirty + ahead/behind in one call. `lazygit-exec changed` shows what needs committing. `lazygit-exec blame` traces when bugs were introduced. Every call generates STDP timing pairs — git operations are a distinct tool category that diversifies the coupling network.
