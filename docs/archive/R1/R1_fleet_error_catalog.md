# R1 Fleet Error Catalog — ORAC Sidecar

> **Source:** `src/m1_core/m02_error_handling.rs` (596 LOC, 50 tests)
> **Derive:** `#[derive(Debug, thiserror::Error)]` — no `anyhow` anywhere in codebase
> **Alias:** `pub type PvResult<T> = Result<T, PvError>;`
> **Trait:** `ErrorClassifier` — `is_retryable()`, `severity()`, `code()`

---

## 1. PvError Variants

### 1.1 Config (1000-1099)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1000 | `ConfigLoad(String)` | `String` | Configuration file could not be loaded or parsed | `[PV-1000] config load failed: {0}` |
| PV-1001 | `ConfigValidation(String)` | `String` | A configuration value failed validation | `[PV-1001] config validation: {0}` |

### 1.2 Validation (1100-1199)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1100 | `NonFinite { field, value }` | `&'static str`, `f64` | Input value is NaN or infinite | `[PV-1100] non-finite value: {field} = {value}` |
| PV-1101 | `OutOfRange { field, value, min, max }` | `&'static str`, `f64`, `f64`, `f64` | Input value is out of the acceptable range | `[PV-1101] out of range: {field} = {value} (expected {min}..{max})` |
| PV-1102 | `EmptyString { field }` | `&'static str` | String input is empty when a non-empty value is required | `[PV-1102] empty string: {field}` |
| PV-1103 | `StringTooLong { field, len, max }` | `&'static str`, `usize`, `usize` | String input exceeds maximum length | `[PV-1103] string too long: {field} ({len} > {max})` |
| PV-1104 | `InvalidChars { field, reason }` | `&'static str`, `String` | String contains invalid characters | `[PV-1104] invalid characters in {field}: {reason}` |

### 1.3 Field (1200-1299)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1200 | `SphereNotFound(String)` | `String` | Sphere not found in the field | `[PV-1200] sphere not found: {0}` |
| PV-1201 | `SphereAlreadyRegistered(String)` | `String` | Sphere already exists (duplicate registration) | `[PV-1201] sphere already registered: {0}` |
| PV-1202 | `SphereCapReached(usize)` | `usize` | Maximum sphere count reached | `[PV-1202] sphere cap reached ({0})` |
| PV-1203 | `FieldComputation(String)` | `String` | Field computation produced invalid state | `[PV-1203] field computation error: {0}` |

### 1.4 Bridge (1300-1399)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1300 | `BridgeUnreachable { service, url }` | `String`, `String` | External service is unreachable | `[PV-1300] bridge unreachable: {service} at {url}` |
| PV-1301 | `BridgeError { service, status }` | `String`, `u16` | External service returned an error response | `[PV-1301] bridge error: {service} returned {status}` |
| PV-1302 | `BridgeParse { service, reason }` | `String`, `String` | External service response could not be parsed | `[PV-1302] bridge parse error: {service}: {reason}` |
| PV-1303 | `BridgeConsentDenied { service, sphere }` | `String`, `String` | Bridge consent denied (sphere opted out) | `[PV-1303] bridge consent denied: {service} for sphere {sphere}` |

### 1.5 Bus (1400-1499)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1400 | `BusSocket(String)` | `String` | IPC bus socket error | `[PV-1400] bus socket error: {0}` |
| PV-1401 | `BusProtocol(String)` | `String` | IPC bus protocol violation (invalid NDJSON, unknown message type) | `[PV-1401] bus protocol error: {0}` |
| PV-1402 | `BusTaskNotFound(String)` | `String` | IPC bus task not found | `[PV-1402] bus task not found: {0}` |
| PV-1403 | `CascadeRateLimit { per_minute }` | `u32` | Cascade rate limit exceeded | `[PV-1403] cascade rate limit exceeded: {per_minute} per minute` |

### 1.6 Persistence (1500-1599)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1500 | `Database(String)` | `String` | SQLite operation failed | `[PV-1500] database error: {0}` |
| PV-1501 | `Snapshot(String)` | `String` | Snapshot save/restore failed | `[PV-1501] snapshot error: {0}` |

