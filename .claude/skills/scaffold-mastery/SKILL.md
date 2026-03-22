---
name: scaffold-mastery
user-invocable: true
description: Generate plan-driven Rust microservice scaffolds from plan.toml, or default 8-layer/41-module scaffolds. Supports custom layers, modules, test kinds, feature gates, consent configuration, and per-module dependencies. Triggers on scaffold, generate scaffold, new project, create microservice, init codebase, build from scratch, scaffold practice, or when Claude needs to create a new Rust project structure from zero.
---

# Scaffold Mastery

Generate complete Rust microservice scaffolds that pass check + clippy + pedantic + test with zero warnings from first compilation.

## Quick Start

Run `scaffold-gen --help` for usage. The binary generates everything without consuming context tokens.

```bash
scaffold-gen --from-plan plan.toml <dir>   # Plan-driven scaffold
scaffold-gen <project-dir> <project-name>  # Default 8L/41M scaffold
scaffold-gen --verify <project-dir>        # Verify existing scaffold
```

## Plan-Driven Mode

Create a `plan.toml` to define custom project architecture:

```toml
[metadata]
name = "my-service"
description = "Custom Rust microservice"
version = "0.1.0"
port = 8200

[dependencies]
serde = { version = "1", features = ["derive"] }
thiserror = { version = "2" }

[quality]
min_tests_per_module = 50
deny_unwrap = true

[consent]
modulation_not_command = true        # "should" vs "must" for non-safety rules
implementation_order = "recommended" # recommended|required

[[layers]]
key = "L1"
dir_name = "m1_core"
name = "Core"
description = "Foundation"
depends_on = []
rationale = "Why this layer exists"

[[modules]]
layer = "L1"
name = "m01_types"
description = "Core types"
test_kind = "unit"                   # unit|integration|async|property
depends_on = ["m01_types"]           # per-module dependencies
```

See `references/patterns.md` for the complete plan.toml specification.

## What Gets Generated

- Layer directories with module scaffolds (each with test matching `test_kind`)
- `lib.rs` + `mod.rs` per layer (with feature gates when specified)
- Binary stubs per `bin_targets` (daemon/client/probe/tool)
- `Cargo.toml` with dependencies, features, clippy lints
- `bacon.toml` with quality gate jobs
- `CLAUDE.md` with dynamic architecture block + rules
- `MASTERPLAN.md` with implementation order + dependency graph
- `README.md`, `CHANGELOG.md`, `CONTRIBUTORS.md`
- `ai_docs/` with per-layer module docs + code module map
- `ai_specs/` with per-layer specs + pattern docs
- `schematics/` with dynamic Mermaid dependency graph
- `config/` with default, production, devenv-service TOML
- `.claude/` with context, patterns, anti-patterns JSON

## Implementation Order (after scaffold)

Build bottom-up. Quality gate after EVERY module. No exceptions.

```
L1 Foundation -> L3 Field -> L4 Coupling -> L5 Learning
-> L2 Services -> L6 Bridges -> L7 Coordination -> L8 Governance
```

For detailed implementation patterns, see `references/patterns.md`.
For the 3 practice runs that built this skill, see `references/practice-runs.md`.

## Gotchas

1. **Plan module layer refs must match layer keys** — validation catches this at load time
2. **Glob `m*` matches `mod.rs`** — use `m[0-9]*` to filter module files
3. **`cargo init` inside workspace** — add `[workspace]` to Cargo.toml to isolate
4. **Doc comments need backticks** — clippy `doc_markdown` requires backticked identifiers
5. **Async test_kind requires tokio** — falls back to unit test if tokio not in deps
6. **No `#![allow]` suppressions** — fix the actual code, never suppress pedantic
7. **Scaffold is spec** — doc comments ARE the implementation spec, not decoration
8. **Quality gate order matters** — check before clippy before pedantic before test
9. **Feature gates need features section** — auto-generated if layers have `feature_gate`
10. **50+ tests per module** — tests force design questions that improve architecture
