//! Watcher service — see SPEC §8.2.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use notify::{EventKind, Watcher};
use notify_debouncer_full::{new_debouncer, DebouncedEvent, FileIdMap};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, Runtime};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::domain::{Action, Folder, FolderMode, WatcherStatus};
use crate::engine::cancellation::CancellationRegistry;
use crate::storage::Database;

use super::filters::PathFilter;
use super::pending::CoalescedPaths;

const WATCHER_SIGNAL_CAPACITY: usize = 1;
const MAX_COALESCED_PATHS: usize = 4_096;

pub struct WatcherService {
    db: Arc<Database>,
    cancellations: Arc<CancellationRegistry>,
    automation_paused: Arc<AtomicBool>,
    /// Active watchers, keyed by folder ID.
    watchers: Mutex<HashMap<Uuid, WatcherHandle>>,
}

struct WatcherHandle {
    /// Drop this to stop the watcher.
    _debouncer: notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        FileIdMap,
    >,
    is_paused: Arc<AtomicBool>,
    is_active: Arc<AtomicBool>,
    pending: Arc<CoalescedPaths>,
    folder_key: String,
}

impl WatcherService {
    pub fn new(db: Arc<Database>, cancellations: Arc<CancellationRegistry>) -> Self {
        let automation_paused = db.settings().get_automation_paused();
        Self {
            db,
            cancellations,
            automation_paused: Arc::new(AtomicBool::new(automation_paused)),
            watchers: Mutex::new(HashMap::new()),
        }
    }

    /// Restore watchers that were running or paused when the app closed.
    pub fn init<R: Runtime>(&self, app: &AppHandle<R>) -> Result<usize> {
        let folders = self.db.folders().list()?;
        let mut restored = 0usize;

        for folder in folders.into_iter().filter(|f| f.mode == FolderMode::Auto) {
            let desired_status = folder.watcher_status;
            if desired_status == WatcherStatus::Stopped {
                continue;
            }

            match self.start_watching(app, &folder) {
                Ok(()) => {
                    if desired_status == WatcherStatus::Paused {
                        self.pause(app, folder.id)?;
                    } else {
                        self.trigger_catch_up(app, &folder);
                    }
                    restored += 1;
                }
                Err(error) => {
                    tracing::warn!(folder_id = ?folder.id, "could not restore watcher: {error}");
                    self.update_status(app, folder.id, WatcherStatus::Stopped);
                }
            }
        }

        tracing::info!("watcher init: restored {restored} folder(s)");
        Ok(restored)
    }

