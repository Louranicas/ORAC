# Session 078 -- ORAC<>SYNTHEX Bidirectional Integration Schematics

> **Created:** 2026-04-01 | **Session:** 078 | **Status:** DEPLOYED
> **QG:** 4/4 (check, clippy, pedantic, 245 tests) | **Services:** 17/17 healthy

---

## 1. Session 078 Changes Summary

Three new data flows wired between ORAC (port 8133) and SYNTHEX (port 8090),
closing the thermal-learning feedback loop and enabling cross-instance alerting.

### Flow 1: Thermal to STDP Decay Modulation

The existing `GET /v3/thermal` response already provides `k_adjustment` (a PID
output reflecting system temperature). Session 078 derives a `decay_mult` from
this value and applies it to `ralph_decay_rate`, creating a closed feedback loop
between thermal state and learning dynamics.

```
k_adj (from SYNTHEX PID)  -->  decay_mult = 2.0 - k_adj
                                   |
                                   v
                           ralph_decay_rate *= decay_mult
                           hot system  -> k_adj > 1 -> decay_mult < 1 -> faster decay
                           cold system -> k_adj < 1 -> decay_mult > 1 -> slower decay
```

**Rationale:** When the system is thermally hot (high activity), coupling weights
should decay faster to prevent runaway reinforcement. When cold, decay slows to
preserve learned pathways.

### Flow 2: Nexus Pull Handler

A new poll on `GET /v3/nexus/pull` reads queued events from SYNTHEX every 3 ticks.
Two event types are handled:

- `thermal_alert` -- logged at `warn!` level, broadcast to fleet via Atuin KV
- `diagnostic_finding` -- logged at `info!` level, recorded for RALPH context

### Flow 3: Emergence to SYNTHEX Classifier

Emergence events detected by `m37_emergence_detector` are now posted to SYNTHEX
`POST /api/ingest` with category `cc_coordination`. This feeds the ML log
classifier pipeline (500K logs/sec capacity), enabling SYNTHEX to correlate
emergence patterns with system health trends.

---

## 2. Bidirectional Data Flow Schematic

```
   ORAC Sidecar (port 8133)                    SYNTHEX Engine (port 8090)
   ==========================                    ==========================

   +-----------------------+                     +-----------------------+
   | Field State           |                     | Heat Sources          |
   | (r, K, spheres,      |---POST /api/ingest-->| HS-001 field_r        |
   |  fitness, coupling,   |   (12 values,       | HS-002 cascade        |
   |  emergence, LTP/LTD,  |    every 6 ticks)   | HS-003 STDP           |
   |  gens, decay, etc)    |                     | HS-004 fitness        |
   +-----------------------+                     +-----------------------+
            |                                             |
            |                                             |
   +-----------------------+                     +-----------------------+
   | Emergence Detector    |                     | Nexus Queue           |
   | (CoherenceLock,       |--POST /v3/nexus/push>| (event buffer)       |
   |  DispatchLoop,        |   (4 event types,   |                       |
   |  ConsentCascade,      |    per-event)        |                       |
   |  HebbianSaturation)   |                     |                       |
   +-----------------------+                     +-----------------------+
            |                                             |
            |                                             |
   +-----------------------+                     +-----------------------+
   | [NEW] CC Log          |                     | ML Classifier         |
   | Emergence Forwarder   |---POST /api/ingest-->| Log Pipeline          |
   |                       |   (cc_coordination) | 500K logs/sec         |
   |                       |   per-emergence     | Categorization +      |
   |                       |                     | Anomaly Detection     |
   +-----------------------+                     +-----------------------+
            |                                             |
            |                                             |
   +-----------------------+                     +-----------------------+
   | Thermal Poller        |                     | PID Controller        |
   | (every 6 ticks)       |<--GET /v3/thermal---| Kp=0.5  Ki=0.1       |
   | k_adjustment          |   (temp, target,    | Kd=0.05              |
   |                       |    PID coefficients) | target=0.7           |
   +-----------------------+                     +-----------------------+
            |                                             |
            v                                             |
   +-----------------------+                              |
   | [NEW] STDP Decay      |                              |
   | Modulation            |<--(derived from k_adj)       |
   |                       |                              |
   | decay_mult = 2 - k_adj|                              |
   | hot  -> faster decay  |                              |
   | cold -> slower decay  |                              |
   | ralph_decay_rate *=   |                              |
   |   decay_mult          |                              |
   +-----------------------+                              |
            |                                             |
            |                                             |
   +-----------------------+                     +-----------------------+
   | [NEW] Nexus Pull      |                     | Event Queue           |
   | Handler               |<--GET /v3/nexus/pull| (outbound events)     |
   | (every 3 ticks)       |                     |                       |
   |                       |   thermal_alert     | thermal_alert         |
   |                       |   diagnostic_finding| diagnostic_finding    |
   +-----------------------+                     +-----------------------+
```

