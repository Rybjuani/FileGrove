//! Filesystem restoration for durable Undo sessions.
//!
//! This module intentionally has no Tauri dependency. IPC can report progress
//! to the window, while tests exercise the exact same restoration code against
//! disposable files and databases.

use std::path::{Path, PathBuf};

use uuid::Uuid;

use crate::engine::file_ops::{move_file, ConflictBehavior, MoveOutcome};
use crate::storage::undo::{JournaledUndoEntry, UndoJournalState};
use crate::storage::Database;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UndoProgress {
    pub done: u32,
    pub failed: u32,
    pub processed: u32,
    pub total: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoFailure {
    pub from_path: String,
    pub to_path: String,
    pub error: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoResult {
    pub done: u32,
    pub failed: u32,
    pub total: u32,
    pub errors: Vec<UndoFailure>,
}

/// Undo is a per-folder stack. An older session cannot be restored while a
/// newer layout still depends on it; repeated Undo eventually reaches every
/// session.
pub fn ensure_latest_session_for_root(
    db: &Database,
    session_id: Uuid,
    root: &Path,
) -> Result<(), String> {
    let root = PathBuf::from(crate::paths::safety::canonical_path_key(root));
    let latest = db
        .undo()
        .list_sessions()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|session| {
            let origin = PathBuf::from(crate::paths::safety::canonical_path_key(Path::new(
                &session.from_path,
            )));
            origin.starts_with(&root)
        });
    if latest.is_some_and(|session| session.session_id != session_id) {
        return Err("UNDO_NEWER_SESSION_EXISTS".to_string());
    }
    Ok(())
}

#[cfg(test)]
async fn undo_session<F>(
    db: &Database,
    session_id: Uuid,
    on_progress: F,
) -> Result<UndoResult, String>
where
    F: FnMut(UndoProgress),
{
    let entries = db
        .undo()
        .read_reverse(session_id)
        .map_err(|error| error.to_string())?;
    if entries.is_empty() {
        return Err("INVALID_SESSION_ID".to_string());
    }

    Ok(undo_entries(db, entries, on_progress).await)
}

pub async fn undo_entries<F>(
    db: &Database,
    entries: Vec<JournaledUndoEntry>,
    mut on_progress: F,
) -> UndoResult
where
    F: FnMut(UndoProgress),
{
    let total = entries.len() as u32;
    let mut done = 0u32;
    let mut failed = 0u32;
    let mut errors = Vec::new();

    for entry in entries {
        let error = if entry.state != UndoJournalState::Applied {
            Some("UNDO_JOURNAL_NEEDS_REVIEW".to_string())
        } else if let Err(error) = db.undo().mark_undoing(&entry) {
            Some(format!("UNDO_LOG_UPDATE_FAILED: {error}"))
        } else {
            let source = entry.to_path.clone();
            let destination = entry.from_path.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                move_file(
                    Path::new(&source),
                    Path::new(&destination),
                    ConflictBehavior::Skip,
                )
            })
            .await;

            match outcome {
                Ok(Ok(MoveOutcome::Moved(_))) => match db.undo().restore_overwrite_backup(&entry) {
                    Ok(()) => match db.undo().delete_entry(&entry) {
                        Ok(()) => {
                            done += 1;
                            None
                        }
                        Err(error) => Some(format!("UNDO_LOG_UPDATE_FAILED: {error}")),
                    },
                    Err(error) => {
                        let _ = db.undo().reconcile();
                        Some(format!("UNDO_BACKUP_RESTORE_FAILED: {error}"))
                    }
                },
                Ok(Ok(MoveOutcome::Skipped)) => {
                    let _ = db.undo().reconcile();
                    Some("UNDO_ORIGIN_ALREADY_EXISTS".to_string())
                }
                Ok(Err(error)) => {
                    let _ = db.undo().reconcile();
                    Some(format!("UNDO_MOVE_FAILED: {error}"))
                }
                Err(error) => {
                    let _ = db.undo().reconcile();
                    Some(format!("UNDO_TASK_FAILED: {error}"))
                }
            }
        };

        if let Some(error) = error {
            failed += 1;
            tracing::warn!(?entry, %error, "undo failed");
            if errors.len() < 100 {
                errors.push(UndoFailure {
                    from_path: entry.from_path.clone(),
                    to_path: entry.to_path.clone(),
                    error,
                });
            }
        }

        on_progress(UndoProgress {
            done,
            failed,
            processed: done + failed,
            total,
        });
    }

    UndoResult {
        done,
        failed,
        total,
        errors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    use chrono::Utc;

    use crate::domain::{
        Action, Condition, Folder, FolderMode, Rule, SubfolderConfig, WatcherStatus,
    };
    use crate::engine::cancellation::CancellationRegistry;
    use crate::engine::executor::Organizer;
    use crate::engine::file_ops::DuplicateBehavior;
    use crate::engine::matcher::FileItem;

    struct Fixture {
        _temporary: tempfile::TempDir,
        watched: PathBuf,
        db: Database,
        rule_id: Uuid,
        action: Action,
    }

    impl Fixture {
        fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let watched = temporary.path().join("watched");
            fs::create_dir(&watched).unwrap();
            let db = Database::open(&temporary.path().join("app.db")).unwrap();
            db.run_migrations().unwrap();
            let rule_id = Uuid::new_v4();
            let action = Action::Move {
                destination: "Documents".into(),
            };
            db.rules()
                .insert(&Rule {
                    id: rule_id,
                    name: "Documents".into(),
                    is_quick_rule: false,
                    enabled: true,
                    order: 0,
                    icon: "File".into(),
                    conditions: vec![Condition::Extension {
                        value: vec![".txt".into()],
                    }],
                    action: action.clone(),
                    created_at: Utc::now(),
                })
                .unwrap();
            db.folders()
                .insert(&Folder {
                    id: Uuid::new_v4(),
                    path: watched.to_string_lossy().into_owned(),
                    name: "Watched".into(),
                    mode: FolderMode::Single,
                    rule_ids: vec![rule_id],
                    watcher_status: WatcherStatus::Stopped,
                    created_at: Utc::now(),
                    subfolders: SubfolderConfig::default(),
                    exclude_folders: Vec::new(),
                    exclude_patterns: Vec::new(),
                })
                .unwrap();
            Self {
                _temporary: temporary,
                watched,
                db,
                rule_id,
                action,
            }
        }

        fn create_file(&self, name: &str, contents: &[u8]) -> FileItem {
            let path = self.watched.join(name);
            fs::write(&path, contents).unwrap();
            FileItem::from_path_sync(path.to_string_lossy().as_ref()).unwrap()
        }

        async fn move_file(
            &self,
            file: &FileItem,
            session_id: Uuid,
            cancellation: Option<crate::engine::cancellation::CancellationToken>,
        ) -> crate::engine::executor::ActionResult {
            Organizer {
                db: &self.db,
                session_id,
                scan_root: self.watched.clone(),
                conflict_behavior: ConflictBehavior::Rename,
                duplicate_behavior: DuplicateBehavior::KeepBoth,
                preserve_relative_structure: false,
                cancellation,
            }
            .execute(file, &self.action, Some(self.rule_id), Some("Documents"))
            .await
        }
    }

    #[tokio::test]
    async fn real_filesystem_sessions_undo_newest_first_then_restore_every_file() {
        let fixture = Fixture::new();
        let first = fixture.create_file("first.txt", b"first");
        let second = fixture.create_file("second.txt", b"second");
        let first_session = Uuid::new_v4();
        let second_session = Uuid::new_v4();

        assert!(fixture.move_file(&first, first_session, None).await.success);
        tokio::time::sleep(Duration::from_millis(2)).await;
        assert!(
            fixture
                .move_file(&second, second_session, None)
                .await
                .success
        );

        let sessions = fixture.db.undo().list_sessions().unwrap();
        assert_eq!(
            sessions
                .iter()
                .map(|session| session.session_id)
                .collect::<Vec<_>>(),
            vec![second_session, first_session]
        );
        assert_eq!(
            ensure_latest_session_for_root(&fixture.db, first_session, &fixture.watched),
            Err("UNDO_NEWER_SESSION_EXISTS".to_string())
        );
        ensure_latest_session_for_root(&fixture.db, second_session, &fixture.watched).unwrap();

        let second_undo = undo_session(&fixture.db, second_session, |_| {})
            .await
            .unwrap();
        assert_eq!((second_undo.done, second_undo.failed), (1, 0));
        assert!(fixture.watched.join("second.txt").exists());
        assert!(!fixture.watched.join("first.txt").exists());
        assert_eq!(fixture.db.undo().list_sessions().unwrap().len(), 1);

        ensure_latest_session_for_root(&fixture.db, first_session, &fixture.watched).unwrap();
        let first_undo = undo_session(&fixture.db, first_session, |_| {})
            .await
            .unwrap();
        assert_eq!((first_undo.done, first_undo.failed), (1, 0));
        assert_eq!(
            fs::read(fixture.watched.join("first.txt")).unwrap(),
            b"first"
        );
        assert_eq!(
            fs::read(fixture.watched.join("second.txt")).unwrap(),
            b"second"
        );
        assert!(fixture.db.undo().list_sessions().unwrap().is_empty());
    }

    #[tokio::test]
    async fn cancellation_keeps_completed_moves_in_one_fully_undoable_session() {
        let fixture = Fixture::new();
        let first = fixture.create_file("first.txt", b"first");
        let second = fixture.create_file("second.txt", b"second");
        let session_id = Uuid::new_v4();
        let cancellations = CancellationRegistry::default();
        let token = cancellations.begin(session_id);

        assert!(
            fixture
                .move_file(&first, session_id, Some(token.clone()))
                .await
                .success
        );
        assert!(cancellations.cancel(session_id));
        let cancelled = fixture.move_file(&second, session_id, Some(token)).await;
        cancellations.finish(session_id);

        assert!(!cancelled.success);
        assert!(cancelled
            .error
            .as_deref()
            .is_some_and(|error| error.contains("MOVE_CANCELLED")));
        assert!(!fixture.watched.join("first.txt").exists());
        assert!(fixture.watched.join("second.txt").exists());
        assert_eq!(fixture.db.undo().stats(session_id).unwrap().entry_count, 1);

        let mut progress = Vec::new();
        let result = undo_session(&fixture.db, session_id, |update| progress.push(update))
            .await
            .unwrap();
        assert_eq!((result.done, result.failed, result.total), (1, 0, 1));
        assert_eq!(progress.last().map(|update| update.processed), Some(1));
        assert_eq!(
            fs::read(fixture.watched.join("first.txt")).unwrap(),
            b"first"
        );
        assert_eq!(
            fs::read(fixture.watched.join("second.txt")).unwrap(),
            b"second"
        );
        assert!(fixture.db.undo().list_sessions().unwrap().is_empty());
    }
}
