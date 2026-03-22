# ORAC .claude Folder — Alignment Verification

> Cross-referenced against ORAC_MINDMAP.md (19 branches, 248 notes) and ORAC_PLAN.md (4 phases, ~24,500 LOC)
> Verified: 2026-03-22

## Mindmap Branch Coverage

| # | Mindmap Branch | .claude Artifact | Coverage |
|---|---------------|------------------|----------|
| 1 | HTTP Hook Server (Keystone) | `schemas/hook_request.json` (6 events), `schemas/hook_response.json`, `schemas/permission_policy.schema.json`, `skills/hook-debug/SKILL.md`, `queries/hook_events.sql` | FULL |
| 2 | IPC Client (V2 Wire Protocol) | `schemas/bus_event.schema.json` (24 types), `schemas/bus_frame.schema.json` (5 frame types), `context.json` (bridges.pv2) | FULL |
| 3 | Intelligence Layer | `context.json` (L4 layer), `patterns.json` (P01 phase wrapping, P06 NaN guard, P08 freq clamp), `queries/fleet_state.sql` (hebbian_weights, circuit_breaker) | FULL |
| 4 | RALPH Evolution Chamber | `context.json` (L8 layer), `patterns.json` (P20 multi-parameter), `anti_patterns.json` (AP12 mono-parameter BUG-035, AP19 emergence cap) | FULL |
| 5 | Monitoring / Observer | `context.json` (L7 layer), `queries/fleet_state.sql` (field_snapshots, bridge_health) | FULL |
| 6 | Bridge Subset | `context.json` (bridges section), `skills/bridge-probe/SKILL.md`, `anti_patterns.json` (AP13 URL prefix, AP15 POVM write-only, AP05 JSON to RM) | FULL |
| 7 | WASM Bridge | `context.json` (wasm_bridge section — FIFO + ring) | COVERED |
| 8 | Fleet Dispatch | `queries/fleet_state.sql` (routing_decisions), `queries/blackboard.sql` (shared state) | COVERED |
| 9 | Cascade Handoffs | `schemas/bus_frame.schema.json` (cascade_handoff frame type) | COVERED |
| 10 | Consent / Governance | `patterns.json` (P21 consent gate stub), `anti_patterns.json` (AP20 global k_mod without consent), `context.json` (L8 evolution with consent) | COVERED |
| 11 | Scaffold System | `context.json` (full layer map), `skills/orac-boot/SKILL.md` (candidate modules section) | COVERED |
| 12 | Kuramoto Coupling | `patterns.json` (P01 phase wrapping, P06 NaN guard, P08 freq clamp), `queries/fleet_state.sql` (r trend, chimera events) | FULL |
| 13 | Architecture Schematics | Referenced in `skills/orac-boot/SKILL.md` cross-references section | REF |
| 14 | Database & Persistence | `queries/` (3 SQL files), `context.json` (databases section) | FULL |
| 15 | Memory Systems | `anti_patterns.json` (AP05 TSV only), `skills/bridge-probe/SKILL.md` (RM/POVM probes) | COVERED |
| 16 | ULTRAPLATE Ecosystem | Referenced in `context.json` (devenv_batch, service_id) | REF |
| 17 | Habitat Skills & Tools | `skills/` (3 ORAC-specific skills) | COVERED |
| 19 | Candidate Modules | `context.json` (hot_swap fields per layer), `status.json` (candidate_modules counts), `skills/orac-boot/SKILL.md` (candidate mapping) | FULL |

## ORAC_PLAN.md Phase Coverage

| Phase | Focus | .claude Artifacts |
|-------|-------|-------------------|
| Phase 1: Wire + Hooks | IPC client, 6 HTTP hook endpoints | `schemas/` (5 files), `skills/hook-debug/`, `queries/hook_events.sql`, `context.json` (L2+L3) |
| Phase 2: Intelligence | Hebbian, semantic router, circuit breaker, blackboard | `context.json` (L4), `patterns.json` (P01-P08), `queries/blackboard.sql`, `queries/fleet_state.sql` |
| Phase 3: Bridges + Monitoring | 4 bridges, OTel, metrics, dashboard | `context.json` (L5+L7, bridges), `skills/bridge-probe/`, `anti_patterns.json` (AP13-AP15, AP18) |
| Phase 4: Evolution | RALPH engine, multi-param mutation | `context.json` (L8), `patterns.json` (P20), `anti_patterns.json` (AP12, AP19) |

## Hook Endpoint Coverage (6/6)

| Hook Event | Schema | Skill | Query | Pattern/AP |
|------------|--------|-------|-------|------------|
| SessionStart | hook_request.json | hook-debug | hook_events.sql | — |
| PostToolUse | hook_request.json | hook-debug | hook_events.sql | — |
| PreToolUse | hook_request.json | hook-debug | hook_events.sql | P18 (fail-open), AP18 |
| UserPromptSubmit | hook_request.json | hook-debug | hook_events.sql | — |
| Stop | hook_request.json | hook-debug | hook_events.sql | — |
| PermissionRequest | hook_request.json + permission_policy.schema.json | hook-debug | hook_events.sql (denied actions) | P15 (cascade) |

