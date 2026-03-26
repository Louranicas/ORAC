# R1: ORAC Sidecar Constants & Configuration Reference

> **Source of truth:** Every value in this document comes from actual source code and configuration files.
> **Generated from:** `src/m1_core/m04_constants.rs`, `config/default.toml`, `config/hooks.toml`, `config/bridges.toml`, and module-local constants across 40 modules.
> **Date:** 2026-03-25 | **Commit context:** Session 060+

---

## 1. Core Constants (`m1_core/m04_constants.rs`)

### 1.1 Tick Timing

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `TICK_INTERVAL_SECS` | `5` | `u64` | Default tick interval in seconds. Configurable via `field.tick_interval_secs`. |
| `COUPLING_STEPS_PER_TICK` | `15` | `usize` | Kuramoto coupling integration steps per tick. |
| `KURAMOTO_DT` | `0.01` | `f64` | Euler integration timestep for Kuramoto phase dynamics. |

### 1.2 Hebbian Learning

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `HEBBIAN_LTP` | `0.01` | `f64` | Long-term potentiation rate (weight increase). |
| `HEBBIAN_LTD` | `0.002` | `f64` | Long-term depression rate (weight decrease). |
| `HEBBIAN_BURST_MULTIPLIER` | `3.0` | `f64` | LTP multiplier during burst activity. |
| `HEBBIAN_NEWCOMER_MULTIPLIER` | `2.0` | `f64` | LTP multiplier for newcomer spheres (first `NEWCOMER_STEPS` ticks). |
| `HEBBIAN_WEIGHT_FLOOR` | `0.15` | `f64` | Minimum coupling weight (prevents complete disconnection). |

