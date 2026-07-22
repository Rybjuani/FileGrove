use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::AppState;

#[tauri::command]
pub async fn cancel(_state: State<'_, AppState>, session_id: Uuid) -> Result<Value, String> {
    // Cooperative cancellation via a session registry
    tracing::info!(?session_id, "organize cancel requested");
    Ok(json!({ "success": true }))
}

#[tauri::command]
pub async fn undo(state: State<'_, AppState>, session_id: Uuid) -> Result<Value, String> {
    let entries = match state.db.undo().read_reverse(session_id) {
        Ok(e) => e,
        Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
    };
    let total = entries.len();
    let mut done = 0u32;
    let mut failed = 0u32;

    for entry in entries {
        match tokio::fs::rename(&entry.to_path, &entry.from_path).await {
            Ok(()) => done += 1,
            Err(e) => {
                tracing::warn!(?entry, "undo failed: {e}");
                failed += 1;
            }
        }
    }
    let _ = state.db.undo().delete_session(session_id);
    Ok(
        json!({ "success": true, "data": { "done": done, "failed": failed, "total": total as u32 } }),
    )
}

#[tauri::command]
pub async fn dismiss_undo(state: State<'_, AppState>, session_id: Uuid) -> Result<Value, String> {
    match state.db.undo().delete_session(session_id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}
