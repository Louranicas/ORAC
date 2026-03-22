# ORAC Bridge Specifications

> Upstream service integrations. Each bridge is a typed HTTP client with health checks, retry, and consent gating.

## Bridge Architecture

```
ORAC (:8133)
  |-- SYNTHEX bridge  --> :8090  (thermal field, Hebbian writeback)
  |-- ME bridge       --> :8080  (fitness signals, observer)
  |-- POVM bridge     --> :8125  (memory hydration, pathways)
  |-- RM bridge       --> :8130  (reasoning persistence, TSV)
```

All bridges share:
- `reqwest::Client` with 2s connect timeout, 5s request timeout
- Circuit breaker (see `patterns/CIRCUIT_BREAKER.md`)
- Consent check before every write operation
- Exponential backoff on failure: 100ms, 200ms, 400ms, max 3 retries

---

## 1. SYNTHEX Bridge (:8090)

Thermal field state and Hebbian weight synchronization.

### Endpoints

#### GET /api/health
```
Response: {"status": "ok", "version": "3.1.0"}
Poll interval: 10s
```

#### GET /v3/thermal
Read current thermal field state.
```
Response:
{
  "temperature": 0.72,
  "cascade_amplification": 1.15,
  "breaker_state": "closed",
  "layers": [{"id": 0, "temp": 0.68}, ...]
}
Poll interval: 5s
```

#### POST /v3/hebbian
Write back Hebbian weights from ORAC's STDP tracker.
```
Request:
{
  "weights": [
    {"pre": "Read", "post": "Edit", "w": 0.34},
    {"pre": "Bash", "post": "Grep", "w": 0.12}
  ],
  "source": "orac-sidecar"
}
Response: {"accepted": 4}
Poll interval: on-demand (post-session flush)
Consent: required (sphere must opt-in to Hebbian sync)
```

---

## 2. ME Bridge (:8080)

Maintenance Engine fitness signals and observer data.

### Endpoints

#### GET /api/health
```
Response: {"status": "ok", "uptime_s": 86400}
Poll interval: 10s
```

#### GET /api/observer
Read observer fitness signals for evolution chamber.
```
Response:
{
  "fitness_signals": [
    {"metric": "latency_p99_ms", "value": 0.8, "weight": 0.2},
    {"metric": "error_rate", "value": 0.001, "weight": 0.3},
    {"metric": "throughput_rps", "value": 150, "weight": 0.15}
  ],
  "synergy_score": 0.891,
  "timestamp_ms": 1742600000000
}
Poll interval: 30s
Consent: not required (read-only, anonymized)
```

---

## 3. POVM Bridge (:8125)

Memory hydration and pathway discovery.

### Endpoints

#### GET /health
```
Response: {"status": "healthy"}
Poll interval: 10s
```

#### GET /memories?sphere_id={id}&limit={n}
Retrieve memories for a sphere.
```
Response:
{
  "memories": [
    {"id": "uuid", "content": "...", "activation": 0.72, "created_ms": 1742600000000}
  ],
  "total": 42
}
Poll interval: on-demand (session start)
Consent: required (sphere owns its memories)
```

#### GET /pathways?sphere_id={id}
Retrieve discovered tool pathways.
```
Response:
{
  "pathways": [
    {"sequence": ["Read", "Edit", "Bash"], "frequency": 14, "avg_duration_ms": 320}
  ]
}
Poll interval: on-demand
Consent: required
```

#### POST /hydrate
Hydrate a sphere with historical context on session start.
```
Request:
{
  "sphere_id": "uuid",
  "cwd": "/path/to/project",
  "max_memories": 50
}
Response:
{
  "injected": 23,
  "pathways": 5,
  "context_tokens": 1200
}
Poll interval: once per session start
Consent: required (sphere must opt-in to hydration)
```

---

## 4. RM Bridge (:8130)

Reasoning memory persistence. **TSV format, not JSON.**

### Endpoints

#### GET /health
```
Response: {"status": "ok"}
Poll interval: 10s
```

#### POST /put
Persist a reasoning entry. **Body is TSV, not JSON.**
```
Content-Type: text/tab-separated-values

Request body (TSV, tab-delimited):
<timestamp>\t<source>\t<category>\t<content>

Example:
1742600000\torac-sidecar\ttool_chain\tRead→Edit→Bash (3 uses, r_delta=0.03)

Response: {"id": "uuid", "stored": true}
Poll interval: on-demand (PostToolUse, session flush)
Consent: not required (system-level telemetry)
```

#### GET /search?q={query}
Search reasoning memory.
```
Response:
{
  "results": [
    {"id": "uuid", "timestamp": 1742600000, "source": "orac-sidecar", "content": "..."}
  ]
}
Poll interval: on-demand
Consent: not required (read-only)
```

#### GET /entries
List recent entries.
```
Query params: ?limit=50&source=orac-sidecar
Response:
{
  "entries": [
    {"id": "uuid", "timestamp": 1742600000, "source": "orac-sidecar", "category": "tool_chain", "content": "..."}
  ]
}
Poll interval: on-demand
```

---

## Consent Model

Write operations to external services require sphere consent:

1. ORAC checks local consent registry (`/consent/{sphere_id}`)
2. If no local entry: sends `ClientFrame::ConsentQuery` to PV2 bus
3. PV2 responds with `ServerFrame::ConsentResponse` (timeout 500ms)
4. Default on timeout: **deny**
5. Consent is cached per-sphere per-bridge for session lifetime

Consent flags per bridge:

| Bridge | Read | Write | Default |
|--------|------|-------|---------|
| SYNTHEX | always | opt-in | deny |
| ME | always | N/A | allow |
| POVM | opt-in | opt-in | deny |
| RM | always | always | allow |
