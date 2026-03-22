-- ORAC Blackboard queries (SQLite shared fleet state)

-- Recent fleet knowledge entries
SELECT key, value, source_sphere, updated_at
FROM blackboard ORDER BY updated_at DESC LIMIT 20;

-- Entries by source sphere
SELECT key, value, source_sphere, updated_at
FROM blackboard WHERE source_sphere = ? ORDER BY updated_at DESC;

-- Stale entries (older than N seconds)
SELECT key, source_sphere, updated_at,
       ROUND((julianday('now') - julianday(updated_at)) * 86400) as age_secs
FROM blackboard
WHERE (julianday('now') - julianday(updated_at)) * 86400 > ?
ORDER BY updated_at ASC;

-- Entry count by source
SELECT source_sphere, COUNT(*) as entries, MAX(updated_at) as last_update
FROM blackboard GROUP BY source_sphere ORDER BY entries DESC;

-- Search blackboard by key pattern
SELECT key, value, source_sphere, updated_at
FROM blackboard WHERE key LIKE ? ORDER BY updated_at DESC LIMIT 20;
