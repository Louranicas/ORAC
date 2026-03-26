# ACP Plan Review: Minimalist Analysis

> **Plan:** `~/.claude/plans/snazzy-shimmying-sedgewick.md`
> **Existing corpus:** 33 files, 14,163 lines in `docs/` + 923 lines in CLAUDE.md/CLAUDE.local.md = **15,086 lines already**
> **4 prior ACP reviews also exist:** adr_evidence (189L), troubleshooting (265L), glossary_sizing (264L), risk (145L)
> **Reviewed:** 2026-03-25

---

## The Plan's Proposal: 6 docs, ~2,100 lines

The plan identifies 5 audit gaps (scalability 55%, maintenance 65%, deep understanding 70%, debugging 65%) and proposes filling them with 6 new documents via 3 parallel agents in ~15 minutes:

| # | Document | Lines | Gap Targeted |
|---|----------|-------|-------------|
| 1 | ADR_INDEX.md | ~500 | Why decisions were made |
| 2 | TROUBLESHOOTING.md | ~400 | Symptom-to-fix runbook |
| 3 | GLOSSARY.md | ~350 | Domain terminology |
| 4 | SCALING.md | ~350 | Performance limits and projections |
| 5 | DEBUG_GUIDE.md | ~300 | Logging, diagnostics, investigation |
| 6 | MAINTENANCE_CHECKLIST.md | ~200 | Routine upkeep tasks |

The plan correctly frames itself as "consolidation over creation." The question is whether it actually consolidates, or whether it creates 6 new files that restate what 33 existing files already say.

---

## Redundancy Analysis

### Document 1: ADR_INDEX.md (~500 lines) -- Redundancy: ~70%

**What already exists:**
- D1 section 1 (511L total): documents WHY ORAC exists -- 10 gaps that justify the entire architecture. This is the rationale for ADR-001, ADR-003, ADR-008.
- D2 (463L): layer DAG with feature gates -- partial coverage of ADR-003 and ADR-007.
- D4 (655L): bridge topology and polling model -- the WHAT behind ADR-004, ADR-005, ADR-010, ADR-011.
- CLAUDE.md "Traps to Avoid" (12 items): operational consequences of architectural decisions -- ADR consequences in compressed form.
- EXECUTIVE_SUMMARY "Why does it exist?" section: plain-English rationale for the system as a whole.

**What is genuinely new:** The "alternatives considered" and "why not X" framing. No existing document records rejected alternatives.

**Critical problem:** The existing ACP_plan_review_adr_evidence.md already audited this: only 1 of 12 ADRs has STRONG evidence, 4 have MODERATE, and 7 have WEAK. 4 of 24 claimed source files (DATABASE_SPEC.md, Critic-2 finding, IPC_BUS_SPEC.md, BRIDGE_PATTERNS.md) do not exist. Writing 7 WEAK ADRs means fabricating "alternatives considered" sections for decisions that were never explicitly debated. That is historical fiction dressed as architecture documentation.

**Net-new lines after dedup:** ~150 (the genuine "alternatives considered" for 4-5 decisions with evidence).

---

### Document 2: TROUBLESHOOTING.md (~400 lines) -- Redundancy: ~40%

**What already exists:**
- CLAUDE.md "Traps to Avoid": 12 items covering pkill chain, cp alias, TSV format, lock ordering, phase wrapping, SIGPIPE, Zellij scripting, fleet-ctl cache, BUG-035, bridge URLs, ProposalManager default, POVM write-only.
- CLAUDE.local.md session notes: GAP-A fix (LTP=0), GAP-B fix (IPC socket), CRIT-01 (Prometheus), POVM ID mismatch -- scattered across sessions 055-060.
- R1_fleet_error_catalog.md (331L): 24 PvError variants with codes, severity, retryability.

**What is genuinely new:** The structured symptom-diagnosis-fix format with copy-paste diagnostic commands. Existing content tells you WHAT went wrong in past sessions; nothing tells you HOW TO INVESTIGATE a new occurrence.

**Critical problem:** The existing ACP_plan_review_troubleshooting.md already found: plan covers only 8 of the top 20 impactful failures. 17 CRITICAL/HIGH bugs from the triage are missing entirely. The plan is ORAC-centric; the most impactful failures are cross-service (Prometheus DoS, DevOps Engine facade, VMS consolidation dead, SYNTHEX heat sources inert). 6 of 20 proposed SYMs are low-impact.

**Net-new lines after dedup:** ~250 (diagnostic workflows and structured format).

---

### Document 3: GLOSSARY.md (~350 lines) -- Redundancy: ~15%

