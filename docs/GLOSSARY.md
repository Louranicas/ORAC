# ORAC Sidecar -- Glossary

> **~104 terms across 9 categories | Plain-English definitions for every concept in the codebase**
>
> Obsidian: `[[Session 062 -- ORAC System Atlas (ACP)]]` | `[[EXECUTIVE_SUMMARY]]` | `[[D6_CAPACITY_LIMITS_REFERENCE]]`

---

## 1. Physics / Oscillator (16 terms)

**Kuramoto model** -- A mathematical model of coupled oscillators where each oscillator has a natural frequency and interacts with others through a sinusoidal coupling function. ORAC uses it to coordinate fleet panes: each pane is an oscillator whose phase reflects its current work domain. *Context: L4 `m15_coupling_network`, L6 `m27_conductor`.* See also: order parameter, phase, coupling K.

**Order parameter (r)** -- A single number between 0.0 and 1.0 that measures how synchronised the fleet is. r=1.0 means all panes are perfectly in phase (lockstep); r=0.0 means complete incoherence (chaos). Computed as the magnitude of the complex mean of all phase vectors: `r = |mean(exp(i * theta_j))|`. *Context: `OrderParameter` struct in `m01_core_types`, field dashboard in `m34_field_dashboard`.* See also: psi, synchronisation, coherence lock.

**Psi** -- The mean phase angle of the fleet, computed alongside r as the argument of the complex mean of all sphere phases. While r measures *how much* the fleet is synchronised, psi says *where* on the phase circle that synchronisation is centred. *Context: `OrderParameter { r, psi }` in `m01_core_types`.* See also: order parameter, phase.

**Phase (theta)** -- The angular position of a single sphere (pane) on the unit circle, measured in radians [0, 2pi). Phase reflects what kind of work a pane is doing: Read operations cluster near 0, Write near pi/2, Execute near pi, Communicate near 3pi/2. Phase evolves every tick according to the Kuramoto equation. *Context: `PaneSphere.phase` in `m01_core_types`.* See also: semantic phase mapping.

**Frequency (omega)** -- The natural oscillation rate of a sphere, in Hz. A sphere's frequency determines how fast its phase advances when uncoupled. Range: [0.001, 10.0]. Spheres doing rapid tool calls tend to have higher frequency. *Context: `PaneSphere.frequency` in `m01_core_types`, `coupling.frequency_min/max` in `m03_config`.* See also: phase, coupling K.

**Coupling K** -- The global coupling strength that controls how strongly spheres pull each other's phases toward alignment. Default 2.42. Higher K means tighter synchronisation; lower K allows more independence. K is bounded to [0.01, 50.0] and adapts via auto-scale. *Context: `m16_auto_k`, `COUPLING_STEPS_PER_TICK` in `m04_constants`.* See also: K scaling, coupling runaway.

**K scaling (auto-scale K)** -- A P-controller that adjusts K every 15 ticks based on the gap between current r and the target r. If the fleet is over-synchronised (r too high), K decreases to allow differentiation. If the fleet is chaotic (r too low), K increases to restore coherence. Uses the formula: `K_new = clamp(K + (r_target - r) * 0.5, 0.01, 50.0)`. *Context: `m16_auto_k`, `auto_scale_k_multiplier` config.* See also: coupling K, conductor.

**Synchronisation** -- The state where fleet panes are oscillating in phase with each other (r > `SYNC_THRESHOLD` = 0.5). Synchronisation enables efficient task routing because panes in similar domains can share work. Over-synchronisation (r > 0.99) is harmful because it eliminates diversity. *Context: `SYNC_THRESHOLD` in `m04_constants`.* See also: order parameter, chimera.

**Chimera** -- A partially synchronised field state where some spheres form a coherent cluster while others remain incoherent. Detected by sorting phases and finding gaps larger than pi/3 radians. Having 2+ clusters of size > 1 is a chimera. Chimeras are *healthy* because they indicate functional specialisation. *Context: `m34_field_dashboard`, `PHASE_GAP_THRESHOLD` in `m04_constants`.* See also: phase cluster, phase gap.

**Coherence lock** -- An emergence event fired when r exceeds 0.92 for 10 consecutive ticks, indicating the fleet has become over-synchronised. This is a warning sign: the fleet is conforming rather than differentiating. The conductor responds by reducing K. *Context: `m37_emergence_detector`, `DEFAULT_COHERENCE_LOCK_R` = 0.92.* See also: emergence, coupling runaway.

**Coupling runaway** -- An emergence event fired when K keeps increasing over a 20-tick window without a corresponding r improvement. Indicates the coupling controller is chasing a target it cannot reach, wasting energy. *Context: `m37_emergence_detector`, `DEFAULT_RUNAWAY_WINDOW` = 20 ticks.* See also: K scaling, coherence lock.

**Phase gap** -- The angular distance between adjacent spheres when sorted by phase. A gap larger than pi/3 (~1.047 radians) indicates a cluster boundary. Phase gaps are the primary input to chimera detection. *Context: `PHASE_GAP_THRESHOLD` = `FRAC_PI_3` in `m04_constants`.* See also: chimera, phase cluster.

