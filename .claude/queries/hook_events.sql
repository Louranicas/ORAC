-- ORAC Hook event tracking queries

-- Recent hook events
SELECT id, event_type, session_id, tool_name, decision, latency_us, created_at
FROM hook_events ORDER BY created_at DESC LIMIT 30;

-- Hook event distribution
SELECT event_type, COUNT(*) as count, AVG(latency_us) as avg_latency_us
FROM hook_events GROUP BY event_type ORDER BY count DESC;

-- Permission decisions (PermissionRequest hooks only)
SELECT session_id, tool_name, decision, reason, created_at
FROM hook_events WHERE event_type = 'PermissionRequest'
ORDER BY created_at DESC LIMIT 20;

-- Denied actions (security audit trail)
SELECT session_id, event_type, tool_name, reason, created_at
FROM hook_events WHERE decision = 'deny'
ORDER BY created_at DESC LIMIT 50;

-- Per-session hook summary
SELECT session_id, COUNT(*) as total_hooks,
       SUM(CASE WHEN decision = 'approve' THEN 1 ELSE 0 END) as approved,
       SUM(CASE WHEN decision = 'deny' THEN 1 ELSE 0 END) as denied,
       AVG(latency_us) as avg_latency_us
FROM hook_events GROUP BY session_id ORDER BY total_hooks DESC LIMIT 15;

-- Slow hooks (latency > 1ms = 1000us threshold)
SELECT id, event_type, session_id, tool_name, latency_us, created_at
FROM hook_events WHERE latency_us > 1000
ORDER BY latency_us DESC LIMIT 20;

-- Thermal gate blocks (PreToolUse denials)
SELECT session_id, tool_name, reason, created_at
FROM hook_events WHERE event_type = 'PreToolUse' AND decision = 'deny'
ORDER BY created_at DESC LIMIT 20;
