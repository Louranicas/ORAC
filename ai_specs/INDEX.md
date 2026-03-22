# ORAC Sidecar — AI Specifications Index

> Envoy-like proxy for AI agent traffic. Port 8133, 8 layers, 40 modules, 3 binaries.

## Layer Specifications

| Layer | Name | Modules | Spec File | Status |
|-------|------|---------|-----------|--------|
| L0 | Transport | Unix socket, TCP listener, TLS | `L0_TRANSPORT.md` | SCAFFOLD |
| L1 | Framing | NDJSON codec, length-prefix, backpressure | `L1_FRAMING.md` | SCAFFOLD |
| L2 | Session | Session lifecycle, auth, identity | `L2_SESSION.md` | SCAFFOLD |
| L3 | Routing | Route table, upstream selection, load balance | `L3_ROUTING.md` | SCAFFOLD |
| L4 | Filter | Request/response filters, transforms | `L4_FILTER.md` | SCAFFOLD |
| L5 | Observability | Metrics, tracing, audit log | `L5_OBSERVABILITY.md` | SCAFFOLD |
| L6 | Policy | Rate limit, circuit breaker, consent | `L6_POLICY.md` | SCAFFOLD |
| L7 | Evolution | RALPH loop, emergence, fitness | `L7_EVOLUTION.md` | SCAFFOLD |

## Cross-Cutting Specifications

| Spec | File | Description |
|------|------|-------------|
| HTTP API | [`API_SPEC.md`](API_SPEC.md) | REST endpoints: health, hooks, metrics, field, blackboard, consent |
| Hook Server | [`HOOKS_SPEC.md`](HOOKS_SPEC.md) | 6 Claude Code hook endpoints, IPC wiring to PV2 bus |
| Wire Protocol | [`WIRE_PROTOCOL_SPEC.md`](WIRE_PROTOCOL_SPEC.md) | V2 NDJSON over Unix socket, handshake, frames, keepalive |
| Bridges | [`BRIDGE_SPEC.md`](BRIDGE_SPEC.md) | SYNTHEX, ME, POVM, RM — upstream service integration |
| Evolution | [`EVOLUTION_SPEC.md`](EVOLUTION_SPEC.md) | RALPH 5-phase loop, emergence, fitness tensor, mutation |

## Pattern Specifications

| Pattern | File | Description |
|---------|------|-------------|
| Builder | [`patterns/BUILDER.md`](patterns/BUILDER.md) | Typestate builder pattern for config structs |
| Circuit Breaker | [`patterns/CIRCUIT_BREAKER.md`](patterns/CIRCUIT_BREAKER.md) | FSM: Closed/Open/HalfOpen with configurable thresholds |
| STDP | [`patterns/STDP.md`](patterns/STDP.md) | Hebbian spike-timing dependent plasticity for tool chains |
| Kuramoto | [`patterns/KURAMOTO.md`](patterns/KURAMOTO.md) | Phase oscillator coupling, order parameter, chimera detection |

## Binaries

| Binary | Crate | Description |
|--------|-------|-------------|
| `orac` | `orac-sidecar` | Main proxy daemon (port 8133) |
| `orac-ctl` | `orac-ctl` | CLI for admin, diagnostics, bridge queries |
| `orac-bench` | `orac-bench` | Load generator and latency benchmarker |

## Conventions

- All specs use JSON schemas for request/response formats
- Latency targets are per-hop unless stated otherwise
- Feature gates use `#[cfg(feature = "...")]` syntax
- Status values: SCAFFOLD, DRAFT, REVIEW, STABLE
