use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

use crate::domain::{DestinationMode, Folder, Rule};
use crate::engine::cancellation::{CancellationRegistry, CancellationToken};
use crate::engine::matcher::FileItem;
use crate::engine::planner::{compute_organize_plan, PlannedAction};
use crate::engine::scanner::{scan as scan_files, ScanOptions, ScanOutput};
use crate::storage::Database;
use crate::AppState;

struct ConfiguredScan {
    output: ScanOutput,
    folder: Option<Folder>,
    options: ScanOptions,
}

/// Single-level scan using the saved folder exclusions.
#[tauri::command]
pub async fn scan(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(scan_response(
        configured_scan(Some(&app), &state.db, &folder_path, false, None).await,
    ))
}

/// Recursive scan using `subfolders.maxDepth`, folder exclusions and the
/// global 100,000-file safety cap.
#[tauri::command]
pub async fn scan_recursive(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(scan_response(
        configured_scan(Some(&app), &state.db, &folder_path, true, None).await,
    ))
}

fn scan_response(result: Result<ConfiguredScan, String>) -> Value {
    match result {
        Ok(result) => json!({
            "success": true,
            "data": {
                "files": result.output.files,
                "stats": result.output.stats,
            }
        }),
        Err(error) => json!({ "success": false, "error": error }),
    }
}

async fn configured_scan(
    app: Option<&AppHandle>,
    db: &Database,
    folder_path: &str,
    recursive_requested: bool,
    cancellation: Option<&CancellationToken>,
) -> Result<ConfiguredScan, String> {
    let folder = find_folder_by_path(db, Path::new(folder_path))?;
    let mut options = ScanOptions::for_folder(folder.as_ref(), recursive_requested);
    if let Some(folder) = folder.as_ref() {
        let rules = db
            .rules()
            .get_by_ids(&folder.rule_ids)
            .map_err(|error| error.to_string())?;
        options.exclude_managed_destinations(&rules);
    }
    let root = PathBuf::from(folder_path);
    let cancellation = cancellation.cloned();
    let progress_app = app.cloned();
    let progress_path = folder_path.to_string();
    let options_for_scan = options.clone();

    let output = tokio::task::spawn_blocking(move || {
        scan_files(
            &root,
            &options_for_scan,
            || {
                cancellation
                    .as_ref()
                    .is_some_and(CancellationToken::is_cancelled)
            },
            |stats| {
                if let Some(app) = progress_app.as_ref() {
                    let _ = app.emit(
                        "scan:progress",
                        json!({ "folderPath": progress_path, "stats": stats }),
                    );
                }
            },
        )
    })
    .await
    .map_err(|error| format!("SCAN_TASK_FAILED: {error}"))?
    .map_err(|error| error.to_string())?;

    Ok(ConfiguredScan {
        output,
        folder,
        options,
    })
}

fn find_folder_by_path(db: &Database, requested: &Path) -> Result<Option<Folder>, String> {
    let requested_string = requested.to_string_lossy();
    if let Some(folder) = db
        .folders()
        .get_by_path(&requested_string)
        .map_err(|error| error.to_string())?
    {
        return Ok(Some(folder));
    }

    let requested_canonical = std::fs::canonicalize(requested).ok();
    if requested_canonical.is_none() {
        return Ok(None);
    }
    db.folders()
        .list()
        .map_err(|error| error.to_string())
        .map(|folders| {
            folders
                .into_iter()
                .find(|folder| std::fs::canonicalize(&folder.path).ok() == requested_canonical)
        })
}

/// Preview: compute the organize plan without executing it.
#[tauri::command]
pub async fn preview(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(preview_inner(&app, &state.db, &folder_path, false).await)
}

#[tauri::command]
pub async fn preview_recursive(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(preview_inner(&app, &state.db, &folder_path, true).await)
}

