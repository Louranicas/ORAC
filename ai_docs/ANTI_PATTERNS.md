# ORAC Sidecar — Anti-Patterns

> 17 banned patterns. Severity: CRITICAL (blocks merge), HIGH (must fix before review), MEDIUM (fix in same PR), LOW (fix opportunistically).

## Rust Anti-Patterns

| ID | Pattern | Severity | Fix |
|----|---------|----------|-----|
| A1 | `unwrap()` | CRITICAL | Use `?`, `ok_or()`, `ok_or_else()` |
| A2 | `expect()` | CRITICAL | Use `?` with descriptive `OracError` variant |
| A3 | `unsafe` | CRITICAL | Redesign. No unsafe in ORAC. |
| A4 | `println!`/`eprintln!` in daemon | HIGH | Use `tracing::info!`/`tracing::error!` |
| A5 | `chrono`/`SystemTime` | HIGH | Use `Timestamp` newtype (P6) |
| A6 | `&mut self` on shared traits | HIGH | Use `&self` + interior mutability (P2) |
| A7 | Glob imports (`use foo::*`) | MEDIUM | Explicit imports only |
| A8 | `#[allow(clippy::...)]` | MEDIUM | Fix the lint. Zero suppressions. |
| A9 | `String::new()` + `push_str` chains | LOW | Use `format!()` or `write!()` |
| A14 | Returning `&T` through `RwLock` | HIGH | Return owned `.cloned()` (P7) |
| A15 | Unbounded channels | MEDIUM | Use `tokio::sync::mpsc::channel(CAP)` with backpressure |
| A16 | `println` for logging | HIGH | Use `tracing` crate with structured fields |
| A17 | Mono-parameter mutation (BUG-035) | HIGH | Never mutate a single field in isolation when it participates in an invariant group |

## Operational Anti-Patterns

| ID | Pattern | Severity | Fix |
|----|---------|----------|-----|
| A10 | `pkill foo && cp ...` | CRITICAL | Separate commands. `pkill` exit 144 kills `&&` chain. Use `;` or separate invocations. |
| A11 | `cp` without `\` prefix | HIGH | Use `\cp -f` to bypass interactive alias. |
| A12 | JSON body to Reasoning Memory | HIGH | Use TSV format: `POST /put` with `key\tvalue` body. |
| A13 | `git status -uall` | HIGH | Omit `-uall` flag. Causes OOM on large repos. |

## Detailed Explanations

### A1: `unwrap()` — CRITICAL

```rust
// BANNED
let config = load_config().unwrap();

// CORRECT
let config = load_config().map_err(|e| OracError::Config(e))?;
```

Every `unwrap()` is a potential panic. ORAC is a long-running proxy daemon — panics kill all proxied agent traffic.

### A2: `expect()` — CRITICAL

```rust
// BANNED
let port = env::var("PORT").expect("PORT must be set");

// CORRECT
let port = env::var("PORT")
    .map_err(|_| OracError::MissingEnv("PORT"))?
    .parse::<u16>()
    .map_err(|e| OracError::InvalidPort(e))?;
```

`expect()` is `unwrap()` with a message. Still panics. Still banned.

### A3: `unsafe` — CRITICAL

```rust
// BANNED — no exceptions
unsafe { std::ptr::read(ptr) }

// CORRECT — redesign to use safe abstractions
```

Zero `unsafe` blocks in ORAC. If you think you need `unsafe`, the design is wrong.

### A4: `println!`/`eprintln!` — HIGH

```rust
// BANNED
println!("Request routed to {}", backend);

// CORRECT
tracing::info!(backend = %backend, "request routed");
```

Daemon output goes nowhere useful. Structured tracing integrates with OTel (L7).

### A10: Chain After `pkill` — CRITICAL

```bash
# BANNED — pkill returns 144 (signal + 128), &&-chain aborts
pkill orac-sidecar && \cp -f target/release/orac-sidecar ~/.local/bin/

# CORRECT — separate commands
pkill orac-sidecar
\cp -f target/release/orac-sidecar ~/.local/bin/
```

This has caused multiple failed deployments. `pkill` exit code 1 (no match) or 128+signal kills the `&&` chain.

### A15: Unbounded Channels — MEDIUM

```rust
// BANNED — memory grows without bound under load
let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

// CORRECT — bounded with backpressure
const EVENT_CAP: usize = 4096;
let (tx, rx) = tokio::sync::mpsc::channel(EVENT_CAP);
```

### A17: Mono-Parameter Mutation (BUG-035) — HIGH

```rust
// BANNED — k_mod participates in invariant with weight_exponent
state.k_mod = new_value;

// CORRECT — update invariant group atomically
state.update_coupling(new_k_mod, new_weight_exp)?;
```

When a field participates in a multi-field invariant, mutating it alone creates transient inconsistency. Always update the invariant group as a unit.
