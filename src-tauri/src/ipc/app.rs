use serde_json::json;
use tauri::{AppHandle, Window};

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
