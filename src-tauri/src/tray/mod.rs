//! Tray icon service — see SPEC §7.4.

use anyhow::Result;
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub struct TrayService {
    _app: AppHandle,
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

        let _tray = TrayIconBuilder::with_id("main")
            .tooltip("FileGrove")
            .menu(&menu)
            .on_menu_event(|app, event| match event.id.as_ref() {
                "open_app" => {
                    if let Some(w) = app.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.set_focus();
                    }
                }
                "pause_all" => {
                    tracing::info!("tray: pause all");
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        state.watcher.pause_all(app);
                    }
                }
                "resume_all" => {
                    tracing::info!("tray: resume all");
                    if let Some(state) = app.try_state::<crate::AppState>() {
                        state.watcher.resume_all(app);
                    }
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            })
            .build(app)?;

        Ok(Self { _app: app.clone() })
    }
}
