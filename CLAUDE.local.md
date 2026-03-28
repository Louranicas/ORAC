# ORAC Sidecar — Local Development Context

```json
{"v":"0.12.0","status":"FEEDBACK_LOOPS_LIVE","phase":"evolution-wired","port":8133,"plan":"ORAC_PLAN.md","mindmap":"ORAC_MINDMAP.md","plan_toml":"plan.toml","scaffold_modules":40,"layers":8,"bin_targets":4,"tests":1711,"loc":41714,"files":55,"clippy":0,"modules_implemented":40,"modules_stub":0,"hooks_migrated":true,"ralph_live":true,"ralph_persisted":true,"ralph_gen":12878,"ralph_fitness":0.802,"ralph_phase":"Harvest","ralph_feedback_loops":3,"ralph_hint_sources":["emergence","dimension","pathway"],"select_with_hint":true,"learned_hint_field":true,"synthex_wired":true,"synthex_temp":0.538,"vms_wired":true,"povm_persist_wired":true,"povm_format_fixed":true,"me_eventbus_confirmed_working":true,"me_observer_subscribed":true,"bridge_urls_fixed":true,"field_poller":true,"coupling_pruning":true,"ipc_client":true,"ipc_connected":true,"ipc_state":"subscribed","orac_client":true,"circuit_breaker_wired":true,"semantic_router_wired":true,"blackboard_wired":true,"blackboard_tables":10,"session_persistence":true,"token_accounting_wired":true,"consent_endpoints":true,"ghost_tracking":true,"chimera_detection":true,"correlation_wired":true,"correlation_feedback_wired":true,"default_features":"api,persistence,bridges,intelligence,monitoring,evolution","endpoints_total":22,"orac_state_fields":33,"busframe_variants":11,"bugs_fixed_session060":8,"bugs_discovered_fleet":45,"docs_active":24,"docs_active_lines":12468,"docs_archived":9,"docs_archived_lines":4023,"gap_fill_docs":5,"gap_fill_lines":3193,"cc_instances_session062":40,"acp_rounds":5,"gap_analysis":{"GAP_A":"FIXED_session058","GAP_B":"FIXED_session058","GAP_C":"FIXED_session060","GAP_D":"NOT_A_BUG","GAP_E":"WORKING","GAP_F":"LOW_PRIORITY"},"coupling_connections":6320,"coupling_weight_mean":0.179,"field_r":0.857,"sphere_count":80,"emergence_events":36,"emergence_monitors":17,"ltp_total":70,"ltd_total":0,"ltp_idle_gating":"G1_G2_G3_active","vault_renamed":"maintenance-engine-v2 → the-habitat-docs","vault_files":121,"vault_links":109,"vault_broken_links":0,"obsidian_notes_created":33,"star_tracker_entries":19,"session":"065","session_065_changes":["select_with_hint_m40","learned_hint_m36","learn_phase_3source_rewrite","emergence_parameter_mapping","dimension_parameter_mapping","pathway_extraction","8_new_tests"],"next_actions":["monitor_hint_guided_mutations","wire_DispatchLoop_ConsentCascade_detectors","fix_prometheus_crash","git_commit_push_session065"]}
```

---

## Session 064 — Bug Hunt + Workflow Architecture (2026-03-27/28)

**Status:** SYSTEM HARDENED — 20 bugs fixed, 1,703 tests, 11 commits to GitLab, 5 slash commands created.

### Habitat Bootstrap (New Context Window)

Run these 4 commands at the start of every new context window:

1. `/zellij-mastery` — Zellij config, layouts, plugins, dispatch stack, keybinds
2. `/primehabitat` — The Habitat: 17 services, IPC bus, memory systems, fleet
3. `/deephabitat` — deep substrate: wire protocol, databases, ecosystem, tools
4. `/sweep` — probes all 17 services + ORAC + thermal + field coherence

Then read this file for ORAC-specific session context.

### Resume State
- ORAC running on :8133 with RALPH gen=12,100+, fitness=0.78+, 17/17 services
- Use slash commands: `/gate`, `/sweep`, `/deploy-orac`, `/acp`, `/battern`
- Coupling hydration: deferred, restored=3,660. VMS feed: REST /v1/query_semantic.
- PV2 POST /bus/events: deployed. PV2 decision: NeedsDivergence (unblocked).

### Slash Commands (use these!)
| Command | Purpose |
|---------|---------|
| `/gate` | Quality gate (4 stages, zero tolerance) |
| `/sweep` | Health sweep (17 services + ORAC + thermal + field) |
| `/deploy-orac` | Build + deploy cycle (encodes all traps) |
| `/acp` | Adversarial Convergence Protocol (3 rounds) |
| `/battern` | Fleet batch dispatch (roles + gates + collect) |
| `/nerve` | Continuous Nerve Center dashboard (10s refresh) |
| `/propagate` | Push command table to all service CLAUDE.md files |
| `/nvim-mastery` | Neovim RPC: LSP, treesitter, 37 keymaps, 22 snacks features, structural analysis |
| `/atuin-mastery` | Shell history intelligence: search, stats, service density, time-of-day, KV store |

### Session 064 Achievements
- 20 bugs fixed across ORAC + PV2 + SYNTHEX (11 GitLab commits, ~300 LOC)
- Coupling weight persistence: restored=3,660 (was 0)
- VMS→RALPH REST feed live (Gen-063f)
- PV2 POST /bus/events endpoint deployed
- 4 CRITICAL security vectors closed (SEC-001 through SEC-004)
- G1b homeostatic normalization fixed (ceiling 0.85, epsilon 0.01)
- Token double-counting fixed
- Zombie session pruning active
- Table pruning bounded (hebbian_summary 1000, consent_audit 500)
- Permission policy loaded from hooks.toml config (SEC-001 complete)
- SYNTHEX synergy silent error fixed (let _ = → if let Err)
- 7 stale blocked spheres deregistered (PV2 decision unblocked)
- Baseline metrics captured (150+ measurements)
- 20 workflows catalogued, 15 tested (all passing)
- 5 slash commands created

### Next Actions
- Wire POVM co-activations (~15 LOC in PostToolUse) — substrate learning
- Fix ME EventBus last mile (bridge silently failing)
- Stable sphere IDs (longer-term, makes all weights persist naturally)

### Previous Session Reference
- Session 062: ORAC System Atlas + Gap Fill (24 docs, 12,468L)
- Session 063: Deep Habitat Exploration (10,726 lines fleet output)
- Session 060: Habitat Activation Plan (MOST CODE ALREADY IMPLEMENTED)

