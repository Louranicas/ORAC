# ACP Plan Risk Review — Single Biggest Risk

> **Plan:** `snazzy-shimmying-sedgewick.md` — 6 documents, ~2,100 lines, 3 parallel agents
> **Reviewed:** 2026-03-25
> **Verdict:** The plan will produce accurate docs on Day 1 that become actively harmful by Session 065.

---

## The Single Biggest Risk

**Staleness compounding with audience mismatch: these docs are consumed by AI agents who trust text at face value, but maintained by a process that doesn't exist.**

This is not a general "docs go stale" concern. It's specific to this system's dynamics:

---

### Why This System Is Uniquely Vulnerable to Doc Rot

**1. The codebase moves faster than any doc can track.**

CLAUDE.local.md already records the test count drifting across 6 sessions:

| Session | Tests | Delta |
|---------|------:|------:|
| 054 | 1,454 | — |
| 055 | 1,506 | +52 |
| 056a | 1,599 | +93 |
| 056b | 1,601 | +2 |
| 058 | 1,649 | +48 |
| 059 | 1,665 | +16 |
| 060 | 1,690 | +25 |

That's **236 new tests across 6 sessions** — roughly 40 per session. Every session also changes LOC counts, bug status, constant values, fitness scores, sphere counts, and connection counts. The plan's 6 documents embed dozens of these volatile numbers.

**2. The primary audience is AI agents, not humans.**

Luke already knows why he chose Kuramoto oscillators. He doesn't need ADR-001. The docs serve Claude instances in fleet panes — agents that are bootstrapped via CLAUDE.md, CLAUDE.local.md, and targeted reads. These agents **trust document content as ground truth**. They don't cross-check "ADR-004 says ureq" against the actual Cargo.toml. They don't verify "SCALING.md says STDP at 50 spheres takes ~5ms" against a benchmark.

**3. The doc maintenance mechanism is "monthly staleness canaries" — which don't exist.**

The plan's own MAINTENANCE_CHECKLIST.md (Document 6, line 251) says "Run staleness canaries (D1 section 9)" monthly. But no such canaries are defined. No script checks if LOC counts match. No test verifies constant values. No CI job flags drift. The maintenance plan for these docs is itself a doc with no enforcement.

**4. Context window cost is real and growing.**

The existing docs are already 31 files / 14,039 lines in `docs/`. CLAUDE.local.md alone is 43KB. MEMORY.md is 363 lines and already truncated. Adding 2,100 more lines of docs means ~60KB more context that agents must consume. Every stale token in an agent's context is a token that could have been source code.

---

### The Specific Failure Scenario

**Session 063 (Day 1):** Docs are accurate. An agent reads SCALING.md and trusts the projections.

**Session 065 (Day 3):** The bridge debt analysis (Session 061) leads to migrating 2 bridges from ureq to async hyper. ADR-004 now says "we chose ureq" but the code says hyper. ADR-011 says "raw TCP HTTP" but http_helpers.rs has been refactored. SCALING.md still says "blocking sync bridge calls are the known bottleneck" but that section of the tick loop is now async.

**Session 067 (Day 5):** A fleet agent is asked to add a new bridge. It reads ADR-004 + ADR-011 + SCALING.md, concludes "the pattern is blocking ureq calls," and writes a blocking bridge that re-introduces the exact tech debt that Sessions 061-065 resolved. When Luke asks "why did you use ureq?" the agent cites three authoritative-looking documents.

**This is worse than having no docs.** No docs means the agent reads source code and sees the current truth. Stale docs mean the agent reads a confident lie and doesn't check.

---

### Why the Other 3 Risks Are Lower

**Volume (will people read 2,100 more lines?):** Moderate risk but manageable. Agents do targeted reads, not cover-to-cover. The real cost is context tokens, not attention.

**Accuracy (are ADR rationales findable?):** Medium risk. Most rationales exist but some are retrospective rationalizations. ADR-004 and ADR-011 reference a bridge debt analysis that recommends *changing* the very decision being documented — so the ADR would record a decision already flagged as technical debt. Uncomfortable but not catastrophic.