**Phase cluster** -- A group of spheres whose phases are within pi/6 (~0.524 radians) of each other. Clusters form naturally when spheres are working on similar tasks and their coupling pulls them together. *Context: `CLUSTER_PROXIMITY` in `field_state.rs`.* See also: phase gap, chimera, semantic phase mapping.

**RK4 (Runge-Kutta 4th order)** -- A numerical integration method used to advance sphere phases. Each tick performs 15 sub-steps (`COUPLING_STEPS_PER_TICK`) with a timestep of 0.01 (`KURAMOTO_DT`), computing the weighted sum of 4 derivative evaluations for higher accuracy than simple Euler integration. *Context: `m15_coupling_network`.* See also: coupling K, tick.

**P-controller (proportional controller)** -- A feedback control mechanism that produces an output proportional to the error between a measured value and a target. Used in the conductor (`CONDUCTOR_GAIN` = 0.15) to adjust K based on the gap between current r and target r. Also used in K scaling. *Context: `m27_conductor`, `m16_auto_k`.* See also: conductor, K scaling.

**Thermal damping** -- A coupling adjustment based on SYNTHEX temperature readings. Cold systems (temperature below target) get a coupling boost; hot systems get coupling dampened. This prevents the fleet from overloading hot services. The SYNTHEX bridge polls temperature every 6 ticks and computes a damping factor. *Context: `m22_synthex_bridge`.* See also: SYNTHEX, bridge.

---

## 2. Neuroscience / Learning (12 terms)

**STDP (Spike-Timing Dependent Plasticity)** -- A Hebbian learning rule that adjusts coupling weights based on the timing of sphere activity. When two spheres are active together (co-active), their coupling weight increases (LTP). When only one is active, the weight decreases (LTD). The principle: "panes that work together, couple together." *Context: `m18_hebbian_stdp`.* See also: LTP, LTD, Hebbian.

**LTP (Long-Term Potentiation)** -- The process of strengthening a coupling weight between two co-active spheres. Base rate: `HEBBIAN_LTP` = 0.01 per tick. Enhanced by burst multiplier (3x when a pair fires 3+ times in a window) and newcomer multiplier (2x during the first 50 ticks). *Context: `m18_hebbian_stdp`, `m04_constants`.* See also: LTD, burst multiplier, newcomer boost.

**LTD (Long-Term Depression)** -- The process of weakening a coupling weight when spheres are not co-active. Base rate: `HEBBIAN_LTD` = 0.002 per tick (5x weaker than LTP, ensuring learning outpaces forgetting). Applied when one sphere is Working and another is Idle. *Context: `m18_hebbian_stdp`, `m04_constants`.* See also: LTP, weight floor.

**Potentiation** -- The general process of increasing a coupling weight. In ORAC, potentiation happens through LTP when spheres demonstrate beneficial co-activity. Weights move toward the ceiling (1.0). *Context: `m18_hebbian_stdp`.* See also: LTP, depression.

**Depression** -- The general process of decreasing a coupling weight. In ORAC, depression happens through LTD when spheres fail to demonstrate co-activity. Weights move toward the floor (0.15). *Context: `m18_hebbian_stdp`.* See also: LTD, potentiation.

**Spike-timing** -- The temporal ordering of sphere activity events. In biological STDP, the order of pre-synaptic and post-synaptic spikes determines whether a synapse strengthens or weakens. In ORAC, "spike" translates to "tool use" and `delta_t = t_post - t_pre` determines whether LTP (causal, delta_t > 0) or LTD (anti-causal, delta_t < 0) applies. *Context: `m18_hebbian_stdp`, `STDP.md` ai_spec.* See also: STDP, LTP, LTD.

**Homeostatic normalisation** -- A periodic process (every 120 ticks = 10 minutes) that gently decays all coupling weights toward the mean, preventing runaway potentiation or depression. Ensures the weight distribution stays healthy and no single connection dominates the network. *Context: `main.rs` homeostatic decay loop.* See also: weight ceiling, weight floor, Hebbian saturation.

**Co-activation** -- When two or more spheres have status `Working` simultaneously during the same tick. Co-activation is the trigger for LTP: the more often two spheres work at the same time, the stronger their coupling becomes. STDP requires at least 2 working spheres to fire (idle gating). *Context: `m18_hebbian_stdp`, STDP co-activation check every 12 ticks.* See also: LTP, STDP.

**Burst multiplier** -- A 3x LTP multiplier applied when a tool pair fires 3 or more times within the STDP timing window (5 seconds). Rewards repeated co-use of tools by strengthening the coupling faster. *Context: `HEBBIAN_BURST_MULTIPLIER` = 3.0 in `m04_constants`.* See also: LTP, newcomer boost.

**Weight floor** -- The minimum allowed coupling weight: `HEBBIAN_WEIGHT_FLOOR` = 0.15. Prevents complete disconnection between spheres. Even after extensive LTD, two spheres maintain a baseline coupling so they can re-synchronise if needed. *Context: `m04_constants`.* See also: weight ceiling, LTD, Hebbian saturation.