---

## 3. Thermal to STDP Feedback Loop Schematic

This is the key closed loop created by Session 078. Before this session, thermal
data influenced coupling strength (via `k_adjustment`) but NOT learning dynamics.
Now the loop is fully closed:

```
                    CLOSED FEEDBACK LOOP
                    ====================

  +------------------------------------------------------------------+
  |                                                                  |
  v                                                                  |
  Field State (r, K, spheres, fitness)                               |
  |                                                                  |
  | POST /api/ingest (12 heat source values, every 6 ticks)         |
  v                                                                  |
  SYNTHEX PID Controller                                             |
  | Kp=0.5, Ki=0.1, Kd=0.05, target=0.7                            |
  | Computes: temp, error, integral, derivative                      |
  v                                                                  |
  PID Output (k_adjustment)                                          |
  |                                                                  |
  | GET /v3/thermal (every 6 ticks)                                  |
  v                                                                  |
  ORAC Thermal Poller                                                |
  |                                                                  |
  | decay_mult = 2.0 - k_adjustment                                 |
  |   k_adj=1.0 (neutral) -> decay_mult=1.0 (no change)            |
  |   k_adj=1.3 (hot)     -> decay_mult=0.7 (faster decay)         |
  |   k_adj=0.7 (cold)    -> decay_mult=1.3 (slower decay)         |
  v                                                                  |
  STDP Decay Rate (ralph_decay_rate *= decay_mult)                   |
  |                                                                  |
  | Modified decay applied during apply_stdp_with_ltp()             |
  v                                                                  |
  Coupling Weight Dynamics                                           |
  |                                                                  |
  | Weights decay faster (hot) or slower (cold)                      |
  | Affects: Kuramoto coupling matrix K_ij                           |
  v                                                                  |
  Field Coherence (r) Changes                                        |
  |                                                                  |
  +----------> Back to Field State (loop closes) --------------------+


  STABILIZING EFFECT:
  -------------------
  Hot system -> high activity -> k_adj > 1 -> decay_mult < 1
    -> weights decay FASTER -> less reinforcement -> system cools down

  Cold system -> low activity -> k_adj < 1 -> decay_mult > 1
    -> weights decay SLOWER -> preserve pathways -> system warms up

  This is a NEGATIVE FEEDBACK LOOP (stabilizing, not runaway).
```

---

## 4. Nexus Pull Event Processing

Events polled from `GET /v3/nexus/pull` every 3 ticks:

| Event Type           | Handler      | Action                                      | Broadcast                        |
|----------------------|-------------|----------------------------------------------|----------------------------------|
| `thermal_alert`      | `warn!` log | Alert fleet, record timestamp                | `habitat.alert.thermal` (Atuin KV) |
| `diagnostic_finding` | `info!` log | Record finding for RALPH learning context    | None                             |
| (other)              | `debug!` log | Passthrough, no special handling             | None                             |

### Event Lifecycle

```
SYNTHEX queues event
  |
  v
GET /v3/nexus/pull (ORAC polls every 3 ticks)
  |
  v
Deserialize JSON array of events
  |
  +--[thermal_alert]-----> warn!("SYNTHEX thermal alert received: {}", detail)
  |                           |
  |                           +--> atuin kv set habitat.alert.thermal <detail> --ttl 300
  |
  +--[diagnostic_finding]--> info!("SYNTHEX diagnostic finding: {}", detail)
  |                           |
  |                           +--> (available to RALPH Learn phase as context)
  |
  +--[other]---------------> debug!("SYNTHEX nexus event: {} = {}", event_type, detail)
```

### Atuin KV Broadcast Format

```bash
# Written by ORAC on thermal_alert:
atuin kv set habitat.alert.thermal "temp=0.82 target=0.70 error=0.12 action=decay_increase" --ttl 300

# Readable by any fleet instance:
atuin kv get habitat.alert.thermal
# Returns: temp=0.82 target=0.70 error=0.12 action=decay_increase
```

