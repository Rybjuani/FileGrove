//! FileGrove backend entry point.
//!
//! Wires up Tauri, registers IPC handlers, and boots background services.

#![cfg_attr(test, allow(dead_code, unused_imports))]

mod domain;
mod engine;
mod exif;
#[cfg(not(test))]
mod ipc;
mod paths;
#[cfg(not(test))]
mod scheduler;
mod shutdown;
mod storage;
#[cfg(not(test))]
mod tray;
mod watcher;

#[cfg(not(test))]
use std::sync::Arc;

#[cfg(not(test))]
use parking_lot::RwLock;
#[cfg(not(test))]
use tauri::Manager;
#[cfg(not(test))]
use tauri_plugin_autostart::ManagerExt;
#[cfg(not(test))]
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[cfg(not(test))]
use crate::storage::Database;

/// Shared application state, accessible from every IPC handler.
#[cfg(not(test))]
pub struct AppState {
    pub db: Arc<Database>,
    pub cancellations: Arc<engine::cancellation::CancellationRegistry>,
    pub watcher: Arc<watcher::WatcherService>,
    pub scheduler: Arc<scheduler::SchedulerService>,
    pub shutdown: Arc<shutdown::ShutdownCoordinator>,
    pub tray: Arc<RwLock<Option<tray::TrayService>>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(not(test))]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        // Register first: a second launch should focus the existing window
        // before any watcher, scheduler, tray or database service is started.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            tracing::info!("FileGrove booting up…");

            // Open the database from the user data dir.
            let db = storage::Database::open_default(app.handle())?;
            db.run_migrations()?;
            db.undo().reconcile()?;
            db.command_ledger().reset_in_progress()?;
            let interrupted_deletes = db.activity().mark_interrupted_deletes_for_review()?;
            if interrupted_deletes > 0 {
                tracing::warn!(interrupted_deletes, "delete activities require review");
            }
            let db = Arc::new(db);

            let cancellations = Arc::new(engine::cancellation::CancellationRegistry::default());
            let watcher = Arc::new(watcher::WatcherService::new(
                db.clone(),
                cancellations.clone(),
            ));
            let scheduler = Arc::new(scheduler::SchedulerService::new(
                db.clone(),
                cancellations.clone(),
            ));
            scheduler.set_app_handle(app.handle().clone());

            // Restore background services exactly as they were when the app closed.
            watcher.init(app.handle())?;
            tauri::async_runtime::block_on(scheduler.init())?;
            if db.settings().get_automation_paused() {
                watcher.pause_all(app.handle());
                scheduler.pause_all();
            }

            let state = AppState {
                db: db.clone(),
                cancellations,
                watcher: watcher.clone(),
                scheduler: scheduler.clone(),
                shutdown: Arc::new(shutdown::ShutdownCoordinator::default()),
                tray: Arc::new(RwLock::new(None)),
            };
            app.manage(state);

            // Initialize the tray icon (best-effort; not fatal if it fails).
            match tray::TrayService::new(app.handle()) {
                Ok(t) => {
                    if let Err(error) = t.set_visible(db.settings().get_show_tray_icon()) {
                        tracing::warn!("Could not apply tray visibility: {error}");
                    }
                    *app.state::<AppState>().tray.write() = Some(t);
                    tracing::info!("Tray icon initialized");
                }
                Err(e) => {
                    tracing::warn!("Tray icon init failed: {e}");
                }
            }

            let autolaunch_result = if db.settings().get_launch_at_startup() {
                app.autolaunch().enable()
            } else {
                app.autolaunch().disable()
            };
            if let Err(error) = autolaunch_result {
                tracing::warn!("Could not reconcile launch-at-startup setting: {error}");
            }

            tracing::info!("FileGrove ready");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // App
            ipc::app::get_version,
            ipc::app::get_icon,
            ipc::app::minimize,
            ipc::app::maximize,
            ipc::app::close,
            ipc::app::set_automation_paused,
            // Folders
            ipc::folders::list_folders,
            ipc::folders::add,
            ipc::folders::remove,
            ipc::folders::update_name,
            ipc::folders::update_mode,
            ipc::folders::assign_rules,
            ipc::folders::unassign_rules,
            ipc::folders::update_subfolders_config,
            ipc::folders::update_configuration,
            ipc::folders::update_exclusions,
            ipc::folders::update_schedule,
            ipc::folders::get_next_run,
            ipc::folders::select_destination,
            ipc::folders::confirm_first_run,
            // Rules
            ipc::rules::list_rules,
            ipc::rules::get_by_ids,
            ipc::rules::create,
            ipc::rules::update,
            ipc::rules::delete_rule,
            ipc::rules::toggle,
            ipc::rules::reorder_rules,
            ipc::commands::test_command,
            ipc::commands::allow_interrupted_retry,
            // Files
            ipc::files::scan,
            ipc::files::scan_recursive,
            ipc::files::preview,
            ipc::files::preview_recursive,
            ipc::files::organize,
            ipc::files::organize_recursive,
            // Organize session
            ipc::organize::cancel,
            ipc::organize::latest_undo_session,
            ipc::organize::undo,
            ipc::organize::dismiss_undo,
            // Watcher
            ipc::watcher::start,
            ipc::watcher::stop,
            ipc::watcher::pause,
            ipc::watcher::resume,
            ipc::watcher::status,
            // Settings
            ipc::settings::get,
            ipc::settings::set,
            // Activity
            ipc::activity::list_activity,
            ipc::activity::delete_activity,
            ipc::activity::clear,
            // Permissions
            ipc::permissions::check_access,
            ipc::permissions::open_settings,
            ipc::permissions::is_macos,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Hide instead of close if "minimize to tray" is on.
                if let Some(state) = window.app_handle().try_state::<AppState>() {
                    let tray_visible = state
                        .tray
                        .read()
                        .as_ref()
                        .is_some_and(tray::TrayService::is_visible);
                    if state.shutdown.phase() == shutdown::ShutdownPhase::Running
                        && state.db.settings().get_minimize_to_tray()
                        && tray_visible
                    {
                        window.hide().ok();
                        api.prevent_close();
                    } else {
                        // A tray icon keeps Tauri's event loop alive after the
                        // last window closes. Explicitly exit here; otherwise
                        // disabling minimize-to-tray can leave an invisible
                        // process that has no window to reopen.
                        api.prevent_close();
                        shutdown::request_exit(window.app_handle());
                    }
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building FileGrove application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                if let Some(state) = app.try_state::<AppState>() {
                    if state.shutdown.phase() != shutdown::ShutdownPhase::ReadyToExit {
                        api.prevent_exit();
                        shutdown::request_exit(app);
                    }
                }
            }
            tauri::RunEvent::Exit => shutdown::force_quiesce(app),
            _ => {}
        });
}

#[cfg(not(test))]
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,filegrove=debug"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_target(false))
        .init();
}