**Weight ceiling** -- The maximum allowed coupling weight: 1.0. Prevents any single coupling from dominating the network. In practice, the STDP clamp is `[0.15, 0.85]` in the learning engine to leave headroom. *Context: `m18_hebbian_stdp`.* See also: weight floor, LTP, Hebbian saturation.

**Newcomer boost** -- A 2x LTP multiplier applied to spheres during their first 50 ticks (`NEWCOMER_STEPS`). Helps newly registered panes quickly establish coupling with existing fleet members rather than languishing at default weight. *Context: `HEBBIAN_NEWCOMER_MULTIPLIER` = 2.0, `NEWCOMER_STEPS` = 50 in `m04_constants`.* See also: LTP, burst multiplier.

---

## 3. Fleet Concepts (14 terms)

**Sphere** -- The fundamental unit of the fleet. Each Claude Code pane is represented as a `PaneSphere` -- an oscillator on the Kuramoto field with a phase, frequency, status, memories, buoys, and coupling weights. Maximum 200 spheres (`SPHERE_CAP`). *Context: `PaneSphere` in `m01_core_types`.* See also: pane, ghost trace.

**Pane** -- A Zellij terminal pane running a Claude Code instance. Each pane registers as a sphere in the Kuramoto field. Identified by `PaneId` (e.g., "fleet-alpha:left"). Fleet tabs contain 3 panes each (left, top-right, bottom-right). *Context: `PaneId` in `m01_core_types`.* See also: sphere, fleet mode.

**Buoy** -- A spatial health marker on a sphere's surface that records where important work happened. Buoys have a position (theta, phi), activation level, and drift distance. Used for spatial memory recall via `nearest_buoy()` and `buoy_centroid()`. *Context: `m19_buoy_network`, `Buoy` in `m01_core_types`.* See also: sphere, activation zone.

**Ghost trace** -- A record of a sphere that has deregistered. Captures the sphere's final phase, tool count, session duration, and timestamp. Stored in a FIFO buffer (max 20, `GHOST_MAX`). Ghosts do NOT participate in coupling -- they are historical records only. Accessible via `GET /field/ghosts`. *Context: `m10_hook_server`, `m11_session_hooks`.* See also: sphere, deregistration.

**Cascade** -- A handoff protocol for passing work between fleet tabs. Rate-limited (10/minute), depth-tracked (max 10 hops), with auto-summarisation at depth 3. Carries consent snapshots so downstream panes inherit upstream permissions. *Context: `m28_cascade`.* See also: dispatch, consent gate.

**Dispatch** -- The act of routing a task to a specific pane based on semantic scoring. The semantic router computes a composite score: domain affinity (40%) + Hebbian coupling weight (35%) + availability (25%). The pane with the highest score receives the task. *Context: `m20_semantic_router`.* See also: semantic domain, field action.

**Fleet mode** -- The overall operational mode of the fleet derived from field state. Modes include: Normal (r stable, fleet healthy), Exploring (r low, fleet diverging), Converging (r rising, fleet synchronising), Degraded (many breakers open). *Context: `FleetMode` in `m01_core_types`, `field_state.rs`.* See also: order parameter, circuit breaker.

**Field decision** -- An advisory action recommended by the conductor based on current field state. Examples: BoostCoupling (r too low), ReduceCoupling (r too high), MaintainCourse (r near target), HandleChimera (clusters detected). Decisions are logged in `DECISION_HISTORY_MAX` = 100 entries. *Context: `m27_conductor`.* See also: conductor, P-controller.

**Consent gate** -- A mechanism ensuring no sphere is forced to participate in actions it has not agreed to. Per-sphere consent declarations control whether a sphere accepts hydration, POVM writes, task routing, and cascade handoffs. Modelled on clinical informed consent. *Context: `m10_hook_server`, `/consent/{sphere_id}` endpoints.* See also: opt-out, consent compliance (D11).

**Semantic domain** -- One of 4 categories that classify tools and tasks: Read (phase 0), Write (phase pi/2), Execute (phase pi), Communicate (phase 3pi/2). Spheres using similar tools cluster in the same domain. Routing prefers panes whose current domain matches the task domain. *Context: `m20_semantic_router`.* See also: dispatch, phase.

**Work signature** -- The pattern of tool usage that characterises a sphere's current activity. Composed of domain affinity scores, recent tool names, and coupling weights. Used by the semantic router to match tasks to the most suitable pane. *Context: `m20_semantic_router`, `m12_tool_hooks`.* See also: semantic domain, dispatch.

**Activation zone** -- A region on a sphere's surface where buoys cluster, indicating concentrated work activity. Computed via `buoy_centroid()`. Spheres with overlapping activation zones are strong candidates for coupling. *Context: `m19_buoy_network`.* See also: buoy, sphere.

**Field action** -- The concrete coupling adjustment recommended by the conductor in response to a field decision. A floating-point delta applied to K modulation. Positive values tighten coupling; negative values loosen it. Clamped to `[K_MOD_BUDGET_MIN, K_MOD_BUDGET_MAX]` = [0.85, 1.15]. *Context: `m27_conductor`.* See also: field decision, coupling K.

**Opt-out** -- A sphere's right to decline participation in specific fleet activities (task routing, cascade handoffs, POVM persistence) without being removed from the field. Modelled on clinical self-determination. Opt-out state is stored per-sphere in the consent system. *Context: `m10_hook_server` consent endpoints.* See also: consent gate.

