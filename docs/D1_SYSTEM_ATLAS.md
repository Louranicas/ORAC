# D1: ORAC Sidecar -- System Atlas

> **Version:** 0.6.0 | **Port:** 8133 | **Batch:** 5 | **55 source files, ~41,369 LOC, ~1,748 tests**
>
> Obsidian: `[[Session 061 -- ORAC System Atlas]]` | `[[ORAC Sidecar -- Architecture Schematics]]` | `[[Session 056 -- ORAC God-Tier Mastery]]` | `[[ULTRAPLATE Master Index]]`

---

## 1. Purpose

ORAC is an **Envoy-like proxy specialized for AI agent traffic**. It replaces the V1 `swarm-sidecar` (546 LOC, non-functional due to V1/V2 wire mismatch) and fills 10 gaps that bash hook scripts cannot address:

1. Real-time push notifications via IPC Unix socket
2. Bidirectional event streaming (V2 wire protocol)
3. Persistent socket multiplexing across sessions
4. Sub-second fleet coordination (tick interval 5s, HTTP response <10ms)
5. Cross-pane awareness via Kuramoto field state caching
6. High-frequency Hebbian STDP weight updates
7. Persistent fleet state via SQLite blackboard (9 tables)
8. WASM plugin bridge (FIFO/ring protocol)
9. Closed-loop thermal damping via SYNTHEX bridge
10. HTTP hook server replacing 6 bash scripts with sub-millisecond endpoints

**Validated by:** arXiv 2508.12314 (Kuramoto oscillators for AI agent coordination).

---

## 2. Technology Stack

| Component | Version | Role |
|-----------|---------|------|
| Rust | Edition 2021, MSRV 1.75 | Language |
| axum | 0.8 | HTTP framework (feature: `api`) |
| tokio | 1.x (full features) | Async runtime |
| parking_lot | 0.12 | RwLock/Mutex (no poison) |
| rusqlite | 0.32 | SQLite blackboard (feature: `persistence`) |
| figment | 0.10 | Config loading (TOML + env) |
| ureq | 2.x | Synchronous HTTP client for bridge calls |
| serde / serde_json | 1.x | Serialization |
| thiserror | 2.x | Error derivation |
| tracing / tracing-subscriber | 0.1 / 0.3 | Structured logging |
| socket2 | 0.5 | Unix domain sockets, `SO_REUSEADDR` |
| tower / tower-http | 0.5 / 0.6 | Middleware (CORS, trace) |
| uuid | 1.x (v4) | Task/session ID generation |
| chrono | 0.4 | Timestamps (serde) |
| opentelemetry / opentelemetry-otlp | 0.27 | Tracing export (feature: `monitoring`) |
| dirs | 6.x | XDG directory resolution |
| libc | 0.2 | Signal handling |
| toml | 0.8 | Config serialization |

**18 direct dependencies.** Dev dependency: `approx = 0.5` (float comparison in tests).

### Lints

```toml
[lints.clippy]
pedantic = { level = "warn", priority = -1 }
unwrap_used = "deny"
expect_used = "deny"
```

### Release Profile

```toml
opt-level = 3
lto = "thin"
strip = "symbols"
```

---

## 3. Position in ULTRAPLATE

| Property | Value |
|----------|-------|
| Service ID | `orac-sidecar` |
| DevEnv Batch | 5 (last to start) |
| Depends on | `pane-vortex` (PV2, Batch 5), `povm-engine` (Batch 1) |
| Depended on by | Nothing (leaf service, proxy role) |
| Health path | `/health` |
| GitLab | `git@gitlab.com:lukeomahoney/orac-sidecar.git` |

### Upstream Dependencies (must be running)

| Service | Port | Required For |
|---------|------|-------------|
| Pane-Vortex V2 | 8132 | Field state, IPC bus, sphere data |
| POVM Engine | 8125 | Memory hydration, pathway persistence |
| SYNTHEX | 8090 | Thermal signal, heat source posting |
| Maintenance Engine | 8080 | Observer fitness, EventBus data |
| Reasoning Memory | 8130 | Cross-session TSV persistence |
| Vortex Memory System | 8120 | Semantic memory queries, consolidation |

### DevEnv Registration

```toml
[services.orac-sidecar]
name = "ORAC Sidecar"
command = "./bin/orac-sidecar"
working_dir = "/home/louranicas/claude-code-workspace/orac-sidecar"
port = 8133
health_path = "/health"
batch = 5
depends_on = ["pane-vortex", "povm-engine"]
```

