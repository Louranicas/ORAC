# IPC Bus Wire Protocol (V2 — ORAC Perspective)

Socket: `/run/user/1000/pane-vortex-bus.sock` | NDJSON | 0700 | SOCK_STREAM

## ORAC as IPC Client

ORAC connects to the PV2 bus as a persistent client via `m07_ipc_client` (406 LOC).
Wire protocol state machine in `m09_wire_protocol` (916 LOC):

```
Disconnected → Handshaking → Connected → Subscribing → Active
```

On SessionStart hook:
1. Connect to Unix socket
2. Send `Hello` frame with sphere_id + persona (CWD)
3. Receive `Welcome` (server_version, tick_rate_ms, sphere_count)
4. Send `Subscribe` for `field.*`, `task.*`, `sphere.*`
5. Receive `Ack`
6. Enter Active state (send/recv queues, keepalive timer)

On Stop hook:
1. Send `Goodbye` frame
2. PV2 creates ghost trace (FIFO max 20)
3. Close socket

## Wire Format (V2 NDJSON)

```
CLIENT → SERVER: {"Hello":{"sphere_id":"uuid","persona":"cwd","version":2}}
SERVER → CLIENT: {"Welcome":{"server_version":2,"tick_rate_ms":100,"sphere_count":5}}

CLIENT → SERVER: {"Subscribe":{"patterns":["field.*","task.*"]}}
SERVER → CLIENT: {"Ack":{"ref_seq":1}}

CLIENT → SERVER: {"StatusUpdate":{"sphere_id":"uuid","status":"working"}}
CLIENT → SERVER: {"ToolPhase":{"sphere_id":"uuid","tool_name":"Read","phase":0.0}}
CLIENT → SERVER: {"Activity":{"sphere_id":"uuid","tool_name":"Read","duration_ms":42}}
CLIENT → SERVER: {"HebbianPulse":{"pre":"Read","post":"Edit","delta_t":150}}
CLIENT → SERVER: {"ConsentQuery":{"sphere_id":"uuid","permission_type":"file_write","resource":"/path"}}
CLIENT → SERVER: {"Ping":{"seq":42}}
SERVER → CLIENT: {"Pong":{"seq":42}}

SERVER → CLIENT: {"Event":{"topic":"field.tick","payload":{"r":0.997,"K":2.42,"spheres":6,"tick":1420}}}
SERVER → CLIENT: {"ConsentResponse":{"sphere_id":"uuid","allowed":true,"reason":null}}
SERVER → CLIENT: {"Error":{"code":4001,"message":"sphere already registered"}}
```

## 10 ClientFrame Variants

| Variant | Fields | ORAC Usage |
|---------|--------|------------|
| Hello | sphere_id, persona, version | SessionStart hook |
| Goodbye | sphere_id | Stop hook |
| Subscribe | patterns[] | SessionStart hook |
| Unsubscribe | patterns[] | On demand |
| StatusUpdate | sphere_id, status | PostToolUse hook (working/idle) |
| ToolPhase | sphere_id, tool_name, phase | PreToolUse hook (semantic phase) |
| Activity | sphere_id, tool_name, duration_ms | PostToolUse hook |
| HebbianPulse | pre, post, delta_t | PostToolUse hook (STDP trigger) |
| ConsentQuery | sphere_id, permission_type, resource | PermissionRequest hook |
| Ping | seq | Keepalive (30s interval) |

## 6 ServerFrame Variants

| Variant | Fields | ORAC Handling |
|---------|--------|---------------|
| Welcome | server_version, tick_rate_ms, sphere_count | Store in OracState |
| Ack | ref_seq | Log, increment counter |
| Event | topic, payload | Route to emergence detector |
| ConsentResponse | sphere_id, allowed, reason | Cache per-sphere per-bridge |
| Error | code, message | Log, circuit breaker if 4012 |
| Pong | seq | Reset keepalive timer |

## Event Topics (subscribed by ORAC)

| Pattern | Events |
|---------|--------|
| `field.tick` | Periodic field state (r, K, sphere_count) |
| `field.chimera` | Chimera detected/resolved |
| `field.sync` | Sync threshold crossed |
| `task.created` | New task on bus |
| `task.completed` | Task finished |
| `task.failed` | Task failed |
| `sphere.registered` | New sphere joined |
| `sphere.deregistered` | Sphere left (ghost trace created) |

## 13 Error Codes

| Code | Name | Description |
|------|------|-------------|
| 4001 | DUPLICATE_SPHERE | sphere_id already registered |
| 4002 | INVALID_FRAME | Malformed JSON or unknown variant |
| 4003 | HANDSHAKE_TIMEOUT | No Hello within 5s |
| 4004 | FRAME_TOO_LARGE | Frame exceeds 65,536 bytes |
| 4005 | UNKNOWN_SPHERE | Operation on unregistered sphere |
| 4006 | SUBSCRIBE_INVALID | Invalid glob pattern |
| 4007 | CONSENT_TIMEOUT | Consent query timed out (500ms) |
| 4008 | TASK_CAP_EXCEEDED | Task count exceeds 1000 |
| 4009 | VERSION_MISMATCH | Client version != server version |
| 4010 | AUTH_FAILED | Authentication failure |
| 4011 | RATE_LIMITED | Too many frames per second |
| 4012 | BUS_FULL | Internal bus capacity exceeded |
| 4013 | INTERNAL | Unexpected server error |

## Keepalive

- Interval: 30 seconds (client sends Ping)
- Timeout: Server disconnects if no frame for 90 seconds
- ORAC resets keepalive timer on any frame sent

## V1 Compat Layer

V2 detects V1 format via JSON `type` field fallback:
- V1 sends: `{"type":"handshake","sphere_id":"..."}`
- V2 responds: `{"type":"HandshakeOk","tick":N,"peer_count":N,"r":0.0,"protocol_version":1}`

## WASM Bridge (m30)

WASI cannot hold sockets. ORAC bridges the gap:

```
Swarm WASM Plugin (Zellij)
    |  writes JSON to FIFO
    v
/tmp/swarm-commands.pipe (named pipe)
    |  ORAC reads & parses
    v
5 commands: dispatch, status, field_state, list_panes, ping
    |  ORAC translates to IPC frames
    v
/run/user/1000/pane-vortex-bus.sock
    |  bus processes, broadcasts events
    v
/tmp/swarm-events.jsonl (ring file, 1,000 line cap, FIFO eviction)
    |  plugin reads tail
    v
Swarm WASM Plugin (reads events)
```

## Serialization Notes

- All frames: serde-tagged enums `{"VariantName":{...}}`
- Timestamps: `u64` milliseconds since Unix epoch
- sphere_id: UUID v4 string
- phase: `f64` in range `[0, 2*pi)` — always `.rem_euclid(TAU)` after arithmetic
- status: lowercase `idle`, `working`, `blocked`, `error`
- Boolean fields default to `false` if absent
