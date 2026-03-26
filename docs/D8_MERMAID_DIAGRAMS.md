# D8: ORAC Sidecar -- Mermaid Diagrams

> **Version:** 0.6.0 | **Verified:** 2026-03-25 (Round 3 corrected module names)
> **8 diagrams** covering architecture, hooks, bridges, RALPH, IPC, state, startup, and data flow

---

## Diagram 1: Corrected Layer Architecture

All 40 modules with verified names, feature gates as subgraph labels, dependency arrows between layers.

```mermaid
%%{init: {'theme': 'dark'}}%%
graph TB
    subgraph L1["L1: Core (always compiled)"]
        m01["m01<br/>core_types"]
        m02["m02<br/>error_handling"]
        m03["m03<br/>config"]
        m04["m04<br/>constants"]
        m05["m05<br/>traits"]
        m06["m06<br/>validation"]
        fs["field_state"]
    end

    subgraph L2["L2: Wire (always compiled)"]
        m07["m07<br/>ipc_client"]
        m08["m08<br/>bus_types"]
        m09["m09<br/>wire_protocol"]
    end

    subgraph L3["L3: Hooks #91;feature: api#93;"]
        m10["m10<br/>hook_server<br/>(KEYSTONE)"]
        m11["m11<br/>session_hooks"]
        m12["m12<br/>tool_hooks"]
        m13["m13<br/>prompt_hooks"]
        m14["m14<br/>permission_policy"]
    end

    subgraph L4["L4: Intelligence #91;feature: intelligence#93;"]
        m15["m15<br/>coupling_network"]
        m16["m16<br/>auto_k"]
        m17["m17<br/>topology"]
        m18["m18<br/>hebbian_stdp"]
        m19["m19<br/>buoy_network"]
        m20["m20<br/>semantic_router"]
        m21["m21<br/>circuit_breaker"]
    end

    subgraph L5["L5: Bridges #91;feature: bridges+persistence#93;"]
        hh["http_helpers"]
        m22["m22<br/>synthex_bridge"]
        m23["m23<br/>me_bridge"]
        m24["m24<br/>povm_bridge"]
        m25["m25<br/>rm_bridge"]
        m26["m26<br/>blackboard"]
    end

    subgraph L6["L6: Coordination (always compiled)"]
        m27["m27<br/>conductor"]
        m28["m28<br/>cascade"]
        m29["m29<br/>tick"]
        m30["m30<br/>wasm_bridge"]
        m31["m31<br/>memory_manager"]
    end

    subgraph L7["L7: Monitoring #91;feature: monitoring#93;"]
        m32["m32<br/>otel_traces"]
        m33["m33<br/>metrics_export"]
        m34["m34<br/>field_dashboard"]
        m35["m35<br/>token_accounting"]
    end

    subgraph L8["L8: Evolution #91;feature: evolution#93;"]
        m36["m36<br/>ralph_engine"]
        m37["m37<br/>emergence_detector"]
        m38["m38<br/>correlation_engine"]
        m39["m39<br/>fitness_tensor"]
        m40["m40<br/>mutation_selector"]
    end

    %% DAG dependency arrows (verified)
    L2 -->|"types, errors"| L1
    L3 -->|"config, field_state"| L1
    L3 -->|"IPC state"| L2
    L4 -->|"types, constants"| L1
    L4 -->|"bus events"| L2
    L5 -->|"types, traits"| L1
    L6 -->|"field_state, types"| L1
    L6 -->|"bus frames"| L2
    L6 -->|"coupling, auto_k"| L4
    L6 -->|"blackboard"| L5
    L7 -->|"PaneId, TaskId"| L1
    L7 -->|"bus events"| L2
    L7 -->|"bridge types"| L5
    L8 -->|"core types"| L1
    L8 -->|"coupling, STDP"| L4
    L8 -->|"bridge data"| L5
    L8 -->|"trace spans"| L7

    %% L3 keystone cross-references (feature-gated)
    L3 -.->|"CouplingNetwork<br/>BreakerRegistry"| L4
    L3 -.->|"Blackboard, Bridges"| L5
    L3 -.->|"TraceStore<br/>Dashboard"| L7
    L3 -.->|"RalphEngine<br/>EmergenceDetector"| L8

    style L1 fill:#1a3a2a,stroke:#4ade80,color:#fff
    style L2 fill:#1a2a3a,stroke:#60a5fa,color:#fff
    style L3 fill:#3a1a3a,stroke:#c084fc,color:#fff
    style L4 fill:#3a3a1a,stroke:#facc15,color:#fff
    style L5 fill:#3a1a1a,stroke:#f87171,color:#fff
    style L6 fill:#1a3a3a,stroke:#2dd4bf,color:#fff
    style L7 fill:#2a2a3a,stroke:#818cf8,color:#fff
    style L8 fill:#3a2a1a,stroke:#fb923c,color:#fff
```

