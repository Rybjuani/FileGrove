//! Executes planned actions and records their activity/undo outcomes.

use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::domain::action::Action;
use crate::domain::activity::{ActivityEntry, ActivityType};
use crate::engine::cancellation::CancellationToken;
use crate::engine::command::{execute_shell_command_cancellable, CommandExecution};
pub use crate::engine::file_ops::ConflictBehavior;
use crate::engine::file_ops::{
    execute_prepared_move_cancellable, files_identical_cancellable, prepare_move, trash_file,
    DuplicateBehavior, MoveOutcome, PrepareMoveOutcome, PreparedMove,
};
use crate::engine::keyed_lock::KeyedAsyncLock;
use crate::engine::matcher::FileItem;
use crate::engine::planner::compute_organize_plan;
use crate::paths::safety::{
    relative_destination_root, validate_destination_directory,
    validate_destination_outside_watched_folders, validate_scan_root, validate_source_file,
};
use crate::paths::tokens::resolve_destination_from_file;
use crate::storage::command_ledger::CommandClaim;
use crate::storage::Database;

static DESTINATION_LOCKS: once_cell::sync::Lazy<KeyedAsyncLock<String>> =
    once_cell::sync::Lazy::new(KeyedAsyncLock::default);

/// Outcome of a single action.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub success: bool,
    pub skipped: bool,
    pub new_path: Option<PathBuf>,
    pub error: Option<String>,
    pub command_execution: Option<CommandExecution>,
    activity_type_override: Option<ActivityType>,
    activity_already_recorded: bool,
}

enum MovePreparation {
    Ready(PreparedMove),
    Skipped,
    Duplicate(DuplicateBehavior),
}

pub struct Organizer<'a> {
    pub db: &'a Database,
    pub session_id: Uuid,
    pub scan_root: PathBuf,
    pub conflict_behavior: ConflictBehavior,
    pub duplicate_behavior: DuplicateBehavior,
    pub preserve_relative_structure: bool,
    pub cancellation: Option<CancellationToken>,
}

impl<'a> Organizer<'a> {
    pub async fn execute(
        &self,
        file: &FileItem,
        action: &Action,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
    ) -> ActionResult {
        if let Err(error) = self
            .db
            .undo()
            .ensure_source_available(Path::new(&file.path))
        {
            let result = action_error(error.to_string());
            self.record_activity(file, action, rule_id, rule_name, &result);
            return result;
        }
        let current_file = match self.revalidate_plan(file, action, rule_id) {
            Ok(file) => file,
            Err(error) => {
                let result = action_error(error);
                self.record_activity(file, action, rule_id, rule_name, &result);
                return result;
            }
        };
        let pending_activity_id = if matches!(action, Action::Delete) {
            let pending_id = Uuid::new_v4();
            let pending = self.activity_entry(
                pending_id,
                &current_file,
                action,
                rule_id,
                rule_name,
                &action_error("DELETE_IN_PROGRESS".to_string()),
            );
            if let Err(error) = self.db.activity().insert(&pending) {
                // Permanent deletion is forbidden unless its audit row is
                // durable first. A disk-full/database error must leave the
                // source file untouched.
                return action_error(format!("ACTIVITY_JOURNAL_FAILED: {error}"));
            }
            Some(pending_id)
        } else {
            None
        };

        let mut result = match action {
            Action::Move { destination } => {
                self.execute_move(&current_file, destination, rule_id, rule_name)
                    .await
            }
            Action::Trash => self.execute_trash(&current_file).await,
            Action::Delete => self.execute_delete(&current_file).await,
            Action::RunCommand {
                command,
                timeout_ms,
                rerun_always,
            } => {
                self.execute_command(&current_file, command, *timeout_ms, *rerun_always, rule_id)
                    .await
            }
        };

        if let Some(activity_id) = pending_activity_id {
            let completed = self.activity_entry(
                activity_id,
                &current_file,
                action,
                rule_id,
                rule_name,
                &result,
            );
            if let Err(error) = self.db.activity().update(&completed) {
                tracing::error!(
                    ?activity_id,
                    "failed to finalize destructive activity: {error}"
                );
                result.success = false;
                result.error = Some(format!("ACTIVITY_FINALIZE_FAILED: {error}"));
            }
        } else if !result.activity_already_recorded {
            let recorded = self.record_activity(&current_file, action, rule_id, rule_name, &result);
            if recorded && result.error.as_deref() == Some("COMMAND_INTERRUPTED_REVIEW_REQUIRED") {
                if let (Some(rule_id), Action::RunCommand { command, .. }) = (rule_id, action) {
                    if let Err(error) = self.db.command_ledger().mark_interrupted_reported(
                        rule_id,
                        &current_file.path,
                        command,
                    ) {
                        tracing::warn!("could not finalize interrupted command review: {error}");
                    }
                }
            }
        }
        result
    }

