CREATE TABLE IF NOT EXISTS projects (
    id              TEXT PRIMARY KEY,
    account_id      TEXT NOT NULL,
    client_user_id  TEXT,
    name            TEXT NOT NULL,
    description     TEXT,
    status          TEXT NOT NULL DEFAULT 'planning',
    start_date      TEXT,
    target_end_date TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_projects_client_user_id ON projects (client_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_account_id ON projects (account_id);
