# ORAC Sidecar — AI Documentation Index

> Envoy-like proxy for AI agent traffic. Port 8133. 8 layers, 40 modules, 3 binaries.
> **Start here:** [`QUICKSTART.md`](QUICKSTART.md) — build, run, architecture, file map, reading order

## Quick Navigation

| Area | Path | Contents |
|------|------|----------|
| **Quick Start** | [`QUICKSTART.md`](QUICKSTART.md) | Build, deploy, architecture, file map, traps, reading order |
| Layer docs | [`layers/L1_CORE.md`](layers/L1_CORE.md) .. [`L8_EVOLUTION.md`](layers/L8_EVOLUTION.md) | Per-layer constraints, dependencies, hot-swap sources |
| Module docs | [`modules/INDEX.md`](modules/INDEX.md) | 41-module inventory: types, tests, design decisions |
| Schematics | `schematics/*.mmd` | 4 Mermaid architecture diagrams |
| Patterns | [`GOLD_STANDARD_PATTERNS.md`](GOLD_STANDARD_PATTERNS.md) | 10 mandatory Rust patterns (P1-P10) |
| Anti-patterns | [`ANTI_PATTERNS.md`](ANTI_PATTERNS.md) | 17 banned patterns (A1-A17) with severity |
| Specs | [`../ai_specs/INDEX.md`](../ai_specs/INDEX.md) | API, hooks, wire protocol, bridges, evolution specs |
| Meta tree | [`../ORAC_MINDMAP.md`](../ORAC_MINDMAP.md) | 248 Obsidian notes, 19 branches, 3 vaults |
| Plan | [`../ORAC_PLAN.md`](../ORAC_PLAN.md) | 4 build phases, ~24.5K LOC, 33-feature backlog |

## Layer → Source → Docs

| Layer | Feature Gate | `src/` | Modules | Layer Doc | Module Doc |
|-------|-------------|--------|---------|-----------|------------|
| **L1 Core** | _(always)_ | `m1_core/` | m01-m06 + field_state | [L1_CORE](layers/L1_CORE.md) | [L1_CORE_MODULES](modules/L1_CORE_MODULES.md) |
| **L2 Wire** | _(always)_ | `m2_wire/` | m07-m09 | [L2_WIRE](layers/L2_WIRE.md) | [L2_WIRE_MODULES](modules/L2_WIRE_MODULES.md) |
| **L3 Hooks** | `api` | `m3_hooks/` | m10-m14 | [L3_HOOKS](layers/L3_HOOKS.md) | [L3_HOOKS_MODULES](modules/L3_HOOKS_MODULES.md) |
| **L4 Intelligence** | `intelligence` | `m4_intelligence/` | m15-m21 | [L4_INTELLIGENCE](layers/L4_INTELLIGENCE.md) | [L4_INTELLIGENCE_MODULES](modules/L4_INTELLIGENCE_MODULES.md) |
| **L5 Bridges** | `bridges` | `m5_bridges/` | m22-m26 | [L5_BRIDGES](layers/L5_BRIDGES.md) | [L5_BRIDGES_MODULES](modules/L5_BRIDGES_MODULES.md) |
| **L6 Coordination** | _(always)_ | `m6_coordination/` | m27-m31 | [L6_COORDINATION](layers/L6_COORDINATION.md) | [L6_COORDINATION_MODULES](modules/L6_COORDINATION_MODULES.md) |
| **L7 Monitoring** | `monitoring` | `m7_monitoring/` | m32-m35 | [L7_MONITORING](layers/L7_MONITORING.md) | [L7_MONITORING_MODULES](modules/L7_MONITORING_MODULES.md) |
| **L8 Evolution** | `evolution` | `m8_evolution/` | m36-m40 | [L8_EVOLUTION](layers/L8_EVOLUTION.md) | [L8_EVOLUTION_MODULES](modules/L8_EVOLUTION_MODULES.md) |

## Architectural Schematics

| Diagram | File | Shows |
|---------|------|-------|
| Layer Architecture | [layer_architecture.mmd](schematics/layer_architecture.mmd) | 8 layers with dependency arrows |
| Hook Flow | [hook_flow.mmd](schematics/hook_flow.mmd) | Claude Code → ORAC → PV2 sequence |
| Bridge Topology | [bridge_topology.mmd](schematics/bridge_topology.mmd) | ORAC connections to 5 services |
| Field Dashboard | [field_dashboard.mmd](schematics/field_dashboard.mmd) | PV2 field → metrics → dashboard pipeline |

## Specifications (ai_specs/)

| Spec | File | Description |
|------|------|-------------|
| HTTP API | [`API_SPEC.md`](../ai_specs/API_SPEC.md) | REST endpoints, request/response schemas |
| Hook Server | [`HOOKS_SPEC.md`](../ai_specs/HOOKS_SPEC.md) | 6 Claude Code hook events, payload structures |
| Wire Protocol | [`WIRE_PROTOCOL_SPEC.md`](../ai_specs/WIRE_PROTOCOL_SPEC.md) | V2 NDJSON, frames, handshake, keepalive |
| Bridges | [`BRIDGE_SPEC.md`](../ai_specs/BRIDGE_SPEC.md) | SYNTHEX, ME, POVM, RM integration |
| Evolution | [`EVOLUTION_SPEC.md`](../ai_specs/EVOLUTION_SPEC.md) | RALPH 5-phase loop, fitness tensor, mutation |
| Patterns | [`patterns/`](../ai_specs/patterns/) | Builder, Circuit Breaker, Kuramoto, STDP |

## Development Context (.claude/)

| File | Purpose |
|------|---------|
| [`context.json`](../.claude/context.json) | Machine-readable: layers, bridges, hooks, bins |
| [`patterns.json`](../.claude/patterns.json) | 22 patterns (P01-P22) |
| [`anti_patterns.json`](../.claude/anti_patterns.json) | 20 anti-patterns (AP01-AP20) |
| [`schemas/`](../.claude/schemas/) | 5 JSON schemas (hooks, bus, permission policy) |
| [`queries/`](../.claude/queries/) | 3 SQL templates (blackboard, hooks, fleet) |
| [`skills/`](../.claude/skills/) | 3 skills: orac-boot, hook-debug, bridge-probe |

## Port Map

| Service | Port | Health | Role |
|---------|------|--------|------|
| ORAC Sidecar | 8133 | `/health` | AI agent traffic proxy |
| Pane-Vortex 2 | 8132 | `/health` | Sphere field coordination (IPC bus) |
| Reasoning Memory | 8130 | `/health` | TSV-format memory store |
| POVM | 8125 | `/health` | Memory hydration + crystallisation |
| SYNTHEX | 8090 | `/api/health` | Brain/synthesis engine (thermal) |
| Maintenance Engine | 8080 | `/api/health` | DevOps maintenance (fitness) |