---

## 4. RALPH Evolution (10 terms)

**RALPH** -- **R**ecognize, **A**nalyze, **L**earn, **P**ropose, **H**arvest. The 5-phase evolutionary engine that continuously improves ORAC's parameters. Each phase runs for multiple ticks, cycling through the full loop to propose, test, and accept or reject mutations. *Context: `m36_ralph_engine`.* See also: generation, mutation, fitness tensor.

**Recognize** -- RALPH Phase 1. Survey the field, identify drifting parameters, query VMS for semantic context. Establishes a baseline understanding of current system state before analysis. *Context: `m36_ralph_engine`.* See also: Analyze, RALPH.

**Analyze** -- RALPH Phase 2. Compute the 12-dimensional fitness tensor, detect trends via linear regression over a 10-generation sliding window. Determines whether the system is improving, degrading, or stable. *Context: `m36_ralph_engine`, `m39_fitness_tensor`.* See also: Recognize, Learn, fitness tensor.

**Learn** -- RALPH Phase 3. Mine correlations from mutation history: temporal (consecutive fitness improvements), causal (parameter change leads to fitness delta), recurring (same mutation succeeds repeatedly), fitness-linked (high/low fitness episodes). Feeds discovered patterns into the Propose phase. *Context: `m36_ralph_engine`, `m38_correlation_engine`.* See also: Analyze, Propose, correlation.

**Propose** -- RALPH Phase 4. Generate diverse mutations using round-robin parameter selection with per-parameter cooldown (10 generations) and diversity rejection gate (>50% same parameter in last 20 = reject). Snapshots system state before applying the mutation for atomic rollback. *Context: `m36_ralph_engine`, `m40_mutation_selector`.* See also: Learn, Harvest, mutation, snapshot.

**Harvest** -- RALPH Phase 5. Evaluate the mutation after `DEFAULT_VERIFICATION_TICKS` = 10 ticks. Accept if fitness improved by >= 0.02 (`DEFAULT_ACCEPT_THRESHOLD`). Rollback if fitness dropped by >= 0.01 (`DEFAULT_ROLLBACK_THRESHOLD`). Otherwise continue observing. *Context: `m36_ralph_engine`.* See also: Propose, rollback, fitness tensor.

**Mutation** -- A proposed change to one of 10 mutable parameters: K_modulation, r_target, thermal_setpoint, dispatch_timeout, ltp_alpha, ltd_alpha, breaker failure threshold, breaker success threshold, session_ttl, emergence_confidence_min. Each mutation is small and reversible. *Context: `m40_mutation_selector`.* See also: Propose, Harvest, rollback.

**Fitness tensor** -- A 12-dimensional weighted evaluation of fleet health. Each dimension (D0-D11) measures a different aspect of performance, weighted by importance (D0 coordination quality at 18% down to D11 consent compliance at 2%). The weighted sum produces a single scalar fitness in [0.0, 1.0]. *Context: `m39_fitness_tensor`.* See also: Analyze, fitness dimensions (Section 7).

**Generation** -- A single complete RALPH cycle (Recognize through Harvest). Each generation advances a counter. The system can run up to `DEFAULT_MAX_CYCLES` = 1,000 generations before auto-pausing. Generation count tracks evolutionary progress. *Context: `m36_ralph_engine`.* See also: RALPH, mutation.

**Snapshot / rollback** -- Before applying a mutation, RALPH saves a snapshot of all mutable parameters (`DEFAULT_SNAPSHOT_CAPACITY` = 50 snapshots deep). If the Harvest phase determines the mutation was harmful (fitness dropped >= 0.01), the system restores the snapshot, undoing the change. *Context: `m36_ralph_engine`.* See also: Harvest, mutation.

---

## 5. Architecture (12 terms)

**Layer (L1-L8)** -- ORAC's 8 architectural layers, each with a single responsibility. Strict DAG: lower layers know nothing about higher layers, enforced at compile time. L1 Core, L2 Wire, L3 Hooks (keystone), L4 Intelligence, L5 Bridges, L6 Coordination, L7 Monitoring, L8 Evolution. *Context: `lib.rs`, all `mod.rs` files.* See also: keystone layer.

**Feature gate** -- Conditional compilation using Rust's `#[cfg(feature = "...")]`. 6 features: `api` (L3), `persistence` (L5 blackboard), `bridges` (L5), `intelligence` (L4), `monitoring` (L7), `evolution` (L8). All enabled by default. Allows minimal builds for testing. *Context: `Cargo.toml`, `lib.rs`.* See also: layer.

**OracState** -- The central state struct in `m10_hook_server` with 32 fields covering every subsystem: field state, coupling network, STDP tracker, bridge handles, blackboard connection, consent declarations, ghost traces, RALPH state, emergence detector, and more. Wrapped in `Arc<RwLock<>>` for thread-safe access. *Context: `m10_hook_server`.* See also: shared state, blackboard.