**What already exists:**
- EXECUTIVE_SUMMARY (567L): 40 modules with "Think of it as:" plain-English analogies. Covers architecture vocabulary.
- D6 (375L): 54 constants with purpose descriptions -- a glossary of tuning knobs.
- R1_fleet_constants.md (616L): overlaps heavily with D6.

**What is genuinely new:** Consolidated alphabetical definitions for Kuramoto theory (16 terms), neuroscience/STDP (14 terms), evolutionary algorithm terms, and fleet-specific concepts. No single document currently defines "chimera," "LTP," "homeostatic normalization," or "buoy" for a reader without physics/neuroscience background. The ACP_plan_review_glossary_sizing.md confirmed 93 domain-specific terms in EXECUTIVE_SUMMARY alone.

**This is the least redundant proposed document.** The EXECUTIVE_SUMMARY covers architecture vocabulary but not the physics and neuroscience vocabulary that permeates every module.

**Net-new lines after dedup:** ~300.

---

### Document 4: SCALING.md (~350 lines) -- Redundancy: ~75%

**What already exists:**
- D6 (375L): ALL 54 hard limits with source file:line. Every cap value SCALING.md would cite is already here.
- D4 bridge topology: documents the sequential blocking bridge concern.
- R1_fleet_concurrency.md (411L): concurrency patterns, lock ordering, thread model.
- R1_fleet_constants.md (616L): all coupling parameters and bounds.

**What is genuinely new:** O(N) complexity analysis (~40 lines), blocking bridge bottleneck analysis (~40 lines).

**Critical problem:** ACP_plan_review_risk.md correctly identifies this as "the most dangerous document in the plan." The plan admits "theoretical, untested" and "bench stubs are empty" then proposes a table projecting "~80ms" at 200 spheres. AI agents consuming this document will treat fabricated numbers as measured data. The plan's own Section 5 is titled "What's NOT Tested" -- an honest section that undermines the credibility of Section 4. Half-life: ~1 session.

**Net-new lines after dedup:** ~80 (genuine complexity analysis only; the rest restates D6 or fabricates projections).

---

### Document 5: DEBUG_GUIDE.md (~300 lines) -- Redundancy: ~60%

**What already exists:**
- CLAUDE.md: traps to avoid (12 items), hook endpoints and timeouts, build commands, diagnostic tool names.
- D5 (570L): full execution flow maps for all 6 hooks and the RALPH cycle -- exactly what you trace when debugging.
- D3 (789L): every endpoint with request/response schema -- what to curl and what to expect.
- R1_fleet_error_catalog.md (331L): error taxonomy with codes and severity.

**What is genuinely new:** RUST_LOG per-module configurations (~30 lines) and structured debug decision trees (~60 lines). No document currently maps "I see symptom X" to "set RUST_LOG to Y and check module Z."

**Net-new lines after dedup:** ~120.

---

### Document 6: MAINTENANCE_CHECKLIST.md (~200 lines) -- Redundancy: ~80%

**What already exists:**
- CLAUDE.md "Build & Quality Gate": the quality gate protocol (check -> clippy -> pedantic -> test).
- CLAUDE.local.md resume protocols: rebuild + deploy sequence, hook verification, service restart.
- Workspace CLAUDE.md: quality gate, health check script, devenv management.

**What is genuinely new:** Monthly LOC drift check, POVM pathway growth monitoring (~40 lines).

**Critical problem:** The plan references "staleness canaries (D1 section 9)" -- which do not exist. A maintenance checklist that points to nonexistent infrastructure is worse than no checklist. The "daily" tasks duplicate CLAUDE.md; the "after code changes" tasks duplicate CLAUDE.local.md; the "git hygiene" section is not ORAC-specific. The document is 80% duplication and 20% aspiration.

**Net-new lines after dedup:** ~40.

---

## Redundancy Summary Table

| Document | Proposed | Net-New | Redundancy | Half-Life |
|----------|----------|---------|------------|-----------|
| ADR_INDEX.md | 500 | ~150 | 70% | ~3 sessions |
| TROUBLESHOOTING.md | 400 | ~250 | 40% | ~5 sessions |
| GLOSSARY.md | 350 | ~300 | 15% | ~20 sessions |
| SCALING.md | 350 | ~80 | 75% | ~1 session |
| DEBUG_GUIDE.md | 300 | ~120 | 60% | ~3 sessions |
| MAINTENANCE_CHECKLIST.md | 200 | ~40 | 80% | ~5 sessions |
| **TOTAL** | **2,100** | **~940** | **55%** | -- |

The plan proposes 2,100 lines. Approximately 1,160 lines already exist somewhere in the current 15,086-line corpus. The genuinely new content is ~940 lines.