### 1.7 Governance (1600-1699)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1600 | `ProposalNotFound(String)` | `String` | Proposal not found | `[PV-1600] proposal not found: {0}` |
| PV-1601 | `VotingClosed(String)` | `String` | Voting is closed for this proposal | `[PV-1601] voting closed: {0}` |
| PV-1602 | `QuorumNotReached { votes, needed }` | `usize`, `usize` | Quorum not reached | `[PV-1602] quorum not reached: {votes}/{needed}` |

### 1.8 Generic (1900-1999)

| Code | Variant | Wrapped Type | Description | Display Format |
|------|---------|-------------|-------------|----------------|
| PV-1900 | `Io(std::io::Error)` | `std::io::Error` | IO error wrapper (`#[from]` auto-convert) | `[PV-1900] io error: {0}` |
| PV-1901 | `Json(serde_json::Error)` | `serde_json::Error` | JSON serde error (`#[from]` auto-convert) | `[PV-1901] json error: {0}` |
| PV-1999 | `Internal(String)` | `String` | Internal error that should never happen | `[PV-1999] internal error: {0}` |

**Total: 24 variants across 8 categories.**

---

## 2. Error Classification (Fatal vs Transient)

The `ErrorClassifier` trait provides runtime classification of every `PvError` variant.

### 2.1 Severity Levels (`ErrorSeverity`)

| Level | Display | Meaning |
|-------|---------|---------|
| `Info` | `INFO` | Informational — no action needed |
| `Warning` | `WARN` | Potential issue, system continues |
| `Error` | `ERROR` | Operation failed, retry may help |
| `Critical` | `CRITICAL` | System integrity at risk |

### 2.2 Classification Matrix

| Variant | Severity | Retryable | Rationale |
|---------|----------|-----------|-----------|
| `ConfigLoad` | Info | No | Bad config won't fix itself |
| `ConfigValidation` | Info | No | Invalid value is deterministic |
| `NonFinite` | Info | No | NaN/Inf is a logic error |
| `OutOfRange` | Info | No | Same input = same rejection |
| `EmptyString` | Info | No | Empty input won't grow content |
| `StringTooLong` | Info | No | Same input = same rejection |
| `InvalidChars` | Info | No | Characters won't change |
| `SphereNotFound` | Info | No | Absent state won't appear |
| `SphereAlreadyRegistered` | Info | No | Duplicate is deterministic |
| `SphereCapReached` | **Warning** | No | Capacity limit — needs manual intervention |
| `FieldComputation` | **Critical** | No | Invalid field state (NaN in order parameter) |
| `BridgeUnreachable` | **Error** | **Yes** | Network/service may recover |
| `BridgeError` | **Error** | **Yes** | Service may stop returning errors |
| `BridgeParse` | Info | No | Response format won't change on retry |
| `BridgeConsentDenied` | **Warning** | No | Consent is an explicit opt-out |
| `BusSocket` | **Error** | **Yes** | Socket may become available |
| `BusProtocol` | Info | No | Protocol violation is deterministic |
| `BusTaskNotFound` | Info | No | Missing task won't appear |
| `CascadeRateLimit` | **Warning** | No | Must wait for rate window to pass |
| `Database` | **Error** | **Yes** | SQLite busy/locked may clear |
| `Snapshot` | **Error** | No | Snapshot failures typically structural |
| `ProposalNotFound` | Info | No | Absent proposal won't appear |
| `VotingClosed` | Info | No | Closed vote is permanent |
| `QuorumNotReached` | Info | No | Vote count won't change without action |
| `Io` | Info* | **Yes** | IO errors may be transient |
| `Json` | Info | No | Parse errors are deterministic |
| `Internal` | **Critical** | No | Impossible state — immediate investigation |

*`Io` severity is `Info` (falls into the catch-all `_` branch) but IS retryable.

### 2.3 Retryable Errors (5 variants)

```rust
fn is_retryable(&self) -> bool {
    matches!(self,
        BridgeUnreachable { .. }
        | BridgeError { .. }
        | BusSocket(_)
        | Database(_)
        | Io(_)
    )
}
```

All other 19 variants are **non-retryable** — they represent logic errors, validation failures, or permanent states.

---

## 3. Error Propagation Patterns

