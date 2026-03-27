CREATE TABLE IF NOT EXISTS api_request_logs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at TEXT NOT NULL DEFAULT (datetime('now')),
    subject     TEXT,
    method      TEXT NOT NULL,
    path        TEXT NOT NULL,
    status_code INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    user_agent  TEXT
);

CREATE INDEX IF NOT EXISTS idx_api_request_logs_occurred_at ON api_request_logs (occurred_at DESC);
