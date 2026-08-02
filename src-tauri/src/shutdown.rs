//! Ordered application shutdown for the watcher and active operations.

use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

#[cfg(not(test))]
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ShutdownPhase {
    Running = 0,
    Stopping = 1,
    ReadyToExit = 2,
}

pub struct ShutdownCoordinator {
    phase: AtomicU8,
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self {
            phase: AtomicU8::new(ShutdownPhase::Running as u8),
        }
    }
}

impl ShutdownCoordinator {
    pub fn phase(&self) -> ShutdownPhase {
        match self.phase.load(Ordering::Acquire) {
            1 => ShutdownPhase::Stopping,
            2 => ShutdownPhase::ReadyToExit,
            _ => ShutdownPhase::Running,
        }
    }

    fn begin(&self) -> bool {
        self.phase
            .compare_exchange(
                ShutdownPhase::Running as u8,
                ShutdownPhase::Stopping as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    fn ready(&self) {
        self.phase
            .store(ShutdownPhase::ReadyToExit as u8, Ordering::Release);
    }
}

#[cfg(not(test))]
pub fn request_exit(app: &AppHandle) {
    let Some(state) = app.try_state::<crate::AppState>() else {
        app.exit(0);
        return;
    };
    if !state.shutdown.begin() {
        return;
    }

    tracing::info!("orderly shutdown requested");
    state.watcher.shutdown_all();
    state.cancellations.begin_shutdown();
    if let Some(tray) = state.tray.read().as_ref() {
        let _ = tray.set_visible(false);
    }

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(state) = app.try_state::<crate::AppState>() else {
            app.exit(0);
            return;
        };
        let deadline = Instant::now() + Duration::from_secs(3);
        while state.cancellations.has_active_work() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        if state.cancellations.has_active_work() {
            tracing::warn!("shutdown drain timed out after cancellation");
        }
        state.shutdown.ready();
        app.exit(0);
    });
}

#[cfg(not(test))]
pub fn force_quiesce(app: &AppHandle) {
    if let Some(state) = app.try_state::<crate::AppState>() {
        state.watcher.shutdown_all();
        state.cancellations.begin_shutdown();
        state.shutdown.ready();
    }
}