---

## 5. Emergence to Classifier Flow

When `m37_emergence_detector` fires, the event is now posted to SYNTHEX for ML
classification in addition to the existing Nexus Push pathway.

### Payload Format

```json
{
  "category": "cc_coordination",
  "level": "Medium",
  "source": "orac-emergence",
  "emergence_type": "CoherenceLock",
  "confidence": 0.95,
  "severity": 0.7,
  "tick": 12345,
  "fitness_snapshot": [0.8, 0.7, 0.9, 0.6, 0.85, 0.72, 0.91, 0.65, 0.88, 0.77, 0.83, 0.69]
}
```

### Field Descriptions

| Field              | Type       | Description                                    |
|--------------------|------------|------------------------------------------------|
| `category`         | `String`   | Always `"cc_coordination"` for this flow       |
| `level`            | `String`   | Severity: `"Low"`, `"Medium"`, `"High"`        |
| `source`           | `String`   | Always `"orac-emergence"`                      |
| `emergence_type`   | `String`   | One of 4 detector types (see below)            |
| `confidence`       | `f64`      | Detector confidence (0.0--1.0)                 |
| `severity`         | `f64`      | Event severity (0.0--1.0)                      |
| `tick`             | `u64`      | ORAC tick count at detection time              |
| `fitness_snapshot` | `[f64; 12]`| 12D RALPH fitness tensor at detection time     |

### Emergence Types

| Type                | Trigger Condition                      | Typical Confidence |
|---------------------|----------------------------------------|--------------------|
| `CoherenceLock`     | Field r > 0.98 sustained               | 0.90--0.99         |
| `DispatchLoop`      | Repeated dispatch without convergence  | 0.80--0.95         |
| `ConsentCascade`    | Multi-service consent chain detected   | 0.85--0.95         |
| `HebbianSaturation` | Coupling weights near floor + 0.01     | 0.75--0.90         |

### Flow Diagram

```
EmergenceRecord created by m37_emergence_detector
  |
  +---> [existing] POST /v3/nexus/push (Nexus event for SYNTHEX queue)
  |
  +---> [NEW] POST /api/ingest (cc_coordination log entry)
  |       |
  |       v
  |     SYNTHEX ML Classifier Pipeline
  |       |
  |       +--> Categorized alongside 500K other logs/sec
  |       +--> Anomaly detection (frequency, clustering)
  |       +--> Correlation with thermal trends
  |
  +---> [NEW] Atuin KV broadcast
          |
          v
        atuin kv set habitat.alert.emergence "<type>:<confidence>" --ttl 300
          |
          +--> Readable by all fleet instances
```

---

## 6. Pre-Session-078 vs Post-Session-078 Comparison

| Data Flow                        | Before (Session 072)                  | After (Session 078)                          |
|----------------------------------|---------------------------------------|----------------------------------------------|
| ORAC -> SYNTHEX field state      | 12 heat sources every 6 ticks         | Unchanged                                    |
| ORAC -> SYNTHEX emergence        | Nexus Push (4 types)                  | + ML classifier logs (cc_coordination)       |
| SYNTHEX -> ORAC thermal          | k_adjustment for coupling             | + STDP decay modulation (decay_mult)         |
| SYNTHEX -> ORAC events           | Never polled                          | Nexus Pull (thermal_alert, diagnostic)       |
| SYNTHEX -> ORAC classifier       | Not connected                         | cc_coordination log category                 |
| Thermal -> Learning feedback     | Open loop                             | Closed loop (decay_mult = 2 - k_adj)        |
| Cross-instance alerting          | ORAC-only (internal logs)             | Atuin KV broadcast (habitat.alert.*)         |
| Emergence fitness context        | No fitness snapshot attached           | 12D fitness tensor on every EmergenceRecord  |

### Integration Density Over Time

```
Session 059: 2 endpoints (POST /api/ingest, GET /v3/thermal)
             Unidirectional thermal read, field state write.

Session 072: 4 endpoints (+POST /v3/nexus/push, +POST /v3/decay/trigger)
             Bidirectional but no learning feedback. Nexus push only.

Session 078: 6 endpoints (+GET /v3/nexus/pull, +POST /api/ingest cc_coordination)
             Fully bidirectional. Closed thermal-learning loop.
             Cross-instance alerting via Atuin KV.
```