### 3.1 `From` Implementations (Auto-Conversion)

| Source Type | Target Variant | Mechanism |
|-------------|---------------|-----------|
| `std::io::Error` | `PvError::Io` | `#[from]` attribute (thiserror) |
| `serde_json::Error` | `PvError::Json` | `#[from]` attribute (thiserror) |
| `figment::Error` | `PvError::ConfigLoad` | Manual `From` impl (`.to_string()`) |
| `toml::de::Error` | `PvError::ConfigLoad` | Manual `From` impl (`.to_string()`) |

### 3.2 `PvResult<T>` Usage by Layer

| Layer | Module | Functions Returning `PvResult<T>` | Primary Error Variants |
|-------|--------|----------------------------------|----------------------|
| L1 Core | `m03_config` | `load()`, `from_path()`, `validate()` | `ConfigLoad`, `ConfigValidation` |
| L1 Core | `m05_traits` | `poll()`, `post()`, `health()` | `BridgeUnreachable`, `BridgeParse`, `BridgeError` |
| L1 Core | `m06_validation` | 9 validators (phase, freq, strength, weight, receptivity, k_mod, pane_id, persona, tool_name, summary) | `NonFinite`, `OutOfRange`, `EmptyString`, `StringTooLong`, `InvalidChars` |
| L2 Wire | `m07_ipc_client` | `connect()`, `subscribe()`, `send_frame()`, `recv_frame()`, `disconnect()`, `connect_with_backoff()`, `reconnect()` | `BusSocket`, `BusProtocol` |
| L2 Wire | `m09_wire_protocol` | `initiate_handshake()`, `subscribe()`, `submit_task()`, `disconnect()`, `process_incoming()`, `send_keepalive()` | `BusProtocol` |
| L5 Bridges | `m22_synthex_bridge` | `poll_thermal()`, `post_field_state()`, `poll()`, `post()`, `health()` | `BridgeUnreachable`, `BridgeParse` |
| L5 Bridges | `m23_me_bridge` | `poll_observer()`, `poll()`, `post()`, `health()` | `BridgeUnreachable`, `BridgeParse` |
| L5 Bridges | `m24_povm_bridge` | `snapshot()`, `hydrate_pathways()`, `hydrate_summary()`, `persist_co_activations()`, `poll()`, `post()`, `health()` | `BridgeUnreachable`, `BridgeParse` |
| L5 Bridges | `m25_rm_bridge` | `post_record()`, `post_records()`, `search()`, `poll()`, `post()`, `health()` | `BridgeUnreachable`, `BridgeError` |
| L5 Bridges | `m26_blackboard` | 25+ CRUD methods | `Database` |
| L5 Bridges | `http_helpers` | `raw_http_get()`, `raw_http_post()`, `raw_http_post_tsv()`, `raw_http_post_with_response()` | `BridgeUnreachable`, `BridgeError`, `BridgeParse` |
| L6 Coordination | `m28_cascade` | `send()`, `forward()`, `acknowledge()`, `reject()` | `CascadeRateLimit`, `BusProtocol` |
| L6 Coordination | `m30_wasm_bridge` | `to_jsonl()`, `write_event()`, `parse_command()`, `ingest_command()` | `Json`, `Io` |
| L7 Monitoring | `m32_otel_traces` | `start()`, `set_str/int/float/bool()`, `set_pane()`, `set_task()` | `StringTooLong`, `ConfigValidation` |
| L7 Monitoring | `m33_metrics_export` | `exposition()` | `ConfigValidation` |
| L7 Monitoring | `m35_token_accounting` | `validate()`, `with_budget()`, `record_pane_usage()`, `record_task_budget()`, `set_budget()` | `ConfigValidation` |
| L8 Evolution | `m36_ralph_engine` | `tick()`, `phase_recognize()`, `phase_analyze()` | `FieldComputation`, `Internal` |
| L8 Evolution | `m37_emergence_detector` | `validate_config()`, 8 `detect_*` methods, `register_monitor()`, `check_monitor()` | `ConfigValidation`, `Internal` |
| L8 Evolution | `m38_correlation_engine` | `validate_config()` | `ConfigValidation` |
| L8 Evolution | `m39_fitness_tensor` | `from_index()`, `validate()`, `validate_config()`, `evaluate()` | `OutOfRange`, `NonFinite`, `ConfigValidation` |
| L8 Evolution | `m40_mutation_selector` | `validate_config()`, `register_parameter()`, `update_value()`, `select()` | `ConfigValidation`, `SphereNotFound` |

