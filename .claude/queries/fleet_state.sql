-- ORAC Fleet state queries (adapted from PV2 field_state.sql)

-- Current sphere registrations (via PV2 HTTP API cache)
SELECT sphere_id, status, persona, phase, frequency, last_heartbeat
FROM sphere_cache ORDER BY last_heartbeat DESC;

-- Field snapshots (from IPC bus subscription)
SELECT tick, ROUND(r, 3) as r, sphere_count, ROUND(k_mod, 3) as k_mod,
       decision_action, timestamp
FROM field_snapshots ORDER BY tick DESC LIMIT ?;

-- R trend over last hour (720 ticks at 5s)
SELECT MIN(r) as r_min, MAX(r) as r_max, AVG(r) as r_avg, COUNT(*) as samples
FROM field_snapshots WHERE tick > (SELECT MAX(tick) - 720 FROM field_snapshots);

-- Chimera events
SELECT tick, sphere_count, decision_action, timestamp
FROM field_snapshots WHERE chimera_detected = 1 ORDER BY tick DESC LIMIT 20;

-- Bridge health status
SELECT bridge_name, port, last_status, last_check, consecutive_failures
FROM bridge_health ORDER BY last_check DESC;

-- Circuit breaker state
SELECT sphere_id, state, failure_count, last_failure, next_retry_at
FROM circuit_breaker_state ORDER BY last_failure DESC;

-- Hebbian co-activation log
SELECT tool_a, tool_b, weight, co_activations, last_updated
FROM hebbian_weights ORDER BY weight DESC LIMIT 30;

-- Task routing decisions
SELECT task_id, source_sphere, target_sphere, routing_method, score, created_at
FROM routing_decisions ORDER BY created_at DESC LIMIT 20;