    fn revalidate_plan(
        &self,
        planned_file: &FileItem,
        planned_action: &Action,
        rule_id: Option<Uuid>,
    ) -> Result<FileItem, String> {
        let source = validate_source_file(&self.scan_root, Path::new(&planned_file.path))
            .map_err(|error| error.to_string())?;
        let source = source.to_str().ok_or("SOURCE_FILE_INVALID")?;
        let current = FileItem::from_path_sync(source)
            .map_err(|error| format!("SOURCE_FILE_INVALID: {error}"))?;
        if !planned_file.is_same_unchanged_file(&current) {
            return Err("SOURCE_FILE_CHANGED".to_string());
        }

        let rule_id = rule_id.ok_or("RULE_NOT_FOUND")?;
        let scan_root_key = crate::paths::safety::canonical_path_key(&self.scan_root);
        let folder = self
            .db
            .folders()
            .list()
            .map_err(|error| format!("FOLDER_LOAD_FAILED: {error}"))?
            .into_iter()
            .find(|folder| {
                crate::paths::safety::canonical_path_key(Path::new(&folder.path)) == scan_root_key
            })
            .ok_or("FOLDER_NOT_FOUND")?;
        let rules = self
            .db
            .rules()
            .get_by_ids(&folder.rule_ids)
            .map_err(|error| format!("RULE_LOAD_FAILED: {error}"))?;
        let winning = compute_organize_plan(vec![current.clone()], rules)
            .pop()
            .ok_or("PLAN_NO_LONGER_VALID")?;
        if winning.rule_id != Some(rule_id) || winning.action.as_ref() != Some(planned_action) {
            return Err("PLAN_NO_LONGER_VALID".to_string());
        }
        Ok(current)
    }

