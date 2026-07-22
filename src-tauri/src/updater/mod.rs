//! Auto-updater plugin — see SPEC §10.4.
//!
//! The actual heavy lifting is done by `tauri-plugin-updater`. This
//! module exposes thin IPC handlers the renderer calls to check, download,
//! and install updates.

#![allow(dead_code)]

use anyhow::Result;
use tauri::AppHandle;

/// Check if a newer release is available.
pub async fn check_for_updates(_app: &AppHandle) -> Result<Option<String>> {
    // Delegates to tauri_plugin_updater::Updater::builder()…
    // TODO(M14): full integration
    Ok(None)
}

/// Download the pending update (if any).
pub async fn download_update(_app: &AppHandle) -> Result<()> {
    // TODO(M14)
    Ok(())
}

/// Install the downloaded update and relaunch.
pub async fn install_update(_app: &AppHandle) -> Result<()> {
    // TODO(M14)
    Ok(())
}
