# D6: ORAC Sidecar -- Capacity and Limits Reference

> **54 compile-time constants + runtime defaults | Every value traced to source file:line**
>
> Obsidian: `[[Session 061 -- ORAC System Atlas]]` | `[[ORAC Sidecar -- Architecture Schematics]]`

---

## 1. Core Limits

Structural caps preventing unbounded memory growth and O(N^2) exhaustion.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `SPHERE_CAP` | 200 | `m1_core/m04_constants.rs:126` | Maximum spheres in field (O(N^2) coupling guard) |
| `MEMORY_MAX_COUNT` | 500 | `m1_core/m04_constants.rs:129` | Maximum memories per sphere |
| `GHOST_MAX` | 20 | `m1_core/m04_constants.rs:132` | Maximum retained ghost traces |
| `LOG_MAX` | 1000 | `m1_core/m04_constants.rs:135` | Maximum message log entries |
| `INBOX_MAX` | 50 | `m1_core/m04_constants.rs:138` | Maximum inbox messages per sphere |
| `R_HISTORY_MAX` | 60 | `m1_core/m04_constants.rs:141` | Maximum r history samples |
| `DECISION_HISTORY_MAX` | 100 | `m1_core/m04_constants.rs:144` | Maximum decision history records |
| `MAX_GHOSTS` | 20 | `m3_hooks/m10_hook_server.rs:60` | FIFO ghost eviction (runtime, mirrors GHOST_MAX) |
| `MAX_TRACKED_PANES` | 256 | `m7_monitoring/m35_token_accounting.rs:38` | Maximum panes tracked for tokens |
| `MAX_TASK_RECORDS` | 5,000 | `m7_monitoring/m35_token_accounting.rs:47` | Maximum token task records (FIFO) |
| `MAX_MONITORS` | 50 | `m8_evolution/m37_emergence_detector.rs:45` | Maximum active emergence monitors |
| `EMERGENCE_HISTORY_CAP` | 100 | `bin/main.rs:25` | r/K history buffer for main loop emergence |

### Config-Space Core Limits

These are defaults from `PvConfig` structs, overridable via TOML or `PV2_*` env vars.

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `sphere.max_count` | 200 | `m1_core/m03_config.rs:224` | Configurable sphere cap |
| `sphere.memory_max_count` | 500 | `m1_core/m03_config.rs:225` | Configurable memory cap |
| `sphere.last_tool_max_chars` | 128 | `m1_core/m03_config.rs:231` | Tool name truncation limit |
| `ipc.max_connections` | 50 | `m1_core/m03_config.rs:399` | Maximum concurrent IPC connections |
| `ipc.event_buffer_size` | 256 | `m1_core/m03_config.rs:400` | Per-client event buffer capacity |
| `ipc.cascade_rate_limit` | 10 | `m1_core/m03_config.rs:404` | Maximum cascade dispatches per minute |
| `ipc.task_ttl_secs` | 3,600 | `m1_core/m03_config.rs:401` | Task TTL before GC (1 hour) |
| `governance.max_active_proposals` | 10 | `m1_core/m03_config.rs:453` | Concurrent proposal cap |

---

## 2. Wire Protocol

V2 wire protocol frame/buffer limits for the PV2 IPC bus.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `PROTOCOL_VERSION` | `"2.0"` | `m2_wire/m09_wire_protocol.rs:40` | Wire protocol version string |
| `MAX_FRAME_SIZE` | 65,536 bytes (64 KB) | `m2_wire/m09_wire_protocol.rs:46` | Maximum single frame size |
| `MAX_SEND_QUEUE` | 1,000 frames | `m2_wire/m09_wire_protocol.rs:49` | Outbound queue depth |
| `MAX_RECV_BUFFER` | 500 frames | `m2_wire/m09_wire_protocol.rs:52` | Inbound buffer depth |
| `DEFAULT_KEEPALIVE_SECS` | 30 seconds | `m2_wire/m09_wire_protocol.rs:43` | Keepalive interval |
| `server.body_limit_bytes` | 65,536 bytes | `m1_core/m03_config.rs:141` | HTTP request body limit |

### Protocol FSM States

```
Disconnected -> Handshaking -> Connected -> Subscribing -> Active -> Closing
     ^                                                                  |
     +------------------------- (error/timeout) -----------------------+
```

---

## 3. WASM Bridge

