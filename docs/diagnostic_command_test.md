# Diagnostic Command Test Results

> **Tested:** 2026-03-25T11:42 UTC
> **Method:** `curl -s --max-time 3` against each endpoint, captured raw JSON responses

---

## Summary

| # | Command | Status | HTTP | Notes |
|---|---------|--------|------|-------|
| 1 | `curl -s localhost:8133/health` | **PASS** | 200 | Full ORAC health with all subsystems |
| 2 | `curl -s localhost:8133/ralph` | **PASS** | 200 | RALPH evolution state |
| 3 | `curl -s localhost:8133/emergence` | **PASS** | 200 | Emergence detector stats |
| 4 | `curl -s localhost:8133/bridges` | **PASS** | 200 | Bridge health summary |
| 5 | `curl -s localhost:8133/thermal` | **PASS** | 200 | SYNTHEX thermal cache |
| 6 | `curl -s localhost:8132/health` | **PASS** | 200 | PV2 Kuramoto field state |
| 7 | `curl -s localhost:8080/api/health` | **PASS** | 200 | Maintenance Engine health |
| 8 | `curl -s localhost:8090/api/health` | **PASS** | 200 | SYNTHEX health |

**Result: 8/8 PASS — all services responding**

---

## Detailed Responses

### (1) ORAC Health — `localhost:8133/health`

```json
{
  "status": "healthy",
  "service": "orac-sidecar",
  "version": "0.6.0",
  "port": 8133,
  "sessions": 12,
  "uptime_ticks": 8908,
  "field_r": 0.894,
  "sphere_count": 39,
  "ralph_gen": 4598,
  "ralph_phase": "Analyze",
  "ralph_fitness": 0.751,
  "ralph_converged": false,
  "ipc_state": "subscribed",
  "breakers": {
    "me":      {"state": "Closed", "successes": 742, "failures": 1},
    "povm":    {"state": "Closed", "successes": 12, "failures": 0},
    "pv2":     {"state": "Closed", "successes": 10937, "failures": 7},
    "rm":      {"state": "Closed", "successes": 154, "failures": 0},
    "synthex": {"state": "Closed", "successes": 2179, "failures": 0},
    "vms":     {"state": "Closed", "successes": 332, "failures": 0}
  },
  "thermal_temperature": 0.699,
  "thermal_target": 0.5,
  "me_fitness": 0.620,
  "me_frozen": false,
  "me_observer_subscribed": true,
  "me_total_correlations": 6000604,
  "me_total_events": 542479,
  "dispatch_total": 2809,
  "coupling_connections": 1482,
  "coupling_weight_mean": 0.179,
  "coupling_weight_range": [0.15, 0.85],
  "co_activations_total": 2994,
  "hebbian_ltp_total": 2994,
  "hebbian_ltd_total": 41976,
  "hebbian_last_tick": 8883,
  "emergence_events": 2928,
  "emergence_active_monitors": 50,
  "synthex_stale": false,
  "rm_stale": false
}
```

**Observations:**
- 6 breakers (me, povm, pv2, rm, synthex, vms) — all Closed
- RALPH at gen 4598, fitness 0.751 (up from 0.581 at Session 060 end)
- 39 spheres, field r = 0.894
- Thermal 0.699 vs target 0.5 — running warm (PID active)
- ME has 6M correlations, 542K events — metabolically active
- LTP:LTD ratio = 2994:41976 = 0.071 (still below 0.15 target)

### (2) RALPH — `localhost:8133/ralph`

```json
{
  "completed_cycles": 503,
  "fitness": 0.751,
  "generation": 4598,
  "mutations_accepted": 43,
  "mutations_proposed": 43,
  "mutations_rolled_back": 0,
  "mutations_skipped": 1661,
  "paused": false,
  "peak_fitness": 0.824,
  "phase": "Analyze",
  "system_state": "Healthy"
}
```

**Observations:**
- 43 accepted, 0 rolled back, 1661 skipped — 97.5% skip rate (diversity gate active)
- Peak fitness 0.824, current 0.751 — healthy range
- 503 completed cycles out of 4598 generations (10.9% yield)

### (3) Emergence — `localhost:8133/emergence`

```json
{
  "active_monitors": 50,
  "by_type": {
    "beneficial_sync": 496,
    "chimera_formation": 742,
    "coherence_lock": 207,
    "thermal_spike": 1483
  },
  "total_detected": 2928
}
```