async fn preview_inner(
    app: &AppHandle,
    db: &Database,
    folder_path: &str,
    recursive: bool,
) -> Value {
    let folder = match find_folder_by_path(db, Path::new(folder_path)) {
        Ok(Some(folder)) => folder,
        Ok(None) => return json!({ "success": false, "error": "FOLDER_NOT_FOUND" }),
        Err(error) => return json!({ "success": false, "error": error }),
    };
    let rules = match load_assigned_rules(db, &folder) {
        Ok(rules) => rules,
        Err(error) => return json!({ "success": false, "error": error }),
    };
    let scan = match configured_scan(Some(app), db, folder_path, recursive, None).await {
        Ok(scan) => scan,
        Err(error) => return json!({ "success": false, "error": error }),
    };
    let plan = compute_organize_plan(scan.output.files, rules);
    json!({
        "success": true,
        "data": { "plan": plan, "stats": scan.output.stats }
    })
}

/// Organize: scan + execute actions. Returns a session ID for later undo.
#[tauri::command]
pub async fn organize(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
) -> Result<Value, String> {
    Ok(organize_inner(&app, &state.db, &state.cancellations, &folder_path, false).await)
}

#[tauri::command]
pub async fn organize_recursive(
    app: AppHandle,
    state: State<'_, AppState>,
    folder_path: String,
    opts: Option<serde_json::Value>,
) -> Result<Value, String> {
    // The persisted folder configuration is authoritative. Keeping `opts` in
    // the command signature preserves renderer compatibility.
    drop(opts);
    Ok(organize_inner(&app, &state.db, &state.cancellations, &folder_path, true).await)
}

pub async fn organize_inner(
    app: &AppHandle,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    recursive: bool,
) -> Value {
    organize_inner_with_gate(app, db, cancellations, folder_path, recursive, None).await
}

/// Background-only entry point. The pause flag is checked before and after
/// session registration: if pause races the first check, either the second
/// check rejects the run or the pause operation can see and cancel its session.
pub async fn organize_automated_inner(
    app: &AppHandle,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    recursive: bool,
    automation_paused: &AtomicBool,
) -> Value {
    organize_inner_with_gate(
        app,
        db,
        cancellations,
        folder_path,
        recursive,
        Some(automation_paused),
    )
    .await
}

async fn organize_inner_with_gate(
    app: &AppHandle,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    recursive: bool,
    automation_paused: Option<&AtomicBool>,
) -> Value {
    if is_automation_paused(automation_paused) {
        return automation_paused_response();
    }
    let folder = match find_folder_by_path(db, Path::new(folder_path)) {
        Ok(Some(folder)) => folder,
        Ok(None) => return json!({ "success": false, "error": "FOLDER_NOT_FOUND" }),
        Err(error) => return json!({ "success": false, "error": error }),
    };
    if folder.recursive_organization_requires_confirmation(recursive) {
        return json!({
            "success": false,
            "error": "FIRST_RUN_CONFIRMATION_REQUIRED"
        });
    }
    let rules = match load_assigned_rules(db, &folder) {
        Ok(rules) => rules,
        Err(error) => return json!({ "success": false, "error": error }),
    };
    let session_id = Uuid::new_v4();
    let folder_key = canonical_folder_key(Path::new(folder_path));
    let Some(cancellation) = cancellations.begin_for_folder(session_id, folder_key) else {
        return json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" });
    };
    let _session_guard = SessionGuard {
        registry: cancellations,
        session_id,
    };
    if is_automation_paused(automation_paused) {
        return automation_paused_response();
    }

    let _ = app.emit(
        "organize:started",
        json!({ "sessionId": session_id, "folderPath": folder_path }),
    );

    let scan =
        match configured_scan(Some(app), db, folder_path, recursive, Some(&cancellation)).await {
            Ok(scan) => scan,
            Err(error) => {
                emit_organize_error(app, session_id, folder_path, &error);
                return json!({ "success": false, "error": error });
            }
        };
    if scan.output.stats.truncated {
        let error = "SCAN_LIMIT_REACHED";
        emit_organize_error(app, session_id, folder_path, error);
        return json!({
            "success": false,
            "error": error,
            "data": { "stats": scan.output.stats }
        });
    }
    if scan.output.stats.errors > 0 {
        let error = "SCAN_INCOMPLETE";
        emit_organize_error(app, session_id, folder_path, error);
        return json!({
            "success": false,
            "error": error,
            "data": { "stats": scan.output.stats }
        });
    }
    if cancellation.is_cancelled() {
        let total = scan.output.files.len() as u32;
        let _ = app.emit(
            "organize:done",
            json!({
                "sessionId": session_id,
                "folderPath": folder_path,
                "cancelled": true,
                "stats": { "done": 0, "failed": 0, "total": total }
            }),
        );
        return json!({
            "success": true,
            "data": {
                "sessionId": session_id,
                "cancelled": true,
                "undoCount": 0,
                "stats": { "done": 0, "failed": 0, "total": total }
            }
        });
    }

    let plan = compute_organize_plan(scan.output.files, rules);
    let preserve_relative_structure = scan.options.recursive
        && scan
            .folder
            .as_ref()
            .is_some_and(|folder| folder.subfolders.destination_mode == DestinationMode::Preserve);
    execute_plan(
        app,
        db,
        &cancellation,
        session_id,
        folder_path,
        folder.id,
        plan,
        preserve_relative_structure,
    )
    .await
}

