//! Durable undo journal for reversible move operations.
//!
//! A row is committed as `pending` before the filesystem is changed, becomes
//! `applied` after the move, and is changed to `undoing` before restoration.
//! Startup/read reconciliation resolves interrupted states conservatively.

use std::fs;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::domain::undo::{UndoEntry, UndoSessionStats, UndoSessionSummary};
use crate::engine::file_ops::{restore_retained_backup, ConflictBehavior};

pub struct UndoRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UndoJournalState {
    Pending,
    Applied,
    Undoing,
    Review,
}

impl UndoJournalState {
    fn parse(value: &str) -> Self {
        match value {
            "pending" => Self::Pending,
            "undoing" => Self::Undoing,
            "review" => Self::Review,
            _ => Self::Applied,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Applied => "applied",
            Self::Undoing => "undoing",
            Self::Review => "review",
        }
    }
}

/// Internal journal metadata plus the public undo operation.
#[derive(Debug, Clone)]
pub struct JournaledUndoEntry {
    pub entry: UndoEntry,
    pub operation_id: Option<Uuid>,
    pub state: UndoJournalState,
    pub backup_path: Option<PathBuf>,
    pub had_destination: bool,
    seq: i64,
}

impl Deref for JournaledUndoEntry {
    type Target = UndoEntry;

    fn deref(&self) -> &Self::Target {
        &self.entry
    }
}

