# R1 Fleet Documentation Accuracy Audit

> **Audited:** 2026-03-25
> **Scope:** `layer_architecture.mmd`, `API_SPEC.md`, `ORAC_PLAN.md`, `CLAUDE.md`, `CLAUDE.local.md`
> **Method:** Compared documentation claims against actual `src/` filenames, `build_router()` in `m10_hook_server.rs`, and `wc -l` counts

---

## 1. Schematic Inaccuracies (layer_architecture.mmd vs src/)

The Mermaid schematic at `ai_docs/schematics/layer_architecture.mmd` uses short labels for each module. Many are stale or were never updated after implementation diverged from the original plan.

### L1 Core — 3 wrong names + 1 missing file

| Schematic Label | Actual Filename | Status |
|-----------------|-----------------|--------|
| m01 types | `m01_core_types.rs` | OK (shorthand acceptable) |
| m02 error | `m02_error_handling.rs` | OK |
| m03 config | `m03_config.rs` | OK |
| **m04 timestamp** | `m04_constants.rs` | **WRONG** — module is constants, not timestamp |
| **m05 event** | `m05_traits.rs` | **WRONG** — module is traits, not event |
| **m06 util** | `m06_validation.rs` | **WRONG** — module is validation, not util |
| *(not listed)* | `field_state.rs` | **MISSING** from schematic |

### L2 Wire — 3 wrong names (swapped/renamed)

| Schematic Label | Actual Filename | Status |
|-----------------|-----------------|--------|
| **m07 bus** | `m07_ipc_client.rs` | **WRONG** — module is IPC client, not bus |
| **m08 ipc** | `m08_bus_types.rs` | **WRONG** — module is bus types (m07/m08 labels are swapped) |
| **m09 codec** | `m09_wire_protocol.rs` | **WRONG** — module is wire protocol, not codec |

### L3 Hooks — 4 of 5 wrong

| Schematic Label | Actual Filename | Status |
|-----------------|-----------------|--------|
| m10 server | `m10_hook_server.rs` | OK |
| **m11 pre_tool** | `m11_session_hooks.rs` | **WRONG** — session hooks, not pre-tool |
| **m12 post_tool** | `m12_tool_hooks.rs` | **WRONG** — combined tool hooks (pre+post), not post-only |
| **m13 notification** | `m13_prompt_hooks.rs` | **WRONG** — prompt hooks, not notification |
| **m14 middleware** | `m14_permission_policy.rs` | **WRONG** — permission policy, not middleware |

### L4 Intelligence — ALL 7 wrong

| Schematic Label | Actual Filename | Status |
|-----------------|-----------------|--------|
| **m15 hebbian** | `m15_coupling_network.rs` | **WRONG** |
| **m16 coupling** | `m16_auto_k.rs` | **WRONG** |
| **m17 routing** | `m17_topology.rs` | **WRONG** |
| **m18 breaker** | `m18_hebbian_stdp.rs` | **WRONG** — ironic: breaker label, STDP content |
| **m19 phase** | `m19_buoy_network.rs` | **WRONG** |
| **m20 chimera** | `m20_semantic_router.rs` | **WRONG** |
| **m21 decision** | `m21_circuit_breaker.rs` | **WRONG** — circuit breaker is here, not m18 |

### L5 Bridges — 2 wrong + 1 missing

| Schematic Label | Actual Filename | Status |
|-----------------|-----------------|--------|
| m22 synthex | `m22_synthex_bridge.rs` | OK |
| **m23 maintenance** | `m23_me_bridge.rs` | **WRONG** — short name is "me", not "maintenance" |
| m24 povm | `m24_povm_bridge.rs` | OK |
| **m25 reasoning** | `m25_rm_bridge.rs` | **WRONG** — short name is "rm", not "reasoning" |
| m26 blackboard | `m26_blackboard.rs` | OK |
| *(not listed)* | `http_helpers.rs` | **MISSING** from schematic |

### L6 Coordination — OK (minor label differences only)

All 5 modules match adequately: m27 conductor, m28 cascade, m29 tick, m30 wasm, m31 memory.

### L7 Monitoring — OK

All 4 modules match: m32 otel, m33 metrics, m34 dashboard, m35 tokens.

### L8 Evolution — OK

All 5 modules match: m36 ralph, m37 emergence, m38 correlation, m39 tensor, m40 mutation.

### Schematic Summary

| Layer | Modules | Wrong Names | Missing Files | Accuracy |
|-------|---------|-------------|---------------|----------|
| L1 Core | 6 | 3 | 1 (`field_state.rs`) | 50% |
| L2 Wire | 3 | 3 | 0 | 0% |
| L3 Hooks | 5 | 4 | 0 | 20% |
| L4 Intelligence | 7 | 7 | 0 | 0% |
| L5 Bridges | 5 | 2 | 1 (`http_helpers.rs`) | 60% |
| L6 Coordination | 5 | 0 | 0 | 100% |
| L7 Monitoring | 4 | 0 | 0 | 100% |
| L8 Evolution | 5 | 0 | 0 | 100% |
| **TOTAL** | **40** | **19** | **2** | **52.5%** |

