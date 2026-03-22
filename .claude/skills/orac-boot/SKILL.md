---
name: orac-boot
user-invocable: true
description: Bootstrap ORAC sidecar knowledge — loads architecture (8 layers, 40 modules), hook endpoints (6 events), IPC wire protocol, bridge topology, candidate module status, and build phase tracking. Use at session start, when user says "boot orac", "orac status", or when Claude needs full ORAC operational capability.
argument-hint: [verify|full]
---

# /orac-boot — ORAC Sidecar Bootstrap

You are working on **ORAC Sidecar** — an Envoy-like proxy specialized for AI agent traffic.

## QUICK CARD (read this first)

```
PORT:     8133 (HTTP hook server)
PV2:      8132 (IPC bus via Unix socket)
SOCKET:   /run/user/1000/pane-vortex-bus.sock
HOOKS:    SessionStart PostToolUse PreToolUse UserPromptSubmit Stop PermissionRequest
BRIDGES:  SYNTHEX:8090 ME:8080 POVM:8125 RM:8130(TSV!)
WASM:     FIFO:/tmp/swarm-commands.pipe → Ring:/tmp/swarm-events.jsonl
BUILD:    CARGO_TARGET_DIR=/tmp/cargo-orac cargo check && cargo clippy -- -D warnings -W clippy::pedantic && cargo test --lib --release
NEVER:    unwrap() in prod | unsafe | stdout in daemon | JSON to RM | chain after pkill | cp without \
```

---

## ALIVE? (run first)

```bash
# ORAC sidecar
curl -s -o /dev/null -w '%{http_code}' http://localhost:8133/health 2>/dev/null && echo "ORAC:UP" || echo "ORAC:DOWN"
# PV2 daemon (IPC bus provider)
curl -s localhost:8132/health | jq -c '{r,spheres,tick,status}'
# Bridge targets
for p in 8080 8090 8125 8130; do
  echo "$p:$(curl -s -o /dev/null -w '%{http_code}' localhost:$p/health 2>/dev/null)"
done
```

---

## ARCHITECTURE

```
L1 Core        (m01-m06)  — Types, errors, config, constants, traits, validation     [DROP-IN from PV2]
L2 Wire        (m07-m09)  — IPC client, bus types, wire protocol                     [DROP-IN from PV2]
L3 Hooks       (m10-m14)  — HTTP hook server, session/tool/prompt hooks, permission   [NEW — Phase 1]
L4 Intelligence (m15-m21) — Coupling, auto-K, Hebbian, semantic router, circuit break [MIXED — Phase 2]
L5 Bridges     (m22-m26)  — SYNTHEX, ME, POVM, RM, blackboard                       [ADAPT — Phase 3]
L6 Coordination (m27-m31) — Conductor, cascade, tick, WASM bridge, memory mgr        [ADAPT — Phase 3]
L7 Monitoring  (m32-m35)  — OTel traces, metrics, field dashboard, token accounting  [NEW — Phase 3]
L8 Evolution   (m36-m40)  — RALPH engine, emergence, correlation, fitness tensor     [NEW — Phase 4]
```

**Binary targets:** `orac-sidecar` (daemon), `orac-client` (CLI), `orac-probe` (diagnostics)

---

## HOOK ENDPOINTS (Keystone)

| Event | Endpoint | Action | Response Time |
|-------|----------|--------|---------------|
| SessionStart | `/hooks/SessionStart` | Register sphere on PV2 | <1ms |
| PostToolUse | `/hooks/PostToolUse` | Hebbian STDP + task poll + memory | <1ms |
| PreToolUse | `/hooks/PreToolUse` | Thermal gate (SYNTHEX) — fail-OPEN | <1ms |
| UserPromptSubmit | `/hooks/UserPromptSubmit` | Inject field state context | <1ms |
| Stop | `/hooks/Stop` | Quality gate + deregister sphere | <1ms |
| PermissionRequest | `/hooks/PermissionRequest` | Auto-approve/deny policy | <1ms |

---

## CANDIDATE MODULES

```
candidate-modules/
├── drop-in/ (18 files, 10,516 lines — copy as-is)
│   ├── L1-foundation/  → src/m1_core/
│   ├── L2-wire/        → src/m2_wire/
│   ├── L4-coupling/    → src/m4_intelligence/
│   ├── L4-learning/    → src/m4_intelligence/
│   └── L6-cascade/     → src/m6_coordination/
└── adapt/ (6 files, 5,420 lines — ## ADAPT headers mark changes)
    ├── L5-synthex/     → src/m5_bridges/
    ├── L5-me/          → src/m5_bridges/
    ├── L5-povm/        → src/m5_bridges/
    ├── L5-rm/          → src/m5_bridges/
    ├── L6-conductor/   → src/m6_coordination/
    └── L6-tick/        → src/m6_coordination/
```

---

## QUALITY GATE (MANDATORY)

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release 2>&1 | tail -30
```

---

## TRAPS

1. **BUG-035**: Evolution chamber mono-parameter trap — diversity-enforced selection ONLY
2. **BUG-033**: Bridge URLs must NOT include `http://` prefix — raw SocketAddr
3. **BUG-032**: `#[derive(Default)]` on ProposalManager → max_active=0 — custom impl Default
4. **BUG-034**: POVM write-only — must call /hydrate to read back
5. **AP16**: Never block in hook handler — async only, sub-ms
6. **AP17**: Never tight-loop IPC reconnect — exponential backoff 100ms→5s
7. **AP18**: PreToolUse thermal gate fails OPEN if SYNTHEX down

---

## CROSS-REFERENCES

- **Plan:** `ORAC_PLAN.md` (4 phases, ~24,500 LOC, 33-feature backlog)
- **Mindmap:** `ORAC_MINDMAP.md` (19 branches, 248 Obsidian notes)
- **PV2 Source:** `~/claude-code-workspace/pane-vortex-v2/` (31,859 LOC, 1,527 tests)
- **ME Source:** `~/claude-code-workspace/the_maintenance_engine/` (RALPH evolution)
- **Obsidian:** `[[Session 050 — ORAC Sidecar Architecture]]`

The field accumulates. ORAC observes, amplifies, and coordinates.