FIFO/ring protocol limits between ORAC and the Zellij swarm-orchestrator WASM plugin.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `RING_LINE_CAP` | 1,000 lines | `m6_coordination/m30_wasm_bridge.rs:54` | Maximum event ring file lines (FIFO eviction) |
| `MAX_COMMAND_LEN` | 8,192 bytes (8 KB) | `m6_coordination/m30_wasm_bridge.rs:57` | Maximum inbound command length |
| `MAX_EVENT_LEN` | 8,192 bytes (8 KB) | `m6_coordination/m30_wasm_bridge.rs:60` | Maximum outbound event length |
| `DEFAULT_FIFO_PATH` | `/tmp/swarm-commands.pipe` | `m6_coordination/m30_wasm_bridge.rs:48` | FIFO pipe path (WASM -> ORAC) |
| `DEFAULT_RING_PATH` | `/tmp/swarm-events.jsonl` | `m6_coordination/m30_wasm_bridge.rs:51` | Ring file path (ORAC -> WASM) |

---

## 4. Coupling Parameters

Kuramoto coupling network weights and Hebbian STDP learning rates.

### Learning Rates

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `HEBBIAN_LTP` | 0.01 | `m1_core/m04_constants.rs:30` | Long-term potentiation rate |
| `HEBBIAN_LTD` | 0.002 | `m1_core/m04_constants.rs:33` | Long-term depression rate |
| `HEBBIAN_BURST_MULTIPLIER` | 3.0 | `m1_core/m04_constants.rs:36` | LTP multiplier during burst activity |
| `HEBBIAN_NEWCOMER_MULTIPLIER` | 2.0 | `m1_core/m04_constants.rs:39` | LTP multiplier for newcomer spheres |
| `HEBBIAN_WEIGHT_FLOOR` | 0.15 | `m1_core/m04_constants.rs:42` | Minimum weight (prevents disconnection) |
| `NEWCOMER_STEPS` | 50 ticks | `m1_core/m04_constants.rs:166` | Duration of newcomer LTP boost |

### Coupling Network

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `DEFAULT_WEIGHT` | 0.18 | `m1_core/m04_constants.rs:49` | New sphere pair initial weight |
| `WEIGHT_EXPONENT` | 2.0 | `m1_core/m04_constants.rs:52` | Weight scaling exponent (w^2) |
| `COUPLING_STEPS_PER_TICK` | 15 | `m1_core/m04_constants.rs:20` | Euler integration sub-steps per tick |
| `KURAMOTO_DT` | 0.01 | `m1_core/m04_constants.rs:23` | Euler integration timestep |

### K Modulation Bounds

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `K_MOD_MIN` | -0.5 | `m1_core/m04_constants.rs:110` | Absolute K modulation floor |
| `K_MOD_MAX` | 1.5 | `m1_core/m04_constants.rs:113` | Absolute K modulation ceiling |
| `K_MOD_BUDGET_MIN` | 0.85 | `m1_core/m04_constants.rs:116` | Combined external influence floor |
| `K_MOD_BUDGET_MAX` | 1.15 | `m1_core/m04_constants.rs:119` | Combined external influence ceiling |

### Config-Space Coupling Defaults

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `coupling.auto_scale_k_period` | 20 ticks | `m1_core/m03_config.rs:264` | Ticks between auto-scale K adjustments |
| `coupling.auto_scale_k_multiplier` | 0.5 | `m1_core/m03_config.rs:265` | Auto-scale K step multiplier |
| `coupling.frequency_min` | 0.001 Hz | `m1_core/m03_config.rs:266` | Minimum natural frequency |
| `coupling.frequency_max` | 10.0 Hz | `m1_core/m03_config.rs:267` | Maximum natural frequency |
| `coupling.strength_min` | 0.0 | `m1_core/m03_config.rs:268` | Minimum coupling strength |
| `coupling.strength_max` | 2.0 | `m1_core/m03_config.rs:269` | Maximum coupling strength |

---

## 5. Evolution

RALPH engine, emergence detection, and mutation selection parameters.

### RALPH Engine

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `DEFAULT_ACCEPT_THRESHOLD` | 0.02 | `m8_evolution/m36_ralph_engine.rs:44` | Minimum fitness improvement to accept mutation |
| `DEFAULT_ROLLBACK_THRESHOLD` | -0.01 | `m8_evolution/m36_ralph_engine.rs:47` | Fitness regression that triggers rollback |
| `DEFAULT_VERIFICATION_TICKS` | 10 ticks | `m8_evolution/m36_ralph_engine.rs:50` | Ticks to wait before harvest decision |
| `DEFAULT_MAX_CYCLES` | 1,000 | `m8_evolution/m36_ralph_engine.rs:53` | Maximum RALPH cycles before auto-pause |
| `DEFAULT_SNAPSHOT_CAPACITY` | 50 | `m8_evolution/m36_ralph_engine.rs:56` | Maximum snapshot history depth |

