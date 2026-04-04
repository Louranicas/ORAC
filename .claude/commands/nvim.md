# /nvim — Neovim as CLI Primitive

Use neovim's LSP (rust-analyzer) for diagnostics, symbols, and formatting via RPC socket.

## Arguments
- `$ARGUMENTS` — subcommand: diag, symbols, format, buffers, open

## Usage
```
/nvim diag src/server.rs          — LSP diagnostics (errors, warnings)
/nvim symbols src/routing.rs      — document symbols (functions, structs)
/nvim format src/evolution.rs     — LSP format file
/nvim buffers                     — list open buffers
/nvim open src/main.rs 42         — open file at line
```

## Action

```bash
CMD="${1:-buffers}"
shift 2>/dev/null
nvim-exec "$CMD" "$@"
```

## Why Use This

nvim-exec gives access to rust-analyzer's LSP without running `cargo check`. Diagnostics are instant (~100ms vs ~5s for cargo). Symbols show the full module structure. Format applies rustfmt through LSP. Every call generates STDP timing pairs through ORAC hooks, contributing to Hebbian learning diversity.