**Blackboard** -- A SQLite database serving as the fleet's shared state store. Contains 10 tables: pane_status, task_history, agent_cards, ghost_traces, consent_declarations, consent_audit, hebbian_summary, ralph_state, sessions, coupling_weights. WAL mode for concurrent readers. *Context: `m26_blackboard`.* See also: WAL mode, OracState.

**Bridge** -- A module that connects ORAC to an external service. 6 bridges: SYNTHEX (thermal), ME (fitness), POVM (persistent memory), RM (cross-session knowledge), VMS (memory system), PV2 (fleet coordination). All bridges implement the `Bridgeable` trait. *Context: `m5_bridges/` directory, `m05_traits`.* See also: specific service entries in Section 6.

**Hook** -- A Claude Code lifecycle event intercepted by ORAC via HTTP POST. 6 hooks: SessionStart, UserPromptSubmit, PreToolUse, PostToolUse, Stop, PermissionRequest. L3 is called the "keystone" layer because hooks are the primary interface between Claude Code and ORAC. *Context: `m3_hooks/` directory.* See also: keystone layer.

**Tick** -- One heartbeat of the ORAC system, occurring every 5 seconds (`TICK_INTERVAL_SECS`). Each tick advances the Kuramoto field, runs STDP, executes the conductor advisory, checks for emergence events, and advances the RALPH cycle. The tick counter is a monotonically increasing u64. *Context: `m29_tick`, `main.rs`.* See also: tick interval (Section 6 of D6).

**Wire protocol** -- The V2 NDJSON (newline-delimited JSON) protocol used for IPC between ORAC and PV2 over a Unix socket. Defines a state machine (Disconnected through Active) and frame types (Handshake, Subscribe, Event, Cascade, etc.). Max frame size 64KB. *Context: `m09_wire_protocol`, `m08_bus_types`.* See also: BusFrame, IPC.

**BusFrame** -- A single message on the IPC bus. An enum with 11 variants: Handshake, Welcome, Subscribe, Subscribed, Submit, TaskSubmitted, Event, Cascade, CascadeAck, Disconnect, Error. Each frame is serialised as NDJSON and validated by the wire protocol state machine. *Context: `m08_bus_types`.* See also: wire protocol.

**Hot-swap** -- The design pattern where ORAC modules were initially cloned from the PV2 codebase (`candidate-modules/`) and adapted to ORAC's architecture. 18 "drop-in" files needed no changes; 6 "adapt" files needed ORAC-specific modifications marked with `## ADAPT` headers. *Context: `CLAUDE.local.md` candidate modules section.* See also: layer.

**Keystone layer** -- Layer 3 (Hooks). Called "keystone" because it is the primary interface between Claude Code and ORAC, importing from every other layer to orchestrate the full system. Every HTTP request arrives here, and every subsystem is wired through here. *Context: `m10_hook_server`.* See also: hook, layer.

**Shared state** -- The thread-safe wrapper around `AppState`: `Arc<RwLock<AppState>>`. Allows multiple async tasks (HTTP handlers, tick loop, bridge pollers, IPC listener) to read and write fleet state concurrently. Lock ordering rule: always acquire AppState before BusState to prevent deadlocks. *Context: `field_state.rs`, `m10_hook_server`.* See also: OracState.

---

## 6. Services (10 terms)

**SYNTHEX** -- The brain of the ULTRAPLATE developer environment. Runs on port 8090. Provides thermal regulation via a PID controller: temperature, target, PID output, heat sources. ORAC polls SYNTHEX every 6 ticks for temperature and posts field state back for bidirectional thermal regulation. *Context: `m22_synthex_bridge`.* See also: thermal damping, bridge.

**ME (Maintenance Engine)** -- The ULTRAPLATE system health observer. Runs on port 8080. Provides fitness signals via `/api/observer`. ORAC polls ME every 12 ticks and feeds the fitness value into D3 (TaskThroughput) of the fitness tensor. Also has an EventBus with 333K+ events but zero external subscribers. *Context: `m23_me_bridge`.* See also: fitness tensor, bridge.

**POVM (Persistent Oscillating Vortex Memory)** -- Long-term memory storage. Runs on port 8125. Stores memories, pathways (coupling relationships), and field snapshots. ORAC hydrates from POVM on session start and crystallises (writes back) coupling weights every 60 ticks. Max response 2MB. *Context: `m24_povm_bridge`.* See also: bridge, crystallisation.

**RM (Reasoning Memory)** -- Cross-session knowledge store. Runs on port 8130. Accepts data in TSV format ONLY (never JSON). ORAC writes observations every 60 ticks and reads via search queries. Agent name: `"orac-sidecar"`. *Context: `m25_rm_bridge`.* See also: TSV, bridge.

**VMS (Vortex Memory System)** -- Oscillating fractal sphere memory. Runs on port 8120. Provides 47 MCP tools, 12D morphogenic tensor, spherical harmonics, Saturn Light embeddings. ORAC posts memory updates every 30 ticks and triggers consolidation every 300 ticks. *Context: `main.rs` VMS bridge sections.* See also: bridge.

**PV2 (Pane-Vortex V2)** -- The fleet coordination daemon. Runs on port 8132. Manages the Kuramoto field, sphere registration, task lifecycle, and governance. ORAC connects to PV2 via HTTP polling (every 5 seconds) and Unix socket IPC for event-driven updates. *Context: `m10_hook_server` field poller, `m07_ipc_client`.* See also: wire protocol, IPC.