### Emergence Detector

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `DEFAULT_HISTORY_CAPACITY` | 5,000 events | `m8_evolution/m37_emergence_detector.rs:36` | Maximum emergence records retained |
| `DEFAULT_TTL_TICKS` | 600 ticks (~50 min) | `m8_evolution/m37_emergence_detector.rs:39` | TTL before decay removal |
| `DEFAULT_MIN_CONFIDENCE` | 0.6 | `m8_evolution/m37_emergence_detector.rs:42` | Minimum confidence to register emergence |
| `DEFAULT_COHERENCE_LOCK_R` | 0.92 | `m8_evolution/m37_emergence_detector.rs:50` | CoherenceLock r threshold (lowered from 0.998 in Gen-059g) |
| `DEFAULT_COHERENCE_LOCK_TICKS` | 10 ticks | `m8_evolution/m37_emergence_detector.rs:53` | CoherenceLock sustained duration |
| `DEFAULT_RUNAWAY_WINDOW` | 20 ticks | `m8_evolution/m37_emergence_detector.rs:56` | CouplingRunaway detection window |
| `DEFAULT_SATURATION_RATIO` | 0.8 (80%) | `m8_evolution/m37_emergence_detector.rs:59` | HebbianSaturation weight fraction |
| `BENEFICIAL_SYNC_R` | 0.78 | `m8_evolution/m37_emergence_detector.rs:64` | Minimum r for BeneficialSync (lowered from 0.85 in Gen-059g) |
| `BENEFICIAL_SYNC_IMPROVEMENT` | 0.005 | `m8_evolution/m37_emergence_detector.rs:70` | Minimum r improvement for BeneficialSync |
| `FIELD_STABILITY_R` | 0.65 | `m8_evolution/m37_emergence_detector.rs:75` | Minimum sustained r for FieldStability |
| `FIELD_STABILITY_WINDOW` | 12 ticks (60s) | `m8_evolution/m37_emergence_detector.rs:81` | Consecutive ticks above threshold |

### Evolution Config-Space Defaults

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `evolution.emergence_cap` | 5,000 | `config/default.toml:22` | Maximum emergence records |
| `evolution.mutation_cooldown_generations` | 10 | `config/default.toml:23` | Cooldown between same-parameter mutations |
| `evolution.diversity_window` | 20 | `config/default.toml:24` | Diversity gate window |
| `evolution.diversity_threshold` | 0.5 | `config/default.toml:25` | Diversity rejection threshold (>50% same = reject) |

---

## 6. Tick Intervals

Bridge poll rates as modulo-tick intervals in `main.rs`. Base tick = 5 seconds (configurable).

| Bridge / Action | Modulo | Wall Clock (at 5s tick) | Source |
|----------------|--------|------------------------|--------|
| SYNTHEX thermal poll | `tick % 6` | 30s | `bin/main.rs:556` |
| ME observer poll | `tick % 12` | 60s | `bin/main.rs:567` |
| POVM snapshot post | `tick % 12` | 60s | `bin/main.rs:582` |
| POVM weight post | `tick % 60` | 5 min | `bin/main.rs:1029` |
| RM TSV post | `tick % 60` | 5 min | `bin/main.rs:1015` |
| VMS memory post | `tick % 30` | 2.5 min | `bin/main.rs:1529` |
| VMS consolidation trigger | `tick % 300` | 25 min | `bin/main.rs:1688` |
| Persist sessions/coupling | `tick % 60` | 5 min | `bin/main.rs:1556` |
| Homeostatic weight decay | `tick % 120` | 10 min | `bin/main.rs:1451` |
| STDP co-activation check | `tick % 12` | 60s | `bin/main.rs:505` |
| Emergence feed (r-based) | `tick % 5` | 25s | `bin/main.rs:532` |
| SYNTHEX field state post | `tick % 6` | 30s | `bin/main.rs:1673` |

### Config-Space Tick Defaults

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `field.tick_interval_secs` | 5 | `m1_core/m03_config.rs:180` | Base tick interval (seconds) |
| `bridges.synthex_poll_interval` | 6 ticks | `m1_core/m03_config.rs:337` | SYNTHEX poll period |
| `bridges.nexus_poll_interval` | 12 ticks | `m1_core/m03_config.rs:338` | Nexus poll period |
| `bridges.me_poll_interval` | 12 ticks | `m1_core/m03_config.rs:339` | ME poll period |
| `bridges.povm_snapshot_interval` | 12 ticks | `m1_core/m03_config.rs:340` | POVM snapshot period |
| `bridges.povm_weights_interval` | 60 ticks | `m1_core/m03_config.rs:341` | POVM weight persist period |
| `bridges.rm_post_interval` | 60 ticks | `m1_core/m03_config.rs:342` | RM TSV persist period |
| `bridges.vms_post_interval` | 60 ticks | `m1_core/m03_config.rs:343` | VMS memory post period |
| `persistence.snapshot_interval` | 60 ticks | `m1_core/m03_config.rs:427` | Field snapshot write period |

