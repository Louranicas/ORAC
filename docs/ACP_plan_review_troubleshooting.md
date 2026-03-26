# ACP Plan Review: Troubleshooting Section (SYM-001 through SYM-020)

> **Review date:** 2026-03-25
> **Plan reviewed:** `snazzy-shimmying-sedgewick.md` — Document 2: TROUBLESHOOTING.md
> **Cross-referenced against:** `bug-triage-session-060.md` (40 bugs + 5 synergies = 45 items), `CLAUDE.local.md` (Session 060 state)
> **Verdict:** Plan covers 8 of the top 20 most impactful failure modes. 12 critical/high items from the triage are completely missing. 6 of the 20 proposed SYMs are low-impact and should be replaced.

---

## 1. Summary of Findings

| Category | Count |
|----------|-------|
| Proposed SYMs that ARE top-20 impactful | 8 |
| Proposed SYMs that are moderate (keep, but not top-20) | 6 |
| Proposed SYMs that are low-impact (should be replaced) | 6 |
| CRITICAL/HIGH bugs from triage MISSING from plan | 17 |
| SYNERGY gaps from triage MISSING from plan | 5 |

**Root problem:** The plan is ORAC-centric. All 20 SYMs describe ORAC-internal failure modes. The bug triage reveals the most impactful failures are in *other services* (Prometheus Swarm DoS, DevOps Engine facade, VMS consolidation dead, SYNTHEX heat sources inert) and in *cross-service wiring gaps* (SYNERGY-01 through -05). A troubleshooting runbook that only covers ORAC misses the fleet-wide failure modes that an operator is most likely to encounter.

---

## 2. Per-SYM Assessment

### 2.1 Genuinely Top-20 Impactful (KEEP — 8 entries)

| SYM | Symptom | Maps To | Why Top-20 |
|-----|---------|---------|------------|
| SYM-002 | field_r = 0.0 (no spheres) | Operational diagnostic | First thing an operator checks; indicates broken PV2 link or no sessions |
| SYM-004 | All circuit breakers OPEN | Partially maps to CRIT-02 | Severed bridge communication; but needs the CRIT-02 specific cause added |
| SYM-005 | IPC socket dead | GAP-B (Session 058 fix) | PV2 unreachable kills real-time coordination |
| SYM-006 | Hebbian LTP=0 | GAP-A (Session 058 fix) | Learning completely dead; fleet cannot adapt |
| SYM-007 | POVM hydration 0 matched IDs | Session 060 finding | Persistence layer disconnected from runtime |
| SYM-008 | ME fitness frozen | MED-07 / SYNERGY-01 | Blocks metabolic health awareness |
| SYM-011 | Hook returns empty (ORAC down) | Operational critical | All fleet intelligence stops if ORAC is dead |
| SYM-012 | devenv won't start all 17 | BUG-001 / LOW-07 | Blocks everything; most common operational headache |

### 2.2 Moderate Impact (KEEP but not top-20 — 6 entries)

| SYM | Symptom | Why Not Top-20 |
|-----|---------|----------------|
| SYM-001 | Port 8133 occupied | Standard operational; covered by SYM-012 at fleet level |
| SYM-003 | RALPH fitness declining | Useful but RALPH is self-correcting via rollback mechanism |
| SYM-009 | SYNTHEX thermal NaN/INF | Already fixed (BUG-H001); guard deployed in Session 058 |
| SYM-010 | Blackboard DB locked | Theoretical; WAL mode + Mutex make this extremely rare |
| SYM-013 | Emergence detectors not firing | Useful diagnostic; thresholds were lowered in Session 060 |
| SYM-014 | Coupling weights all at floor | Useful; symptom of SYM-006 (LTP=0) — redundant |

### 2.3 Low Impact (REPLACE — 6 entries)