---

## Diagram 2: Hook Lifecycle Sequence

All 6 hook events showing the flow from Claude Code through the shell forwarder to ORAC and back, including bridge side effects.

```mermaid
%%{init: {'theme': 'dark'}}%%
sequenceDiagram
    participant CC as Claude Code
    participant SH as orac-hook.sh
    participant ORAC as ORAC :8133
    participant PV2 as PV2 :8132
    participant SYN as SYNTHEX :8090
    participant POVM as POVM :8125
    participant RM as RM :8130
    participant BB as Blackboard (SQLite)

    Note over CC,ORAC: SessionStart (5s timeout)
    CC->>SH: stdin JSON (event: SessionStart)
    SH->>ORAC: POST /hooks/session_start
    ORAC->>PV2: Register sphere via IPC
    ORAC->>POVM: GET /hydrate (restore pathways)
    ORAC->>RM: GET /search?q=session (restore context)
    ORAC->>BB: INSERT pane_status + agent_card
    ORAC-->>SH: {"decision":"approve","inject":"Welcome..."}
    SH-->>CC: stdout JSON

    Note over CC,ORAC: UserPromptSubmit (3s timeout)
    CC->>SH: stdin JSON (prompt text)
    SH->>ORAC: POST /hooks/user_prompt_submit
    ORAC->>ORAC: Read cached field_state (r, tick, spheres)
    ORAC->>ORAC: Check pending tasks from blackboard
    ORAC-->>SH: {"decision":"approve","inject":"[r=0.92, K=1.2, ...]"}
    SH-->>CC: stdout JSON (field state injected)

    Note over CC,ORAC: PreToolUse (2s timeout)
    CC->>SH: stdin JSON (tool_name, input)
    SH->>ORAC: POST /hooks/pre_tool_use
    ORAC->>SYN: Check thermal gate (cached)
    ORAC-->>SH: {"decision":"approve"} or {"decision":"deny","reason":"thermal"}
    SH-->>CC: stdout JSON

    Note over CC,ORAC: PostToolUse (3s timeout)
    CC->>SH: stdin JSON (tool_name, output)
    SH->>ORAC: POST /hooks/post_tool_use
    ORAC->>ORAC: Increment total_tool_calls
    ORAC->>ORAC: Semantic classify + route
    ORAC->>BB: UPSERT pane_status + task_history
    ORAC->>ORAC: Record trace span (monitoring)
    ORAC->>ORAC: Record pane token usage
    ORAC-->>SH: {"decision":"approve"}
    SH-->>CC: stdout JSON

    Note over CC,ORAC: Stop (5s timeout)
    CC->>SH: stdin JSON (event: Stop)
    SH->>ORAC: POST /hooks/stop
    ORAC->>BB: Fail active tasks, save ghost record
    ORAC->>PV2: Deregister sphere via IPC
    ORAC->>ORAC: Record ghost trace
    ORAC-->>SH: {"decision":"approve"}
    SH-->>CC: stdout JSON

    Note over CC,ORAC: PermissionRequest (2s timeout)
    CC->>SH: stdin JSON (tool_name, action)
    SH->>ORAC: POST /hooks/permission_request
    ORAC->>ORAC: Policy check (read=allow, write=notice, deny list)
    ORAC->>BB: Audit consent decision
    ORAC-->>SH: {"decision":"approve"} or {"decision":"deny"}
    SH-->>CC: stdout JSON
```