**Observations:**
- 4 of 8 emergence types firing (beneficial_sync, chimera_formation, coherence_lock, thermal_spike)
- 4 types silent: CouplingRunaway, HebbianSaturation, DispatchLoop, ConsentCascade
- thermal_spike dominant (50.6% of all events) — consistent with temp 0.699 > target 0.5
- 50 active monitors (MAX_MONITORS cap hit)

### (4) Bridges — `localhost:8133/bridges`

```json
{
  "breakers_closed": 6,
  "breakers_half_open": 0,
  "breakers_open": 0,
  "ipc_state": "subscribed",
  "me_fitness": 0.620,
  "me_frozen": false,
  "synthex_last_poll": 8904
}
```

**Observations:**
- All 6 breakers Closed — no connectivity issues
- IPC subscribed — PV2 event stream active
- ME not frozen — observer polling live

### (5) Thermal — `localhost:8133/thermal`

```json
{
  "bridge_consecutive_failures": 0,
  "heat_sources": [
    {"id": "HS-001", "reading": 0.911, "weight": 0.30},
    {"id": "HS-002", "reading": 0.367, "weight": 0.35},
    {"id": "HS-003", "reading": 0.736, "weight": 0.20},
    {"id": "HS-004", "reading": 1.000, "weight": 0.15}
  ],
  "k_adjustment": 0.960,
  "last_poll_tick": 8904,
  "orac_tick": 8908,
  "pid_output": 0.199,
  "source": "orac_cache",
  "target": 0.5,
  "temperature": 0.699
}
```

**Observations:**
- 4 heat sources with weights summing to 1.0
- HS-004 at 1.0 (saturated) — primary heat contributor
- HS-002 at 0.367 (coolest) — weight 0.35 (highest weight)
- PID output 0.199, k_adjustment 0.960 — PID reducing coupling to cool
- Source is `orac_cache` — reading from ORAC's cached SYNTHEX poll, not direct

### (6) PV2 Health — `localhost:8132/health`

```json
{
  "fleet_mode": "Full",
  "k": 1.676,
  "k_modulation": 0.850,
  "r": 0.897,
  "spheres": 39,
  "status": "healthy",
  "tick": 415573,
  "warmup_remaining": 0
}
```

**Observations:**
- 39 spheres matches ORAC's sphere_count
- r = 0.897 (PV2) vs 0.894 (ORAC cache) — 0.003 drift is normal poll lag
- k_modulation 0.850 — active coupling modulation
- 415K ticks — long-running session

### (7) ME Health — `localhost:8080/api/health`

```json
{
  "db_connected": true,
  "last_fitness": 0.620,
  "overall_health": 0.587,
  "service": "maintenance-engine",
  "status": "healthy",
  "uptime_secs": 291322,
  "version": "1.0.0"
}
```

**Observations:**
- Uptime 291,322s = ~3.37 days
- Fitness 0.620 matches ORAC's me_fitness reading
- overall_health 0.587 — lower than fitness (other health dimensions contributing)

### (8) SYNTHEX Health — `localhost:8090/api/health`

```json
{
  "status": "healthy",
  "timestamp": "2026-03-25T11:42:34.678982329+00:00"
}
```

**Observations:**
- Minimal health response — just status and timestamp
- No thermal data in health endpoint (thermal data served via `/v3/thermal` or `/api/ingest`)

---

## Cross-Service Consistency Check

| Metric | ORAC (:8133) | PV2 (:8132) | ME (:8080) | Match? |
|--------|-------------|-------------|------------|--------|
| Sphere count | 39 | 39 | N/A | YES |
| Field r | 0.894 | 0.897 | N/A | ~YES (poll lag) |
| ME fitness | 0.620 | N/A | 0.620 | YES |

---

## Key Findings

1. **All 8 endpoints operational** — zero failures, all returning valid JSON
2. **RALPH has advanced significantly** — gen 4598, fitness 0.751 (up from 0.581 at Session 060 end)
3. **Thermal running warm** — 0.699 vs 0.5 target; PID active but HS-004 saturated at 1.0
4. **Emergence monitors at cap** — 50/50 (MAX_MONITORS hit), may be suppressing new monitors
5. **LTP:LTD ratio still low** — 0.071 vs 0.15 target (improvement from 0.055 at Session 060)
6. **High skip rate** — 97.5% mutations skipped by diversity gate, only 43 accepted in 4598 gens
7. **Zero rollbacks** — all 43 accepted mutations were beneficial (or neutral-accepted via BUG-039 fix)
