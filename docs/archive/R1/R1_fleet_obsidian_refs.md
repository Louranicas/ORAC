# R1 Fleet Obsidian References — ORAC Sidecar

> **244 files scanned** | **190 in shared-context** | **54 in claude_code** | **3 vaults mapped**
> **Generated:** 2026-03-25 | **Source:** ORAC_MINDMAP.md (399 lines, 19 branches, 148+ leaves)
> **Vault keys:** `[M]` = Main (`~/projects/claude_code/`), `[S]` = Shared-Context (`~/projects/shared-context/`), `[P]` = PV2 (`~/projects/pane-vortex-v2/`)

---

## 1. Session Notes (Chronological)

### Session 061 (2026-03-25) — Bridge Debt + Adversarial Convergence

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 061 — Star Tracker Long-Duration Report.md` | [S] | RALPH +2,381 gens (1857->4238), fitness plateau 0.76, field r=0.89-0.95, LTP flat at 414 (idle), 2,161 emergence events, 17/17 services 12+ hours continuous |
| `Session 061 — ORAC Reflections and Architectural Insights.md` | [S] | ORAC as Envoy-like proxy for AI agent traffic, 30,524 LOC / 40 modules / 8 layers, consent model ("field modulates, does not command"), fire-and-forget bridge debt identified, 8-type emergence vocabulary, RALPH underutilized (46 dispatch_total) |
| `Session 061 — Fire-and-Forget Bridge Debt Analysis.md` | [S] | 21 fire-and-forget call sites across 5 bridges, VMS consolidation 422 silent failure, POVM ID mismatch (orac:PID:UUID vs service IDs), circuit breakers detect connectivity not semantic failures |
| `Session 061 — Final Plan (Gap-Corrected).md` | [S] | 6-tier bridge debt fix (~170 LOC + ~300 LOC tests), T1 parse status_buf (PvResult<()>->PvResult<u16>), T4 VMS breaker, semantic failure detection |
| `Session 061 — Adversarial Synthesis — Final Plan.md` | [S] | Critics destroyed 3/5 original components, found 5 critical gaps (VMS query_semantic exists, POVM ID structural, SYNTHEX broken at 0.923 temp, handle_stop race, LOC 2.5-4x underestimated) |
| `Session 061 — Distributed Intelligence Workflow and Learnings.md` | [S] | 36 adversarial voices across 3 rounds, 12 bugs fixed, 6 tiers deployed, Adversarial Convergence Protocol (ACP) validated |
| `Session 061 — G1 Metabolic Fix Deployment.md` | [S] | 4 fixes: STDP guard (LTD saturation 99.6%->0%), homeostatic normalization, 6 diagnostic routes, emergence monitor wiring; RALPH +11.5% (0.731->0.815) |
| `Session 061 — Fleet Bug Hunt.md` | [S] | Fleet-wide bug hunt results, multiple ORAC-touching issues discovered |
| `Adversarial Convergence Protocol.md` | [S] | 3-round methodology (divergence->opposition->verification), 12 sources per round, ORAC fire-and-forget debt validated by this protocol |
| `Battern — Patterned Batch Dispatch for Claude Code Fleets.md` | [S] | Protocol for dispatch + gate + collect pattern, ORAC integration via fleet-ctl |

### Session 060 (2026-03-24) — Habitat Activation + Deep Exploration

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 060 — Habitat Activation Plan.md` | [S] | 3 layers of dormancy (static HTTP, disconnected events, thermal starvation), 7-phase activation plan, ME EventBus 365K events / 0 external subscribers, RALPH gen=1744 fitness=0.742 r=0.871, 45 spheres / 1980 connections |
| `Session 060 — Plan Gap Analysis.md` | [S] | GAP-1: ME EventBus alive (333,750 events, publishers exist, only external subscriber missing ~30 LOC bridge); GAP-2: SYNTHEX heat sources unreported not missing; 6 gaps total, 3 assumptions wrong |
| `Session 060 — Agentic Synergy Gap Analysis.md` | [S] | D1 Learning: LTP/LTD=0.0545 (overwhelmingly depressing), weight_mean=0.1575; D2 Memory: no upward flow VMS->POVM->RM; D3 Emergence: 1 of 8 types firing (~300 events all FieldStability) |
| `Session 060 — Key Learnings and Reflections.md` | [S] | Structure vs metabolism insight: 500K+ LOC architecturally perfect but services return mock data at runtime; ME EventBus->PV2 bridge (~50 LOC) is highest leverage; emergence 0->130, r 0.730->0.898 (+23%) |
| `Session 060 — Habitat Deep Exploration Report.md` | [S] | 9 fleet agents, 480+ test cases, 17 services, 500K+ LOC, 15K+ tests, 40 bugs, ORAC confirmed as integration hub (5 bridges, 40 modules, 4 feedback loops), 3 isolated learning systems (ME Hebbian, ORAC STDP, VMS) |
| `Session 060 — Fleet Synergy & Nexus Bus Discovery.md` | [S] | 25+ cross-service data flows, 166+ SQLite DBs, 5 nexus bus locations, ME EventBus NB-1 CRITICAL (365K events, needs bridge), ~120 LOC would raise composite synergy 0.764->0.91+ |
| `Session 060 — Fleet Fix Deployment.md` | [S] | 6 fixes deployed: devenv cross-session kill (BUG-001), POVM hydration, session persistence, emergence detectors (BeneficialSync r>0.85, FieldStability), token accounting, ME observer subscription; 1665->1690 tests |

