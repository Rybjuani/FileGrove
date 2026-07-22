//! Scheduler service — see SPEC §8.3.

use std::sync::Arc;

use anyhow::Result;
use chrono::TimeZone;
use tauri::AppHandle;
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::domain::FolderMode;
use crate::engine::cancellation::CancellationRegistry;
use crate::storage::Database;

pub struct SchedulerService {
    db: Arc<Database>,
    cancellations: Arc<CancellationRegistry>,
    scheduler: tokio::sync::OnceCell<JobScheduler>,
    jobs: parking_lot::Mutex<std::collections::HashMap<Uuid, uuid::Uuid>>,
    app: parking_lot::Mutex<Option<AppHandle>>,
}

impl SchedulerService {
    pub fn new(db: Arc<Database>, cancellations: Arc<CancellationRegistry>) -> Self {
        Self {
            db,
            cancellations,
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
    pub async fn init(&self) -> Result<usize> {
        let folders = self.db.folders().list()?;
        let mut restored = 0usize;
        for folder in folders {
            if folder.mode != FolderMode::Scheduled {
                continue;
            }
            match self.upsert(folder.id).await {
                Ok(true) => restored += 1,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(folder_id = ?folder.id, "could not restore schedule: {error}");
                }
            }
        }
        tracing::info!("scheduler init: restored {restored} folder(s)");
        Ok(restored)
    }

    /// (Re)register a folder's schedule. Returns true if a job was created.
    pub async fn upsert(&self, folder_id: Uuid) -> Result<bool> {
        // Always remove an old job first, including when a schedule was
        // disabled or the folder changed to another mode.
        self.remove(folder_id).await?;

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

        let scheduler = self.get_scheduler().await?;
        let fid = folder_id;
        let db = self.db.clone();
        let cancellations = self.cancellations.clone();
        let app_handle = self.app.lock().clone();

        let job = Job::new_async_tz(cron.as_str(), chrono::Local, move |_uuid, _l| {
            let fid = fid;
            let db = db.clone();
            let cancellations = cancellations.clone();
            let app = app_handle.clone();
            Box::pin(async move {
                tracing::info!(?fid, "scheduled tick for folder");
                if let Ok(Some(folder)) = db.folders().get(fid) {
                    if let Some(app) = app {
                        let _ = crate::ipc::files::organize_inner(
                            &app,
                            &db,
                            &cancellations,
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

    pub fn next_run(&self, folder_id: Uuid) -> Option<chrono::DateTime<chrono::Utc>> {
        if !self.jobs.lock().contains_key(&folder_id) {
            return None;
        }
        let folder = self.db.folders().get(folder_id).ok()??;
        let schedule = folder.schedule?;
        let next_local = schedule.next_local_run_after(chrono::Local::now().naive_local())?;
        chrono::Local
            .from_local_datetime(&next_local)
            .earliest()
            .map(|run| run.with_timezone(&chrono::Utc))
    }

    pub async fn shutdown(&self) {
        // tokio_cron_scheduler 0.10 does not expose a shutdown method on immutable ref.
        // The scheduler is cleaned up automatically when dropped.
        tracing::info!("scheduler shutdown");
    }
}