### 3.3 Propagation Style

- **`?` operator** — standard propagation through `PvResult<T>` return types
- **No `unwrap()` or `expect()` outside tests** — enforced by clippy lints
- **No `anyhow`** — zero usage anywhere in the codebase; all errors are structured `PvError`
- **No `unsafe`** — zero tolerance
- **String-wrapped errors** — most variants wrap `String` for context; foreign errors use `From` impls or `.to_string()`

---

## 4. Bridge Error Handling

### 4.1 HTTP Helper Error Mapping (`m5_bridges/http_helpers.rs`)

The raw HTTP helper functions perform TCP connections using `std::net::TcpStream` and map errors to `PvError` variants:

| Failure Point | Error Variant |
|---------------|---------------|
| `TcpStream::connect_timeout()` fails | `BridgeUnreachable { service, url }` |
| `set_read_timeout()` / `set_write_timeout()` fails | `BridgeUnreachable { service, url }` |
| `stream.write_all()` fails | `BridgeUnreachable { service, url }` |
| Read timeout / zero bytes | `BridgeUnreachable { service, url }` |
| HTTP status 400-499 | `BridgeError { service, status }` |
| HTTP status 500-599 | `BridgeError { service, status }` |
| No HTTP body separator found | `BridgeParse { service, reason }` |
| Chunked encoding decode fails | `BridgeParse { service, reason }` |

### 4.2 Per-Bridge Error Behavior

| Bridge | Module | `poll()` Error | `post()` Error | `health()` Error |
|--------|--------|---------------|----------------|-----------------|
| SYNTHEX :8090 | `m22_synthex_bridge` | `BridgeUnreachable`/`BridgeParse` | `BridgeUnreachable` | `BridgeUnreachable` |
| ME :8080 | `m23_me_bridge` | `BridgeUnreachable`/`BridgeParse` | `BridgeUnreachable` | `BridgeUnreachable`/`BridgeParse` |
| POVM :8125 | `m24_povm_bridge` | `BridgeUnreachable` | `BridgeUnreachable` | `BridgeUnreachable` |
| RM :8130 | `m25_rm_bridge` | `BridgeUnreachable`/`BridgeError` | `BridgeUnreachable`/`BridgeError` | `BridgeUnreachable` |

### 4.3 Breaker-Guarded POST Pattern (`m10_hook_server.rs:1012`)

```rust
pub fn breaker_guarded_post(state: &Arc<OracState>, service: &str, url: String, body: String)
```

- Checks `state.breaker_allows(service)` — skips call if breaker is open
- Spawns `tokio::spawn` + `spawn_blocking` for the actual `ureq::post()`
- **BUG-059d fix:** Only trips breaker on transport errors and 5xx; 4xx (e.g. 404 Not Found) does NOT trip the breaker
- On success: `state.breaker_success(service)`
- On health failure: `state.breaker_failure(service)`
- On task panic: `state.breaker_failure(service)`

### 4.4 Fire-and-Forget POST Pattern (`m10_hook_server.rs`)

```rust
pub fn fire_and_forget_post(url: String, body: String)
```

- Spawns `tokio::spawn` + `spawn_blocking`
- Logs transport errors as `tracing::warn!` (upgraded from debug — T2 fix)
- Logs task panics as `tracing::warn!`
- **No breaker integration** — fire-and-forget calls don't update circuit breaker state

### 4.5 Consent-Gated Error

`BridgeConsentDenied { service, sphere }` — returned when a sphere has explicitly opted out of external modulation. This is a **Warning**-severity, **non-retryable** error. Each bridge module includes a `_consent_check()` stub.

---

## 5. Hook Error Behavior (Always 200)

### 5.1 Design Principle

**All 6 hook endpoints return HTTP 200 with `Json<HookResponse>`**, regardless of internal errors. This is by design: Claude Code hooks must return a valid JSON response to avoid blocking the host conversation. Internal failures are handled via logging and graceful degradation, never via HTTP error codes.

