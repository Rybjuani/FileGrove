//! First-run profile for the conservative, always-on personal-file setup.
//!
//! This is deliberately small and idempotent. It creates only the user's
//! standard folders, never walks the filesystem itself, and delegates every
//! move to the normal journaled organizer.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use directories::UserDirs;
use serde_json::Value;
use uuid::Uuid;

use crate::domain::{Action, Folder, FolderMode, MaxDepth, SubfolderConfig, WatcherStatus};
use crate::storage::Database;

const PROFILE_KEY: &str = "defaultProfileConfigured";

pub fn ensure_personal_profile(db: &Database) -> Result<()> {
    if db
        .settings()
        .get(PROFILE_KEY)?
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
    {
        return Ok(());
    }

    // Existing installations are already configured by their owner. Never
    // rewrite their folders, rules, destinations or startup preference just
    // because this profile was added in a later application version.
    if !db.folders().list()?.is_empty() {
        db.settings().set(PROFILE_KEY, &Value::Bool(true))?;
        return Ok(());
    }

    let user_dirs = UserDirs::new().context("could not resolve user directories")?;
    let home = user_dirs.home_dir();
    let desktop = user_dirs
        .desktop_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("Desktop"));
    let downloads = user_dirs
        .download_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("Downloads"));
    let documents = user_dirs
        .document_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("Documents"));
    let pictures = user_dirs
        .picture_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("Pictures"));
    let videos = user_dirs
        .video_dir()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join("Videos"));

    for directory in [&desktop, &downloads, &documents, &pictures, &videos] {
        fs::create_dir_all(directory)
            .with_context(|| format!("could not create {}", directory.display()))?;
        if !directory.is_dir() {
            anyhow::bail!(
                "default directory is not a directory: {}",
                directory.display()
            );
        }
    }

    let mut destinations = HashMap::new();
    destinations.insert("Image".to_string(), pictures.to_string_lossy().into_owned());
    destinations.insert(
        "FileText".to_string(),
        documents.to_string_lossy().into_owned(),
    );
    destinations.insert("Video".to_string(), videos.to_string_lossy().into_owned());

    ensure_folder(db, &desktop, "Desktop", false, &destinations)?;
    ensure_folder(db, &downloads, "Downloads", false, &destinations)?;

    db.settings().set(PROFILE_KEY, &Value::Bool(true))?;
    // Startup is reconciled by the existing Tauri autostart plugin during
    // bootstrap. Persisting this here makes the preference survive upgrades.
    db.settings().set("launchAtStartup", &Value::Bool(true))?;
    db.settings()
        .set("conflictBehavior", &Value::String("rename".into()))?;
    db.settings()
        .set("duplicateBehavior", &Value::String("keepBoth".into()))?;
    db.settings()
        .set("onboardingCompleted", &Value::Bool(true))?;
    Ok(())
}

fn ensure_folder(
    db: &Database,
    path: &Path,
    name: &str,
    recursive: bool,
    destinations: &HashMap<String, String>,
) -> Result<()> {
    let canonical = crate::paths::safety::validate_scan_root(path)?;
    let path_string = canonical.to_string_lossy().into_owned();
    let folder = if let Some(folder) = db.folders().get_by_path(&path_string)? {
        folder
    } else {
        let folder = Folder {
            id: Uuid::new_v4(),
            path: path_string,
            name: name.to_string(),
            mode: FolderMode::Auto,
            rule_ids: vec![],
            watcher_status: WatcherStatus::Running,
            created_at: chrono::Utc::now(),
            subfolders: SubfolderConfig {
                enabled: recursive,
                destination_mode: crate::domain::DestinationMode::Flatten,
                first_run_confirmed: recursive,
                max_depth: MaxDepth::Level(64),
            },
            exclude_folders: vec![],
            exclude_patterns: vec![],
        };
        db.folders().insert(&folder)?;
        folder
    };

    let configured = db.folders().apply_recommended_rules_with_destinations(
        folder.id,
        "es",
        Some(destinations),
    )?;
    let mut configured = configured;
    configured.mode = FolderMode::Auto;
    configured.watcher_status = WatcherStatus::Running;
    configured.subfolders.enabled = recursive;
    configured.subfolders.first_run_confirmed = recursive;
    configured.subfolders.max_depth = MaxDepth::Level(64);
    db.folders().update(&configured)?;

    // The always-on profile only enables the three requested media classes.
    // Other built-in rules remain available in the UI but cannot move files
    // unexpectedly on first run.
    for mut rule in db.rules().get_by_ids(&configured.rule_ids)? {
        if !matches!(rule.action, Action::Move { .. }) {
            continue;
        }
        rule.enabled = matches!(rule.icon.as_str(), "Image" | "FileText" | "Video");
        if rule.enabled {
            if let Action::Move { destination } = &mut rule.action {
                if let Some(value) = destinations.get(&rule.icon) {
                    *destination = value.clone();
                }
            }
        }
        db.rules().update(&rule)?;
    }
    Ok(())
}