---

## 7. Circuit Breaker

Per-service health gating with Closed/Open/HalfOpen FSM.

| Parameter | Default | Source | Purpose |
|-----------|---------|--------|---------|
| `failure_threshold` | 5 | `m4_intelligence/m21_circuit_breaker.rs:74` | Consecutive failures to trip Open |
| `success_threshold` | 2 | `m4_intelligence/m21_circuit_breaker.rs:75` | Consecutive HalfOpen successes to Close |
| `open_timeout_ticks` | 30 ticks (~2.5 min) | `m4_intelligence/m21_circuit_breaker.rs:76` | Open->HalfOpen transition timeout |
| `half_open_max_requests` | 1 | `m4_intelligence/m21_circuit_breaker.rs:77` | Probe requests allowed in HalfOpen |

### FSM Transitions

```
Closed ----[failure >= 5]----> Open ----[30 ticks]----> HalfOpen
  ^                                                        |
  |                    [probe succeeds >= 2]                |
  +--------------------------------------------------------+
                       [probe fails] -> back to Open
```

### Registered Breakers

5 breakers initialized at startup for: `pv2`, `synthex`, `me`, `povm`, `rm`. VMS uses a tolerant config (10 fail / 3 success / 10 tick timeout) -- see `m3_hooks/m10_hook_server.rs:974`.

---

## 8. Token Budget

Fleet-wide token cost tracking with soft/hard budget enforcement.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `DEFAULT_INPUT_COST` | $0.000015/token | `m7_monitoring/m35_token_accounting.rs:32` | Input token cost (USD) |
| `DEFAULT_OUTPUT_COST` | $0.000075/token | `m7_monitoring/m35_token_accounting.rs:35` | Output token cost (USD) |
| `DEFAULT_SOFT_LIMIT` | $10.00 | `m7_monitoring/m35_token_accounting.rs:41` | Soft budget (warning threshold) |
| `DEFAULT_HARD_LIMIT` | $50.00 | `m7_monitoring/m35_token_accounting.rs:44` | Hard budget (enforcement threshold) |
| `MAX_TRACKED_PANES` | 256 | `m7_monitoring/m35_token_accounting.rs:38` | Maximum pane tracking entries |
| `MAX_TASK_RECORDS` | 5,000 | `m7_monitoring/m35_token_accounting.rs:47` | Maximum task token records (FIFO) |

**Cost model:** `cost = (input_tokens * 0.000015) + (output_tokens * 0.000075)`. PostToolUse estimates tokens as `chars / 4`.

---

## 9. Conductor

PI breathing controller parameters for field synchronization.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `CONDUCTOR_GAIN` | 0.15 | `m1_core/m04_constants.rs:100` | Proportional gain for PI controller |
| `EMERGENT_BLEND` | 0.3 | `m1_core/m04_constants.rs:103` | Fraction of emergent signal blended into output |
| `_DIVERGENCE_COOLDOWN_TICKS` | 3 ticks | `m6_coordination/m27_conductor.rs:28` | Cooldown after divergence kick |
| `MIN_SPHERES_FOR_BREATHING` | 3 | `m6_coordination/m27_conductor.rs:31` | Minimum spheres for breathing to activate |
| `R_TARGET_BASE` | 0.93 | `m1_core/m04_constants.rs:87` | Target r for small/medium fleets |
| `R_TARGET_LARGE_FLEET` | 0.85 | `m1_core/m04_constants.rs:90` | Target r for large fleets (>50 spheres) |
| `LARGE_FLEET_THRESHOLD` | 50.0 | `m1_core/m04_constants.rs:93` | Sphere count above which large-fleet target applies |

### Config-Space Conductor Defaults

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `conductor.gain` | 0.15 | `m1_core/m03_config.rs:363` | Configurable PI gain |
| `conductor.breathing_blend` | 0.3 | `m1_core/m03_config.rs:364` | Configurable emergent blend |
| `conductor.divergence_cooldown_ticks` | 3 | `m1_core/m03_config.rs:365` | Configurable divergence cooldown |

---

## 10. Field Thresholds