---

## 4. Port and Socket Map

| Resource | Address | Protocol | Direction |
|----------|---------|----------|-----------|
| ORAC HTTP | `127.0.0.1:8133` | HTTP/1.1 (axum) | Inbound (hooks, queries) |
| PV2 HTTP | `127.0.0.1:8132` | HTTP/1.1 | Outbound (field state polling) |
| PV2 IPC | `/run/user/1000/pane-vortex-bus.sock` | Unix socket, NDJSON | Bidirectional (V2 wire protocol) |
| SYNTHEX | `127.0.0.1:8090` | Raw TCP HTTP | Outbound (thermal read/write) |
| ME | `127.0.0.1:8080` | Raw TCP HTTP | Outbound (observer poll) |
| POVM | `127.0.0.1:8125` | Raw TCP HTTP | Outbound (hydrate/persist) |
| RM | `127.0.0.1:8130` | Raw TCP HTTP (TSV body) | Outbound (TSV persist/search) |
| VMS | `127.0.0.1:8120` | Raw TCP HTTP | Outbound (memory/query/consolidation) |
| WASM FIFO | `/tmp/swarm-commands.pipe` | Named pipe (FIFO) | Inbound (WASM plugin commands) |
| WASM Ring | `/tmp/swarm-events.jsonl` | File (JSONL, 1000-line cap) | Outbound (events to WASM plugin) |
| Blackboard DB | `~/.local/share/orac/blackboard.db` | SQLite WAL | Local |
| Bus tracking DB | `data/bus_tracking.db` | SQLite WAL | Local |
| Field tracking DB | `data/field_tracking.db` | SQLite WAL | Local |

**Bridge calls are BLOCKING SYNC** -- all 6 bridges (SYNTHEX, ME, POVM, RM, VMS, PV2) use raw `TcpStream` via `tokio::spawn_blocking`, not fire-and-forget. Each bridge call blocks the spawned task until response or 2s TCP timeout.

---

## 5. Architecture Overview

### 8 Layers, 55 Files, 40 Modules + 2 Extra Files + 4 Binaries

```
Layer  Directory             Modules                        Feature Gate
-----  --------------------  -----------------------------  -----------
L1     src/m1_core/          m01-m06 + field_state (7 files)  (always)
L2     src/m2_wire/          m07-m09 (4 files)                (always)
L3     src/m3_hooks/         m10-m14 (6 files)                api
L4     src/m4_intelligence/  m15-m21 (8 files)                intelligence
L5     src/m5_bridges/       m22-m26 + http_helpers (7 files) bridges
L6     src/m6_coordination/  m27-m31 (6 files)                (always)
L7     src/m7_monitoring/    m32-m35 (5 files)                monitoring
L8     src/m8_evolution/     m36-m40 (6 files)                evolution
---    src/lib.rs            Layer declarations (1 file)      (always)
---    src/bin/              main, client, probe, ralph_bench (4 files)
```

**Total: 55 source files, ~41,369 LOC, ~1,748 tests.**

### Module Map