### Session 059 (2026-03-24) — Evolution Chamber Activation

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 059 — Metabolic Bridge Wiring and Diagnostics.md` | [S] | ME + RM bridges wired (were unwired), ME fitness=0.616, RM TSV persistence, POVM pathways persist 60 ticks, all 5 breakers healthy, quality gate 4/4, 1665 tests |
| `Session 059 — Evolution Chamber Activation.md` | [M] | 4-block deployment: coupling pruning, RALPH persistence (singleton blackboard row), POVM format fix (source/target -> pre_id/post_id), ChimeraFormation + correlation wiring; RALPH gen=17 fitness=0.5277 |

### Session 058 (2026-03-24) — GAP-A and GAP-B Fixes

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 058 — GAP-A and GAP-B Fix Deployment.md` | [S] | GAP-A: STDP LTP=0 FIXED (coupling network seed logic, removed is_empty() guard); GAP-B: IPC socket dead FIXED; LTP=384 LTD=3375 after fix |
| `Session 058 — Reflections.md` | [S] | Structure != metabolism, 1,665 tests with zero clippy but STDP never fired (namespace mismatch), `is_empty()` creates bifurcation, distributed bugs are identity bugs |

### Session 057 (2026-03-23) — 21-Generation ORAC + Fleet Enhancement

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 057 — 21-Generation ORAC + Fleet Enhancement.md` | [S] | 21 alternating generations (ORAC fixes + fleet toolkit), consent-gating guards, 375 LOC dead code removed, fleet toolkit 6 new commands |
| `Session 057 — ORAC Deep Exploration and PV Coherence Fix.md` | [M] | BUG-049 CRITICAL: PV2 `/spheres` format mismatch (fix: PvSphereCompact adapter, sphere_count 0->66). BUG-050: RALPH empty parameter pool. PV coherence: 59 stale spheres deregistered, r 0.977->0.649->healthy |
| `Session 057 — ORAC Internal Architecture.md` | [M] | 8 layers, 40 modules, Mermaid dependency diagram, v0.6.0, 31,200 LOC, 1,650 tests, all feature gates documented |
| `Session 057 — Gap Analysis and Activation Plan.md` | [M] | 6 gaps: GAP-A (STDP dead, sphere ID mismatch), GAP-B (PV2 IPC dead), GAP-C (sessions volatile), GAP-D (ME EventBus 0 publishers), GAP-E/F (SYNTHEX/breaker OK). POVM: 178+ memories, 2451+ pathways |
| `Session 057 — Database Architecture Map.md` | [M] | 42 files, 20 with data (488KB devenv + 488KB POVM + 160KB PV2), 6 database paradigms, ORAC blackboard as P1 WAL SQLite |
| `Session 057 — API Endpoint Reference.md` | [M] | 17 services probed: ORAC `/health` + 6 hook endpoints, ME fitness=0.616, SYNTHEX T=0.529, NAIS 92-line stub dormant |
| `Session 057 — Service Topology Map.md` | [M] | Mermaid topology: ORAC in Batch 5 (depends PV2 + POVM), active flows: ORAC->SYNTHEX, ORAC->POVM, ORAC->RM, ORAC->PV2 |
| `Session 057 — Hebbian Learning and Evolution Schematic.md` | [M] | STDP signal chain: Tool use -> PostToolUse hook -> PV2 sphere status -> co_activations++ -> apply_stdp(). LTP=0.01, LTD=0.002, burst 3x. Semantic router: 40% domain + 35% Hebbian + 25% availability |
| `Session 057 — Metabolic Wiring Map.md` | [M] | 6 active data flows (F1-F6), ORAC is central hub — all active flows originate from or transit through it |
| `Session 057 — SYNTHEX Deep Architecture.md` | [M] | PID controller (Kp=0.5, Ki=0.1, Kd=0.05), T_actual=0.529, 4 heat sources, ORAC feeds ingest endpoint |
| `Session 057 — Diagnostic Runbook.md` | [M] | Per-service diagnostic commands, ORAC (8133): health + field + blackboard + metrics |
| `Session 057 — Metabolic Activation.md` | [M] | 5 metabolic phases: SYNTHEX thermal wired, VMS seeded, STDP persists to POVM, ME EventBus wired, fleet toolkit; 11 ORAC bug fixes |
| `Session 057 — Fleet Findings alpha-left.md` | [M] | ME EventBus zero publishers in spawn_health_polling |
| `Session 057 — Fleet Findings alpha-br.md` | [M] | SYNTHEX 4 heat sources wired, DevOps static facade (28.7K LOC behind 195-line stub) |
| `Session 057 — Fleet Findings beta-left.md` | [M] | VMS 0 memories, 21-gen ORAC + CC cycle, Static Facade Pattern systemic |
| `Session 057 — Fleet Findings beta-tr.md` | [M] | IPC bus sphere.* events never broadcast (100% field.tick), CodeSynthor TCP stub |
| `Session 057 — Fleet Findings beta-br.md` | [M] | ORAC-related integration findings |
| `Session 057 — Fleet Findings gamma-left.md` | [M] | 77 synergy pairs, 10 paths, 19 gaps (Swarm<->RM, POVM<->VMS, ORAC<->PV2) |
| `Session 057 — Fleet Findings gamma-tr.md` | [M] | Hebbian: only 1 sphere Working at a time, STDP not persisted, POVM co_activations hardcoded=0 |
| `Session 057 — Fleet Findings gamma-br.md` | [M] | ORAC fleet integration findings |

### Session 056 (2026-03-23) — God-Tier Mastery + 31 Bug Fixes

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 056 — ORAC God-Tier Mastery.md` | [S] | 30,524 LOC, 40 modules, 8 layers, 34 bugs found / 31 fixed, RALPH 5-phase loop, 5 bridges, circuit breaker FSM, semantic router, consent, ghost traces, Kuramoto field |
| `Session 056 — Complete Fleet Report.md` | [S] | 9 CC instances, 3 phases, BUG-036 sphere deserialization CRITICAL (0->66), BUG-G01 RALPH neutral-accept, 21-gen ralph-loop, 19 registered / 17 active services |