/// Organize only the root-level files reported by the live watcher. A normal
/// filesystem event must never trigger a full rescan of the watched folder;
/// full scans are reserved for explicit start/resume catch-up and manual or
/// automatic runs.
pub async fn organize_automated_paths_inner(
    app: &AppHandle,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    paths: Vec<PathBuf>,
    automation_paused: &AtomicBool,
) -> Value {
    organize_paths_inner_with_gate(
        app,
        db,
        cancellations,
        folder_path,
        paths,
        Some(automation_paused),
    )
    .await
}

async fn organize_paths_inner_with_gate(
    app: &AppHandle,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    paths: Vec<PathBuf>,
    automation_paused: Option<&AtomicBool>,
) -> Value {
    if is_automation_paused(automation_paused) {
        return automation_paused_response();
    }
    let folder = match find_folder_by_path(db, Path::new(folder_path)) {
        Ok(Some(folder)) => folder,
        Ok(None) => return json!({ "success": false, "error": "FOLDER_NOT_FOUND" }),
        Err(error) => return json!({ "success": false, "error": error }),
    };
    if folder.recursive_organization_requires_confirmation(folder.subfolders.enabled) {
        return json!({
            "success": false,
            "error": "FIRST_RUN_CONFIRMATION_REQUIRED"
        });
    }
    let rules = match load_assigned_rules(db, &folder) {
        Ok(rules) => rules,
        Err(error) => return json!({ "success": false, "error": error }),
    };
    let session_id = Uuid::new_v4();
    let folder_key = canonical_folder_key(Path::new(folder_path));
    let Some(cancellation) = cancellations.begin_for_folder(session_id, folder_key) else {
        return json!({ "success": false, "error": "ORGANIZE_ALREADY_RUNNING" });
    };
    let _session_guard = SessionGuard {
        registry: cancellations,
        session_id,
    };
    if is_automation_paused(automation_paused) {
        return automation_paused_response();
    }
    let _ = app.emit(
        "organize:started",
        json!({ "sessionId": session_id, "folderPath": folder_path }),
    );

    let root = match crate::paths::safety::validate_scan_root(Path::new(folder_path)) {
        Ok(root) => root,
        Err(error) => {
            emit_organize_error(app, session_id, folder_path, &error.to_string());
            return json!({ "success": false, "error": error.to_string() });
        }
    };
    let mut exclude_folders = folder.exclude_folders.clone();
    for rule in &rules {
        let crate::domain::Action::Move { destination } = &rule.action else {
            continue;
        };
        let Ok(Some(managed_root)) = crate::paths::safety::relative_destination_root(destination)
        else {
            continue;
        };
        if !exclude_folders
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&managed_root))
        {
            exclude_folders.push(managed_root);
        }
    }
    let filter = match crate::watcher::filters::PathFilter::new(
        &root,
        &exclude_folders,
        &folder.exclude_patterns,
    ) {
        Ok(filter) => filter,
        Err(error) => {
            emit_organize_error(app, session_id, folder_path, &error.to_string());
            return json!({ "success": false, "error": error.to_string() });
        }
    };
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for path in paths {
        if cancellation.is_cancelled() || filter.is_ignored(&path) {
            continue;
        }
        let source = match crate::paths::safety::validate_source_file(&root, &path) {
            Ok(source) => source,
            Err(_) => continue,
        };
        let subfolder_depth = source
            .strip_prefix(&root)
            .map(|relative| relative.components().count().saturating_sub(1))
            .unwrap_or(usize::MAX);
        if (!folder.subfolders.enabled && subfolder_depth > 0)
            || !folder
                .subfolders
                .max_depth
                .includes(u32::try_from(subfolder_depth).unwrap_or(u32::MAX))
        {
            continue;
        }
        let key = crate::paths::safety::canonical_path_key(&source);
        if !seen.insert(key) {
            continue;
        }
        if let Some(source) = source.to_str() {
            if let Ok(file) = FileItem::from_path_sync(source) {
                files.push(file);
            }
        }
    }
    let plan = compute_organize_plan(files, rules);
    execute_plan(
        app,
        db,
        &cancellation,
        session_id,
        folder_path,
        folder.id,
        plan,
        folder.subfolders.enabled
            && folder.subfolders.destination_mode == DestinationMode::Preserve,
    )
    .await
}