**ORAC** -- **O**rchestrating **R**untime for **A**gent **C**oordination. The intelligent proxy described by this glossary. Runs on port 8133. Intercepts Claude Code lifecycle events, enriches them with fleet intelligence, and coordinates across 6 upstream services. *Context: everything.* See also: hook, RALPH, Kuramoto.

**SAN-K7** -- ULTRAPLATE orchestrator with 59 modules (M1-M55) and 3,340 tests. Runs on port 8100. Provides module-level orchestration, command nexus, and neural adaptive integration. Not directly bridged by ORAC but part of the broader ULTRAPLATE ecosystem. *Context: `CLAUDE.md` services table.* See also: NAIS.

**NAIS (Neural Adaptive Intelligence System)** -- 327 NAM components across 5 subsystems (NOE/ARA/QSS/DCC/MLIL). Runs on port 8101. Provides neural adaptive intelligence via internal IPC rather than HTTP. Part of the ULTRAPLATE ecosystem. *Context: `CLAUDE.md` services table.* See also: SAN-K7.

**DevOps Engine** -- Neural orchestration service. Runs on port 8081. Batch 1 (no dependencies). Provides orchestration capabilities for the ULTRAPLATE ecosystem. Currently metabolically dormant (40 agents, load=0.0). *Context: `CLAUDE.md` services table.* See also: ULTRAPLATE.

---

## 7. Fitness Dimensions (12 terms)

**D0 -- CoordinationQuality (18%)** -- Measures coupling network density and health. How well-connected is the fleet? Considers number of active connections, mean weight, and distribution shape. The highest-weighted dimension because coordination is ORAC's primary purpose. *Context: `m39_fitness_tensor`.* See also: coupling K, STDP.

**D1 -- FieldCoherence (15%)** -- Measures the Kuramoto order parameter r. Higher r (up to the target) means better coordination. Penalises both chaos (r < 0.3) and over-synchronisation (r > 0.99). Optimal range: [0.85, 0.95]. *Context: `m39_fitness_tensor`.* See also: order parameter, coherence lock.

**D2 -- DispatchAccuracy (12%)** -- Measures semantic routing success rate. What fraction of dispatched tasks were completed by the pane they were routed to? Higher accuracy means the router is matching tasks to the right specialists. *Context: `m39_fitness_tensor`.* See also: dispatch, semantic router.

**D3 -- TaskThroughput (10%)** -- Measures the ME fitness signal: how efficiently is work flowing through the system? Sourced from the ME bridge's observer fitness poll. *Context: `m39_fitness_tensor`, `m23_me_bridge`.* See also: ME, bridge.

**D4 -- ErrorRate (10%)** -- Inverse of bridge and hook error frequency. Lower errors = higher fitness on this dimension. Tracks failed HTTP calls, parse errors, timeout events, and bridge unreachable states. *Context: `m39_fitness_tensor`.* See also: circuit breaker.

**D5 -- Latency (8%)** -- Measures SYNTHEX thermal convergence speed: how quickly does the system reach its temperature target? Faster convergence indicates better thermal regulation and more responsive bridges. *Context: `m39_fitness_tensor`, `m22_synthex_bridge`.* See also: SYNTHEX, thermal damping.

**D6 -- HebbianHealth (7%)** -- Measures weight distribution health. A healthy distribution has weights spread between floor and ceiling, not all clustered at one extreme. Penalises saturation (>80% at floor or ceiling). *Context: `m39_fitness_tensor`.* See also: STDP, weight floor, Hebbian saturation.

**D7 -- CouplingStability (6%)** -- Measures the fraction of circuit breakers in the Closed state. All breakers closed = maximum stability. Open breakers indicate service failures. *Context: `m39_fitness_tensor`, `m21_circuit_breaker`.* See also: circuit breaker.

**D8 -- ThermalBalance (5%)** -- Measures how close SYNTHEX temperature is to its target. Deviation from target reduces this dimension. Perfectly balanced = temperature equals setpoint. *Context: `m39_fitness_tensor`.* See also: SYNTHEX, thermal damping.

**D9 -- FleetUtilization (4%)** -- Measures the ratio of Working spheres to total spheres. Higher utilisation means the fleet is actively productive, not idle. Penalises both complete idleness and 100% utilisation (no slack). *Context: `m39_fitness_tensor`.* See also: sphere, idle ratio.

**D10 -- EmergenceRate (3%)** -- Measures detected emergence events per unit time. More emergence events (of beneficial types) indicate a healthy, adaptive fleet that is discovering new patterns. *Context: `m39_fitness_tensor`, `m37_emergence_detector`.* See also: emergence, coherence lock.

**D11 -- ConsentCompliance (2%)** -- Measures how well the fleet respects consent declarations. 100% compliance means no sphere was forced into an action it opted out of. The lowest-weighted dimension because violations are rare, but critical when they occur. *Context: `m39_fitness_tensor`.* See also: consent gate, opt-out.

---

## 8. Acronyms (14 terms)