| SYM | Symptom | Why Replace |
|-----|---------|-------------|
| SYM-015 | Token budget exceeded | **Never observed.** Hard limit ($50) has never been approached. Theoretical edge case. |
| SYM-016 | WASM plugin not receiving events | **WASM bridge is not deployed.** m30_wasm_bridge is implemented but no WASM plugin exists to consume events. Dead code path. |
| SYM-017 | Binary stale after code change | **Basic operational hygiene, not a failure mode.** This is "remember to rebuild" — not a symptom requiring diagnosis. |
| SYM-018 | 404 on hook endpoints | **One-time misconfiguration.** PascalCase vs lowercase is a setup mistake caught once; not a recurring failure mode. |
| SYM-019 | Cascade rate limited | **By design.** Rate limiting at 10/min is intentional safety. Not a failure. |
| SYM-020 | Ghost traces not appearing | **Cosmetic.** Ghost traces are debugging aids with a 20-entry cap. Missing ghosts don't impair functionality. |

---

## 3. CRITICAL Bugs Missing from Plan

These are service-breaking, data-loss, or DoS-class failures documented in the triage that have NO corresponding SYM entry.

### 3.1 CRIT-01: Prometheus Swarm — POST /api/tasks crashes the service (DoS)

**Triage:** One `curl -X POST` to `:10001/api/tasks` triggers an `unwrap()` on `None` panic, killing the entire swarm coordinator (40 PBFT agents).
**Impact:** Trivially exploitable denial-of-service. Any service or operator accidentally posting a task kills the swarm.
**Why it must be in the runbook:** This is the single most dangerous failure mode in the fleet — a one-command kill of a 40-agent PBFT coordinator. An operator needs to know: (a) don't POST to Prometheus until the fix lands, (b) how to restart the swarm after a crash.

### 3.2 CRIT-02 root cause: SYNTHEX health path mismatch (`/health` vs `/api/health`)

**Triage:** ORAC polls SYNTHEX at `/health` (returns 404) but SYNTHEX's health endpoint is `/api/health` (returns 200). After 5 failures, the circuit breaker opens permanently.
**Plan gap:** SYM-004 covers "all breakers OPEN" generically but does NOT document this specific root cause. An operator following SYM-004 would check generic causes (service down, network issues) and miss the path mismatch entirely.
**Required addition:** A dedicated SYM with the specific diagnosis: `curl localhost:8090/health` returns 404, `curl localhost:8090/api/health` returns 200. Fix: change SYNTHEX bridge health path.

### 3.3 CRIT-03 + CRIT-04 + CRIT-05: DevOps Engine is a facade

**Triage:** DevOps Engine (batch 1, port 8081) accepts any input without validation (CRIT-03), fabricates status for nonexistent pipeline IDs (CRIT-04), and has permanently frozen health metrics with `uptime_seconds=0` (CRIT-05). It is not a functioning service — it is a facade.
**Impact:** Any monitoring or orchestration relying on DevOps Engine pipeline status gets false positives. ME health polling receives meaningless data.
**Why it must be in the runbook:** An operator investigating "why does the health mesh report DevOps as healthy?" needs to know the health endpoint is static.

### 3.4 CRIT-06: VMS consolidation never triggers (morphogenic_cycle=0)

**Triage:** VMS accepts memories (128+ stored) but has never processed any. `morphogenic_cycle: 0`, `closed_count: 0`, `fractal_depth_avg: 0.0`. The entire VMS pipeline is accept-only, never consolidate.
**Impact:** Blocks SYNERGY-05 (unified learning). Every memory written by ORAC, PV2, and fleet instances accumulates without processing.
**Why it must be in the runbook:** An operator checking "why is VMS not consolidating?" needs the diagnostic workflow: check `/health` metrics, check consolidation trigger logic, check for silent panics in the consolidation task.

### 3.5 CRIT-07: SYNTHEX heat sources permanently at 0.0

**Triage:** ORAC posts field state to `/api/ingest` every 6 ticks (confirmed by bridge metrics). SYNTHEX accepts the data but never updates its 4 heat sources (Hebbian 30%, Cascade 35%, Resonance 20%, CrossSync 15%). The "brain of the environment" has no environmental awareness.
**Impact:** SYNTHEX thermal model is inert. VMS tensor coupling, field-aware consolidation, and cross-service thermal routing are all blocked.
**Distinction from SYM-009:** SYM-009 covers NaN/INF (a parse error). CRIT-07 is the heat sources being structurally unwired — different root cause, different fix.