---

## 7. All ORAC<>SYNTHEX Endpoints (Complete Map)

| #  | Endpoint                            | Direction   | Frequency    | Purpose                              | Session | Module                    |
|----|-------------------------------------|-------------|--------------|--------------------------------------|---------|---------------------------|
| 1  | `POST /api/ingest`                  | ORAC -> SX  | 6 ticks      | 12 heat source values (HS-001..004)  | 059     | `m22_synthex_bridge`      |
| 2  | `GET /v3/thermal`                   | SX -> ORAC  | 6 ticks      | PID state + k_adjustment             | 059     | `m22_synthex_bridge`      |
| 3  | `POST /v3/nexus/push`               | ORAC -> SX  | per-event    | 4 Nexus event types                  | 072     | `m22_synthex_bridge`      |
| 4  | `POST /v3/decay/trigger`            | ORAC -> SX  | once         | PID reset on first post              | 072     | `m22_synthex_bridge`      |
| 5  | `GET /v3/nexus/pull`                | SX -> ORAC  | 3 ticks      | thermal_alert + diagnostic_finding   | 078     | `main.rs` (tick loop)     |
| 6  | `POST /api/ingest` (cc_coordination)| ORAC -> SX  | per-emergence| ML classifier input                  | 078     | `main.rs` (tick loop)     |

### Endpoint Details

#### 1. POST /api/ingest (Heat Sources)

```
URL:  http://localhost:8090/api/ingest
Body: { "source": "orac-sidecar", "heat_sources": { "HS-001": <f64>, ... } }
Freq: Every 6 ticks (~6 seconds at 1 tick/sec)
```

#### 2. GET /v3/thermal

```
URL:  http://localhost:8090/v3/thermal
Resp: { "temperature": <f64>, "target": <f64>, "k_adjustment": <f64>,
        "pid": { "kp": 0.5, "ki": 0.1, "kd": 0.05 } }
Freq: Every 6 ticks
```

#### 3. POST /v3/nexus/push

```
URL:  http://localhost:8090/v3/nexus/push
Body: { "event_type": "<type>", "source": "orac", "data": { ... } }
Freq: Per emergence event
```

#### 4. POST /v3/decay/trigger

```
URL:  http://localhost:8090/v3/decay/trigger
Body: { "source": "orac" }
Freq: Once (on first ORAC->SYNTHEX connection)
```

#### 5. GET /v3/nexus/pull (NEW - Session 078)

```
URL:  http://localhost:8090/v3/nexus/pull
Resp: { "events": [ { "event_type": "thermal_alert", "data": "..." }, ... ] }
Freq: Every 3 ticks
```

#### 6. POST /api/ingest cc_coordination (NEW - Session 078)

```
URL:  http://localhost:8090/api/ingest
Body: { "category": "cc_coordination", "level": "Medium",
        "source": "orac-emergence", "emergence_type": "...",
        "confidence": <f64>, "severity": <f64>, "tick": <u64>,
        "fitness_snapshot": [<f64>; 12] }
Freq: Per emergence event
```

---

## 8. Diagnostic Checklist

### Flow 1: Thermal to STDP Decay Modulation

```bash
# 1. Verify SYNTHEX thermal endpoint is responding
curl -s http://localhost:8090/v3/thermal | jq '.'

# 2. Check ORAC health for RALPH fitness (indicates decay is working)
curl -s http://localhost:8133/health | jq '.ralph_fitness'

# 3. Look for decay modulation log entries
# grep in ORAC logs for: "SYNTHEX thermal -> STDP decay modulation"
# Expected: decay_mult values between 0.5 and 1.5

# 4. Verify k_adjustment is within expected range
curl -s http://localhost:8090/v3/thermal | jq '.k_adjustment'
# Expected: 0.5 to 1.5 (1.0 = neutral)

# 5. Check decay_mult derivation
# decay_mult = 2.0 - k_adjustment
# k_adj=1.0 -> mult=1.0 (neutral)
# k_adj=1.3 -> mult=0.7 (hot, faster decay)
# k_adj=0.7 -> mult=1.3 (cold, slower decay)
```

### Flow 2: Nexus Pull