Kuramoto field synchronization and chimera detection thresholds.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `PHASE_GAP_THRESHOLD` | pi/3 (~1.047 rad) | `m1_core/m04_constants.rs:59` | Chimera detection angular gap |
| `SYNC_THRESHOLD` | 0.5 | `m1_core/m04_constants.rs:62` | r above which field is synchronized |
| `TUNNEL_THRESHOLD` | 0.8 rad | `m1_core/m04_constants.rs:65` | Angular distance for buoy tunneling |
| `R_HIGH_THRESHOLD` | 0.8 | `m1_core/m04_constants.rs:68` | Highly coherent field threshold |
| `R_LOW_THRESHOLD` | 0.3 | `m1_core/m04_constants.rs:71` | Incoherent field threshold |
| `R_FALLING_THRESHOLD` | -0.03 | `m1_core/m04_constants.rs:74` | Slope triggering RTrend::Falling |
| `R_RISING_THRESHOLD` | 0.03 | `m1_core/m04_constants.rs:77` | Slope triggering RTrend::Rising |
| `IDLE_RATIO_THRESHOLD` | 0.6 | `m1_core/m04_constants.rs:80` | Idle sphere fraction triggering IdleFleet |
| `CLUSTER_PROXIMITY` | pi/6 (~0.524 rad) | `m1_core/field_state.rs:21` | Phase proximity for cluster grouping |
| `CHIMERA_GAP` | pi/3 (~1.047 rad) | `m1_core/field_state.rs:24` | Minimum gap between clusters for chimera |
| `CHIMERA_R_THRESHOLD` | 0.95 | `m1_core/field_state.rs:28` | r above which chimera detection is disabled |

---

## 11. Sphere Dynamics

Per-sphere memory activation and decay parameters.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `DECAY_PER_STEP` | 0.995 | `m1_core/m04_constants.rs:151` | Multiplicative activation decay per tick |
| `SWEEP_BOOST` | 0.05 | `m1_core/m04_constants.rs:154` | Activation boost from sweep |
| `ACTIVATION_THRESHOLD` | 0.3 | `m1_core/m04_constants.rs:157` | Below this, memories are prunable |
| `MEMORY_PRUNE_INTERVAL` | 200 steps | `m1_core/m04_constants.rs:160` | Steps between prune checks |
| `SEMANTIC_NUDGE_STRENGTH` | 0.02 | `m1_core/m04_constants.rs:163` | Gentle semantic phase nudge |
| `NEWCOMER_STEPS` | 50 ticks | `m1_core/m04_constants.rs:166` | Duration of newcomer LTP boost |

---

## 12. Network and Persistence

Server binding, TCP timeouts, and SQLite configuration.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `DEFAULT_PORT` | 8132 | `m1_core/m04_constants.rs:183` | Default PV2 port (ORAC overrides to 8133 via config) |
| `BIND_MAX_RETRIES` | 5 | `m1_core/m04_constants.rs:186` | Maximum bind retry attempts |
| `BIND_INITIAL_DELAY_MS` | 500 ms | `m1_core/m04_constants.rs:189` | Initial delay between bind retries |
| `DEFAULT_TCP_TIMEOUT_MS` | 2,000 ms (2s) | `m5_bridges/http_helpers.rs:15` | TCP connection timeout for bridges |
| `DEFAULT_MAX_RESPONSE_SIZE` | 32,768 bytes (32 KB) | `m5_bridges/http_helpers.rs:20` | Default max HTTP response body |
| `MAX_RESPONSE_SIZE` (POVM) | 2,097,152 bytes (2 MB) | `m5_bridges/m24_povm_bridge.rs:60` | POVM response limit (large hydration payloads) |
| `SNAPSHOT_INTERVAL` | 60 ticks | `m1_core/m04_constants.rs:173` | Ticks between field snapshots |
| `WARMUP_TICKS` | 5 | `m1_core/m04_constants.rs:176` | Reduced-dynamics ticks after snapshot restore |

### Config-Space Persistence Defaults

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `persistence.snapshot_interval` | 60 ticks | `m1_core/m03_config.rs:427` | Configurable snapshot interval |
| `persistence.wal_busy_timeout_ms` | 5,000 ms | `m1_core/m03_config.rs:428` | SQLite WAL busy timeout |
| `persistence.bus_db_path` | `data/bus_tracking.db` | `m1_core/m03_config.rs:429` | Bus tracking database path |
| `persistence.field_db_path` | `data/field_tracking.db` | `m1_core/m03_config.rs:430` | Field tracking database path |
| `ipc.socket_path` | `/run/user/1000/pane-vortex-bus.sock` | `m1_core/m03_config.rs:399` | IPC Unix socket path |
| `ipc.socket_permissions` | `0o700` | `m1_core/m03_config.rs:398` | Socket file permissions |

---

## 13. Governance

Collective voting parameters for fleet governance proposals.

| Config Field | Default | Source | Purpose |
|-------------|---------|--------|---------|
| `governance.proposal_voting_window_ticks` | 5 ticks | `m1_core/m03_config.rs:450` | Voting window duration |
| `governance.quorum_threshold` | 0.5 (50%) | `m1_core/m03_config.rs:451` | Required fraction for quorum |
| `governance.max_active_proposals` | 10 | `m1_core/m03_config.rs:453` | Concurrent proposal cap |