---

## 4. HIGH Bugs Missing from Plan

### 4.1 HIGH-01: Tool Library port swap (NAIS 8101 ↔ Bash Engine 8102)

Any cross-service routing through Tool Library for bash/NAIS targets hits the wrong service. ~5 LOC fix. NOT in plan.

### 4.2 HIGH-02: Bash Engine safety pattern bypasses (6+ classes)

Variable-indirection, base64 decode pipelines, python reverse shells, sensitive file reads all bypass safety checks. Security issue. NOT in plan.

### 4.3 HIGH-05: RM accepts negative/unbounded confidence values

Confidence field accepts -5.0 and 999.99. Non-numeric silently defaults to 0.5. Poisons 3,250 entries of cross-session decision data. NOT in plan.

### 4.4 HIGH-07: POVM /consolidate runs on ANY body including broken JSON

Invalid JSON triggers actual consolidation — decayed 201 memories in testing. Accidental POST destroys production data. NOT in plan.

### 4.5 HIGH-08: POVM accepts 500KB pathway IDs and unbounded weights

Storage amplification attack vector. Pathway weights accept 999999.999 (should be [0.0, 1.0]). Corrupt weights propagate on hydration. NOT in plan.

### 4.6 HIGH-10: ORAC dispatch_total=0 — fleet coordination completely inert

ORAC has never dispatched any work in 7,220+ ticks of uptime. The entire fleet coordination layer is a no-op. NOT in plan.

### 4.7 HIGH-11: Prometheus Swarm — 40 agents idle, 0 active tasks

40 PBFT agents consuming resources with zero output. Blocked by CRIT-01 (crash on POST) and HIGH-10 (ORAC never dispatches). NOT in plan.

---

## 5. SYNERGY Gaps Completely Missing from Plan

The plan's troubleshooting section has **zero entries** for synergy/metabolic failures — the highest-ROI items in the triage.

| Synergy | LOC | Impact | Plan Status |
|---------|-----|--------|-------------|
| SYNERGY-01: ME EventBus activation | ~50 | Unlocks health-aware field for ALL 17 services. Highest ROI item in entire triage. | **MISSING** |
| SYNERGY-02: PV2 sphere lifecycle broadcast | ~30 | Unlocks dynamic fleet topology awareness | **MISSING** |
| SYNERGY-03: NexusBus result publishing | ~40 | Unlocks cross-service circuit breaker visibility | **MISSING** |
| SYNERGY-04: RM + CCM integration | ~100 | Unlocks institutional memory | **MISSING** |
| SYNERGY-05: Unified learning signal | ~150 | Unlocks cross-domain transfer learning | **MISSING** |

These are not traditional "bugs" but they are the most frequently encountered operational questions: "why is the fleet not learning?", "why does ME report 0 correlations?", "why does VMS never consolidate?". A troubleshooting runbook that excludes them misses the most common diagnostic scenarios.

---

## 6. Recommended Revised SYM List (Top 20)

Replace the 6 low-impact SYMs with the 6 most critical missing items. Reorder by impact.

