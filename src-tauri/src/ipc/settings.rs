use serde_json::{json, Value};
use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn get(state: State<'_, AppState>) -> Result<Value, String> {
    match state.db.settings().get_all() {
        Ok(map) => {
            let obj = serde_json::Value::Object(map);
            Ok(json!({ "success": true, "data": obj }))
        }
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn set(
    state: State<'_, AppState>,
    settings: serde_json::Map<String, serde_json::Value>,
) -> Result<Value, String> {
    match state.db.settings().set_many(&settings) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}
