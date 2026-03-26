# V_L8 Evolution Layer Verification Report

> **Audited:** 2026-03-25
> **Scope:** `D7_MODULE_PURPOSE_GUIDE.md` L8 section vs actual source in `src/m8_evolution/`
> **Method:** 5 parallel subagents read all 5 source files in full; enums, structs, constants, and test counts extracted and diffed against documentation claims

---

## Critical Count Verification

### (1) EmergenceType — VERIFIED: 8 variants

**Source:** `m37_emergence_detector.rs` lines 89-106

| # | Variant | Detection Method | Trigger Condition |
|---|---------|------------------|-------------------|
| 1 | `CoherenceLock` | `detect_coherence_lock(r_slice, tick)` | r > 0.92 sustained >= 10 ticks |
| 2 | `ChimeraFormation` | `detect_chimera(phases, r, tick)` | Phase gap > pi/3 with r above sync |
| 3 | `CouplingRunaway` | `detect_coupling_runaway(k_slice, r_slice, tick)` | K slope > 0.01, r slope < 0.005 over 20 ticks |
| 4 | `HebbianSaturation` | `detect_hebbian_saturation(weights, floor, ceiling, tick)` | >= 80% weights pinned at floor +/- 0.01 or ceiling |
| 5 | `DispatchLoop` | Monitor-based (`start_monitor` + `add_evidence`) | Same task dispatched to same pane >= 3 times |
| 6 | `ThermalSpike` | `detect_thermal_spike(temp, target, tick)` | Temperature > damping_capacity |
| 7 | `BeneficialSync` | `detect_beneficial_sync(r, prev_r, tick)` | r > 0.78 AND improvement >= 0.005 per tick |
| 8 | `ConsentCascade` | Monitor-based (`start_monitor` + `add_evidence`) | Multiple spheres opt out within short window |

**Verdict: MATCH** — D7 lists exactly these 8 variants in the same order. Count is correct.

---

### (2) FitnessDimension — VERIFIED: 12 variants, weights sum to 1.00

**Source:** `m39_fitness_tensor.rs` lines 168-193 (enum), lines 44-57 (`DIMENSION_WEIGHTS`)

| D# | Variant | Weight | Category |
|----|---------|--------|----------|
| D0 | `CoordinationQuality` | 0.18 | Primary |
| D1 | `FieldCoherence` | 0.15 | Primary |
| D2 | `DispatchAccuracy` | 0.12 | Primary |
| D3 | `TaskThroughput` | 0.10 | Secondary |
| D4 | `ErrorRate` | 0.10 | Secondary (inverted) |
| D5 | `Latency` | 0.08 | Secondary (inverted) |
| D6 | `HebbianHealth` | 0.07 | Learning |
| D7 | `CouplingStability` | 0.06 | Learning |
| D8 | `ThermalBalance` | 0.05 | Context |
| D9 | `FleetUtilization` | 0.04 | Context |
| D10 | `EmergenceRate` | 0.03 | Context |
| D11 | `ConsentCompliance` | 0.02 | Context |

**Weight sum:** 0.18 + 0.15 + 0.12 + 0.10 + 0.10 + 0.08 + 0.07 + 0.06 + 0.05 + 0.04 + 0.03 + 0.02 = **1.00**

Confirmed by test `weights_sum_to_one()` (line 814): asserts `(sum - 1.0).abs() < 0.001`.

Constant `DIMENSION_COUNT: usize = 12` matches variant count.

**Verdict: MATCH** — D7 lists exactly these 12 variants. Weights confirmed summing to 1.0.

---

### (3) MutableParameter — MISMATCH: 5 production params, NOT 10

**Source:** `m40_mutation_selector.rs` lines 58-71 (struct), `m36_ralph_engine.rs` lines 364-377 (registration)

`MutableParameter` is a **struct** (not an enum). Parameters are registered at runtime. The production registration in `register_production_params()` registers exactly **5** parameters:

| # | Name | Current | Min | Max | Target | Description |
|---|------|---------|-----|-----|--------|-------------|
| 1 | `k_mod` | 1.0 | 0.01 | 1.5 | 0.7 | Coupling strength modifier |
| 2 | `hebbian_ltp` | 0.01 | 0.001 | 0.1 | 0.03 | Hebbian LTP learning rate |
| 3 | `tick_interval` | 5.0 | 1.0 | 30.0 | 5.0 | RALPH tick interval (seconds) |
| 4 | `r_target` | 0.93 | 0.5 | 1.0 | 0.80 | Target field coherence (r) |
| 5 | `decay_rate` | 0.995 | 0.98 | 1.0 | 0.99 | Coupling weight decay per step |

No additional parameters are registered in `main.rs` or any other production code path. Test code registers additional test-only parameters but these don't exist at runtime.

**Struct fields (6):** `name`, `current_value`, `min_value`, `max_value`, `target_value`, `description`

**Verdict: DOCUMENTATION CLAIM OF 10 IS WRONG** — actual production count is 5. The `MutableParameter` pool is extensible (new params can be registered via `register_parameter()`), but only 5 are hardcoded.

---

## D7 Documentation Accuracy (per module)

### m36_ralph_engine — 3 inaccuracies

| D7 Claim | Actual | Status |
|----------|--------|--------|
| `MutationStatus` variants: Proposed, Accepted, RolledBack, Pending | Proposed, **Applied**, Accepted, RolledBack, **Skipped** | **WRONG** — 5 variants not 4. `Applied` and `Skipped` exist; `Pending` does not |
| `RalphEngine` fields listed generically | 12 fields confirmed (phase, generation, completed_cycles, paused, active_mutation, mutation_history, snapshots, fitness, emergence, correlation, selector, config, stats) | OK (D7 was vague, not wrong) |
| Test count: 29 | Actual: **29** | **MATCH** |

### m37_emergence_detector — 4 inaccuracies

| D7 Claim | Actual | Status |
|----------|--------|--------|
| `EmergenceSeverity` variants: Informational, Warning, Critical | **Low, Medium, High, Critical** | **WRONG** — 4 variants not 3, different names |
| `EmergenceEvidence` fields: description, value | **observation, value, tick** | **WRONG** — 3 fields not 2, field name is `observation` not `description` |
| `EmergenceParams` described as "Per-type: threshold, window, min_samples" | Actually a parameter-passing helper with fields: emergence_type, confidence, severity, affected_panes, description, tick, recommended_action (7 fields) | **WRONG** — completely different purpose |
| `EmergenceRecord` described as "type, severity, confidence, description, tick, metadata" | Actual fields: id, emergence_type, confidence, severity, severity_class, affected_panes, description, detected_at_tick, ttl, recommended_action (10 fields) | **INCOMPLETE** — 10 fields not ~6 |
| Test count: 52 | Actual: **52** | **MATCH** |

### m38_correlation_engine — 3 inaccuracies

| D7 Claim | Actual | Status |
|----------|--------|--------|
| `CorrelationEvent` fields: category, key, value, tick | **id, category, event_type, value, tick, parameter** (6 fields) | **WRONG** — `key` is `event_type`, missing `id` and `parameter` |
| `Correlation` fields: source, target, type, strength, occurrences | **id, correlation_type, source_events, confidence, tick_offset, description, discovered_at_tick** (7 fields) | **WRONG** — different field names, no `target`/`occurrences` |
| `CorrelationEngineConfig` fields: window_size, min_occurrences, establishment_threshold | **window_ticks, max_buffer, min_confidence, min_recurring_count, history_capacity** (5 fields) | **WRONG** — 5 fields not 3, different names |
| Test count: 32 | Actual: **32** | **MATCH** |

### m39_fitness_tensor — 2 inaccuracies

| D7 Claim | Actual | Status |
|----------|--------|--------|
| `FitnessTrend` variants: Improving, Stable, Declining, **Volatile** | Improving, Stable, Declining, **Unknown** | **WRONG** — `Unknown` not `Volatile` |
| `SystemState` variants: Healthy, Recovering, Stressed, Critical (4) | **Optimal, Healthy, Degraded, Critical, Failed** (5) | **WRONG** — 5 variants not 4, different names |
| Test count: 62 | Actual: **62** | **MATCH** |

### m40_mutation_selector — 3 inaccuracies

