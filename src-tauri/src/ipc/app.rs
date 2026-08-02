use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, Window};

#[tauri::command]
pub async fn get_version(app: AppHandle) -> serde_json::Value {
    let version = app.package_info().version.to_string();
    json!({ "success": true, "data": version })
}

#[tauri::command]
pub async fn get_icon(_app: AppHandle) -> serde_json::Value {
    json!({ "success": true, "data": "" })
}

#[tauri::command]
pub fn minimize(window: Window) {
    let _ = window.minimize();
}

#[tauri::command]
pub fn maximize(window: Window) {
    let _ = window.maximize();
}

#[tauri::command]
pub fn close(window: Window) {
    let _ = window.close();
}

#[tauri::command]
pub fn set_automation_paused(app: AppHandle, paused: bool) -> serde_json::Value {
    match apply_automation_paused(&app, paused) {
        Ok(()) => json!({ "success": true, "data": { "paused": paused } }),
        Err(error) => json!({ "success": false, "error": error }),
    }
}

pub(crate) fn apply_automation_paused(app: &AppHandle, paused: bool) -> Result<(), String> {
    let state = app
        .try_state::<crate::AppState>()
        .ok_or_else(|| "APP_NOT_READY".to_string())?;

    // Persist first so a crash immediately after the click restores the safe
    // state on the next launch.
    state
        .db
        .settings()
        .set("automationPaused", &serde_json::Value::Bool(paused))
        .map_err(|error| error.to_string())?;
    if paused {
        state.watcher.pause_all(app);
    } else {
        state.watcher.resume_all(app);
    }
    let _ = app.emit(
        "automation:pause-changed",
        serde_json::json!({ "paused": paused }),
    );
    Ok(())
}
