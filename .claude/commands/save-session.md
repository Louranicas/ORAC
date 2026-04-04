# /save-session — Persist Session State Across All Memory Substrates

Save current session findings to ALL 7 memory substrates in parallel.
Run before /clear, /compact, or ending a session.

topology: broadcast

When the user runs /save-session, ask them for:
1. Session number (e.g. "067")
2. One-line summary of what was accomplished
3. Key findings (bullet points)
4. Next session priorities

Then persist across ALL substrates:

## Substrates to Write

### 1. POVM (crystallised memory)
```bash
curl -sf -X POST localhost:8125/memories -H 'Content-Type: application/json' -d '{
  "content": "SESSION_SUMMARY_HERE. Obsidian: [[Session NNN — TITLE]]",
  "theta": 0.0, "phi": 0.0,
  "tensor": [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
  "session_id": "session-NNN"
}'
```

### 2. Reasoning Memory (TSV)
```bash
printf 'session-NNN-final\torchestrator\t0.99\t0\tSUMMARY_HERE' | curl -sf -X POST localhost:8130/put --data-binary @-
```

### 3. Obsidian (shared-context vault)
Write `~/projects/shared-context/Session NNN — TITLE.md` with:
- Date, duration, scope
- What was done (numbered sections)
- Key metrics (before → after)
- Bootstrap protocol for next session
- Next session priorities
- Related notes with [[wikilinks]]

### 4. Tracking Databases
```bash
DEM=~/claude-code-workspace/developer_environment_manager
sqlite3 "$DEM/service_tracking.db" "INSERT INTO optimization_events (timestamp, system, optimization_type, description, impact, token_savings) VALUES (datetime('now'), 'SESSION_SYSTEM', 'session-NNN', 'SUMMARY', IMPACT_SCORE, 0);"
sqlite3 "$DEM/service_tracking.db" "INSERT INTO learned_patterns (timestamp, pattern_type, pattern_name, description, strength, reinforcement_count) VALUES (datetime('now'), 'session-learning', 'session-NNN-KEY_LEARNING', 'DESCRIPTION', 0.95, 1);"
```

### 5. MCP Knowledge Graph
Use `mcp__memory__create_entities` to create a session entity with observations.

### 6. Auto-Memory
Write/update `~/.claude/projects/-home-louranicas-claude-code-workspace/memory/session-NNN.md` with frontmatter:
```yaml
---
name: Session NNN
description: One-line summary
type: project
---
```
Update `MEMORY.md` index with new entry at top.

### 7. ULTRAPLATE Master Index
Update `~/projects/shared-context/ULTRAPLATE Master Index.md` with session entry.

## Verification

After writing, confirm each substrate:
```bash
echo "POVM: $(curl -s localhost:8125/memories?limit=1 | python3 -c 'import sys,json;print(json.load(sys.stdin)[0].get(\"id\",\"?\")[:16])' 2>/dev/null)"
echo "RM: $(curl -s localhost:8130/health | python3 -c 'import sys,json;print(json.load(sys.stdin).get(\"active_entries\",0))' 2>/dev/null) entries"
echo "Obsidian: $(ls ~/projects/shared-context/Session*.md | wc -l) session notes"
echo "DB: $(sqlite3 ~/claude-code-workspace/developer_environment_manager/service_tracking.db 'SELECT COUNT(*) FROM optimization_events;' 2>/dev/null) optimization events"
```

## Also Run /tensor for Final State Capture

Include the 6D tensor output in the Obsidian note as the session's closing snapshot.