---

## 14. Mathematical Constants

Re-exported for convenience.

| Constant | Value | Source | Purpose |
|----------|-------|--------|---------|
| `TWO_PI` | 6.283... (TAU) | `m1_core/m04_constants.rs:196` | Full circle in radians |
| `FRAC_PI_3` | 1.047... | `std::f64::consts` | Phase gap threshold |
| `FRAC_PI_6` | 0.524... | `std::f64::consts` | Cluster proximity threshold |

---

## Staleness Canary

```bash
# Verify constant count in m04_constants.rs (expect: ~30 pub const lines)
rg '^pub const' src/m1_core/m04_constants.rs --count-matches

# Verify all private constants referenced in this doc still exist
rg 'const (DEFAULT_HISTORY_CAPACITY|DEFAULT_TTL_TICKS|DEFAULT_MIN_CONFIDENCE|MAX_FRAME_SIZE|MAX_SEND_QUEUE|MAX_RECV_BUFFER|RING_LINE_CAP|MAX_COMMAND_LEN|DEFAULT_INPUT_COST|DEFAULT_OUTPUT_COST|DEFAULT_SOFT_LIMIT|DEFAULT_HARD_LIMIT)' src/ --count-matches

# Verify emergence detector thresholds (these change between RALPH generations)
rg 'const (DEFAULT_COHERENCE_LOCK_R|BENEFICIAL_SYNC_R|FIELD_STABILITY_R|FIELD_STABILITY_WINDOW)' src/ -n
```

---

## 15. Theoretical Complexity

Asymptotic cost of each major subsystem per tick. These are derived from the algorithms, not measured.

| Subsystem | Complexity | Variables | Notes |
|-----------|-----------|-----------|-------|
| Kuramoto field update | O(N x S) | N = spheres, S = `COUPLING_STEPS_PER_TICK` = 15 | Each sub-step iterates all N spheres; each sphere sums contributions from all neighbours via weighted adjacency |
| STDP Hebbian pass | O(C) where C = N x (N-1) / 2 | N = spheres | Quadratic in sphere count -- every sphere pair is checked for co-activation status |
| Emergence scan | O(H) | H = history buffer, capped at `DEFAULT_HISTORY_CAPACITY` = 5,000 | Linear scan for TTL expiry and pattern detection across ring buffer |
| Semantic routing | O(N) | N = spheres | Single pass through spheres to compute composite score per candidate |
| Blackboard queries | O(1) amortised | Indexed by pane_id, finished_at | SQLite B-tree index ensures constant-time lookups |
| Chimera detection | O(N log N) | N = spheres | Sort by phase, then linear gap scan |
| Auto-scale K | O(1) | Reads aggregate r and frequency variance | P-controller arithmetic on pre-computed statistics |
| Bridge poll (per bridge) | O(1) | One HTTP request + response parse | Network-bound, not CPU-bound |

**Dominant cost:** STDP at O(N^2) is the primary scaling bottleneck. At the current `SPHERE_CAP` = 200, worst case is 19,900 pair evaluations per tick.

---

## 16. Known Bottleneck: Blocking Bridge Calls

### Current Bridge Architecture

The tick loop contains 2 active bridge *polls* (blocking HTTP GET requests inside `spawn_blocking`):

| Bridge | Direction | Frequency | Timeout | Operation |
|--------|-----------|-----------|---------|-----------|
| SYNTHEX | poll (GET) | `tick % 6` (30s) | 2s | Read temperature, PID output, heat sources |
| ME | poll (GET) | `tick % 12` (60s) | 2s | Read observer fitness |

The remaining bridges are periodic *posts* (non-blocking writes), not polls:

| Bridge | Direction | Frequency | Timeout | Operation |
|--------|-----------|-----------|---------|-----------|
| POVM | post (POST) | `tick % 12` snapshot, `tick % 60` weights | 2s | Write field snapshot, coupling weights |
| RM | post (POST) | `tick % 60` (5 min) | 2s | Write TSV observations |
| VMS | post (POST) | `tick % 30` (2.5 min) | 2s | Write memory updates |
| PV2 | poll (GET) | every tick (5s) via field poller | 2s | Read field state (separate task, not in RALPH loop) |

### Why This Matters

Posts fire-and-forget (success logged, failure retried next interval). Polls block the RALPH loop thread until response or timeout. Since hooks already use `spawn_blocking` correctly (the pattern exists in `m10_hook_server`), the fix template is established.

### Worst Case Stall Analysis

If both SYNTHEX and ME are unreachable simultaneously:

```
Worst case stall = 2 bridges x 2s timeout = 4s blocking
Tick interval    = 5s
Headroom         = 5s - 4s = 1s
```