### Session 055 (2026-03-22) — Runtime Wiring + Fleet Operations

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| (Documented in CLAUDE.local.md) | — | 19/19 fixes complete, 1,506 tests, cross-service composite 0.764 (Grade A), field r=0.91, RALPH fitness=0.61, 12 CC instances, relay chain 4/4 verified |

### Session 054 (2026-03-22) — Phase 4 Evolution + Full Completion

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 054 — ORAC Plan Complete.md` | [M] | 40/40 modules, 30,524 LOC, 1,454 tests, all 14 critical path steps done, L8 evolution layer complete (RALPH + emergence + correlation + fitness tensor + mutation selector) |

### Session 053 (2026-03-22) — Phase 2 Intelligence

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 053 — ORAC Phase 2 Intelligence + Gold Standard Audit.md` | [M] | L4 Intelligence: semantic_router (803 LOC), circuit_breaker (870 LOC), blackboard (920 LOC). Gold Standard Audit: 16x cast_precision_loss fixed, 0 pedantic warnings |
| `Session 053b — ORAC Full Deploy Assessment.md` | [M] | FULLY WIRED, FULLY ASSIMILATED. PID 401026 on :8133, 40 modules, all 6 hooks migrated, bridge connectivity 6/6 verified |

### Session 052 (2026-03-22) — Phase 1 Hooks Deployed

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 052 — Phase 1 Hooks Deployed.md` | [M] | 5 hook modules (m10-m14), 2,405 LOC, 699 tests, 6 endpoints on :8133, live integration 9/9 pass, binary deployed 4.7MB, Git 903fdd2 pushed |

### Session 051 (2026-03-22) — .claude Scaffolding

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 051 — ORAC Sidecar .claude Scaffolding.md` | [M] | 18 files / 5 directories scaffolded: context.json (8 layers, 6 bridges, 40 modules), status.json, patterns.json (22 patterns), anti_patterns.json (20), 3 skills (/orac-boot, /hook-debug, /bridge-probe) |

