use serde_json::{json, Value};
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::AppState;

#[tauri::command]
pub async fn start(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    let folder = match state.db.folders().get(folder_id) {
        Ok(Some(f)) => f,
        _ => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    };
    match state.watcher.start_watching(&app, &folder) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn stop(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    match state.watcher.stop_watching(&app, folder_id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn pause(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    match state.watcher.pause(&app, folder_id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn resume(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    match state.watcher.resume(&app, folder_id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn status(state: State<'_, AppState>, folder_id: Uuid) -> Result<Value, String> {
    let s = state.watcher.status(folder_id);
    Ok(json!({ "success": true, "data": { "status": s.as_str() } }))
}