    async fn execute_move(
        &self,
        file: &FileItem,
        destination: &str,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
    ) -> ActionResult {
        let source = match validate_source_file(&self.scan_root, Path::new(&file.path)) {
            Ok(path) => path,
            Err(error) => return action_error(error.to_string()),
        };
        let scan_root = match validate_scan_root(&self.scan_root) {
            Ok(path) => path,
            Err(error) => return action_error(error.to_string()),
        };
        let mut destination_directory = match resolve_destination_from_file(destination, file).await
        {
            Ok(path) => path,
            Err(error) => return action_error(error),
        };
        if destination_directory.is_relative() {
            if let Err(error) = relative_destination_root(destination) {
                return action_error(error.to_string());
            }
            destination_directory = scan_root.join(destination_directory);
        }

        if self.preserve_relative_structure {
            if let Some(relative_parent) = source
                .parent()
                .and_then(|parent| parent.strip_prefix(&scan_root).ok())
                .filter(|relative| !relative.as_os_str().is_empty())
            {
                destination_directory.push(relative_parent);
            }
        }

        let destination_directory =
            match validate_destination_directory(&scan_root, &destination_directory) {
                Ok(path) => path,
                Err(error) => return action_error(error.to_string()),
            };
        if let Err(error) = self.validate_against_other_watched_folders(&destination_directory) {
            return action_error(error);
        }
        if let Err(error) = tokio::fs::create_dir_all(&destination_directory).await {
            return action_error(format!("DESTINATION_CREATE_FAILED: {error}"));
        }
        // Resolve again after creation to close the gap between checking a
        // missing path and using it (including directory symlink redirects).
        let destination_directory =
            match validate_destination_directory(&scan_root, &destination_directory) {
                Ok(path) => path,
                Err(error) => return action_error(error.to_string()),
            };
        if let Err(error) = self.validate_against_other_watched_folders(&destination_directory) {
            return action_error(error);
        }
        let Some(file_name) = source.file_name() else {
            return action_error("SOURCE_FILE_INVALID".into());
        };
        let requested_path = destination_directory.join(file_name);
        if requested_path == source {
            return action_error("SOURCE_EQUALS_DESTINATION".into());
        }

        // Two watched folders can target the same destination concurrently.
        // Serialize selection+journaling+move for that requested path so
        // Rename mode can observe the first result and choose the next suffix.
        let destination_lock =
            DESTINATION_LOCKS.for_key(crate::paths::safety::canonical_path_key(&requested_path));
        let _destination_guard = destination_lock.lock().await;

        let conflict_behavior = self.conflict_behavior;
        let duplicate_behavior = self.duplicate_behavior;
        let expected = file.clone();
        let source_for_prepare = source.clone();
        let destination_for_prepare = requested_path.clone();
        let cancellation_for_compare = self.cancellation.clone();
        let prepared = match tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            revalidate_immediately_before_action(&expected, &source_for_prepare)?;
            let duplicate = destination_for_prepare.try_exists()?
                && files_identical_cancellable(
                    &source_for_prepare,
                    &destination_for_prepare,
                    cancellation_for_compare.as_ref(),
                )?;
            if duplicate && duplicate_behavior != DuplicateBehavior::KeepBoth {
                return Ok(MovePreparation::Duplicate(duplicate_behavior));
            }
            let behavior = if duplicate && conflict_behavior == ConflictBehavior::Overwrite {
                ConflictBehavior::Rename
            } else {
                conflict_behavior
            };
            match prepare_move(&source_for_prepare, &destination_for_prepare, behavior)? {
                PrepareMoveOutcome::Ready(prepared) => Ok(MovePreparation::Ready(prepared)),
                PrepareMoveOutcome::Skipped => Ok(MovePreparation::Skipped),
            }
        })
        .await
        {
            Ok(Ok(MovePreparation::Ready(prepared))) => prepared,
            Ok(Ok(MovePreparation::Skipped)) => return action_skipped(),
            Ok(Ok(MovePreparation::Duplicate(DuplicateBehavior::Trash))) => {
                return self
                    .execute_duplicate_trash(file, source, requested_path)
                    .await;
            }
            Ok(Ok(MovePreparation::Duplicate(DuplicateBehavior::Delete))) => {
                return self
                    .execute_duplicate_delete(file, source, requested_path, rule_id, rule_name)
                    .await;
            }
            Ok(Ok(MovePreparation::Duplicate(DuplicateBehavior::KeepBoth))) => {
                return action_error("DUPLICATE_POLICY_INVALID".to_string());
            }
            Ok(Err(error)) => return action_error(error.to_string()),
            Err(error) => return action_error(format!("MOVE_PREPARE_TASK_FAILED: {error}")),
        };
        let journal = match self.db.undo().prepare_move(
            self.session_id,
            prepared.source(),
            prepared.destination(),
            prepared.behavior(),
            prepared.overwrite_backup(),
            prepared.had_destination(),
        ) {
            Ok(entry) => entry,
            Err(error) => return action_error(format!("UNDO_JOURNAL_PREPARE_FAILED: {error}")),
        };
        let cancellation = self.cancellation.clone();
        let moved = tokio::task::spawn_blocking(move || {
            execute_prepared_move_cancellable(prepared, cancellation)
        })
        .await;
        match moved {
            Ok(Ok(MoveOutcome::Moved(destination))) => {
                if let Err(error) = self.db.undo().mark_applied(&journal) {
                    // The durable pending row already contains the full
                    // recovery plan. Reconciliation can promote it from the
                    // observed filesystem state, now or after a restart.
                    tracing::error!(
                        session_id = ?self.session_id,
                        "could not finalize undo journal after move: {error}"
                    );
                    if let Err(reconcile_error) = self.db.undo().reconcile() {
                        tracing::error!(
                            session_id = ?self.session_id,
                            "undo journal reconciliation deferred: {reconcile_error}"
                        );
                    }
                }
                ActionResult {
                    success: true,
                    skipped: false,
                    new_path: Some(destination),
                    error: None,
                    command_execution: None,
                    activity_type_override: None,
                    activity_already_recorded: false,
                }
            }
            Ok(Ok(MoveOutcome::Skipped)) => {
                if let Err(error) = self.db.undo().delete_entry(&journal) {
                    tracing::warn!("could not discard skipped move journal: {error}");
                }
                action_skipped()
            }
            Ok(Err(error)) => {
                let requires_recovery = error.to_string().contains("DESTINATION_RESTORE_FAILED");
                let cleanup = if requires_recovery {
                    self.db.undo().reconcile()
                } else {
                    self.db.undo().delete_entry(&journal)
                };
                if let Err(cleanup_error) = cleanup {
                    tracing::warn!("could not reconcile failed move journal: {cleanup_error}");
                }
                action_error(error.to_string())
            }
            Err(error) => {
                if let Err(reconcile_error) = self.db.undo().reconcile() {
                    tracing::warn!("could not reconcile interrupted move: {reconcile_error}");
                }
                action_error(format!("MOVE_TASK_FAILED: {error}"))
            }
        }
    }

    async fn execute_duplicate_trash(
        &self,
        file: &FileItem,
        source: PathBuf,
        destination: PathBuf,
    ) -> ActionResult {
        let expected = file.clone();
        let cancellation = self.cancellation.clone();
        match tokio::task::spawn_blocking(move || {
            if cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
            {
                anyhow::bail!("MOVE_CANCELLED");
            }
            revalidate_immediately_before_action(&expected, &source)?;
            if !files_identical_cancellable(&source, &destination, cancellation.as_ref())? {
                anyhow::bail!("DUPLICATE_CHANGED");
            }
            trash_file(&source)
        })
        .await
        {
            Ok(Ok(())) => ActionResult {
                success: true,
                skipped: false,
                new_path: None,
                error: None,
                command_execution: None,
                activity_type_override: Some(ActivityType::Trash),
                activity_already_recorded: false,
            },
            Ok(Err(error)) => action_error(error.to_string()),
            Err(error) => action_error(format!("TRASH_TASK_FAILED: {error}")),
        }
    }

    async fn execute_duplicate_delete(
        &self,
        file: &FileItem,
        source: PathBuf,
        destination: PathBuf,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
    ) -> ActionResult {
        let activity_id = Uuid::new_v4();
        let delete_action = Action::Delete;
        let pending = self.activity_entry(
            activity_id,
            file,
            &delete_action,
            rule_id,
            rule_name,
            &action_error("DELETE_IN_PROGRESS".to_string()),
        );
        if let Err(error) = self.db.activity().insert(&pending) {
            return action_error(format!("ACTIVITY_JOURNAL_FAILED: {error}"));
        }

        let expected = file.clone();
        let cancellation = self.cancellation.clone();
        let mut result = match tokio::task::spawn_blocking(move || {
            if cancellation
                .as_ref()
                .is_some_and(CancellationToken::is_cancelled)
            {
                anyhow::bail!("MOVE_CANCELLED");
            }
            revalidate_immediately_before_action(&expected, &source)?;
            if !files_identical_cancellable(&source, &destination, cancellation.as_ref())? {
                anyhow::bail!("DUPLICATE_CHANGED");
            }
            std::fs::remove_file(source).map_err(anyhow::Error::from)
        })
        .await
        {
            Ok(Ok(())) => ActionResult {
                success: true,
                skipped: false,
                new_path: None,
                error: None,
                command_execution: None,
                activity_type_override: Some(ActivityType::Delete),
                activity_already_recorded: true,
            },
            Ok(Err(error))
                if error.to_string().starts_with("SOURCE_FILE_CHANGED")
                    || error.to_string().starts_with("DUPLICATE_CHANGED")
                    || error.to_string().starts_with("MOVE_CANCELLED") =>
            {
                action_error(error.to_string())
            }
            Ok(Err(error)) => action_error(format!("DELETE_FAILED: {error}")),
            Err(error) => action_error(format!("DELETE_TASK_FAILED: {error}")),
        };
        result.activity_type_override = Some(ActivityType::Delete);
        result.activity_already_recorded = true;

        let completed = self.activity_entry(
            activity_id,
            file,
            &delete_action,
            rule_id,
            rule_name,
            &result,
        );
        if let Err(error) = self.db.activity().update(&completed) {
            tracing::error!(
                ?activity_id,
                "failed to finalize duplicate deletion: {error}"
            );
            result.success = false;
            result.error = Some(format!("ACTIVITY_FINALIZE_FAILED: {error}"));
        }
        result
    }

    async fn execute_trash(&self, file: &FileItem) -> ActionResult {
        let path = match validate_source_file(&self.scan_root, Path::new(&file.path)) {
            Ok(path) => path,
            Err(error) => return action_error(error.to_string()),
        };
        let expected = file.clone();
        match tokio::task::spawn_blocking(move || {
            revalidate_immediately_before_action(&expected, &path)?;
            trash_file(&path)
        })
        .await
        {
            Ok(Ok(())) => ActionResult {
                success: true,
                skipped: false,
                new_path: None,
                error: None,
                command_execution: None,
                activity_type_override: None,
                activity_already_recorded: false,
            },
            Ok(Err(error)) => action_error(error.to_string()),
            Err(error) => action_error(format!("TRASH_TASK_FAILED: {error}")),
        }
    }

    fn validate_against_other_watched_folders(&self, destination: &Path) -> Result<(), String> {
        let folders = self
            .db
            .folders()
            .list()
            .map_err(|error| format!("FOLDER_LOAD_FAILED: {error}"))?;
        let current_root = crate::paths::safety::canonical_path_key(&self.scan_root);
        validate_destination_outside_watched_folders(
            destination,
            folders
                .iter()
                .filter(|folder| {
                    crate::paths::safety::canonical_path_key(Path::new(&folder.path))
                        != current_root
                })
                .map(|folder| Path::new(&folder.path)),
        )
        .map_err(|error| error.to_string())
    }

    async fn execute_delete(&self, file: &FileItem) -> ActionResult {
        let path = match validate_source_file(&self.scan_root, Path::new(&file.path)) {
            Ok(path) => path,
            Err(error) => return action_error(error.to_string()),
        };
        let expected = file.clone();
        match tokio::task::spawn_blocking(move || {
            revalidate_immediately_before_action(&expected, &path)?;
            std::fs::remove_file(path).map_err(anyhow::Error::from)
        })
        .await
        {
            Ok(Ok(())) => ActionResult {
                success: true,
                skipped: false,
                new_path: None,
                error: None,
                command_execution: None,
                activity_type_override: None,
                activity_already_recorded: false,
            },
            Ok(Err(error)) if error.to_string().starts_with("SOURCE_FILE_CHANGED") => {
                action_error(error.to_string())
            }
            Ok(Err(error)) => action_error(format!("DELETE_FAILED: {error}")),
            Err(error) => action_error(format!("DELETE_TASK_FAILED: {error}")),
        }
    }

    async fn execute_command(
        &self,
        file: &FileItem,
        command: &str,
        timeout_ms: u32,
        rerun_always: bool,
        rule_id: Option<Uuid>,
    ) -> ActionResult {
        let source = match validate_source_file(&self.scan_root, Path::new(&file.path)) {
            Ok(path) => path,
            Err(error) => return action_error(error.to_string()),
        };
        let Some(rule_id) = rule_id else {
            return action_error("RULE_NOT_FOUND".to_string());
        };
        let Some(source_path) = source.to_str() else {
            return action_error("SOURCE_FILE_INVALID".to_string());
        };
        // Refresh metadata immediately before claiming the fingerprint so a
        // file changed between scan and execution is never mistaken for an
        // older version.
        let current_file = match FileItem::from_path_sync(source_path) {
            Ok(file) => file,
            Err(error) => return action_error(format!("SOURCE_FILE_INVALID: {error}")),
        };
        if !file.is_same_unchanged_file(&current_file) {
            return action_error("SOURCE_FILE_CHANGED".to_string());
        }
        let claim = match self.db.command_ledger().try_begin(
            rule_id,
            &current_file.path,
            command,
            current_file.size,
            current_file.modified_at,
            rerun_always,
        ) {
            Ok(claim) => claim,
            Err(error) => return action_error(format!("COMMAND_LEDGER_FAILED: {error}")),
        };
        match claim {
            CommandClaim::Claimed => {}
            CommandClaim::AlreadyCompleted
            | CommandClaim::InProgress
            | CommandClaim::InterruptedAwaitingReview => {
                return ActionResult {
                    success: true,
                    skipped: true,
                    new_path: None,
                    error: None,
                    command_execution: None,
                    activity_type_override: None,
                    activity_already_recorded: false,
                };
            }
            CommandClaim::InterruptedNeedsReview => {
                return action_error("COMMAND_INTERRUPTED_REVIEW_REQUIRED".to_string());
            }
        }

        let execution = execute_shell_command_cancellable(
            command,
            &current_file,
            timeout_ms,
            self.cancellation.clone(),
        )
        .await;
        let latest_file = FileItem::from_path_sync(&current_file.path).unwrap_or(current_file);
        let ledger_error = if execution.cancelled {
            self.db
                .command_ledger()
                .abort(rule_id, &latest_file.path, command)
                .err()
        } else {
            self.db
                .command_ledger()
                .finish(
                    rule_id,
                    &latest_file.path,
                    command,
                    latest_file.size,
                    latest_file.modified_at,
                )
                .err()
        };
        let mut result = ActionResult {
            success: execution.success,
            skipped: false,
            new_path: None,
            error: execution.error.clone(),
            command_execution: Some(execution),
            activity_type_override: None,
            activity_already_recorded: false,
        };
        if let Some(error) = ledger_error {
            result.success = false;
            result.error = Some(format!("COMMAND_LEDGER_FAILED: {error}"));
        }
        result
    }

    fn record_activity(
        &self,
        file: &FileItem,
        action: &Action,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
        result: &ActionResult,
    ) -> bool {
        // A command fingerprint skip is expected bookkeeping, not a new
        // activity. Omitting it avoids flooding History on watcher catch-up.
        if matches!(action, Action::RunCommand { .. }) && result.skipped {
            return true;
        }
        let entry = self.activity_entry(Uuid::new_v4(), file, action, rule_id, rule_name, result);
        if let Err(error) = self.db.activity().insert(&entry) {
            tracing::warn!("failed to record activity: {error}");
            false
        } else {
            true
        }
    }

    fn activity_entry(
        &self,
        id: Uuid,
        file: &FileItem,
        action: &Action,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
        result: &ActionResult,
    ) -> ActivityEntry {
        let command_execution = result.command_execution.as_ref();
        ActivityEntry {
            id,
            at: Utc::now(),
            r#type: result.activity_type_override.unwrap_or(match action {
                Action::Move { .. } => ActivityType::Move,
                Action::Trash => ActivityType::Trash,
                Action::Delete => ActivityType::Delete,
                Action::RunCommand { .. } => ActivityType::RunCommand,
            }),
            file_name: file.name.clone(),
            file_path: file.path.clone(),
            folder_id: self
                .scan_root
                .to_str()
                .and_then(|path| self.db.folders().get_by_path(path).ok().flatten())
                .map(|folder| folder.id),
            rule_id,
            rule_name: rule_name.map(str::to_string),
            destination: result
                .new_path
                .as_ref()
                .and_then(|path| path.to_str())
                .map(str::to_string),
            command: command_execution
                .map(|execution| execution.command.clone())
                .or_else(|| match action {
                    Action::RunCommand { command, .. } => Some(command.clone()),
                    _ => None,
                }),
            exit_code: command_execution.and_then(|execution| execution.exit_code),
            output: command_execution.map(|execution| execution.output.clone()),
            duration_ms: command_execution.map(|execution| execution.duration_ms),
            timed_out: command_execution.map(|execution| execution.timed_out),
            success: result.success,
            error: result.error.clone(),
        }
    }
}