### What Was Done
- **ORAC System Atlas (ACP):** 3 rounds, 29 CC instances, 8 deliverables (D1-D8), 9 fleet reports, 10 verification reports
- **Gap Analysis:** Audited 5 developer concerns, found ADR/runbook/glossary/scaling/debug gaps
- **ACP Gap Plan Review:** 2 rounds, corrected from 6→5 docs, added CONTRIBUTING + PRODUCTION_READINESS
- **Gap-Fill:** ADR_INDEX (624L, 15 ADRs), OPERATIONS (1,396L, 25 SYMs), GLOSSARY (348L, 104 terms), CONTRIBUTING (465L, 0 AI refs), PRODUCTION_READINESS (360L)
- **D6 Scaling Merge:** +260L (complexity, bottleneck, O(N²), SQLite contention, projections)
- **EXECUTIVE_SUMMARY:** Added 4 audience reading paths (developer/ops/architect/AI)
- **R1 Fleet Archived:** 9 files → docs/archive/R1/ (superseded)
- **Vault Renamed:** maintenance-engine-v2/ → the-habitat-docs/, Obsidian config updated
- **Cross References:** 18 files in Cross References/ subdir (borrowed, labeled)
- **Empty Files Fixed:** 4 files populated from source vaults
- **Links Verified:** 109 wikilinks, 0 broken

### Key ACP Corrections
- BusFrame: single enum (11 variants), NOT ClientFrame/ServerFrame
- OracState: 32 fields (not 56)
- Routes: 22 (not 18 or 23)
- Bridge calls: blocking sync in RALPH tick loop (not fire-and-forget)
- Schematic labels: 14/40 were wrong, all corrected in D8
- Entire doc corpus was AI-oriented — CONTRIBUTING.md addresses human developers

### RALPH Autonomous Evolution (20+ hours unattended)
- Gen: 1,711 → 5,678 (+3,967 generations)
- Fitness: 0.581 → 0.779 (+34%)
- Emergence: 95 → 4,585 (+4,490 events)
- LTP/LTD ratio: 0.0735 → 0.052 (declining — fleet idle, will recover)

### Obsidian
- `[[Session 062 — ORAC System Atlas (ACP)]]` — main note (32 bidirectional links)
- `[[Session 062 — Reflections and Learnings]]` — reflections
- `[[Adversarial Convergence Protocol]]` — updated with Session 062 metrics
- `[[ULTRAPLATE Master Index]]` — updated with ORAC atlas deliverable table
- Vault: `the-habitat-docs/` (121 files, 0 broken links)

### Quality Gate
- 1,748 tests (--features full) — 0 failures
- `cargo clippy -D warnings -W clippy::pedantic` — 0 warnings

---

## Deployment Plan (READY — execute with "deploy plan")

**Read these 3 docs in order:**
1. `~/projects/shared-context/Session 060 — Habitat Activation Plan.md` (the plan)
2. `~/projects/shared-context/Session 060 — Plan Gap Analysis.md` (corrections)
3. `~/projects/shared-context/Session 060 — Agentic Synergy Gap Analysis.md` (synergy fixes)

**Ignition Core (~110 LOC) — do first:**
- ME EventBus → PV2 bridge: `the_maintenance_engine/src/main.rs` — add HTTP POST bridge from EventBus to `localhost:8132/bus/events` (~30 LOC). Publishers ALREADY EXIST (333K events). Just need external consumer.
- SYNTHEX thermal: Check `developer_environment_manager/synthex/src/api/rest_server.rs` — verify `/api/ingest` handler calls `thermal.update_heat_sources()`. May be 5 LOC serialization fix.
- VMS consolidation: Add `POST /v1/adaptation/trigger` call every 300 ticks in ORAC RALPH loop (~5 LOC)
- ORAC VMS query: Add semantic query to RALPH Recognize phase reading VMS memories (~40 LOC)
- Coupling weights: Add `coupling_weights` table to blackboard following sessions pattern (~30 LOC)

**Tier S Synergy (~45 LOC) — do second:**
- `src/m4_intelligence/m18_hebbian_stdp.rs`: Skip STDP when working_count < 2 (~5 LOC)
- `src/m8_evolution/m37_emergence_detector.rs`: HebbianSaturation check `weight < floor + 0.01` (~3 LOC)
- `src/bin/main.rs`: Wire DispatchLoop + ConsentCascade in feed_emergence_observations (~30 LOC)
- `src/m8_evolution/m37_emergence_detector.rs`: `COHERENCE_LOCK_R: 0.998 → 0.98` (~1 LOC)

**Current system state (live at session end):**
- ORAC: gen=1754, fitness=0.735, r=0.948, emergence=243, 44 spheres, 1892 coupling
- ME: fitness=0.609, EventBus 333K events (0 external subscribers), 4.16M correlations
- VMS: r=0.999, 135 memories, morphogenic_cycle=0
- SYNTHEX: temp=0.600, target=0.500, overall_health=0.75
- LTP/LTD ratio: 0.055 (target: >0.15)
- All 5 breakers: Closed, 100% success rate

**Bug triage:** `~/projects/shared-context/tasks/bug-triage-session-060.md` (587 lines, 45 bugs ranked)
**Fleet findings:** `~/projects/shared-context/tasks/findings-fleet-*.md` (9 files, 6544 lines)

---

## Session 060 — Habitat Deep Exploration + Fleet Fix Deployment (2026-03-24)

**Status:** EVOLUTION ACTIVE — 7-gen RALPH loop, 9-pane fleet exploration, 37 docs produced, emergence firing.

### Resume Protocol
1. Run `/primehabitat` then `/deephabitat`
2. Read this file
3. ORAC running on :8133 with RALPH gen=1711+, emergence=95+, field_r=0.92
4. 17/17 services healthy (Prometheus needs restart if crashed — CRIT-01)
5. Check `~/projects/shared-context/tasks/bug-triage-session-060.md` for fix priority list

### Code Changes Deployed
- **POVM parse fix:** Serde aliases `pre_id/post_id`, both array and wrapped response formats, MAX_RESPONSE 512KB
- **Session hydration (GAP-C):** New `sessions` table in blackboard, save/load/remove methods, periodic persist
- **Emergence detection:** BeneficialSync threshold lowered (r>0.85), FieldStability detector (r>0.70, 20-tick window), tick_decay wiring
- **Token accounting:** PostToolUse → record_pane_usage with chars/4 estimate
- **ME observer:** successful_polls counter, is_subscribed() accessor, me_observer_subscribed health field
- **Tool Library:** Port swap fix BashEngine 8101→8102, NAIS 8102→8101
- **devenv BUG-001:** Cross-session kill via nix::sys::signal (SIGTERM→SIGKILL fallback)
- **main.rs refactor:** hydrate_startup_state() extracted, clippy single_match_else fix

### Fleet Exploration (9 panes, 6 agents)
- 9 fleet findings files: SYNTHEX, VMS, ME, SAN-K7, Tool chain, RM+NAIS, ORAC integration, bugs, synergies
- 37 documentation files, 12,500+ lines
- 587-line bug triage: 7 CRIT, 13 HIGH, 8 MED, 7 LOW + 5 synergy gaps
- Consolidated report: `Session 060 — Habitat Deep Exploration Report.md`

### Quality Gate
- 1690 tests (--features full) — 0 failures
- `cargo clippy -D warnings -W clippy::pedantic` — 0 warnings

