use std::path::Path;

use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::engine::command::execute_shell_command_cancellable;
use crate::engine::matcher::FileItem;
use crate::AppState;

#[tauri::command]
pub async fn test_command(
    app: AppHandle,
    command: String,
    timeout_ms: u32,
) -> Result<Value, String> {
    let command = command.trim().to_string();
    if command.is_empty() {
        return Ok(json!({ "success": false, "error": "COMMAND_INVALID" }));
    }
    let Some(path) = pick_file(&app).await else {
        return Ok(json!({ "success": false, "error": "USER_CANCELED_FILE_SELECT" }));
    };
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
        _ => return Ok(json!({ "success": false, "error": "SOURCE_FILE_INVALID" })),
    }
    let Some(path_string) = path.to_str() else {
        return Ok(json!({ "success": false, "error": "SOURCE_FILE_INVALID" }));
    };
    let file = match FileItem::from_path_sync(path_string) {
        Ok(file) => file,
        Err(error) => {
            return Ok(json!({
                "success": false,
                "error": format!("SOURCE_FILE_INVALID: {error}")
            }))
        }
    };
    let execution = execute_shell_command_cancellable(&command, &file, timeout_ms, None).await;
    Ok(json!({
        "success": true,
        "data": {
            "filePath": file.path,
            "exitCode": execution.exit_code,
            "output": execution.output,
            "durationMs": execution.duration_ms,
            "timedOut": execution.timed_out,
            "success": execution.success,
            "error": execution.error,
        }
    }))
}

#[tauri::command]
pub async fn allow_interrupted_retry(
    state: State<'_, AppState>,
    rule_id: Uuid,
    file_path: String,
    command: String,
) -> Result<Value, String> {
    match state
        .db
        .command_ledger()
        .allow_interrupted_retry(rule_id, &file_path, &command)
    {
        Ok(true) => Ok(json!({ "success": true })),
        Ok(false) => Ok(json!({ "success": false, "error": "COMMAND_RETRY_NOT_PENDING" })),
        Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
    }
}

async fn pick_file(app: &AppHandle) -> Option<std::path::PathBuf> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Select a file to test")
        .pick_file(move |selection| {
            let path = selection.and_then(|selection| selection.as_path().map(Path::to_path_buf));
            let _ = sender.send(path);
        });
    receiver.await.ok().flatten()
}
