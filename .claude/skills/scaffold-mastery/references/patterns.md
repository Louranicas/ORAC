# Scaffold Implementation Patterns

## plan.toml Complete Specification

```toml
# ═══════════════════════════════════════════════════════════════
# Scaffold Plan — plan.toml
# ═══════════════════════════════════════════════════════════════

[metadata]
name = "my-service"                    # Required: crate name
description = "Custom Rust service"    # Required: crate description
version = "0.1.0"                      # Required: semver version
edition = "2021"                       # Default: "2021"
rust_version = "1.75"                  # Default: "1.75"
authors = ["Dev <dev@example.com>"]    # Optional
license = "MIT"                        # Optional (default in README)
repository = ""                        # Optional
port = 8200                            # Optional: server port
service_id = "my-service"              # Optional: devenv service ID
devenv_batch = 3                       # Optional: devenv startup batch

[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = { version = "2" }
tokio = { version = "1", features = ["full"] }  # Needed for async test_kind

[features]
default = ["api"]
api = []
persistence = []

[quality]
gate = ["check", "clippy", "pedantic", "test"]
min_tests_per_module = 50             # Default: 50
deny_unwrap = true                     # Default: true
deny_unsafe = true                     # Default: true
k7_compliance = true                   # Adds agent dispatch to MASTERPLAN
k7_compliance_target = 99.0
hebbian_feedback = true

[consent]
sphere_can_add_modules = true
sphere_can_skip_layers = true
modulation_not_command = true          # "should" not "must" for non-safety rules
implementation_order = "recommended"   # recommended|required

[[layers]]
key = "L1"                            # Unique layer identifier
dir_name = "m1_core"                  # src/ subdirectory name
name = "Core"                         # Human-readable name
description = "Core types"            # Used in docs
depends_on = []                       # Layer dependencies (layer keys)
feature_gate = "api"                  # Optional: feature gate this layer
rationale = "Why this layer exists"   # Optional: flows into ai_specs/

[[modules]]
layer = "L1"                          # Must match a layer key
name = "m01_types"                    # Module filename (without .rs)
description = "Core types"            # Used in docs and module header
test_kind = "unit"                    # unit|integration|async|property
depends_on = ["m01_types"]            # Per-module dependencies (module names)
quality_override = "experimental"     # strict|standard|experimental

[[bin_targets]]
name = "my-service"                   # Binary name
path = "src/bin/main.rs"             # Binary source path
kind = "daemon"                       # daemon|client|probe|tool

[implementation]
order = ["L1", "L2"]                  # Layer build order

[config.server]                       # Generates config/default.toml
bind_addr = "\"127.0.0.1\""
port = 8200
```

## Test Kind Scaffolds

| Kind | Scaffold | Requires |
|------|----------|----------|
| `unit` (default) | `#[test] fn scaffold_compiles()` | Nothing |
| `integration` | `#[test] fn integration_scaffold()` | Nothing |
| `async` | `#[tokio::test] async fn async_scaffold()` | `tokio` in deps |
| `property` | `#[test] fn property_scaffold()` with loop | Nothing |

**Gotcha:** If `async` is specified but `tokio` is not in dependencies, falls back to unit test.

## Per-Module Dependencies (GAP 2)

Module `depends_on` generates:
- `//! Dependencies: \`m01_types\`, \`m02_errors\`` in module header
- Dependency column in `ai_docs/CODE_MODULE_MAP.md`

## Consent Language (NA-GAP-1)

When `[consent] modulation_not_command = true`:
- Safety rules: always "must" (no unwrap, no unsafe)
- Non-safety rules: "should" (doc comments, test count)

## Bridge Consent Stubs (NA-GAP-5)

Modules with "bridge" in name or layer get:
```rust
fn _consent_check() -> bool { true }
```

## Sphere Attribution (NA-GAP-3)

Every module scaffold includes:
```rust
//! Implemented by: (sphere attribution)
//! Session: (recorded on implementation)
```

## Rust Best Practices (Enforced)

- **Error handling**: `thiserror` enum + `Result<T, Error>` alias, never panic
- **Concurrency**: `parking_lot::RwLock` + `Arc` sharing, explicit lock ordering
- **Config**: `Figment` with TOML defaults + env overlay
- **Serialization**: `serde` derive on all public types
- **Identifiers**: `uuid::Uuid` v4 for unique IDs
- **Time**: `chrono::DateTime<Utc>` with serde support
- **Logging**: `tracing` (not `log`) with structured fields

## Anti-Patterns (Avoided)

- `unwrap()`/`expect()` outside tests — denied via `[lints.clippy]`
- `unsafe` blocks — zero tolerance
- `stdout` in daemons — `SIGPIPE` death
- Global mutable state — use `Arc<RwLock<T>>` instead
- `#![allow(clippy::...)]` — fix the code, don't suppress the warning
- Panic-based error handling — use `?` propagation
- `mod.rs` that re-exports blindly — export only public API

## Module Naming Convention

```
m01_core_types.rs    # 2-digit prefix for ordering
m02_error_handling.rs
...
m41_evolution.rs
```

Layer directories: `m1_foundation/`, `m2_services/`, etc.

## Quality Gate Protocol

```bash
cargo check               # Compilation
cargo clippy -- -D warnings     # Standard lints
cargo clippy -- -D warnings -W clippy::pedantic  # Strict lints
cargo test --lib          # All tests
```

Run after EVERY module. Never batch fixes.

## Doc Comment Rules

- Backtick all identifiers: `` `PaneId` ``, `` `m01_core_types` ``
- Use `//!` for module-level docs, `///` for item-level
- Every public item must have a doc comment
- Include `# Errors` section on fallible functions