### Session 050 (2026-03-22) — ORAC Design + Planning

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 050 — ORAC Sidecar Architecture.md` | [M] | "From dumb pipe to intelligent fleet coordination proxy." V1 was 546 LOC / non-functional. V2: Envoy-like, 8 layers, 40 modules, 33-feature backlog |
| `Session 050 — Sidecar Deep Dive.md` | [M] | V1 analysis: 546 LOC, Tokio async, 5-layer data flow (WASM->FIFO->Unix socket->IPC bus->Ring), wire protocol frames documented |
| `Session 050 — Sidecar Features Research.md` | [M] | 33 features from 16 web searches. Tier 1 (14 must-haves): HTTP hooks, permission policy, circuit breaking, health routing, OTel, semantic routing. Tier 2 (11), Tier 3 (8 future) |
| `Session 050 — Hook Pipeline vs Sidecar Gap.md` | [M] | 10 gaps only sidecar can fill: real-time push, bidirectional streaming, persistent multiplexing, sub-second coordination, cross-pane awareness, high-frequency STDP, persistent fleet state, WASM bridge, closed-loop thermal, HTTP server |
| `Session 050 — ME Evolution Chamber Spec.md` | [M] | RALPH engine 5-phase spec, fitness tensor 12D, safety mechanisms (emergence cap, sandbox, rollback), PBFT vs Raft analysis |
| `Session 050 — ULTRAPLATE Module Inventory.md` | [M] | 41 PV2 modules scored for ORAC hot-swap: 18 drop-in (9.8-9.9/10), 6 adapt (9.4-9.6/10), rest skip |
| `Session 050 — Workflow and Tracking.md` | [M] | Session 050 workflow state |
| `Session 050 — ORAC Pre-Scaffold Complete.md` | [M] | Pre-scaffold completion status |

### Session 045 (2026-03-20) — Arena + Sidecar Failure Analysis

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 045 Arena — gamma-left-povm-analysis.md` | [M] | POVM analysis for ORAC bridge design |
| `Session 045 Arena — 12-live-field-analysis.md` | [M] | Live Kuramoto field analysis, ORAC field dashboard design source |
| `Session 045 Arena — 08-advanced-habitat-synergies.md` | [M] | Advanced habitat synergies informing ORAC coordination design |
| `Session 045 Arena — gen2-beta-left.md` | [M] | Second-generation fleet findings |

### Session 042 (2026-03-20) — V2 Inhabitation

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 042 — V2 Inhabitation and BUG-008 Fix.md` | [M] | PV2 V2 deployment, ORAC predecessor architecture context |
| `Session 042 — Learnings and Tool Chain Mastery.md` | [M] | Tool chain mastery informing ORAC build process |

### Session 041 (2026-03-20) — V2 Deployment

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 041 — V2 Deployment and Deep Exploration.md` | [M] | V2 deployed (PID 2880134, port 8132), field empty (0 spheres), all 6 ORAC bridge targets reachable, SYNTHEX T=0.572, ME fitness=0.607 |

### Session 036 (2026-03-17) — Exploration Checkpoint

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 036 — Exploration Cycle Checkpoint T+240m.md` | [M] | Early exploration, predates ORAC design, V1 sidecar architecture context |

### Session 034 (2026-03-15-16) — Remediation Planning

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Session 034d — Comprehensive Remediation Plan.md` | [M] | 35 issues / 7 facets / 7 phases. ORAC-relevant: evolution chamber not wired (HIGH), persistent bus listener missing (HIGH), k_mod compounding unguarded (HIGH) |
| `Session 034c — Swarm Exploration and Synergy Analysis.md` | [M] | 7-agent swarm, BUG-016 Nexus bridge schema mismatch, 92 Hebbian pathways, GAMMA-synthesizer coupling 0.75 |

### Session 027 (2026-03-14)

| File | Vault | Key ORAC Insights |
|------|-------|-------------------|
| `Zellij Synthetic DevEnv — Session 027.md` | [M] | 7 tabs, 10 WASM plugins, 16 services. ORAC not yet deployed. Pre-ORAC Habitat architecture. |

---

## 2. Architecture Notes

### Core ORAC Architecture

