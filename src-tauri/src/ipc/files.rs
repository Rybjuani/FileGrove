use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::engine::cancellation::{CancellationRegistry, CancellationToken};
use crate::engine::matcher::FileItem;
use crate::engine::planner::compute_organize_plan;
use crate::storage::Database;
use crate::AppState;

/// Single-level scan: list files (no subfolders) in `folder_path`.
#[tauri::command]
pub async fn scan(folder_path: String) -> Result<Value, String> {
    Ok(scan_inner(&folder_path, false, None).await)
}

/// Recursive scan: walk the folder tree up to `subfolders.max_depth`,
/// honoring excludes. Streams `scan:progress` events.
#[tauri::command]
pub async fn scan_recursive(folder_path: String) -> Result<Value, String> {
    Ok(scan_inner(&folder_path, true, None).await)
}

async fn scan_inner(
    folder_path: &str,
    recursive: bool,
    cancellation: Option<&CancellationToken>,
) -> Value {
    let root = PathBuf::from(folder_path);
    if !root.exists() {
        return json!({ "success": false, "error": "WATCHED_FOLDER_INVALID" });
    }

    let mut files: Vec<FileItem> = vec![];
    let mut truncated = false;
    let mut read = 0u64;
    const MAX_FILES: u64 = 100_000;

    let walker = if recursive {
        walkdir::WalkDir::new(&root).max_depth(5)
    } else {
        walkdir::WalkDir::new(&root).max_depth(1)
    };

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            break;
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if crate::watcher::filters::should_ignore(entry.path()) {
            continue;
        }
        if read >= MAX_FILES {
            truncated = true;
            break;
        }
        if let Ok(item) = FileItem::from_path_sync(entry.path().to_string_lossy().as_ref()) {
            files.push(item);
            read += 1;
        }
    }

    json!({
        "success": true,
        "data": {
            "files": files,
            "stats": {
                "scanned": read,
                "truncated": truncated,
                "cancelled": cancellation.is_some_and(CancellationToken::is_cancelled),
            }
        }
    })
}

/// Preview: compute the organize plan without executing it.
#[tauri::command]
pub async fn preview(state: State<'_, AppState>, folder_path: String) -> Result<Value, String> {
    Ok(preview_inner(&state.db, &folder_path, false).await)
}

#[tauri::command]
pub async fn preview_recursive(
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(preview_inner(&state.db, &folder_path, true).await)
}

async fn preview_inner(db: &Database, folder_path: &str, recursive: bool) -> Value {
    let scan_result = scan_inner(folder_path, recursive, None).await;
    if !scan_result["success"].as_bool().unwrap_or(false) {
        return scan_result;
    }
    let files: Vec<FileItem> =
        serde_json::from_value(scan_result["data"]["files"].clone()).unwrap_or_default();

    // Load every rule for now; the matching engine will skip disabled ones.
    let rules = match db.rules().list() {
        Ok(r) => r,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };

    let plan = compute_organize_plan(files, rules);
    json!({ "success": true, "data": { "plan": plan, "stats": scan_result["data"]["stats"] } })
}

/// Organize: scan + execute actions. Returns a session ID for later undo.
#[tauri::command]
pub async fn organize(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(organize_inner(&app, &state.db, &state.cancellations, &folder_path, false).await)
}

#[tauri::command]
pub async fn organize_recursive(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
    opts: Option<serde_json::Value>,
) -> Result<Value, String> {
    drop(opts);
    Ok(organize_inner(&app, &state.db, &state.cancellations, &folder_path, true).await)
}

pub async fn organize_inner(
    app: &AppHandle,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    recursive: bool,
) -> Value {
    let session_id = Uuid::new_v4();
    let cancellation = cancellations.begin(session_id);
    let _session_guard = SessionGuard {
        registry: cancellations,
        session_id,
    };

    let _ = app.emit(
        "organize:started",
        json!({
            "sessionId": session_id,
            "folderPath": folder_path,
        }),
    );

    let scan_result = scan_inner(folder_path, recursive, Some(&cancellation)).await;
    if !scan_result["success"].as_bool().unwrap_or(false) {
        let _ = app.emit(
            "organize:done",
            json!({
                "sessionId": session_id,
                "folderPath": folder_path,
                "cancelled": false,
                "error": scan_result["error"],
                "stats": { "done": 0, "failed": 0, "total": 0 }
            }),
        );
        return scan_result;
    }
    let files: Vec<FileItem> =
        serde_json::from_value(scan_result["data"]["files"].clone()).unwrap_or_default();
    if cancellation.is_cancelled() {
        let total = files.len() as u32;
        let _ = app.emit(
            "organize:done",
            json!({
                "sessionId": session_id,
                "folderPath": folder_path,
                "cancelled": true,
                "stats": { "done": 0, "failed": 0, "total": total }
            }),
        );
        return json!({
            "success": true,
            "data": {
                "sessionId": session_id,
                "cancelled": true,
                "undoCount": 0,
                "stats": { "done": 0, "failed": 0, "total": total }
            }
        });
    }

    let rules = match db.rules().list() {
        Ok(r) => r,
        Err(e) => {
            let error = e.to_string();
            let _ = app.emit(
                "organize:done",
                json!({
                    "sessionId": session_id,
                    "folderPath": folder_path,
                    "cancelled": false,
                    "error": error,
                    "stats": { "done": 0, "failed": 0, "total": 0 }
                }),
            );
            return json!({ "success": false, "error": error });
        }
    };

    let plan = compute_organize_plan(files, rules);
    let conflict_behavior_str = db.settings().get_conflict_behavior_str();
    let conflict_behavior =
        crate::engine::executor::ConflictBehavior::from_str(&conflict_behavior_str);
    let scan_root = PathBuf::from(folder_path);

    let mut done = 0u32;
    let mut failed = 0u32;
    let total = plan.len() as u32;
    let mut processed = 0u32;
    let mut cancelled = cancellation.is_cancelled();

    for action in &plan {
        if cancellation.is_cancelled() {
            cancelled = true;
            break;
        }
        if let Some(ref act) = action.action {
            let organizer = crate::engine::executor::Organizer {
                db,
                session_id,
                scan_root: scan_root.clone(),
                conflict_behavior,
            };
            let result = organizer
                .execute(
                    &action.file,
                    act,
                    action.rule_id,
                    action.rule_name.as_deref(),
                )
                .await;
            if result.success && !result.skipped {
                done += 1;
            } else if !result.skipped {
                failed += 1;
            }
        }
        processed += 1;
        let _ = app.emit(
            "organize:progress",
            json!({
                "sessionId": session_id,
                "folderPath": folder_path,
                "done": done,
                "failed": failed,
                "processed": processed,
                "total": total,
            }),
        );
    }

    let _ = app.emit(
        "organize:done",
        json!({
            "sessionId": session_id,
            "folderPath": folder_path,
            "cancelled": cancelled,
            "stats": { "done": done, "failed": failed, "total": total }
        }),
    );

    let undo_count = db
        .undo()
        .stats(session_id)
        .map(|stats| stats.entry_count as u32)
        .unwrap_or(0);

    json!({
        "success": true,
        "data": {
            "sessionId": session_id,
            "cancelled": cancelled,
            "undoCount": undo_count,
            "stats": { "done": done, "failed": failed, "total": total }
        }
    })
}

struct SessionGuard<'a> {
    registry: &'a CancellationRegistry,
    session_id: Uuid,
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        self.registry.finish(self.session_id);
    }
}
