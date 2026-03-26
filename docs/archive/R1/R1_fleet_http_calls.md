# R1: ORAC Sidecar — Fleet HTTP Calls Catalog

> **Generated:** 2026-03-25 | **Source:** All `.rs` files in `orac-sidecar/src/`
> **Scope:** Every outbound HTTP call from the ORAC sidecar to fleet services
> **Transport:** Raw TCP (`TcpStream`) via `http_helpers.rs` OR `ureq` via `m10_hook_server.rs`
> **Method:** Full grep of 18 patterns across entire `src/` tree, then manual verification of every call site

---

## Table of Contents

1. [Transport Mechanisms](#1-transport-mechanisms)
2. [Per-Service Call Sites](#2-per-service-call-sites)
   - [SYNTHEX :8090](#21-synthex-8090)
   - [ME :8080](#22-me-8080)
   - [POVM :8125](#23-povm-8125)
   - [RM :8130](#24-rm-8130)
   - [VMS :8120](#25-vms-8120)
   - [PV2 :8132](#26-pv2-8132)
3. [Fire-and-Forget vs Awaited](#3-fire-and-forget-vs-awaited)
4. [Consent-Gated Calls](#4-consent-gated-calls)
5. [Circuit-Breaker-Guarded Calls](#5-circuit-breaker-guarded-calls)
6. [Diagnostic Binaries (orac-probe, orac-client)](#6-diagnostic-binaries)

---

## 1. Transport Mechanisms

ORAC uses two distinct HTTP transport layers:

| Layer | Implementation | Timeout | Blocking Model | Used By |
|-------|---------------|---------|----------------|---------|
| **Raw TCP** | `http_helpers.rs` — manual `TcpStream` + HTTP/1.1 framing | 5s connect, 5s read | Synchronous (called from `spawn_blocking` or sync context) | L5 bridge modules (m22-m25), `main.rs` bridge functions |
| **ureq** | `m10_hook_server.rs` — `ureq::get/post` wrapped in `spawn_blocking` | Per-call (1-5s) | Async via `tokio::task::spawn_blocking` | L3 hook handlers (m11-m13), `fire_and_forget_post`, `breaker_guarded_post` |

**Key functions:**

| Function | File | Transport | Returns | Content-Type |
|----------|------|-----------|---------|-------------|
| `raw_http_get` | `http_helpers.rs:30` | Raw TCP | `PvResult<String>` (body) | N/A (GET) |
| `raw_http_get_with_limit` | `http_helpers.rs:39` | Raw TCP | `PvResult<String>` (body, size-capped) | N/A (GET) |
| `raw_http_post` | `http_helpers.rs:148` | Raw TCP | `PvResult<u16>` (status code only) | `application/json` |
| `raw_http_post_with_response` | `http_helpers.rs:162` | Raw TCP | `PvResult<String>` (body) | `application/json` |
| `raw_http_post_tsv` | `http_helpers.rs:273` | Raw TCP | `PvResult<u16>` (status code only) | `text/tab-separated-values` |
| `http_get` | `m10_hook_server.rs:1292` | ureq | `Option<String>` | N/A (GET) |
| `http_post` | `m10_hook_server.rs:1310` | ureq | `Option<String>` | `application/json` |
| `fire_and_forget_post` | `m10_hook_server.rs:988` | ureq (spawned) | None (fire-and-forget) | `application/json` |
| `breaker_guarded_post` | `m10_hook_server.rs:1012` | ureq (spawned) | None (fire-and-forget + breaker tracking) | `application/json` |

---

## 2. Per-Service Call Sites

### 2.1 SYNTHEX :8090

Default base: `127.0.0.1:8090` (configured via `SynthexBridge::with_config` or `OracState.synthex_url`)

| # | File:Line | Method | Path | Payload | Caller | Transport | Awaited? |
|---|-----------|--------|------|---------|--------|-----------|----------|
| S1 | `m22_synthex_bridge.rs:237` | GET | `/v3/thermal` | — | `SynthexBridge::poll_thermal()` | raw TCP | Yes (returns `PvResult<f64>`) |
| S2 | `m22_synthex_bridge.rs:271` | POST | `/api/ingest` | JSON: field state (`r`, `k_mod`, `spheres`, heat sources, etc.) | `SynthexBridge::post_field_state()` | raw TCP | Yes (returns `PvResult<()>`) |
| S3 | `m22_synthex_bridge.rs:341` | GET | `/api/health` | — | `SynthexBridge::health()` (Bridgeable trait) | raw TCP | Yes (returns `PvResult<bool>`) |
| S4 | `main.rs:758` | POST | `/v3/decay/trigger` | empty body `b""` | `post_field_to_synthex()` (one-time PID reset on first successful ingest) | raw TCP | Yes (inline, error logged) |
| S5 | `m13_prompt_hooks.rs:89,93` | GET | `/v3/thermal` | — | `handle_user_prompt_submit()` | ureq `http_get` | Yes (awaited for prompt injection) |
| S6 | `m12_tool_hooks.rs:304` | GET | `/v3/thermal` | — | `handle_pre_tool_use()` | ureq `http_get` | Yes (awaited for thermal gate check) |

**Call chain for S1:** `main.rs` tick loop (every `poll_interval` ticks, default 6) -> `state.synthex_bridge.poll_thermal()` -> `raw_http_get("127.0.0.1:8090", "/v3/thermal", "synthex")`

**Call chain for S2:** `main.rs` tick loop (every 6 ticks) -> `post_field_to_synthex()` -> `state.synthex_bridge.post_field_state(payload)` -> `raw_http_post("127.0.0.1:8090", "/api/ingest", payload, "synthex")`

---

### 2.2 ME :8080

Default base: `127.0.0.1:8080` (configured via `MeBridge::with_config`)

| # | File:Line | Method | Path | Payload | Caller | Transport | Awaited? |
|---|-----------|--------|------|---------|--------|-----------|----------|
| M1 | `m23_me_bridge.rs:364` | GET | `/api/observer` | — | `MeBridge::poll_observer()` | raw TCP | Yes (returns `PvResult<f64>`) |
| M2 | `m23_me_bridge.rs:479` | GET | `/api/health` | — | `MeBridge::health()` (Bridgeable trait) | raw TCP | Yes (returns `PvResult<bool>`) |

**Call chain for M1:** `main.rs` tick loop (every 12 ticks, breaker-gated) -> `state.me_bridge.poll_observer()` -> `raw_http_get("127.0.0.1:8080", "/api/observer", "me")`

**Note:** ME bridge is **read-only** for coupling purposes. `MeBridge::post()` is a no-op.

---

### 2.3 POVM :8125

Default base: `127.0.0.1:8125` (configured via `PovmBridge::with_config` or `OracState.povm_url`)

| # | File:Line | Method | Path | Payload | Caller | Transport | Awaited? |
|---|-----------|--------|------|---------|--------|-----------|----------|
| P1 | `m24_povm_bridge.rs:272` | POST | `/memories` | JSON: sphere snapshot (`sphere_id`, `r`, `event`) | `PovmBridge::snapshot()` | raw TCP | Yes (returns `PvResult<()>`) |
| P2 | `m24_povm_bridge.rs:285` | GET | `/pathways` | — | `PovmBridge::hydrate_pathways()` | raw TCP (512KB limit) | Yes (returns `PvResult<Vec<Pathway>>`) |
| P3 | `m24_povm_bridge.rs:312` | GET | `/summary` | — | `PovmBridge::hydrate_summary()` | raw TCP (512KB limit) | Yes (returns `PvResult<PovmSummary>`) |
| P4 | `m24_povm_bridge.rs:396-401` | POST | `/pathways` | JSON per-connection: `{pre_id, post_id, weight, co_activations, last_activated}` | `PovmBridge::write_pathways()` | raw TCP | Yes (returns `PvResult<usize>`) |
| P5 | `m24_povm_bridge.rs:458` | GET | `/health` | — | `PovmBridge::health()` (Bridgeable trait) | raw TCP (512KB limit) | Yes (returns `PvResult<bool>`) |
| P6 | `main.rs:824-830` | POST | `/pathways` | JSON per-connection: `{pre_id, post_id, weight, co_activations}` | `persist_stdp_to_povm()` (top 10 coupling connections) | raw TCP | Yes (inline, ok/err counted) |
| P7 | `m11_session_hooks.rs:55,60` | GET | `/hydrate` | — | `handle_session_start()` | ureq `http_get` | Yes (awaited, consent-gated) |
| P8 | `m11_session_hooks.rs:177-185` | POST | `/snapshots` | JSON: `{sphere_id, r, event: "session_end"}` | `handle_stop()` | `breaker_guarded_post` / `fire_and_forget_post` | No (fire-and-forget, consent-gated) |

**Call chain for P6:** `main.rs` tick loop (every 60 ticks) -> `persist_stdp_to_povm()` -> loop over top 10 connections -> `raw_http_post("127.0.0.1:8125", "/pathways", payload, "povm")` per connection

---

### 2.4 RM :8130

Default base: `127.0.0.1:8130` (configured via `RmBridge::with_config` or `OracState.rm_url`)

**CRITICAL:** All POST payloads to RM MUST be `text/tab-separated-values` (TSV). NEVER JSON.

| # | File:Line | Method | Path | Payload | Caller | Transport | Awaited? |
|---|-----------|--------|------|---------|--------|-----------|----------|
| R1 | `m25_rm_bridge.rs:310` | POST | `/put` | TSV: single record (`category\tagent\tconfidence\tttl\tcontent`) | `RmBridge::post_record()` | raw TCP (TSV) | Yes (returns `PvResult<()>`) |
| R2 | `m25_rm_bridge.rs:328` | POST | `/put` | TSV: multiple records (newline-joined) | `RmBridge::post_records()` | raw TCP (TSV) | Yes (returns `PvResult<()>`) |
| R3 | `m25_rm_bridge.rs:343` | GET | `/search?q=<query>` | — (query in URL) | `RmBridge::search()` | raw TCP | Yes (returns `PvResult<RmSearchResult>`) |
| R4 | `m25_rm_bridge.rs:416` | POST | `/put` | TSV: raw bytes from `Bridgeable::post()` | `RmBridge::post()` (Bridgeable trait) | raw TCP (TSV) | Yes (returns `PvResult<()>`) |
| R5 | `m25_rm_bridge.rs:428` | GET | `/health` | — | `RmBridge::health()` (Bridgeable trait) | raw TCP | Yes (returns `PvResult<bool>`) |
| R6 | `m11_session_hooks.rs:56,61` | GET | `/search?q=discovery` | — | `handle_session_start()` | ureq `http_get` | Yes (awaited, consent-gated) |
| R7 | `m11_session_hooks.rs:197-221` | POST | `/put` | TSV: `session\t{pane_id}\tsession-end\t3600\tsession-end r={r}` | `handle_stop()` (with intelligence feature) | ureq (direct `spawn_blocking`, 3s timeout cap) | Yes (awaited with 3s timeout, consent+breaker gated) |
| R8 | `m11_session_hooks.rs:229-233` | POST | `/put` | TSV: `session\t{pane_id}\tsession-end\t3600\tsession-end r={r}` | `handle_stop()` (without intelligence feature) | `fire_and_forget_post` | No (fire-and-forget, consent-gated) |
| R9 | `main.rs:1069` | POST | `/put` | TSV: `shared_state\torac-sidecar\t0.90\t600\ttick=N r=X gen=N fitness=X phase=P spheres=N me_fitness=X` | `post_state_to_rm()` (every 60 ticks) | raw TCP via `RmBridge::post_record()` | Yes (inline, error logged) |
| R10 | `main.rs:1117` | POST | `/put` | TSV: `emergence\torac-sidecar\t{confidence}\t1800\ttype=T severity=S confidence=C tick=N desc=D` | `relay_emergence_to_rm()` (on new emergence events) | raw TCP via `RmBridge::post_record()` | Yes (inline, error logged) |

---

### 2.5 VMS :8120

No dedicated bridge module. All VMS calls are inline in `main.rs` using `raw_http_post` / `raw_http_post_with_response`.

| # | File:Line | Method | Path | Payload | Caller | Transport | Awaited? |
|---|-----------|--------|------|---------|--------|-----------|----------|
| V1 | `main.rs:890-912` | POST | `/mcp/tools/call` | JSON: `{tool: "write_memory", params: {content: {type, tick, r, ralph_gen, ralph_fitness, ralph_phase, spheres}, region: "field_state"}}` | `post_state_to_vms()` (every 30 ticks) | raw TCP | Yes (inline, breaker outcome recorded) |
| V2 | `main.rs:934-948` | POST | `/v1/adaptation/trigger` | JSON: `{intensities: [{region: "consolidation", intensity: 1.0}]}` | `trigger_vms_consolidation()` (every 300 ticks) | raw TCP | Yes (inline, breaker outcome recorded) |
| V3 | `main.rs:982-1029` | POST | `/mcp/tools/call` | JSON: `{tool: "query_relevant", params: {query: "field r=X fitness=X", k: 5, region: "field_state", threshold: 0.3}}` | `query_vms_for_ralph_context()` (every 30 ticks during RALPH Recognize phase) | raw TCP (`raw_http_post_with_response`) | Yes (response parsed for VMS memories) |

**Note:** VMS calls are all breaker-guarded via `state.breaker_allows("vms")` check at function entry.

---

### 2.6 PV2 :8132

Default base: `http://127.0.0.1:8132` (configured via `OracState.pv2_url`)

PV2 calls use the ureq transport layer exclusively (via `http_get`, `http_post`, `fire_and_forget_post`, or `breaker_guarded_post`).

| # | File:Line | Method | Path | Payload | Caller | Transport | Awaited? |
|---|-----------|--------|------|---------|--------|-----------|----------|
| PV-1 | `m11_session_hooks.rs:45,51` | POST | `/sphere/{pane_id}/register` | JSON: `{persona: "orac-agent", frequency: 0.1}` | `handle_session_start()` | `fire_and_forget_post` | No |
| PV-2 | `m11_session_hooks.rs:159-160` | POST | `/bus/fail/{task_id}` | JSON: `{}` | `handle_stop()` (active task cleanup) | `fire_and_forget_post` | No |
| PV-3 | `m11_session_hooks.rs:168-170` | POST | `/sphere/{pane_id}/status` | JSON: `{status: "complete"}` | `handle_stop()` (mark sphere complete) | `fire_and_forget_post` | No |
| PV-4 | `m11_session_hooks.rs:308-309` | POST | `/sphere/{pane_id}/deregister` | empty string | `handle_stop()` (deregister sphere) | `fire_and_forget_post` | No |
| PV-5 | `m12_tool_hooks.rs:107,114` | POST | `/sphere/{pane_id}/memory` | JSON: `{tool_name, summary}` | `handle_post_tool_use()` | `breaker_guarded_post` (or `fire_and_forget_post` without intelligence feature) | No |
| PV-6 | `m12_tool_hooks.rs:119,126` | POST | `/sphere/{pane_id}/status` | JSON: `{status: "working", last_tool: tool_name}` | `handle_post_tool_use()` | `breaker_guarded_post` (or `fire_and_forget_post`) | No |
| PV-7 | `m12_tool_hooks.rs:167,169` | POST | `/bus/complete/{task_id}` | JSON: `{}` | `handle_post_tool_use()` (TASK_COMPLETE detected) | `breaker_guarded_post` (or `fire_and_forget_post`) | No |
| PV-8 | `m12_tool_hooks.rs:207-208` | GET | `/bus/tasks` | — | `poll_route_and_claim()` | ureq `http_get` | Yes (awaited for task polling) |
| PV-9 | `m12_tool_hooks.rs:240-242` | POST | `/bus/claim/{task_id}` | JSON: `{claimer: pane_id}` | `poll_route_and_claim()` | ureq `http_post` | Yes (awaited for atomic claim) |
| PV-10 | `m13_prompt_hooks.rs:80,94` | GET | `/bus/tasks` | — | `handle_user_prompt_submit()` | ureq `http_get` | Yes (awaited for pending task injection) |
| PV-11 | `m10_hook_server.rs:1069,1074` | GET | `/health` | — | `spawn_field_poller()` (every 5s) | ureq `http_get` | Yes (awaited for field state cache) |
| PV-12 | `m10_hook_server.rs:1070,1075` | GET | `/spheres` | — | `spawn_field_poller()` (every 5s) | ureq `http_get` | Yes (awaited for sphere map cache) |
| PV-13 | `m10_hook_server.rs:1490-1491` | GET | `/health` | — | `field_handler()` (`/field` endpoint) | ureq `http_get` | Yes (awaited for k/k_mod enrichment) |
| PV-14 | `m10_hook_server.rs:2001-2010` | POST | `/sphere/{sphere_id}/status` | JSON: `{status: "consent_updated", consent_fields: updated}` | `consent_put_handler()` | `fire_and_forget_post` | No |

---

## 3. Fire-and-Forget vs Awaited

### Fire-and-Forget (via `fire_and_forget_post` / `breaker_guarded_post` spawn)

All fire-and-forget calls spawn a background `tokio` task via `spawn_blocking`. The caller does not wait for the HTTP response. Errors are logged but never propagated.

| Call ID | Target | Path | Caller |
|---------|--------|------|--------|
| PV-1 | PV2 :8132 | `/sphere/{id}/register` | `handle_session_start()` |
| PV-2 | PV2 :8132 | `/bus/fail/{task_id}` | `handle_stop()` |
| PV-3 | PV2 :8132 | `/sphere/{id}/status` | `handle_stop()` |
| PV-4 | PV2 :8132 | `/sphere/{id}/deregister` | `handle_stop()` |
| PV-5 | PV2 :8132 | `/sphere/{id}/memory` | `handle_post_tool_use()` |
| PV-6 | PV2 :8132 | `/sphere/{id}/status` | `handle_post_tool_use()` |
| PV-7 | PV2 :8132 | `/bus/complete/{task_id}` | `handle_post_tool_use()` |
| PV-14 | PV2 :8132 | `/sphere/{id}/status` | `consent_put_handler()` |
| P8 | POVM :8125 | `/snapshots` | `handle_stop()` |
| R8 | RM :8130 | `/put` | `handle_stop()` (non-intelligence) |

### Awaited (synchronous return value used)

These calls block the caller (either within a sync bridge function or via `spawn_blocking.await`) and their return values drive control flow.

| Call ID | Target | Path | Caller | Return Value Used For |
|---------|--------|------|--------|-----------------------|
| S1 | SYNTHEX | `/v3/thermal` | `poll_thermal()` | Compute `k_adjustment` for coupling modulation |
| S2 | SYNTHEX | `/api/ingest` | `post_field_state()` | Error logging, first-post PID reset trigger |
| S3 | SYNTHEX | `/api/health` | `health()` | Health check bool |
| S4 | SYNTHEX | `/v3/decay/trigger` | `post_field_to_synthex()` | One-time PID reset (first post only) |
| S5 | SYNTHEX | `/v3/thermal` | `handle_user_prompt_submit()` | Temperature injection into prompt |
| S6 | SYNTHEX | `/v3/thermal` | `handle_pre_tool_use()` | Thermal gate for write operations |
| M1 | ME | `/api/observer` | `poll_observer()` | Fitness signal -> coupling adjustment |
| M2 | ME | `/api/health` | `health()` | Health check bool |
| P1 | POVM | `/memories` | `snapshot()` | Status code check |
| P2 | POVM | `/pathways` | `hydrate_pathways()` | Pathway vec for Hebbian weight seeding |
| P3 | POVM | `/summary` | `hydrate_summary()` | Summary for startup hydration |
| P4 | POVM | `/pathways` | `write_pathways()` | Count of successfully written pathways |
| P5 | POVM | `/health` | `health()` | Health check bool |
| P6 | POVM | `/pathways` | `persist_stdp_to_povm()` | Per-connection ok/err count for breaker |
| P7 | POVM | `/hydrate` | `handle_session_start()` | Memory/pathway counts for hydration message |
| R1-R4 | RM | `/put` | `post_record()`, `post_records()`, `post()` | Status code, error propagation |
| R5 | RM | `/health` | `health()` | Health check bool |
| R6 | RM | `/search?q=discovery` | `handle_session_start()` | Discovery count for hydration message |
| R7 | RM | `/put` | `handle_stop()` (intelligence) | Awaited with 3s timeout (race-condition fix) |
| R9 | RM | `/put` | `post_state_to_rm()` | Error logging |
| R10 | RM | `/put` | `relay_emergence_to_rm()` | Error logging |
| V1 | VMS | `/mcp/tools/call` | `post_state_to_vms()` | Breaker outcome recording |
| V2 | VMS | `/v1/adaptation/trigger` | `trigger_vms_consolidation()` | Breaker outcome recording |
| V3 | VMS | `/mcp/tools/call` | `query_vms_for_ralph_context()` | VMS memories fed into RALPH Recognize |
| PV-8 | PV2 | `/bus/tasks` | `poll_route_and_claim()` | Pending task list for routing/claiming |
| PV-9 | PV2 | `/bus/claim/{id}` | `poll_route_and_claim()` | Claim success/failure for task assignment |
| PV-10 | PV2 | `/bus/tasks` | `handle_user_prompt_submit()` | Pending task count for prompt injection |
| PV-11 | PV2 | `/health` | `spawn_field_poller()` | r, tick, K, sphere data for SharedState cache |
| PV-12 | PV2 | `/spheres` | `spawn_field_poller()` | Sphere map for coupling network sync |
| PV-13 | PV2 | `/health` | `field_handler()` | k/k_mod enrichment for /field endpoint |

---

## 4. Consent-Gated Calls

Consent is per-sphere, per-field. Checked via `state.consent_allows(sphere_id, field)` before making the HTTP call. If consent is denied, the call is skipped entirely (fail-closed).

| Call ID | Consent Field | File:Line | Description |
|---------|--------------|-----------|-------------|
| P7 | `"hydration"` | `m11_session_hooks.rs:54` | POVM hydration on SessionStart. Both P7 and R6 gated by same check. |
| R6 | `"hydration"` | `m11_session_hooks.rs:54` | RM discovery search on SessionStart. Same gate as P7. |
| P8 | `"povm_write"` | `m11_session_hooks.rs:176` | POVM snapshot on Stop (session end crystallization). |
| R7 | `"rm_write"` + breaker | `m11_session_hooks.rs:196` | RM session-end crystallization (intelligence feature). Dual-gated: consent AND breaker. |
| R8 | `"rm_write"` | `m11_session_hooks.rs:228` | RM session-end crystallization (non-intelligence feature). Consent-only gate. |

**Consent fields and their defaults (from `OracConsent` defaults):**

| Field | Default | Controls |
|-------|---------|----------|
| `hydration` | `true` | POVM + RM reads on SessionStart |
| `synthex_write` | `true` | SYNTHEX thermal ingest (not currently checked at call site) |
| `povm_read` | `true` | POVM pathway reads (not currently checked at call site) |
| `povm_write` | `false` | POVM snapshot writes on Stop |
| `rm_write` | `true` | RM crystallization on Stop |
| `hebbian_coupling` | `true` | Hebbian STDP weight updates (indirect) |

**Note:** L5 bridge module calls (S1-S4, M1-M2, P1-P6, R1-R5, V1-V3) do NOT check consent. Consent gating is only applied at the L3 hooks layer (m11/m12/m13) and for the consent PUT notification (PV-14).

---

## 5. Circuit-Breaker-Guarded Calls

Circuit breakers are per-service FSMs (Closed -> Open -> HalfOpen -> Closed). Managed by `BreakerRegistry` in `OracState`. Available only with `feature = "intelligence"`.

### Calls using `breaker_guarded_post` (spawned background + breaker tracking)

| Call ID | Service Key | File:Line | Description |
|---------|------------|-----------|-------------|
| PV-5 | `"pv2"` | `m12_tool_hooks.rs:114` | Sphere memory POST (PostToolUse) |
| PV-6 | `"pv2"` | `m12_tool_hooks.rs:126` | Sphere status POST (PostToolUse) |
| PV-7 | `"pv2"` | `m12_tool_hooks.rs:169` | Task complete POST (PostToolUse + TASK_COMPLETE) |
| P8 | `"povm"` | `m11_session_hooks.rs:185` | POVM snapshot POST (Stop handler) |

### Calls using `breaker_allows` guard (checked before calling)

| Call ID | Service Key | File:Line | Guard Type | Description |
|---------|------------|-----------|------------|-------------|
| S1 | `"synthex"` | `main.rs:1699` | `breaker_allows` before poll | SYNTHEX thermal poll in tick loop |
| S6 | `"synthex"` | `m12_tool_hooks.rs:299` | `breaker_allows` before http_get | PreToolUse thermal gate |
| M1 | `"me"` | `main.rs:1717` | `breaker_allows` before poll | ME observer poll in tick loop |
| V1 | `"vms"` | `main.rs:866` | `breaker_allows` at function entry | VMS memory POST |
| V2 | `"vms"` | `main.rs:925` | `breaker_allows` at function entry | VMS consolidation trigger |
| V3 | `"vms"` | `main.rs:961` | `breaker_allows` at function entry | VMS semantic query |
| R7 | `"rm"` | `m11_session_hooks.rs:196` | `breaker_allows` + `consent_allows` (dual-gated) | RM session-end crystallization |
| PV-10 | `"pv2"` | `m13_prompt_hooks.rs:83` | `!breaker_allows("pv2")` skips tasks call | UserPromptSubmit task polling |

### Breaker outcome recording (not guard, but tracking success/failure)

| Location | Service | Outcome | File:Line |
|----------|---------|---------|-----------|
| S1 poll | `"synthex"` | success/failure | `main.rs:1705,1709` |
| S6 gate | `"synthex"` | success/failure | `m12_tool_hooks.rs:309,311` |
| M1 poll | `"me"` | success/failure | `main.rs:1726,1739` |
| V1 post | `"vms"` | success/failure | `main.rs:897,907` |
| V2 trigger | `"vms"` | success/failure | `main.rs:941,945` |
| V3 query | `"vms"` | success/failure | `main.rs:989,1018` |
| P6 persist | `"povm"` | success/failure (majority) | `main.rs:841,843` |
| PV-11 poll | `"pv2"` | failure on miss | `m10_hook_server.rs:1087` |
| PV-10 tasks | `"pv2"` | success/failure | `m13_prompt_hooks.rs:99,101` |

**Breaker 4xx exclusion (BUG-059d):** `breaker_guarded_post` only records breaker failure for transport errors and 5xx status codes. 4xx client errors (e.g., 404 Not Found) do not trip the breaker.

---

## 6. Diagnostic Binaries

### orac-probe (`src/bin/probe.rs`)

Read-only connectivity check. Does NOT modify any service state.

| # | Method | Target | Path | Line |
|---|--------|--------|------|------|
| D1 | GET | ORAC :8133 | `/health` | 12 |
| D2 | GET | PV2 :8132 | `/health` | 13 |
| D3 | GET | SYNTHEX :8090 | `/api/health` | 14 |
| D4 | GET | ME :8080 | `/api/health` | 15 |
| D5 | GET | POVM :8125 | `/health` | 16 |
| D6 | GET | RM :8130 | `/health` | 17 |

### orac-client (`src/bin/client.rs`)

CLI tool for interacting with ORAC and PV2.

| # | Method | Target | Path | Subcommand | Line |
|---|--------|--------|------|------------|------|
| C1 | GET | ORAC :8133 | `/health` | `status` | 142 |
| C2 | GET | ORAC :8133 | `/field` | `field` | 180 |
| C3 | GET | ORAC :8133 | `/blackboard` | `blackboard` | 217 |
| C4 | GET | ORAC :8133 | `/metrics` | `metrics` | 264 |
| C5 | POST | ORAC :8133 | `/hooks/{event}` | `hook-test` | 315 |
| C6 | GET | ORAC :8133 | `/health` + `/field` | `watch` | 375-376 |
| C7 | GET | PV2 :8132 | `/spheres` | `fleet` | 91 |
| C8 | POST | PV2 :8132 | `/bus/submit` | `dispatch` | 657 |
| C9 | GET | All 6 services | `/health` or `/api/health` | `probe` | 336-341 |

---

## Summary Statistics

| Metric | Count |
|--------|-------|
| **Total unique call sites (daemon)** | 36 (S1-S6 + M1-M2 + P1-P8 + R1-R10 + V1-V3 + PV1-PV14) |
| **Fire-and-forget** | 10 (all PV2 writes from hooks + P8 + R8) |
| **Awaited** | 26 |
| **Consent-gated** | 5 (P7, R6, P8, R7, R8) |
| **Breaker-guarded (spawn)** | 4 (PV-5, PV-6, PV-7, P8) |
| **Breaker-guarded (check)** | 8 (S1, S6, M1, V1, V2, V3, R7, PV-10) |
| **Raw TCP transport** | 22 (all L5 bridge calls + main.rs inline calls) |
| **ureq transport** | 14 (all L3 hook calls) |
| **Services called** | 6 (SYNTHEX, ME, POVM, RM, VMS, PV2) |
| **Diagnostic binary calls** | 15 (D1-D6 + C1-C9) |
