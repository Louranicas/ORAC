# Scaffold Practice Runs

3 iterations in the arena, each learning from the previous.

## Run 1: habitat-sentinel (Foundation)

**Method**: Manual bash loops with `declare -A` for module map
**Time**: ~60 seconds
**Issues found**:
- Glob `m*` matched `mod.rs` — fixed with `m[0-9]*`
- `cargo init` inside orchestrator workspace — needed `[workspace]` isolation
- Doc comments triggered 219 clippy `doc_markdown` errors

**Learnings**: Glob filtering matters. Workspace isolation is mandatory.

## Run 2: habitat-nexus (Optimized)

**Method**: Reusable `scaffold-gen.sh` bash script (154 lines)
**Time**: 4142ms (4.1 seconds)
**Issues found**:
- 95 clippy errors from doc comments (reduced from 219)
- Missing CLAUDE.md and MASTERPLAN.md

**Learnings**: Script reuse eliminates manual errors. But bash is fragile for string manipulation.

## Run 3: habitat-forge (God-tier)

**Method**: Same script + manual doc/config additions
**Time**: 1027ms
**Issues found**:
- `Figment`, `SO_REUSEADDR`, `SIGPIPE` needed backticks in bin/main.rs doc comments
- Clippy pedantic caught `format!` appended to `String` (use `writeln!`)
- `map_or` simplification
- Unneeded `return` statement

**Learnings**: Bin scaffolds need same doc rigor as lib. Pedantic catches real code quality issues.

## Meta-Learning: Scaffold Generator as Rust Binary

After 3 bash iterations, built `scaffold-gen` as compiled Rust (420KB):
- Type-safe module generation
- Correct doc comment formatting built-in
- `--verify` flag runs full quality gate
- Zero clippy warnings from generated code
- Generates 41 modules + all docs in <1 second

## Progression

| Run | Method | Time | Clippy Errors | Quality Gate |
|-----|--------|------|--------------|--------------|
| 1 | Manual bash | ~60s | 219 | 3/4 PASS |
| 2 | Bash script | 4.1s | 95 | 3/4 PASS |
| 3 | Bash + fixes | 1.0s | 1 | 3/4 PASS |
| Final | Rust binary | <1s | 0 | 4/4 PASS |

Arena location: `the-orchestrator/the developer environment arena/scaffold-practice/`