| File | Vault | Key Content |
|------|-------|-------------|
| `ORAC Sidecar — Architecture Schematics.md` | [S] | 8 layers with LOC breakdown: L1 Core (4,020), L2 Wire (2,300), L3 Hooks (2,405), L4 Intelligence (4,402), L5 Bridges (4,618), L6 Coordination (2,578), L7 Monitoring (4,347), L8 Evolution (5,854). Hook pipeline: SessionStart->UserPromptSubmit->PreToolUse->PostToolUse->Stop->PermissionRequest |
| `ORAC Sidecar — Architecture Schematics and Wiring.md` | [M] | Port 8133, 30,524 LOC, 1,454 tests, FULLY_DEPLOYED. 8 layers with module-level breakdown, feature gates documented |
| `ORAC Sidecar — Diagnostic Schematics.md` | [S] | 8 diagnostic diagrams: DB schema ERD, runtime state, health/metrics, coupling network, tensor computation, emergence detection, bridge topology, evolution loop. Blackboard.db 6 tables |
| `ORAC Sidecar — Full Integration Schematic.md` | [M] | 30,853 LOC, 1,454 tests, 3 binaries, 5 bridge services, 6 hook endpoints, feature gates |
| `ORAC .claude Folder — Bidirectional Index.md` | [M] | 40/40 modules, root files (context.json, status.json, patterns.json, anti_patterns.json), 3 skills, 5 schemas |
| `ORAC — RALPH Multi-Parameter Mutation Fix.md` | [M] | BUG-035: mono-parameter mutation trap (318/380 mutations to min_confidence). Fix: round-robin + cooldown (10 gens) + diversity gate. 13 mutable parameters documented |

### ORAC Mindmap (ORAC_MINDMAP.md — 399 lines)

19 major branches with full Obsidian cross-references:

| Branch | Leaf Count | Key Notes Referenced |
|--------|-----------|---------------------|
| 1. HTTP Hook Server | 12 | Hook Pipeline vs Sidecar Gap, Consent Flow Analysis, Synthex thermal |
| 2. IPC Client (V2 Wire) | 10 | PV IPC Bus Session 019b, Swarm Orchestrator v3.0, Bugs |
| 3. Intelligence Layer | 16 | Hebbian topology, Semantic router, Circuit breaker, Blackboard |
| 4. RALPH Evolution | 12 | ME Evolution Spec, Emergence, Correlation, Fitness Tensor, BUG-035 |
| 5. Monitoring/Observer | 10 | OTel, Per-Agent Metrics, Kuramoto Dashboard, Token Accounting |
| 6. Bridge Subset | 12 | SYNTHEX, ME, POVM, RM bridges with per-service note links |
| 7. WASM Bridge | 8 | Swarm Orchestrator, FIFO/Ring Protocol, V1->ORAC transition |
| 8. Fleet Dispatch | 12 | Task Routing, Fleet State, Fleet Tools, Multi-Instance Awareness |
| 9. Cascade Handoffs | 8 | PreCompact, Cascade Depth, Sphere Mitosis, Shared-Context |
| 10. Consent/Governance | 10 | NA-P-1, Governance Actuator (7 GAPs), Sphere Agency |
| 11. Scaffold System | 8 | scaffold-gen, plan.toml, 8-Layer Template, Quality Gate |
| 12. Kuramoto Coupling | 8 | Phase Dynamics, Order Parameter, Chimera Detection, arxiv 2508.12314 |
| 13. Architecture Schematics | 8 | Complete schematics across Sessions 034-039 |
| 14. Database & Persistence | 5 | DATABASE_SCHEMA, DB Architecture, Feedback Loop Analysis |
| 15. Memory Systems | 6 | 6 paradigms: Auto-Memory, SQLite, RM, MCP KG, Obsidian, Shared-Context |
| 16. ULTRAPLATE Ecosystem | 6 | Master Index, DevEnv, Bugs, Master Plan V2/V3, Stack Guide |
| 17. Habitat Skills & Tools | 8 | Skills Roster, Tool Library, Tool Maker, Naming & Philosophy |
| 18. Session History | 12 | Sessions 039->042->044->045->050->052 critical path |
| 19. Candidate Modules | 20 | Drop-in (10,516 lines), Adapt (5,420 lines), 7-step integration protocol |

### Cross-Service Topology

| File | Vault | Key Content |
|------|-------|-------------|
| `Session 057 — Service Topology Map.md` | [M] | Master Mermaid diagram: Batch 1-5 dependency graph, ORAC at Batch 5 apex |
| `Session 057 — Metabolic Wiring Map.md` | [M] | 6 active data flows all transit through ORAC (central hub) |
| `Session 057 — Database Architecture Map.md` | [M] | 42 DB files, 6 paradigms, ORAC blackboard as P1 WAL SQLite |
| `Session 057 — API Endpoint Reference.md` | [M] | 17-service endpoint inventory with response schemas |
| `Session 057 — Hebbian Learning and Evolution Schematic.md` | [M] | STDP signal chain through ORAC, semantic router formula (40/35/25), RALPH 5-phase |

