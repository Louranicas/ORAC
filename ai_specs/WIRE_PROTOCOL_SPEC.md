# ORAC Wire Protocol Specification (V2)

> NDJSON over Unix domain socket. Bidirectional, event-driven, with handshake and keepalive.

## Transport

| Parameter | Value |
|-----------|-------|
| Socket path | `/run/user/1000/pane-vortex-bus.sock` |
| Socket type | `SOCK_STREAM` (Unix) |
| Permissions | `0700` (owner only) |
| Framing | NDJSON (newline-delimited JSON, one frame per line) |
| Max frame size | 65536 bytes |
| Encoding | UTF-8 |

## Handshake Sequence

```
Client                          Server
  |                               |
  |--- ClientFrame::Hello ------->|
  |                               |  (validate, register sphere)
  |<-- ServerFrame::Welcome ------|
  |                               |
  |--- ClientFrame::Subscribe --->|
  |                               |  (register event patterns)
  |<-- ServerFrame::Ack ----------|
  |                               |
```

**Timeout**: Server closes connection if no `Hello` within 5 seconds of connect.

### Hello frame
```json
{"Hello":{"sphere_id":"uuid","persona":"cwd-or-name","version":2}}
```

### Welcome frame
```json
{"Welcome":{"server_version":2,"tick_rate_ms":100,"sphere_count":5}}
```

If rejected (e.g., duplicate sphere_id):
```json
{"Error":{"code":4001,"message":"sphere already registered"}}
```

## ClientFrame Variants (10)

| Variant | Fields | Purpose |
|---------|--------|---------|
| `Hello` | `sphere_id, persona, version` | Handshake initiation |
| `Goodbye` | `sphere_id` | Graceful disconnect |
| `Subscribe` | `patterns: [string]` | Event subscription (glob) |
| `Unsubscribe` | `patterns: [string]` | Remove subscriptions |
| `StatusUpdate` | `sphere_id, status` | Report sphere status change |
| `ToolPhase` | `sphere_id, tool_name, phase` | Report semantic phase |
| `Activity` | `sphere_id, tool_name, duration_ms` | Report tool completion |
| `HebbianPulse` | `pre, post, delta_t` | STDP weight update trigger |
| `ConsentQuery` | `sphere_id, permission_type, resource` | Request consent decision |
| `Ping` | `seq: u64` | Keepalive ping |

## ServerFrame Variants (6)

| Variant | Fields | Purpose |
|---------|--------|---------|
| `Welcome` | `server_version, tick_rate_ms, sphere_count` | Handshake response |
| `Ack` | `ref_seq: u64` | Acknowledge client frame |
| `Event` | `topic, payload` | Subscribed event delivery |
| `ConsentResponse` | `sphere_id, allowed, reason` | Consent decision |
| `Error` | `code, message` | Error notification |
| `Pong` | `seq: u64` | Keepalive response |

## Event Subscription

Subscribe using glob patterns:

```json
{"Subscribe":{"patterns":["field.*","task.*"]}}
```

### Event Topics

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

### Event frame example
```json
{"Event":{"topic":"field.tick","payload":{"r":0.997,"K":2.42,"spheres":6,"tick":1420}}}
```

## Keepalive

| Parameter | Value |
|-----------|-------|
| Interval | 30 seconds |
| Direction | Client sends `Ping`, server replies `Pong` |
| Timeout | Server disconnects if no frame received for 90 seconds |

```json
{"Ping":{"seq":42}}
```
```json
{"Pong":{"seq":42}}
```

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| 4001 | `DUPLICATE_SPHERE` | sphere_id already registered |
| 4002 | `INVALID_FRAME` | Malformed JSON or unknown variant |
| 4003 | `HANDSHAKE_TIMEOUT` | No Hello within 5s |
| 4004 | `FRAME_TOO_LARGE` | Frame exceeds 65536 bytes |
| 4005 | `UNKNOWN_SPHERE` | Operation on unregistered sphere |
| 4006 | `SUBSCRIBE_INVALID` | Invalid glob pattern |
| 4007 | `CONSENT_TIMEOUT` | Consent query timed out (500ms) |
| 4008 | `TASK_CAP_EXCEEDED` | Task count exceeds 1000 |
| 4009 | `VERSION_MISMATCH` | Client version != server version |
| 4010 | `AUTH_FAILED` | Authentication failure |
| 4011 | `RATE_LIMITED` | Too many frames per second |
| 4012 | `BUS_FULL` | Internal bus capacity exceeded |
| 4013 | `INTERNAL` | Unexpected server error |

## Serialization Notes

- All frames are serde-tagged enums: `{"VariantName":{...}}`
- Timestamps are `u64` milliseconds since Unix epoch
- `sphere_id` is a UUID v4 string
- `phase` is `f64` in range `[0, 2*pi)`
- `status` is lowercase: `idle`, `working`, `blocked`, `error`
- Boolean fields default to `false` if absent