| ID | Module | Layer | Purpose |
|----|--------|-------|---------|
| m01 | `m01_core_types` | L1 | `PaneId`, `TaskId`, `PaneSphere`, `OrderParameter`, `FleetMode` |
| m02 | `m02_error_handling` | L1 | `PvError` (26 variants), `PvResult<T>` |
| m03 | `m03_config` | L1 | `PvConfig` (10 sections), figment loading, validation |
| m04 | `m04_constants` | L1 | 54 compile-time constants |
| m05 | `m05_traits` | L1 | `TensorContributor`, `FieldObserver` traits |
| m06 | `m06_validation` | L1 | Input validation, sanitization |
| -- | `field_state` | L1 | `FieldState`, `SharedState`, chimera detection, harmonics |
| m07 | `m07_ipc_client` | L2 | Unix socket IPC client, V2 wire FSM, reconnect |
| m08 | `m08_bus_types` | L2 | `BusFrame` (11 variants), `BusTask`, `BusEvent` |
| m09 | `m09_wire_protocol` | L2 | V2 wire protocol FSM, frame validation, keepalive |
| m10 | `m10_hook_server` | L3 | Axum router (22 routes), `OracState` (32 fields) |
| m11 | `m11_session_hooks` | L3 | `SessionStart`, `Stop` handlers |
| m12 | `m12_tool_hooks` | L3 | `PostToolUse`, `PreToolUse` handlers |
| m13 | `m13_prompt_hooks` | L3 | `UserPromptSubmit` handler |
| m14 | `m14_permission_policy` | L3 | `PermissionRequest` auto-approve/deny engine |
| m15 | `m15_coupling_network` | L4 | Kuramoto coupling weights, w^2 scaling |
| m16 | `m16_auto_k` | L4 | Auto-scale K adjustment |
| m17 | `m17_topology` | L4 | Network topology analysis |
| m18 | `m18_hebbian_stdp` | L4 | Spike-timing dependent plasticity, LTP/LTD |
| m19 | `m19_buoy_network` | L4 | Semantic buoy tunneling |
| m20 | `m20_semantic_router` | L4 | Content-aware dispatch (domain 40% + Hebbian 35% + availability 25%) |
| m21 | `m21_circuit_breaker` | L4 | Per-pane Closed/Open/HalfOpen FSM, `BreakerRegistry` |
| m22 | `m22_synthex_bridge` | L5 | Thermal read (`/v3/thermal`), field state posting |
| m23 | `m23_me_bridge` | L5 | Observer fitness read (`/api/observer`) |
| m24 | `m24_povm_bridge` | L5 | Memory hydration (`/hydrate`), pathway persistence |
| m25 | `m25_rm_bridge` | L5 | Cross-session TSV persistence (NOT JSON) |
| m26 | `m26_blackboard` | L5 | SQLite shared fleet state (9 tables) |
| -- | `http_helpers` | L5 | Raw TCP HTTP GET/POST (BUG-042 extraction) |
| m27 | `m27_conductor` | L6 | PI breathing controller, k_delta recommendations |
| m28 | `m28_cascade` | L6 | Cascade handoff dispatch |
| m29 | `m29_tick` | L6 | Tick orchestrator (5-phase: field, conductor, STDP, governance) |
| m30 | `m30_wasm_bridge` | L6 | FIFO/ring protocol bridge to Zellij WASM plugin |
| m31 | `m31_memory_manager` | L6 | Per-sphere memory lifecycle |
| m32 | `m32_otel_traces` | L7 | In-process trace store for OTel-style spans |
| m33 | `m33_metrics_export` | L7 | Prometheus text format metrics |
| m34 | `m34_field_dashboard` | L7 | Kuramoto field dashboard (r history, clusters, chimera) |
| m35 | `m35_token_accounting` | L7 | Per-pane/task token cost tracking, budget enforcement |
| m36 | `m36_ralph_engine` | L8 | 5-phase RALPH loop (Recognize/Analyze/Learn/Propose/Harvest) |
| m37 | `m37_emergence_detector` | L8 | 8 emergence types, ring buffer, TTL decay |
| m38 | `m38_correlation_engine` | L8 | Temporal, causal, recurring, fitness-linked correlations |
| m39 | `m39_fitness_tensor` | L8 | 12-dim weighted fitness, trend via linear regression |
| m40 | `m40_mutation_selector` | L8 | Round-robin mutation cycling, diversity rejection gate |

### `OracState` (32 Fields)

The central shared state struct (`Arc<OracState>`) passed to all axum handlers:

| # | Field | Type | Notes |
|---|-------|------|-------|
| 1 | `config` | `PvConfig` | Immutable after startup |
| 2 | `field_state` | `SharedState` | Cached from PV2 |
| 3 | `pv2_url` | `String` | `http://127.0.0.1:8132` |
| 4 | `synthex_url` | `String` | `http://127.0.0.1:8090` |
| 5 | `povm_url` | `String` | `http://127.0.0.1:8125` |
| 6 | `rm_url` | `String` | `http://127.0.0.1:8130` |
| 7 | `sessions` | `RwLock<HashMap<String, SessionTracker>>` | Per-session tracking |
| 8 | `tick` | `AtomicU64` | Global tick counter |
| 9 | `ipc_state` | `RwLock<String>` | IPC connection state |
| 10 | `ghosts` | `RwLock<VecDeque<OracGhost>>` | Deregistered sphere traces |
| 11 | `consents` | `RwLock<HashMap<String, OracConsent>>` | Per-sphere consent |
| 12 | `blackboard` | `Option<Mutex<Blackboard>>` | SQLite (feature: persistence) |
| 13 | `ralph` | `RalphEngine` | Evolution engine (feature: evolution) |
| 14 | `coupling` | `RwLock<CouplingNetwork>` | Hebbian coupling weights |
| 15 | `breakers` | `RwLock<BreakerRegistry>` | Circuit breakers (feature: intelligence) |
| 16 | `dispatch_total` | `AtomicU64` | Total dispatches |
| 17 | `dispatch_read` | `AtomicU64` | Read domain dispatches |
| 18 | `dispatch_write` | `AtomicU64` | Write domain dispatches |
| 19 | `dispatch_execute` | `AtomicU64` | Execute domain dispatches |
| 20 | `dispatch_communicate` | `AtomicU64` | Communicate domain dispatches |
| 21 | `co_activations_total` | `AtomicU64` | Co-activation events |
| 22 | `hebbian_ltp_total` | `AtomicU64` | LTP event count |
| 23 | `hebbian_ltd_total` | `AtomicU64` | LTD event count |
| 24 | `hebbian_last_tick` | `AtomicU64` | Last STDP tick |
| 25 | `me_bridge` | `MeBridge` | ME bridge (feature: bridges) |
| 26 | `rm_bridge` | `RmBridge` | RM bridge (feature: bridges) |
| 27 | `total_tool_calls` | `AtomicU64` | Global tool call counter |
| 28 | `tool_calls_at_last_thermal` | `AtomicU64` | Snapshot for rate calc |
| 29 | `synthex_bridge` | `SynthexBridge` | SYNTHEX bridge (feature: bridges) |
| 30 | `trace_store` | `TraceStore` | Span recording (feature: monitoring) |
| 31 | `dashboard` | `FieldDashboard` | Field dashboard (feature: monitoring) |
| 32 | `token_accountant` | `TokenAccountant` | Token budget (feature: monitoring) |

### 22 HTTP Routes

| Method | Path | Handler Module | Purpose |
|--------|------|---------------|---------|
| GET | `/health` | m10 | Liveness probe |
| GET | `/field` | m10 | Proxied PV2 Kuramoto field state |
| GET | `/thermal` | m10 | SYNTHEX thermal state |
| GET | `/blackboard` | m10 | Session state + RALPH snapshot |
| GET | `/metrics` | m10 | Prometheus text format |
| GET | `/field/ghosts` | m10 | Deregistered sphere traces |
| GET | `/traces` | m10 | OTel trace store query |
| GET | `/dashboard` | m10 | Kuramoto field dashboard |
| GET | `/tokens` | m10 | Token accounting summary |
| GET | `/coupling` | m10 | Coupling network state |
| GET | `/hebbian` | m10 | Hebbian STDP statistics |
| GET | `/emergence` | m10 | Emergence event history |
| GET | `/bridges` | m10 | Bridge health summary |
| GET | `/ralph` | m10 | RALPH engine state |
| GET | `/dispatch` | m10 | Dispatch statistics |
| GET | `/consent/{sphere_id}` | m10 | Read consent declaration |
| PUT | `/consent/{sphere_id}` | m10 | Update consent declaration |
| POST | `/hooks/SessionStart` | m11 | Register sphere, hydrate from POVM/RM |
| POST | `/hooks/Stop` | m11 | Deregister, crystallize, ghost trace |
| POST | `/hooks/PostToolUse` | m12 | Memory, status update, task poll |
| POST | `/hooks/PreToolUse` | m12 | SYNTHEX thermal gate |
| POST | `/hooks/UserPromptSubmit` | m13 | Inject r/tick/spheres/thermal |
| POST | `/hooks/PermissionRequest` | m14 | Auto-approve/deny policy |

### 11 `BusFrame` Variants (V2 Wire Protocol)

| Variant | Direction | Purpose |
|---------|-----------|---------|
| `Handshake` | Client -> Server | Identity + version |
| `Welcome` | Server -> Client | Session ID assignment |
| `Subscribe` | Client -> Server | Event pattern subscription |
| `Subscribed` | Server -> Client | Subscription confirmation |
| `Submit` | Client -> Server | Task submission |
| `TaskSubmitted` | Server -> Client | Task ID acknowledgement |
| `Event` | Server -> Client | Event notification |
| `Cascade` | Bidirectional | Handoff request (source, target, brief) |
| `CascadeAck` | Bidirectional | Handoff acknowledgement |
| `Disconnect` | Client -> Server | Graceful disconnect |
| `Error` | Server -> Client | Error notification (code + message) |

### 9 Blackboard Tables