---

## Diagram 3: Bridge Topology

ORAC's connections to 6 external services with data flow direction, protocol, and tick cadences.

```mermaid
%%{init: {'theme': 'dark'}}%%
graph LR
    subgraph ORAC["ORAC Sidecar :8133"]
        HK["Hook Server<br/>(m10)"]
        SB["SynthexBridge<br/>(m22)"]
        MB["MeBridge<br/>(m23)"]
        PB["PovmBridge<br/>(m24)"]
        RB["RmBridge<br/>(m25)"]
        BB["Blackboard<br/>(m26)"]
        IPC["IpcClient<br/>(m07)"]
    end

    PV2["PV2 Daemon<br/>:8132"]
    SYN["SYNTHEX<br/>:8090"]
    ME["Maintenance Engine<br/>:8080"]
    POVM["POVM Engine<br/>:8125"]
    RM["Reasoning Memory<br/>:8130"]
    VMS["Vortex Memory System<br/>:8120"]

    %% PV2 -- bidirectional
    IPC <-->|"Unix socket IPC<br/>field.* + sphere.*<br/>continuous"| PV2
    HK -->|"HTTP GET /health<br/>field poller<br/>every 5s"| PV2

    %% SYNTHEX -- bidirectional
    SB -->|"HTTP POST /api/ingest<br/>12 heat source fields<br/>tick%6"| SYN
    SB -->|"HTTP GET /v3/thermal<br/>k_adjustment read<br/>every poll cycle"| SYN

    %% ME -- read only
    MB -->|"HTTP GET /api/observer<br/>fitness signal<br/>tick%12"| ME

    %% POVM -- bidirectional
    PB -->|"HTTP POST /pathways<br/>STDP weights<br/>tick%60"| POVM
    PB -->|"HTTP GET /hydrate<br/>pathway restore<br/>on SessionStart"| POVM

    %% RM -- write only
    RB -->|"HTTP POST /put (TSV)<br/>RALPH state<br/>tick%60"| RM
    RB -->|"HTTP POST /put (TSV)<br/>emergence events<br/>each tick"| RM

    %% VMS -- bidirectional (via raw HTTP helpers, not dedicated bridge module)
    HK -->|"HTTP POST /mcp/tools/call<br/>field observations<br/>tick%30"| VMS
    HK -->|"HTTP POST /v1/adaptation/trigger<br/>consolidation<br/>tick%300"| VMS
    HK -->|"HTTP POST /mcp/tools/call<br/>semantic query<br/>tick%30 (Recognize)"| VMS

    %% Blackboard -- local
    BB <-->|"SQLite WAL<br/>9 tables<br/>tick%6 hebbian<br/>tick%60 RALPH+sessions"| BB

    style ORAC fill:#1a1a2e,stroke:#e94560,color:#fff
    style PV2 fill:#0f3460,stroke:#60a5fa,color:#fff
    style SYN fill:#0f3460,stroke:#60a5fa,color:#fff
    style ME fill:#0f3460,stroke:#60a5fa,color:#fff
    style POVM fill:#0f3460,stroke:#60a5fa,color:#fff
    style RM fill:#0f3460,stroke:#60a5fa,color:#fff
    style VMS fill:#0f3460,stroke:#60a5fa,color:#fff
```

---

## Diagram 4: RALPH Evolution State Machine

The 5-phase RALPH cycle: Recognize, Analyze, Learn, Propose, Harvest. Shows transitions, triggers, and side effects.

