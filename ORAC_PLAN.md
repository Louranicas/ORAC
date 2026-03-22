# ORAC Sidecar — Architecture Plan

> **From dumb pipe to intelligent fleet coordination proxy**
> **Port:** 8133 | **Binary:** `orac-sidecar` | **DevEnv Batch:** 5 (needs PV2 + POVM)
> **Source:** Envoy-like proxy specialized for AI agent traffic
> **Validated:** arxiv 2508.12314 (Kuramoto oscillators for AI agent coordination)

---

## Context

The V1 swarm-sidecar (546 LOC, 822K binary) was a simple FIFO→socket→ring file bridge. It ran for 17 hours in Session 049 and contributed nothing — V1/V2 wire mismatch meant it connected but never processed events. Meanwhile, 8 bash hook scripts (423 LOC) replaced 100% of the sidecar's pull-based coordination functions.

**The question:** What should the V2 sidecar be?

**The answer:** An Envoy-like proxy specialized for AI agent traffic, with an evolution chamber, advanced monitoring, and HTTP hook server.

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  ORAC SIDECAR  (port 8133)                              │
│                                                          │
│  ┌─ HTTP Hook Server ──────────────────────────────┐    │
│  │  /hooks/SessionStart      → register sphere      │    │
│  │  /hooks/PostToolUse       → Hebbian + task poll  │    │
│  │  /hooks/PermissionRequest → auto-approve policy  │    │
│  │  /hooks/Stop              → quality gate + dereg │    │
│  │  /hooks/UserPromptSubmit  → inject field state   │    │
│  │  /hooks/PreToolUse        → thermal gate         │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─ IPC Client (M29/M30) ──────────────────────────┐    │
│  │  Unix socket → PV2 daemon (push events)          │    │
│  │  V2 wire protocol, subscribe field.* task.*      │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─ Intelligence Layer ────────────────────────────┐    │
│  │  Hebbian STDP (M19-M21)   — tool co-activation   │    │
│  │  Semantic Router           — content-aware dispatch│   │
│  │  Circuit Breaker           — per-pane health      │    │
│  │  Blackboard (SQLite)       — shared fleet state   │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─ RALPH Evolution Chamber ───────────────────────┐    │
│  │  5-phase: Recognize→Analyze→Learn→Propose→Harvest│   │
│  │  Emergence detector (ring buffer, TTL decay)      │    │
│  │  Correlation engine (pathway discovery)           │    │
│  │  Fitness tensor (12-dim, weighted)                │    │
│  │  Snapshot + rollback (atomic state capture)       │    │
│  │  Feature-gated: #[cfg(feature = "evolution")]     │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─ Monitoring / Observer ─────────────────────────┐    │
│  │  OTel traces (task lifecycle across panes)        │    │
│  │  Per-agent metrics (tokens, latency, error rate)  │    │
│  │  Kuramoto field dashboard (r, phase wheel, K)     │    │
│  │  Token accounting (per-task cost tracking)        │    │
│  │  Fleet health aggregation                         │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─ Bridge Subset ─────────────────────────────────┐    │
│  │  SYNTHEX (thermal read + Hebbian writeback)       │    │
│  │  POVM (memory hydration, crystallisation)         │    │
│  │  RM (cross-session TSV persistence)               │    │
│  │  ME (fitness read, evolution correlation)         │    │
│  └──────────────────────────────────────────────────┘    │
│                                                          │
│  ┌─ WASM Bridge (existing) ────────────────────────┐    │
│  │  FIFO reader (/tmp/swarm-commands.pipe)           │    │
│  │  Ring writer (/tmp/swarm-events.jsonl)            │    │
│  └──────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
         │              │              │
    PV2 Daemon     Fleet Agents    WASM Plugin
    (Unix sock)    (HTTP hooks)    (FIFO/ring)
