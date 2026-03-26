# ACP Plan Review: Glossary Sizing

> **Source document:** `EXECUTIVE_SUMMARY.md` (567 lines, ~4,200 words)
> **Glossary reference:** Plan's "~80 term glossary"
> **Finding:** **The glossary does not exist.** No file named glossary, no glossary section in
> ORAC_PLAN.md, D1_SYSTEM_ATLAS.md, ORAC_MINDMAP.md, VERIFICATION_REPORT.md, or any other
> document in `orac-sidecar/docs/` or `orac-sidecar/`. Full-text search for "glossary",
> "Glossary", "GLOSSARY", "terminology", "~80 terms" across all `.md` files: zero matches.
> **Date:** 2026-03-25

---

## Methodology

1. Read EXECUTIVE_SUMMARY.md line by line
2. Flag every term a **senior Rust developer without physics/neuroscience/oscillator theory background** would not immediately understand
3. Criteria for "would not know":
   - Not standard Rust (ownership, traits, lifetimes, cargo, clippy, etc.)
   - Not standard web/systems (HTTP, REST, JSON, SQLite, Unix socket, etc.)
   - Not standard DevOps (Prometheus, OpenTelemetry, circuit breaker, rate limit, etc.)
   - IS domain-specific to: Kuramoto oscillator theory, neuroscience (Hebbian/STDP), evolutionary algorithms, the ULTRAPLATE ecosystem, or ORAC-invented concepts

---

## Domain-Specific Terms Found (93 terms)

### Physics / Oscillator Theory (16 terms)

| # | Term | First Appearance (line) | Context in EXECUTIVE_SUMMARY |
|---|------|------------------------|------------------------------|
| 1 | **Kuramoto field** | :25 | "field coherence", "Kuramoto field model" |
| 2 | **order parameter (r)** | :95 | "Kuramoto OrderParameter { r, psi }" — r=1.0 lockstep, r=0.0 chaos |
| 3 | **psi (ψ)** | :95 | Mean phase angle in OrderParameter |
| 4 | **phase** | :95 | Oscillator phase on [0, 2π) |
| 5 | **frequency** | :157 | Per-sphere oscillation frequency |
| 6 | **coupling constant K** | :233 | Global coupling strength in Kuramoto model |
| 7 | **K scaling** | :233 | Automatic K adjustment based on frequency spread |
| 8 | **phase integration (RK4)** | :233 | Runge-Kutta 4th order integration of Kuramoto ODEs |
| 9 | **chimera** | :35 | Chimera state — coexistence of synchronised and desynchronised groups |
| 10 | **over-synchronised** | :349 | r too high — fleet in lockstep, no diversity |
| 11 | **coherence** | :349 | Degree of phase alignment (high r = coherent) |
| 12 | **divergence** | :349 | Opposite of coherence — phases spreading apart |
| 13 | **breathing rhythm** | :347 | Oscillation of K to keep fleet in dynamic equilibrium |
| 14 | **phase clusters** | :411 | Groups of spheres with similar phases |
| 15 | **phase gaps** | :411 | Empty regions in the phase distribution |
| 16 | **unit sphere** | :95 | Geometric surface where sphere embeddings live |

### Neuroscience / Learning Theory (14 terms)

| # | Term | First Appearance (line) | Context in EXECUTIVE_SUMMARY |
|---|------|------------------------|------------------------------|
| 17 | **Hebbian STDP** | :33 | Spike-Timing Dependent Plasticity — learning rule |
| 18 | **LTP (Long-Term Potentiation)** | :257 | Weight increase when two panes are co-active |
| 19 | **LTD (Long-Term Depression)** | :257 | Weight decrease when one pane is idle |
| 20 | **co-activation** | :257 | Two panes both in Working state simultaneously |
| 21 | **potentiation** | :257 | Strengthening of a coupling connection |
| 22 | **depression** (synaptic) | :257 | Weakening of a coupling connection |
| 23 | **weight floor** | :257 | Minimum coupling weight (0.15) to prevent total disconnection |
| 24 | **weight ceiling** | :257 | Maximum coupling weight (0.85) to prevent saturation |
| 25 | **saturation** | :257 | All weights pinned at floor or ceiling — no learning signal |
| 26 | **anti-saturation guard** | :257 | Skip STDP when <2 working panes |
| 27 | **burst multiplier** | :257 | 3x LTP boost for co-active events |
| 28 | **newcomer multiplier** | :257 | 2x LTP boost for recently joined panes |
| 29 | **coupling weight** | :25 | Numeric strength of connection between two spheres |
| 30 | **homeostatic normalization** | implied | Periodic pull of extreme weights toward mean |