```mermaid
%%{init: {'theme': 'dark'}}%%
stateDiagram-v2
    [*] --> Recognize: RALPH engine created

    Recognize --> Analyze: Observations collected
    note right of Recognize
        Sample 12D tensor from live state
        Query VMS for semantic memories (tick%30)
        Feed emergence events to correlation engine
        Build fitness snapshot
    end note

    Analyze --> Learn: Patterns identified
    note right of Analyze
        Correlation mining: temporal, causal, recurring
        Pathway discovery with establishment threshold
        Fitness trend analysis (linear regression)
        Stability/volatility assessment
    end note

    Learn --> Propose: Correlations learned
    note right of Learn
        Update coupling weights from STDP
        Persist Hebbian summary to blackboard (tick%6)
        Record emergence patterns
        Update fitness baseline
    end note

    Propose --> Harvest: Mutation selected
    note right of Propose
        Diversity-enforced parameter selection
        Round-robin cycling + 10-gen cooldown
        >50% diversity rejection gate (BUG-035 fix)
        Snapshot state for potential rollback
    end note

    Harvest --> Recognize: Cycle complete, gen++
    note right of Harvest
        Evaluate mutation outcome (accepted/rejected)
        Rollback if fitness decreased
        Persist RALPH state to blackboard (tick%60)
        Persist to RM as TSV (tick%60)
        Increment generation counter
    end note

    Recognize --> Paused: max_cycles reached
    Paused --> Recognize: Manual resume

    state "Snapshot/Rollback" as SR {
        [*] --> PreMutation: Snapshot taken
        PreMutation --> PostMutation: Mutation applied
        PostMutation --> Accepted: fitness improved
        PostMutation --> RolledBack: fitness decreased
        Accepted --> [*]
        RolledBack --> [*]: State restored
    }
```

---

## Diagram 5: IPC Wire Protocol State Machine

V2 wire protocol FSM for PV2 bus connection: Disconnected through Active with reconnection handling.

```mermaid
%%{init: {'theme': 'dark'}}%%
stateDiagram-v2
    [*] --> Disconnected

    Disconnected --> Handshaking: connect_with_backoff() success
    note right of Disconnected
        Initial state or after recv error
        Escalating backoff: 5s base, 30s cap
        Total reconnects tracked
    end note

    Handshaking --> Connected: Handshake frame exchanged
    note right of Handshaking
        Send: ClientHello with PaneId
        Recv: ServerWelcome with session_id
        V2 wire format validation
    end note

    Connected --> Subscribing: subscribe() called
    note right of Connected
        Connection established
        Session ID assigned
        Ready for subscription
    end note

    Subscribing --> Active: Subscribe ACK received
    note right of Subscribing
        Subscribe to: field.*, sphere.*
        Configurable via PvConfig.ipc.subscribe_patterns
        Count of matched patterns returned
    end note

    Active --> Active: BusFrame::Event received
    note right of Active
        process_bus_event() dispatches:
        - field.tick / field.state --> update cached r, psi
        - sphere.registered --> add to spheres map
        - sphere.deregistered --> remove from spheres map
        - sphere.status --> update phase
        - unknown --> no-op
    end note

    Active --> Disconnected: recv error or timeout
    Handshaking --> Disconnected: handshake failed
    Connected --> Disconnected: connection lost
    Subscribing --> Disconnected: subscribe failed

    note left of Disconnected
        On disconnect:
        - client.disconnect() to release socket
        - ipc_state = "disconnected"
        - Backoff delay before retry
        - Backoff resets on successful connect
    end note
```

---

## Diagram 6: OracState Field Diagram

All 32 fields grouped by subsystem with feature gate annotations.