| Table | Primary Key | Purpose |
|-------|------------|---------|
| `pane_status` | `pane_id TEXT` | Live pane state (status, tool, tick) |
| `task_history` | `task_id TEXT` | Task lifecycle records |
| `agent_cards` | `pane_id TEXT` | A2A-inspired capability declarations |
| `ghost_traces` | (none, `sphere_id` NOT NULL) | Deregistered sphere records |
| `consent_declarations` | `sphere_id TEXT` | Per-sphere consent state |
| `consent_audit` | (none, `sphere_id` NOT NULL) | Consent change audit log |
| `hebbian_summary` | `id INTEGER AUTOINCREMENT` | STDP weight snapshots |
| `ralph_state` | `id INTEGER CHECK (id=1)` | Singleton RALPH engine state |
| `sessions` | `session_id TEXT` | Session persistence across restarts |
| `coupling_weights` | (`from_id`, `to_id`) | Hebbian coupling weight persistence |

---

## 6. Feature Gate Summary

| Feature | Modules Enabled | Dependencies Pulled | Default |
|---------|----------------|--------------------|---------|
| `api` | L3 (m10-m14) | axum 0.8, tower-http 0.6 | Yes |
| `persistence` | m26 blackboard | rusqlite 0.32 | Yes |
| `bridges` | L5 (m22-m26) | (none extra) | Yes |
| `intelligence` | L4 (m15-m21) | tower 0.5 | Yes |
| `monitoring` | L7 (m32-m35) | opentelemetry 0.27, opentelemetry-otlp 0.27 | Yes |
| `evolution` | L8 (m36-m40) | (none extra) | Yes |
| `full` | All of the above | All of the above | No (alias) |

**All 6 features are in `default`.** The `full` feature is an explicit alias.

### Build Matrix

```
cargo build                          # all 6 features (default)
cargo build --features full          # identical to default
cargo build --no-default-features    # L1 + L2 + L6 only (minimal)
cargo build --features "api,bridges" # hooks + bridges, no intelligence/monitoring/evolution
```

---

## 7. Configuration

### `config/default.toml`

```toml
[server]
bind_addr = "127.0.0.1"
port = 8133

[ipc]
pv2_socket = "/run/user/1000/pane-vortex-bus.sock"
pv2_http = "http://127.0.0.1:8132"

[bridges]
synthex_addr = "127.0.0.1:8090"
me_addr = "127.0.0.1:8080"
povm_addr = "127.0.0.1:8125"
rm_addr = "127.0.0.1:8130"

[evolution]
enabled = false
emergence_cap = 5000
mutation_cooldown_generations = 10
diversity_window = 20
diversity_threshold = 0.5
```

### Environment Variable Overrides

Figment merges `PV2_*` environment variables over file config. Split on `_` for nested keys.

| Env Var | Config Path | Example |
|---------|------------|---------|
| `PV2_SERVER_PORT` | `server.port` | `8133` |
| `PV2_SERVER_BIND_ADDR` | `server.bind_addr` | `0.0.0.0` |
| `PV2_FIELD_TICK_INTERVAL_SECS` | `field.tick_interval_secs` | `10` |
| `PV2_BRIDGES_SYNTHEX_POLL_INTERVAL` | `bridges.synthex_poll_interval` | `12` |
| `PV2_BRIDGES_K_MOD_BUDGET_MIN` | `bridges.k_mod_budget_min` | `0.90` |

### Runtime Environment Variables (used directly)

| Var | Purpose | Example |
|-----|---------|---------|
| `PORT` | Override server port | `8133` |
| `PV2_ADDR` | PV2 HTTP address | `127.0.0.1:8132` |
| `SYNTHEX_ADDR` | SYNTHEX address | `127.0.0.1:8090` |
| `POVM_ADDR` | POVM address | `127.0.0.1:8125` |
| `RM_ADDR` | RM address | `127.0.0.1:8130` |
| `RUST_LOG` | Tracing filter | `orac_sidecar=info` |

### Loading Priority

1. `config/default.toml`
2. `config/production.toml` (overlay)
3. `PV2_*` environment variables (highest priority)

### `PvConfig` Sections (10 Sections)

`ServerConfig`, `FieldConfig`, `SphereConfig`, `CouplingConfig`, `LearningConfig`, `BridgesConfig`, `ConductorConfig`, `IpcConfig`, `PersistenceConfig`, `GovernanceConfig` -- all with `#[serde(default)]` for backward compatibility.

---

## 8. Quick Start

### Build

