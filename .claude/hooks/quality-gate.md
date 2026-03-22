# ORAC Quality Gate Hook

## Purpose
Enforce the 4-step quality gate before any deployment or commit.

## Gate Sequence (MANDATORY order)
1. `cargo check` — compilation
2. `cargo clippy -- -D warnings` — standard lints
3. `cargo clippy -- -D warnings -W clippy::pedantic` — pedantic lints
4. `cargo test --lib --release` — all tests pass

## Environment
```bash
CARGO_TARGET_DIR=/tmp/cargo-orac
```

## Rules Enforced
- Zero `unwrap()` or `expect()` outside `#[cfg(test)]`
- Zero `unsafe` blocks
- Zero `println!` / `eprintln!` in production code (tracing only)
- All public items have `///` doc comments
- All fallible public functions have `# Errors` sections
- Backticked identifiers in docs
- FMA for multi-step float arithmetic
- Import ordering: `std` → external → `crate::`
- 50+ tests per layer minimum

## When to Run
- Before every commit
- Before deploying binary
- After integrating candidate modules
- After any ADAPT changes to bridge modules

## Full Command
```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```