**RALPH** -- Recognize, Analyze, Learn, Propose, Harvest. The 5-phase evolutionary engine. See Section 4.

**STDP** -- Spike-Timing Dependent Plasticity. The Hebbian learning rule. See Section 2.

**LTP** -- Long-Term Potentiation. Weight strengthening. Rate: 0.01/tick. See Section 2.

**LTD** -- Long-Term Depression. Weight weakening. Rate: 0.002/tick. See Section 2.

**POVM** -- Persistent Oscillating Vortex Memory. Long-term memory service on port 8125. See Section 6.

**PV2** -- Pane-Vortex V2. Fleet coordination daemon on port 8132. See Section 6.

**IPC** -- Inter-Process Communication. ORAC uses a Unix domain socket (`/run/user/1000/pane-vortex-bus.sock`) to communicate with PV2 using the V2 NDJSON wire protocol. See also: wire protocol, BusFrame.

**NDJSON** -- Newline-Delimited JSON. The wire protocol format used for IPC frames. Each frame is a single JSON object followed by a newline character. Max frame size: 64KB. See also: wire protocol, BusFrame.

**FSM** -- Finite State Machine. Used in the wire protocol (6 states: Disconnected through Closing) and the circuit breaker (3 states: Closed, Open, HalfOpen). See also: wire protocol, circuit breaker.

**WAL** -- Write-Ahead Logging. SQLite journaling mode used by the blackboard. Allows concurrent readers with a single writer. Busy timeout: 5,000ms. Retry strategy: 3 attempts at 100/200/400ms. See also: blackboard.

**OTel** -- OpenTelemetry. The distributed tracing standard used by `m32_otel_traces`. Records spans for every hook call, bridge poll, and task lifecycle event. W3C Trace Context compatible. See also: monitoring (L7).

**TSV** -- Tab-Separated Values. The ONLY format accepted by Reasoning Memory (port 8130). Format: `category\tagent\tconfidence\tttl\tcontent`. Never send JSON to RM. See also: RM.

**ADR** -- Architecture Decision Record. A formal record of a significant architectural decision, its context, and its consequences. Referenced in ORAC documentation but not stored as separate files. See also: layer.

**FMA** -- Fused Multiply-Add. A floating-point operation `a.mul_add(b, c)` that computes `(a * b) + c` in a single step with better precision than separate multiply and add. Required by ORAC's coding standard for all float arithmetic. See also: quality gate.

---

## 9. Operations / Build (8 terms)

**devenv** -- The ULTRAPLATE developer environment manager. Binary at `~/.local/bin/devenv`. Manages 17 active services across 5 dependency batches. Config at `~/.config/devenv/devenv.toml`. Known issue: `devenv stop` may leave processes alive (BUG-001). *Context: `CLAUDE.md`.* See also: batch, deploy sequence.

**Batch** -- A dependency-ordered startup group. 5 batches: Batch 1 (no deps: DevOps, CodeSynthor, POVM), Batch 2 (needs B1: SYNTHEX, SAN-K7, ME, Architect, Prometheus), Batch 3 (needs B2: NAIS, Bash Engine, Tool Maker), Batch 4 (needs B3: Context Manager, Tool Library, RM), Batch 5 (needs B4: VMS, PV2, ORAC). *Context: `CLAUDE.md` services table.* See also: devenv.

**Quality gate** -- The mandatory 4-step verification before any code is accepted: (1) `cargo check` (compilation), (2) `cargo clippy -- -D warnings` (standard lints), (3) `cargo clippy -- -D warnings -W clippy::pedantic` (strict lints), (4) `cargo test --lib --release` (all tests pass). Zero tolerance at every stage. *Context: `CLAUDE.md`.* See also: FMA.

**spawn_blocking** -- Tokio's mechanism for running blocking operations (like `ureq` HTTP calls or SQLite queries) on a dedicated thread pool without blocking the async runtime. All bridge polls and blackboard writes use `spawn_blocking`. *Context: `main.rs`, `m10_hook_server`.* See also: tick, bridge.

**WAL mode** -- SQLite's Write-Ahead Logging journal mode. Enables concurrent read access while a single writer holds the lock. The blackboard opens with `PRAGMA journal_mode=WAL` and `busy_timeout=5000`. Retry strategy for SQLITE_BUSY: 3 attempts at 100/200/400ms. *Context: `m26_blackboard`.* See also: blackboard, SQLite WAL contention (D6).

**Health path** -- The HTTP endpoint that devenv uses to verify a service is alive. ORAC's health path is `/health` on port 8133. Returns JSON with status, port, sessions, uptime_ticks, and bridge states. Each of the 17 ULTRAPLATE services has its own health path. *Context: `m10_hook_server`, `CLAUDE.md` services table.* See also: devenv, star probe.

**Star probe** -- A diagnostic script (`fleet-star.sh`) that queries ORAC, PV2, POVM, ME, SYNTHEX, and RM health endpoints, collecting fitness, r, temperature, memory count, and generation data. Appends TSV to `/tmp/fleet-star-generations.tsv` with burn-rate colouring and anomaly flags. *Context: `CLAUDE.local.md`.* See also: health path.

