//! Watcher service — see SPEC §8.2.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use notify::Watcher;
use notify_debouncer_full::{new_debouncer, DebouncedEvent, FileIdMap};
use parking_lot::Mutex;
use tauri::AppHandle;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::domain::{Folder, WatcherStatus};
use crate::storage::Database;

use super::filters::should_ignore;

pub struct WatcherService {
    db: Arc<Database>,
    /// Active watchers, keyed by folder ID.
    watchers: Mutex<HashMap<Uuid, WatcherHandle>>,
}

struct WatcherHandle {
    /// Drop this to stop the watcher.
    _debouncer: notify_debouncer_full::Debouncer<
        notify_debouncer_full::notify::RecommendedWatcher,
        FileIdMap,
    >,
    is_paused: bool,
}

impl WatcherService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            watchers: Mutex::new(HashMap::new()),
        }
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
        let folder_path = folder.path.clone();
        let app_handle = app.clone();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                match msg {
                    Ok(events) => {
                        let mut has_new_files = false;
                        for ev in events {
                            for path in &ev.paths {
                                if should_ignore(path) {
                                    continue;
                                }
                                tracing::debug!(?path, "file added/modified in watched folder");
                                has_new_files = true;
                            }
                        }
                        if has_new_files {
                            tracing::info!(?folder_path, "triggering auto-organize for folder");
                            let _ = crate::ipc::files::organize_inner(
                                &app_handle,
                                &db,
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

        self.update_status(folder_id, WatcherStatus::Running);
        self.watchers.lock().insert(
            folder_id,
            WatcherHandle {
                _debouncer: debouncer,
                is_paused: false,
            },
        );
        Ok(())
    }

    pub fn stop_watching(&self, _app: &AppHandle, folder_id: Uuid) -> Result<()> {
        if self.watchers.lock().remove(&folder_id).is_some() {
            self.update_status(folder_id, WatcherStatus::Stopped);
        }
        Ok(())
    }

    pub fn pause(&self, _app: &AppHandle, folder_id: Uuid) -> Result<()> {
        let mut map = self.watchers.lock();
        if let Some(h) = map.get_mut(&folder_id) {
            h.is_paused = true;
            self.update_status(folder_id, WatcherStatus::Paused);
        }
        Ok(())
    }

    pub fn resume(&self, _app: &AppHandle, folder_id: Uuid) -> Result<()> {
        let mut map = self.watchers.lock();
        if let Some(h) = map.get_mut(&folder_id) {
            h.is_paused = false;
            self.update_status(folder_id, WatcherStatus::Running);
        }
        Ok(())
    }

    pub fn pause_all(&self, app: &AppHandle) {
        let mut map = self.watchers.lock();
        for (id, h) in map.iter_mut() {
            h.is_paused = true;
            self.update_status(*id, WatcherStatus::Paused);
        }
        let _ = app;
    }

    pub fn resume_all(&self, app: &AppHandle) {
        let mut map = self.watchers.lock();
        for (id, h) in map.iter_mut() {
            h.is_paused = false;
            self.update_status(*id, WatcherStatus::Running);
        }
        let _ = app;
    }

    pub fn status(&self, folder_id: Uuid) -> WatcherStatus {
        let map = self.watchers.lock();
        map.get(&folder_id)
            .map(|h| {
                if h.is_paused {
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

    fn update_status(&self, folder_id: Uuid, status: WatcherStatus) {
        if let Ok(Some(mut folder)) = self.db.folders().get(folder_id) {
            folder.watcher_status = status;
            let _ = self.db.folders().update(&folder);
        }
    }
}
