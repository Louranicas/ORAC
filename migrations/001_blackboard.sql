-- ORAC Sidecar Blackboard Schema v1
-- SQLite persistent state for fleet coordination

CREATE TABLE IF NOT EXISTS pane_status (
    pane_id     TEXT PRIMARY KEY,
    status      TEXT NOT NULL DEFAULT 'idle',
    last_seen   INTEGER NOT NULL,
    phase       REAL NOT NULL DEFAULT 0.0,
    tool_name   TEXT
);

CREATE INDEX idx_pane_status_status ON pane_status(status);
CREATE INDEX idx_pane_status_last_seen ON pane_status(last_seen);

CREATE TABLE IF NOT EXISTS task_history (
    id              TEXT PRIMARY KEY,
    pane_id         TEXT NOT NULL,
    description     TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'pending',
    created_at      INTEGER NOT NULL,
    completed_at    INTEGER,
    FOREIGN KEY (pane_id) REFERENCES pane_status(pane_id)
);

CREATE INDEX idx_task_history_pane_id ON task_history(pane_id);
CREATE INDEX idx_task_history_status ON task_history(status);
CREATE INDEX idx_task_history_created_at ON task_history(created_at);

CREATE TABLE IF NOT EXISTS agent_cards (
    pane_id         TEXT PRIMARY KEY,
    capabilities    TEXT NOT NULL DEFAULT '[]',
    domain          TEXT NOT NULL DEFAULT 'general',
    token_budget    INTEGER NOT NULL DEFAULT 200000,
    FOREIGN KEY (pane_id) REFERENCES pane_status(pane_id)
);

CREATE INDEX idx_agent_cards_domain ON agent_cards(domain);

CREATE TABLE IF NOT EXISTS coupling_snapshot (
    source      TEXT NOT NULL,
    target      TEXT NOT NULL,
    weight      REAL NOT NULL DEFAULT 0.5,
    updated_at  INTEGER NOT NULL,
    PRIMARY KEY (source, target)
);

CREATE INDEX idx_coupling_snapshot_updated_at ON coupling_snapshot(updated_at);

CREATE TABLE IF NOT EXISTS fleet_metrics (
    timestamp           INTEGER NOT NULL,
    order_param         REAL NOT NULL,
    k_effective         REAL NOT NULL,
    active_panes        INTEGER NOT NULL,
    chimera_detected    INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_fleet_metrics_timestamp ON fleet_metrics(timestamp);
