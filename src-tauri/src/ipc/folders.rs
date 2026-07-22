use std::path::PathBuf;

use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::domain::{Folder, FolderMode, Schedule, SubfolderConfig, WatcherStatus};
use crate::AppState;

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Value, String> {
    match state.db.folders().list() {
        Ok(folders) => Ok(json!({ "success": true, "data": folders })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn add(app: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    let path = match pick_folder(&app).await {
        Some(p) => p,
        None => return Ok(json!({ "success": false, "error": "USER_CANCELED_FOLDER_SELECT" })),
    };

    let name = PathBuf::from(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Folder")
        .to_string();

    match state.db.folders().get_by_path(&path) {
        Ok(Some(folder)) => return Ok(json!({ "success": true, "data": folder })),
        Ok(None) => {}
        Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
    }

    let folder = Folder {
        id: Uuid::new_v4(),
        path: path.clone(),
        name,
        mode: FolderMode::Single,
        rule_ids: vec![],
        watcher_status: WatcherStatus::Stopped,
        created_at: chrono::Utc::now(),
        subfolders: SubfolderConfig::default(),
        schedule: None,
        exclude_folders: vec![],
        exclude_patterns: vec![],
    };

    if let Err(e) = state.db.folders().insert(&folder) {
        return Ok(json!({ "success": false, "error": e.to_string() }));
    }

    Ok(json!({ "success": true, "data": folder }))
}

#[tauri::command]
pub async fn remove(app: AppHandle, state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    if let Err(error) = state.watcher.stop_watching(&app, id) {
        return Ok(json!({ "success": false, "error": error.to_string() }));
    }
    if let Err(error) = state.scheduler.remove(id).await {
        return Ok(json!({ "success": false, "error": error.to_string() }));
    }
    match state.db.folders().delete(id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn update_name(
    state: State<'_, AppState>,
    id: Uuid,
    name: String,
) -> Result<Value, String> {
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            folder.name = name;
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn update_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    mode: String,
) -> Result<Value, String> {
    let new_mode = match FolderMode::from_str(&mode) {
        Some(m) => m,
        None => return Ok(json!({ "success": false, "error": "INVALID_MODE" })),
    };
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let previous_folder = folder.clone();
            folder.mode = new_mode;
            if new_mode == FolderMode::Auto {
                if let Some(schedule) = folder.schedule.as_mut() {
                    schedule.enabled = false;
                }
            }
            if let Err(error) = state.db.folders().update(&folder) {
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }

            let reconcile_result = match new_mode {
                FolderMode::Single => match state.watcher.stop_watching(&app, id) {
                    Ok(()) => state.scheduler.remove(id).await,
                    Err(error) => Err(error),
                },
                FolderMode::Auto => match state.scheduler.remove(id).await {
                    Ok(()) => state.watcher.start_watching_and_catch_up(&app, &folder),
                    Err(error) => Err(error),
                },
                FolderMode::Scheduled => match state.watcher.stop_watching(&app, id) {
                    Ok(()) => state.scheduler.upsert(id).await.map(|_| ()),
                    Err(error) => Err(error),
                },
            };
            if let Err(error) = reconcile_result {
                let _ = state.db.folders().update(&previous_folder);
                match previous_folder.mode {
                    FolderMode::Single => {
                        let _ = state.watcher.stop_watching(&app, id);
                        let _ = state.scheduler.remove(id).await;
                    }
                    FolderMode::Auto => {
                        let _ = state.scheduler.remove(id).await;
                        let _ = state
                            .watcher
                            .start_watching_and_catch_up(&app, &previous_folder);
                    }
                    FolderMode::Scheduled => {
                        let _ = state.watcher.stop_watching(&app, id);
                        let _ = state.scheduler.upsert(id).await;
                    }
                }
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }

            match state.db.folders().get(id) {
                Ok(Some(updated)) => Ok(json!({ "success": true, "data": updated })),
                Ok(None) => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn assign_rules(
    state: State<'_, AppState>,
    folder_id: Uuid,
    rule_ids: Vec<Uuid>,
) -> Result<Value, String> {
    match state.db.folders().get(folder_id) {
        Ok(Some(mut folder)) => {
            for rid in &rule_ids {
                if !folder.rule_ids.contains(rid) {
                    folder.rule_ids.push(*rid);
                }
            }
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn unassign_rules(
    state: State<'_, AppState>,
    folder_id: Uuid,
    rule_ids: Vec<Uuid>,
) -> Result<Value, String> {
    match state.db.folders().get(folder_id) {
        Ok(Some(mut folder)) => {
            folder.rule_ids.retain(|r| !rule_ids.contains(r));
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn update_subfolders_config(
    state: State<'_, AppState>,
    id: Uuid,
    config: SubfolderConfig,
) -> Result<Value, String> {
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            folder.subfolders = config;
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn update_schedule(
    state: State<'_, AppState>,
    id: Uuid,
    schedule: Schedule,
) -> Result<Value, String> {
    if schedule.enabled && schedule.to_cron().is_none() {
        return Ok(json!({ "success": false, "error": "INVALID_SCHEDULE" }));
    }
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let previous_schedule = folder.schedule.clone();
            folder.schedule = Some(schedule);
            match state.db.folders().update(&folder) {
                Ok(()) => {
                    // Reconcile the scheduler job.
                    if let Err(error) = state.scheduler.upsert(id).await {
                        folder.schedule = previous_schedule;
                        let _ = state.db.folders().update(&folder);
                        let _ = state.scheduler.upsert(id).await;
                        return Ok(json!({ "success": false, "error": error.to_string() }));
                    }
                    Ok(json!({ "success": true, "data": folder }))
                }
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn get_next_run(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    let next_run = state.scheduler.next_run(id).map(|run| run.to_rfc3339());
    Ok(json!({ "success": true, "data": { "nextRun": next_run } }))
}

#[tauri::command]
pub async fn select_destination(app: AppHandle) -> Result<Value, String> {
    match pick_folder(&app).await {
        Some(p) => Ok(json!({ "success": true, "data": p })),
        None => Ok(json!({ "success": false, "error": "USER_CANCELED_FOLDER_SELECT" })),
    }
}

#[tauri::command]
pub async fn confirm_first_run(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            folder.subfolders.first_run_confirmed = true;
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

async fn pick_folder(app: &AppHandle) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Select folder")
        .pick_folder(move |path| {
            let p = path.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string()));
            let _ = tx.send(p);
        });
    rx.await.ok().flatten()
}