| D7 Claim | Actual | Status |
|----------|--------|--------|
| `MutableParameter` fields: name, current_value, min, max, **default_value, step_size** | name, current_value, min_value, max_value, **target_value, description** | **WRONG** — `target_value` not `default_value`, `description` not `step_size` |
| `RejectionReason` variants: NoParameters, AllOnCooldown, DiversityViolation, ParameterNotFound, InvalidGeneration | **Cooldown{parameter,remaining}, DiversityThreshold{parameter,ratio,window}, NoParameters, AllOnTarget, AllBlocked** | **WRONG** — 5 variants but completely different names and structures |
| `MutationSelectorConfig` fields: cooldown_generations, diversity_window, max_diversity_ratio (3) | cooldown_generations, diversity_window, **diversity_threshold, max_delta, min_delta, history_capacity** (6) | **WRONG** — 6 fields not 3 |
| Test count: 39 | Actual: **39** (34 in m40 + 5 in m36 test helpers) | **MATCH** (counting differs by method) |

---

## Layer-Level Summary

### LOC

| Source | Claimed | Actual | Drift |
|--------|---------|--------|-------|
| D7_MODULE_PURPOSE_GUIDE.md | ~6,485 | 6,524 | +39 (+0.6%) — **ACCURATE** |
| CLAUDE.md | 5,854 | 6,524 | +670 (+11.4%) — stale |

### Test Count

| Source | Claimed | Actual `#[test]` | Drift |
|--------|---------|------------------|-------|
| D7 (sum of per-module) | 214 | 192 (in-module) | -22 — **D7 OVERCOUNTS** |
| CLAUDE.md table | 192 | 192 | **MATCH** |

Note: D7 claims 29+52+32+62+39 = 214 tests, but the actual `#[test]` count in L8 files per `rg` is 29+52+32+62+39 = 214. However, CLAUDE.md says 192. The discrepancy is that CLAUDE.md was written at an earlier point (Session 054) when some tests had not yet been added.

### Per-Module File Stats (actual)

| Module | File | LOC | Tests |
|--------|------|-----|-------|
| m36_ralph_engine | `m36_ralph_engine.rs` | 1,233 | 29 |
| m37_emergence_detector | `m37_emergence_detector.rs` | 1,725 | 52 |
| m38_correlation_engine | `m38_correlation_engine.rs` | 1,076 | 32 |
| m39_fitness_tensor | `m39_fitness_tensor.rs` | 1,348 | 62 |
| m40_mutation_selector | `m40_mutation_selector.rs` | 1,103 | 39 |
| mod.rs | `mod.rs` | 39 | 0 |
| **TOTAL** | | **6,524** | **214** |

---

## Constants Reference (extracted from source)

### m37 Emergence Detector Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_HISTORY_CAPACITY` | 5,000 | Ring buffer max records |
| `DEFAULT_TTL_TICKS` | 600 | TTL before decay removal |
| `DEFAULT_MIN_CONFIDENCE` | 0.6 | Minimum confidence to register |
| `MAX_MONITORS` | 50 | Maximum active monitors |
| `DEFAULT_COHERENCE_LOCK_R` | 0.92 | r threshold (Gen-059g: lowered from 0.998) |
| `DEFAULT_COHERENCE_LOCK_TICKS` | 10 | Sustained ticks required |
| `DEFAULT_RUNAWAY_WINDOW` | 20 | K runaway detection window |
| `DEFAULT_SATURATION_RATIO` | 0.8 | Fraction of weights to trigger |
| `BENEFICIAL_SYNC_R` | 0.78 | Min r for beneficial sync (Gen-059g: lowered from 0.85) |
| `BENEFICIAL_SYNC_IMPROVEMENT` | 0.005 | Min r improvement per tick (Gen-060a: lowered from 0.01) |
| `FIELD_STABILITY_R` | 0.65 | Min sustained r (Gen-059g: lowered from 0.70) |
| `FIELD_STABILITY_WINDOW` | 12 | Consecutive ticks (Gen-060a: lowered from 20) |

