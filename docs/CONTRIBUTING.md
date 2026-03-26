# Contributing to ORAC Sidecar

This guide covers everything you need to start developing on the ORAC Sidecar
codebase: an Envoy-like proxy specialized for AI agent traffic with HTTP hooks,
Hebbian STDP learning, and RALPH evolution.

---

## Prerequisites

| Requirement | Minimum | Notes |
|-------------|---------|-------|
| Rust | 1.75+ | `rustup show` to verify; edition 2021 |
| SQLite dev headers | 3.35+ | `libsqlite3-dev` on Debian/Ubuntu |
| Linux | Any recent | Unix domain sockets are required (no Windows support) |
| `pkg-config` | Any | Needed by `rusqlite` build script |

Install on Debian/Ubuntu:

```bash
sudo apt install build-essential pkg-config libsqlite3-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

---

## Quick Start

```bash
# Clone
git clone git@gitlab.com:lukeomahoney/orac-sidecar.git
cd orac-sidecar

# Build (all features)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --features full

# Run tests
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full

# Build release binaries
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release --features full
```

The `CARGO_TARGET_DIR` override is recommended to keep the project directory
clean and avoid bloating the source tree with build artifacts.

After building, three binaries are produced:

| Binary | Purpose | Approx Size |
|--------|---------|-------------|
| `orac-sidecar` | Main daemon (HTTP hooks, RALPH, bridges) | 5.5 MB |
| `orac-client` | CLI for querying a running ORAC instance | 340 KB |
| `orac-probe` | Diagnostic tool for fleet inspection | 2.3 MB |
| `ralph-bench` | RALPH evolution benchmarks | ~2 MB |

---

## Project Structure

ORAC is organized into **8 layers** with a strict downward dependency DAG.
Higher layers may import from lower layers but never the reverse.

```
src/
  lib.rs                      # Layer declarations only
  bin/
    main.rs                   # Daemon entry point
    client.rs                 # CLI entry point
    probe.rs                  # Diagnostics entry point
    ralph_bench.rs            # Benchmark entry point
  m1_core/                    # L1: Foundation types, errors, config, traits
    mod.rs
    m01_core_types.rs
    m02_error_handling.rs
    m03_config.rs
    m04_constants.rs
    m05_traits.rs
    m06_validation.rs
    field_state.rs
  m2_wire/                    # L2: IPC client, bus types, wire protocol
    mod.rs
    m07_ipc_client.rs
    m08_bus_types.rs
    m09_wire_protocol.rs
  m3_hooks/                   # L3: HTTP hook server (Axum)
    mod.rs
    m10_hook_server.rs
    m11_session_hooks.rs
    m12_tool_hooks.rs
    m13_prompt_hooks.rs
    m14_permission_policy.rs
  m4_intelligence/            # L4: Hebbian STDP, coupling, routing
    mod.rs
    m15_coupling_network.rs
    m16_auto_k.rs
    m17_topology.rs
    m18_hebbian_stdp.rs
    m19_buoy_network.rs
    m20_semantic_router.rs
    m21_circuit_breaker.rs
  m5_bridges/                 # L5: Service bridges, SQLite blackboard
    mod.rs
    http_helpers.rs
    m22_synthex_bridge.rs
    m23_me_bridge.rs
    m24_povm_bridge.rs
    m25_rm_bridge.rs
    m26_blackboard.rs
  m6_coordination/            # L6: Conductor, cascade, tick, WASM bridge
    mod.rs
    m27_conductor.rs
    m28_cascade.rs
    m29_tick.rs
    m30_wasm_bridge.rs
    m31_memory_manager.rs
  m7_monitoring/              # L7: OpenTelemetry, metrics, dashboards
    mod.rs
    m32_otel_traces.rs
    m33_metrics_export.rs
    m34_field_dashboard.rs
    m35_token_accounting.rs
  m8_evolution/               # L8: RALPH evolution chamber
    mod.rs
    m36_ralph_engine.rs
    m37_emergence_detector.rs
    m38_correlation_engine.rs
    m39_fitness_tensor.rs
    m40_mutation_selector.rs
config/
  default.toml                # Default configuration