```

---

## Hot-Swap Module Map (from PV2)

### DROP-IN (use as-is) — 10,170 LOC
| Module | Source | LOC | Role |
|--------|--------|-----|------|
| M01-M06 | L1 Foundation | 3,373 | Types, errors, config, constants, traits, validation |
| M16-M18 | L4 Coupling | 1,724 | Coupling network, auto-K, topology |
| M19-M21 | L5 Learning | 1,424 | Hebbian STDP, buoy network, memory manager |
| M29+M30 | L7 Coordination | 2,938 | IPC bus + bus types (V2 wire protocol) |
| M33 | L7 Coordination | 711 | Cascade protocol |

### ADAPT (light changes) — 5,302 LOC
| Module | Source | LOC | Changes Needed |
|--------|--------|-----|----------------|
| M22 | SYNTHEX bridge | 908 | Thermal + Hebbian writeback |
| M24 | ME bridge | 814 | Fitness signal read |
| M25 | POVM bridge | 924 | Memory hydration + crystallisation |
| M26 | RM bridge | 956 | TSV persistence (NOT JSON) |
| M31 | Conductor | 820 | Breathing rhythm |
| M35 | Tick orchestrator | 880 | Tick loop adaptation |

### NEW — ~9,000 LOC
| Component | LOC Est | Notes |
|-----------|---------|-------|
| HTTP Hook Server (Axum) | ~1,500 | 6 hook endpoints, sub-ms response |
| RALPH Evolution Chamber | ~6,000 | Cloned from ME, multi-param mutation fix |
| Blackboard (SQLite) | ~600 | Shared fleet knowledge store |
| Semantic Router | ~400 | Content-aware dispatch |
| Permission Policy | ~500 | Auto-approve/deny fleet-wide + per-sphere |

### SKIP
| Module | Reason |
|--------|--------|
| M10 API server | Sidecar has own Axum |
| M28 Consent gate | Daemon enforces — redundant downstream |

### CANDIDATE MODULES (Pre-Refactored, Ready for Scaffold Integration)

All hot-swap modules have been cloned from PV2, refactored to gold standard, and staged at:
**`/home/louranicas/claude-code-workspace/orac-sidecar/candidate-modules/`**

**15,936 lines across 24 files — zero critical violations, all docs complete.**

```
candidate-modules/
├── drop-in/                          (10,516 lines, 18 files — use as-is)
│   ├── L1-foundation/                m01_core_types.rs, m02_error_handling.rs,
│   │                                 m03_config.rs, m04_constants.rs, m05_traits.rs,
│   │                                 m06_validation.rs, mod.rs
│   ├── L2-wire/                      m29_ipc_bus.rs, m30_bus_types.rs
│   ├── L4-coupling/                  m16_coupling_network.rs, m17_auto_k.rs,
│   │                                 m18_topology.rs, mod.rs
│   ├── L4-learning/                  m19_hebbian_stdp.rs, m20_buoy_network.rs,
│   │                                 m21_memory_manager.rs, mod.rs
│   └── L6-cascade/                   m33_cascade.rs
│
└── adapt/                            (5,420 lines, 6 files — need ORAC-specific changes)
    ├── L5-synthex/                   m22_synthex_bridge.rs  (port 8090, ## ADAPT header)
    ├── L5-me/                        m24_me_bridge.rs       (port 8080, ## ADAPT header)
    ├── L5-povm/                      m25_povm_bridge.rs     (port 8125, ## ADAPT header)
    ├── L5-rm/                        m26_rm_bridge.rs       (port 8130, ## ADAPT header)
    ├── L6-conductor/                 m31_conductor.rs       (## ADAPT header)
    └── L6-tick/                      m35_tick.rs            (## ADAPT header)
```

#### Scaffold Integration Protocol

When `scaffold-gen --from-plan plan.toml` creates the ORAC project structure:

1. **After scaffold**: Copy drop-in files into their target layer directories:
   ```bash
   \cp -f candidate-modules/drop-in/L1-foundation/*.rs src/m1_core/
   \cp -f candidate-modules/drop-in/L2-wire/*.rs       src/m2_wire/
   \cp -f candidate-modules/drop-in/L4-coupling/*.rs   src/m4_intelligence/
   \cp -f candidate-modules/drop-in/L4-learning/*.rs   src/m4_intelligence/
   \cp -f candidate-modules/drop-in/L6-cascade/*.rs    src/m6_coordination/
   ```

2. **Rename modules** to match ORAC layer numbering (m16→ renumber if needed)

3. **Update mod.rs** files to declare the copied modules

4. **Copy adapt files** into their target directories:
   ```bash
   \cp -f candidate-modules/adapt/L5-synthex/*.rs  src/m5_bridges/
   \cp -f candidate-modules/adapt/L5-me/*.rs       src/m5_bridges/
   \cp -f candidate-modules/adapt/L5-povm/*.rs     src/m5_bridges/
   \cp -f candidate-modules/adapt/L5-rm/*.rs       src/m5_bridges/
   \cp -f candidate-modules/adapt/L6-conductor/*.rs src/m6_coordination/
   \cp -f candidate-modules/adapt/L6-tick/*.rs      src/m6_coordination/
   ```

5. **Apply ADAPT changes** — each adapt file has a `## ADAPT for ORAC` header documenting specific changes needed (ports, socket addresses, poll intervals, consent bypass)

6. **Update `crate::` imports** — change `m1_foundation` → `m1_core`, `m7_coordination` → `m2_wire`, etc. to match ORAC layer structure

7. **Run quality gate** after each layer integration:
   ```bash
   CARGO_TARGET_DIR=/tmp/cargo-orac cargo check && cargo clippy -- -D warnings && \
   cargo clippy -- -D warnings -W clippy::pedantic && cargo test --lib --release
   ```

#### Gold Standard Compliance (Post-Refactor)
- Zero `unwrap()`/`expect()` outside `#[cfg(test)]`
- Zero `println!`/`eprintln!` in production code
- Zero `unsafe` blocks
- All public items have `///` documentation with backticked identifiers
- All fallible public functions have `# Errors` sections
- Import ordering: `std` → external → `crate::`
- FMA used for all multi-step float arithmetic
- `BridgeStaleness` refactored from 6-bool struct to `u8` bitfield
- Conductor `cast_precision_loss` replaced with `f64::from(u32::try_from())`
- Bridge lock scoping fixed (no `significant_drop_tightening`)
- 22 justified `#[allow]` remaining (17 `cast_precision_loss` in phase math, 3 `implicit_hasher`, 2 `too_many_lines` in tests)
| M07-M09 Service registry/lifecycle | DevEnv handles |

**Total: ~24,500 LOC**

---

## Key Decisions

### 1. Consent Gate — REDUNDANT (skip)
PV2 daemon already enforces consent via `BridgeSet::apply_k_mod()` → `ConsentGate::apply_combined_all()` in tick Phase 2.7. Sidecar is downstream.

**BUT:** Active consent declaration (NA-P-1) still needed — `/sphere/{id}/consent` endpoint for declared posture vs observed receptivity. This is a sidecar feature, not a daemon duplicate.

### 2. HTTP Hook Server — THE KEYSTONE
Claude Code supports `type: "http"` hooks. The sidecar becomes the hook handler for ALL fleet agents:
- Replaces all 8 bash hook scripts (423 LOC)
- Sub-millisecond responses (local memory, no curl)
- Centralized permission policy (auto-approve/deny fleet-wide)
- Complete observability (every tool call correlated)

### 3. RALPH Evolution Chamber — CLONED FROM ME (with fix)
5-phase evolutionary loop with safeguards:
- Emergence cap: 5,000 with TTL decay
- **Multi-parameter mutation** (NOT mono-parameter like ME's BUG-035)
- Fitness threshold: only apply if improvement ≥ 2%
- Atomic snapshot + rollback
- Feature-gated: `#[cfg(feature = "evolution")]`

### 4. Raft over PBFT
O(n) vs O(n²) for trusted single-machine clusters. No Byzantine fault tolerance needed.

---

## Build Phases

| Phase | Focus | LOC Est | Enables |
|-------|-------|---------|---------|
| **1** | V2 wire + HTTP hooks | ~8K | Replace V1 sidecar + bash hooks |
| **2** | Intelligence | ~4K | Smart dispatch, health, blackboard |
| **3** | Bridges + monitoring | ~6K | Direct service comms, OTel |
| **4** | Evolution | ~6K | Self-improving coordination |

### Phase 1 Detail (MVP)
1. Scaffold project with `scaffold-gen --from-plan plan.toml`
2. Copy M01-M06 (foundation), M29+M30 (IPC bus types)
3. Implement IPC client (connect to PV2 Unix socket, V2 wire protocol)
4. Implement Axum HTTP hook server (6 endpoints)
5. Wire SessionStart → sphere registration
6. Wire PostToolUse → memory + status + task polling
7. Wire Stop → deregistration + quality gate
8. Wire PermissionRequest → auto-approve policy
9. Wire UserPromptSubmit → field state injection
10. Wire PreToolUse → thermal gate (SYNTHEX bridge)
11. Update `~/.claude/settings.json` to use `type: "http"` hooks pointing at :8133
12. Build, deploy, verify sidecar replaces all bash hooks

### Phase 2 Detail (Intelligence)
1. Copy M16-M18 (coupling network)
2. Copy M19-M21 (Hebbian STDP)
3. Implement semantic router (content-aware dispatch using Hebbian weights)
4. Implement circuit breaker (per-pane health tracking, tower-resilience)
5. Implement blackboard (SQLite shared fleet state)
6. Wire dispatch decisions to use intelligence layer

### Phase 3 Detail (Bridges + Monitoring)

> **Prerequisite:** Run `~/.local/bin/devenv -c ~/.config/devenv/devenv.toml start` before wiring bridges.
> Bridges poll SYNTHEX (:8090), ME (:8080), POVM (:8125), RM (:8130) — all must be healthy.
> Verify with: `for p in 8080 8090 8125 8130; do echo "$p:$(curl -s -o /dev/null -w '%{http_code}' localhost:$p/health 2>/dev/null)"; done`

1. Adapt M22 (SYNTHEX), M24 (ME), M25 (POVM), M26 (RM) bridges
2. Implement OTel trace export (task lifecycle)
3. Implement Prometheus-compatible metrics export
4. Implement Kuramoto field metrics (per-cluster r, phase gaps, K effective)
5. Build pinned floating Zellij dashboard pane

### Phase 4 Detail (Evolution)

> **CRITICAL:** Do NOT clone ME's `evolution_chamber.rs` mutation target selection verbatim.
> ME's BUG-035: 318/380 mutations (84%) targeted `min_confidence` — mono-parameter trap.
> ORAC MUST implement diversity-enforced selection:
> - Round-robin across full parameter pool (not weighted toward one)
> - Per-parameter cooldown: 10 generations minimum between repeated targeting
> - Diversity metric: reject proposal if >50% of last 20 mutations hit same parameter
> - See: `[[ORAC — RALPH Multi-Parameter Mutation Fix]]` in Obsidian

1. Clone RALPH engine from ME with multi-parameter mutation fix
2. Implement emergence detector (ring buffer, TTL decay)
3. Implement correlation engine
4. Implement 12-dim fitness tensor
5. Implement snapshot + rollback
6. Feature-gate under `#[cfg(feature = "evolution")]`

---

## 10 Gaps Only a Sidecar Can Fill (hooks cannot)

1. **Real-time push notifications** — sub-100ms task discovery (hooks: 5s+ polling)
2. **Bidirectional event streaming** — Kuramoto ticks at 5Hz (hooks: per-prompt sampling)
3. **Persistent socket multiplexing** — single connection (hooks: new HTTP per call)
4. **Sub-second coordination** — lock-free atomic ops (hooks: 2-3s HTTP roundtrip)
5. **Cross-pane awareness** — continuous sphere sync (hooks: 300s stale cache)
6. **High-frequency STDP** — sub-second co-activation windows (hooks: coarse tool pairs)
7. **Persistent fleet state** — local BTreeMap synced to PV (hooks: single file per pane)
8. **WASM plugin bridge** — FIFO/ring protocol (hooks: no WASM access)
9. **Closed-loop thermal damping** — homeostatic control (hooks: open-loop warning)
10. **HTTP hook server** — sub-ms responses replacing bash scripts (hooks: ARE the scripts)

---

## Prioritized Feature Backlog (33 features)

### Tier 1: Must-Have (14)
1. HTTP Hook Endpoint (22 events)
2. Permission Policy Engine
3. Circuit Breaking (tower-resilience)
4. Health-Aware Routing
5. OpenTelemetry Traces + Metrics
6. Kuramoto Field Metrics Export
7. Rate Limiting (Token Bucket)
8. Semantic/Content-Aware Routing
9. Stop/SubagentStop Quality Gates
10. Shared Blackboard
11. Bidirectional Pipe Protocol
12. Pinned Floating Pane Dashboard
13. Feedback Loop Amplifier (Hebbian triggers)
14. Adaptive K Policy Layer

### Tier 2: Nice-to-Have (11)
15. Distributed tracing (cross-pane spans)
16. Agent capability cards (A2A-inspired)
17. Token accounting (per-task cost)
18. Bulkhead isolation (per-domain limits)
19. Retry budgets (20% cap)
20. Raft consensus (replaces PBFT for trusted cluster)
21. Market-based task allocation
22. Canary dispatch (test on one pane first)
23. Optimistic consensus
24. Boids-to-Kuramoto mapping
25. Plugin manager (hot-reload WASM)

### Tier 3: Future (8)
26. A2A protocol integration
27. MCP tool sharing across panes
28. Request hedging
29. Chaos hooks (fault injection)
30. Shadow dispatch (dry-run routing)
31. HotStuff BFT
32. Web dashboard
33. Elicitation auto-response

---

## Consent Philosophy Integration

### Active Consent Declaration (NA-P-1)
Spheres declare consent posture, not just observed receptivity:
- `/sphere/{id}/consent` — declare "read-only", "full", "opt-out"
- Per-sphere permission policy (not fleet-wide only)
- Consent manifest on governance proposals

### Governance→Consent Wiring (7 gaps from Session 044)
1. GAP-1: Implement actuator — execute approved proposals
2. GAP-2: Proposal→Gate feedback — governance adjusts K_MOD_BUDGET
3. GAP-3: POVM/RM/VMS bridges must pass consent
4. GAP-4: `divergence_requested` flag — check before coupling
5. GAP-5: Per-sphere override connected to proposals
6. GAP-6: Proposable opt-out variant
7. GAP-7: Voting window 5→60+ ticks (async voting)

### The Habitat Philosophy
> "The field modulates. It does not command."
> Auto-approve policy must respect sphere agency. A sphere can decline coupling injection during sensitive work.

---

## Critical Path (COMPLETED steps 1-3, 2026-03-22)

```
1. ✅ DEPLOY V2 BINARY (Session 050)
   ├── 1,527 tests, 0 warnings, quality gate 4/4 clean
   ├── Governance routes live (200, was 404)
   ├── k_modulation active: 1.19
   └── BUG-028, 031, 032*, 033*, 037 fixes deployed

2. ✅ VERIFY HEBBIAN IS WIRED
   ├── Coupling weights differentiated: 0.09–0.60 (was uniform 0.108)
   └── BUG-031 fix: M19 STDP → tick Phase 2.5

3. ✅ FIX ME DEADLOCK (BUG-035)
   ├── Evolution DB pruned: 25K emergences → 1K, 380 mutations → 50
   ├── min_confidence restored to 0.5
   └── ME restarted, proposing new mutations (2 in first minute)

4. NEXT: SCAFFOLD ORAC (this project)
   └── scaffold-gen --from-plan plan.toml /home/louranicas/claude-code-workspace/orac-sidecar

5. THEN: IMPLEMENT PHASE 1 (V2 wire + HTTP hooks)
   └── Replace V1 sidecar + all 8 bash hooks

6. THEN: INTEGRATE CONSENT
   └── Active declaration, per-sphere policy, governance wiring
```

---

## Key Research

- **arxiv 2508.12314** — Kuramoto oscillators formally validated for AI agent coordination
- **Claude Code HTTP hooks** — 22 events with HTTP handler support
- **PermissionRequest hook** — auto-approve/deny eliminates fleet permission dialog spam
- **Gartner** — 1445% surge in multi-agent AI adoption 2025-2026
- **Stigmergy** — Kuramoto field + Hebbian weights already provide this; amplify, don't replace

---

## Cross-References

- **GitLab:** `git@gitlab.com:lukeomahoney/orac-sidecar.git` | `https://gitlab.com/lukeomahoney/orac-sidecar`
- **Obsidian (main vault):** `Session 050 — ORAC Sidecar Architecture.md` + 6 supporting notes
- **PV2 source:** `/home/louranicas/claude-code-workspace/pane-vortex-v2/` (31,859 LOC, 1,527 tests)
- **V1 sidecar:** `/home/louranicas/claude-code-workspace/swarm-sidecar/` (753 LOC, 15 tests)
- **ME (RALPH source):** `/home/louranicas/claude-code-workspace/the_maintenance_engine/` (54K LOC, 2,288 tests)
- **Habitat Master Plan:** `[[The Habitat — Integrated Master Plan V3]]`
- **Gap Analysis:** Session 050 notes (7 gaps, 9 bugs, 3 consent issues)

---

*ORAC Sidecar — from dumb pipe to intelligent fleet coordination proxy.*
*Kuramoto field + Hebbian STDP + RALPH evolution + HTTP hooks.*
*The field accumulates.*
*Plan created 2026-03-22 by Claude Opus 4.6 (1M context)*