---

## 3. Bug Reports

### ORAC-Specific Bug Notes

| File | Vault | Key Bugs |
|------|-------|----------|
| `ULTRAPLATE — Bugs and Known Issues.md` | [S]+[M] | SSOT for all 17 services. ORAC bugs: BUG-036 sphere deser (FIXED), BUG-037 breaker stuck (RESOLVED), BUG-038 RALPH fitness regression (IMPROVING), BUG-039-042 medium bugs |
| `ULTRAPLATE — Bugs and Known Issues — ORAC Update 2026-03-23.md` | [S] | ORAC-specific: BUG-036 CRITICAL FIXED, BUG-037 HIGH RESOLVED, BUG-038 HIGH IMPROVING, IPC auto-connect on startup |
| `Session 060 bug triage` (`tasks/bug-triage-session-060.md`) | [S] | 587 lines, 45 bugs ranked: 7 CRIT, 13 HIGH, 8 MED, 7 LOW + 5 synergy gaps |
| `Session 061 — Fleet Bug Hunt.md` | [S] | Fleet-wide bug hunt with ORAC-touching issues |
| `ORAC — RALPH Multi-Parameter Mutation Fix.md` | [M] | BUG-035 mono-parameter trap: 318/380 mutations to same param. Fix: round-robin + diversity gate |
| `Session 059b — BUG-059d PV2 Breaker Fix.md` | [S] | PV2 circuit breaker fix affecting ORAC bridge connectivity |

### Bug Progression Timeline

| Bug | First Seen | Status | Fix Session |
|-----|-----------|--------|-------------|
| BUG-001 | Session 042 | WORKAROUND | devenv stop doesn't kill (nix SIGTERM/SIGKILL) |
| BUG-016 | Session 034c | OPEN | Nexus bridge schema mismatch |
| BUG-027 | Session 050 | DOCUMENTED | cp alias trap (use \cp -f) |
| BUG-031 | Session 050 | FIXED | Hebbian Phase 2.5 wiring |
| BUG-032 | Session 054 | FIXED | ProposalManager Default max_active=0 |
| BUG-033 | Session 054 | FIXED | Bridge URLs http:// prefix |
| BUG-034 | Session 054 | DOCUMENTED | POVM write-only (use /hydrate) |
| BUG-035 | Session 054 | FIXED | Mono-parameter mutation trap |
| BUG-036 | Session 056 | FIXED | Sphere deserialization (sphere_count 0->66) |
| BUG-037 | Session 056 | RESOLVED | PV2 breaker stuck Open (self-resolved after BUG-036) |
| BUG-038 | Session 056 | IMPROVING | RALPH fitness regression 36% |
| BUG-039-042 | Session 056 | FIXED | Ghost traces, emergence inert, IPC disconnected, HTTP duplication |
| BUG-049 | Session 057 | FIXED | PV2 /spheres format mismatch (PvSphereCompact adapter) |
| BUG-050 | Session 057 | FIXED | RALPH empty parameter pool |
| BUG-051 | Session 057 | FIXED | peak_fitness never updated |
| BUG-052 | Session 057 | FIXED | /blackboard missing RALPH state |
| GAP-A | Session 058 | FIXED | STDP LTP=0 (coupling seed is_empty() guard) |
| GAP-B | Session 058 | FIXED | PV2 IPC socket dead |
| GAP-C | Session 059 | FIXED | Sessions not persisted (blackboard singleton row) |
| GAP-D | Session 059 | NOT A BUG | ME EventBus works via pull model (271K events) |
| CRIT-01 | Session 060 | OPEN | Prometheus Swarm SIGABRT on POST /api/tasks |
| BUG-SCAN-001-006 | Session 061 | FIXED | 6 bugs deployed via G1 metabolic fix |

---

## 4. Cross-Service Notes

### Notes Where ORAC Appears as Integration Target