**Audience (who reads this?):** Low risk as phrased. The audience mismatch is real (Luke doesn't need ADRs, agents do) but this is a feature of the plan, not a failure. The problem is that agents are the *wrong* audience for slowly-decaying human-maintained docs.

---

### The 4 Documents Ranked by Staleness Half-Life

| Document | Half-life | Why |
|----------|-----------|-----|
| GLOSSARY.md | ~20 sessions | Pure definitions. "Kuramoto" won't stop meaning Kuramoto. Slowest to decay. |
| TROUBLESHOOTING.md | ~5 sessions | Symptoms are stable but fixes evolve. SYM-006 "LTP=0 fix" may gain new causes. |
| ADR_INDEX.md | ~3 sessions | Decisions are permanent but 2 of 12 (ADR-004, ADR-011) are already flagged for reversal. |
| SCALING.md | **~1 session** | Contains **fabricated projections** ("~5ms at 50 spheres") with no benchmark backing. Any code change invalidates the numbers. |
| DEBUG_GUIDE.md | ~3 sessions | RUST_LOG paths are stable. Diagnostic commands may change. Log examples will drift. |
| MAINTENANCE_CHECKLIST.md | ~5 sessions | Process docs are slow to decay. But the irony: who maintains the maintenance checklist? |

**SCALING.md is the most dangerous document in the plan.** It presents fabricated numbers in a table that looks empirical. The plan itself notes "theoretical, untested" and "bench stubs are empty" — yet proceeds to project "~80ms" for 200 spheres. An agent reading that table will treat it as measured data.

---

## Proposed Mitigation: Machine-Verifiable Docs

**Don't write 6 human-narrative documents. Write 2 + embed the rest in code.**

### Keep (high value, slow decay)

1. **GLOSSARY.md** — Write it. Pure definitions have the longest half-life. A senior Rust dev reading the codebase needs this. ~350 lines, worth it.

2. **TROUBLESHOOTING.md** — Write it, but make every SYM entry include a **verification command** that can be copy-pasted to confirm the symptom still applies:
   ```
   ### SYM-006: Hebbian LTP=0
   - **Verify still relevant:** `grep -c 'working.len() < 2' src/m4_intelligence/m18_hebbian_stdp.rs`
   - (If returns 0, this SYM may be outdated — the guard was removed or refactored)
   ```
   This turns staleness detection from "someone remembers to check" into "the doc tells you how to check itself."

### Kill (high decay, low unique value)

3. **ADR_INDEX.md** — Don't write a separate file. Instead, embed 1-line ADR comments in source code:
   ```rust
   // ADR-004: ureq chosen over async hyper — simplicity over throughput.
   // Revisit: Session 061 bridge debt analysis recommends async migration.
   use ureq;
   ```
   ADRs in source travel with the code. When ureq is replaced, the comment is deleted in the same PR. Zero staleness lag.

4. **SCALING.md** — Don't write projections without benchmarks. Instead, create an actual benchmark:
   ```rust
   // benches/stdp_scaling.rs
   #[bench] fn stdp_50_spheres(b: &mut Bencher) { ... }
   #[bench] fn stdp_100_spheres(b: &mut Bencher) { ... }
   ```
   A benchmark that runs is worth more than a table that guesses. If benches can't be written now, write nothing — an empty cell is honest, a fabricated number is not.

5. **DEBUG_GUIDE.md** — Don't write a static doc. Add a `--diagnostic` mode to `orac-client`:
   ```
   orac-client diagnostic        # runs all health checks, prints RUST_LOG hints
   orac-client diagnostic --verbose  # includes module-level debug suggestions
   ```
   Executable diagnostics can't go stale because they're tested by the quality gate.

6. **MAINTENANCE_CHECKLIST.md** — Already lives in CLAUDE.local.md's "After Code Changes" and "Quality Gate" sections. A separate file adds duplication without adding value. If needed, add a 10-line section to CLAUDE.md instead of a 200-line standalone file.

### Net Result

| Plan as-is | Mitigation |
|------------|-----------|
| 6 files, ~2,100 lines | 2 files, ~750 lines + 12 source comments + 1 bench file |
| ~60KB context cost | ~25KB context cost |
| Staleness: undetectable | Staleness: self-verifying (TROUBLESHOOTING) or impossible (source-embedded ADRs) |
| SCALING projections: fabricated | SCALING projections: measured or absent |
| Maintenance burden: ~2,100 lines/month | Maintenance burden: ~750 lines/quarter |

---

## Summary

The plan is well-structured and addresses real gaps. But it optimizes for **coverage** (fill all 5 concern gaps) when it should optimize for **durability** (which docs will still be true in 5 sessions?). The single biggest risk is that 4 of the 6 documents will decay faster than they're maintained, and AI agents — the actual readers — will trust stale content over current source code.

Write the GLOSSARY and the TROUBLESHOOTING (with verification commands). Embed everything else in code where it can't drift.