```mermaid
%%{init: {'theme': 'dark'}}%%
graph TB
    subgraph CONFIG["Configuration (5 fields)"]
        cfg["config: PvConfig"]
        pv2["pv2_url: String"]
        syn_url["synthex_url: String"]
        povm_url["povm_url: String"]
        rm_url["rm_url: String"]
    end

    subgraph CORE["Core State (5 fields)"]
        fstate["field_state: SharedState<br/>(RwLock&lt;AppState&gt;)"]
        sess["sessions: RwLock&lt;HashMap&gt;"]
        tick["tick: AtomicU64"]
        ipc["ipc_state: RwLock&lt;String&gt;"]
        ghosts["ghosts: RwLock&lt;VecDeque&gt;"]
    end

    subgraph GOV["Governance (1 field)"]
        cons["consents: RwLock&lt;HashMap&gt;"]
    end

    subgraph PERSIST["Persistence (1 field) #91;persistence#93;"]
        bb["blackboard: Option&lt;Mutex&lt;Blackboard&gt;&gt;"]
    end

    subgraph EVOL["Evolution (1 field) #91;evolution#93;"]
        ralph["ralph: RalphEngine"]
    end

    subgraph INTEL["Intelligence (2 fields)"]
        coup["coupling: RwLock&lt;CouplingNetwork&gt;"]
        brk["breakers: RwLock&lt;BreakerRegistry&gt;<br/>#91;intelligence#93;"]
    end

    subgraph DISPATCH["Dispatch Counters (5 fields)"]
        dt["dispatch_total: AtomicU64"]
        dr["dispatch_read: AtomicU64"]
        dw["dispatch_write: AtomicU64"]
        de["dispatch_execute: AtomicU64"]
        dc["dispatch_communicate: AtomicU64"]
    end

    subgraph HEBBIAN["Hebbian Tracking (4 fields)"]
        coact["co_activations_total: AtomicU64"]
        ltp["hebbian_ltp_total: AtomicU64"]
        ltd["hebbian_ltd_total: AtomicU64"]
        hlast["hebbian_last_tick: AtomicU64"]
    end

    subgraph BRIDGES["Bridge Instances (3 fields) #91;bridges#93;"]
        meb["me_bridge: MeBridge"]
        rmb["rm_bridge: RmBridge"]
        sxb["synthex_bridge: SynthexBridge"]
    end

    subgraph TOOLS["Tool Tracking (2 fields)"]
        ttc["total_tool_calls: AtomicU64"]
        tclt["tool_calls_at_last_thermal: AtomicU64"]
    end

    subgraph MON["Monitoring (3 fields) #91;monitoring#93;"]
        traces["trace_store: TraceStore"]
        dash["dashboard: FieldDashboard"]
        tokens["token_accountant: TokenAccountant"]
    end

    style CONFIG fill:#1a3a2a,stroke:#4ade80,color:#fff
    style CORE fill:#1a2a3a,stroke:#60a5fa,color:#fff
    style GOV fill:#2a2a3a,stroke:#c084fc,color:#fff
    style PERSIST fill:#3a1a1a,stroke:#f87171,color:#fff
    style EVOL fill:#3a2a1a,stroke:#fb923c,color:#fff
    style INTEL fill:#3a3a1a,stroke:#facc15,color:#fff
    style DISPATCH fill:#1a3a3a,stroke:#2dd4bf,color:#fff
    style HEBBIAN fill:#2a1a3a,stroke:#a78bfa,color:#fff
    style BRIDGES fill:#3a1a2a,stroke:#fb7185,color:#fff
    style TOOLS fill:#1a2a2a,stroke:#67e8f9,color:#fff
    style MON fill:#2a3a2a,stroke:#86efac,color:#fff
```

---

## Diagram 7: Daemon Startup Sequence

The `orac-sidecar` main.rs startup flow from process launch to steady state.