    /// Start watching a folder.
    pub fn start_watching<R: Runtime>(&self, app: &AppHandle<R>, folder: &Folder) -> Result<()> {
        let folder_id = folder.id;
        {
            let map = self.watchers.lock();
            if map.contains_key(&folder_id) {
                return Ok(());
            }
        }

        let watched_path =
            crate::paths::safety::validate_scan_root(PathBuf::from(&folder.path).as_path())
                .map_err(|error| anyhow!(error.to_string()))?;
        let mut exclude_folders = folder.exclude_folders.clone();
        let rules = self.db.rules().get_by_ids(&folder.rule_ids)?;
        for rule in rules {
            let Action::Move { destination } = rule.action else {
                continue;
            };
            if let Ok(Some(root)) = crate::paths::safety::relative_destination_root(&destination) {
                if !exclude_folders
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&root))
                {
                    exclude_folders.push(root);
                }
                continue;
            }
            let dest_path = std::path::Path::new(&destination);
            if dest_path.is_absolute() {
                if let Ok(relative) = dest_path.strip_prefix(&watched_path) {
                    if let Some(std::path::Component::Normal(comp)) = relative.components().next() {
                        if let Some(comp_str) = comp.to_str() {
                            if !exclude_folders
                                .iter()
                                .any(|existing| existing.eq_ignore_ascii_case(comp_str))
                            {
                                exclude_folders.push(comp_str.to_string());
                            }
                        }
                    }
                }
            }
        }
        let path_filter =
            PathFilter::new(&watched_path, &exclude_folders, &folder.exclude_patterns)
                .map_err(|error| anyhow!(error.to_string()))?;

        let globally_paused = self.automation_paused.load(Ordering::Acquire);
        let is_paused = Arc::new(AtomicBool::new(globally_paused));
        let callback_is_paused = is_paused.clone();
        let is_active = Arc::new(AtomicBool::new(true));
        let callback_is_active = is_active.clone();
        let callback_automation_paused = self.automation_paused.clone();
        let pending = Arc::new(CoalescedPaths::new(MAX_COALESCED_PATHS));
        let callback_pending = pending.clone();
        let callback_filter = path_filter.clone();
        // The channel is only a wake-up edge. Paths live in the bounded,
        // deduplicated accumulator, so `try_send` never blocks notify's
        // callback thread and a full signal slot naturally coalesces bursts.
        let (tx, mut rx) = mpsc::channel::<()>(WATCHER_SIGNAL_CAPACITY);
        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |res: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
                if !callback_is_active.load(Ordering::Acquire)
                    || callback_automation_paused.load(Ordering::Acquire)
                {
                    return;
                }
                match res {
                    Ok(events) => {
                        if callback_is_paused.load(Ordering::Acquire) {
                            return;
                        }
                        let candidates = events
                            .iter()
                            .filter(|event| {
                                matches!(&event.kind, EventKind::Create(_) | EventKind::Modify(_))
                            })
                            .flat_map(|event| event.paths.iter())
                            .filter(|path| !callback_filter.is_ignored(path))
                            .cloned();
                        if !callback_pending.try_extend(candidates) {
                            tracing::warn!(
                                "watcher burst exceeded its bounded path buffer; catch-up queued"
                            );
                        }
                    }
                    Err(errors) => {
                        tracing::warn!(?errors, "watcher event stream error; catch-up queued");
                        callback_pending.mark_catch_up_required();
                    }
                }
                let _ = tx.try_send(());
            },
        )?;

        let recursive_mode = if folder.subfolders.enabled {
            notify::RecursiveMode::Recursive
        } else {
            notify::RecursiveMode::NonRecursive
        };
        debouncer.watcher().watch(&watched_path, recursive_mode)?;

        let db = self.db.clone();
        let cancellations = self.cancellations.clone();
        let automation_paused = self.automation_paused.clone();
        let folder_path = folder.path.clone();
        let app_handle = app.clone();
        let task_is_paused = is_paused.clone();
        let task_is_active = is_active.clone();
        let task_pending = pending.clone();

        // `start_watching` is called during Tauri setup as well as from IPC.
        // At setup time there is no Tokio runtime entered on the current
        // thread, so `tokio::spawn` panics as soon as an Automatic folder is
        // restored. Use Tauri's application runtime in both call paths.
        tauri::async_runtime::spawn(async move {
            while rx.recv().await.is_some() {
                if !task_is_active.load(Ordering::Acquire) {
                    break;
                }
                if automation_paused.load(Ordering::Acquire)
                    || task_is_paused.load(Ordering::Acquire)
                {
                    task_pending.clear();
                    continue;
                }
                loop {
                    if automation_paused.load(Ordering::Acquire)
                        || !task_is_active.load(Ordering::Acquire)
                        || task_is_paused.load(Ordering::Acquire)
                    {
                        task_pending.clear();
                        break;
                    }

                    let batch = task_pending.take();
                    if batch.paths.is_empty() && !batch.requires_catch_up {
                        break;
                    }

                    let paths = if batch.requires_catch_up {
                        tracing::info!(
                            ?folder_path,
                            "running watcher catch-up after an event burst or stream error"
                        );
                        None
                    } else {
                        tracing::info!(
                            ?folder_path,
                            count = batch.paths.len(),
                            "organizing changed files"
                        );
                        Some(batch.paths.as_slice())
                    };
                    if let Some(error) = organize_with_retry(
                        &app_handle,
                        &db,
                        &cancellations,
                        &folder_path,
                        paths,
                        WatcherRunGate {
                            automation_paused: &automation_paused,
                            is_active: &task_is_active,
                            is_paused: &task_is_paused,
                        },
                    )
                    .await
                    {
                        tracing::warn!(?folder_path, %error, "auto-organize failed");
                        crate::ipc::files::report_background_error(
                            &app_handle,
                            &db,
                            &folder_path,
                            "watcher",
                            &error,
                        );
                    }
                }
            }
        });

        let status = if globally_paused {
            WatcherStatus::Paused
        } else {
            WatcherStatus::Running
        };
        self.update_status(app, folder_id, status);
        let folder_key = crate::paths::safety::canonical_path_key(&watched_path);
        self.watchers.lock().insert(
            folder_id,
            WatcherHandle {
                _debouncer: debouncer,
                is_paused,
                is_active,
                pending,
                folder_key,
            },
        );
        Ok(())
    }

    pub fn start_watching_and_catch_up<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        folder: &Folder,
    ) -> Result<()> {
        self.start_watching(app, folder)?;
        self.trigger_catch_up(app, folder);
        Ok(())
    }

    pub fn stop_watching<R: Runtime>(&self, app: &AppHandle<R>, folder_id: Uuid) -> Result<()> {
        if let Some(handle) = self.watchers.lock().remove(&folder_id) {
            handle.is_active.store(false, Ordering::Release);
            handle.is_paused.store(true, Ordering::Release);
            handle.pending.clear();
            self.cancellations.cancel_folder(&handle.folder_key);
        } else if let Ok(Some(folder)) = self.db.folders().get(folder_id) {
            let folder_key =
                crate::paths::safety::canonical_path_key(PathBuf::from(folder.path).as_path());
            self.cancellations.cancel_folder(&folder_key);
        }
        self.update_status(app, folder_id, WatcherStatus::Stopped);
        Ok(())
    }

    pub fn pause<R: Runtime>(&self, app: &AppHandle<R>, folder_id: Uuid) -> Result<()> {
        let map = self.watchers.lock();
        let Some(handle) = map.get(&folder_id) else {
            return Err(anyhow!("WATCHER_NOT_RUNNING"));
        };
        handle.is_paused.store(true, Ordering::Release);
        handle.pending.clear();
        let folder_key = handle.folder_key.clone();
        drop(map);
        self.cancellations.cancel_folder(&folder_key);
        self.update_status(app, folder_id, WatcherStatus::Paused);
        Ok(())
    }

    pub fn resume<R: Runtime>(&self, app: &AppHandle<R>, folder_id: Uuid) -> Result<()> {
        let map = self.watchers.lock();
        let Some(handle) = map.get(&folder_id) else {
            return Err(anyhow!("WATCHER_NOT_RUNNING"));
        };
        handle.pending.clear();
        handle.is_paused.store(false, Ordering::Release);
        drop(map);
        self.update_status(app, folder_id, WatcherStatus::Running);
        if let Ok(Some(folder)) = self.db.folders().get(folder_id) {
            self.trigger_catch_up(app, &folder);
        }
        Ok(())
    }

    /// Resume after undo without a catch-up scan. The undo operation already
    /// waited for debounced restore events to be discarded while paused; a
    /// catch-up here would immediately reapply the rule that was just undone.
    pub fn resume_without_catch_up<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        folder_id: Uuid,
    ) -> Result<()> {
        let map = self.watchers.lock();
        let Some(handle) = map.get(&folder_id) else {
            return Err(anyhow!("WATCHER_NOT_RUNNING"));
        };
        handle.pending.clear();
        handle.is_paused.store(false, Ordering::Release);
        drop(map);
        self.update_status(app, folder_id, WatcherStatus::Running);
        Ok(())
    }

    pub fn pause_all<R: Runtime>(&self, app: &AppHandle<R>) {
        self.automation_paused.store(true, Ordering::Release);
        let map = self.watchers.lock();
        let folder_ids: Vec<Uuid> = map.keys().copied().collect();
        let mut folder_keys = Vec::with_capacity(map.len());
        for h in map.values() {
            h.is_paused.store(true, Ordering::Release);
            h.pending.clear();
            folder_keys.push(h.folder_key.clone());
        }
        drop(map);
        for folder_key in folder_keys {
            self.cancellations.cancel_folder(&folder_key);
        }
        for folder_id in folder_ids {
            self.update_status(app, folder_id, WatcherStatus::Paused);
        }
    }

    pub fn resume_all<R: Runtime>(&self, app: &AppHandle<R>) {
        self.automation_paused.store(false, Ordering::Release);
        let map = self.watchers.lock();
        let folder_ids: Vec<Uuid> = map.keys().copied().collect();
        for h in map.values() {
            h.pending.clear();
            h.is_paused.store(false, Ordering::Release);
        }
        drop(map);
        for folder_id in folder_ids {
            self.update_status(app, folder_id, WatcherStatus::Running);
            if let Ok(Some(folder)) = self.db.folders().get(folder_id) {
                self.trigger_catch_up(app, &folder);
            }
        }
    }

    pub fn status(&self, folder_id: Uuid) -> WatcherStatus {
        let map = self.watchers.lock();
        map.get(&folder_id)
            .map(|h| {
                if self.automation_paused.load(Ordering::Acquire)
                    || h.is_paused.load(Ordering::Acquire)
                {
                    WatcherStatus::Paused
                } else {
                    WatcherStatus::Running
                }
            })
            .unwrap_or(WatcherStatus::Stopped)
    }

    /// Shut down every active watcher. Called on app quit.
    pub fn shutdown_all(&self) {
        self.automation_paused.store(true, Ordering::Release);
        let mut map = self.watchers.lock();
        let count = map.len();
        let mut folder_keys = Vec::with_capacity(count);
        for handle in map.values() {
            handle.is_active.store(false, Ordering::Release);
            handle.is_paused.store(true, Ordering::Release);
            handle.pending.clear();
            folder_keys.push(handle.folder_key.clone());
        }
        map.clear();
        drop(map);
        for folder_key in folder_keys {
            self.cancellations.cancel_folder(&folder_key);
        }
        tracing::info!("watcher shutdown: stopped {} watcher(s)", count);
    }

    fn update_status<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        folder_id: Uuid,
        status: WatcherStatus,
    ) {
        let _ = self.db.folders().update_status(folder_id, status);
        self.emit_status(app, folder_id, status);
    }

    fn emit_status<R: Runtime>(&self, app: &AppHandle<R>, folder_id: Uuid, status: WatcherStatus) {
        let _ = app.emit(
            "watcher:status-changed",
            serde_json::json!({ "folderId": folder_id, "status": status.as_str() }),
        );
    }

    fn trigger_catch_up<R: Runtime>(&self, app: &AppHandle<R>, folder: &Folder) {
        let (is_active, is_paused) = {
            let map = self.watchers.lock();
            let Some(handle) = map.get(&folder.id) else {
                return;
            };
            (handle.is_active.clone(), handle.is_paused.clone())
        };
        let app = app.clone();
        let db = self.db.clone();
        let cancellations = self.cancellations.clone();
        let automation_paused = self.automation_paused.clone();
        let folder_path = folder.path.clone();
        tauri::async_runtime::spawn(async move {
            if let Some(error) = organize_with_retry(
                &app,
                &db,
                &cancellations,
                &folder_path,
                None,
                WatcherRunGate {
                    automation_paused: &automation_paused,
                    is_active: &is_active,
                    is_paused: &is_paused,
                },
            )
            .await
            {
                tracing::warn!(?folder_path, %error, "watcher catch-up failed");
                crate::ipc::files::report_background_error(
                    &app,
                    &db,
                    &folder_path,
                    "watcher",
                    &error,
                );
            }
        });
    }
}

