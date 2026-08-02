-- Durable idempotency ledger for external commands.

CREATE TABLE IF NOT EXISTS command_ledger (
    rule_id       TEXT NOT NULL,
    file_path     TEXT NOT NULL,
    command       TEXT NOT NULL,
    file_size     INTEGER NOT NULL,
    modified_at   TEXT NOT NULL,
    -- 0=completed, 1=running, 2=interrupted/unreported, 3=awaiting review.
    in_progress   INTEGER NOT NULL DEFAULT 1,
    updated_at    TEXT NOT NULL,
    PRIMARY KEY (rule_id, file_path, command)
);

CREATE INDEX IF NOT EXISTS idx_command_ledger_state
    ON command_ledger(in_progress);
