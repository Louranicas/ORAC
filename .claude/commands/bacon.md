# /bacon — Continuous Rust Quality as CLI Primitive

Run cargo check/clippy/test without entering bacon TUI. Auto-detects `CARGO_TARGET_DIR` per project.

## Arguments
- `$ARGUMENTS` — subcommand: check, clippy, pedantic, test, gate, summary, errors, warnings

## Usage
```
/bacon gate                         — full 4-stage quality gate (check→clippy→pedantic→test)
/bacon summary                      — one-line: "check:0err clippy:0err tests:138 passed/0 failed"
/bacon check                        — cargo check with error count
/bacon clippy                       — clippy -D warnings
/bacon pedantic                     — clippy pedantic warnings
/bacon test                         — run tests, show result line
/bacon errors                       — just the error count (for scripting)
/bacon warnings                     — just the warning count
/bacon watch . 10                   — poll check every 10s
```

## Action

```bash
CMD="${1:-summary}"
shift 2>/dev/null
bacon-exec "$CMD" "$@"
```

## Why Use This

Quality gate in one command. `bacon-exec gate` runs all 4 stages with pass/fail verdict. `bacon-exec summary` gives a one-liner for dashboards. `bacon-exec errors` returns just a number for conditional logic. Auto-detects CARGO_TARGET_DIR for ORAC (/tmp/cargo-orac), PSwarm (/tmp/cargo-prom), K7 (/tmp/cargo-k7).