```mermaid
%%{init: {'theme': 'dark'}}%%
flowchart TB
    START([orac-sidecar launched]) --> TRACE[Initialize tracing_subscriber<br/>with env filter]
    TRACE --> CONFIG[Load PvConfig<br/>TOML + env overlay via figment]
    CONFIG --> STATE["Construct Arc&lt;OracState&gt;<br/>32 fields initialized"]

    STATE --> HYDRATE{hydrate_startup_state}

    subgraph HYDRATE_STEPS["Hydration (4 steps, non-fatal)"]
        H1["1. RALPH state from blackboard<br/>#91;persistence + evolution#93;"]
        H2["2. Active sessions from blackboard<br/>#91;persistence#93;"]
        H3["3. Coupling weights from blackboard<br/>#91;persistence + intelligence#93;"]
        H4["4. Coupling weights from POVM<br/>#91;persistence + bridges#93; (fallback)"]
        H1 --> H2 --> H3 --> H4
    end

    HYDRATE --> HYDRATE_STEPS
    HYDRATE_STEPS --> SPAWN_POLLER

    SPAWN_POLLER["spawn_field_poller()<br/>PV2 :8132/health polling<br/>updates SharedState"] --> SPAWN_IPC

    SPAWN_IPC["spawn_ipc_listener()<br/>PV2 Unix socket bus<br/>field.* + sphere.* events<br/>2s initial delay"] --> HALT_CHAN

    HALT_CHAN["Create halt channel<br/>watch::channel(false)"] --> SPAWN_RALPH

    SPAWN_RALPH["spawn_ralph_loop()<br/>5s interval tick<br/>#91;evolution#93;"] --> BUILD_ADDR

    BUILD_ADDR["Resolve bind address<br/>config.server.bind_addr:port"] --> BUILD_ROUTER

    BUILD_ROUTER["build_router(state)<br/>Axum router with 18 endpoints"] --> BIND

    BIND["TcpListener::bind(addr)<br/>:8133"] --> SERVE

    SERVE["axum::serve() with<br/>graceful_shutdown(SIGINT)"]

    SERVE --> STEADY([Steady State:<br/>HTTP hooks + RALPH ticks +<br/>IPC events + bridge polls])

    SERVE --> SHUTDOWN["SIGINT received<br/>halt_send(true)<br/>RALPH loop stops"]
    SHUTDOWN --> EXIT([Process exit])

    style START fill:#1a3a2a,stroke:#4ade80,color:#fff
    style STEADY fill:#0f3460,stroke:#60a5fa,color:#fff
    style EXIT fill:#3a1a1a,stroke:#f87171,color:#fff
    style HYDRATE_STEPS fill:#1a1a2e,stroke:#818cf8,color:#fff
```

---

## Diagram 8: Cross-Service Data Flow with Cadences

All data pipelines between ORAC and external services, annotated with tick cadences (1 tick = 5 seconds).

```mermaid
%%{init: {'theme': 'dark'}}%%
graph TB
    subgraph ORAC["ORAC Sidecar :8133"]
        RALPH["RALPH Engine<br/>5s tick interval"]
        STDP["Hebbian STDP<br/>every tick"]
        EMERGE["Emergence Detector<br/>every tick"]
        TENSOR["Fitness Tensor<br/>12D, every tick"]
        CONDUCTOR["Conductor<br/>every tick"]
    end

    subgraph PV2_SVC["PV2 :8132"]
        PV2_H["HTTP /health"]
        PV2_IPC["IPC Bus (Unix socket)"]
    end

    subgraph SYN_SVC["SYNTHEX :8090"]
        SYN_ING["/api/ingest"]
        SYN_TH["/v3/thermal"]
        SYN_DEC["/v3/decay/trigger"]
    end

    subgraph ME_SVC["ME :8080"]
        ME_OBS["/api/observer"]
    end

    subgraph POVM_SVC["POVM :8125"]
        POVM_PW["/pathways"]
        POVM_HY["/hydrate"]
    end

    subgraph RM_SVC["RM :8130 (TSV only)"]
        RM_PUT["POST /put"]
        RM_SEARCH["GET /search"]
    end

    subgraph VMS_SVC["VMS :8120"]
        VMS_MCP["/mcp/tools/call"]
        VMS_ADAPT["/v1/adaptation/trigger"]
    end

    subgraph BB_SVC["Blackboard (SQLite)"]
        BB_RALPH["ralph_state table"]
        BB_SESS["sessions table"]
        BB_COUP["coupling_weights table"]
        BB_HEB["hebbian_summary table"]
        BB_PANE["pane_status table"]
    end

    %% PV2 flows
    ORAC -->|"HTTP GET<br/>continuous (5s)"| PV2_H
    PV2_IPC -->|"field.tick / sphere.*<br/>continuous stream"| ORAC

    %% SYNTHEX flows
    RALPH -->|"tick%6 (30s)<br/>12 heat source fields"| SYN_ING
    ORAC -->|"every poll cycle<br/>k_adjustment read"| SYN_TH
    ORAC -->|"once<br/>PID reset on first ingest"| SYN_DEC

    %% ME flows
    ORAC -->|"tick%12 (60s)<br/>fitness signal"| ME_OBS

    %% POVM flows
    STDP -->|"tick%60 (5min)<br/>top 10 weights"| POVM_PW
    ORAC -->|"on SessionStart<br/>pathway restore"| POVM_HY

    %% RM flows
    RALPH -->|"tick%60 (5min)<br/>RALPH state TSV"| RM_PUT
    EMERGE -->|"each tick<br/>new emergence events TSV"| RM_PUT
    ORAC -->|"on SessionStart<br/>context restore"| RM_SEARCH

    %% VMS flows
    RALPH -->|"tick%30 (2.5min)<br/>field observation memory"| VMS_MCP
    RALPH -->|"tick%30 in Recognize<br/>semantic query (k=5)"| VMS_MCP
    ORAC -->|"tick%300 (25min)<br/>consolidation trigger"| VMS_ADAPT

    %% Blackboard flows
    RALPH -->|"tick%60 (5min)"| BB_RALPH
    RALPH -->|"tick%60 (5min)"| BB_SESS
    STDP -->|"tick%60 (5min)"| BB_COUP
    STDP -->|"tick%6 (30s)"| BB_HEB
    ORAC -->|"on PostToolUse"| BB_PANE

    %% Internal flows
    TENSOR -->|"every tick"| RALPH
    STDP -->|"every tick"| TENSOR
    EMERGE -->|"every tick"| RALPH
    CONDUCTOR -->|"every tick"| ORAC

    style ORAC fill:#1a1a2e,stroke:#e94560,color:#fff
    style PV2_SVC fill:#0f3460,stroke:#60a5fa,color:#fff
    style SYN_SVC fill:#0f3460,stroke:#60a5fa,color:#fff
    style ME_SVC fill:#0f3460,stroke:#60a5fa,color:#fff
    style POVM_SVC fill:#0f3460,stroke:#60a5fa,color:#fff
    style RM_SVC fill:#0f3460,stroke:#60a5fa,color:#fff
    style VMS_SVC fill:#0f3460,stroke:#60a5fa,color:#fff
    style BB_SVC fill:#2a1a3a,stroke:#a78bfa,color:#fff
```