---

## Merge Opportunities

### TROUBLESHOOTING + DEBUG_GUIDE + MAINTENANCE_CHECKLIST --> OPERATIONS.md

These three documents share a single audience (the person operating ORAC) and a single question ("something is wrong or needs attention -- what do I do?"). The debug guide's RUST_LOG config is preamble to the troubleshooting SYM entries. The debug decision trees lead to the same fixes documented in SYM entries. The maintenance checklist is proactive troubleshooting. Splitting them into 3 files means an operator checks 3 places when something breaks, and 3 files must be kept consistent with each other and with CLAUDE.md.

**Combined: ~300 lines vs 900 lines across 3 files.**

### SCALING --> Absorb into D6

D6 already documents every constant and limit. Sections 1 (Hard Limits), 5 ("What's NOT Tested"), and 6 (Recommendations) of the proposed SCALING.md restate D6 content. The genuinely new content -- complexity analysis and bottleneck identification -- belongs as new sections in D6, which is literally named "Capacity and Limits Reference." Adding ~80 lines to a 375-line document is natural growth. Creating a parallel 350-line file about the same topic is duplication.

### ADR_INDEX --> Keep standalone but radically smaller

ADRs do not merge naturally with any other document type. But writing 12 ADRs when only 4-5 have evidence is padding. Write the substantiated ones; stub the rest.

### GLOSSARY --> Keep standalone

No natural merge partner. Lowest redundancy. Longest half-life.

---

## Minimum Viable Documentation (3 docs, ~1,200 lines)

### Doc 1: GLOSSARY.md (~350 lines)

- **Content:** ~80 terms from the glossary sizing review's 93 identified terms, trimmed to exclude standard Rust/web/DevOps vocabulary
- **Categories to include:** Kuramoto theory (16 terms), neuroscience/STDP (14 terms), evolutionary algorithms (11 terms), fleet concepts (sphere, buoy, ghost trace, cascade -- ~10 terms), ORAC-specific architecture (~15 terms not explained inline in EXECUTIVE_SUMMARY)
- **Categories to exclude:** Standard acronyms (IPC, FSM, WAL), service names (defined in D1), fitness dimension names (defined in D6/m39 source)
- **Format:** Alphabetical. 1-2 sentence definition + "In ORAC:" context sentence + "See also:" cross-reference
- **Why keep at ~350 not ~150:** The glossary sizing review found 93 genuinely opaque terms. Cutting to 30 "hardest" terms assumes the reviewer can predict which terms trip up a newcomer. Physics terms seem obvious to cut -- but "chimera state" is not obvious to a neuroscience-trained reader either. Keep the full physics + neuroscience + evolutionary set; cut only the terms already defined in EXECUTIVE_SUMMARY with "Think of it as:" explanations.
- **Estimated net-new:** ~300 lines. Longest half-life of any proposed document (~20 sessions).

### Doc 2: OPERATIONS.md (~550 lines)

- **Merged from:** TROUBLESHOOTING (best 14 SYMs) + DEBUG_GUIDE (RUST_LOG + decision trees) + MAINTENANCE_CHECKLIST (monthly tasks only)
- **Structure:**
  1. **Quick Diagnostic Commands** (~30 lines) -- the 5 commands to run first when something is wrong
  2. **RUST_LOG Configuration** (~40 lines) -- per-layer and per-module paths, common combinations
  3. **Debug Decision Trees** (~80 lines) -- 5 workflows: hook not firing, bridge timeout, RALPH stuck, coupling dead, fleet not coordinating
  4. **Symptom-Fix Runbook** (~350 lines, 14 entries) -- 8 top-20 ORAC entries (from troubleshooting review) + 4 fleet-wide entries from bug triage (Prometheus DoS, SYNTHEX inert, DevOps facade, VMS consolidation) + 2 cross-service wiring entries. Each entry uses the self-verifying format from ACP_plan_review_risk.md (includes a grep/curl command to confirm the SYM still applies).
  5. **Maintenance Cadence** (~50 lines) -- monthly only (LOC drift, test count, blackboard prune, POVM pathway growth, cargo outdated). Daily and after-code-change tasks are NOT included because they already live in CLAUDE.md.
- **Sections to exclude:** Log output examples (decay in ~1 session), profiling instructions (no benchmarks exist to profile against), health check response field interpretation (already in D3), diagnostic command details beyond the top 5 (already in CLAUDE.md)
- **Estimated net-new:** ~350 lines.

### Doc 3: ADR_INDEX.md (~300 lines)