### Metrics Trajectory
| Metric | Start | End |
|--------|-------|-----|
| emergence_events | 0 | 95+ |
| field_r | 0.730 | 0.922 |
| ralph_gen | 1665 | 1711 |
| ralph_fitness | 0.520 | 0.581 |
| coupling_connections | 506 | 1260 |
| sphere_count | 23 | 36 |
| sessions_persist | no | yes |
| POVM_hydration | parse_error | 2504 pathways loaded |

### Known Issues
- **CRIT-01:** Prometheus Swarm SIGABRT on POST /api/tasks (pre-compiled binary, Python wrapper?)
- **POVM ID mismatch:** 2504 pathways loaded but 0 matched coupling IDs (ORAC uses orac-hostname:pid:uuid sphere IDs)
- **DispatchLoop/ConsentCascade:** Implemented in m37 but not wired in feed loop (monitor-based API)

### Obsidian
- `[[Session 060 — Fleet Fix Deployment]]`
- `[[Session 060 — Habitat Deep Exploration Report]]`
- `[[Session 060 — Fleet Synergy & Nexus Bus Discovery]]`

---

## Session 059 — Evolution Chamber Activation (2026-03-24)

**Status:** EVOLUTION ACTIVE — 4-block deployment, RALPH persists across restarts, STDP firing, chimera detection wired.

### Resume Protocol
1. Run `/primehabitat` then `/deephabitat`
2. Read this file
3. ORAC running on :8133 with RALPH persisted (gen=17+), STDP active, all bridges live
4. Monitor RALPH fitness trajectory — should trend upward from 0.528
5. Check POVM co_activations after 60+ ticks (format fix deployed)

### Changes Deployed (4 Blocks)
- **B1: Coupling pruning** — stale entries removed each poll cycle (`m10_hook_server.rs`)
- **B2: RALPH persistence (GAP-C)** — `ralph_state` table in blackboard, save/60 ticks, hydrate on startup (`m26_blackboard.rs`, `m36_ralph_engine.rs`, `main.rs`)
- **B3: POVM format fix** — `source/target` → `pre_id/post_id/weight` (`main.rs:persist_stdp_to_povm`)
- **B4: Emergence wiring** — ChimeraFormation (6th detector) + emergence→correlation engine (`main.rs`)

### Gap Analysis Corrections
- **GAP-D DOWNGRADED:** ME EventBus has 271K events via pull model. Not a bug.
- **Phase force-sync REMOVED:** Would disrupt Kuramoto natural evolution.
- **DispatchLoop/ConsentCascade DEFERRED:** Low ROI.

### Quality Gate
- 1665 tests (--features full) — 0 failures
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings

### Obsidian
- `[[Session 059 — Evolution Chamber Activation]]`
- Reasoning Memory: `r69c19eca0beb`

### Next Steps
1. Monitor RALPH for 50+ generations — fitness should trend >0.55
2. Verify POVM co_activations > 0 (format fix needs 60 ticks to take effect)
3. Git commit + push all changes
4. Update [[ULTRAPLATE Master Index]] with Session 059 references

---

## Session 058 — GAP-A and GAP-B Fix Deployment (2026-03-24)

**Status:** METABOLICALLY ACTIVE — 2 critical gaps fixed, STDP learning live (LTP=138), IPC subscribed, weight differentiation active (0.15→0.43), 8 fleet missions dispatched.

### Resume Protocol
1. Run `/primehabitat` then `/deephabitat`
2. Read this file
3. ORAC running on :8133 — start via: `cd /home/louranicas/claude-code-workspace/orac-sidecar && RUST_LOG=orac_sidecar=info PORT=8133 PV2_ADDR=127.0.0.1:8132 SYNTHEX_ADDR=127.0.0.1:8090 POVM_ADDR=127.0.0.1:8125 RM_ADDR=127.0.0.1:8130 nohup /home/louranicas/.local/bin/orac-sidecar > /tmp/orac-sidecar-session058.log 2>&1 &`
4. NOTE: devenv restart orac-sidecar may fail (stdout redirect issue) — use manual start above
5. If PV2 IPC fails: kill old PV2 (`ss -tlnp sport=:8132`), `/usr/bin/rm -f /run/user/1000/pane-vortex-bus.sock`, `devenv restart pane-vortex`, restart ORAC

### GAP-A: STDP LTP=0 — FIXED
**Root cause:** Coupling network seed code only ran when `connections.is_empty()`. After SessionStart hooks registered `orac-<uuid>` connections, PV2 sphere IDs never got registered. STDP endpoint mismatch → LTP=0 permanently.
**Fix:** Removed `is_empty()` guard in `spawn_field_poller` (~line 1149 of `m10_hook_server.rs`). Now always syncs PV2 spheres into coupling network. Also fixed `hebbian_ltp_total` and `hebbian_ltd_total` counters in `main.rs`.
**Result:** LTP=138, LTD=3375, weight range 0.15→0.43 after 23 ticks. Learning alive.

### GAP-B: PV2 IPC Socket Dead — FIXED
**Root cause:** Old PV2 process (40h uptime) survived devenv restart. New process couldn't bind. Socket existed but listener dead.
**Fix:** Kill old PV2, `/usr/bin/rm -f` socket (bypass trash alias), fresh PV2 start, ORAC manual start.
**Result:** IPC state=subscribed on first connect.

### Remaining Gaps
- **GAP-C (sessions=0):** In-memory only. Expected after ORAC restart. Long-term: hydrate from blackboard.
- **GAP-D (ME events=0):** Observer not subscribed to "health"/"metrics" channels. Fleet agent investigating.

### ORAC Bug Fixes (11)
- C001: Host header uses full addr:port (IPv6-safe)
- C002: IPC reconnect escalating backoff (5s→120s cap)
- H001: Thermal NaN/INF returns neutral 1.0
- H002: Silent DB errors → tracing::warn
- M001: Consent defaults consistent (serde default_true)
- M002: First thermal poll fires immediately
- M003: Breaker tracks last_failure_tick + failure_age_ticks()
- M004: field_state debug_assert + div-by-zero guard
- M005: Breaker FSM: success in Open stays Open
- L002: TSV single-pass sanitize_into (zero-alloc)
- L004: Reconnect counter resets on success
- **CRITICAL**: All 4 bridge URLs fixed: localhost→127.0.0.1

### CC Fleet Toolkit (10 scripts)
- **New:** cc-common.sh, cc-monitor, cc-abort, cc-capture, cc-replay, fleet-constants.sh
- **Enhanced:** cc-dispatch (audit log), cc-scan (cc-common), cc-status (parallel), cc-deploy/cc-cascade/cc-harvest (--json)

### Metabolic Activation (4 Phases)
- **Phase 1:** SYNTHEX heat sources wired — ORAC posts field state to /api/ingest every 6 ticks
- **Phase 2:** VMS seeded (24 memories, 9 spheres, r=0.9994), ORAC→VMS bridge posts every 30 ticks
- **Phase 3:** STDP weight changes persist to POVM pathways every 60 ticks
- **Phase 4:** ME EventBus wired (3 publish calls in spawn_health_polling), cross-service synergy mapped