### Cadence Reference Table

| Pipeline | Direction | Cadence | Real Time (1 tick = 5s) |
|----------|-----------|---------|------------------------|
| PV2 field poller | ORAC -> PV2 | every tick | 5s |
| PV2 IPC bus | PV2 -> ORAC | continuous | sub-second |
| SYNTHEX ingest | ORAC -> SYNTHEX | tick%6 | 30s |
| SYNTHEX thermal | SYNTHEX -> ORAC | every poll cycle | ~5s |
| ME observer | ORAC -> ME | tick%12 | 60s |
| POVM pathway persist | ORAC -> POVM | tick%60 | 5min |
| POVM hydrate | POVM -> ORAC | on SessionStart | event-driven |
| RM state persist | ORAC -> RM | tick%60 | 5min |
| RM emergence relay | ORAC -> RM | each tick | 5s (when events exist) |
| VMS memory post | ORAC -> VMS | tick%30 | 2.5min |
| VMS semantic query | VMS -> ORAC | tick%30 (Recognize) | 2.5min |
| VMS consolidation | ORAC -> VMS | tick%300 | 25min |
| Blackboard RALPH | ORAC -> SQLite | tick%60 | 5min |
| Blackboard sessions | ORAC -> SQLite | tick%60 | 5min |
| Blackboard coupling | ORAC -> SQLite | tick%60 | 5min |
| Blackboard hebbian | ORAC -> SQLite | tick%6 | 30s |
| Blackboard pane_status | ORAC -> SQLite | on PostToolUse | event-driven |
| Blackboard prune | ORAC -> SQLite | tick%60 | 5min |
| Homeostatic normalization | internal | tick%120 | 10min |
| STDP pass | internal | every tick | 5s |
| RALPH tick | internal | every tick | 5s |
| Emergence detection | internal | every tick | 5s |
| Conductor advisory | internal | every tick | 5s |
| Breaker FSM tick | internal | every tick | 5s |