This is tight but functional. The circuit breaker prevents repeated stalls: after 5 consecutive failures, the breaker opens and skips the poll entirely for 30 ticks (~2.5 minutes).

---

## 17. O(N^2) Coupling -- Primary CPU Scaling Concern

The STDP Hebbian pass evaluates all sphere pairs per tick. This is the primary CPU scaling concern.

### Connection Count by Sphere Count

| Spheres (N) | Connections C = N(N-1)/2 | Growth Factor |
|-------------|--------------------------|---------------|
| 10 | 45 | baseline |
| 20 | 190 | 4.2x |
| 50 | 1,225 | 27x |
| 100 | 4,950 | 110x |
| 200 (cap) | 19,900 | 442x |

### HashMap Cache-Miss Pattern

The coupling network stores weights in a `HashMap<(PaneId, PaneId), f64>`. At scale:

- **10-50 spheres:** HashMap fits comfortably in L2 cache (~256KB). Fast random access.
- **100 spheres:** 4,950 entries x ~80 bytes/entry = ~396KB. Spills into L3. Some cache misses.
- **200 spheres:** 19,900 entries x ~80 bytes/entry = ~1.6MB. Consistent L3 hits, possible cache pressure with other working set data.

The `PaneId` key is a heap-allocated `String`, which adds pointer chasing. A future optimisation could use integer IDs or a contiguous adjacency matrix.

### Mitigation Already in Place

- `SPHERE_CAP` = 200 provides a hard upper bound
- STDP idle gating skips the pass when fewer than 2 spheres are Working
- Memory pruning (every 200 steps) removes low-activation entries
- Homeostatic decay (every 120 ticks) prevents weight explosion

### What Would Be Needed at Scale

If `SPHERE_CAP` were raised beyond 200, these mitigations would be necessary:

1. **Neighbourhood-limited STDP:** Only evaluate top-K neighbours per sphere (O(N x K) instead of O(N^2))
2. **Adjacency matrix:** Replace HashMap with contiguous `Vec<Vec<f64>>` for cache-friendly access
3. **Parallel STDP:** Use `rayon` for data-parallel pair evaluation
4. **Profiling data:** No empirical measurements exist (see Section 20)

---

## 18. SQLite WAL Contention

### Write Contention Points

Two categories of writes contend for the blackboard SQLite lock:

**Hook-triggered writes (unpredictable timing):**
- `PostToolUse` -> `pane_status` upsert, `task_history` insert
- `SessionStart` -> `sessions` insert, `pane_status` insert
- `Stop` -> `sessions` update, `ghost_traces` insert

**Tick-triggered writes (periodic, predictable):**
- Every 12 ticks -> `hebbian_summary` upsert
- Every 60 ticks -> `ralph_state` upsert, `sessions` upsert, `coupling_weights` bulk upsert
- Every 720 ticks -> blackboard GC (DELETE old records)

### BUSY Retry Strategy

When SQLite returns `SQLITE_BUSY` (another writer holds the lock):

```
Attempt 1: wait 100ms, retry
Attempt 2: wait 200ms, retry
Attempt 3: wait 400ms, retry
Total max wait: 700ms
```

If all 3 retries fail, the write is logged as a warning and skipped (data loss for that tick, recovered next interval).

### WAL Configuration

```sql
PRAGMA journal_mode = WAL;        -- Write-Ahead Logging
PRAGMA busy_timeout = 5000;       -- 5s busy timeout (SQLite-level)
PRAGMA synchronous = NORMAL;      -- Durability vs performance tradeoff
```

WAL mode allows unlimited concurrent readers. Only writers contend. Since ORAC is single-process (all writes from one tokio runtime), WAL contention is intra-process only -- no cross-process lock conflicts.

### Observed Behaviour

In production (Session 060, 44 spheres, 1,892 coupling connections), no BUSY retries were observed in logs. Contention is theoretical at current scale.

---

## 19. Scaling Projections (THEORETICAL -- UNTESTED)

**WARNING:** These are theoretical estimates derived from algorithmic complexity. No empirical validation exists. Do not use these numbers for capacity planning without benchmarking.

### Per-Tick Compute Estimates

| Spheres | Connections | STDP/tick | Field compute | Memory (coupling) |
|---------|-------------|-----------|---------------|-------------------|
| 10 | 45 | <1ms | <1ms | ~4KB |
| 20 | 190 | ~1ms | <1ms | ~15KB |
| 50 | 1,225 | ~5ms | ~3ms | ~100KB |
| 100 | 4,950 | ~20ms | ~10ms | ~400KB |
| 200 | 19,900 | ~80ms | ~40ms | ~1.6MB |

### Assumptions Behind These Estimates

