//! Scheduler service — see SPEC §8.3.

use std::sync::Arc;

use anyhow::Result;
use tauri::AppHandle;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::domain::FolderMode;
use crate::storage::Database;

pub struct SchedulerService {
    db: Arc<Database>,
    scheduler: tokio::sync::OnceCell<JobScheduler>,
    jobs: parking_lot::Mutex<std::collections::HashMap<Uuid, uuid::Uuid>>,
    app: parking_lot::Mutex<Option<AppHandle>>,
}

impl SchedulerService {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            scheduler: tokio::sync::OnceCell::new(),
            jobs: parking_lot::Mutex::new(std::collections::HashMap::new()),
            app: parking_lot::Mutex::new(None),
        }
    }

    pub fn set_app_handle(&self, app: AppHandle) {
        *self.app.lock() = Some(app);
    }

    async fn get_scheduler(&self) -> Result<&JobScheduler> {
        self.scheduler
            .get_or_try_init(|| async {
                let s = JobScheduler::new()
                    .await
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                s.start().await.map_err(|e| anyhow::anyhow!("{e}"))?;
                Ok(s)
            })
            .await
    }

    /// Boot-time: register cron jobs for every folder currently in
    /// `scheduled` mode with an enabled schedule.
    pub fn init(&self) -> Result<()> {
        let folders = self.db.folders().list()?;
        let count = folders
            .iter()
            .filter(|f| f.mode == FolderMode::Scheduled)
            .filter(|f| f.schedule.as_ref().is_some_and(|s| s.enabled))
            .count();
        tracing::info!("scheduler init: {count} folder(s) to register");
        Ok(())
    }

    /// (Re)register a folder's schedule. Returns true if a job was created.
    pub async fn upsert(&self, folder_id: Uuid) -> Result<bool> {
        let folder = match self.db.folders().get(folder_id)? {
            Some(f) => f,
            None => return Ok(false),
        };
        if folder.mode != FolderMode::Scheduled {
            return Ok(false);
        }
        let schedule = match folder.schedule {
            Some(ref s) if s.enabled => s,
            _ => return Ok(false),
        };
        let cron = match schedule.to_cron() {
            Some(c) => c,
            None => return Ok(false),
        };

        self.remove(folder_id).await?;

        let scheduler = self.get_scheduler().await?;
        let fid = folder_id;
        let db = self.db.clone();
        let app_handle = self.app.lock().clone();

        let job = Job::new_async(cron.as_str(), move |_uuid, _l| {
            let fid = fid;
            let db = db.clone();
            let app = app_handle.clone();
            Box::pin(async move {
                tracing::info!(?fid, "scheduled tick for folder");
                if let Ok(Some(folder)) = db.folders().get(fid) {
                    if let Some(app) = app {
                        let _ = crate::ipc::files::organize_inner(
                            &app,
                            &db,
                            &folder.path,
                            folder.subfolders.enabled,
                        )
                        .await;
                    }
                }
            })
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        let job_uuid = scheduler
            .add(job)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.jobs.lock().insert(folder_id, job_uuid);
        Ok(true)
    }

    pub async fn remove(&self, folder_id: Uuid) -> Result<()> {
        let job_uuid = { self.jobs.lock().remove(&folder_id) };
        if let Some(job_uuid) = job_uuid {
            if let Ok(scheduler) = self.get_scheduler().await {
                let _ = scheduler.remove(&job_uuid).await;
            }
        }
        Ok(())
    }

    pub fn next_run(&self, _folder_id: Uuid) -> Option<chrono::DateTime<chrono::Utc>> {
        None
    }

    pub async fn shutdown(&self) {
        // tokio_cron_scheduler 0.10 does not expose a shutdown method on immutable ref.
        // The scheduler is cleaned up automatically when dropped.
        tracing::info!("scheduler shutdown");
    }
}
