use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tauri_plugin_autostart::ManagerExt;

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
    app: AppHandle,
    state: State<'_, AppState>,
    settings: serde_json::Map<String, serde_json::Value>,
) -> Result<Value, String> {
    if let Err(error) = validate_settings(&settings) {
        return Ok(json!({ "success": false, "error": error }));
    }
    if let Some(enabled) = settings.get("launchAtStartup").and_then(Value::as_bool) {
        let result = if enabled {
            app.autolaunch().enable()
        } else {
            app.autolaunch().disable()
        };
        if let Err(error) = result {
            return Ok(json!({ "success": false, "error": error.to_string() }));
        }
    }
    if let Some(visible) = settings.get("showTrayIcon").and_then(Value::as_bool) {
        if let Some(tray) = state.tray.read().as_ref() {
            if let Err(error) = tray.set_visible(visible) {
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }
        }
    }
    match state.db.settings().set_many(&settings) {
        Ok(_) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

fn validate_settings(settings: &serde_json::Map<String, Value>) -> Result<(), &'static str> {
    const BOOLEAN_KEYS: &[&str] = &[
        "launchAtStartup",
        "minimizeToTray",
        "showTrayIcon",
        "notifications",
        "automationPaused",
        "onboardingCompleted",
        "firstFolderHintShown",
        "subfolderExplainerShown",
        "commandWarningShown",
    ];
    for (key, value) in settings {
        match key.as_str() {
            key if BOOLEAN_KEYS.contains(&key) && value.is_boolean() => {}
            "theme" if matches!(value.as_str(), Some("dark" | "light" | "system")) => {}
            "language" if matches!(value.as_str(), Some("en" | "es")) => {}
            "conflictBehavior"
                if matches!(value.as_str(), Some("rename" | "overwrite" | "skip")) => {}
            "rulesSortOrder" | "foldersSortOrder"
                if matches!(value.as_str(), Some("created" | "name")) => {}
            "subfolderDefaults.maxDepth"
                if value
                    .as_u64()
                    .is_some_and(|depth| (1..=64).contains(&depth)) => {}
            _ => return Err("INVALID_SETTINGS"),
        }
    }
    Ok(())
}
