-- v1: initial schema for FileGrove.

PRAGMA foreign_keys = ON;

-- Key/value settings store. Values are JSON-encoded.
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings (key, value) VALUES
    ('theme',             '"dark"'),
    ('language',          '"en"'),
    ('launchAtStartup',   'false'),
    ('minimizeToTray',    'true'),
    ('showTrayIcon',      'true'),
    ('notifications',     'true'),
    ('conflictBehavior',  '"rename"'),
    ('onboardingCompleted', 'false'),
    ('firstFolderHintShown', 'false'),
    ('subfolderExplainerShown', 'false'),
    ('commandWarningShown', 'false'),
    ('rulesSortOrder',    '"created"'),
    ('foldersSortOrder',  '"created"'),
    ('subfolderDefaults.maxDepth', '5');

-- Watched folders.
CREATE TABLE IF NOT EXISTS folders (
    id               TEXT PRIMARY KEY,         -- uuid
    path             TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    mode             TEXT NOT NULL,            -- single|auto|scheduled
    rule_ids         TEXT NOT NULL DEFAULT '[]', -- JSON array of uuids
    watcher_status   TEXT NOT NULL,            -- stopped|running|paused
    created_at       TEXT NOT NULL,            -- ISO 8601
    subfolders       TEXT NOT NULL,            -- JSON
    schedule         TEXT,                     -- JSON or NULL
    exclude_folders  TEXT NOT NULL DEFAULT '[]', -- JSON array
    exclude_patterns TEXT NOT NULL DEFAULT '[]'  -- JSON array
);

CREATE INDEX IF NOT EXISTS idx_folders_mode ON folders(mode);

-- User-defined rules (and seeded quick rules).
CREATE TABLE IF NOT EXISTS rules (
    id          TEXT PRIMARY KEY,              -- uuid
    name        TEXT NOT NULL,
    is_quick_rule INTEGER NOT NULL DEFAULT 0,
    enabled     INTEGER NOT NULL DEFAULT 1,
    "order"     INTEGER NOT NULL DEFAULT 0,
    icon        TEXT NOT NULL DEFAULT 'File',
    conditions  TEXT NOT NULL,                  -- JSON array
    action      TEXT NOT NULL,                  -- JSON object
    created_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rules_order ON rules("order");

-- Activity log — every file action records one row.
CREATE TABLE IF NOT EXISTS activity (
    id          TEXT PRIMARY KEY,                -- uuid
    at          TEXT NOT NULL,                   -- ISO 8601
    type        TEXT NOT NULL,                   -- move|trash|delete|runCommand
    file_name   TEXT NOT NULL,
    file_path   TEXT NOT NULL,
    folder_id   TEXT,
    rule_id     TEXT,
    rule_name   TEXT,
    destination TEXT,
    command     TEXT,
    exit_code   INTEGER,
    output      TEXT,
    duration_ms INTEGER,
    timed_out   INTEGER,
    success     INTEGER NOT NULL,
    error       TEXT
);

CREATE INDEX IF NOT EXISTS idx_activity_at       ON activity(at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_folder   ON activity(folder_id);
CREATE INDEX IF NOT EXISTS idx_activity_rule     ON activity(rule_id);
CREATE INDEX IF NOT EXISTS idx_activity_success  ON activity(success);

-- Undo entries — one row per reversible operation.
CREATE TABLE IF NOT EXISTS undo_entries (
    session_id  TEXT NOT NULL,
    at          TEXT NOT NULL,
    from_path   TEXT NOT NULL,
    to_path     TEXT NOT NULL,
    seq         INTEGER NOT NULL,                -- monotonic per session
    PRIMARY KEY (session_id, seq)
);

CREATE INDEX IF NOT EXISTS idx_undo_session ON undo_entries(session_id, seq);
