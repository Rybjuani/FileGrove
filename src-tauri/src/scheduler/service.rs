//! Scheduler service — see SPEC §8.3.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use anyhow::Result;
use chrono::TimeZone;
use tauri::{AppHandle, Emitter};
use tokio_cron_scheduler::{Job, JobScheduler};
use uuid::Uuid;

use crate::domain::{Folder, FolderMode, Schedule, ScheduleRegistration};
use crate::engine::cancellation::CancellationRegistry;
use crate::engine::keyed_lock::KeyedAsyncLock;
use crate::storage::Database;

pub struct SchedulerService {
    db: Arc<Database>,
    cancellations: Arc<CancellationRegistry>,
    scheduler: tokio::sync::OnceCell<JobScheduler>,
    jobs: Arc<parking_lot::Mutex<HashMap<Uuid, RegisteredJob>>>,
    operation_locks: Arc<KeyedAsyncLock<Uuid>>,
    next_generation: AtomicU64,
    automation_paused: Arc<AtomicBool>,
    shutting_down: Arc<AtomicBool>,
    lifecycle: tokio::sync::Mutex<()>,
    app: parking_lot::Mutex<Option<AppHandle>>,
}

#[derive(Debug)]
struct RegisteredJob {
    job_uuid: uuid::Uuid,
    registration: ScheduleRegistration,
}