### 1.3 Coupling Network

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_WEIGHT` | `0.18` | `f64` | Default connection weight for new sphere pairs. |
| `WEIGHT_EXPONENT` | `2.0` | `f64` | Weight scaling exponent (fixed `w^2`). |

### 1.4 Field Thresholds

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `PHASE_GAP_THRESHOLD` | `π/3` (~1.047) | `f64` | Phase gap for chimera detection (re-exported from `FRAC_PI_3`). |
| `SYNC_THRESHOLD` | `0.5` | `f64` | Order parameter `r` above which the field is synchronized. |
| `TUNNEL_THRESHOLD` | `0.8` | `f64` | Angular distance (radians) below which buoys form a tunnel. |
| `R_HIGH_THRESHOLD` | `0.8` | `f64` | `r` above which the field is highly coherent. |
| `R_LOW_THRESHOLD` | `0.3` | `f64` | `r` below which the field is incoherent. |
| `R_FALLING_THRESHOLD` | `-0.03` | `f64` | `r` trend below which `RTrend::Falling` triggers. |
| `R_RISING_THRESHOLD` | `0.03` | `f64` | `r` trend above which `RTrend::Rising` triggers. |
| `IDLE_RATIO_THRESHOLD` | `0.6` | `f64` | Fraction of idle spheres above which `IdleFleet` action triggers. |

### 1.5 R Target Dynamics

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `R_TARGET_BASE` | `0.93` | `f64` | Base `r` target for small/medium fleets. |
| `R_TARGET_LARGE_FLEET` | `0.85` | `f64` | `r` target for large fleets (>50 spheres). |
| `LARGE_FLEET_THRESHOLD` | `50.0` | `f64` | Sphere count above which `R_TARGET_LARGE_FLEET` applies. |

### 1.6 Conductor (Breathing Controller)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `CONDUCTOR_GAIN` | `0.15` | `f64` | Proportional gain for the PI breathing controller. |
| `EMERGENT_BLEND` | `0.3` | `f64` | Fraction of emergent signal blended into conductor output. |

### 1.7 K Modulation Bounds

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `K_MOD_MIN` | `-0.5` | `f64` | Minimum `k` modulation value (absolute floor). |
| `K_MOD_MAX` | `1.5` | `f64` | Maximum `k` modulation value (absolute ceiling). |
| `K_MOD_BUDGET_MIN` | `0.85` | `f64` | Combined external influence floor (budget constraint). |
| `K_MOD_BUDGET_MAX` | `1.15` | `f64` | Combined external influence ceiling (budget constraint). |

**Invariants:** `K_MOD_MIN < K_MOD_BUDGET_MIN < 1.0 < K_MOD_BUDGET_MAX < K_MOD_MAX`.

### 1.8 Sphere Limits

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `SPHERE_CAP` | `200` | `usize` | Maximum number of spheres (prevents O(N^2) exhaustion). |
| `MEMORY_MAX_COUNT` | `500` | `usize` | Maximum memories per sphere. |
| `GHOST_MAX` | `20` | `usize` | Maximum ghost traces retained. |
| `LOG_MAX` | `1000` | `usize` | Maximum entries in the message log. |
| `INBOX_MAX` | `50` | `usize` | Maximum inbox messages per sphere. |
| `R_HISTORY_MAX` | `60` | `usize` | Maximum `r` history samples retained. |
| `DECISION_HISTORY_MAX` | `100` | `usize` | Maximum decision history records. |

### 1.9 Sphere Dynamics

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DECAY_PER_STEP` | `0.995` | `f64` | Memory activation decay per tick (multiplicative, sub-unitary). |
| `SWEEP_BOOST` | `0.05` | `f64` | Boost strength from sweep activation. |
| `ACTIVATION_THRESHOLD` | `0.3` | `f64` | Activation below which memories are prunable. |
| `MEMORY_PRUNE_INTERVAL` | `200` | `u64` | Steps between memory prune checks. |
| `SEMANTIC_NUDGE_STRENGTH` | `0.02` | `f64` | Gentle semantic nudge (doesn't override coupling). |
| `NEWCOMER_STEPS` | `50` | `u64` | Ticks during which a newcomer gets boosted LTP. |

### 1.10 Persistence

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `SNAPSHOT_INTERVAL` | `60` | `u64` | Ticks between field snapshots. |
| `WARMUP_TICKS` | `5` | `u32` | Warmup ticks after snapshot restore (reduced dynamics). |

### 1.11 Network / Server

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_PORT` | `8132` | `u16` | Default HTTP server port (PV2 port; ORAC uses 8133 via config). |
| `BIND_MAX_RETRIES` | `5` | `u32` | Maximum bind retry attempts. |
| `BIND_INITIAL_DELAY_MS` | `500` | `u64` | Initial delay between bind retries (milliseconds). |

### 1.12 Mathematical Constants

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `TWO_PI` | `6.283...` (TAU) | `f64` | Full circle in radians. Re-export of `std::f64::consts::TAU`. |

---

## 2. Wire Protocol Limits (`m2_wire/`)

### 2.1 Protocol (`m09_wire_protocol.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `PROTOCOL_VERSION` | `"2.0"` | `&str` | V2 wire protocol version string. |
| `DEFAULT_KEEPALIVE_SECS` | `30` | `u64` | Keepalive interval for wire connections. |
| `MAX_FRAME_SIZE` | `65_536` | `usize` | Maximum wire frame size (64KB). |
| `MAX_SEND_QUEUE` | `1000` | `usize` | Maximum outbound message queue depth. |
| `MAX_RECV_BUFFER` | `500` | `usize` | Maximum inbound message buffer depth. |

### 2.2 IPC Client (`m07_ipc_client.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_SOCKET_PATH` | `"/run/user/1000/pane-vortex-bus.sock"` | `&str` | Default PV2 Unix domain socket path. |
| `PROTOCOL_VERSION` | `"2.0"` | `&str` | IPC handshake protocol version. |
| `HANDSHAKE_TIMEOUT_SECS` | `5` | `u64` | Timeout for handshake completion. |
| `SUBSCRIBE_TIMEOUT_SECS` | `5` | `u64` | Timeout for subscription confirmation. |
| `RECV_TIMEOUT_SECS` | `300` | `u64` | Timeout for receive operations (5 minutes). |
| `BACKOFF_INITIAL_MS` | `100` | `u64` | Initial reconnect backoff delay. |
| `BACKOFF_MAX_MS` | `5000` | `u64` | Maximum reconnect backoff delay. |
| `BACKOFF_MAX_ATTEMPTS` | `10` | `u32` | Maximum reconnect attempts before giving up. |

### 2.3 WASM Bridge (`m6_coordination/m30_wasm_bridge.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_FIFO_PATH` | `"/tmp/swarm-commands.pipe"` | `&str` | WASM-to-sidecar command FIFO path. |
| `DEFAULT_RING_PATH` | `"/tmp/swarm-events.jsonl"` | `&str` | Sidecar-to-WASM event ring path. |
| `RING_LINE_CAP` | `1000` | `usize` | Maximum lines in event ring (FIFO eviction). |
| `MAX_COMMAND_LEN` | `8192` | `usize` | Maximum command message length. |
| `MAX_EVENT_LEN` | `8192` | `usize` | Maximum event message length. |

---

## 3. Bridge Timeouts

### 3.1 Config File: `config/bridges.toml`

| Bridge | URL | Poll Interval (ms) | Retry Count | Timeout (ms) | Consent Required |
|--------|-----|---------------------|-------------|--------------|------------------|
| `reasoning_memory` | `http://127.0.0.1:8130` | `5000` | `3` | `3000` | `false` |
| `pane_vortex` | `http://127.0.0.1:8132` | `2000` | `3` | `3000` | `false` |
| `vortex_memory` | `http://127.0.0.1:8120` | `10000` | `2` | `5000` | `true` |
| `synthex` | `http://127.0.0.1:8090` | `10000` | `2` | `5000` | `true` |

### 3.2 Config File: `config/default.toml`

| Section | Field | Value | Purpose |
|---------|-------|-------|---------|
| `[server]` | `bind_addr` | `"127.0.0.1"` | HTTP bind address. |
| `[server]` | `port` | `8133` | ORAC HTTP server port. |
| `[ipc]` | `pv2_socket` | `"/run/user/1000/pane-vortex-bus.sock"` | PV2 Unix domain socket. |
| `[ipc]` | `pv2_http` | `"http://127.0.0.1:8132"` | PV2 HTTP fallback URL. |
| `[bridges]` | `synthex_addr` | `"127.0.0.1:8090"` | SYNTHEX bridge address (raw SocketAddr, no `http://`). |
| `[bridges]` | `me_addr` | `"127.0.0.1:8080"` | Maintenance Engine bridge address. |
| `[bridges]` | `povm_addr` | `"127.0.0.1:8125"` | POVM Engine bridge address. |
| `[bridges]` | `rm_addr` | `"127.0.0.1:8130"` | Reasoning Memory bridge address. |

### 3.3 Per-Bridge Source Constants

#### SYNTHEX Bridge (`m5_bridges/m22_synthex_bridge.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `SYNTHEX_PORT` | `8090` | `u16` | Default SYNTHEX port. |
| `DEFAULT_BASE_URL` | `"127.0.0.1:8090"` | `&str` | Default SYNTHEX address. |
| `HEALTH_PATH` | `"/api/health"` | `&str` | Health check endpoint. |
| `THERMAL_PATH` | `"/v3/thermal"` | `&str` | Thermal state endpoint. |
| `INGEST_PATH` | `"/api/ingest"` | `&str` | Data ingest endpoint. |
| `DEFAULT_POLL_INTERVAL` | `6` | `u64` | Poll interval (ticks). |

#### Maintenance Engine Bridge (`m5_bridges/m23_me_bridge.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `ME_PORT` | `8080` | `u16` | Default ME port. |
| `DEFAULT_BASE_URL` | `"127.0.0.1:8080"` | `&str` | Default ME address. |
| `HEALTH_PATH` | `"/api/health"` | `&str` | Health check endpoint. |
| `OBSERVER_PATH` | `"/api/observer"` | `&str` | Observer/fitness endpoint. |
| `DEFAULT_POLL_INTERVAL` | `12` | `u64` | Poll interval (ticks). |
| `FROZEN_TOLERANCE` | `0.003` | `f64` | Fitness delta below which metric is considered frozen. |
| `FROZEN_THRESHOLD` | `3` | `u32` | Consecutive frozen polls before declaring metric frozen. |

#### POVM Bridge (`m5_bridges/m24_povm_bridge.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `POVM_PORT` | `8125` | `u16` | Default POVM port. |
| `DEFAULT_BASE_URL` | `"127.0.0.1:8125"` | `&str` | Default POVM address. |
| `HEALTH_PATH` | `"/health"` | `&str` | Health check endpoint. |
| `MEMORIES_PATH` | `"/memories"` | `&str` | Memories API endpoint. |
| `PATHWAYS_PATH` | `"/pathways"` | `&str` | Pathways API endpoint. |
| `SUMMARY_PATH` | `"/summary"` | `&str` | Summary API endpoint. |
| `DEFAULT_WRITE_INTERVAL` | `12` | `u64` | STDP write interval (ticks). |
| `DEFAULT_READ_INTERVAL` | `60` | `u64` | Hydration read interval (ticks). |
| `MAX_RESPONSE_SIZE` | `2_097_152` | `usize` | Maximum response body size (2MB). |

#### Reasoning Memory Bridge (`m5_bridges/m25_rm_bridge.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `RM_PORT` | `8130` | `u16` | Default RM port. |
| `DEFAULT_BASE_URL` | `"127.0.0.1:8130"` | `&str` | Default RM address. |
| `HEALTH_PATH` | `"/health"` | `&str` | Health check endpoint. |
| `PUT_PATH` | `"/put"` | `&str` | TSV write endpoint. |
| `SEARCH_PATH` | `"/search"` | `&str` | Search endpoint. |
| `DEFAULT_POLL_INTERVAL` | `30` | `u64` | Poll interval (ticks). |
| `DEFAULT_FIELD_STATE_TTL` | `300` | `u64` | Field state TTL in ticks. |
| `DEFAULT_AGENT` | `"orac-sidecar"` | `&str` | Agent identifier for RM records. |

#### HTTP Helpers (`m5_bridges/http_helpers.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_TCP_TIMEOUT_MS` | `2000` | `u64` | Default TCP connect timeout for all bridge HTTP calls. |
| `DEFAULT_MAX_RESPONSE_SIZE` | `32_768` | `usize` | Default max response body (32KB). |

---

## 4. Evolution Parameters (`m8_evolution/`)

### 4.1 RALPH Engine (`m36_ralph_engine.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_ACCEPT_THRESHOLD` | `0.02` | `f64` | Minimum fitness improvement to accept a mutation. |
| `DEFAULT_ROLLBACK_THRESHOLD` | `-0.01` | `f64` | Fitness regression threshold that triggers rollback. |
| `DEFAULT_VERIFICATION_TICKS` | `10` | `u64` | Ticks to wait before verifying mutation effect. |
| `DEFAULT_MAX_CYCLES` | `1000` | `u64` | Maximum RALPH cycles before auto-pause. |
| `DEFAULT_SNAPSHOT_CAPACITY` | `50` | `usize` | Maximum snapshots for rollback history. |

### 4.2 Emergence Detector (`m37_emergence_detector.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_HISTORY_CAPACITY` | `5000` | `usize` | Maximum emergence events in ring buffer. |
| `DEFAULT_TTL_TICKS` | `600` | `u64` | Tick TTL for emergence events (auto-decay). |
| `DEFAULT_MIN_CONFIDENCE` | `0.6` | `f64` | Minimum confidence for emergence detection. |
| `MAX_MONITORS` | `50` | `usize` | Maximum emergence monitors. |
| `DEFAULT_COHERENCE_LOCK_R` | `0.92` | `f64` | `r` threshold for CoherenceLock detection. |
| `DEFAULT_COHERENCE_LOCK_TICKS` | `10` | `u64` | Consecutive ticks above threshold to trigger CoherenceLock. |
| `DEFAULT_RUNAWAY_WINDOW` | `20` | `u64` | Tick window for CouplingRunaway detection. |
| `DEFAULT_SATURATION_RATIO` | `0.8` | `f64` | Weight ratio for HebbianSaturation detection. |
| `BENEFICIAL_SYNC_R` | `0.78` | `f64` | `r` threshold for BeneficialSync detection. |
| `BENEFICIAL_SYNC_IMPROVEMENT` | `0.005` | `f64` | Minimum `r` improvement per tick for BeneficialSync. |
| `FIELD_STABILITY_R` | `0.65` | `f64` | `r` threshold for FieldStability detection. |
| `FIELD_STABILITY_WINDOW` | `12` | `usize` | Tick window for FieldStability variance calculation. |

### 4.3 Correlation Engine (`m38_correlation_engine.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_WINDOW_TICKS` | `30` | `u64` | Sliding window for temporal correlation. |
| `DEFAULT_MAX_BUFFER` | `10_000` | `usize` | Maximum observation buffer size. |
| `DEFAULT_MIN_CONFIDENCE` | `0.5` | `f64` | Minimum confidence for pathway establishment. |
| `DEFAULT_MIN_RECURRING_COUNT` | `3` | `u32` | Occurrences needed to establish a recurring pattern. |
| `DEFAULT_HISTORY_CAPACITY` | `1000` | `usize` | Maximum correlation history records. |
| `MAX_PATHWAYS` | `500` | `usize` | Maximum discovered pathways. |

### 4.4 Mutation Selector (`m40_mutation_selector.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_COOLDOWN_GENERATIONS` | `10` | `u64` | Generations before re-mutating the same parameter. |
| `DEFAULT_DIVERSITY_WINDOW` | `20` | `usize` | Lookback window for diversity check. |
| `DEFAULT_DIVERSITY_THRESHOLD` | `0.5` | `f64` | Max fraction of same parameter in window before rejection. |
| `DEFAULT_MAX_DELTA` | `0.20` | `f64` | Maximum mutation delta magnitude. |
| `DEFAULT_MIN_DELTA` | `0.001` | `f64` | Minimum mutation delta magnitude. |
| `DEFAULT_HISTORY_CAPACITY` | `1000` | `usize` | Maximum mutation history records. |

### 4.5 Config File: `config/default.toml` (`[evolution]`)

| Field | Value | Type | Purpose |
|-------|-------|------|---------|
| `enabled` | `false` | `bool` | Whether RALPH evolution is enabled at startup. |
| `emergence_cap` | `5000` | `u64` | Maximum emergence events (matches `DEFAULT_HISTORY_CAPACITY`). |
| `mutation_cooldown_generations` | `10` | `u64` | Matches `DEFAULT_COOLDOWN_GENERATIONS`. |
| `diversity_window` | `20` | `u64` | Matches `DEFAULT_DIVERSITY_WINDOW`. |
| `diversity_threshold` | `0.5` | `f64` | Matches `DEFAULT_DIVERSITY_THRESHOLD`. |

---

## 5. Coupling Parameters

### 5.1 Coupling Network (`m4_intelligence/m15_coupling_network.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `COUPLING_SUM_CAP` | `3.0` | `f64` | Maximum sum of coupling weights per sphere (prevents runaway). |

### 5.2 Hebbian STDP (`m4_intelligence/m18_hebbian_stdp.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `HEBBIAN_SOFT_CEILING` | `0.85` | `f64` | Soft weight ceiling for Hebbian learning (above floor but below 1.0). |

### 5.3 Semantic Router (`m4_intelligence/m20_semantic_router.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DOMAIN_WEIGHT` | `0.4` | `f64` | Domain affinity weight in composite routing score. |
| `HEBBIAN_WEIGHT` | `0.35` | `f64` | Hebbian coupling weight in composite routing score. |
| `AVAILABILITY_WEIGHT` | `0.25` | `f64` | Pane availability weight in composite routing score. |
| `PREFERRED_BONUS` | `0.15` | `f64` | Bonus score for preferred pane in routing. |

---

## 6. Tick Intervals (Summary)

All tick intervals expressed in **ticks** (each tick = `TICK_INTERVAL_SECS` = 5 seconds by default).

| Operation | Interval (ticks) | Wall Clock (default) | Source |
|-----------|-------------------|----------------------|--------|
| Kuramoto coupling steps | 15 per tick | per 5s | `COUPLING_STEPS_PER_TICK` |
| SYNTHEX poll | 6 | 30s | `m22_synthex_bridge.rs` |
| ME observer poll | 12 | 60s | `m23_me_bridge.rs` |
| POVM STDP write | 12 | 60s | `m24_povm_bridge.rs` |
| POVM hydration read | 60 | 5m | `m24_povm_bridge.rs` |
| RM poll | 30 | 2.5m | `m25_rm_bridge.rs` |
| RM field state TTL | 300 | 25m | `m25_rm_bridge.rs` |
| Memory prune check | 200 | ~16.7m | `MEMORY_PRUNE_INTERVAL` |
| Field snapshots | 60 | 5m | `SNAPSHOT_INTERVAL` |
| Newcomer boost | 50 | ~4.2m | `NEWCOMER_STEPS` |
| PostToolUse task poll | every 5th call | — | `m12_tool_hooks.rs:POLL_EVERY_N` |
| PV2 bridge config poll | 2000ms | 2s | `bridges.toml` |
| RM bridge config poll | 5000ms | 5s | `bridges.toml` |
| VMS bridge config poll | 10000ms | 10s | `bridges.toml` |
| SYNTHEX bridge config poll | 10000ms | 10s | `bridges.toml` |

---

## 7. Circuit Breaker Thresholds (`m4_intelligence/m21_circuit_breaker.rs`)

### 7.1 Default Profile

| Field | Value | Purpose |
|-------|-------|---------|
| `failure_threshold` | `5` | Consecutive failures before opening the circuit. |
| `success_threshold` | `2` | Consecutive successes in HalfOpen to close the circuit. |
| `open_timeout_ticks` | `30` | Ticks in Open state before transitioning to HalfOpen. |
| `half_open_max_requests` | `1` | Requests allowed through during HalfOpen probe. |

### 7.2 Aggressive Profile

| Field | Value | Purpose |
|-------|-------|---------|
| `failure_threshold` | `3` | Faster trip for latency-sensitive paths. |
| `success_threshold` | `1` | Single success closes the circuit. |
| `open_timeout_ticks` | `15` | Shorter recovery wait. |
| `half_open_max_requests` | `1` | Single probe request. |

### 7.3 Tolerant Profile

| Field | Value | Purpose |
|-------|-------|---------|
| `failure_threshold` | `10` | Higher tolerance for slow/flaky services. |
| `success_threshold` | `3` | Requires 3 consecutive successes to close. |
| `open_timeout_ticks` | `60` | Longer recovery wait. |
| `half_open_max_requests` | `2` | Two probe requests allowed. |

### 7.4 State Machine

```
Closed --[failure >= threshold]--> Open --[timeout]--> HalfOpen
  ^                                                       |
  |              probe succeeds (>= success_threshold)    |
  +-------------------------------------------------------+
                 probe fails --> back to Open
```

---

## 8. Fitness Tensor (`m8_evolution/m39_fitness_tensor.rs`)

### 8.1 Dimension Layout (12D)

| D# | Name | Weight | Category | Notes |
|----|------|--------|----------|-------|
| D0 | `coordination_quality` | 0.18 | Primary | |
| D1 | `field_coherence` | 0.15 | Primary | |
| D2 | `dispatch_accuracy` | 0.12 | Primary | |
| D3 | `task_throughput` | 0.10 | Secondary | |
| D4 | `error_rate` | 0.10 | Secondary | Inverted: lower = better |
| D5 | `latency` | 0.08 | Secondary | Inverted: lower = better |
| D6 | `hebbian_health` | 0.07 | Learning | |
| D7 | `coupling_stability` | 0.06 | Learning | |
| D8 | `thermal_balance` | 0.05 | Context | |
| D9 | `fleet_utilization` | 0.04 | Context | |
| D10 | `emergence_rate` | 0.03 | Context | |
| D11 | `consent_compliance` | 0.02 | Context | |

**Weight sum:** 1.00

### 8.2 Trend Analysis Constants

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_HISTORY_CAPACITY` | `200` | `usize` | Maximum fitness history snapshots. |
| `DEFAULT_TREND_WINDOW` | `10` | `usize` | Window size for linear regression trend. |
| `DEFAULT_STABILITY_TOLERANCE` | `0.02` | `f64` | Stddev below which fitness is "stable". |
| `DEFAULT_VOLATILITY_THRESHOLD` | `0.10` | `f64` | Stddev above which fitness is "volatile". |
| `DEFAULT_MIN_IMPROVEMENT` | `0.02` | `f64` | Minimum improvement for RALPH to accept a mutation. |

---

## 9. Hook Configuration (`config/hooks.toml`)

### 9.1 Timeouts

| Event | Timeout (ms) | Purpose |
|-------|-------------|---------|
| `pre_tool_use_ms` | `2000` | Max wait for PreToolUse hook evaluation. |
| `post_tool_use_ms` | `1000` | Max wait for PostToolUse processing. |
| `pre_compact_ms` | `5000` | Max wait for PreCompact cascade dispatch. |
| `notification_ms` | `500` | Max wait for notification delivery. |

### 9.2 Auto-Approve Patterns

The following tool patterns bypass full ORAC evaluation (glob syntax):

- `Read`
- `Glob`
- `Grep`
- `Bash:ls *`
- `Bash:git status*`
- `Bash:git diff*`
- `Bash:git log*`

### 9.3 Thermal Throttling

| Field | Value | Purpose |
|-------|-------|---------|
| `warn_temp` | `0.7` | Temperature above which hooks log warnings. |
| `critical_temp` | `0.9` | Temperature above which hooks are throttled. |
| `cooldown_ticks` | `10` | Ticks to remain in cooldown after critical temp. |

---

## 10. Monitoring Constants (`m7_monitoring/`)

### 10.1 OTel Traces (`m32_otel_traces.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `MAX_SPANS` | `10_000` | `usize` | Maximum trace spans in memory. |
| `MAX_ATTRIBUTES` | `32` | `usize` | Maximum attributes per span. |
| `MAX_SPAN_NAME_LEN` | `256` | `usize` | Maximum span name length. |
| `DEFAULT_BATCH_SIZE` | `100` | `usize` | Default export batch size. |
| `STATUS_UNSET` | `0` | `u8` | Span status: unset. |
| `STATUS_OK` | `1` | `u8` | Span status: OK. |
| `STATUS_ERROR` | `2` | `u8` | Span status: error. |

### 10.2 Metrics Export (`m33_metrics_export.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `MAX_LABEL_KEY_LEN` | `64` | `usize` | Maximum metric label key length. |
| `MAX_LABEL_VALUE_LEN` | `128` | `usize` | Maximum metric label value length. |
| `LATENCY_BUCKETS` | `[0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0]` | `[f64; 10]` | Histogram bucket boundaries (milliseconds). |

### 10.3 Field Dashboard (`m34_field_dashboard.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `R_HISTORY_MAX` | `60` | `usize` | Re-imports from `m04_constants`. |
| `MAX_CLUSTERS` | `32` | `usize` | Maximum phase clusters tracked. |
| `MAX_SPHERES` | `200` | `usize` | Re-imports `SPHERE_CAP`. |
| `PHASE_GAP_THRESHOLD` | `π/3` | `f64` | Re-imports from `m04_constants`. |

### 10.4 Token Accounting (`m35_token_accounting.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `DEFAULT_INPUT_COST` | `0.000015` | `f64` | Cost per input token (USD). |
| `DEFAULT_OUTPUT_COST` | `0.000075` | `f64` | Cost per output token (USD). |
| `MAX_TRACKED_PANES` | `256` | `usize` | Maximum panes tracked for token accounting. |
| `DEFAULT_SOFT_LIMIT` | `10.0` | `f64` | Soft budget limit (USD). |
| `DEFAULT_HARD_LIMIT` | `50.0` | `f64` | Hard budget limit (USD). |
| `MAX_TASK_RECORDS` | `5_000` | `usize` | Maximum task records in accounting history. |

---

## 11. Validation Limits (`m1_core/m06_validation.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `PANE_ID_MAX_LEN` | `128` | `usize` | Maximum pane ID string length. |
| `PERSONA_MAX_LEN` | `256` | `usize` | Maximum persona string length. |
| `TOOL_NAME_MAX_LEN` | `128` | `usize` | Maximum tool name string length. |
| `SUMMARY_MAX_LEN` | `1024` | `usize` | Maximum summary string length. |

---

## 12. Field State (`m1_core/field_state.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `CLUSTER_PROXIMITY` | `π/6` (~0.524) | `f64` | Angular proximity for cluster membership. |
| `CHIMERA_GAP` | `π/3` (~1.047) | `f64` | Phase gap for chimera state detection. |
| `CHIMERA_R_THRESHOLD` | `0.95` | `f64` | `r` threshold above which chimera detection is skipped. |
| `WARMUP_TICKS` | `5` | `u32` | Re-imports from `m04_constants`. |
| `ALPHA` | `0.2` | `f64` | EMA smoothing factor for field trend. |
| `STALE_THRESHOLD` | `3` | `u32` | Consecutive poll misses before field state is considered stale. |

---

## 13. Cascade Coordination (`m6_coordination/m28_cascade.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `MAX_CASCADES_PER_MINUTE` | `10` | `u32` | Rate limit: cascades per minute. |
| `RATE_WINDOW_SECS` | `60.0` | `f64` | Rate limit window duration. |
| `MAX_PENDING_CASCADES` | `50` | `usize` | Maximum pending cascades. |
| `AUTO_SUMMARIZE_DEPTH` | `3` | `u32` | Cascade depth at which auto-summarization triggers. |
| `MAX_BRIEF_CHARS` | `4096` | `usize` | Maximum handoff brief character length. |

---

## 14. Conductor (`m6_coordination/m27_conductor.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `_DIVERGENCE_COOLDOWN_TICKS` | `3` | `u32` | Cooldown ticks after divergence correction (prefixed `_`, currently unused). |
| `MIN_SPHERES_FOR_BREATHING` | `3` | `usize` | Minimum sphere count before breathing controller activates. |

---

## 15. Hook Server (`m3_hooks/`)

### 15.1 Hook Server (`m10_hook_server.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `MAX_GHOSTS` | `20` | `usize` | Maximum ghost records (matches `GHOST_MAX`). |
| `CONSENT_DEFAULT_TIMESTAMP` | `0` | `u64` | Default consent timestamp (epoch = "never consented"). |

### 15.2 Tool Hooks (`m12_tool_hooks.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `POLL_EVERY_N` | `5` | `u64` | PostToolUse polls for pending tasks every Nth call. |

### 15.3 Prompt Hooks (`m13_prompt_hooks.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `MIN_PROMPT_LENGTH` | `20` | `usize` | Minimum prompt character length for field injection. |

---

## 16. Binary Constants

### 16.1 Main Binary (`bin/main.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `EMERGENCE_HISTORY_CAP` | `100` | `usize` | Emergence event display cap for health endpoint. |
| `RECONNECT_BASE_SECS` | `5` | `u64` | IPC reconnect base delay. |
| `RECONNECT_CAP_SECS` | `30` | `u64` | IPC reconnect maximum delay. |

### 16.2 Client Binary (`bin/client.rs`)

| Constant | Value | Type | Purpose |
|----------|-------|------|---------|
| `ORAC_ADDR` | `"127.0.0.1:8133"` | `&str` | Default ORAC server address. |
| `TIMEOUT` | `3s` | `Duration` | HTTP request timeout for client commands. |

---

## Appendix A: Port Map

| Port | Service | Source |
|------|---------|--------|
| 8080 | Maintenance Engine | `m23_me_bridge.rs`, `default.toml` |
| 8090 | SYNTHEX | `m22_synthex_bridge.rs`, `default.toml` |
| 8120 | Vortex Memory System | `bridges.toml` |
| 8125 | POVM Engine | `m24_povm_bridge.rs`, `default.toml` |
| 8130 | Reasoning Memory | `m25_rm_bridge.rs`, `default.toml` |
| 8132 | Pane-Vortex V2 | `m04_constants.rs:DEFAULT_PORT`, `default.toml` |
| 8133 | ORAC Sidecar | `default.toml:server.port`, `client.rs:ORAC_ADDR` |

## Appendix B: Invariant Summary

From the test suite in `m04_constants.rs`:

- `R_LOW_THRESHOLD < R_HIGH_THRESHOLD < R_TARGET_BASE`
- `K_MOD_MIN < K_MOD_BUDGET_MIN < 1.0 < K_MOD_BUDGET_MAX < K_MOD_MAX`
- `HEBBIAN_LTP > HEBBIAN_LTD`
- `R_TARGET_LARGE_FLEET < R_TARGET_BASE`
- `DECAY_PER_STEP` is in (0.0, 1.0)
- `ACTIVATION_THRESHOLD` is in (0.0, 1.0)
- `PHASE_GAP_THRESHOLD == π/3`
- `TWO_PI == TAU`