**Verdict:** L1-L5 schematic is severely outdated. L6-L8 are accurate. The schematic was likely written during the planning phase (ORAC_PLAN.md) and never updated after implementation.

---

## 2. API Spec Gaps (API_SPEC.md vs build_router())

### Hook Path Case Mismatch

API_SPEC.md documents hook paths in snake_case. The actual router uses PascalCase:

| API_SPEC.md | build_router() | Status |
|-------------|----------------|--------|
| `/hooks/session_start` | `/hooks/SessionStart` | **CASE MISMATCH** |
| `/hooks/pre_tool_use` | `/hooks/PreToolUse` | **CASE MISMATCH** |
| `/hooks/post_tool_use` | `/hooks/PostToolUse` | **CASE MISMATCH** |
| `/hooks/user_prompt_submit` | `/hooks/UserPromptSubmit` | **CASE MISMATCH** |
| `/hooks/permission_request` | `/hooks/PermissionRequest` | **CASE MISMATCH** |
| `/hooks/stop` | `/hooks/Stop` | **CASE MISMATCH** |

### Endpoints Missing from API_SPEC.md

The router registers **26 route entries** (18 GET + 6 POST + 2 on consent). API_SPEC.md documents only **12** (health + 6 hooks + metrics + field + blackboard + consent GET/PUT + field/ghosts). **10 GET endpoints are undocumented:**

| Endpoint | Handler | Purpose |
|----------|---------|---------|
| `GET /thermal` | `thermal_handler` | SYNTHEX thermal state |
| `GET /traces` | `traces_handler` | OTel trace store query |
| `GET /dashboard` | `dashboard_endpoint_handler` | Kuramoto field dashboard |
| `GET /tokens` | `tokens_handler` | Token accounting summary |
| `GET /coupling` | `coupling_handler` | Coupling network state |
| `GET /hebbian` | `hebbian_handler` | Hebbian STDP state |
| `GET /emergence` | `emergence_handler` | Emergence detector state |
| `GET /bridges` | `bridges_handler` | Bridge health/status |
| `GET /ralph` | `ralph_handler` | RALPH evolution state |
| `GET /dispatch` | `dispatch_handler` | Dispatch/routing state |

### API Spec Summary

| Category | Documented | Actual | Gap |
|----------|-----------|--------|-----|
| Hook endpoints | 6 | 6 | 0 (but all 6 have wrong path casing) |
| GET endpoints | 6 | 18 | **12 undocumented** (10 new + consent GET/PUT counted separately) |
| Total routes | 12 | 26 | **14 missing or mismatched** |

---

## 3. LOC Estimate Drift

### ORAC_PLAN.md Estimates vs Actual

ORAC_PLAN.md estimated ~24,500 LOC total across all phases.

| Component | Plan Estimate | Actual | Drift |
|-----------|---------------|--------|-------|
| DROP-IN modules | 10,170 | *(absorbed into layers)* | N/A |
| ADAPT modules | 5,302 | *(absorbed into layers)* | N/A |
| NEW code | ~9,000 | *(absorbed into layers)* | N/A |
| **Total Plan Estimate** | **~24,500** | **41,369** | **+68.8%** |

### CLAUDE.md Per-Layer Claims vs Actual

| Layer | CLAUDE.md LOC | Actual LOC | Drift | Drift % |
|-------|---------------|------------|-------|---------|
| L1 Core | 4,020 | 4,071 | +51 | +1.3% |
| L2 Wire | 2,300 | 3,028 | +728 | +31.6% |
| L3 Hooks | 2,405 | 5,694 | +3,289 | **+136.8%** |
| L4 Intelligence | 4,402 | 4,752 | +350 | +8.0% |
| L5 Bridges | 4,618 | 7,074 | +2,456 | **+53.2%** |
| L6 Coordination | 2,578 | 2,968 | +390 | +15.1% |
| L7 Monitoring | 4,347 | 4,467 | +120 | +2.8% |
| L8 Evolution | 5,854 | 6,524 | +670 | +11.4% |
| bin/ targets | *(not listed)* | 2,743 | N/A | N/A |
| lib.rs | *(not listed)* | 48 | N/A | N/A |
| **CLAUDE.md Total** | **30,524** | **41,369** | **+10,845** | **+35.5%** |

### Test Count Claims vs Actual

| Source | Claimed Tests | Actual (`#[test]` count) | Drift |
|--------|---------------|--------------------------|-------|
| CLAUDE.md | 1,454 | 1,714 (1,684 src + 30 tests/) | +260 (+17.9%) |
| CLAUDE.local.md | 1,690 | 1,714 | +24 (+1.4%) |

**Worst offenders:** L3 Hooks (137% over) and L5 Bridges (53% over) — both grew significantly during Sessions 055-060 with hook migration, blackboard expansion, and metabolic wiring.

---

## 4. CLAUDE.md vs Reality

### Module Table Accuracy

CLAUDE.md's "Key Modules" table references:

| Module | CLAUDE.md Layer | Actual Layer | Status |
|--------|----------------|--------------|--------|
| m10_hook_server | L3 | L3 (m3_hooks) | OK |
| m20_semantic_router | L4 | L4 (m4_intelligence) | OK |
| m21_circuit_breaker | **L5** | **L4** (m4_intelligence) | **WRONG LAYER** |
| m26_blackboard | L5 | L5 (m5_bridges) | OK |
| m36_ralph_engine | L8 | L8 (m8_evolution) | OK |
| m37_emergence_detector | L8 | L8 (m8_evolution) | OK |
| m39_fitness_tensor | L8 | L8 (m8_evolution) | OK |
| m40_mutation_selector | L8 | L8 (m8_evolution) | OK |

**Issue:** `m21_circuit_breaker` is claimed as L5 but lives in `src/m4_intelligence/`. The schematic also places it wrong (L4 in schematic labels it "decision").

### Module Count and File Extras

CLAUDE.md claims "40 modules" across 8 layers. Actual unique `.rs` files in `src/` (excluding mod.rs, lib.rs, bin/):

- Layer modules: 40 files (m01 through m40) — **matches**
- Extra files not in the 40-module count: `field_state.rs`, `http_helpers.rs` — 2 unlisted support files
- bin/ targets: `main.rs`, `client.rs`, `probe.rs`, `ralph_bench.rs` — 4 binaries (CLAUDE.md says 3: orac-sidecar, orac-client, orac-probe — `ralph_bench` is undocumented)

### Feature List

CLAUDE.md: `api`, `persistence`, `bridges` (default) | `intelligence`, `monitoring`, `evolution` | `full`

CLAUDE.local.md: `default = ["api", "persistence", "bridges", "intelligence", "monitoring", "evolution"]`

**Conflict:** CLAUDE.md says default is `api,persistence,bridges` but CLAUDE.local.md (Session 055 notes) says all 6 are default. The local.md is likely correct (FIX-017 expanded defaults).

### LOC/Test Staleness

CLAUDE.md header says "30,524 LOC, 1,601 tests" but the architecture table sums to 30,524 with 1,454 tests. Meanwhile actual is 41,369 LOC and 1,714 tests. **Three different numbers exist across CLAUDE.md header, CLAUDE.md table, and CLAUDE.local.md — none match reality.**

---

## 5. Recommendations for Corrections

### Priority 1 — High Impact, Low Effort

1. **Regenerate `layer_architecture.mmd`** — 19 of 40 module labels are wrong. The entire L1-L5 section needs rewriting to match actual filenames. This is the single highest-value fix.

2. **Fix API_SPEC.md hook paths** — Change all 6 hook paths from snake_case to PascalCase to match the router (`/hooks/session_start` → `/hooks/SessionStart`).

3. **Add 10 missing endpoints to API_SPEC.md** — Document `/thermal`, `/traces`, `/dashboard`, `/tokens`, `/coupling`, `/hebbian`, `/emergence`, `/bridges`, `/ralph`, `/dispatch` with response schemas.

### Priority 2 — Consistency Fixes

4. **Update CLAUDE.md LOC/test counts** — Header should say ~41,400 LOC, ~1,714 tests. Per-layer table needs updating (L3 Hooks: 5,694, L5 Bridges: 7,074, etc.).

5. **Fix CLAUDE.md m21 layer claim** — `m21_circuit_breaker` is in L4 (m4_intelligence), not L5.

6. **Document `field_state.rs` and `http_helpers.rs`** — Two support files missing from all module listings.

7. **Document `ralph_bench` binary** — 4th bin target not mentioned in CLAUDE.md.

8. **Reconcile default features** — CLAUDE.md and CLAUDE.local.md disagree on whether `intelligence`, `monitoring`, `evolution` are default.

### Priority 3 — Architectural Accuracy

9. **Update ORAC_PLAN.md LOC estimate** — Original ~24,500 estimate is 69% under actual. Add a "Post-Implementation Actuals" section.

10. **Add layer dependency arrows in schematic** — The `.mmd` file references `L8`, `L7` etc. in arrow syntax but these aren't defined as subgraph IDs (subgraphs use quoted strings like `"L8 Evolution [feature: evolution]"`). The dependency arrows at lines 68-90 are syntactically broken.

---

## Appendix: Raw LOC Data (2026-03-25)

```
Layer           Directory            Actual LOC   CLAUDE.md Claim
L1 Core         src/m1_core/         4,071        4,020
L2 Wire         src/m2_wire/         3,028        2,300
L3 Hooks        src/m3_hooks/        5,694        2,405
L4 Intelligence src/m4_intelligence/ 4,752        4,402
L5 Bridges      src/m5_bridges/      7,074        4,618
L6 Coordination src/m6_coordination/ 2,968        2,578
L7 Monitoring   src/m7_monitoring/   4,467        4,347
L8 Evolution    src/m8_evolution/    6,524        5,854
bin/ targets    src/bin/             2,743        (unlisted)
lib.rs          src/                    48        (unlisted)
TOTAL                               41,369       30,524
```

```
Test functions:  src/ = 1,684  |  tests/ = 30  |  TOTAL = 1,714
```
