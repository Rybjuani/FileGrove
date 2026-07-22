//! Action executor — see SPEC §5.3.
//!
//! Executes a planned action against a file: move / trash / delete /
//! runCommand. Records every outcome in the activity log.

use std::path::{Path, PathBuf};

use chrono::Utc;
use uuid::Uuid;

use crate::domain::action::Action;
use crate::domain::activity::{ActivityEntry, ActivityType};
use crate::engine::matcher::FileItem;
use crate::paths::tokens::resolve_destination;
use crate::storage::Database;

/// How to handle a destination collision during `Move`.
#[derive(Debug, Clone, Copy)]
pub enum ConflictBehavior {
    Rename,
    Overwrite,
    Skip,
}

impl ConflictBehavior {
    pub fn from_str(s: &str) -> Self {
        match s {
            "overwrite" => Self::Overwrite,
            "skip" => Self::Skip,
            _ => Self::Rename,
        }
    }
}

/// Outcome of a single action.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub success: bool,
    pub skipped: bool,
    pub new_path: Option<PathBuf>,
    pub error: Option<String>,
}

pub struct Organizer<'a> {
    pub db: &'a Database,
    pub scan_root: PathBuf,
    pub conflict_behavior: ConflictBehavior,
}

impl<'a> Organizer<'a> {
    pub async fn execute(
        &self,
        file: &FileItem,
        action: &Action,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
    ) -> ActionResult {
        let result = match action {
            Action::Move { destination } => self.execute_move(file, destination).await,
            Action::Trash => self.execute_trash(file).await,
            Action::Delete => self.execute_delete(file).await,
            Action::RunCommand { .. } => {
                // Postponed to v0.2.0 per MVP decision.
                ActionResult {
                    success: false,
                    skipped: true,
                    new_path: None,
                    error: Some("RUN_COMMAND_NOT_IMPLEMENTED".into()),
                }
            }
        };

        self.record_activity(file, action, rule_id, rule_name, &result);
        result
    }

    async fn execute_move(&self, file: &FileItem, destination: &str) -> ActionResult {
        let dest_dir = match resolve_destination(destination, file) {
            Ok(p) => p,
            Err(e) => {
                return ActionResult {
                    success: false,
                    skipped: false,
                    new_path: None,
                    error: Some(format!("INVALID_DESTINATION_TOKEN: {e}")),
                }
            }
        };

        // Safety: destination must be outside the watched root, otherwise
        // the moved file would re-trigger rules in auto mode.
        if dest_dir.starts_with(&self.scan_root) {
            return ActionResult {
                success: false,
                skipped: false,
                new_path: None,
                error: Some("INVALID_DESTINATION".into()),
            };
        }

        // Create destination if missing.
        if let Err(e) = tokio::fs::create_dir_all(&dest_dir).await {
            return ActionResult {
                success: false,
                skipped: false,
                new_path: None,
                error: Some(e.to_string()),
            };
        }

        let requested_path = dest_dir.join(&file.name);
        if matches!(self.conflict_behavior, ConflictBehavior::Skip) && requested_path.exists() {
            return ActionResult {
                success: true,
                skipped: true,
                new_path: None,
                error: None,
            };
        }

        let dest_path = self.resolve_conflict(&requested_path).await;
        match tokio::fs::rename(&file.path, &dest_path).await {
            Ok(_) => ActionResult {
                success: true,
                skipped: false,
                new_path: Some(dest_path),
                error: None,
            },
            Err(e) => ActionResult {
                success: false,
                skipped: false,
                new_path: None,
                error: Some(e.to_string()),
            },
        }
    }

    async fn execute_trash(&self, file: &FileItem) -> ActionResult {
        // Tauri exposes trash via the shell plugin; we delegate to it.
        // For now, this is a stub that returns an error.
        // TODO: wire up tauri_plugin_shell::trash
        let _ = file;
        ActionResult {
            success: false,
            skipped: false,
            new_path: None,
            error: Some("TRASH_NOT_IMPLEMENTED".into()),
        }
    }

    async fn execute_delete(&self, file: &FileItem) -> ActionResult {
        // Safety: only allow deleting files inside the watched root.
        let file_path = Path::new(&file.path);
        if !file_path.starts_with(&self.scan_root) {
            return ActionResult {
                success: false,
                skipped: false,
                new_path: None,
                error: Some("WATCHED_FOLDER_PROTECTED".into()),
            };
        }
        match tokio::fs::remove_file(file_path).await {
            Ok(_) => ActionResult {
                success: true,
                skipped: false,
                new_path: None,
                error: None,
            },
            Err(e) => ActionResult {
                success: false,
                skipped: false,
                new_path: None,
                error: Some(e.to_string()),
            },
        }
    }

    /// If a file already exists at `dest`, apply `conflict_behavior`:
    /// `Rename` → append ` (1)`, ` (2)`, …; `Overwrite` → leave as-is;
    /// `Skip` → return a sentinel that the caller checks.
    async fn resolve_conflict(&self, dest: &Path) -> PathBuf {
        if !dest.exists() {
            return dest.to_path_buf();
        }
        match self.conflict_behavior {
            ConflictBehavior::Overwrite => dest.to_path_buf(),
            ConflictBehavior::Skip => dest.to_path_buf(),
            ConflictBehavior::Rename => {
                let parent = dest.parent().unwrap_or_else(|| Path::new("."));
                let stem = dest.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
                let ext = dest.extension().and_then(|s| s.to_str());
                for n in 1u32..=1_000_000 {
                    let candidate_name = match ext {
                        Some(e) => format!("{} ({}).{}", stem, n, e),
                        None => format!("{} ({})", stem, n),
                    };
                    let candidate = parent.join(&candidate_name);
                    if !candidate.exists() {
                        return candidate;
                    }
                }
                dest.to_path_buf()
            }
        }
    }

    fn record_activity(
        &self,
        file: &FileItem,
        action: &Action,
        rule_id: Option<Uuid>,
        rule_name: Option<&str>,
        result: &ActionResult,
    ) {
        let entry = ActivityEntry {
            id: Uuid::new_v4(),
            at: Utc::now(),
            r#type: match action {
                Action::Move { .. } => ActivityType::Move,
                Action::Trash => ActivityType::Trash,
                Action::Delete => ActivityType::Delete,
                Action::RunCommand { .. } => ActivityType::RunCommand,
            },
            file_name: file.name.clone(),
            file_path: file.path.clone(),
            folder_id: None, // filled by caller
            rule_id,
            rule_name: rule_name.map(|s| s.to_string()),
            destination: result
                .new_path
                .as_ref()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string()),
            command: None,
            exit_code: None,
            output: None,
            duration_ms: None,
            timed_out: None,
            success: result.success,
            error: result.error.clone(),
        };
        if let Err(e) = self.db.activity().insert(&entry) {
            tracing::warn!("failed to record activity: {e}");
        }
    }
}