impl SchedulerService {
    pub fn new(db: Arc<Database>, cancellations: Arc<CancellationRegistry>) -> Self {
        let automation_paused = db.settings().get_automation_paused();
        Self {
            db,
            cancellations,
            scheduler: tokio::sync::OnceCell::new(),
            jobs: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            operation_locks: Arc::new(KeyedAsyncLock::default()),
            next_generation: AtomicU64::new(1),
            automation_paused: Arc::new(AtomicBool::new(automation_paused)),
            shutting_down: Arc::new(AtomicBool::new(false)),
            lifecycle: tokio::sync::Mutex::new(()),
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
        let operation_lock = self.folder_operation_lock(folder_id);
        let _operation_guard = operation_lock.lock().await;
        let _lifecycle_guard = self.lifecycle.lock().await;
        if self.shutting_down.load(Ordering::Acquire) {
            return Ok(false);
        }
        self.upsert_locked(folder_id).await
    }

    async fn upsert_locked(&self, folder_id: Uuid) -> Result<bool> {
        // Always remove an old job first, including when a schedule was
        // disabled or the folder changed to another mode.
        self.remove_locked(folder_id).await?;

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
        let generation = self.next_generation.fetch_add(1, Ordering::Relaxed);
        let captured_schedule = schedule.clone();
        let db = self.db.clone();
        let cancellations = self.cancellations.clone();
        let app_handle = self.app.lock().clone();
        let jobs = self.jobs.clone();
        let operation_locks = self.operation_locks.clone();
        let job_schedule = captured_schedule.clone();
        let shutting_down = self.shutting_down.clone();
        let automation_paused = self.automation_paused.clone();

        let job = Job::new_async_tz(cron.as_str(), chrono::Local, move |_uuid, _l| {
            let fid = fid;
            let db = db.clone();
            let cancellations = cancellations.clone();
            let app = app_handle.clone();
            let jobs = jobs.clone();
            let operation_locks = operation_locks.clone();
            let expected_schedule = job_schedule.clone();
            let shutting_down = shutting_down.clone();
            let automation_paused = automation_paused.clone();
            Box::pin(async move {
                if shutting_down.load(Ordering::Acquire)
                    || automation_paused.load(Ordering::Acquire)
                {
                    return;
                }
                let operation_lock = folder_operation_lock_from(&operation_locks, fid);
                let _operation_guard = operation_lock.lock().await;
                if shutting_down.load(Ordering::Acquire)
                    || automation_paused.load(Ordering::Acquire)
                    || !registration_is_current(&jobs, fid, generation, &expected_schedule)
                {
                    tracing::debug!(?fid, generation, "ignoring stale scheduled tick");
                    return;
                }
                tracing::info!(?fid, "scheduled tick for folder");
                if let Ok(Some(folder)) = db.folders().get(fid) {
                    let schedule_is_active = schedule_matches(&folder, &expected_schedule);
                    if schedule_is_active {
                        if let Some(app) = app {
                            let result = crate::ipc::files::organize_automated_inner(
                                &app,
                                &db,
                                &cancellations,
                                &folder.path,
                                folder.subfolders.enabled,
                                &automation_paused,
                            )
                            .await;
                            let _ = app.emit(
                                "schedule:run-completed",
                                serde_json::json!({
                                    "folderId": fid,
                                    "result": result.clone(),
                                }),
                            );
                            if !result["success"].as_bool().unwrap_or(false) {
                                let error = result["error"].as_str().unwrap_or("ORGANIZE_FAILED");
                                crate::ipc::files::report_background_error(
                                    &app,
                                    &db,
                                    &folder.path,
                                    "schedule",
                                    error,
                                );
                            }
                        }
                    }
                }
            })
        })
        .map_err(|e| anyhow::anyhow!("{e}"))?;

        let job_uuid = scheduler
            .add(job)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.jobs.lock().insert(
            folder_id,
            RegisteredJob {
                job_uuid,
                registration: ScheduleRegistration::new(generation, captured_schedule),
            },
        );
        Ok(true)
    }

    pub async fn remove(&self, folder_id: Uuid) -> Result<()> {
        let operation_lock = self.folder_operation_lock(folder_id);
        let _operation_guard = operation_lock.lock().await;
        let _lifecycle_guard = self.lifecycle.lock().await;
        self.remove_locked(folder_id).await
    }

    async fn remove_locked(&self, folder_id: Uuid) -> Result<()> {
        // Invalidate before awaiting the scheduler. Even if removal fails, a
        // callback already queued by the cron library can no longer execute.
        let job_uuid = {
            let mut jobs = self.jobs.lock();
            jobs.get_mut(&folder_id).map(|job| {
                job.registration.deactivate();
                job.job_uuid
            })
        };
        if let Some(job_uuid) = job_uuid {
            let scheduler = self.get_scheduler().await?;
            scheduler
                .remove(&job_uuid)
                .await
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            let mut jobs = self.jobs.lock();
            if jobs
                .get(&folder_id)
                .is_some_and(|job| job.job_uuid == job_uuid)
            {
                jobs.remove(&folder_id);
            }
        }
        Ok(())
    }

    pub fn next_run(&self, folder_id: Uuid) -> Option<chrono::DateTime<chrono::Utc>> {
        let registered_schedule = self
            .jobs
            .lock()
            .get(&folder_id)
            .filter(|job| job.registration.is_active())
            .map(|job| job.registration.schedule().clone())?;
        let folder = self.db.folders().get(folder_id).ok()??;
        if !schedule_matches(&folder, &registered_schedule) {
            return None;
        }
        let next_local =
            registered_schedule.next_local_run_after(chrono::Local::now().naive_local())?;
        chrono::Local
            .from_local_datetime(&next_local)
            .earliest()
            .map(|run| run.with_timezone(&chrono::Utc))
    }

    pub fn pause_all(&self) {
        self.automation_paused.store(true, Ordering::Release);
        let folder_ids: Vec<Uuid> = self.jobs.lock().keys().copied().collect();
        for folder_id in folder_ids {
            if let Ok(Some(folder)) = self.db.folders().get(folder_id) {
                let folder_key =
                    crate::paths::safety::canonical_path_key(std::path::Path::new(&folder.path));
                self.cancellations.cancel_folder(&folder_key);
            }
        }
    }

    pub fn resume_all(&self) {
        if !self.shutting_down.load(Ordering::Acquire) {
            self.automation_paused.store(false, Ordering::Release);
        }
    }

    /// Synchronously invalidate every registration. This is safe to call from
    /// an exit event before the async runtime has a chance to drain.
    pub fn begin_shutdown(&self) -> bool {
        let first_request = !self.shutting_down.swap(true, Ordering::AcqRel);
        self.automation_paused.store(true, Ordering::Release);
        for job in self.jobs.lock().values_mut() {
            job.registration.deactivate();
        }
        first_request
    }

    /// Stop the cron driver's ticking loop. A clone of JobScheduler shares the
    /// underlying state and provides the mutable receiver required by 0.10.
    pub async fn shutdown(&self) {
        self.begin_shutdown();
        let _lifecycle_guard = self.lifecycle.lock().await;
        // Catch a registration whose normal upsert already held the lifecycle
        // lock when shutdown was first requested.
        for job in self.jobs.lock().values_mut() {
            job.registration.deactivate();
        }
        if let Some(scheduler) = self.scheduler.get() {
            let mut scheduler = scheduler.clone();
            if let Err(error) = scheduler.shutdown().await {
                tracing::warn!("scheduler shutdown failed: {error}");
            }
        }
        self.jobs.lock().clear();
        self.app.lock().take();
        tracing::info!("scheduler shutdown complete");
    }

    fn folder_operation_lock(&self, folder_id: Uuid) -> Arc<tokio::sync::Mutex<()>> {
        self.operation_locks.for_key(folder_id)
    }
}

fn folder_operation_lock_from(
    operation_locks: &KeyedAsyncLock<Uuid>,
    folder_id: Uuid,
) -> Arc<tokio::sync::Mutex<()>> {
    operation_locks.for_key(folder_id)
}

fn registration_is_current(
    jobs: &parking_lot::Mutex<HashMap<Uuid, RegisteredJob>>,
    folder_id: Uuid,
    generation: u64,
    schedule: &Schedule,
) -> bool {
    jobs.lock()
        .get(&folder_id)
        .is_some_and(|job| job.registration.is_current(generation, schedule))
}

fn schedule_matches(folder: &Folder, expected: &Schedule) -> bool {
    folder.mode == FolderMode::Scheduled
        && expected.enabled
        && folder.schedule.as_ref() == Some(expected)
}