```

### Layer Dependency Rules

```
L8 Evolution      -> L1, L4, L5, L7
L7 Monitoring     -> L1, L2, L5
L6 Coordination   -> L1, L2, L4, L5
L5 Bridges        -> L1
L4 Intelligence   -> L1, L2
L3 Hooks          -> L1, L2
L2 Wire           -> L1
L1 Core           -> (no internal deps)
```

Imports that violate this DAG will fail at compile time due to module visibility.

### Feature Gates

| Feature | Enables | Optional Deps |
|---------|---------|---------------|
| `api` | L3 (Axum HTTP hooks) | `axum`, `tower-http` |
| `persistence` | SQLite blackboard | `rusqlite` |
| `bridges` | L5 service bridges | (none) |
| `intelligence` | L4 Hebbian/routing | `tower` |
| `monitoring` | L7 OpenTelemetry | `opentelemetry`, `opentelemetry-otlp` |
| `evolution` | L8 RALPH engine | (none) |
| `full` | All of the above | All of the above |

The `default` feature set enables all six individual features. Use `--features full`
or `--no-default-features --features api,persistence` for selective builds.

---

## Running Locally

### Required Services

ORAC bridges to several upstream services. At minimum, these must be running:

| Service | Port | Health Endpoint | Required For |
|---------|------|-----------------|-------------|
| Pane-Vortex V2 (PV2) | 8132 | `/health` | IPC bus, field state |
| POVM Engine | 8125 | `/health` | Memory persistence |

Optional but recommended:

| Service | Port | Health Endpoint | Required For |
|---------|------|-----------------|-------------|
| SYNTHEX | 8090 | `/api/health` | Thermal gating |
| Maintenance Engine | 8080 | `/api/health` | Observer metrics |
| Reasoning Memory | 8130 | `/health` | Cross-session state (TSV protocol) |

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8133` | HTTP listen port |
| `RUST_LOG` | `info` | Log level filter (`tracing` syntax) |
| `PV2_ADDR` | `127.0.0.1:8132` | Pane-Vortex V2 address |
| `SYNTHEX_ADDR` | `127.0.0.1:8090` | SYNTHEX address |
| `POVM_ADDR` | `127.0.0.1:8125` | POVM Engine address |
| `RM_ADDR` | `127.0.0.1:8130` | Reasoning Memory address |

### Starting the Daemon

```bash
RUST_LOG=orac_sidecar=info \
PORT=8133 \
PV2_ADDR=127.0.0.1:8132 \
SYNTHEX_ADDR=127.0.0.1:8090 \
POVM_ADDR=127.0.0.1:8125 \
RM_ADDR=127.0.0.1:8130 \
  /path/to/orac-sidecar
```

Verify it is running:

```bash
curl -s http://localhost:8133/health | python3 -m json.tool
```

### Configuration File

Default configuration lives in `config/default.toml`. Figment merges values
from the TOML file, then environment variables (env vars take precedence):

```toml
[server]
bind_addr = "127.0.0.1"
port = 8133

[ipc]
pv2_socket = "/run/user/1000/pane-vortex-bus.sock"
pv2_http = "http://127.0.0.1:8132"

[bridges]
synthex_addr = "127.0.0.1:8090"
me_addr = "127.0.0.1:8080"
povm_addr = "127.0.0.1:8125"
rm_addr = "127.0.0.1:8130"

[evolution]
enabled = false
emergence_cap = 5000
mutation_cooldown_generations = 10
diversity_window = 20
diversity_threshold = 0.5
```

**Important:** Bridge addresses must be raw `host:port` values without an
`http://` prefix. The bridges construct URLs internally.

---

## Code Organization

### Module Naming Convention

Every module uses a two-digit numeric prefix that determines its identity and
ordering within a layer:

- `m01` through `m06` + `field_state` belong to L1 Core
- `m07` through `m09` belong to L2 Wire
- `m10` through `m14` belong to L3 Hooks
- ...and so on through `m40` in L8 Evolution

Each layer has a `mod.rs` that re-exports public items and provides layer-level
documentation. The top-level `lib.rs` contains only `pub mod` declarations for
each layer directory.

### Binary Targets

| Binary | Source | Description |
|--------|--------|-------------|
| `orac-sidecar` | `src/bin/main.rs` | Async daemon: starts Axum, RALPH loop, bridge pollers |
| `orac-client` | `src/bin/client.rs` | CLI with subcommands for querying the daemon |
| `orac-probe` | `src/bin/probe.rs` | Fleet diagnostics and health inspection |
| `ralph-bench` | `src/bin/ralph_bench.rs` | RALPH evolution benchmarks |

---

## Adding a Module

1. **Pick the correct layer.** Determine which layer your module belongs to
   based on its responsibilities and what it needs to import.

2. **Choose the next number.** If L5 currently ends at `m26`, your new module
   is `m27` (though in practice, coordinate with the team since module numbers
   are fixed by the architecture plan).

3. **Create the file.** For example, `src/m5_bridges/m27_new_bridge.rs`.

4. **Register in `mod.rs`.** Add `pub mod m27_new_bridge;` to the layer's
   `mod.rs`, with a feature gate if the module requires optional dependencies:

   ```rust
   #[cfg(feature = "bridges")]
   pub mod m27_new_bridge;
   ```

5. **Write at least 50 tests.** Every module must include a `#[cfg(test)]`
   block at the bottom with meaningful unit tests. The minimum is 50 per module.

6. **Document all public items.** Every `pub fn`, `pub struct`, `pub enum`, and
   `pub trait` needs a `///` doc comment. Fallible functions need an `# Errors`
   section.

---

## Adding a Bridge

Service bridges live in `src/m5_bridges/` and follow a consistent pattern.

### 1. Implement the `Bridgeable` Trait