### Evolutionary Algorithm (11 terms)

| # | Term | First Appearance (line) | Context in EXECUTIVE_SUMMARY |
|---|------|------------------------|------------------------------|
| 31 | **RALPH** | :34 | Recognize-Analyze-Learn-Propose-Harvest evolution engine |
| 32 | **fitness** | :25 | Scalar measure of fleet performance (0.0–1.0) |
| 33 | **fitness tensor** | :435 | 12-dimensional weighted fitness evaluation |
| 34 | **mutation** | :437 | Parameter change proposed by the evolution engine |
| 35 | **generation** | :440 | One complete RALPH cycle |
| 36 | **mutation history** | :466 | Record of all past mutations and their outcomes |
| 37 | **snapshot/rollback** | :440 | State capture before mutation for atomic revert |
| 38 | **round-robin parameter selection** | :500 | Cycling through all parameters to prevent getting stuck |
| 39 | **diversity rejection gate** | :502 | Rejects if >50% of recent mutations hit same parameter |
| 40 | **per-parameter cooldown** | :501 | 10-generation minimum between targeting the same param |
| 41 | **correlation mining** | :466 | Finding temporal/causal/recurring patterns in mutation data |

### ORAC / ULTRAPLATE Ecosystem (37 terms)

| # | Term | First Appearance (line) | Context in EXECUTIVE_SUMMARY |
|---|------|------------------------|------------------------------|
| 42 | **ORAC** | :21 | The sidecar proxy itself (name from Blake's 7 supercomputer) |
| 43 | **ULTRAPLATE** | :54 | The parent 17-service ecosystem |
| 44 | **PV2 / Pane-Vortex** | :36 | Fleet coordination daemon, Kuramoto field authority |
| 45 | **SYNTHEX** | :36 | Thermal regulation brain (port 8090) |
| 46 | **Maintenance Engine (ME)** | :36 | Fitness signal source (port 8080) |
| 47 | **POVM Engine** | :36 | Persistent Oscillating Vortex Memory (port 8125) |
| 48 | **Reasoning Memory (RM)** | :36 | Cross-session TSV knowledge store (port 8130) |
| 49 | **VMS** | :36 | Vortex Memory System (port 8120) |
| 50 | **DevOps Engine** | :59 | Neural orchestration service |
| 51 | **CodeSynthor** | :59 | Code synthesis service |
| 52 | **SAN-K7** | :60 | 59-module orchestrator |
| 53 | **NAIS** | :62 | Neural Adaptive Intelligence System |
| 54 | **Bash Engine** | :62 | Shell script safety analysis service |
| 55 | **Tool Maker** | :62 | Tool generation service |
| 56 | **Context Manager** | :63 | Claude context management service |
| 57 | **Tool Library** | :63 | Tool catalog service |
| 58 | **Architect Agent** | :60 | Pattern library and design service |
| 59 | **Prometheus Swarm** | :60 | Multi-agent swarm service |
| 60 | **Batch 1–5** | :58 | Dependency-ordered startup groups |
| 61 | **sphere** | :95 | Per-pane oscillator representation |
| 62 | **PaneSphere** | :95 | The Rust struct representing a sphere |
| 63 | **PaneId** | :95 | Newtype for pane identifiers |
| 64 | **ghost trace** | :195 | Record of a deregistered sphere |
| 65 | **consent gate** | :561 | Permission check before bridge operations |
| 66 | **consent snapshot** | :357 | Frozen consent state carried through cascades |
| 67 | **blackboard** | :335 | SQLite fleet state database |
| 68 | **cascade handoff** | :355 | Work delegation between fleet panes |
| 69 | **conductor** | :347 | P-controller for field breathing |
| 70 | **tick** | :363 | One 5-second ORAC cycle |
| 71 | **field poller** | :143 | Background task polling PV2 every 5s |
| 72 | **thermal gate** | :203 | PreToolUse check against SYNTHEX temperature |
| 73 | **thermal regulation** | :301 | SYNTHEX PID temperature control |
| 74 | **hydration** | :319 | Loading persisted state on startup |
| 75 | **crystallisation** | :319 | Persisting state on shutdown |
| 76 | **fire-and-forget** | :implicit | Non-blocking HTTP POST pattern |
| 77 | **breaker-guarded** | :implicit | Circuit breaker checked before POST |
| 78 | **devenv** | :56 | ULTRAPLATE developer environment manager |

### Emergence / Pattern Detection (10 terms)

| # | Term | First Appearance (line) | Context in EXECUTIVE_SUMMARY |
|---|------|------------------------|------------------------------|
| 79 | **emergence** | :446 | Spontaneous fleet-level behaviour from local rules |
| 80 | **CoherenceLock** | :449 | Fleet stuck at high r |
| 81 | **ChimeraFormation** | :450 | Healthy phase cluster + gap structure |
| 82 | **CouplingRunaway** | :451 | K increasing without r improvement |
| 83 | **HebbianSaturation** | :452 | >80% weights at extremes |
| 84 | **DispatchLoop** | :453 | Repeated routing to same pane |
| 85 | **ThermalSpike** | :454 | Temperature exceeds damping capacity |
| 86 | **BeneficialSync** | :455 | Spontaneous synchronisation |
| 87 | **ConsentCascade** | :456 | Multiple spheres opting out together |
| 88 | **TTL decay** | :458 | Time-to-live eviction of old events |

### Semantic Routing (5 terms)

| # | Term | First Appearance (line) | Context in EXECUTIVE_SUMMARY |
|---|------|------------------------|------------------------------|
| 89 | **semantic domain** | :273 | Read/Write/Execute/Communicate classification |
| 90 | **domain affinity** | :273 | Per-pane strength in each semantic domain |
| 91 | **phase region** | :273 | Kuramoto phase angle mapped to a domain |
| 92 | **composite scoring** | :273 | Weighted blend of affinity + Hebbian + availability |
| 93 | **preferred pane bonus** | :273 | 15% routing boost for designated pane |

---

## Coverage Assessment

### Glossary Status: **DOES NOT EXIST**

Searched exhaustively:
- `orac-sidecar/docs/` — 28 markdown files, zero contain "glossary" or "Glossary"
- `orac-sidecar/ORAC_PLAN.md` — no glossary section in 415 lines
- `orac-sidecar/ORAC_MINDMAP.md` — no glossary section
- `orac-sidecar/CLAUDE.md` — no glossary
- `orac-sidecar/CLAUDE.local.md` — no glossary
- Full recursive grep of all `.md` files in the repo: zero matches

**The "plan's glossary of ~80 terms" referenced in the task does not exist.** There is no glossary anywhere in the ORAC sidecar repository.

### Gap Analysis

| Category | Terms Found | Terms Explained Inline | Unexplained |
|----------|------------|----------------------|-------------|
| Physics / Oscillator Theory | 16 | 8 (r, phase, chimera, K, over-sync, coherence, breathing, clusters) | 8 (psi, RK4, unit sphere, frequency spread, divergence cooldown, phase gaps, phase integration, K scaling) |
| Neuroscience / Learning | 14 | 7 (STDP, LTP, LTD, co-activation, weight floor, saturation, burst) | 7 (potentiation vs depression as neuroscience terms, homeostatic normalization, newcomer multiplier details, anti-saturation guard mechanism) |
| Evolutionary Algorithm | 11 | 8 (RALPH acronym, fitness, mutation, generation, snapshot, cooldown, diversity gate, correlation) | 3 (fitness tensor dimensionality rationale, temporal vs causal vs recurring correlation types) |
| ORAC / ULTRAPLATE | 37 | 20 (ORAC, PV2, SYNTHEX, ME, POVM, RM, sphere, ghost, consent, blackboard, cascade, tick, hydration, crystallisation) | 17 (most service names just listed without explanation, devenv, batch ordering rationale, fire-and-forget pattern) |
| Emergence | 10 | 8 (all 8 detector names get one-line explanations) | 2 (TTL decay, emergence as a concept) |
| Semantic Routing | 5 | 3 (domains, composite scoring, preferred bonus) | 2 (phase region mapping rationale, domain affinity computation) |
| **Total** | **93** | **54** | **39** |

### Inline Explanation Quality

The EXECUTIVE_SUMMARY uses "Think of it as:" analogies for every module, which is excellent for module-level understanding. However, **terms cross-cut modules** — a reader encountering "Kuramoto" in m15 needs the definition from m01, and "STDP" in m29 needs the definition from m18.

**What the summary does well:**
- Explains RALPH as an acronym (line 433)
- Explains r=1.0 and r=0.0 (line 95)
- Explains LTP/LTD with concrete numbers (line 257)
- Explains all 8 emergence types with one-liners (lines 449–456)
- Explains the 12 fitness dimensions with weights (lines 476–489)

**What the summary does NOT explain:**
- What a Kuramoto oscillator *is* (a differential equation for coupled oscillators)
- Why RK4 (4th-order Runge-Kutta) and what it integrates
- What STDP stands for beyond the expansion (the neuroscience model)
- Why coupling weights have a floor and ceiling (prevents trivial equilibria)
- What psi (ψ) means geometrically (mean phase angle)
- Why phases map to [0, 2π) (circular topology)
- What "thermal" means in SYNTHEX (not physical heat — computational load)
- What "field" means in this context (not electromagnetic — Kuramoto order field)

---

## Recommendations

### 1. Create the Glossary (93 terms, not 80)

The term count exceeds the ~80 estimate. A glossary file should be created at
`orac-sidecar/docs/D9_GLOSSARY.md` with these sections:

```
- Physics / Oscillator Theory (16 terms)
- Neuroscience / Learning Theory (14 terms)
- Evolutionary Algorithm (11 terms)
- ORAC / ULTRAPLATE Ecosystem (37 terms)
- Emergence / Pattern Detection (10 terms)
- Semantic Routing (5 terms)
```

### 2. Priority Terms (must-define for onboarding)

These 15 terms are the absolute minimum for a new developer to read the codebase:

1. **Kuramoto model** — what it is, why coupled oscillators model AI agent coordination
2. **order parameter r** — what 0.0 and 1.0 mean, how it's computed
3. **phase** — oscillator angle on [0, 2π), why circular
4. **coupling constant K** — what it controls, why it modulates
5. **sphere** — why a pane is modelled as an oscillator
6. **STDP** — the neuroscience model, how it maps to pane co-activity
7. **LTP / LTD** — concrete: which direction, what triggers each
8. **RALPH** — the 5 phases, what each phase does
9. **fitness** — what the scalar represents, how 12 dims collapse to 1
10. **chimera** — why it's desirable (diversity), not a failure
11. **thermal** — that it means computational load, not physical temperature
12. **field** — that it means the Kuramoto order field, not electromagnetic
13. **hydration / crystallisation** — load-from-DB / persist-to-DB
14. **consent gate** — not OAuth — informed consent for bridge operations
15. **emergence** — spontaneous macro-pattern from micro-rules

### 3. Executive Summary Inline Fixes

Add a 10-line "Key Concepts" box at the top of EXECUTIVE_SUMMARY.md before the module guide,
defining: Kuramoto, r, phase, K, sphere, STDP, LTP/LTD, RALPH, fitness, chimera. This would
reduce the 39 unexplained terms to ~20 and make the document self-contained.

### 4. Cross-Reference Strategy

Each term in the glossary should link to:
- The module where it's defined (e.g., "Kuramoto" → m01, m15, m27, m29)
- The constant where it's tuned (e.g., "LTP rate" → m04 `HEBBIAN_LTP`)
- The Obsidian note with deeper theory (e.g., "Kuramoto" → `[[Vortex Sphere Brain-Body Architecture]]`)

---

## Summary

| Metric | Value |
|--------|-------|
| Domain-specific terms in EXECUTIVE_SUMMARY | **93** |
| Terms explained inline | 54 (58%) |
| Terms unexplained | 39 (42%) |
| Glossary exists | **NO** |
| Glossary file found anywhere in repo | **NO** |
| Original estimate | ~80 terms |
| Actual count | 93 terms (16% over estimate) |
| Priority terms for onboarding | 15 |
| Recommended glossary location | `docs/D9_GLOSSARY.md` |