fn is_automation_paused(flag: Option<&AtomicBool>) -> bool {
    flag.is_some_and(|paused| paused.load(Ordering::Acquire))
}

fn automation_paused_response() -> Value {
    json!({
        "success": true,
        "data": {
            "cancelled": true,
            "paused": true,
            "undoCount": 0,
            "stats": { "done": 0, "failed": 0, "total": 0 }
        }
    })
}

#[allow(clippy::too_many_arguments)]
async fn execute_plan(
    app: &AppHandle,
    db: &Database,
    cancellation: &CancellationToken,
    session_id: Uuid,
    folder_path: &str,
    folder_id: Uuid,
    plan: Vec<PlannedAction>,
    preserve_relative_structure: bool,
) -> Value {
    let conflict_behavior = crate::engine::executor::ConflictBehavior::from_str(
        &db.settings().get_conflict_behavior_str(),
    );
    let duplicate_behavior = crate::engine::file_ops::DuplicateBehavior::from_str(
        &db.settings().get_duplicate_behavior_str(),
    );
    let scan_root = PathBuf::from(folder_path);
    let mut done = 0u32;
    let mut failed = 0u32;
    let total = plan.len() as u32;
    let mut processed = 0u32;
    let mut cancelled = cancellation.is_cancelled();
    let mut last_progress = Instant::now();

    for action in &plan {
        if cancellation.is_cancelled() {
            cancelled = true;
            break;
        }
        if let Some(ref planned_action) = action.action {
            let organizer = crate::engine::executor::Organizer {
                db,
                session_id,
                scan_root: scan_root.clone(),
                conflict_behavior,
                duplicate_behavior,
                preserve_relative_structure,
                cancellation: Some(cancellation.clone()),
            };
            let result = organizer
                .execute(
                    &action.file,
                    planned_action,
                    action.rule_id,
                    action.rule_name.as_deref(),
                )
                .await;
            if result.success && !result.skipped {
                done += 1;
            } else if !result.skipped {
                failed += 1;
            }
        }
        processed += 1;
        if processed == total
            || processed % 500 == 0
            || last_progress.elapsed() >= Duration::from_millis(250)
        {
            let _ = app.emit(
                "organize:progress",
                json!({
                    "sessionId": session_id,
                    "folderPath": folder_path,
                    "done": done,
                    "failed": failed,
                    "processed": processed,
                    "total": total,
                }),
            );
            last_progress = Instant::now();
        }
    }
    // Cancellation may arrive while the final action is running (notably a
    // shell command). There would be no next loop iteration to observe it, so
    // refresh the flag before publishing the session result.
    cancelled |= cancellation.is_cancelled();

    let _ = app.emit(
        "organize:done",
        json!({
            "sessionId": session_id,
            "folderPath": folder_path,
            "cancelled": cancelled,
            "stats": { "done": done, "failed": failed, "total": total }
        }),
    );
    if done + failed > 0 {
        let _ = app.emit(
            "activity:changed",
            json!({ "kind": "organize", "folderId": folder_id }),
        );
        notify_background_completion(app, db, done, failed);
    }

    let undo_count = db
        .undo()
        .stats(session_id)
        .map(|stats| stats.entry_count as u32)
        .unwrap_or(0);
    json!({
        "success": true,
        "data": {
            "sessionId": session_id,
            "cancelled": cancelled,
            "undoCount": undo_count,
            "stats": { "done": done, "failed": failed, "total": total }
        }
    })
}