- **Content:** Write 5 full ADRs that have MODERATE-to-STRONG evidence:
  - ADR-004 (blocking sync bridges via ureq) -- STRONG, Session 061 bridge debt analysis provides alternatives
  - ADR-007 (feature gates) -- MODERATE, FIX-017 analysis provides rationale
  - ADR-008 (consent gates) -- MODERATE, CONSENT_SPEC.md exists
  - ADR-011 (raw TCP HTTP bridges) -- MODERATE, http_helpers.rs header discusses tradeoffs
  - ADR-012 (TSV for RM) -- STRONG, 10+ corroborating sources, JSON rejection well-documented
- **Stub the remaining 7:** One-line summary + "Evidence status: WEAK -- rationale not documented at decision time" + link to best available source. This is honest: it acknowledges gaps rather than filling them with invention.
- **Format per full ADR:** Context (why the problem existed), Decision (what was chosen), Alternatives Considered (what was rejected and why), Consequences (tradeoffs accepted), References (source files, session notes). ~50 lines each.
- **Estimated net-new:** ~250 lines.

### Addendum: D6 sections 13-14 (~80 lines, not a new file)

- **Section 13: Theoretical Complexity** (~50 lines) -- O(N*S) Kuramoto field, O(C) STDP pass, O(H) emergence scan, O(N) semantic routing, O(1) blackboard queries
- **Section 14: Known Bottleneck -- Blocking Bridge Calls** (~30 lines) -- sequential polling model, worst-case stall calculation, circuit breaker mitigation, Session 061 bridge debt analysis reference
- No projections table. No fabricated numbers. If someone wants scaling numbers, they write benchmarks.

### Summary

| Deliverable | Lines | Files | Replaces |
|-------------|-------|-------|----------|
| GLOSSARY.md | ~350 | 1 new | Plan Doc 3 (trimmed) |
| OPERATIONS.md | ~550 | 1 new | Plan Docs 2+5+6 (merged) |
| ADR_INDEX.md | ~300 | 1 new | Plan Doc 1 (5 full + 7 stubs vs 12 full) |
| D6 addendum | ~80 | 0 new (added to existing) | Plan Doc 4 (absorbed) |
| **Total** | **~1,280** | **3 new files** | **6 proposed files at ~2,100 lines** |

Reduction: 39% fewer lines, 50% fewer files, substantially less duplication.

---

## What to CUT Entirely

### Fabricated scaling projections

The plan's SCALING.md Section 4 presents a table: "10 spheres: <1ms, 50 spheres: ~5ms, 100 spheres: ~20ms, 200 spheres: ~80ms." These numbers are invented. The plan admits "theoretical, untested" and "bench stubs are empty." An AI agent reading this table cannot distinguish it from measured data. Writing imaginary numbers in a table that looks empirical is not documentation -- it is misinformation with plausible formatting.

### ADR rationale that does not exist

7 of 12 ADRs have WEAK evidence. ADR-003 ("why 8 layers?") has no documented alternatives analysis because the layer count was inherited from PV2. ADR-005 ("why SQLite?") has no documented alternatives analysis because DATABASE_SPEC.md does not exist. Writing "we considered Postgres and rejected it because..." when no one documented considering Postgres is fiction. Listing these as stubs is honest; writing them as full ADRs is not.

### Log output examples

The proposed DEBUG_GUIDE Section 5 ("Log Output Examples") would show "Successful startup log," "Normal tick cycle log," "Bridge failure log." These examples become wrong every time a tracing call is added, removed, or reformatted. Every refactor touches logging. Half-life: ~1 session. These examples would be the first section to go stale and the last anyone would think to update.

### Daily/after-code-change maintenance checklist

These sections duplicate CLAUDE.md's quality gate protocol and CLAUDE.local.md's resume protocols. A third copy in a third file creates a third place that must be updated when the quality gate changes. The workspace CLAUDE.md already says: "Order: check -> clippy -> pedantic -> test. Zero tolerance at every stage." The proposed MAINTENANCE_CHECKLIST says the same thing in 40 lines instead of 2.

### 6 low-impact SYM entries

SYM-015 (token budget): never observed. SYM-016 (WASM ring): no WASM plugin exists -- this is a dead code path. SYM-017 (stale binary): "remember to rebuild" is not a failure mode. SYM-018 (404 on hooks): one-time setup mistake. SYM-019 (cascade rate limited): by design, not a failure. SYM-020 (ghost traces): cosmetic debugging aid. Replace with 4 CRITICAL/HIGH items from the bug triage that the plan completely missed.

---

## Risk: Volume Compounds Confusion

The existing documentation corpus:

| Category | Files | Lines |
|----------|-------|-------|
| D1-D8 System Atlas | 8 | 5,172 |
| R1 fleet research | 9 | 3,631 |
| V_ verification reports | 8 | 3,321 |
| ACP reviews (including this one) | 5 | 1,008 |
| EXECUTIVE_SUMMARY | 1 | 567 |
| VERIFICATION_REPORT | 1 | 217 |
| Project CLAUDE files | 2 | 923 |
| **Total** | **34** | **14,839** |

The plan proposes adding 6 files and ~2,100 lines, reaching ~40 files and ~16,939 lines. This is a 14% volume increase.

The real cost is not volume -- it is **sync surface area**. Every file that embeds a constant value, test count, module name, port number, or fitness score is a staleness liability. The existing corpus already demonstrates this problem:

- D1 says "55 source files, ~41,369 LOC, ~1,748 tests"
- CLAUDE.md says "30,524 LOC, 1,601 tests"
- CLAUDE.local.md says "32000 LOC, 1690 tests"

Three files, three different numbers, all claiming to be current. None is marked as stale. An AI agent reading any of the three will trust it.

Adding 6 more files does not help a new developer. It creates 6 more places where the answer MIGHT be, 6 more chances for the answer to be WRONG, and 6 more files that nobody will update when Session 064 adds 40 tests and changes 3 constants.

The ACP_plan_review_risk.md states it precisely: "Stale docs mean the agent reads a confident lie and doesn't check. This is worse than having no docs." The plan's own MAINTENANCE_CHECKLIST proposes "staleness canaries (D1 section 9)" that do not exist, referencing infrastructure that was never built. The document about maintaining documents is itself unmaintainable. The meta-irony is instructive.

Does more documentation help a new developer? Only when:

1. **It is the ONLY place to find the information.** GLOSSARY passes. TROUBLESHOOTING partially passes. SCALING fails (D6 exists). MAINTENANCE_CHECKLIST fails (CLAUDE.md exists). ADR_INDEX partially passes (alternatives not documented elsewhere). DEBUG_GUIDE partially passes (RUST_LOG paths are novel).

2. **It includes a mechanism to detect its own decay.** Only TROUBLESHOOTING with self-verifying grep/curl commands (per the risk review) passes this test. Everything else relies on "someone remembers to check" -- the same mechanism that has already produced 3 conflicting LOC counts across 3 files.

3. **Its half-life exceeds the update cadence.** GLOSSARY passes (~20 sessions). TROUBLESHOOTING passes (~5 sessions). SCALING fails catastrophically (~1 session). DEBUG_GUIDE is marginal (~3 sessions). MAINTENANCE_CHECKLIST passes if it existed in isolation, but it does not -- it duplicates faster-changing sources.

4. **It does not duplicate content from another file.** GLOSSARY passes (85% novel). TROUBLESHOOTING partially passes (60% novel). SCALING fails (75% duplicates D6). MAINTENANCE_CHECKLIST fails (80% duplicates CLAUDE.md). DEBUG_GUIDE partially passes (60% novel). ADR_INDEX partially passes (70% duplicates D1/D2/D4 but in a new frame).

Only GLOSSARY passes all 4 tests. TROUBLESHOOTING passes 3 of 4. Everything else fails at least 2.

---

## Recommendation

Write 3 documents totaling ~1,200 lines, not 6 documents totaling ~2,100 lines. Add ~80 lines to the existing D6 instead of creating a parallel SCALING.md.

**GLOSSARY.md** (~350 lines) fills the only gap that no existing document addresses -- domain vocabulary for non-specialists -- and has the longest half-life of any proposed document. Write it as proposed, trimming only the standard-vocabulary padding.

**OPERATIONS.md** (~550 lines) merges TROUBLESHOOTING, DEBUG_GUIDE, and MAINTENANCE_CHECKLIST into a single operator reference. Every SYM entry includes a self-verifying command so the document signals its own staleness. Includes the 4 fleet-wide critical failures the original plan missed entirely.

**ADR_INDEX.md** (~300 lines) documents 5 decisions that have real evidence and stubs the 7 that do not. This is honest documentation: it captures what was actually decided and acknowledges what was inherited or implicit. Writing 12 full ADRs when 7 lack evidence would produce plausible-looking fiction that agents treat as ground truth.

**D6 addendum** (~80 lines) adds complexity analysis to the document that already owns capacity and limits data. No new file. No fabricated projections.

This delivers ~85% of the plan's gap-closing value at 60% of the volume, 50% of the files, and a fraction of the sync surface area. The existing 34-file, 14,839-line corpus does not need 6 more companions. It needs 3 surgical additions that fill genuine gaps without compounding the problem that already gives us 3 different answers for how many lines of code ORAC has.
