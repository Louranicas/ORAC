# /yazi — File Manager Intelligence as CLI Primitive

Access yazi-style file operations: tree, find, inventory, preview, size analysis.

## Arguments
- `$ARGUMENTS` — subcommand: tree, find, rust, large, recent, preview, size, count, orphans

## Usage
```
/yazi rust src/                    — Rust file inventory with LOC counts
/yazi large . 10                   — 10 largest files
/yazi find "*.toml"                — find files by pattern
/yazi recent 10                    — 10 most recently modified files
/yazi preview src/server.rs        — preview with metadata
/yazi size .                       — directory size breakdown
/yazi count src/                   — file count by extension
/yazi orphans .                    — git untracked files
/yazi duplicates src/              — duplicate filenames
```

## Action

```bash
CMD="${1:-tree}"
shift 2>/dev/null
yazi-exec "$CMD" "$@"
```

## Why Use This

File system intelligence without leaving the CLI. `yazi-exec rust` gives instant LOC inventory across any Rust project. `yazi-exec large` finds bloated files. `yazi-exec recent` shows what changed. Every call generates STDP timing pairs — diverse file operations contribute to Hebbian learning.
