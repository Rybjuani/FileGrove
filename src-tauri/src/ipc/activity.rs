use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

use crate::domain::activity::ActivityType;
use crate::storage::activity::ActivityFilters;
use crate::AppState;

#[tauri::command]
pub async fn list_activity(
    state: State<'_, AppState>,
    opts: serde_json::Value,
) -> Result<Value, String> {
    let limit = opts
        .get("limit")
        .and_then(|value| value.as_u64())
        .unwrap_or(50)
        .clamp(1, 200) as u32;
    let offset = opts
        .get("offset")
        .and_then(|value| value.as_u64())
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;
    let filters = ActivityFilters {
        folder_id: opts
            .get("folderId")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok()),
        rule_id: opts
            .get("ruleId")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok()),
        success: opts.get("success").and_then(|v| v.as_bool()),
        r#type: opts
            .get("type")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "move" => Some(ActivityType::Move),
                "trash" => Some(ActivityType::Trash),
                "delete" => Some(ActivityType::Delete),
                "runCommand" => Some(ActivityType::RunCommand),
                _ => None,
            }),
        since: opts
            .get("since")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc)),
        until: opts
            .get("until")
            .and_then(|v| v.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&chrono::Utc)),
    };

    match state.db.activity().list(&filters, limit, offset) {
        Ok((items, total)) => {
            Ok(json!({ "success": true, "data": { "items": items, "total": total } }))
        }
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn delete_activity(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<Uuid>,
) -> Result<Value, String> {
    match state.db.activity().delete(&ids) {
        Ok(()) => {
            let _ = app.emit("activity:changed", json!({ "kind": "delete", "ids": ids }));
            Ok(json!({ "success": true }))
        }
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn clear(app: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    match state.db.activity().clear() {
        Ok(()) => {
            let _ = app.emit("activity:changed", json!({ "kind": "clear" }));
            Ok(json!({ "success": true }))
        }
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}