### Fleet Reports (7 findings)
- findings-alpha-left-me-eventbus.md (228 lines)
- findings-alpha-br-synthex-heat.md (219 lines)
- findings-beta-left-vms-activation.md (228 lines)
- findings-beta-tr-ipc-nexus.md (205 lines)
- findings-gamma-left-synergy-map.md (208 lines)
- findings-gamma-tr-hebbian-network.md (226 lines)
- metabolic-activation-plan-2026-03-23.md

### Quality Gate
- 1649 tests (ORAC), 0 clippy pedantic
- ME: compiles clean with EventBus wiring

### Next Steps
1. PV2 IPC event broadcasting (4 insertions in api.rs — gamma-br findings pending)
2. Monitor SYNTHEX PID convergence to target 0.50
3. Verify ME EventBus subscriber count increases
4. Git commit + push ORAC + ME changes

---

## Session 056 — Full Fleet Exploration + 31 Bug Fixes + Schematics (2026-03-23)

**Status:** PRODUCTION HARDENED — 9 CC instances deployed, 34 bugs found, 31 fixed, 1,601 tests, 0 clippy pedantic, commit `54671db` pushed to GitLab.

### Resume Protocol (New Context Window)
1. Run `/primehabitat` then `/deephabitat`
2. Read this file (`CLAUDE.local.md`)
3. ORAC is running on :8133, RALPH cycling with neutral-accept fix
4. All persisted: GitLab, Obsidian (6 notes), POVM, RM, auto-memory, Master Index

### Key Artifacts Created This Session
- **Obsidian:** `Session 056 — ORAC God-Tier Mastery.md` (consolidated learnings)
- **Obsidian:** `Session 056 — Complete Fleet Report.md` (full fleet ops report)
- **Obsidian:** `ORAC Sidecar — Architecture Schematics.md` (8 Mermaid diagrams, 25K)
- **Obsidian:** `ORAC Sidecar — Diagnostic Schematics.md` (8 diagnostic diagrams, 18K)
- **Obsidian:** `ULTRAPLATE — Bugs and Known Issues — ORAC Update 2026-03-23.md` (34 bugs, fix status)
- **Obsidian:** `Fleet Commander — Modularization Plan and Gap Analysis.md` (future project)
- **Master Index:** Updated with §13 Projects for Future Deployment + all ORAC notes
- **cc-* toolkit:** 6 scripts at `~/.local/bin/` (scan, status, dispatch, cascade, deploy, harvest)
- **fleet-* enhancements:** fleet-star, fleet-ctl (3 new subcommands), fleet-heartbeat, fleet-inventory, fleet-sphere-sync
- **Git:** `54671db` — 36 files, +5133/-1303, pushed to `git@gitlab.com:lukeomahoney/orac-sidecar.git`

### Bug Fix Summary (34 found, 31 fixed)
- **3 CRITICAL fixed:** BUG-036 (sphere deser), BUG-L4-001 (coupling bypass), BUG-L4-002 (breaker tick=0), BUG-G01 (RALPH neutral-accept)
- **9 HIGH fixed:** L3-002, L3-001, L1-002, L1-003, BUG-037 (self-resolved), BUG-038 (improving)
- **15 MED fixed:** L3-003, L4-004, L3-005, L4-003, L4-006, L1-004, L1-005, L1-006, L1-007, L2-001, L2-002, L3-004, BUG-041, BUG-040, BUG-042
- **4 LOW fixed:** L2-005, L1-008, L1-010, L2-003, L2-004, L1-009, L3-006, BUG-043
- **3 DEFERRED:** monitoring endpoints (dormant), WASM bridge (by design), tick Phase 4+5 (planned)

### Fleet Topology Used
```
Phase 1 Exploration:  Command + ALPHA-left + BETA-left (3 instances)
Phase 2 Bug Fixes:    ALPHA-tr + ALPHA-br + BETA-left + BETA-br + BETA-tr + GAMMA-br + GAMMA-tr (7 instances)
Phase 3 Schematics:   ALPHA-left + BETA-left (2 instances)
```

### Next Steps (Recommended)
1. **Fleet Commander v0.5** — Phase 0: enumerate callers, define fleet-state.json contract
2. **BUG-G01 verification** — Monitor RALPH fitness over 100+ generations to confirm neutral-accept fix holds
3. **ORAC devenv registration** — Verify `devenv restart orac-sidecar` works
4. **Emergence detector** — Feed real observations, verify 8 detection types trigger

---

## Session 056 — Fleet Bug-Fix + Integration Test (2026-03-23)

**Status:** INTEGRATION VERIFIED — 3-instance fleet exploration found 9 bugs, 6 fixed (4 by beta-left, 2 by beta-tr), 3 remaining low/deferred. All endpoints operational, 1,599 tests, quality gate 4/4 clean.

### Bug Fix Summary (Session 056)