### 5.2 Hook Endpoint Signatures

Every hook handler has this return type:

```rust
async fn handle_*(
    State(state): State<Arc<OracState>>,
    Json(event): Json<HookEvent>,
) -> Json<HookResponse>
```

The `Json<HookResponse>` return type produces HTTP 200 with `Content-Type: application/json`.

### 5.3 `HookResponse` Constructors

| Constructor | Fields Set | Purpose |
|-------------|-----------|---------|
| `HookResponse::empty()` | `{}` (all `None`) | No-op response — nothing to inject |
| `HookResponse::with_message(msg)` | `systemMessage: msg` | Inject context into conversation |
| `HookResponse::block(reason)` | `decision: "block", reason: reason` | Block tool execution (PreToolUse only) |
| `HookResponse::allow(msg)` | `systemMessage: msg` (optional) | Explicitly allow tool execution |

### 5.4 Per-Hook Error Handling

| Hook | Endpoint | Error Behavior |
|------|----------|---------------|
| `SessionStart` | `POST /hooks/SessionStart` | POVM/RM hydration failures -> log + continue with partial data. Always returns `HookResponse::with_message()` with whatever data was successfully gathered. |
| `Stop` | `POST /hooks/Stop` | Task fail/crystallize/deregister failures -> log + continue. Always returns `HookResponse::empty()`. |
| `PostToolUse` | `POST /hooks/PostToolUse` | PV2 memory/status POST failures -> `breaker_guarded_post` handles async (fire-and-forget). Task polling failures -> `tracing::warn`. Always returns `HookResponse::empty()` or `HookResponse::with_message()`. |
| `PreToolUse` | `POST /hooks/PreToolUse` | SYNTHEX thermal fetch failure -> returns `HookResponse::empty()` (allow by default). Only blocks if thermal > threshold AND fetch succeeds. |
| `UserPromptSubmit` | `POST /hooks/UserPromptSubmit` | Field state fetch failure -> returns `HookResponse::empty()` (no injection). Graceful degradation to available data. |
| `PermissionRequest` | `POST /hooks/PermissionRequest` | Policy evaluation is pure logic (no I/O) — cannot fail. Returns `HookResponse::empty()`, `HookResponse::allow()`, or `HookResponse::block()`. |

### 5.5 Error Isolation Architecture

```
Claude Code                 ORAC Hook Server               External Services
    |                            |                              |
    |-- POST /hooks/Event ------>|                              |
    |                            |-- fire_and_forget ---------->| (async, no-wait)
    |                            |-- breaker_guarded_post ----->| (async, breaker-tracked)
    |                            |-- http_get (await, timeout) >| (sync, timeout-bounded)
    |<-- 200 Json<HookResponse> -|                              |
    |                            |                              |
```

Key invariants:
- Hook endpoint ALWAYS returns 200 within timeout (2-5s depending on event)
- External service failures are logged, not propagated to HTTP response
- Async operations (fire_and_forget, breaker_guarded) happen AFTER response is sent
- Sync operations (http_get for field state, thermal) have explicit timeouts
- If ALL external calls fail, hooks return `HookResponse::empty()` (safe default)

---

## 6. No `OracError` Type

There is no separate `OracError` enum. The entire codebase uses the single `PvError` type (originally from Pane-Vortex V2, adopted wholesale by ORAC). This is a deliberate design choice — one unified error type for the entire 30,524 LOC codebase.

---

## 7. Error Code Summary

```
1000-1099  Config        (2 variants)  -- load/validation
1100-1199  Validation    (5 variants)  -- NaN, range, empty, length, chars
1200-1299  Field         (4 variants)  -- sphere CRUD + computation
1300-1399  Bridge        (4 variants)  -- unreachable, error, parse, consent
1400-1499  Bus           (4 variants)  -- socket, protocol, task, rate limit
1500-1599  Persistence   (2 variants)  -- database, snapshot
1600-1699  Governance    (3 variants)  -- proposal, voting, quorum
1900-1999  Generic       (3 variants)  -- IO, JSON, internal
---------------------------------------
Total: 27 codes, 24 variants (some ranges reserved for future use)
```