**Deploy sequence** -- The steps to deploy a new ORAC binary: (1) build release (`cargo build --release --features full`), (2) copy binary (`/usr/bin/cp -f target/release/orac-sidecar ~/.local/bin/`), (3) restart via devenv or manual `nohup` start, (4) verify health (`curl localhost:8133/health`), (5) run star probe to confirm fleet integration. Never use plain `cp` (alias trap). *Context: `CLAUDE.local.md`.* See also: devenv, quality gate.

---

## Reverse Index: Code Identifier to Glossary Term

> If you see this identifier in code, look up the corresponding glossary term.

| Code Identifier | Glossary Term |
|----------------|---------------|
| `PaneId` | Pane |
| `TaskId` | Dispatch |
| `PaneSphere` | Sphere |
| `OrderParameter` | Order parameter (r) |
| `OrderParameter.r` | Order parameter (r) |
| `OrderParameter.psi` | Psi |
| `FleetMode` | Fleet mode |
| `Point3D` | Buoy (used for buoy positioning) |
| `SphereMemory` | Sphere (memories field) |
| `Buoy` | Buoy |
| `AppState` | Shared state |
| `OracState` | OracState |
| `SharedState` | Shared state |
| `BusFrame` | BusFrame |
| `BusTask` | Dispatch |
| `TaskStatus` | Dispatch |
| `PvError` | Layer (L1 error handling) |
| `PvResult` | Layer (L1 error handling) |
| `Bridgeable` | Bridge |
| `HookEvent` | Hook |
| `HookResponse` | Hook |
| `StdpTracker` | STDP |
| `BreakerRegistry` | Circuit breaker (D7) |
| `BreakerState` | FSM |
| `HEBBIAN_LTP` | LTP |
| `HEBBIAN_LTD` | LTD |
| `HEBBIAN_BURST_MULTIPLIER` | Burst multiplier |
| `HEBBIAN_NEWCOMER_MULTIPLIER` | Newcomer boost |
| `HEBBIAN_WEIGHT_FLOOR` | Weight floor |
| `DEFAULT_WEIGHT` | Coupling K |
| `WEIGHT_EXPONENT` | Coupling K |
| `COUPLING_STEPS_PER_TICK` | RK4 |
| `KURAMOTO_DT` | RK4 |
| `TICK_INTERVAL_SECS` | Tick |
| `SPHERE_CAP` | Sphere |
| `GHOST_MAX` | Ghost trace |
| `MEMORY_MAX_COUNT` | Sphere |
| `SYNC_THRESHOLD` | Synchronisation |
| `TUNNEL_THRESHOLD` | Buoy |
| `PHASE_GAP_THRESHOLD` | Phase gap |
| `R_HIGH_THRESHOLD` | Order parameter (r) |
| `R_LOW_THRESHOLD` | Order parameter (r) |
| `R_TARGET_BASE` | K scaling |
| `R_TARGET_LARGE_FLEET` | K scaling |
| `LARGE_FLEET_THRESHOLD` | K scaling |
| `CONDUCTOR_GAIN` | P-controller |
| `EMERGENT_BLEND` | Conductor / P-controller |
| `K_MOD_MIN` / `K_MOD_MAX` | Coupling K |
| `K_MOD_BUDGET_MIN` / `K_MOD_BUDGET_MAX` | Field action |
| `DECAY_PER_STEP` | Homeostatic normalisation |
| `SWEEP_BOOST` | Buoy |
| `ACTIVATION_THRESHOLD` | Buoy |
| `MEMORY_PRUNE_INTERVAL` | Sphere |
| `SEMANTIC_NUDGE_STRENGTH` | Semantic domain |
| `NEWCOMER_STEPS` | Newcomer boost |
| `SNAPSHOT_INTERVAL` | Snapshot / rollback |
| `WARMUP_TICKS` | Tick |
| `DEFAULT_PORT` | PV2 |
| `TWO_PI` | Phase |
| `CLUSTER_PROXIMITY` | Phase cluster |
| `CHIMERA_GAP` | Chimera |
| `CHIMERA_R_THRESHOLD` | Chimera |
| `IDLE_RATIO_THRESHOLD` | Fleet utilization (D9) |
| `auto_scale_k()` | K scaling |
| `field_poller` | PV2, shared state |
| `cached_field` | Shared state |
| `spawn_blocking` | spawn_blocking |
| `raw_http_get()` / `raw_http_post()` | Bridge |
| `DEFAULT_TCP_TIMEOUT_MS` | Bridge |
| `persist_stdp_to_povm` | POVM, crystallisation |
| `feed_emergence_observations` | Emergence (D10) |
| `ralph_loop` | RALPH |
| `TickResult` | Tick |
| `FitnessTensor` | Fitness tensor |
| `EmergenceEvent` | Emergence (D10) |
| `MutationSelector` | Mutation |
| `CorrelationEngine` | Correlation (Learn phase) |

---

*Generated: 2026-03-25 | Source: `/home/louranicas/claude-code-workspace/orac-sidecar/`*
*Obsidian backlinks: `[[Session 062 -- ORAC System Atlas (ACP)]]`, `[[EXECUTIVE_SUMMARY]]`, `[[D6_CAPACITY_LIMITS_REFERENCE]]`*
