# L2 Wire — IPC Client & Bus Layer

> Always compiled. Unix socket IPC client connecting to PV2 daemon bus.

## Feature Gate

None (always on).

## Modules

| Module | File | Description | Test Kind |
|--------|------|-------------|-----------|
| m07_ipc_client | `src/m2_wire/m07_ipc_client.rs` | Unix socket client connecting to PV2 daemon bus (V2 wire protocol), exponential backoff reconnect | unit + integration |
| m08_bus_types | `src/m2_wire/m08_bus_types.rs` | `ClientFrame` (10 variants), `ServerFrame` (6 variants), `TaskTarget` (4 variants), `TaskStatus` FSM, NDJSON framing | unit |
| m09_wire_protocol | `src/m2_wire/m09_wire_protocol.rs` | V2 wire format: handshake, subscribe, frames, keepalive, V1 compat layer | unit + property |

## Dependencies

- **L1 Core** — `OracError`, `PaneId`, `TaskId`, config (socket path, timeouts)

## Design Constraints

- IPC socket path: `/run/user/1000/pane-vortex-bus.sock` (PV2 daemon, 0700 permissions)
- ORAC is a **client**, not a server — PV2 owns the socket
- Handshake timeout: 5 seconds. Reject slow connections.
- NDJSON framing: one JSON object per line, `\n` delimiter
- Reconnect backoff: exponential 100ms→5s (AP17 — never tight loop)
- `BusTask` cap: 1000 concurrent tasks
- Lock ordering: always `AppState` before `BusState` (deadlock prevention)
- Frame variants must be exhaustively matched — no `_ =>` wildcards

## Hot-Swap Source

- m07, m08: DROP-IN from PV2 `m7_coordination/m29_ipc_bus.rs`, `m30_bus_types.rs` (candidate-modules/drop-in/L2-wire/)
- m09: NEW (ORAC-specific wire protocol adapter)

## Cross-References

- [[Pane-Vortex IPC Bus — Session 019b]]
- [[Session 050 — Sidecar Deep Dive]]
- `.claude/schemas/bus_event.schema.json` (24 event types)
- `.claude/schemas/bus_frame.schema.json` (5 frame types)
- ORAC_PLAN.md §Phase 1 Detail
