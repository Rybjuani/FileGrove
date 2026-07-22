use serde_json::{json, Value};
use std::path::Path;
use tauri::AppHandle;

#[tauri::command]
pub async fn check_access(_app: AppHandle, folder_path: String) -> Value {
    let path = Path::new(&folder_path);
    let has_access = path.exists() && path.read_dir().is_ok();
    json!({
        "success": true,
        "data": {
            "hasAccess": has_access,
            "message": if has_access { "" } else { "Folder not accessible" },
        }
    })
}

#[tauri::command]
pub async fn open_settings(app: AppHandle) -> Value {
    // macOS: open System Settings > Privacy
    // Windows: nothing useful to open
    // Linux: nothing
    let _ = app;
    json!({ "success": true })
}

#[tauri::command]
pub async fn is_macos() -> Value {
    json!({ "success": true, "data": cfg!(target_os = "macos") })
}