| # | Symptom | Severity | Source | Replaces |
|---|---------|----------|--------|----------|
| SYM-001 | **Prometheus Swarm crashes on POST /api/tasks** | CRITICAL | CRIT-01 | (was: port occupied) |
| SYM-002 | **ORAC↔SYNTHEX breaker permanently OPEN (health path mismatch)** | CRITICAL | CRIT-02 | (was: SYM-004 generic; this is the specific cause) |
| SYM-003 | **SYNTHEX heat sources all 0.0 despite ORAC posting data** | CRITICAL | CRIT-07 | (was: SYM-009 NaN which is already fixed) |
| SYM-004 | **VMS consolidation never triggers (morphogenic_cycle=0)** | CRITICAL | CRIT-06 | (was: cascade rate limited) |
| SYM-005 | **DevOps Engine health metrics frozen (uptime=0, facade service)** | CRITICAL | CRIT-03/04/05 | (was: ghost traces) |
| SYM-006 | **ORAC dispatch_total=0 — fleet coordination inert** | HIGH | HIGH-10 | (was: token budget) |
| SYM-007 | **Prometheus Swarm — 40 agents, 0 active tasks** | HIGH | HIGH-11 | (was: WASM bridge) |
| SYM-008 | **ME EventBus has 0 external subscribers (metabolic starvation)** | HIGH | SYNERGY-01 | (was: binary stale) |
| SYM-009 | **POVM /consolidate runs on any body (data destruction risk)** | HIGH | HIGH-07 | (was: 404 on hooks) |
| SYM-010 | field_r = 0.0 (no spheres registered) | HIGH | original SYM-002 | (keep) |
| SYM-011 | IPC socket dead (PV2 unreachable) | HIGH | original SYM-005 / GAP-B | (keep) |
| SYM-012 | Hebbian LTP=0 (no learning) | HIGH | original SYM-006 / GAP-A | (keep) |
| SYM-013 | POVM hydration returns 0 matched IDs | MEDIUM | original SYM-007 | (keep) |
| SYM-014 | ME fitness frozen at same value | MEDIUM | original SYM-008 / MED-07 | (keep) |
| SYM-015 | Hook returns empty (ORAC down) | MEDIUM | original SYM-011 | (keep) |
| SYM-016 | devenv won't start all 17 services | MEDIUM | original SYM-012 / BUG-001 | (keep) |
| SYM-017 | All circuit breakers OPEN (generic — check each service) | MEDIUM | original SYM-004 (generic) | (keep, demoted) |
| SYM-018 | RALPH fitness declining over 3+ generations | MEDIUM | original SYM-003 | (keep, demoted) |
| SYM-019 | Coupling weights all at floor (0.15) | MEDIUM | original SYM-014 | (keep) |
| SYM-020 | Emergence detectors not firing | LOW | original SYM-013 | (keep) |

---

## 7. Structural Recommendations for the Troubleshooting Document

### 7.1 Add a "Fleet-Wide Failures" section

The current plan implicitly scopes all SYMs to "ORAC won't do X." Add a top-level section for failures in other services that an ORAC operator would encounter:
- Prometheus Swarm crash (CRIT-01)
- DevOps Engine facade (CRIT-03/04/05)
- VMS stalled (CRIT-06)
- SYNTHEX heat sources dead (CRIT-07)
- Tool Library port swap (HIGH-01)

### 7.2 Add a "Metabolic Dormancy" section

The triage's most recurring theme is services that are structurally complete but metabolically inert. This manifests as:
- "All metrics are frozen" (DevOps, VMS, SYNTHEX uptime)
- "Counters are zero" (dispatch, correlations, morphogenic cycles)
- "Subscribers are zero" (ME EventBus, PV2 lifecycle broadcast)

These share a common diagnostic pattern: check if the data pipeline has producers, consumers, and active wiring between them.

### 7.3 Add a "Data Integrity" section

Multiple HIGH bugs involve accepting garbage input (RM confidence, POVM pathway IDs, VMS null content, POVM consolidation on broken JSON). Group these as "if you see unexpected values in downstream consumers, check input validation on the upstream writer."

### 7.4 Cross-reference the fix dependency chain

The triage documents a critical dependency chain: CRIT-01 → CRIT-02 → HIGH-10 → HIGH-11 (fleet coordination chain). The troubleshooting doc should note: "fixing SYM-007 (Prometheus idle) requires SYM-006 (ORAC dispatch) which requires SYM-002 (SYNTHEX breaker) which requires SYM-001 (Prometheus crash)."

---

## 8. What the Plan Gets Right

