use std::path::Path;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::domain::WatcherStatus;
use crate::AppState;

#[tauri::command]
pub async fn cancel(state: State<'_, AppState>, session_id: Uuid) -> Result<Value, String> {
    let active = state.cancellations.cancel(session_id);
    tracing::info!(?session_id, active, "organize cancel requested");
    Ok(json!({ "success": true, "data": { "active": active } }))
}

#[tauri::command]
pub async fn latest_undo_session(
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    let folder = match state.db.folders().get(folder_id) {
        Ok(Some(folder)) => folder,
        Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    let root = std::path::PathBuf::from(crate::paths::safety::canonical_path_key(Path::new(
        &folder.path,
    )));
    let sessions = match state.db.undo().list_sessions() {
        Ok(sessions) => sessions,
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    let latest = sessions.into_iter().find(|session| {
        let origin = std::path::PathBuf::from(crate::paths::safety::canonical_path_key(Path::new(
            &session.from_path,
        )));
        origin.starts_with(&root)
    });
    Ok(json!({ "success": true, "data": latest }))
}

#[tauri::command]
pub async fn list_undo_sessions(
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    let folder = match state.db.folders().get(folder_id) {
        Ok(Some(folder)) => folder,
        Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    let root = std::path::PathBuf::from(crate::paths::safety::canonical_path_key(Path::new(
        &folder.path,
    )));
    let sessions = match state.db.undo().list_sessions() {
        Ok(sessions) => sessions
            .into_iter()
            .filter(|session| {
                let origin = std::path::PathBuf::from(crate::paths::safety::canonical_path_key(
                    Path::new(&session.from_path),
                ));
                origin.starts_with(&root)
            })
            .collect::<Vec<_>>(),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    Ok(json!({ "success": true, "data": sessions }))
}

#[tauri::command]
pub async fn undo(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: Uuid,
) -> Result<Value, String> {
    if !state.cancellations.begin_undo(session_id) {
        return Ok(json!({ "success": false, "error": "UNDO_ALREADY_RUNNING" }));
    }
    let _undo_guard = UndoGuard {
        registry: &state.cancellations,
        session_id,
    };
    let entries = match state.db.undo().read_reverse(session_id) {
        Ok(entries) => entries,
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    if entries.is_empty() {
        return Ok(json!({ "success": false, "error": "INVALID_SESSION_ID" }));
    }

    let origin_folder = state.db.folders().list().ok().and_then(|folders| {
        let origin = std::path::PathBuf::from(crate::paths::safety::canonical_path_key(Path::new(
            &entries[0].from_path,
        )));
        folders.into_iter().find(|folder| {
            let root = std::path::PathBuf::from(crate::paths::safety::canonical_path_key(
                Path::new(&folder.path),
            ));
            origin.starts_with(root)
        })
    });
    let folder_lock_session = Uuid::new_v4();
    let _folder_guard = if let Some(folder) = origin_folder.as_ref() {
        let folder_key = crate::paths::safety::canonical_path_key(Path::new(&folder.path));
        if state
            .cancellations
            .begin_for_folder(folder_lock_session, folder_key)
            .is_none()
        {
            return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
        }
        Some(FolderOperationGuard {
            registry: &state.cancellations,
            session_id: folder_lock_session,
        })
    } else {
        None
    };

    if let Some(folder) = origin_folder.as_ref() {
        if let Err(error) = crate::engine::undo::ensure_latest_session_for_root(
            &state.db,
            session_id,
            Path::new(&folder.path),
        ) {
            return Ok(json!({ "success": false, "error": error }));
        }
    }

    let watcher_was_running = if let Some(folder) = origin_folder.as_ref() {
        if state.watcher.status(folder.id) == WatcherStatus::Running {
            if let Err(error) = state.watcher.pause(&app, folder.id) {
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }
            true
        } else {
            false
        }
    } else {
        false
    };
    let result = crate::engine::undo::undo_entries(&state.db, entries, |progress| {
        let _ = app.emit(
            "undo:progress",
            json!({
                "sessionId": session_id,
                "done": progress.done,
                "failed": progress.failed,
                "processed": progress.processed,
                "total": progress.total,
            }),
        );
    })
    .await;

    if watcher_was_running {
        // notify-debouncer uses 500 ms. Staying paused slightly longer than
        // that consumes the restore events instead of immediately re-running
        // the rules that the user just undid.
        tokio::time::sleep(std::time::Duration::from_millis(650)).await;
        if let Some(folder) = origin_folder.as_ref() {
            if let Err(error) = state.watcher.resume_without_catch_up(&app, folder.id) {
                tracing::warn!(folder_id = ?folder.id, "could not resume watcher after undo: {error}");
            }
        }
    }

    Ok(json!({
        "success": result.failed == 0,
        "data": {
            "done": result.done,
            "failed": result.failed,
            "total": result.total,
            "retryable": result.failed > 0,
            "errors": result.errors.into_iter().map(|error| json!({
                "fromPath": error.from_path,
                "toPath": error.to_path,
                "error": error.error,
            })).collect::<Vec<_>>(),
        },
        "error": if result.failed > 0 { Some("UNDO_PARTIAL_FAILURE") } else { None },
    }))
}

#[tauri::command]
pub async fn dismiss_undo(state: State<'_, AppState>, session_id: Uuid) -> Result<Value, String> {
    match state.db.undo().stats(session_id) {
        Ok(stats) if stats.entry_count == 0 => {
            return Ok(json!({ "success": false, "error": "INVALID_SESSION_ID" }))
        }
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
        Ok(_) => {}
    }
    match state.db.undo().delete_session(session_id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
    }
}

struct UndoGuard<'a> {
    registry: &'a crate::engine::cancellation::CancellationRegistry,
    session_id: Uuid,
}

impl Drop for UndoGuard<'_> {
    fn drop(&mut self) {
        self.registry.finish_undo(self.session_id);
    }
}

struct FolderOperationGuard<'a> {
    registry: &'a crate::engine::cancellation::CancellationRegistry,
    session_id: Uuid,
}

impl Drop for FolderOperationGuard<'_> {
    fn drop(&mut self) {
        self.registry.finish(self.session_id);
    }
}