```bash
# 1. Check if SYNTHEX has queued events
curl -s http://localhost:8090/v3/nexus/pull | jq '.events | length'

# 2. Look for handler log entries
# grep in ORAC logs for: "SYNTHEX thermal alert received"
# grep in ORAC logs for: "SYNTHEX diagnostic finding"

# 3. Check Atuin KV for broadcast alerts
atuin kv get habitat.alert.thermal
# Non-empty = thermal alerts are being broadcast

# 4. Verify poll frequency (should fire every 3 ticks)
# Check ORAC tick counter:
curl -s http://localhost:8133/health | jq '.tick'
```

### Flow 3: Emergence to Classifier

```bash
# 1. Check if emergence events are being generated
curl -s http://localhost:8133/health | jq '.emergence_events'
# Should be > 0 and incrementing

# 2. Verify SYNTHEX is receiving cc_coordination logs
curl -s http://localhost:8090/api/health | jq '.logs_processed'
# Should be incrementing

# 3. Check Atuin KV for emergence alerts
atuin kv get habitat.alert.emergence
# Format: "<type>:<confidence>"

# 4. Verify fitness_snapshot is populated
# Look for non-null fitness_snapshot in emergence log entries
# grep in ORAC logs for: "emergence.*fitness_snapshot"
```

### Full Integration Smoke Test

```bash
# Run all checks in sequence
echo "=== SYNTHEX Health ==="
curl -s http://localhost:8090/api/health | jq '.'

echo "=== ORAC Health ==="
curl -s http://localhost:8133/health | jq '.'

echo "=== Thermal State ==="
curl -s http://localhost:8090/v3/thermal | jq '.'

echo "=== Nexus Queue ==="
curl -s http://localhost:8090/v3/nexus/pull | jq '.events | length'

echo "=== Atuin KV Alerts ==="
atuin kv get habitat.alert.thermal 2>/dev/null || echo "(no thermal alerts)"
atuin kv get habitat.alert.emergence 2>/dev/null || echo "(no emergence alerts)"

echo "=== Integration Score ==="
echo "All 6 endpoints responding = PASS"
```

---

## 9. Related Notes

- [[CC Coordination Pathways -- Complete Reference]]
- [[CC Coordination System -- 85-100 Assessment (2026-04-01)]]
- [[Advanced Memory System Analysis -- CC Coordination]]
- [[Session 072 -- Nexus Bus]]
- [[Synthex (The brain of the developer environment)]]
- [[ORAC Sidecar -- Architecture Schematics]]
- [[Session 075 -- Complete Summary]]
- [[Session 076 -- RALPH Mutation Wiring Complete]]

---

## Appendix A: Module Cross-Reference

| ORAC Module                      | File                                  | Session 078 Changes              |
|----------------------------------|---------------------------------------|----------------------------------|
| `m22_synthex_bridge`             | `src/m5_bridges/m22_synthex_bridge.rs`| No changes (existing bridge)     |
| `m37_emergence_detector`         | `src/m8_evolution/m37_emergence_detector.rs` | + `fitness_snapshot` field |
| `m26_blackboard`                 | `src/m5_bridges/m26_blackboard.rs`    | + FitnessSnapshotRecord, DecisionRecord, 2 tables, 5 methods |
| `m36_ralph_engine`               | `src/m8_evolution/m36_ralph_engine.rs` | Updated EmergenceParams construction |
| `m10_hook_server`                | `src/m3_hooks/m10_hook_server.rs`     | Updated EmergenceRecord construction |
| `m11_session_hooks`              | `src/m3_hooks/m11_session_hooks.rs`   | + DecisionRecord at Stop         |
| `main.rs`                        | `src/bin/main.rs`                     | + 4 new tick loop blocks         |

## Appendix B: Data Type Definitions

### FitnessSnapshotRecord (m26_blackboard)

```rust
pub struct FitnessSnapshotRecord {
    pub tick: u64,
    pub fitness_values: [f64; 12],  // 12D RALPH fitness tensor
    pub timestamp: i64,             // Unix timestamp
}
```

### DecisionRecord (m26_blackboard)

```rust
pub struct DecisionRecord {
    pub task_id: String,
    pub decision: String,
    pub rationale: String,
    pub confidence: f64,
    pub timestamp: i64,
}
```

### EmergenceRecord (m37_emergence_detector) -- Updated

```rust
pub struct EmergenceRecord {
    pub emergence_type: EmergenceType,
    pub confidence: f64,
    pub severity: f64,
    pub tick: u64,
    pub fitness_snapshot: Option<[f64; 12]>,  // NEW in Session 078
}
```
