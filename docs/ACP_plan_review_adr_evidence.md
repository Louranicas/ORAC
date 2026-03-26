# ACP Plan Review: ADR Evidence Audit

> **Plan:** `~/.claude/plans/snazzy-shimmying-sedgewick.md` (Document 1: ADR_INDEX.md)
> **Audited:** 2026-03-25 | **Method:** 3 parallel subagents searching orac-sidecar codebase, shared-context vault, Obsidian archive vault
> **Scope:** 12 proposed ADRs — verify every claimed rationale source exists and contains WHY (not just WHAT)

---

## Verdict Summary

| ADR | Decision | Evidence | Verdict |
|-----|----------|----------|---------|
| ADR-001 | Kuramoto oscillators | arXiv 2508.12314, ORAC_PLAN.md 10-gap analysis, Obsidian reflections | **STRONG** |
| ADR-002 | Hebbian STDP | BUG-035 root cause chain, FIX-017 "entire reason ORAC exists" | **STRONG** |
| ADR-003 | 8-layer architecture | Session 061 nervous system analogy, Fleet Commander CLI comparison | **STRONG** |
| ADR-004 | Blocking sync bridges (raw TCP) | Session 061 bridge debt analysis, 5-challenge adversarial debate | **STRONG** |
| ADR-005 | SQLite blackboard | FIX-014 "survives restarts, no HTTP round-trips" — no alt comparison | **PARTIAL** |
| ADR-006 | Single BusFrame enum | ACP correction documented, PV2 Swarm v3 had separate frames | **PARTIAL** |
| ADR-007 | Feature gates | FIX-017 full taxonomy, "lobotomised binary" consequence | **STRONG** |
| ADR-008 | Consent gates (clinical ethics) | Session 034d NAM axioms, "social worker who put clinical ethics into Rust" | **STRONG** |
| ADR-009 | RALPH evolution | Session 050 spec, Session 018 decision review, fitness trajectory data | **STRONG** |
| ADR-010 | Unix domain socket IPC | Session 019b founding document, design-vs-pragmatic-fallback documented | **PARTIAL** |
| ADR-011 | Raw TCP HTTP for bridges | Session 061 "minimal overhead, ~100KB binary savings, zero-dependency" | **STRONG** |
| ADR-012 | TSV for Reasoning Memory | m25 header "NEVER JSON — AP05", but no WHY TSV-over-JSON rationale | **PARTIAL** |

**Totals: 8 STRONG, 4 PARTIAL, 0 UNSUPPORTED**

---

## Per-ADR Evidence Detail

### ADR-001: Kuramoto Oscillators for Fleet Coordination

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `ORAC_PLAN.md` | YES | YES — "Validated: arxiv 2508.12314" + 10-gap justification vs hooks |
| `ai_specs/patterns/KURAMOTO.md` | YES | PARTIAL — equations/chimera but not WHY over alternatives |
| `[[Vortex Sphere Brain-Body Architecture]]` (Obsidian) | YES | YES — philosophical rationale, K-value regime table |
| Session 061 Reflections (shared-context) | YES | YES — "built Kuramoto before arxiv validated the approach" |
| Session 018 Reflections (Obsidian) | YES | YES — "operate *near* critical coupling, not above it" |

96+ files reference Kuramoto. **STRONG** — arXiv, 10 capability gaps, philosophical + empirical validation.

### ADR-002: Hebbian STDP for Coupling Learning

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `ai_specs/patterns/STDP.md` | YES | PARTIAL — full spec, not WHY over alternatives |
| `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` (Obsidian) | YES | YES — 7-step BUG-035 root cause, diversity enforcement |
| FIX-017 analysis (shared-context) | YES | YES — "the entire reason ORAC exists beyond a dumb proxy" |

190+ files reference STDP. **STRONG** — BUG-035 lesson + identity-level justification.

### ADR-003: 8-Layer Architecture

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `ORAC_PLAN.md` | YES | PARTIAL — describes phases not WHY 8 layers |
| Session 061 Reflections | YES | YES — "biological nervous systems: sensation bottom, reflexes middle, learning top" |
| Fleet Commander doc | YES | YES — "flat structure is simpler and equally correct [for CLI tools]" |

**STRONG** — nervous system analogy + explicit alternative comparison.

### ADR-004: Blocking Sync Bridge Calls via Raw TCP

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `Session 061 — Fire-and-Forget Bridge Debt Analysis.md` | YES | YES — 21 call sites, 14 failure modes, adversarial debate |
| `http_helpers.rs` header | YES | PARTIAL — "minimal overhead — no HTTP library dependency" |
| Session 061 Challenge 5 | YES | YES — "ureq: ~100KB binary. Raw TCP: zero-dependency" |

**STRONG**. Inconsistency: `ai_specs/BRIDGE_SPEC.md` claims `reqwest::Client` but code uses raw `TcpStream`.

### ADR-005: SQLite Blackboard

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `docs/DATABASE_SPEC.md` | **NO** | — |
| `m26_blackboard.rs` header | YES | NO — WHAT not WHY |
| FIX-014 (shared-context) | YES | PARTIAL — "survives PV2 restarts, fast cross-pane reads without HTTP round-trips" |

**PARTIAL** — `DATABASE_SPEC.md` missing. No Postgres/Redis comparison documented.