/// Organize a watcher batch, waiting only while another operation owns the
/// folder. Any other error is final and must be surfaced to the user rather
/// than retried indefinitely.
struct WatcherRunGate<'a> {
    automation_paused: &'a AtomicBool,
    is_active: &'a AtomicBool,
    is_paused: &'a AtomicBool,
}

async fn organize_with_retry<R: Runtime>(
    app: &AppHandle<R>,
    db: &Database,
    cancellations: &CancellationRegistry,
    folder_path: &str,
    paths: Option<&[PathBuf]>,
    gate: WatcherRunGate<'_>,
) -> Option<String> {
    loop {
        if gate.automation_paused.load(Ordering::Acquire)
            || !gate.is_active.load(Ordering::Acquire)
            || gate.is_paused.load(Ordering::Acquire)
        {
            return None;
        }
        let result = match paths {
            Some(paths) => {
                crate::ipc::files::organize_automated_paths_inner(
                    app,
                    db,
                    cancellations,
                    folder_path,
                    paths.to_vec(),
                    gate.automation_paused,
                )
                .await
            }
            None => {
                crate::ipc::files::organize_automated_inner(
                    app,
                    db,
                    cancellations,
                    folder_path,
                    true,
                    gate.automation_paused,
                )
                .await
            }
        };
        if result["success"].as_bool().unwrap_or(false) {
            return None;
        }
        let error = result["error"]
            .as_str()
            .unwrap_or("ORGANIZE_FAILED")
            .to_string();
        if error != "ORGANIZE_ALREADY_RUNNING" {
            return Some(error);
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