fn notify_background_completion(app: &AppHandle, db: &Database, done: u32, failed: u32) {
    if !db.settings().get_notifications() {
        return;
    }
    if !app_is_in_background(app) {
        return;
    }

    let language = db.settings().get_language();
    let body = if language == "es" {
        if failed == 0 {
            format!("Se organizaron {done} archivo(s).")
        } else {
            format!("Se organizaron {done} archivo(s); {failed} terminaron con error.")
        }
    } else if failed == 0 {
        format!("Organized {done} file(s).")
    } else {
        format!("Organized {done} file(s); {failed} failed.")
    };
    if let Err(error) = app
        .notification()
        .builder()
        .title("FileGrove")
        .body(body)
        .show()
    {
        tracing::warn!("could not show completion notification: {error}");
    }
}

pub fn report_background_error(
    app: &AppHandle,
    db: &Database,
    folder_path: &str,
    trigger: &str,
    error: &str,
) {
    let _ = app.emit(
        "background:error",
        json!({
            "folderPath": folder_path,
            "trigger": trigger,
            "error": error,
        }),
    );
    if !db.settings().get_notifications() || !app_is_in_background(app) {
        return;
    }
    let body = if db.settings().get_language() == "es" {
        format!("No se pudo organizar una carpeta: {error}")
    } else {
        format!("A folder could not be organized: {error}")
    };
    if let Err(notification_error) = app
        .notification()
        .builder()
        .title("FileGrove")
        .body(body)
        .show()
    {
        tracing::warn!("could not show failure notification: {notification_error}");
    }
}

fn app_is_in_background(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .map(|window| {
            !window.is_visible().unwrap_or(false) || window.is_minimized().unwrap_or(false)
        })
        .unwrap_or(true)
}

fn load_assigned_rules(db: &Database, folder: &Folder) -> Result<Vec<Rule>, String> {
    if folder.rule_ids.is_empty() {
        return Err("NO_RULES_ASSIGNED".into());
    }
    let mut rules = db
        .rules()
        .get_by_ids(&folder.rule_ids)
        .map_err(|error| error.to_string())?;
    rules.retain(Rule::is_actionable);
    if rules.is_empty() {
        Err("NO_RULES_ENABLED".into())
    } else {
        Ok(rules)
    }
}

fn canonical_folder_key(path: &Path) -> String {
    crate::paths::safety::canonical_path_key(path)
}

fn emit_organize_error(app: &AppHandle, session_id: Uuid, folder_path: &str, error: &str) {
    let _ = app.emit(
        "organize:done",
        json!({
            "sessionId": session_id,
            "folderPath": folder_path,
            "cancelled": false,
            "error": error,
            "stats": { "done": 0, "failed": 0, "total": 0 }
        }),
    );
}

struct SessionGuard<'a> {
    registry: &'a CancellationRegistry,
    session_id: Uuid,
}

impl Drop for SessionGuard<'_> {
    fn drop(&mut self) {
        self.registry.finish(self.session_id);
    }
}