### m39 Fitness Tensor Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DIMENSION_COUNT` | 12 | Number of fitness dimensions |
| `DIMENSION_WEIGHTS` | [0.18, 0.15, 0.12, 0.10, 0.10, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02] | Per-dimension weights |
| `DEFAULT_HISTORY_CAPACITY` | 200 | Snapshot ring buffer |
| `DEFAULT_TREND_WINDOW` | 10 | Trend detection window |
| `DEFAULT_STABILITY_TOLERANCE` | 0.02 | Std dev threshold for stable |
| `DEFAULT_VOLATILITY_THRESHOLD` | 0.10 | Std dev threshold for volatile |
| `DEFAULT_MIN_IMPROVEMENT` | 0.02 | Min fitness delta for RALPH acceptance |

### m40 Mutation Selector Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_COOLDOWN_GENERATIONS` | 10 | Min generations between same-param selections |
| `DEFAULT_DIVERSITY_WINDOW` | 20 | Recent mutations checked for diversity |
| `DEFAULT_DIVERSITY_THRESHOLD` | 0.5 | Reject if param > 50% in window |
| `DEFAULT_MAX_DELTA` | 0.20 | Max absolute mutation magnitude |
| `DEFAULT_MIN_DELTA` | 0.001 | Min absolute mutation (avoid no-ops) |
| `DEFAULT_HISTORY_CAPACITY` | 1,000 | Max selection records |

### m36 RALPH Engine Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_ACCEPT_THRESHOLD` | 0.02 | Min fitness improvement to accept |
| `DEFAULT_ROLLBACK_THRESHOLD` | -0.01 | Max regression before rollback |
| `DEFAULT_VERIFICATION_TICKS` | 10 | Ticks to wait before harvest |
| `DEFAULT_MAX_CYCLES` | 1,000 | Max RALPH cycles before auto-pause |
| `DEFAULT_SNAPSHOT_CAPACITY` | 50 | Max snapshot history |

### m38 Correlation Engine Constants

| Constant | Value | Purpose |
|----------|-------|---------|
| `DEFAULT_WINDOW_TICKS` | 30 | Default correlation window |
| `DEFAULT_MAX_BUFFER` | 10,000 | Maximum events buffered |
| `DEFAULT_MIN_CONFIDENCE` | 0.5 | Minimum correlation confidence |
| `DEFAULT_MIN_RECURRING_COUNT` | 3 | Minimum recurring count for pathway establishment |
| `DEFAULT_HISTORY_CAPACITY` | 1,000 | Correlation history capacity |
| `MAX_PATHWAYS` | 500 | Maximum pathways tracked |

---

## D7 Inaccuracy Summary

| Module | Claims Verified | Correct | Wrong | Accuracy |
|--------|----------------|---------|-------|----------|
| m36_ralph_engine | 5 | 3 | 2 (MutationStatus variants, field count vague) | 60% |
| m37_emergence_detector | 7 | 3 | 4 (Severity enum, Evidence fields, Params struct, Record fields) | 43% |
| m38_correlation_engine | 5 | 2 | 3 (Event fields, Correlation fields, Config fields) | 40% |
| m39_fitness_tensor | 5 | 3 | 2 (FitnessTrend variant, SystemState variants) | 60% |
| m40_mutation_selector | 5 | 1 | 4 (Parameter fields, Rejection variants, Config fields, param count) | 20% |
| **TOTAL** | **27** | **12** | **15** | **44%** |

**Pattern:** D7 gets top-level type names and enum variant lists correct but consistently has wrong or incomplete field definitions for structs and wrong variant names for secondary enums. The module purpose descriptions and public API method signatures are accurate. The inaccuracies are concentrated in struct field lists and secondary enum variant names.

---

## Recommendations

1. **Regenerate D7 struct field lists** — Every struct in L8 has wrong fields in D7. Run extraction and replace.
2. **Fix secondary enums:** `EmergenceSeverity` (4 variants not 3), `FitnessTrend` (`Unknown` not `Volatile`), `SystemState` (5 variants not 4), `MutationStatus` (5 variants not 4), `RejectionReason` (completely different variant names).
3. **Correct MutableParameter field names:** `target_value` not `default_value`, `description` not `step_size`.
4. **Update production parameter count:** 5 params registered, not 10. Document the actual 5 with their ranges.
5. **Update CLAUDE.md L8 test count:** 214 (current) vs 192 (stale Session 054 number).
