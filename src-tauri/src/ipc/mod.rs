//! IPC handlers — one submodule per channel group.
//!
//! Every handler is a `#[tauri::command]` async fn. The shared
//! `AppState` is injected by Tauri via `State<AppState>`.

pub mod activity;
pub mod app;
pub mod commands;
pub mod dedup;
pub mod files;
pub mod folders;
pub mod organize;
pub mod permissions;
pub mod rules;
pub mod settings;
pub mod watcher;