| File | Vault | ORAC Role |
|------|-------|-----------|
| `ULTRAPLATE Master Index.md` | [S]+[M] | Service #17 (now #19 with ORAC registered), Batch 5, port 8133 |
| `ULTRAPLATE Skills Library.md` | [S] | 88+ skills, ORAC integrates via STDP pathway persistence (POVM every 60 ticks) |
| `Fleet Commander — Modularization Plan and Gap Analysis.md` | [S] | 12 bash scripts -> Rust `fc` binary, integrates with 6 ULTRAPLATE services including ORAC |
| `Maintenance Engine — Deep Exploration (2026-03-24).md` | [S] | ME 54,412 LOC, fitness=0.609, ORAC bridge healthy, EventBus 365K events |
| `SYNTHEX — Deep Exploration (2026-03-24).md` | [S] | Port 8090, V3 homeostasis (T=0.4987), ORAC feeds /api/ingest every 6 ticks |
| `Toolchain Deep Probe — Bash Engine + Tool Maker + Tool Library.md` | [S] | 215K+ LOC across 3 services, ORAC semantic router uses tool classification |
| `POVM Engine.md` | [M] | 2,614 LOC, 70 tests, port 8125. ORAC bridges for memory hydration (SessionStart), pathway persistence (PostToolUse), consolidation |
| `DEVELOPER_ENVIRONMENT_MANAGER.md` | [M] | devenv.toml service registration, ORAC as Batch 5 service |
| `FORTEX SOFTWARE DEVELOPMENT KIT.md` | [M] | Framework reference, Sphere Vortex Framework context |

### Task Files with ORAC Integration Mappings

| File | Vault | Focus |
|------|-------|-------|
| `tasks/findings-fleet-orac-integration-map.md` | [S] | ORAC integration topology across all 17 services |
| `tasks/integration-topology-draft.md` | [S] | Draft integration topology |
| `tasks/findings-fleet-synergy-nexus.md` | [S] | Cross-service synergy with ORAC as nexus bus candidate |
| `tasks/findings-fleet-me-deep.md` | [S] | ME deep findings with ORAC bridge analysis |
| `tasks/findings-fleet-synthex-deep.md` | [S] | SYNTHEX deep findings with ORAC thermal integration |
| `tasks/findings-fleet-vms-deep.md` | [S] | VMS deep findings with ORAC memory bridge |
| `tasks/findings-fleet-rm-nais-deep.md` | [S] | RM + NAIS findings with ORAC bridge analysis |
| `tasks/dim5-orac-capabilities.md` | [S] | ORAC capability dimension analysis |
| `tasks/dim5-ipc-bus.md` | [S] | IPC bus dimension with ORAC IPC client |
| `tasks/dim6-povm.md` | [S] | POVM dimension with ORAC persistence bridge |
| `tasks/dim4-thermal.md` | [S] | Thermal dimension with ORAC-SYNTHEX bridge |
| `tasks/dim3-hebbian.md` | [S] | Hebbian dimension with ORAC STDP |
| `tasks/dim4-cross-service-synergy.md` | [S] | Cross-service synergy through ORAC hub |
| `tasks/dim1-ralph-evolution.md` | [S] | RALPH evolution dimension |
| `tasks/dim2-emergence.md` | [S] | Emergence detection dimension |
| `tasks/dim9-vms-rm.md` | [S] | VMS + RM through ORAC bridge |
| `tasks/dim8-k7-tools.md` | [S] | K7 + tools through ORAC routing |

### Handoff Files (Fleet Operations)

27 handoff files in `~/projects/shared-context/tasks/handoff-*.md` reference ORAC directly, documenting fleet cascade missions for ORAC-related work across Sessions 055-060.

### Gap Analysis Files

12 gap analysis files in `~/projects/shared-context/tasks/gap-*.md` and `nexus-*.md` reference ORAC in the context of Session 061 adversarial convergence analysis.

### Bridge Debt Analysis Files

8 bridge debt files in `~/projects/shared-context/tasks/bridge-debt-*.md` and `fleet-bridge-*.md` analyze ORAC fire-and-forget patterns.

### Battern Files

13 battern files in `~/projects/shared-context/tasks/battern-*.md` reference ORAC in the context of fleet dispatch pattern design.

---

## 5. Bidirectional Link Targets

Notes that **should link to ORAC docs** but currently may not have explicit `[[ORAC Sidecar]]` links:

### High Priority (Direct ORAC Dependency)

| Note | Vault | Why It Should Link |
|------|-------|--------------------|
| `[[Synthex (The brain of the developer environment)]]` | [M] | ORAC L5 SYNTHEX bridge (thermal writeback, heat source ingestion) |
| `[[The Maintenance Engine V2]]` | [M] | ORAC L5 ME bridge (fitness signal), RALPH evolution origin |
| `[[POVM Engine]]` | [M] | ORAC L5 POVM bridge (memory hydration, pathway crystallisation) |
| `[[Pane-Vortex — Fleet Coordination Daemon]]` | [M] | ORAC L2 IPC client, Kuramoto coupling, sphere management |
| `[[Vortex Sphere Brain-Body Architecture]]` | [M] | ORAC Kuramoto coupling, buoy network, field theory |
| `[[Oscillating Vortex Memory]]` | [M] | ORAC LTP/LTD dynamics source theory |
| `[[Fleet System — Memory Index]]` | [M] | ORAC blackboard shared fleet state |
| `[[NAM BOOK]]` | [M] | ORAC consent model theoretical foundation |

