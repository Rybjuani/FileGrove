use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::domain::{Folder, FolderMode, MaxDepth, SubfolderConfig, WatcherStatus};
use crate::AppState;

static FOLDER_CATALOG_LOCK: once_cell::sync::Lazy<parking_lot::Mutex<()>> =
    once_cell::sync::Lazy::new(|| parking_lot::Mutex::new(()));

#[tauri::command]
pub async fn list_folders(state: State<'_, AppState>) -> Result<Value, String> {
    match state.db.folders().list() {
        Ok(folders) => Ok(json!({ "success": true, "data": folders })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn add(app: AppHandle, state: State<'_, AppState>) -> Result<Value, String> {
    let selected_path = match pick_folder(&app).await {
        Some(p) => p,
        None => return Ok(json!({ "success": false, "error": "USER_CANCELED_FOLDER_SELECT" })),
    };
    let canonical_path = match crate::paths::safety::validate_scan_root(Path::new(&selected_path)) {
        Ok(path) => path,
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    if let Err(error) = crate::ipc::permissions::check_read_write_access(&canonical_path) {
        return Ok(json!({ "success": false, "error": error }));
    }
    let path = canonical_path.to_string_lossy().into_owned();
    // Serialize overlap validation with insertion. Otherwise two concurrent
    // folder pickers can both pass the check and create overlapping roots.
    let _catalog_guard = FOLDER_CATALOG_LOCK.lock();

    let name = PathBuf::from(&path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Folder")
        .to_string();

    match state.db.folders().get_by_path(&path) {
        Ok(Some(folder)) => return Ok(json!({ "success": true, "data": folder })),
        Ok(None) => match state.db.folders().list() {
            Ok(folders) => {
                for folder in folders {
                    let Ok(existing) =
                        crate::paths::safety::validate_scan_root(Path::new(&folder.path))
                    else {
                        continue;
                    };
                    if existing == canonical_path {
                        return Ok(json!({ "success": true, "data": folder }));
                    }
                    // Nested watched roots could process the same destructive
                    // action concurrently under two different folder locks.
                    if crate::paths::safety::paths_overlap(&existing, &canonical_path) {
                        return Ok(json!({
                            "success": false,
                            "error": "WATCHED_FOLDER_OVERLAP"
                        }));
                    }
                }
            }
            Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
        },
        Err(e) => return Ok(json!({ "success": false, "error": e.to_string() })),
    }

    let folder = Folder {
        id: Uuid::new_v4(),
        path: path.clone(),
        name,
        mode: FolderMode::Single,
        rule_ids: vec![],
        watcher_status: WatcherStatus::Stopped,
        created_at: chrono::Utc::now(),
        subfolders: SubfolderConfig::default(),
        exclude_folders: vec![],
        exclude_patterns: vec![],
    };

    if let Err(e) = state.db.folders().insert(&folder) {
        return Ok(json!({ "success": false, "error": e.to_string() }));
    }

    Ok(json!({ "success": true, "data": folder }))
}

#[tauri::command]
pub async fn remove(app: AppHandle, state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    let folder = match state.db.folders().get(id) {
        Ok(Some(folder)) => folder,
        Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
        return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
    };
    if let Err(error) = state.watcher.stop_watching(&app, id) {
        return Ok(json!({ "success": false, "error": error.to_string() }));
    }
    match state.db.folders().delete(id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn update_name(
    state: State<'_, AppState>,
    id: Uuid,
    name: String,
) -> Result<Value, String> {
    let name = name.trim().to_string();
    if name.is_empty() || name.chars().count() > 100 {
        return Ok(json!({ "success": false, "error": "INVALID_FOLDER_NAME" }));
    }
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            folder = match state.db.folders().get(id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            folder.name = name;
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn update_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    mode: String,
    disable_subfolders: Option<bool>,
) -> Result<Value, String> {
    let new_mode = match FolderMode::from_str(&mode) {
        Some(m) => m,
        None => return Ok(json!({ "success": false, "error": "INVALID_MODE" })),
    };
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            // The folder may have changed between the first read and lease
            // acquisition. Work from the current persisted record.
            folder = match state.db.folders().get(id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            if new_mode == FolderMode::Auto
                && folder.recursive_organization_requires_confirmation(true)
            {
                return Ok(json!({
                    "success": false,
                    "error": "FIRST_RUN_CONFIRMATION_REQUIRED"
                }));
            }
            // Kept in the command signature for compatibility with older
            // renderers. Automatic mode now supports confirmed subfolders.
            let _ = disable_subfolders;
            let previous_folder = folder.clone();
            folder.mode = new_mode;
            if let Err(error) = state.db.folders().update(&folder) {
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }

            let reconcile_result = match new_mode {
                FolderMode::Single => state.watcher.stop_watching(&app, id),
                FolderMode::Auto => state.watcher.start_watching_and_catch_up(&app, &folder),
            };
            if let Err(error) = reconcile_result {
                let _ = state.db.folders().update(&previous_folder);
                match previous_folder.mode {
                    FolderMode::Single => {
                        let _ = state.watcher.stop_watching(&app, id);
                    }
                    FolderMode::Auto => {
                        let _ = state
                            .watcher
                            .start_watching_and_catch_up(&app, &previous_folder);
                    }
                }
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }

            match state.db.folders().get(id) {
                Ok(Some(updated)) => Ok(json!({ "success": true, "data": updated })),
                Ok(None) => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

/// Save every advanced folder field in one database write. Rule assignment,
/// recursion and exclusions must never become partially applied because a
/// later renderer request failed.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn update_configuration(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    mut config: SubfolderConfig,
    exclude_folders: Vec<String>,
    exclude_patterns: Vec<String>,
    rule_ids: Vec<Uuid>,
    confirm_recursive: Option<bool>,
) -> Result<Value, String> {
    if matches!(config.max_depth, MaxDepth::Level(0) | MaxDepth::Level(65..)) {
        return Ok(json!({ "success": false, "error": "INVALID_MAX_DEPTH" }));
    }
    let exclude_folders = match normalize_exclude_folders(exclude_folders) {
        Ok(values) => values,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    let exclude_patterns = match normalize_exclude_patterns(exclude_patterns) {
        Ok(values) => values,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    if rule_ids.len() > 100 {
        return Ok(json!({ "success": false, "error": "TOO_MANY_RULES" }));
    }
    let mut unique_rule_ids = Vec::new();
    for rule_id in rule_ids {
        if !unique_rule_ids.contains(&rule_id) {
            unique_rule_ids.push(rule_id);
        }
    }
    let persisted_rules = match state.db.rules().get_by_ids(&unique_rule_ids) {
        Ok(rules) => rules,
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    if persisted_rules.len() != unique_rule_ids.len() {
        return Ok(json!({ "success": false, "error": "RULE_NOT_FOUND" }));
    }

    let mut folder = match state.db.folders().get(id) {
        Ok(Some(folder)) => folder,
        Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
        return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
    };
    folder = match state.db.folders().get(id) {
        Ok(Some(folder)) => folder,
        Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    if let Err(error) = crate::watcher::filters::PathFilter::new(
        Path::new(&folder.path),
        &exclude_folders,
        &exclude_patterns,
    ) {
        return Ok(json!({ "success": false, "error": error.to_string() }));
    }

    let previous = folder.clone();
    let previous_status = state.watcher.status(id);
    folder.reconcile_recursive_confirmation(&mut config, &exclude_folders, &exclude_patterns);
    if folder.mode == FolderMode::Auto && config.enabled && !config.first_run_confirmed {
        if !confirm_recursive.unwrap_or(false) {
            return Ok(json!({
                "success": false,
                "error": "FIRST_RUN_CONFIRMATION_REQUIRED"
            }));
        }
        config.first_run_confirmed = true;
    }
    folder.subfolders = config;
    folder.exclude_folders = exclude_folders;
    folder.exclude_patterns = exclude_patterns;
    folder.rule_ids = unique_rule_ids;
    if let Err(error) = state.db.folders().update(&folder) {
        return Ok(json!({ "success": false, "error": error.to_string() }));
    }

    if previous_status != WatcherStatus::Stopped {
        let restarted = state
            .watcher
            .stop_watching(&app, id)
            .and_then(|()| state.watcher.start_watching(&app, &folder))
            .and_then(|()| {
                if previous_status == WatcherStatus::Paused {
                    state.watcher.pause(&app, id)
                } else {
                    Ok(())
                }
            });
        if let Err(error) = restarted {
            let _ = state.db.folders().update(&previous);
            let _ = state.watcher.stop_watching(&app, id);
            let _ = state.watcher.start_watching(&app, &previous);
            if previous_status == WatcherStatus::Paused {
                let _ = state.watcher.pause(&app, id);
            }
            return Ok(json!({ "success": false, "error": error.to_string() }));
        }
    }

    match state.db.folders().get(id) {
        Ok(Some(updated)) => Ok(json!({ "success": true, "data": updated })),
        Ok(None) => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
    }
}

#[tauri::command]
pub async fn assign_rules(
    state: State<'_, AppState>,
    folder_id: Uuid,
    rule_ids: Vec<Uuid>,
) -> Result<Value, String> {
    if rule_ids.len() > 100 {
        return Ok(json!({ "success": false, "error": "TOO_MANY_RULES" }));
    }
    let mut unique_rule_ids = Vec::new();
    for rule_id in rule_ids {
        if !unique_rule_ids.contains(&rule_id) {
            unique_rule_ids.push(rule_id);
        }
    }
    let existing_rules = match state.db.rules().get_by_ids(&unique_rule_ids) {
        Ok(rules) => rules,
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    if existing_rules.len() != unique_rule_ids.len() {
        return Ok(json!({ "success": false, "error": "RULE_NOT_FOUND" }));
    }
    match state.db.folders().get(folder_id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            folder = match state.db.folders().get(folder_id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            let added_count = unique_rule_ids
                .iter()
                .filter(|rule_id| !folder.rule_ids.contains(rule_id))
                .count();
            if folder.rule_ids.len().saturating_add(added_count) > 100 {
                return Ok(json!({ "success": false, "error": "TOO_MANY_RULES" }));
            }
            for rid in &unique_rule_ids {
                if !folder.rule_ids.contains(rid) {
                    folder.rule_ids.push(*rid);
                }
            }
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn unassign_rules(
    state: State<'_, AppState>,
    folder_id: Uuid,
    rule_ids: Vec<Uuid>,
) -> Result<Value, String> {
    if rule_ids.len() > 100 {
        return Ok(json!({ "success": false, "error": "TOO_MANY_RULES" }));
    }
    match state.db.folders().get(folder_id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            folder = match state.db.folders().get(folder_id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            folder.rule_ids.retain(|r| !rule_ids.contains(r));
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn apply_recommended_rules(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_id: Uuid,
) -> Result<Value, String> {
    let folder = match state.db.folders().get(folder_id) {
        Ok(Some(folder)) => folder,
        Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };
    let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
        return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
    };
    let previous_status = state.watcher.status(folder_id);
    let language = state.db.settings().get_language();
    let updated = match state
        .db
        .folders()
        .apply_recommended_rules(folder_id, &language)
    {
        Ok(folder) => folder,
        Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
    };

    if previous_status != WatcherStatus::Stopped {
        let restarted = state
            .watcher
            .stop_watching(&app, folder_id)
            .and_then(|()| state.watcher.start_watching(&app, &updated))
            .and_then(|()| {
                if previous_status == WatcherStatus::Paused {
                    state.watcher.pause(&app, folder_id)
                } else {
                    Ok(())
                }
            });
        if let Err(error) = restarted {
            let _ = state.db.folders().update(&folder);
            let _ = state.watcher.stop_watching(&app, folder_id);
            let _ = state.watcher.start_watching(&app, &folder);
            if previous_status == WatcherStatus::Paused {
                let _ = state.watcher.pause(&app, folder_id);
            }
            return Ok(json!({ "success": false, "error": error.to_string() }));
        }
    }

    Ok(json!({ "success": true, "data": updated }))
}

#[tauri::command]
pub async fn update_subfolders_config(
    state: State<'_, AppState>,
    id: Uuid,
    mut config: SubfolderConfig,
) -> Result<Value, String> {
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            folder = match state.db.folders().get(id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            if matches!(config.max_depth, MaxDepth::Level(0) | MaxDepth::Level(65..)) {
                return Ok(json!({ "success": false, "error": "INVALID_MAX_DEPTH" }));
            }
            let exclude_folders = folder.exclude_folders.clone();
            let exclude_patterns = folder.exclude_patterns.clone();
            folder.reconcile_recursive_confirmation(
                &mut config,
                &exclude_folders,
                &exclude_patterns,
            );
            if config.enabled && folder.mode == FolderMode::Auto && !config.first_run_confirmed {
                return Ok(json!({
                    "success": false,
                    "error": "FIRST_RUN_CONFIRMATION_REQUIRED"
                }));
            }
            folder.subfolders = config;
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

#[tauri::command]
pub async fn update_exclusions(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Uuid,
    exclude_folders: Vec<String>,
    exclude_patterns: Vec<String>,
) -> Result<Value, String> {
    let exclude_folders = match normalize_exclude_folders(exclude_folders) {
        Ok(values) => values,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    let exclude_patterns = match normalize_exclude_patterns(exclude_patterns) {
        Ok(values) => values,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };

    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            folder = match state.db.folders().get(id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            if let Err(error) = crate::watcher::filters::PathFilter::new(
                Path::new(&folder.path),
                &exclude_folders,
                &exclude_patterns,
            ) {
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }
            let previous = folder.clone();
            let previous_status = state.watcher.status(id);
            if !folder.subfolders.enabled
                || folder.exclude_folders != exclude_folders
                || folder.exclude_patterns != exclude_patterns
            {
                folder.subfolders.first_run_confirmed = false;
            }
            folder.exclude_folders = exclude_folders;
            folder.exclude_patterns = exclude_patterns;
            if let Err(error) = state.db.folders().update(&folder) {
                return Ok(json!({ "success": false, "error": error.to_string() }));
            }

            if previous_status != WatcherStatus::Stopped {
                let restarted = state
                    .watcher
                    .stop_watching(&app, id)
                    .and_then(|()| state.watcher.start_watching(&app, &folder))
                    .and_then(|()| {
                        if previous_status == WatcherStatus::Paused {
                            state.watcher.pause(&app, id)
                        } else {
                            Ok(())
                        }
                    });
                if let Err(error) = restarted {
                    let _ = state.db.folders().update(&previous);
                    let _ = state.watcher.stop_watching(&app, id);
                    let _ = state.watcher.start_watching(&app, &previous);
                    if previous_status == WatcherStatus::Paused {
                        let _ = state.watcher.pause(&app, id);
                    }
                    return Ok(json!({ "success": false, "error": error.to_string() }));
                }
            }

            match state.db.folders().get(id) {
                Ok(Some(folder)) => Ok(json!({ "success": true, "data": folder })),
                Ok(None) => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
            }
        }
        Ok(None) => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
        Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
    }
}

#[tauri::command]
pub async fn select_destination(app: AppHandle) -> Result<Value, String> {
    match pick_folder(&app).await {
        Some(p) => Ok(json!({ "success": true, "data": p })),
        None => Ok(json!({ "success": false, "error": "USER_CANCELED_FOLDER_SELECT" })),
    }
}

#[tauri::command]
pub async fn confirm_first_run(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    match state.db.folders().get(id) {
        Ok(Some(mut folder)) => {
            let Some(_operation_guard) = acquire_folder_operation(&state, &folder) else {
                return Ok(json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" }));
            };
            folder = match state.db.folders().get(id) {
                Ok(Some(folder)) => folder,
                Ok(None) => return Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
                Err(error) => return Ok(json!({ "success": false, "error": error.to_string() })),
            };
            if !folder.subfolders.enabled {
                return Ok(json!({
                    "success": false,
                    "error": "RECURSIVE_SCAN_DISABLED"
                }));
            }
            folder.subfolders.first_run_confirmed = true;
            match state.db.folders().update(&folder) {
                Ok(()) => Ok(json!({ "success": true, "data": folder })),
                Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
            }
        }
        _ => Ok(json!({ "success": false, "error": "FOLDER_NOT_FOUND" })),
    }
}

async fn pick_folder(app: &AppHandle) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Select folder")
        .pick_folder(move |path| {
            let p = path.and_then(|p| p.as_path().map(|p| p.to_string_lossy().to_string()));
            let _ = tx.send(p);
        });
    rx.await.ok().flatten()
}

fn normalize_exclude_folders(values: Vec<String>) -> Result<Vec<String>, &'static str> {
    if values.len() > 100 {
        return Err("TOO_MANY_EXCLUSIONS");
    }
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().replace('\\', "/");
        if value.is_empty() {
            continue;
        }
        let path = std::path::Path::new(&value);
        if path.is_absolute()
            || path
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err("INVALID_EXCLUDE_FOLDER");
        }
        if !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn normalize_exclude_patterns(values: Vec<String>) -> Result<Vec<String>, &'static str> {
    if values.len() > 100 {
        return Err("TOO_MANY_EXCLUSIONS");
    }
    let mut normalized = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if value.is_empty() {
            continue;
        }
        if value.chars().count() > 256 || glob::Pattern::new(&value).is_err() {
            return Err("INVALID_EXCLUDE_PATTERN");
        }
        if !normalized.contains(&value) {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

fn acquire_folder_operation(
    state: &State<'_, AppState>,
    folder: &Folder,
) -> Option<FolderOperationGuard> {
    let operation_id = Uuid::new_v4();
    let folder_key = crate::paths::safety::canonical_path_key(Path::new(&folder.path));
    state
        .cancellations
        .begin_folder_operation(operation_id, folder_key)
        .then(|| FolderOperationGuard {
            registry: state.cancellations.clone(),
            operation_id,
        })
}

struct FolderOperationGuard {
    registry: Arc<crate::engine::cancellation::CancellationRegistry>,
    operation_id: Uuid,
}

impl Drop for FolderOperationGuard {
    fn drop(&mut self) {
        self.registry.finish(self.operation_id);
    }
}