| Bug | Severity | Fix Instance | Status |
|-----|----------|-------------|--------|
| BUG-036 | CRITICAL | Session 056 command | FIXED + VERIFIED LIVE (sphere_count=66) |
| BUG-037 | HIGH | Self-resolved | RESOLVED (breakers Closed after BUG-036 fix) |
| BUG-038 | HIGH | — | IMPROVING (fitness 0.432, recovering with real data) |
| BUG-041 | MEDIUM | Session 055/056 | FIXED (IPC auto-connect, subscribed) |
| BUG-L3-001 | HIGH | beta-left | FIXED (pv2_open → pv2_blocked) |
| BUG-L3-002 | HIGH | beta-left | FIXED (total_tool_calls increment) |
| BUG-L3-003 | MED | beta-left | FIXED (UTF-8 safe truncation) |
| BUG-L4-004 | MED | beta-left | FIXED (word-boundary matching) |
| m27 partial move | — | beta-tr | FIXED (.clone() on decision.action) |
| m22 unused import | — | beta-tr | FIXED (#[cfg(test)] gate) |

### RALPH State

- Generation: 10+
- Fitness: 0.432 (recovering from 0.427 with real sphere data)
- Phase: Cycling through 5-phase RALPH loop
- IPC: Connected and subscribed to field.* + sphere.*
- Sphere data: 66 spheres, r=0.925

### Quality Gate (Session 056 Final)

- `cargo check` — 0 errors, 0 warnings
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings
- `cargo test --lib --release --features full` — **1,599 tests**, 0 failures

### Cross-References

- Integration test findings: `~/projects/shared-context/tasks/findings-beta-tr-integration-test.md`
- Beta-left fix findings: `~/projects/shared-context/tasks/findings-beta-left-critical-fixes.md`
- Bug report: `~/projects/shared-context/ULTRAPLATE — Bugs and Known Issues — ORAC Update 2026-03-23.md`

---

## Session 055 Final — All 19 Fixes Complete (2026-03-22)

**Status:** ALL FIXES COMPLETE — 19/19 fixes deployed, 1,506 tests, 0 clippy warnings (pedantic), RALPH live, field poller active, 12 CC instances mapped, cross-service composite 0.764 (Grade A).

### All 19 Fixes

| Fix | Summary | Key Detail |
|-----|---------|------------|
| FIX-001 | `/field` GET endpoint | Proxies PV2 Kuramoto field state |
| FIX-002 | `/blackboard` GET endpoint | Session state + RALPH snapshot |
| FIX-003 | `/metrics` GET endpoint | Prometheus text format (5 metrics) |
| FIX-004 | Deephabitat gotcha #21 corrected | Hooks are live, not rolled back |
| FIX-005 | CLAUDE.md status SCAFFOLD→COMPLETE | Documentation accuracy |
| FIX-006 | Consent + ghosts plan | Superseded by FIX-018/019 implementation |
| FIX-007 | SYNTHEX thermal analysis | Fleet handoff (158 LOC report) |
| FIX-008 | POVM x evolution cross-correlation | Fleet handoff (130 LOC report) |
| FIX-009 | RALPH tick loop wired into main.rs | gen=0, fitness=0.528→0.667 |
| FIX-010 | Circuit breaker wired into hooks | `breaker_guarded_post` on 8 call sites (m11+m12) |
| FIX-011 | IPC client implemented | 827 LOC (was 288 stub), subscribe/send/recv |
| FIX-012 | field_state poller deployed | PV2 health polled every 5s, SharedState populated |
| FIX-013 | Semantic router wired into m12 | `classify_content` + `route` in task claiming |
| FIX-014 | Blackboard wired into PostToolUse | SQLite pane_status + task_history writes |
| FIX-015 | orac-client implemented | 399 LOC (was 29 stub), 7 subcommands |
| FIX-016 | parse_thermal default verified | Code correct at 0.5 (not 0.0), no fix needed |
| FIX-017 | Default features include all 6 | intelligence + monitoring + evolution added |
| FIX-018 | `/consent/{sphere_id}` GET/PUT | `OracConsent` on `OracState`, per-sphere control |
| FIX-019 | `/field/ghosts` GET endpoint | `OracGhost` tracking on deregistration |

### Runtime Systems Now Live

| System | Status | Detail |
|--------|--------|--------|
| RALPH evolution | LIVE | 30s tick interval, gen=0, fitness=0.667 |
| Field poller | LIVE | PV2 health every 5s → SharedState cache |
| Circuit breaker | WIRED | 5 services (pv2/synthex/me/povm/rm), 8 POST + 2 GET call sites |
| Semantic router | WIRED | Domain affinity (40%) + Hebbian (35%) + availability (25%) in task claim |
| Blackboard | WIRED | pane_status upsert on PostToolUse, task_history on claim/complete |
| Consent | WIRED | Per-sphere declarations, gates hydration + POVM writes |
| Ghost tracking | WIRED | Deregistration captures timing, phase, tool count |
| IPC client | IMPLEMENTED | V2 wire protocol FSM, subscribe/send/recv (ready for PV2 socket) |

### Fleet Topology (12 CC Instances)

```
Tab 1 Orchestrator:  orch-up, orch-right, orch-down
Tab 4 Fleet-ALPHA:   alpha-left, alpha-tr, alpha-br
Tab 5 Fleet-BETA:    beta-left, beta-tr, beta-br
Tab 6 Fleet-GAMMA:   gamma-left, gamma-tr, gamma-br
```

Relay chain: 4/4 legs verified (Orchestrator→ALPHA→BETA→GAMMA).

### Cross-Service Composite: 0.764 (Grade A)

| Dimension | Score |
|-----------|-------|
| Field coherence (r) | 0.91 |
| RALPH fitness | 0.61 |
| POVM coverage | 119 memories, 2,437 pathways |
| ME evolution | 172 mutations, 20,820 correlations |
| SYNTHEX thermal | 0.0 (metabolically inactive) |

### Fleet Star Graph Tool

5-generation RALPH product at `scripts/fleet-star.sh`. Burn-rate coloring, auto-delegation, context exhaustion warnings, ORAC/SYNTHEX/r integration.

### Quality Gate (Final)

- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings
- `cargo test --lib --release --features full` — **1,506 tests**, 0 failures

### Cross-References

- Fix list: `~/projects/shared-context/tasks/session-055-orac-fixes.md`
- Obsidian summary: `~/projects/shared-context/tasks/session-055-obsidian-summary.md`
- Field theory handoff: `~/projects/shared-context/tasks/handoff-gamma-tr-field-theory.md`
- FIX-017 analysis: `~/projects/shared-context/tasks/fix-017-default-features.md`
- RALPH star graph log: `~/projects/shared-context/tasks/ralph-star-graph-evolution.md`
- Relay chain: `~/projects/shared-context/tasks/handoff-1774172104-relay-chain.md`
- Deep architecture: auto-memory `orac-deep-architecture.md`

---

## Session 055 — Runtime Wiring + Fleet Operations (2026-03-22)

**Status:** Superseded by Session 055 Final above. Original 9/19 fixes expanded to 19/19 through fleet dispatch + orchestrator wiring.

### What Was Done (Session 055 — early phase)

1. **RALPH tick loop wired** — Evolution engine running live. Generation 0, fitness=0.667. 5-phase cycle (Recognize→Analyze→Learn→Propose→Harvest) executing against real field state. BUG-035 mono-parameter fix active (round-robin + diversity gate).
2. **field_state poller active** — ORAC now polls PV2 :8132/health on tick interval, caching r/K/spheres/tick into `AppState.field`. Feeds conductor decisions and dashboard.
3. **3 new endpoints deployed:**
   - `/traces` — OTel trace store query (recent spans, by-trace, by-pane, errors)
   - `/dashboard` — Kuramoto field dashboard snapshot (r history, clusters, gaps, chimera)
   - `/tokens` — Token accounting summary (fleet totals, per-pane, budget status)
4. **Default features expanded** — `default = ["api", "persistence", "bridges", "intelligence", "monitoring", "evolution"]` — all 6 features now build by default. No more `--features full` required.
5. **Fleet comms practice** — Full 4-leg relay chain (Orchestrator→ALPHA→BETA→GAMMA) verified. All 9 fleet instances (3 tabs × 3 panes) deployed and communicating via shared-context cascade files.
6. **Fleet-GAMMA audit** — 8 ORAC modules (M27-M35) audited: 6,544 LOC, 38 structs, 152 functions, 290 tests, 6 architectural TODOs (all deferred-by-design), 0 FIXMEs, 0 HACKs.
7. **Evolution DB deep dive** — evolution_tracking.db: 172 mutations (BUG-035 confirmed: 122/172 are no-ops on same parameter), fitness stagnant at 0.62. hebbian_pulse.db: 38 pathways at avg 0.895 strength, 6 patterns (all grade A).
8. **POVM x Evolution cross-correlation** — 120 POVM memories, 2,437 pathways (0% co-activated), 3 cross-system pathways found. Systems structurally aware but operationally disconnected.
9. **FIX-015 plan** — orac-client CLI implementation designed (7 subcommands, ~280 LOC, uses ureq). Plan at `shared-context/tasks/fix-015-orac-client-impl.md`.

---

## Session 054 — Phase 4 Evolution + Full Completion (2026-03-22)

**Status:** PLAN COMPLETE — 40/40 modules, 30,524 LOC, 1,454 tests, hooks migrated, all 14 critical path steps done.

### Step 9 — Hook Migration (2026-03-22)
11. **Hook forwarder created** — `hooks/orac-hook.sh` (generic stdin→curl→stdout bridge)
12. **6 hooks migrated to ORAC** — SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, PermissionRequest (NEW)
13. **3 bash scripts consolidated** — PostToolUse (post_tool_use.sh + post_tool_povm_pathway.sh + post_tool_nexus_pattern.sh → single ORAC endpoint)
14. **2 hooks kept as bash** — SubagentStop (no ORAC endpoint), PreCompact (cascade system)
15. **1 hook kept as bash** — Stop/check-cipher-messages.sh (non-PV2, cipher system)
16. **Backup** — `~/.claude/settings.json.pre-orac-backup`
17. **All 6 ORAC endpoints verified live** — SessionStart (POVM 111 mem, 2437 paths), UserPromptSubmit (r=0.9276, spheres=63), PreToolUse ({}), PostToolUse ({}), Stop ({}), PermissionRequest (auto-approved)
18. **Rollback command (if needed)** — `\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json`
19. **HOOKS LIVE** — settings.json has all 6 ORAC hooks wired via orac-hook.sh. Verified 2026-03-22.

### What Was Done (Session 054)
1. **m39_fitness_tensor** (1,317 LOC) — 12-dim weighted fitness evaluation with ORAC-specific dimensions (coordination_quality, field_coherence, dispatch_accuracy, etc.), trend detection via linear regression, stability/volatility assessment. 60 tests.
2. **m37_emergence_detector** (1,446 LOC) — 8 fleet emergence types (CoherenceLock, ChimeraFormation, CouplingRunaway, HebbianSaturation, DispatchLoop, ThermalSpike, BeneficialSync, ConsentCascade). Ring buffer with TTL decay, 5,000-event cap, monitor accumulation pattern. 41 tests.
3. **m38_correlation_engine** (976 LOC) — Temporal, causal, recurring, and fitness-linked correlation mining. Pathway discovery with establishment threshold, pattern key tracking, sliding window. 29 tests.
4. **m40_mutation_selector** (998 LOC) — BUG-035 fix: round-robin cycling, 10-generation cooldown, >50% diversity rejection gate. No mono-parameter monopoly. 34 tests.
5. **m36_ralph_engine** (1,117 LOC) — 5-phase RALPH orchestrator (Recognize→Analyze→Learn→Propose→Harvest) with snapshot/rollback, generation tracking, auto-pause at max cycles. 28 tests.
6. **m09_wire_protocol** (916 LOC) — V2 wire protocol state machine (Disconnected→Handshaking→Connected→Subscribing→Active), frame validation, send/recv queues, keepalive. 37 tests.
7. **m30_wasm_bridge** (729 LOC) — FIFO/ring protocol bridge: command parsing (dispatch/status/field_state/list_panes/ping), EventRingBuffer (1,000 line cap, FIFO eviction), JSONL serialization. 34 tests.
8. **Quality gate 4/4 clean** — check 0, clippy 0, pedantic 0, 1,454 tests 0 failures
9. **Release build** — 3 binaries deployed: orac-sidecar (5.5MB), orac-probe (2.3MB), orac-client (337KB)
10. **All stubs filled** — 40/40 modules implemented, zero scaffolds remaining

### Test Results
- **1,454 tests** (--features full) — 0 failures, 0 ignored
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings

### Per-Layer Summary

| Layer | Dir | Modules | LOC | Tests |
|-------|-----|---------|-----|-------|
| L1 Core | `m1_core` | m01-m06 + field_state | 4,020 | 193 |
| L2 Wire | `m2_wire` | m07-m09 | 2,300 | 111 |
| L3 Hooks | `m3_hooks` | m10-m14 | 2,405 | 138 |
| L4 Intelligence | `m4_intelligence` | m15-m21 | 4,402 | 229 |
| L5 Bridges | `m5_bridges` | m22-m26 | 4,618 | 244 |
| L6 Coordination | `m6_coordination` | m27-m31 | 2,578 | 119 |
| L7 Monitoring | `m7_monitoring` | m32-m35 | 4,347 | 230 |
| L8 Evolution | `m8_evolution` | m36-m40 | 5,854 | 192 |
| **TOTAL** | | **40** | **30,524** | **1,454** |

---

## Session 053 — Phase 2 Intelligence Layer (2026-03-22)

**Status:** PHASE 2 COMPLETE — 3 new modules, 2,593 LOC, quality gate 4/4 clean, 972 tests.

### What Was Done (Session 053)
1. **m20_semantic_router** (803 LOC) — Content-aware dispatch using Hebbian weights + domain affinity. 4 semantic domains (Read/Write/Execute/Communicate) mapped to Kuramoto phase regions. Tool classifier, content classifier, weighted composite scoring (domain 40% + Hebbian 35% + availability 25%), preferred pane bonus. 45 tests.
2. **m21_circuit_breaker** (870 LOC) — Per-pane health gating with Closed/Open/HalfOpen FSM. Configurable failure/success thresholds, tick-based Open→HalfOpen timeout, probe request limiting. `BreakerRegistry` for fleet-wide management with `tick_all()`, `state_counts()`, independent per-pane tracking. 38 tests.
3. **m26_blackboard** (920 LOC) — SQLite shared fleet state via rusqlite. 3 tables: `pane_status` (upsert/get/list/remove), `task_history` (insert/recent/count), `agent_cards` (A2A-inspired capability declarations). Indexed by pane_id and finished_at. In-memory mode for tests. 35 SQLite tests + 5 data type tests.
4. **Quality gate 4/4 clean** — check 0, clippy 0, pedantic 0, 972 tests 0 failures
5. **Feature gates** — `intelligence` (m20, m21), `persistence` (m26) — both already in Cargo.toml

### Test Results
- **972 tests** (--features full) — 0 failures, 0 ignored
- **734 tests** (default features) — 0 failures
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings

---

## Session 052 — Phase 1 Hooks Deployed (2026-03-22)

**Status:** PHASE 1 COMPLETE — HTTP hook server live on :8133, 17/17 services healthy.

### What Was Done (Session 052)
1. **5 hook modules implemented** (m10-m14) — 2,405 LOC, quality gate 4/4 clean
2. **m10_hook_server** (735 LOC) — Axum router, `OracState`, `HookEvent`/`HookResponse` types, HTTP helpers, health endpoint
3. **m11_session_hooks** (398 LOC) — `SessionStart` (register+hydrate from POVM+RM), `Stop` (fail tasks, crystallize, deregister)
4. **m12_tool_hooks** (559 LOC) — `PostToolUse` (memory+status, 1-in-5 task poll, atomic claim), `PreToolUse` (SYNTHEX thermal gate)
5. **m13_prompt_hooks** (351 LOC) — `UserPromptSubmit` (inject r/tick/spheres/thermal + pending tasks)
6. **m14_permission_policy** (362 LOC) — `PermissionRequest` auto-approve/deny engine (read=allow, write=notice, deny list)
7. **main.rs wired** — Feature-gated `api` starts Axum, graceful shutdown on SIGINT
8. **Binary deployed** — `~/.local/bin/orac-sidecar` (4.7MB), daemon running
9. **Integration tested** — All 6 endpoints verified live: POVM hydration (110 mem, 2437 paths), field state (r=0.993), thermal check
10. **Git committed + pushed** — `903fdd2` on main, pushed to GitLab
11. **RM recorded** — `r69bf788f008a` deployment entry

### Test Results
- **699 tests** — 0 failures, 0 ignored
- `cargo check` — 0 errors
- `cargo clippy -D warnings` — 0 warnings
- `cargo clippy -W pedantic` — 0 warnings
- **Live integration** — 9/9 endpoint tests pass

---

## Session 050 — Plan Complete (2026-03-22)

**Status:** SCAFFOLD-READY — All pre-scaffold tasks complete. `plan.toml` created. Awaiting deploy order.

### What Was Done (Session 050)
1. **V2 binary deployed** — PV2 daemon live on :8132, 1,527 tests, governance routes active (200)
2. **Hebbian wired** — BUG-031 fix verified, coupling weights differentiated 0.09–0.60
3. **ME deadlock addressed** — BUG-035 pruned (25K emergences → 1K), evolution chamber breathing
4. **ORAC_PLAN.md** — Full architecture: 4 phases, ~24,500 LOC, 33-feature backlog
5. **ORAC_MINDMAP.md** — 19 branches, 148+ leaves, 127 Obsidian notes, 16 recommended new notes
6. **Rust Gold Standard** documented — 10 constraints, 9 pattern categories, 17 anti-patterns from ME V2 L1+L2
7. **CLAUDE.md + CLAUDE.local.md** — project context files
8. **candidate-modules/** — 24 files (15,936 lines) cloned from PV2, refactored to gold standard, staged for scaffold integration. 42 violations found and fixed. Scaffold integration protocol documented in ORAC_PLAN.md.
9. **plan.toml** — 8 layers, 40 modules, 3 bin targets (orac-sidecar, orac-client, orac-probe), 7 features, consent config, quality gate, server/IPC/bridge/evolution config sections
10. **Git initialized** — commit `2d40fdc` with all planning artifacts + candidate modules
11. **scripts/test-hook-server.py** — Minimal HTTP hook format test server for Phase 1 de-risking
12. **Obsidian note** — `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` documenting BUG-035 lesson + diversity-enforced selection design
13. **ORAC_PLAN.md updated** — Phase 3 devenv prerequisite + Phase 4 mono-parameter mutation warning

### Critical Path Status
```
✅ Step 1: Deploy V2 binary (PV2 healthy, governance 200, k_modulation 1.21)
✅ Step 2: Verify Hebbian wired (coupling range 0.09–0.60, BUG-031 committed)
✅ Step 3: Fix ME deadlock (DB pruned, min_confidence 0.5, 57 mutations in 11 RALPH cycles)
✅ Step 3b: Git initialized (commit 2d40fdc, 28 files)
✅ Step 3c: plan.toml created (8 layers, 40 modules, 3 bin targets, 7 features)
✅ Step 3d: HTTP hook test server staged (scripts/test-hook-server.py)
✅ Step 3e: RALPH mutation fix documented (Obsidian + ORAC_PLAN.md Phase 4 warning)
✅ Step 3f: Phase 3 prerequisite documented (devenv start before bridges)
✅ Step 4: Scaffold ORAC (scaffold-gen ran, 53 files, 8 layers)
✅ Step 5: Phase 1 HTTP hooks (5 modules, 2,405 LOC, 699 tests, quality gate 4/4 clean)
✅ Step 6: Deploy binary + test against live PV2 (17/17 services, all 6 endpoints verified)
✅ Step 7: Git committed + pushed (903fdd2 + 4bf9335, GitLab main)
✅ Step 8: Phase 2 — Intelligence (m20 semantic router, m21 circuit breaker, m26 blackboard)
✅ Step 9: Migrate settings.json hooks from bash to HTTP (6 hooks → ORAC, SubagentStop+PreCompact kept as bash)
✅ Step 10: Phase 3 — Bridges + monitoring (m22-m26 bridges, m32-m35 monitoring, 8,965 LOC, 474 tests)
✅ Step 11: Phase 4 — Evolution (m36-m40 RALPH, 5,854 LOC, 192 tests, BUG-035 fixed)
✅ Step 12: Fill remaining stubs (m09 wire protocol 916 LOC, m30 WASM bridge 729 LOC)
✅ Step 13: Full quality gate (1,454 tests, 0 failures, 0 clippy warnings)
✅ Step 14: Release build (orac-sidecar 5.5MB, orac-probe 2.3MB, orac-client 337KB)
```

---

## BOOTSTRAP PROTOCOL (New Context Window)

**MANDATORY — execute these steps at the start of EVERY new context window:**

1. **Run `/primehabitat`** — loads The Habitat: Zellij tabs, 16 services, IPC bus, memory systems
2. **Run `/deephabitat`** — loads deep substrate: wire protocol, databases, ecosystem, tools
3. **Read this file** (`CLAUDE.local.md`) — current state, phase tracking
4. **Read `ORAC_PLAN.md`** — full architecture and build phases
5. **Read `ORAC_MINDMAP.md`** — Obsidian cross-references and Rust gold standard

**After bootstrap, WAIT for Luke to give a specific task before taking action.**

Bootstrap gives you god-tier understanding. All 4 build phases and all 19 runtime fixes are complete. The system is production-ready.

---

## What Is ORAC

ORAC is an Envoy-like proxy specialized for AI agent traffic — replacing the V1 swarm-sidecar (546 LOC, non-functional for 17+ hours due to V1/V2 wire mismatch). It fills 10 gaps that bash hooks cannot: real-time push notifications, bidirectional event streaming, persistent socket multiplexing, sub-second coordination, cross-pane awareness, high-frequency STDP, persistent fleet state, WASM plugin bridge, closed-loop thermal damping, and HTTP hook server replacing all 8 bash scripts.

**Validated by:** arxiv 2508.12314 (Kuramoto oscillators for AI agent coordination — we're ahead of academia).

---

## Git Repository

**Remote:** `git@gitlab.com:lukeomahoney/orac-sidecar.git`
**URL:** `https://gitlab.com/lukeomahoney/orac-sidecar`
**Branch:** `main`
**Commits:** 6 (latest: `4bf9335` Phase 1 hooks + Session 052 record)

---

## Next Steps: Post-Fix Hardening

All 19 fixes complete. All 40 modules wired. Focus shifts to **production hardening** and **metabolic activation**.

**Priority order:**
1. **DevEnv registration** — Register orac-sidecar as service #18 in devenv.toml (port 8133, batch 5)
2. **IPC client live connect** — Connect m07 to PV2 Unix socket for event-driven coordination (currently HTTP poll)
3. **SYNTHEX thermal activation** — Investigate why thermal=0.0 (all heat sources dormant)
4. **RALPH generation advancement** — Run coupling steps to generate fitness improvement proposals
5. **Release build + deploy** — Fresh `cargo build --release --features full`, deploy to `~/.local/bin/`

```bash
# Verify ORAC is still running
curl -s http://localhost:8133/health | python3 -c "import sys,json;d=json.load(sys.stdin);print(f'ORAC {d[\"status\"]} port={d[\"port\"]} sessions={d[\"sessions\"]} ticks={d[\"uptime_ticks\"]}')"

# Quality gate
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```

### Architecture (8 Layers, 40 Modules, 3 Binaries)

```
L1 Core        (m01-m06)  — Types, errors, config, constants, traits, validation
L2 Wire        (m07-m09)  — IPC client, bus types, wire protocol
L3 Hooks       (m10-m14)  — HTTP hook server, session/tool/prompt hooks, permission policy
L4 Intelligence (m15-m21) — Coupling, auto-K, topology, Hebbian, buoy, semantic router, circuit breaker
L5 Bridges     (m22-m26)  — SYNTHEX, ME, POVM, RM bridges, blackboard
L6 Coordination (m27-m31) — Conductor, cascade, tick, WASM bridge, memory manager
L7 Monitoring  (m32-m35)  — OTel traces, metrics, field dashboard, token accounting
L8 Evolution   (m36-m40)  — RALPH engine, emergence, correlation, fitness tensor, mutation selector
```

**Bin targets:** `orac-sidecar` (daemon), `orac-client` (CLI), `orac-probe` (diagnostics)
**Features:** `api`, `persistence`, `bridges`, `intelligence`, `monitoring`, `evolution`, `full`

### Candidate Modules (pre-refactored, staged)

```
candidate-modules/
├── drop-in/ (18 files, 10,516 lines — copy as-is into scaffolded src/)
│   ├── L1-foundation/  → src/m1_core/
│   ├── L2-wire/        → src/m2_wire/
│   ├── L4-coupling/    → src/m4_intelligence/
│   ├── L4-learning/    → src/m4_intelligence/
│   └── L6-cascade/     → src/m6_coordination/
└── adapt/ (6 files, 5,420 lines — need ORAC-specific changes marked with ## ADAPT headers)
    ├── L5-synthex/     → src/m5_bridges/
    ├── L5-me/          → src/m5_bridges/
    ├── L5-povm/        → src/m5_bridges/
    ├── L5-rm/          → src/m5_bridges/
    ├── L6-conductor/   → src/m6_coordination/
    └── L6-tick/        → src/m6_coordination/
```

### Key Services (must be running for relevant phases)

| Service | Port | Health | Needed For |
|---------|------|--------|------------|
| PV2 | 8132 | `/health` | Phase 1+ (IPC bus) |
| SYNTHEX | 8090 | `/api/health` | Phase 3 (bridge) |
| ME | 8080 | `/api/health` | Phase 3 (bridge) |
| POVM | 8125 | `/health` | Phase 3 (bridge) |
| RM | 8130 | `/health` | Phase 3 (bridge) |

Start all: `~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start`

---

## Traps to Avoid

1. **Never chain after `pkill`** (exit 144 kills the `&&` chain)
2. **Always `\cp -f`** (cp aliased to interactive — BUG-027)
3. **TSV only for Reasoning Memory** (JSON causes parse failure)
4. **Lock ordering: AppState before BusState** (deadlock prevention)
5. **Phase wrapping: `.rem_euclid(TAU)`** after all phase arithmetic
6. **No stdout in daemons** (SIGPIPE → death, BUG-018)
7. **Don't script Zellij plugin interactions** (zombie behaviour — keybind-only)
8. **fleet-ctl cache is STALE** (300s TTL — `dump-screen` is the only reliable pane state)
9. **BUG-035 mono-parameter trap** — evolution chamber MUST use multi-parameter mutation selection
10. **Bridge URLs must NOT include `http://` prefix** (BUG-033 — raw SocketAddr only)
11. **`#[derive(Default)]` on ProposalManager** → `max_active=0` (BUG-032 — use custom `impl Default`)
12. **POVM is write-only** (BUG-034 — must call `/hydrate` to read back state)

---

## Hot-Swap File Map

When scaffolding, these PV2 modules will be copied and adapted:

| ORAC Layer | PV2 Source | Files | Action |
|------------|-----------|-------|--------|
| L1 Core | `m1_foundation/m01-m06` | 6 files | DROP-IN |
| L2 Wire | `m7_coordination/m29,m30` | 2 files | DROP-IN |
| L4 Intelligence | `m4_coupling/m16-m18` | 3 files | DROP-IN |
| L4 Intelligence | `m5_learning/m19-m21` | 3 files | DROP-IN |
| L5 Bridges | `m6_bridges/m22,m24-m26` | 4 files | ADAPT |
| L6 Coordination | `m7_coordination/m31,m33,m35` | 3 files | ADAPT |

**Source:** `/home/louranicas/claude-code-workspace/pane-vortex-v2/src/`

---

## DevEnv Integration

When ORAC is ready for devenv registration:

```toml
# In ~/.config/devenv/devenv.toml
[services.orac-sidecar]
name = "ORAC Sidecar"
command = "./bin/orac-sidecar"
working_dir = "/home/louranicas/claude-code-workspace/orac-sidecar"
port = 8133
health_path = "/health"
batch = 5
depends_on = ["pane-vortex", "povm-engine"]
description = "Intelligent fleet coordination proxy — HTTP hooks, Hebbian STDP, RALPH evolution"
```

## Hook Migration — COMPLETE (2026-03-22)

Hooks migrated from PV2 bash scripts to ORAC HTTP endpoints via `hooks/orac-hook.sh` forwarder.

**Forwarder:** `orac-sidecar/hooks/orac-hook.sh <EventName> [timeout]` — reads stdin, POSTs to ORAC, outputs response.

| Event | Before (bash) | After (ORAC) | Timeout |
|-------|---------------|--------------|---------|
| SessionStart | session_start.sh | orac-hook.sh SessionStart | 5s |
| UserPromptSubmit | user_prompt_field_inject.sh | orac-hook.sh UserPromptSubmit | 3s |
| PreToolUse | pre_tool_thermal_gate.sh | orac-hook.sh PreToolUse | 2s |
| PostToolUse | 3 scripts (tool+povm+nexus) | orac-hook.sh PostToolUse | 3s |
| Stop | session_end.sh | orac-hook.sh Stop | 5s |
| PermissionRequest | (none) | orac-hook.sh PermissionRequest | 2s |
| SubagentStop | subagent_field_aggregate.sh | **KEPT** (no ORAC endpoint) | 5s |
| PreCompact | handoff-dispatch.sh | **KEPT** (cascade system) | 30s |
| Stop (cipher) | check-cipher-messages.sh | **KEPT** (non-PV2) | — |

**Rollback:** `\cp -f ~/.claude/settings.json.pre-orac-backup ~/.claude/settings.json`

---

## Quality Gate

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```

## Working Directory
`/home/louranicas/claude-code-workspace/orac-sidecar`