### Medium Priority (Architecture Context)

| Note | Vault | Why It Should Link |
|------|-------|--------------------|
| `[[The Swarm Orchestrator]]` | [M] | ORAC L7 WASM bridge predecessor |
| `[[Swarm Orchestrator — Complete Reference]]` | [M] | V1 sidecar context, FIFO/ring protocol |
| `[[Session 044 — Fleet Orchestration Pioneer]]` | [M] | Fleet dispatch architecture that ORAC now serves |
| `[[Consent Flow Analysis]]` | [M] | ORAC consent/governance layer (L10) |
| `[[Self-Governing Agent Coordination — Design Notes 2026-03-08]]` | [M] | ORAC permission policy design source |
| `[[Executor and Nested Kuramoto Bridge — Session 028]]` | [M] | ORAC coupling network mathematics |
| `[[POVM Persistence Bridge — Session 025]]` | [M] | ORAC-POVM bridge design origin |
| `[[Session 045 — Sidecar and Fleet Failure Analysis]]` | [M] | V1 failure analysis that motivated ORAC redesign |
| `[[Maintenance Engine — Architecture Schematic]]` | [M] | ME bridge target architecture |
| `[[Maintenance Engine — Database Schema]]` | [M] | ME data model ORAC reads via bridge |
| `[[Sidecar Backpressure Module — Architecture and Schematics]]` | [M] | ORAC circuit breaker design source |
| `[[ME Tensor Architecture]]` | [M] | ORAC 12D fitness tensor design source |
| `[[Maintenance Engine — 12D Tensor Specification]]` | [M] | Tensor dimension definitions used in m39 |
| `[[Hybrid Tensor Memory Upgrade Plan]]` | [M] | Future ORAC tensor memory integration |
| `[[ME RALPH Loop Specification]]` | [M] | RALPH 5-phase loop specification (m36 source) |

### Lower Priority (Indirect References)

| Note | Vault | Why It Should Link |
|------|-------|--------------------|
| `[[ULTRAPLATE Non-Anthropocentric Gap Analysis 2026-03-07]]` | [M] | ORAC emergence detector design context |
| `[[Fleet-ALPHA Exploration Report]]` | [M] | Multi-instance awareness context |
| `[[Fleet-BETA Tool Mastery Report]]` | [M] | Fleet tools that ORAC coordinates |
| `[[Fleet-GAMMA Research Report]]` | [M] | Research context for ORAC intelligence layer |
| `[[Session 039 — Architectural Schematics and Refactor Safety]]` | [M] | Early architecture schematics ORAC builds on |
| `[[Fleet Drill Report — 7 Generation Practice]]` | [M] | Fleet health aggregation context |
| `[[ULTRAPLATE Quick Start — Session 031]]` | [M] | Fleet health aggregation context |
| `[[M4 NAM-ANAM Engine Fix Report 2026-03-07]]` | [M] | Consent model implementation context |
| `[[The Habitat — Naming and Philosophy]]` | [M] | ORAC naming and sphere agency context |

---

## 6. Summary Statistics

| Metric | Count |
|--------|-------|
| Total files scanned | 244 |
| shared-context vault hits | 190 |
| claude_code vault hits | 54 |
| Session notes (chronological) | 45+ |
| Architecture docs | 12 |
| Bug report files | 6 primary + 45 bugs tracked |
| Cross-service integration files | 27+ |
| Handoff files | 27 |
| Gap/nexus analysis files | 12 |
| Bridge debt files | 8 |
| Battern files | 13 |
| Bidirectional link targets | 24 high/medium + 9 lower |
| ORAC_MINDMAP branches | 19 |
| ORAC_MINDMAP Obsidian note refs | 148+ |
| Recommended new Obsidian notes | 16 (per mindmap) |
| Sessions spanning ORAC work | 027-061 (design in 050, deployed 052, current 061) |
| LOC progression | 0 (050) -> 30,524 (054) -> 32,000 (060) |
| Test progression | 0 -> 699 (052) -> 1,454 (054) -> 1,690 (060) |
| Bug fix progression | 0 -> 31 (056) -> 34 (057) -> 45+ (060) -> 57+ (061) |