- STDP: ~4 microseconds per pair evaluation (HashMap lookup + float arithmetic + clamp)
- Field compute: ~2 microseconds per sphere per sub-step (15 sub-steps x N spheres)
- Memory: ~80 bytes per coupling entry (two PaneId strings + f64 weight + HashMap overhead)
- CPU: single-threaded (no rayon), modern x86-64 (3+ GHz)

### Tick Budget Analysis

With a 5-second tick interval:

| Spheres | STDP + Field | % of Tick Budget | Verdict |
|---------|-------------|------------------|---------|
| 10 | <2ms | <0.04% | Negligible |
| 50 | ~8ms | 0.16% | Comfortable |
| 100 | ~30ms | 0.6% | Comfortable |
| 200 | ~120ms | 2.4% | Comfortable |

Even at the 200-sphere cap, compute uses only ~2.4% of the tick budget. The real bottleneck would be bridge poll latency (up to 4 seconds), not CPU.

### Memory Growth Estimates

| Spheres | Coupling Map | Sphere State | Buoys | Ghost Traces | Total |
|---------|-------------|-------------|-------|-------------|-------|
| 10 | ~4KB | ~50KB | ~10KB | ~2KB | ~66KB |
| 50 | ~100KB | ~250KB | ~50KB | ~10KB | ~410KB |
| 100 | ~400KB | ~500KB | ~100KB | ~20KB | ~1MB |
| 200 | ~1.6MB | ~1MB | ~200KB | ~40KB | ~2.8MB |

Memory is not a concern at any supported sphere count.

---

## 20. What's NOT Tested

### Missing Validation

| Gap | Description | Risk |
|-----|-------------|------|
| No load test harness | `ralph-bench` binary exists (120 LOC) but bench stubs are empty. No criterion benchmarks. | Cannot validate scaling projections in Section 19 |
| No memory profiling | No heap profiling under sustained load. Memory estimates are theoretical. | Potential for hidden allocations or leaks under long-running sessions |
| No multi-day endurance test | ORAC has run for hours in fleet sessions, never for days. | Unknown behaviour of ring buffers, HashMap growth, SQLite WAL file size over time |
| No concurrent hook stress test | Hooks tested sequentially. No test simulates 50 panes sending PostToolUse simultaneously. | Unknown contention behaviour under concurrent HTTP load |
| No bridge failure cascade test | Circuit breakers tested in unit tests. No integration test where all 6 bridges fail simultaneously. | Unknown system behaviour under total upstream failure |

### Production Maximums Observed

| Metric | Maximum Observed | Session | Cap |
|--------|-----------------|---------|-----|
| Sphere count | 66 | Session 056 | 200 |
| Coupling connections | 1,892 | Session 060 | 19,900 |
| RALPH generations | 1,754 | Session 060 | 1,000 (auto-pause, overridden) |
| Emergence events | 243 | Session 060 | 5,000 |
| POVM pathways hydrated | 2,504 | Session 060 | limited by 2MB response |
| Concurrent fleet panes | 9 | Session 060 | 200 |

The system has operated at ~33% of sphere cap (66/200) and ~9.5% of connection cap (1,892/19,900). The O(N^2) region (>100 spheres) has never been exercised in production.

---

## 21. Validation Methodology

### Minimal Benchmark Proposal

A ~20 LOC criterion benchmark to validate STDP scaling estimates:

```rust
// benches/stdp_scaling.rs
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use orac_sidecar::m4_intelligence::m18_hebbian_stdp::StdpTracker;

fn bench_stdp_tick(c: &mut Criterion) {
    let mut group = c.benchmark_group("stdp_tick");
    for sphere_count in [10, 50, 100, 200] {
        group.bench_with_input(
            BenchmarkId::new("spheres", sphere_count),
            &sphere_count,
            |b, &n| {
                let tracker = StdpTracker::with_random_spheres(n);
                b.iter(|| tracker.tick_all_pairs());
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_stdp_tick);
criterion_main!(benches);
```

**Purpose:** Validate or refute the ~4 microsecond/pair assumption from Section 19. Would also reveal cache effects at 100+ spheres.

### Recommended Next Steps

1. **Add criterion dependency** to `[dev-dependencies]` in `Cargo.toml`
2. **Implement `StdpTracker::with_random_spheres(n)`** test helper
3. **Run benchmark** at 10/50/100/200 spheres, compare against theoretical estimates
4. **Add memory profiling** via `dhat` or `jemalloc` stats for coupling HashMap at scale
5. **Endurance test** -- run ORAC for 24h with synthetic sphere registration/deregistration

---

*Generated: 2026-03-25 | Source: `/home/louranicas/claude-code-workspace/orac-sidecar/`*
*Obsidian backlinks: `[[Session 061 -- ORAC System Atlas]]`, `[[ORAC Sidecar -- Architecture Schematics]]`*