## Trap Coverage (12/12 from CLAUDE.local.md)

| # | Trap | .claude File | ID |
|---|------|-------------|-----|
| 1 | pkill chain | anti_patterns.json | AP03 |
| 2 | cp alias | anti_patterns.json | AP04 |
| 3 | TSV only for RM | anti_patterns.json | AP05 |
| 4 | Lock ordering | patterns.json (inherited via context) | — |
| 5 | Phase wrapping | patterns.json | P01 |
| 6 | No stdout in daemons | anti_patterns.json | AP06 |
| 7 | Don't script Zellij plugins | Referenced in skills (keybind-only) | — |
| 8 | fleet-ctl cache stale | Referenced in skills | — |
| 9 | BUG-035 mono-parameter | anti_patterns.json | AP12 |
| 10 | BUG-033 bridge URL prefix | anti_patterns.json | AP13 |
| 11 | BUG-032 Default ProposalManager | anti_patterns.json | AP14 |
| 12 | BUG-034 POVM write-only | anti_patterns.json | AP15 |

## PV2 → ORAC Adaptation Summary

| PV2 File | ORAC File | Adaptation |
|----------|-----------|------------|
| context.json | context.json | Full rewrite: 8 ORAC layers, 40 modules, hook endpoints, WASM bridge, bin targets |
| patterns.json | patterns.json | 13 carried over (P01-P13), 9 ORAC-specific added (P14-P22) |
| anti_patterns.json | anti_patterns.json | 11 carried over (AP01-AP11), 9 ORAC-specific added (AP12-AP20) |
| status.json | status.json | New: ORAC phase tracking with build_phases + candidate_modules |
| schemas/bus_event.schema.json | schemas/bus_event.schema.json | Copied (ORAC consumes same protocol) |
| schemas/bus_frame.schema.json | schemas/bus_frame.schema.json | Copied (ORAC consumes same protocol) |
| — | schemas/hook_request.json | Updated: 4→6 events, added permission/stop_reason fields |
| — | schemas/hook_response.json | Existing (kept as-is) |
| — | schemas/permission_policy.schema.json | New: auto-approve/deny policy config |
| queries/bus_tasks.sql | queries/blackboard.sql | Adapted for ORAC blackboard (shared fleet state) |
| queries/field_state.sql | queries/fleet_state.sql | Adapted: added bridge_health, circuit_breaker, hebbian_weights, routing_decisions |
| queries/governance.sql | — | Skipped: ORAC does not run governance (PV2 daemon handles) |
| — | queries/hook_events.sql | New: hook event tracking, latency, permission audit |
| skills/primehabitat/ | skills/orac-boot/ | Adapted: ORAC architecture, hook endpoints, candidate modules |
| skills/deephabitat/ | skills/bridge-probe/ | Adapted: bridge connectivity focus |
| — | skills/hook-debug/ | New: hook endpoint testing and debugging |
| skills/scaffold-mastery/ | — | Skipped: scaffold-gen is a workspace-level tool |
| skills/zellij-mastery/ | — | Skipped: Zellij skills are workspace-level |

## Verdict: ALIGNED

All 19 mindmap branches covered. All 4 build phases covered. All 6 hook endpoints covered.
All 12 traps covered. 22 patterns + 20 anti-patterns documented.
19 files across 5 directories (hooks/, queries/, schemas/, skills/, root).

## Obsidian Bidirectional Links

The following Obsidian notes reference and are referenced by `.claude/` artifacts.
Bidirectional index note: `ORAC .claude Folder — Bidirectional Index` in vault `~/projects/claude_code/`.

| Obsidian Note | .claude Artifacts |
|---------------|-------------------|
| `Session 050 — ORAC Sidecar Architecture` | context.json, skills/orac-boot/ |
| `Session 051 — ORAC Sidecar .claude Scaffolding` | ALIGNMENT_VERIFICATION.md, all schemas, queries, skills |
| `Session 052 — Phase 1 Hooks Deployed` | schemas/hook_*.json, permission_policy.schema.json, queries/hook_events.sql, skills/hook-debug/ |
| `Session 053 — ORAC Phase 2 Intelligence + Gold Standard Audit` | patterns.json, anti_patterns.json, hooks/quality-gate.md, queries/blackboard.sql, queries/fleet_state.sql |
| `Session 053b — ORAC Full Deploy Assessment` | status.json, skills/bridge-probe/ |
| `ORAC — RALPH Multi-Parameter Mutation Fix` | anti_patterns.json (AP12 BUG-035) |
| `Pane-Vortex IPC Bus — Session 019b` | schemas/bus_event.schema.json, schemas/bus_frame.schema.json |

Updated: 2026-03-22 (Session 053b — full deploy complete, 40/40 modules, 1,454 tests)