```bash
cd /home/louranicas/claude-code-workspace/orac-sidecar

# Development build (all features, default)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build

# Release build (3 binaries)
CARGO_TARGET_DIR=/tmp/cargo-orac cargo build --release
```

### Deploy

```bash
# Copy release binary
/usr/bin/cp -f /tmp/cargo-orac/release/orac-sidecar ~/.local/bin/orac-sidecar

# Verify binary
~/.local/bin/orac-sidecar --version 2>/dev/null || echo "no --version flag; check binary exists"
```

### Start (manual)

```bash
cd /home/louranicas/claude-code-workspace/orac-sidecar
RUST_LOG=orac_sidecar=info \
PORT=8133 \
PV2_ADDR=127.0.0.1:8132 \
SYNTHEX_ADDR=127.0.0.1:8090 \
POVM_ADDR=127.0.0.1:8125 \
RM_ADDR=127.0.0.1:8130 \
nohup ~/.local/bin/orac-sidecar > /tmp/orac-session.log 2>&1 &
```

### Start (via devenv)

```bash
~/.local/bin/devenv -c ~/.config/devenv/devenv.toml restart orac-sidecar
```

### Verify

```bash
# Health check
curl -s http://localhost:8133/health | python3 -m json.tool

# Field state
curl -s http://localhost:8133/field | python3 -m json.tool

# RALPH state
curl -s http://localhost:8133/ralph | python3 -m json.tool

# Bridge health
curl -s http://localhost:8133/bridges | python3 -m json.tool
```

### Explore

```bash
# Emergence events
curl -s http://localhost:8133/emergence | python3 -m json.tool

# Coupling network
curl -s http://localhost:8133/coupling | python3 -m json.tool

# Token budget
curl -s http://localhost:8133/tokens | python3 -m json.tool

# Dispatch stats
curl -s http://localhost:8133/dispatch | python3 -m json.tool
```

### Quality Gate (mandatory before any release)

```bash
CARGO_TARGET_DIR=/tmp/cargo-orac cargo check 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo clippy -- -D warnings -W clippy::pedantic 2>&1 | tail -20 && \
CARGO_TARGET_DIR=/tmp/cargo-orac cargo test --lib --release --features full 2>&1 | tail -30
```

---

## 9. Staleness Canary

Run these commands to verify this document matches the codebase. If any value diverges, this document needs updating.

```bash
# Source file count (expect: 55)
ls src/lib.rs src/bin/*.rs src/m1_core/*.rs src/m2_wire/*.rs src/m3_hooks/*.rs \
   src/m4_intelligence/*.rs src/m5_bridges/*.rs src/m6_coordination/*.rs \
   src/m7_monitoring/*.rs src/m8_evolution/*.rs 2>/dev/null | wc -l

# Total LOC (expect: ~41,369)
wc -l src/lib.rs src/bin/*.rs src/m1_core/*.rs src/m2_wire/*.rs src/m3_hooks/*.rs \
   src/m4_intelligence/*.rs src/m5_bridges/*.rs src/m6_coordination/*.rs \
   src/m7_monitoring/*.rs src/m8_evolution/*.rs 2>/dev/null | tail -1

# Test count (expect: ~1,748)
rg '#\[test\]' src/ tests/ benches/ --count-matches 2>/dev/null | \
   awk -F: '{sum+=$2} END {print sum}'

# HTTP route count (expect: 22+)
rg '\.route\(' src/m3_hooks/m10_hook_server.rs --count-matches

# BusFrame variant count (expect: 11)
rg '^\s+///.*→' src/m2_wire/m08_bus_types.rs | wc -l

# OracState field count (expect: 32)
rg '^\s+pub ' src/m3_hooks/m10_hook_server.rs | \
   sed -n '/pub struct OracState/,/^impl/p' | grep 'pub ' | wc -l

# Blackboard table count (expect: 10 -- 9 original + coupling_weights)
rg 'CREATE TABLE' src/m5_bridges/m26_blackboard.rs --count-matches

# Feature count (expect: 7 including full)
rg '^\w+ =' Cargo.toml | grep -c 'feature'
```

---

*Generated: 2026-03-25 | Source: `/home/louranicas/claude-code-workspace/orac-sidecar/`*
*Obsidian backlinks: `[[Session 061 -- ORAC System Atlas]]`, `[[ORAC Sidecar -- Architecture Schematics]]`, `[[Session 056 -- ORAC God-Tier Mastery]]`, `[[ULTRAPLATE Master Index]]`*
