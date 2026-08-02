//! Tray icon service — see SPEC §7.4.

use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{TrayIcon, TrayIconBuilder},
    AppHandle, Manager,
};

pub struct TrayService {
    _app: AppHandle,
    icon: TrayIcon,
    visible: AtomicBool,
}

impl TrayService {
    pub fn new(app: &AppHandle) -> Result<Self> {
        let open_app = MenuItem::with_id(app, "open_app", "Open FileGrove", true, None::<&str>)?;
        let pause_all = MenuItem::with_id(app, "pause_all", "Pause All", true, None::<&str>)?;
        let resume_all = MenuItem::with_id(app, "resume_all", "Resume All", true, None::<&str>)?;
        let separator = PredefinedMenuItem::separator(app)?;
        let quit = MenuItem::with_id(app, "quit", "Quit FileGrove", true, None::<&str>)?;

        let menu = Menu::with_items(
            app,
            &[&open_app, &pause_all, &resume_all, &separator, &quit],
        )?;

        let mut tray_builder = TrayIconBuilder::with_id("main")
            .tooltip("FileGrove")
            .menu(&menu)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "open_app" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.unminimize();
                        let _ = w.set_focus();
                    }
                }
                "pause_all" => {
                    tracing::info!("tray: pause all");
                    if let Err(error) = crate::ipc::app::apply_automation_paused(app, true) {
                        tracing::warn!("could not pause automation: {error}");
                    }
                }
                "resume_all" => {
                    tracing::info!("tray: resume all");
                    if let Err(error) = crate::ipc::app::apply_automation_paused(app, false) {
                        tracing::warn!("could not resume automation: {error}");
                    }
                }
                "quit" => {
                    crate::shutdown::request_exit(app);
                }
                _ => {}
            });
        if let Some(default_icon) = app.default_window_icon().cloned() {
            tray_builder = tray_builder.icon(default_icon);
        }
        let icon = tray_builder.build(app)?;

        Ok(Self {
            _app: app.clone(),
            icon,
            visible: AtomicBool::new(true),
        })
    }

    pub fn set_visible(&self, visible: bool) -> Result<()> {
        self.icon.set_visible(visible)?;
        self.visible.store(visible, Ordering::Release);
        Ok(())
    }

    pub fn is_visible(&self) -> bool {
        self.visible.load(Ordering::Acquire)
    }
}