fn action_error(error: String) -> ActionResult {
    ActionResult {
        success: false,
        skipped: false,
        new_path: None,
        error: Some(error),
        command_execution: None,
        activity_type_override: None,
        activity_already_recorded: false,
    }
}

fn action_skipped() -> ActionResult {
    ActionResult {
        success: true,
        skipped: true,
        new_path: None,
        error: None,
        command_execution: None,
        activity_type_override: None,
        activity_already_recorded: false,
    }
}

fn revalidate_immediately_before_action(expected: &FileItem, path: &Path) -> anyhow::Result<()> {
    let current = FileItem::from_path_sync(path.to_string_lossy().as_ref())
        .map_err(|error| anyhow::anyhow!("SOURCE_FILE_INVALID: {error}"))?;
    if !expected.is_same_unchanged_file(&current) {
        anyhow::bail!("SOURCE_FILE_CHANGED");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::activity::ActivityFilters;
    use std::fs;

    #[tokio::test]
    async fn identical_conflict_keeps_both_by_default_and_records_undo() {
        let temporary = tempfile::tempdir().unwrap();
        let watched = temporary.path().join("watched");
        let destination = watched.join("Documents");
        fs::create_dir(&watched).unwrap();
        let source = watched.join("report.txt");
        fs::write(&source, b"report").unwrap();
        fs::create_dir(&destination).unwrap();
        fs::write(destination.join("report.txt"), b"report").unwrap();

        let database_path = temporary.path().join("app.db");
        let database = Database::open(&database_path).unwrap();
        database.run_migrations().unwrap();
        let session_id = Uuid::new_v4();
        let file = FileItem::from_path_sync(source.to_string_lossy().as_ref()).unwrap();
        let action = Action::Move {
            destination: "Documents".into(),
        };
        let rule_id = Uuid::new_v4();
        database
            .rules()
            .insert(&crate::domain::Rule {
                id: rule_id,
                name: "Documents".into(),
                is_quick_rule: false,
                enabled: true,
                order: 0,
                icon: "File".into(),
                conditions: vec![crate::domain::Condition::Extension {
                    value: vec![".txt".into()],
                }],
                action: action.clone(),
                created_at: Utc::now(),
            })
            .unwrap();
        database
            .folders()
            .insert(&crate::domain::Folder {
                id: Uuid::new_v4(),
                path: watched.to_string_lossy().into_owned(),
                name: "Watched".into(),
                mode: crate::domain::FolderMode::Single,
                rule_ids: vec![rule_id],
                watcher_status: crate::domain::WatcherStatus::Stopped,
                created_at: Utc::now(),
                subfolders: crate::domain::SubfolderConfig::default(),
                exclude_folders: Vec::new(),
                exclude_patterns: Vec::new(),
            })
            .unwrap();
        let organizer = Organizer {
            db: &database,
            session_id,
            scan_root: watched,
            conflict_behavior: ConflictBehavior::Rename,
            duplicate_behavior: DuplicateBehavior::KeepBoth,
            preserve_relative_structure: false,
            cancellation: None,
        };

        let result = organizer
            .execute(&file, &action, Some(rule_id), Some("Documents"))
            .await;

        assert!(result.success, "{:?}", result.error);
        assert!(!source.exists());
        assert!(destination.join("report.txt").exists());
        assert!(destination.join("report (1).txt").exists());
        let undo_entries = database.undo().read_reverse(session_id).unwrap();
        assert_eq!(undo_entries.len(), 1);
        assert_eq!(undo_entries[0].from_path, file.path);
        assert_eq!(
            undo_entries[0].to_path,
            destination.join("report (1).txt").to_string_lossy()
        );

        let (activity, total) = database
            .activity()
            .list(&ActivityFilters::default(), 10, 0)
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(activity[0].r#type, ActivityType::Move);
        assert!(activity[0].success);
    }

    #[tokio::test]
    async fn identical_duplicate_deletion_is_audited_before_removal() {
        let temporary = tempfile::tempdir().unwrap();
        let watched = temporary.path().join("watched");
        let destination = watched.join("Documents");
        fs::create_dir_all(&destination).unwrap();
        let source = watched.join("report.txt");
        fs::write(&source, b"identical").unwrap();
        fs::write(destination.join("report.txt"), b"identical").unwrap();

        let database = Database::open(&temporary.path().join("app.db")).unwrap();
        database.run_migrations().unwrap();
        let session_id = Uuid::new_v4();
        let file = FileItem::from_path_sync(source.to_string_lossy().as_ref()).unwrap();
        let action = Action::Move {
            destination: "Documents".into(),
        };
        let rule_id = Uuid::new_v4();
        database
            .rules()
            .insert(&crate::domain::Rule {
                id: rule_id,
                name: "Documents".into(),
                is_quick_rule: false,
                enabled: true,
                order: 0,
                icon: "File".into(),
                conditions: vec![crate::domain::Condition::Extension {
                    value: vec![".txt".into()],
                }],
                action: action.clone(),
                created_at: Utc::now(),
            })
            .unwrap();
        database
            .folders()
            .insert(&crate::domain::Folder {
                id: Uuid::new_v4(),
                path: watched.to_string_lossy().into_owned(),
                name: "Watched".into(),
                mode: crate::domain::FolderMode::Single,
                rule_ids: vec![rule_id],
                watcher_status: crate::domain::WatcherStatus::Stopped,
                created_at: Utc::now(),
                subfolders: crate::domain::SubfolderConfig::default(),
                exclude_folders: Vec::new(),
                exclude_patterns: Vec::new(),
            })
            .unwrap();
        let organizer = Organizer {
            db: &database,
            session_id,
            scan_root: watched,
            conflict_behavior: ConflictBehavior::Rename,
            duplicate_behavior: DuplicateBehavior::Delete,
            preserve_relative_structure: false,
            cancellation: None,
        };

        let result = organizer
            .execute(&file, &action, Some(rule_id), Some("Documents"))
            .await;

        assert!(result.success, "{:?}", result.error);
        assert!(!source.exists());
        assert_eq!(
            fs::read(destination.join("report.txt")).unwrap(),
            b"identical"
        );
        assert!(database.undo().read_reverse(session_id).unwrap().is_empty());
        let (activity, total) = database
            .activity()
            .list(&ActivityFilters::default(), 10, 0)
            .unwrap();
        assert_eq!(total, 1);
        assert_eq!(activity[0].r#type, ActivityType::Delete);
        assert!(activity[0].success);
    }
}