impl UndoRepo {
    /// Prevent automation from touching a source involved in an unresolved
    /// recovery. The user can inspect/retry or explicitly dismiss that undo
    /// session without compounding an ambiguous filesystem state.
    pub fn ensure_source_available(&self, source: &Path) -> Result<()> {
        let source = source.to_string_lossy();
        let conn = self.conn.lock();
        let blocked: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM undo_entries \
             WHERE state IN ('pending', 'undoing', 'review') AND from_path = ?1)",
            [source.as_ref()],
            |row| row.get(0),
        )?;
        if blocked {
            return Err(anyhow!("UNDO_JOURNAL_NEEDS_REVIEW"));
        }
        Ok(())
    }

    /// Persist the exact filesystem plan before its first destructive step.
    pub fn prepare_move(
        &self,
        session_id: Uuid,
        source: &Path,
        destination: &Path,
        conflict_behavior: ConflictBehavior,
        backup_path: Option<&Path>,
        had_destination: bool,
    ) -> Result<JournaledUndoEntry> {
        let operation_id = Uuid::new_v4();
        let entry = UndoEntry {
            session_id,
            at: chrono::Utc::now(),
            from_path: source.to_string_lossy().into_owned(),
            to_path: destination.to_string_lossy().into_owned(),
        };
        let backup_path_string = backup_path.map(|path| path.to_string_lossy().into_owned());
        let behavior = match conflict_behavior {
            ConflictBehavior::Rename => "rename",
            ConflictBehavior::Overwrite => "overwrite",
            ConflictBehavior::Skip => "skip",
        };

        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let next_seq: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM undo_entries WHERE session_id = ?1",
            [entry.session_id.to_string()],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO undo_entries \
             (session_id, at, from_path, to_path, seq, operation_id, state, \
              conflict_behavior, backup_path, had_destination) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8, ?9)",
            rusqlite::params![
                entry.session_id.to_string(),
                entry.at.to_rfc3339(),
                entry.from_path,
                entry.to_path,
                next_seq,
                operation_id.to_string(),
                behavior,
                backup_path_string,
                i64::from(had_destination),
            ],
        )?;
        transaction.commit()?;

        Ok(JournaledUndoEntry {
            entry,
            operation_id: Some(operation_id),
            state: UndoJournalState::Pending,
            backup_path: backup_path.map(Path::to_path_buf),
            had_destination,
            seq: next_seq,
        })
    }

    pub fn mark_applied(&self, entry: &JournaledUndoEntry) -> Result<()> {
        self.set_state(entry, UndoJournalState::Applied)
    }

    pub fn mark_undoing(&self, entry: &JournaledUndoEntry) -> Result<()> {
        if entry.state != UndoJournalState::Applied {
            return Err(anyhow!("UNDO_JOURNAL_NEEDS_REVIEW"));
        }
        self.set_state(entry, UndoJournalState::Undoing)
    }

    fn mark_review(&self, entry: &JournaledUndoEntry, reason: &str) -> Result<()> {
        tracing::warn!(
            session_id = ?entry.session_id,
            operation_id = ?entry.operation_id,
            from = %entry.from_path,
            to = %entry.to_path,
            %reason,
            "undo journal requires review"
        );
        self.set_state(entry, UndoJournalState::Review)
    }

    fn set_state(&self, entry: &JournaledUndoEntry, state: UndoJournalState) -> Result<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE undo_entries SET state = ?1 WHERE session_id = ?2 AND seq = ?3",
            rusqlite::params![state.as_str(), entry.session_id.to_string(), entry.seq],
        )?;
        if changed != 1 {
            return Err(anyhow!("UNDO_JOURNAL_ENTRY_NOT_FOUND"));
        }
        Ok(())
    }

    /// Read entries newest-first after reconciling interrupted operations.
    pub fn read_reverse(&self, session_id: Uuid) -> Result<Vec<JournaledUndoEntry>> {
        self.reconcile()?;
        let conn = self.conn.lock();
        let mut statement = conn.prepare(
            "SELECT session_id, at, from_path, to_path, seq, operation_id, state, \
                    backup_path, had_destination \
             FROM undo_entries WHERE session_id = ?1 ORDER BY seq DESC",
        )?;
        let rows = statement.query_map([session_id.to_string()], parse_journal_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn stats(&self, session_id: Uuid) -> Result<UndoSessionStats> {
        self.reconcile()?;
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM undo_entries WHERE session_id = ?1",
            [session_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(UndoSessionStats {
            session_id,
            entry_count: usize::try_from(count).unwrap_or_default(),
            completed: 0,
            failed: 0,
        })
    }

    pub fn list_sessions(&self) -> Result<Vec<UndoSessionSummary>> {
        self.reconcile()?;
        let conn = self.conn.lock();
        let mut statement = conn.prepare(
            "SELECT session_id, MAX(at), COUNT(*), MIN(from_path) \
             FROM undo_entries GROUP BY session_id ORDER BY MAX(at) DESC",
        )?;
        let rows = statement.query_map([], |row| {
            let session_id: String = row.get(0)?;
            let at: String = row.get(1)?;
            let entry_count: i64 = row.get(2)?;
            let from_path: String = row.get(3)?;
            let session_id = Uuid::parse_str(&session_id).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })?;
            let at = chrono::DateTime::parse_from_rfc3339(&at)
                .map(|value| value.with_timezone(&chrono::Utc))
                .map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        1,
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
            Ok(UndoSessionSummary {
                session_id,
                at,
                entry_count: usize::try_from(entry_count).unwrap_or_default(),
                from_path,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    /// Explicitly discard a session. Retained overwrite backups are removed
    /// before their journal references, so a cleanup failure remains visible
    /// and retryable instead of orphaning hidden user data.
    pub fn delete_session(&self, session_id: Uuid) -> Result<()> {
        self.reconcile()?;
        let backups = {
            let conn = self.conn.lock();
            let mut statement = conn.prepare(
                "SELECT backup_path FROM undo_entries \
                 WHERE session_id = ?1 AND backup_path IS NOT NULL",
            )?;
            let rows = statement.query_map([session_id.to_string()], |row| {
                row.get::<_, String>(0).map(PathBuf::from)
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        for backup in backups {
            remove_managed_backup(&backup)?;
        }

        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM undo_entries WHERE session_id = ?1",
            [session_id.to_string()],
        )?;
        Ok(())
    }

    /// Remove one operation after its filesystem undo has completed.
    pub fn delete_entry(&self, entry: &JournaledUndoEntry) -> Result<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "DELETE FROM undo_entries WHERE session_id = ?1 AND seq = ?2",
            rusqlite::params![entry.session_id.to_string(), entry.seq],
        )?;
        if changed != 1 {
            return Err(anyhow!("UNDO_JOURNAL_ENTRY_NOT_FOUND"));
        }
        Ok(())
    }

    /// Restore the file that existed at an overwritten destination. This is
    /// called only after the newly moved file has been returned to its source.
    pub fn restore_overwrite_backup(&self, entry: &JournaledUndoEntry) -> Result<()> {
        if !entry.had_destination {
            return Ok(());
        }
        let backup = entry
            .backup_path
            .as_deref()
            .ok_or_else(|| anyhow!("UNDO_OVERWRITE_BACKUP_MISSING"))?;
        restore_backup(backup, Path::new(&entry.to_path))
    }

    /// Reconcile every interrupted journal row. Ambiguous situations are
    /// marked `review`; reconciliation never guesses by deleting user data.
    pub fn reconcile(&self) -> Result<()> {
        let entries = {
            let conn = self.conn.lock();
            let mut statement = conn.prepare(
                "SELECT session_id, at, from_path, to_path, seq, operation_id, state, \
                        backup_path, had_destination \
                 FROM undo_entries ORDER BY session_id, seq",
            )?;
            let rows = statement.query_map([], parse_journal_row)?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        for entry in entries {
            match entry.state {
                UndoJournalState::Pending => self.reconcile_pending(&entry)?,
                UndoJournalState::Applied => self.reconcile_applied(&entry)?,
                UndoJournalState::Undoing => self.reconcile_undoing(&entry)?,
                UndoJournalState::Review => {}
            }
        }
        Ok(())
    }

    fn reconcile_pending(&self, entry: &JournaledUndoEntry) -> Result<()> {
        let source = file_state(Path::new(&entry.from_path))?;
        let destination = file_state(Path::new(&entry.to_path))?;
        let backup = backup_state(entry)?;
        if source == FileState::Unsafe
            || destination == FileState::Unsafe
            || backup == FileState::Unsafe
        {
            return self.mark_review(entry, "unsafe path type in pending move");
        }

        match (source, destination) {
            (FileState::Regular, FileState::Missing) => {
                if entry.had_destination {
                    if backup != FileState::Regular {
                        return self.mark_review(entry, "overwrite backup is missing");
                    }
                    self.restore_overwrite_backup(entry)?;
                }
                self.delete_entry(entry)
            }
            (FileState::Missing, FileState::Regular) => {
                if entry.had_destination && backup != FileState::Regular {
                    self.mark_review(entry, "completed overwrite lost its backup")
                } else {
                    self.mark_applied(entry)
                }
            }
            (FileState::Regular, FileState::Regular) => {
                if entry.had_destination && backup == FileState::Missing {
                    // The overwrite never began, or its failure path restored
                    // the prior destination completely.
                    return self.delete_entry(entry);
                }
                if entry.had_destination
                    && backup == FileState::Regular
                    && entry.backup_path.as_deref().is_some_and(|path| {
                        same_file::is_same_file(&entry.to_path, path).unwrap_or(false)
                    })
                {
                    // Crash between publishing the overwrite backup and
                    // removing the original destination. Both names still
                    // reference the untouched old file.
                    if let Some(backup_path) = entry.backup_path.as_deref() {
                        remove_regular_file(backup_path)?;
                    }
                    return self.delete_entry(entry);
                }
                if same_file::is_same_file(&entry.from_path, &entry.to_path).unwrap_or(false) {
                    remove_regular_file(Path::new(&entry.to_path))?;
                    if entry.had_destination {
                        self.restore_overwrite_backup(entry)?;
                    }
                    self.delete_entry(entry)
                } else {
                    self.mark_review(entry, "both move paths exist with different identities")
                }
            }
            (FileState::Missing, FileState::Missing) => {
                self.mark_review(entry, "both move paths are missing")
            }
            _ => self.mark_review(entry, "unexpected pending filesystem state"),
        }
    }

    fn reconcile_applied(&self, entry: &JournaledUndoEntry) -> Result<()> {
        let source = file_state(Path::new(&entry.from_path))?;
        let destination = file_state(Path::new(&entry.to_path))?;
        let backup = backup_state(entry)?;
        if source == FileState::Unsafe
            || destination == FileState::Unsafe
            || backup == FileState::Unsafe
        {
            return self.mark_review(entry, "unsafe path type in applied move");
        }
        if entry.had_destination && backup != FileState::Regular {
            return self.mark_review(entry, "applied overwrite lost its backup");
        }

        match (source, destination) {
            (FileState::Missing, FileState::Regular) | (FileState::Regular, FileState::Regular) => {
                Ok(())
            }
            (FileState::Regular, FileState::Missing) => {
                // Handles a crash from older builds after the filesystem undo
                // but before deleting the legacy applied row.
                if entry.had_destination {
                    self.restore_overwrite_backup(entry)?;
                }
                self.delete_entry(entry)
            }
            (FileState::Missing, FileState::Missing) => {
                self.mark_review(entry, "both applied move paths are missing")
            }
            _ => self.mark_review(entry, "unexpected applied filesystem state"),
        }
    }

    fn reconcile_undoing(&self, entry: &JournaledUndoEntry) -> Result<()> {
        let source = file_state(Path::new(&entry.from_path))?;
        let destination = file_state(Path::new(&entry.to_path))?;
        let backup = backup_state(entry)?;
        if source == FileState::Unsafe
            || destination == FileState::Unsafe
            || backup == FileState::Unsafe
        {
            return self.mark_review(entry, "unsafe path type while undoing");
        }

        match (source, destination) {
            (FileState::Missing, FileState::Regular) => {
                if entry.had_destination && backup != FileState::Regular {
                    self.mark_review(entry, "overwrite backup is missing before undo")
                } else {
                    self.mark_applied(entry)
                }
            }
            (FileState::Regular, FileState::Missing) => {
                if entry.had_destination {
                    if backup != FileState::Regular {
                        return self
                            .mark_review(entry, "overwrite backup is missing after undo move");
                    }
                    self.restore_overwrite_backup(entry)?;
                }
                self.delete_entry(entry)
            }
            (FileState::Regular, FileState::Regular) => {
                if entry.had_destination {
                    if backup == FileState::Regular {
                        // The origin already existed and Skip prevented Undo.
                        self.mark_applied(entry)
                    } else {
                        // New file is back at its origin and the old
                        // destination has already been restored.
                        self.delete_entry(entry)
                    }
                } else if same_file::is_same_file(&entry.from_path, &entry.to_path).unwrap_or(false)
                {
                    // Crash between publishing a hard-link at the origin and
                    // removing the destination: roll the undo attempt back.
                    remove_regular_file(Path::new(&entry.from_path))?;
                    self.mark_applied(entry)
                } else {
                    // Normal Skip conflict: neither file was touched.
                    self.mark_applied(entry)
                }
            }
            (FileState::Missing, FileState::Missing) => {
                self.mark_review(entry, "both paths are missing while undoing")
            }
            _ => self.mark_review(entry, "unexpected undo filesystem state"),
        }
    }
}

fn parse_journal_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournaledUndoEntry> {
    let session_id_text: String = row.get(0)?;
    let at_text: String = row.get(1)?;
    let operation_id_text: Option<String> = row.get(5)?;
    let state_text: String = row.get(6)?;
    let backup_path: Option<String> = row.get(7)?;
    let session_id = Uuid::parse_str(&session_id_text).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let at = chrono::DateTime::parse_from_rfc3339(&at_text)
        .map(|date| date.with_timezone(&chrono::Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?;
    let operation_id = operation_id_text
        .map(|value| {
            Uuid::parse_str(&value).map_err(|error| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(error),
                )
            })
        })
        .transpose()?;

    Ok(JournaledUndoEntry {
        entry: UndoEntry {
            session_id,
            at,
            from_path: row.get(2)?,
            to_path: row.get(3)?,
        },
        operation_id,
        state: UndoJournalState::parse(&state_text),
        backup_path: backup_path.map(PathBuf::from),
        had_destination: row.get::<_, i64>(8)? != 0,
        seq: row.get(4)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileState {
    Missing,
    Regular,
    Unsafe,
}

fn file_state(path: &Path) -> Result<FileState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            Ok(FileState::Regular)
        }
        Ok(_) => Ok(FileState::Unsafe),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(FileState::Missing),
        Err(error) => Err(error).with_context(|| format!("checking {}", path.display())),
    }
}

fn backup_state(entry: &JournaledUndoEntry) -> Result<FileState> {
    match entry.backup_path.as_deref() {
        Some(path) => file_state(path),
        None => Ok(FileState::Missing),
    }
}

fn restore_backup(backup: &Path, destination: &Path) -> Result<()> {
    if file_state(backup)? != FileState::Regular {
        return Err(anyhow!("UNDO_OVERWRITE_BACKUP_MISSING"));
    }
    if file_state(destination)? != FileState::Missing {
        return Err(anyhow!("UNDO_DESTINATION_ALREADY_EXISTS"));
    }
    restore_retained_backup(backup, destination).context("UNDO_BACKUP_RESTORE_FAILED")
}

fn remove_regular_file(path: &Path) -> Result<()> {
    if file_state(path)? != FileState::Regular {
        return Err(anyhow!("UNDO_UNSAFE_FILE_TYPE"));
    }
    fs::remove_file(path).context("UNDO_ROLLBACK_REMOVE_FAILED")
}

fn remove_managed_backup(path: &Path) -> Result<()> {
    let is_managed_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(".filegrove-overwrite-backup-"));
    if !is_managed_name {
        return Err(anyhow!("UNDO_BACKUP_PATH_INVALID"));
    }
    match file_state(path)? {
        FileState::Missing => Ok(()),
        FileState::Regular => fs::remove_file(path).context("UNDO_BACKUP_CLEANUP_FAILED"),
        FileState::Unsafe => Err(anyhow!("UNDO_BACKUP_PATH_UNSAFE")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::file_ops::{execute_prepared_move, prepare_move, PrepareMoveOutcome};
    use crate::storage::Database;

    fn database() -> (tempfile::TempDir, Database) {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(&directory.path().join("filegrove.db")).unwrap();
        database.run_migrations().unwrap();
        (directory, database)
    }

    fn ready_move(
        source: &Path,
        destination: &Path,
        behavior: ConflictBehavior,
    ) -> crate::engine::file_ops::PreparedMove {
        match prepare_move(source, destination, behavior).unwrap() {
            PrepareMoveOutcome::Ready(prepared) => prepared,
            PrepareMoveOutcome::Skipped => panic!("unexpected skip"),
        }
    }

    #[test]
    fn pending_move_completed_on_disk_becomes_applied() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"data").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Rename);
        let session_id = Uuid::new_v4();
        database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();
        execute_prepared_move(prepared).unwrap();

        database.undo().reconcile().unwrap();

        let entries = database.undo().read_reverse(session_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].state, UndoJournalState::Applied);
        assert!(!source.exists());
        assert_eq!(fs::read(destination).unwrap(), b"data");
    }

    #[test]
    fn pending_move_that_never_started_is_removed() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"data").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Rename);
        let session_id = Uuid::new_v4();
        database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();

        database.undo().reconcile().unwrap();

        assert!(database.undo().read_reverse(session_id).unwrap().is_empty());
        assert_eq!(fs::read(source).unwrap(), b"data");
        assert!(!destination.exists());
    }

    #[test]
    fn interrupted_overwrite_before_move_restores_old_destination() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&destination, b"old").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Overwrite);
        let backup = prepared.overwrite_backup().unwrap().to_path_buf();
        let session_id = Uuid::new_v4();
        database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();
        fs::rename(&destination, &backup).unwrap();

        database.undo().reconcile().unwrap();

        assert!(database.undo().read_reverse(session_id).unwrap().is_empty());
        assert_eq!(fs::read(source).unwrap(), b"new");
        assert_eq!(fs::read(destination).unwrap(), b"old");
        assert!(!backup.exists());
    }

    #[test]
    fn interrupted_hard_link_publish_rolls_back_without_losing_source() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"data").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Rename);
        let session_id = Uuid::new_v4();
        database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();
        fs::hard_link(&source, &destination).unwrap();

        database.undo().reconcile().unwrap();

        assert!(database.undo().read_reverse(session_id).unwrap().is_empty());
        assert_eq!(fs::read(source).unwrap(), b"data");
        assert!(!destination.exists());
    }

    #[test]
    fn ambiguous_pending_move_preserves_both_files_and_blocks_automation() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"source").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Rename);
        let session_id = Uuid::new_v4();
        database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();
        fs::write(&destination, b"external").unwrap();

        database.undo().reconcile().unwrap();

        let entries = database.undo().read_reverse(session_id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].state, UndoJournalState::Review);
        assert_eq!(fs::read(&source).unwrap(), b"source");
        assert_eq!(fs::read(&destination).unwrap(), b"external");
        assert!(database.undo().ensure_source_available(&source).is_err());
    }

    #[test]
    fn overwrite_undo_restores_both_original_files() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&destination, b"old").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Overwrite);
        let session_id = Uuid::new_v4();
        let journal = database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();

        execute_prepared_move(prepared).unwrap();
        database.undo().mark_applied(&journal).unwrap();
        let applied = database.undo().read_reverse(session_id).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(fs::read(&destination).unwrap(), b"new");
        assert_eq!(
            fs::read(applied[0].backup_path.as_ref().unwrap()).unwrap(),
            b"old"
        );

        database.undo().mark_undoing(&applied[0]).unwrap();
        crate::engine::file_ops::move_file(&destination, &source, ConflictBehavior::Skip).unwrap();
        database
            .undo()
            .restore_overwrite_backup(&applied[0])
            .unwrap();
        database.undo().delete_entry(&applied[0]).unwrap();

        assert_eq!(fs::read(source).unwrap(), b"new");
        assert_eq!(fs::read(destination).unwrap(), b"old");
        assert!(database.undo().read_reverse(session_id).unwrap().is_empty());
    }

    #[test]
    fn interrupted_undo_is_completed_by_reconciliation() {
        let (directory, database) = database();
        let source = directory.path().join("source.txt");
        let destination = directory.path().join("destination.txt");
        fs::write(&source, b"data").unwrap();
        let prepared = ready_move(&source, &destination, ConflictBehavior::Rename);
        let session_id = Uuid::new_v4();
        let journal = database
            .undo()
            .prepare_move(
                session_id,
                prepared.source(),
                prepared.destination(),
                prepared.behavior(),
                prepared.overwrite_backup(),
                prepared.had_destination(),
            )
            .unwrap();
        execute_prepared_move(prepared).unwrap();
        database.undo().mark_applied(&journal).unwrap();
        let applied = database.undo().read_reverse(session_id).unwrap();
        database.undo().mark_undoing(&applied[0]).unwrap();
        crate::engine::file_ops::move_file(&destination, &source, ConflictBehavior::Skip).unwrap();

        database.undo().reconcile().unwrap();

        assert_eq!(fs::read(source).unwrap(), b"data");
        assert!(!destination.exists());
        assert!(database.undo().read_reverse(session_id).unwrap().is_empty());
    }
}