Despite the gaps, the plan's approach is sound:
- The SYM format (Symptom/Cause/Diagnosis/Fix/Prevention) is exactly right for a runbook
- ORAC-internal entries (field_r, LTP, POVM IDs, IPC, ME fitness) ARE real failure modes operators hit
- Source attribution to specific sessions and bug IDs enables traceability
- The plan correctly identifies bug-triage-session-060.md as a source — it just doesn't extract enough from it

The fix is **additive, not structural**: add the missing fleet-wide, metabolic, and data-integrity entries to the existing format.

---

## 9. Quick Reference: All 45 Triage Items vs Plan Coverage

| Triage ID | Severity | Covered by Plan SYM? | Notes |
|-----------|----------|----------------------|-------|
| CRIT-01 | CRITICAL | NO | Prometheus DoS crash |
| CRIT-02 | CRITICAL | Partial (SYM-004 generic) | SYNTHEX health path mismatch |
| CRIT-03 | CRITICAL | NO | DevOps zero validation |
| CRIT-04 | CRITICAL | NO | DevOps fabricated status |
| CRIT-05 | CRITICAL | NO | DevOps frozen metrics |
| CRIT-06 | CRITICAL | NO | VMS consolidation dead |
| CRIT-07 | CRITICAL | NO | SYNTHEX heat sources inert |
| HIGH-01 | HIGH | NO | Tool Library port swap |
| HIGH-02 | HIGH | NO | Bash Engine safety bypasses |
| HIGH-03 | HIGH | NO | NAIS catch-all 200 |
| HIGH-04 | HIGH | Partial (SYM-009 overlaps) | SYNTHEX uptime=0 |
| HIGH-05 | HIGH | NO | RM confidence unbounded |
| HIGH-06 | HIGH | NO | VMS null content accepted |
| HIGH-07 | HIGH | NO | POVM consolidate on garbage |
| HIGH-08 | HIGH | NO | POVM unbounded weights/IDs |
| HIGH-09 | HIGH | NO | POVM health no metrics |
| HIGH-10 | HIGH | NO | ORAC dispatch=0 |
| HIGH-11 | HIGH | NO | Prometheus Swarm idle |
| HIGH-12 | HIGH | NO | VMS dev_health inverted |
| HIGH-13 | HIGH | NO | Tool Library 0 pathways |
| MED-01 | MEDIUM | NO | Catch-all 200 on 3 services |
| MED-02 | MEDIUM | NO | PV2 evolution routes 404 |
| MED-03 | MEDIUM | NO | ORAC hooks accept any JSON |
| MED-04 | MEDIUM | NO | Sphere count naming collision |
| MED-05 | MEDIUM | NO | ORAC 932 ghost panes |
| MED-06 | MEDIUM | NO | CodeSynthor frozen synergy |
| MED-07 | MEDIUM | YES (SYM-008) | ME fitness frozen |
| MED-08 | MEDIUM | NO | VMS /mcp/tools/call 200 for unknown |
| LOW-01 | LOW | NO | VMS ignores request body |
| LOW-02 | LOW | NO | No payload size limits |
| LOW-03 | LOW | NO | ORAC-POVM bridge barely used |
| LOW-04 | LOW | NO | RALPH fitness drift |
| LOW-05 | LOW | NO | SYNTHEX+RM no uptime |
| LOW-06 | LOW | NO | Request counters = health polling only |
| LOW-07 | LOW | YES (SYM-012) | devenv stop doesn't kill |
| SYNERGY-01 | P0 | NO | ME EventBus activation |
| SYNERGY-02 | P0 | NO | PV2 sphere lifecycle broadcast |
| SYNERGY-03 | P1 | NO | NexusBus result publishing |
| SYNERGY-04 | P2 | NO | RM + CCM integration |
| SYNERGY-05 | P3 | NO | Unified learning signal |

**Coverage: 3 full matches + 2 partial = 5/45 items (11% coverage of triage)**

The remaining 15 proposed SYMs cover ORAC-internal scenarios not in the triage (because they were already fixed in Sessions 058-060 or are operational gotchas rather than bugs). These are valid runbook content — but they should not displace the unaddressed CRITICAL and HIGH items.
