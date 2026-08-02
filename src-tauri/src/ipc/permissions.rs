use serde_json::{json, Value};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use uuid::Uuid;

#[tauri::command]
pub async fn check_access(_app: AppHandle, folder_path: String) -> Value {
    let path = Path::new(&folder_path);
    let result = check_read_write_access(path);
    let (has_access, message) = match result {
        Ok(()) => (true, String::new()),
        Err(message) => (false, message),
    };
    json!({
        "success": true,
        "data": {
            "hasAccess": has_access,
            "message": message,
        }
    })
}

pub(crate) fn check_read_write_access(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err("WATCHED_FOLDER_INVALID".to_string());
    }
    path.read_dir()
        .map_err(|error| format!("FOLDER_READ_DENIED: {error}"))?;

    // Metadata flags are not sufficient on Windows because ACLs can deny
    // writes independently. A zero-byte, create-new probe verifies the same
    // permission FileGrove needs and is removed before returning.
    let probe = access_probe_path(path);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
        .map_err(|error| format!("FOLDER_WRITE_DENIED: {error}"))?;
    std::fs::remove_file(&probe).map_err(|error| format!("FOLDER_WRITE_DENIED: {error}"))?;
    Ok(())
}

fn access_probe_path(folder: &Path) -> PathBuf {
    folder.join(format!(".filegrove-access-{}.tmp", Uuid::new_v4()))
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
