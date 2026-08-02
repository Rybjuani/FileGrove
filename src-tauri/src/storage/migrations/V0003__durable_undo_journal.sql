-- Promote the original undo list into a write-ahead filesystem journal.

ALTER TABLE undo_entries ADD COLUMN operation_id TEXT;
ALTER TABLE undo_entries ADD COLUMN state TEXT NOT NULL DEFAULT 'applied';
ALTER TABLE undo_entries ADD COLUMN conflict_behavior TEXT NOT NULL DEFAULT 'rename';
ALTER TABLE undo_entries ADD COLUMN backup_path TEXT;
ALTER TABLE undo_entries ADD COLUMN had_destination INTEGER NOT NULL DEFAULT 0;

CREATE UNIQUE INDEX IF NOT EXISTS idx_undo_operation
    ON undo_entries(operation_id) WHERE operation_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_undo_state ON undo_entries(state);
