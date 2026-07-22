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
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::domain::{Folder, FolderMode, WatcherStatus};
use crate::engine::cancellation::CancellationRegistry;
use crate::storage::Database;

use super::filters::should_ignore;

pub struct WatcherService {
    db: Arc<Database>,
    cancellations: Arc<CancellationRegistry>,
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
}

impl WatcherService {
    pub fn new(db: Arc<Database>, cancellations: Arc<CancellationRegistry>) -> Self {
        Self {
            db,
            cancellations,
            watchers: Mutex::new(HashMap::new()),
        }
    }

    /// Restore watchers that were running or paused when the app closed.
    pub fn init(&self, app: &AppHandle) -> Result<usize> {
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
    pub fn start_watching(&self, app: &AppHandle, folder: &Folder) -> Result<()> {
        let folder_id = folder.id;
        {
            let map = self.watchers.lock();
            if map.contains_key(&folder_id) {
                return Ok(());
            }
        }

        let (tx, mut rx) = mpsc::channel::<Result<Vec<DebouncedEvent>>>(16);
        let mut debouncer = new_debouncer(
            Duration::from_millis(500),
            None,
            move |res: Result<Vec<DebouncedEvent>, Vec<notify::Error>>| {
                let _ = tx.blocking_send(res.map_err(|e| anyhow!("{e:?}")));
            },
        )?;

        debouncer.watcher().watch(
            PathBuf::from(&folder.path).as_path(),
            notify::RecursiveMode::NonRecursive,
        )?;

        let db = self.db.clone();
        let cancellations = self.cancellations.clone();
        let folder_path = folder.path.clone();
        let app_handle = app.clone();
        let is_paused = Arc::new(AtomicBool::new(false));
        let task_is_paused = is_paused.clone();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if task_is_paused.load(Ordering::Acquire) {
                    continue;
                }
                match msg {
                    Ok(events) => {
                        let mut has_new_files = false;
                        for ev in events {
                            if !matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                                continue;
                            }
                            for path in &ev.paths {
                                if should_ignore(path) {
                                    continue;
                                }
                                tracing::debug!(?path, "file added/modified in watched folder");
                                has_new_files = true;
                            }
                        }
                        if has_new_files && !task_is_paused.load(Ordering::Acquire) {
                            tracing::info!(?folder_path, "triggering auto-organize for folder");
                            let _ = crate::ipc::files::organize_inner(
                                &app_handle,
                                &db,
                                &cancellations,
                                &folder_path,
                                false,
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        tracing::warn!(?folder_path, "watcher error: {e}");
                    }
                }
            }
        });

        self.update_status(app, folder_id, WatcherStatus::Running);
        self.watchers.lock().insert(
            folder_id,
            WatcherHandle {
                _debouncer: debouncer,
                is_paused,
            },
        );
        Ok(())
    }

    pub fn start_watching_and_catch_up(&self, app: &AppHandle, folder: &Folder) -> Result<()> {
        self.start_watching(app, folder)?;
        self.trigger_catch_up(app, folder);
        Ok(())
    }

    pub fn stop_watching(&self, app: &AppHandle, folder_id: Uuid) -> Result<()> {
        self.watchers.lock().remove(&folder_id);
        self.update_status(app, folder_id, WatcherStatus::Stopped);
        Ok(())
    }

    pub fn pause(&self, app: &AppHandle, folder_id: Uuid) -> Result<()> {
        let map = self.watchers.lock();
        let Some(handle) = map.get(&folder_id) else {
            return Err(anyhow!("WATCHER_NOT_RUNNING"));
        };
        handle.is_paused.store(true, Ordering::Release);
        drop(map);
        self.update_status(app, folder_id, WatcherStatus::Paused);
        Ok(())
    }

    pub fn resume(&self, app: &AppHandle, folder_id: Uuid) -> Result<()> {
        let map = self.watchers.lock();
        let Some(handle) = map.get(&folder_id) else {
            return Err(anyhow!("WATCHER_NOT_RUNNING"));
        };
        handle.is_paused.store(false, Ordering::Release);
        drop(map);
        self.update_status(app, folder_id, WatcherStatus::Running);
        if let Ok(Some(folder)) = self.db.folders().get(folder_id) {
            self.trigger_catch_up(app, &folder);
        }
        Ok(())
    }

    pub fn pause_all(&self, app: &AppHandle) {
        let map = self.watchers.lock();
        let folder_ids: Vec<Uuid> = map.keys().copied().collect();
        for h in map.values() {
            h.is_paused.store(true, Ordering::Release);
        }
        drop(map);
        for folder_id in folder_ids {
            self.update_status(app, folder_id, WatcherStatus::Paused);
        }
    }

    pub fn resume_all(&self, app: &AppHandle) {
        let map = self.watchers.lock();
        let folder_ids: Vec<Uuid> = map.keys().copied().collect();
        for h in map.values() {
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
                if h.is_paused.load(Ordering::Acquire) {
                    WatcherStatus::Paused
                } else {
                    WatcherStatus::Running
                }
            })
            .unwrap_or(WatcherStatus::Stopped)
    }

    /// Shut down every active watcher. Called on app quit.
    pub fn shutdown_all(&self) {
        let mut map = self.watchers.lock();
        let count = map.len();
        map.clear();
        tracing::info!("watcher shutdown: stopped {} watcher(s)", count);
    }

    fn update_status(&self, app: &AppHandle, folder_id: Uuid, status: WatcherStatus) {
        if let Ok(Some(mut folder)) = self.db.folders().get(folder_id) {
            folder.watcher_status = status;
            let _ = self.db.folders().update(&folder);
        }
        let _ = app.emit(
            "watcher:status-changed",
            serde_json::json!({ "folderId": folder_id, "status": status.as_str() }),
        );
    }

    fn trigger_catch_up(&self, app: &AppHandle, folder: &Folder) {
        let app = app.clone();
        let db = self.db.clone();
        let cancellations = self.cancellations.clone();
        let folder_path = folder.path.clone();
        tauri::async_runtime::spawn(async move {
            let _ =
                crate::ipc::files::organize_inner(&app, &db, &cancellations, &folder_path, false)
                    .await;
        });
    }
}