Defined in `src/m1_core/m05_traits.rs`, the trait requires four methods:

```rust
pub trait Bridgeable: Send + Sync + std::fmt::Debug {
    /// Service name (e.g. "synthex", "me").
    fn service_name(&self) -> &str;

    /// Poll the service for its current state. Returns an adjustment factor.
    fn poll(&self) -> PvResult<f64>;

    /// Post data to the service (fire-and-forget semantics).
    fn post(&self, payload: &[u8]) -> PvResult<()>;

    /// Check if the service is healthy.
    fn health(&self) -> PvResult<bool>;

    /// Whether the last poll result is stale (based on configured interval).
    fn is_stale(&self, current_tick: u64) -> bool;
}
```

### 2. Add a Circuit Breaker

Wrap outgoing calls through a `CircuitBreaker` (from `m21_circuit_breaker`)
to prevent cascading failures. The breaker tracks per-service success/failure
counts and trips to Open state after a configurable failure threshold.

### 3. Add a Consent Stub

Every bridge should include a `_consent_check()` method that returns `true`
by default. This is the extension point for per-sphere consent gating.

### 4. Register in the Hook Server

Wire your bridge into `m10_hook_server.rs` so it gets polled on the tick
cycle and reports health to the dashboard.

---

## Style Guide

These rules are enforced by `clippy` lints and code review. Violations will
cause the quality gate to fail.

### Hard Rules

- **No `unwrap()` or `expect()` outside `#[cfg(test)]` blocks.** Use the `?`
  operator and return `Result<T>`. This is enforced in `Cargo.toml` via
  `unwrap_used = "deny"` and `expect_used = "deny"`.

- **No `unsafe` code.** Zero tolerance.

- **No glob imports.** Write `use crate::m1_core::m01_core_types::{PaneId, AgentId};`
  not `use crate::m1_core::m01_core_types::*;`.

- **Doc comments on all public items.** Use `///` with backticked identifiers
  (e.g., `` `PaneId` ``). Fallible functions must have an `# Errors` section.

- **`Send + Sync` bounds on all traits** intended for shared state. Required
  for `Arc<dyn Trait>` usage.

- **`parking_lot` for locks.** Use `parking_lot::RwLock` / `Mutex`, not
  `std::sync`. Scope lock guards in brace blocks and drop before acquiring
  the next lock.

- **FMA for floating-point arithmetic.** Use `a.mul_add(b, c)` instead of
  `a * b + c` to avoid intermediate rounding.

- **Owned returns from `RwLock` reads.** Always `.read().get(key).cloned()`;
  never return a reference into a lock guard.

- **No `println!()` in library or daemon code.** Use `tracing::info!()`,
  `tracing::warn!()`, etc.

- **No `#[allow(clippy::...)]` directives.** Fix the code instead.

### Conventions

- **Builder pattern** for configuration structs: chain setters returning `Self`,
  use `const fn` where possible.

- **Newtypes for domain identifiers:** `ModuleId`, `AgentId`, `PaneId`,
  `Severity` -- not raw strings or integers.

- **Error accumulation:** Validation functions collect all errors into a `Vec`
  and join with `"; "` rather than failing on the first.

- **Phase arithmetic:** Always call `.rem_euclid(TAU)` after addition or
  subtraction on phase angles.

---

## Quality Gate

Every change must pass the full quality gate before it can be merged. Run
these four commands in order -- each must produce **zero errors and zero
warnings** before proceeding to the next:

```bash
# 1. Compile check
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check

# 2. Clippy (deny all warnings)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings

# 3. Clippy pedantic (additional strictness)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic

# 4. Tests (all features, release mode)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full
```

Or as a single command:

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full
```

The `pedantic` lint group catches subtle issues like missing docs, needless
borrows, and redundant closures. Zero tolerance means zero tolerance -- no
warnings are acceptable.

---

## Submitting Changes

1. **Branch from `main`.** Use a descriptive branch name:
   `fix/breaker-halfopen-timeout` or `feat/new-bridge-widget`.

2. **Run the full quality gate** (see above). All four stages must pass cleanly.

3. **Write a clear commit message.** First line is a short summary (imperative
   mood, under 72 characters). Body explains *why* the change was made.

4. **Push to GitLab** and open a merge request against `main`:
   ```bash
   git push -u origin feat/my-branch
   ```

5. **Verify CI passes.** The merge request pipeline runs the same quality gate.

### Remote

```
origin  git@gitlab.com:lukeomahoney/orac-sidecar.git
```

### What Reviewers Look For

- Quality gate passes (check, clippy, pedantic, tests)
- Layer dependency DAG is respected (no upward imports)
- New public items have doc comments with `# Errors` where applicable
- Test count meets the 50-per-module minimum
- No `unwrap`, `expect`, `unsafe`, or glob imports outside test code
- Lock guards are scoped correctly (no deadlock risk)
- Bridge addresses are raw `host:port` (no `http://` prefix)
