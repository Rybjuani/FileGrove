//! IPC commands for the file deduplication feature.
//!
//! All deduplication is **non-destructive by default**: duplicates are sent to
//! the system Recycle Bin / Trash, never permanently deleted.

use serde::Serialize;

use crate::engine::dedup::{scan_for_duplicates, DuplicateReport};

// ─── Response wrapper ────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcResponse<T: Serialize> {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl<T: Serialize> IpcResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }
    fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

// ─── find_duplicates ─────────────────────────────────────────────────────────

/// Scan `folder_path` for byte-identical files.
///
/// Returns a [`DuplicateReport`] sorted by wasted bytes (largest group first).
/// The scan skips `.git` / `.hg` / `.svn` repositories and respects the
/// folder's exclude lists so it cannot trigger the auto-organiser loop.
#[tauri::command]
pub async fn find_duplicates(
    folder_path: String,
    exclude_folders: Vec<String>,
    exclude_patterns: Vec<String>,
) -> IpcResponse<DuplicateReport> {
    let path = std::path::Path::new(&folder_path);
    match scan_for_duplicates(path, &exclude_folders, &exclude_patterns) {
        Ok(report) => IpcResponse::ok(report),
        Err(e) => IpcResponse::err(format!("{e:#}")),
    }
}

// ─── trash_duplicates ────────────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashResult {
    pub trashed: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// Move the given file paths to the system Recycle Bin / Trash.
///
/// Each path must have been returned by a previous `find_duplicates` call as a
/// **duplicate** (not an original).  The frontend is responsible for filtering
/// the selection; the backend does *not* re-verify identity here (the user
/// confirmed in the UI).
///
/// Returns the number of files successfully trashed and any per-file errors.
#[tauri::command]
pub async fn trash_duplicates(paths: Vec<String>) -> IpcResponse<TrashResult> {
    let mut trashed = 0usize;
    let mut failed = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for path_str in &paths {
        let path = std::path::Path::new(path_str);

        // Safety: refuse to trash directories or non-existent paths.
        if !path.exists() {
            failed += 1;
            errors.push(format!("{path_str}: archivo no encontrado"));
            continue;
        }
        if path.is_dir() {
            failed += 1;
            errors.push(format!("{path_str}: es una carpeta, no un archivo"));
            continue;
        }

        match trash::delete(path) {
            Ok(()) => trashed += 1,
            Err(e) => {
                failed += 1;
                errors.push(format!("{path_str}: {e}"));
            }
        }
    }

    IpcResponse::ok(TrashResult {
        trashed,
        failed,
        errors,
    })
}
