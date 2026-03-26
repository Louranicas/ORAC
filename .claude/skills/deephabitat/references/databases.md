# Cross-Database Architecture (ORAC Sidecar Edition)

## 6 Database Paradigms

| Paradigm | Example | Pattern |
|----------|---------|---------|
| WAL SQLite | PV field_tracking.db | High-write snapshots |
| Tracking DB | service_tracking.db | Append-only events |
| Tensor Memory | tensor_memory.db | 11D tensor encoding |
| Hebbian Pulse | hebbian_pulse.db | Pathway strength + LTP/LTD |
| Synergy Scoring | system_synergy.db | Cross-service scores |
| TSV Flat File | Reasoning Memory | Category\tAgent\tConf\tTTL\tContent |

## ORAC-Owned Databases

### Blackboard (SQLite, in-process)

Schema: `migrations/001_blackboard.sql` | Module: `m26_blackboard` (919 LOC, rusqlite)

**5 Tables:**

```sql
-- Fleet pane tracking (upsert on every PostToolUse)
pane_status (pane_id PK, status TEXT, last_seen INT, phase REAL, tool_name TEXT)
  Indexes: status, last_seen

-- Task lifecycle (insert on submit, update on claim/complete/fail)
task_history (id PK, pane_id FK, description TEXT, status TEXT, created_at INT, completed_at INT)
  Indexes: pane_id, status, created_at

-- A2A-inspired capability declarations
agent_cards (pane_id PK FK, capabilities JSON, domain TEXT, token_budget INT)
  Indexes: domain

-- Hebbian coupling weight snapshots (for evolution)
coupling_snapshot (source+target PK, weight REAL, updated_at INT)
  Indexes: updated_at

-- Field state time series (for dashboard + evolution)
fleet_metrics (timestamp INT, order_param REAL, k_effective REAL, active_panes INT, chimera_detected INT)
  Indexes: timestamp
```

**Operations:**
- `pane_status`: upsert, get, list_by_status, remove, count
- `task_history`: insert, recent(n), count_by_status, by_pane
- `agent_cards`: register, get, query_by_domain, update_capabilities
- `coupling_snapshot`: upsert, get_matrix, prune_old
- `fleet_metrics`: insert, range_query, latest

**Access:** In-memory for tests, file-backed for prod. No external SQLite file — ORAC owns the connection.
**HTTP:** `GET /blackboard` endpoint for read-only fleet state queries.

## Key External Databases by Service

| Service | Database | Key Data |
|---------|----------|----------|
| PV | field_tracking.db | field_snapshots, sphere_history, coupling |
| PV | bus_tracking.db | bus_tasks, bus_events, cascade_events |
| SYNTHEX | synthex.db | Core state |
| SYNTHEX | v3_homeostasis.db | Thermal PID |
| SYNTHEX | hebbian_pulse.db | Neural pathways (0 pathways, 5 pulses — gotcha!) |
| SYNTHEX | flow_tensor_memory.db | Tensor encoding |
| DevEnv | service_tracking.db | Health history |
| DevEnv | system_synergy.db | Cross-service scores |
| DevEnv | episodic_memory.db | Session records |
| Orchestrator | code.db | Module registry |
| Orchestrator | tensor_memory.db | SAN-K7 tensors |
| Orchestrator | performance.db | Benchmarks |
| POVM | povm_data.db | 272 memories, 2,573 pathways |
| RM | TSV flat file | 3,400+ entries |

## Cross-DB Query Patterns

```bash
# Synergy scores (highest integration pairs)
sqlite3 -header -column ~/claude-code-workspace/developer_environment_manager/system_synergy.db \
  "SELECT system_1, system_2, ROUND(synergy_score,2), integration_points FROM system_synergy WHERE integration_points > 5 ORDER BY integration_points DESC;"

# SAN-K7 ↔ SYNTHEX: 59 integration points (highest)
# SYNTHEX ↔ DevOps: 10 points, 97.3% synergy
# Swarm ↔ RM: 12 points, 98.0% synergy

# Service health history
sqlite3 -header -column ~/claude-code-workspace/developer_environment_manager/service_tracking.db \
  "SELECT service_id, status, ROUND(health_score,2) FROM service_health ORDER BY health_score DESC LIMIT 10;"

# POVM memories
curl -s localhost:8125/hydrate | jq '{memory_count, pathway_count}'

# RM search
curl -s "localhost:8130/search?q=orac" | jq '.results[:5]'

# ORAC blackboard (via HTTP)
curl -s localhost:8133/blackboard | jq .

# ORAC blackboard SQL queries (from .claude/queries/)
# See .claude/queries/blackboard.sql, hook_events.sql, fleet_state.sql
```

## Database Gotchas
- 166 databases total, 360.6 MB — 20-30% are empty schemas
- hebbian_pulse.db has 0 neural_pathways, only 5 pulses
- field_tracking.db is at pane-vortex/data/ NOT ~/.local/share/
- ME EventBus has 275K events but subscriber_count=0 (cosmetic — uses polling)
- POVM is write-only — must call `/hydrate` to read back state (BUG-034)
- Always `.schema` before writing SQL — column names differ from guesses
- ORAC blackboard uses in-memory SQLite in tests — no file to inspect