### ADR-006: Single BusFrame Enum

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `m08_bus_types.rs` header | YES | NO — "11 kinds" but not WHY single enum |
| "Critic-2 finding" | **NO** | **Fabricated — not found in any vault** |
| Swarm v3 IPC planning | YES | PARTIAL — older design had separate frames |

**PARTIAL** — decision confirmed, WHY must be inferred from code. "Critic-2" reference is fabricated.

### ADR-007: Feature Gates for Optional Layers

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `Cargo.toml` features | YES | PARTIAL — structure shown |
| `fix-017-default-features.md` | YES | YES — full taxonomy of gate purposes |
| Session 055 Obsidian | YES | YES — "lobotomised binary" consequence |

**STRONG** — complete decision analysis + consequence documentation.

### ADR-008: Consent Gates (Clinical Ethics)

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `docs/CONSENT_SPEC.md` | **NO** | — |
| `[[Session 034d — NA Consent Gate Implementation]]` (Obsidian) | YES | YES — 5 NA gaps, NAM axiom refs |
| `ORAC_PLAN.md` Consent Philosophy | YES | YES — "The field modulates. It does not command." |
| EXECUTIVE_SUMMARY.md | YES | YES — "social worker who put clinical ethics into Rust" |

**STRONG** — `CONSENT_SPEC.md` missing but consent WHY is among the richest in the entire project.

### ADR-009: RALPH Evolution

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `m36_ralph_engine.rs` header | YES | PARTIAL — mechanism not WHY |
| `[[Session 050 — ME Evolution Chamber Spec]]` (Obsidian) | YES | YES — RALPH loop, Raft vs PBFT |
| `decisions/session-018-deep-review-ralph-loop.md` | YES | YES — root cause hierarchy |
| DIM-1 analysis | YES | YES — fitness 0.43→0.82 trajectory |

174+ files reference RALPH. **STRONG**.

### ADR-010: Unix Domain Socket IPC

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `docs/IPC_BUS_SPEC.md` | **NO** | — |
| `m07_ipc_client.rs` header | YES | NO — mechanism only |
| `Pane-Vortex IPC Bus — Session 019b.md` (Obsidian) | YES | YES — founding WHY (gaps in HTTP) |
| `ORAC_PLAN.md` gap #3 | YES | PARTIAL — "single connection vs new HTTP per call" |

**PARTIAL** — `IPC_BUS_SPEC.md` missing. No TCP/shared-memory/MQ comparison.

### ADR-011: Raw TCP HTTP for Bridges

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `http_helpers.rs` header | YES | PARTIAL — "minimal overhead" |
| `docs/BRIDGE_PATTERNS.md` | **NO** | — |
| Session 061 bridge debt | YES | YES — "ureq: ~100KB. Raw TCP: zero-dependency." |

**STRONG** — `BRIDGE_PATTERNS.md` missing but Session 061 covers it fully.

### ADR-012: TSV for Reasoning Memory

| Source | Exists? | Contains WHY? |
|--------|---------|---------------|
| `m25_rm_bridge.rs` header | YES | YES — "NEVER JSON — AP05: parse failures" |
| CLAUDE.md anti-pattern table | YES | YES — repeated 3 times |
| fleet-rm-nais findings | YES | PARTIAL — "API is JSON, Persistence is TSV" |

**PARTIAL** — WHAT is clear. WHY TSV at RM's design time is not recorded. ADR should frame as "conform to upstream protocol."

---

## Missing Source Files (4 claimed docs do not exist)

| Claimed File | ADR | Workaround |
|-------------|-----|------------|
| `docs/DATABASE_SPEC.md` | ADR-005 | FIX-014 + Session 057 provide partial rationale |
| `docs/CONSENT_SPEC.md` | ADR-008 | ORAC_PLAN.md + Executive Summary + Session 034d fully cover it |
| `docs/IPC_BUS_SPEC.md` | ADR-010 | WIRE_PROTOCOL_SPEC.md + Session 019b provide partial rationale |
| `docs/BRIDGE_PATTERNS.md` | ADR-011 | Session 061 bridge debt analysis fully covers it |

## Fabricated References (1)

| Claimed Reference | ADR | Finding |
|------------------|-----|---------|
| "Critic-2 finding from ACP Round 2" | ADR-006 | **Not found in any file across all 3 vaults** |

---

## Recommendations

### Can consolidate from existing docs (8 ADRs)

ADR-001, 002, 003, 004, 007, 008, 009, 011 — strong WHY exists scattered across session notes and analysis docs. Pure consolidation.

### Must write new rationale (4 ADRs)

| ADR | What's Missing | Approach |
|-----|---------------|----------|
| ADR-005 (SQLite) | No alternative comparison | Infer: zero-config, embedded, !Send constraint, survives restarts, ~0ms latency |
| ADR-006 (BusFrame) | No WHY for single enum | Infer: bidirectional Cascade/CascadeAck, serde internally-tagged, PV2 v3 had split and simplified |
| ADR-010 (Unix IPC) | No alternative comparison | ORAC_PLAN.md gap #3 + Session 019b + 0700 permissions |
| ADR-012 (TSV) | WHY TSV specifically | Reframe: conform to upstream RM wire format, not a design choice |
