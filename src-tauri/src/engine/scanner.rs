//! Bounded filesystem scanner used by preview, manual runs and catch-up.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use serde::Serialize;
use walkdir::WalkDir;

use crate::domain::{Folder, MaxDepth};
use crate::engine::matcher::FileItem;
use crate::watcher::filters::PathFilter;

const MAX_FILES: usize = 100_000;
const PROGRESS_INTERVAL: usize = 500;

#[derive(Debug, Clone)]
pub struct ScanOptions {
    pub recursive: bool,
    pub max_depth: MaxDepth,
    pub exclude_folders: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

impl ScanOptions {
    pub fn for_folder(folder: Option<&Folder>, recursive_requested: bool) -> Self {
        let recursive =
            recursive_requested && folder.is_some_and(|folder| folder.subfolders.enabled);
        Self {
            recursive,
            max_depth: folder
                .map(|folder| folder.subfolders.max_depth)
                .unwrap_or(MaxDepth::Level(1)),
            exclude_folders: folder
                .map(|folder| folder.exclude_folders.clone())
                .unwrap_or_default(),
            exclude_patterns: folder
                .map(|folder| folder.exclude_patterns.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub scanned: u32,
    pub errors: u32,
    pub truncated: bool,
    pub cancelled: bool,
}

#[derive(Debug)]
pub struct ScanOutput {
    pub files: Vec<FileItem>,
    pub stats: ScanStats,
}

pub fn scan(
    root: &Path,
    options: &ScanOptions,
    mut is_cancelled: impl FnMut() -> bool,
    mut on_progress: impl FnMut(&ScanStats),
) -> Result<ScanOutput> {
    let root = crate::paths::safety::validate_scan_root(root)?;
    let filter = PathFilter::new(&root, &options.exclude_folders, &options.exclude_patterns)
        .map_err(|error| anyhow!(error.to_string()))?;
    let max_depth = if options.recursive {
        match options.max_depth {
            MaxDepth::Unlimited => usize::MAX,
            MaxDepth::Level(level) => usize::try_from(level)
                .unwrap_or(usize::MAX)
                .saturating_add(1),
        }
    } else {
        1
    };
    let walker = WalkDir::new(&root)
        .follow_links(false)
        .max_depth(max_depth)
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !filter.is_ignored(entry.path()));

    let mut files = Vec::new();
    let mut stats = ScanStats::default();
    for entry in walker {
        if is_cancelled() {
            stats.cancelled = true;
            break;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                tracing::warn!(%error, "scan entry failed");
                stats.errors = stats.errors.saturating_add(1);
                continue;
            }
        };
        if entry.depth() == 0 || !entry.file_type().is_file() || filter.is_ignored(entry.path()) {
            continue;
        }
        if entry.path_is_symlink() {
            continue;
        }
        if files.len() >= MAX_FILES {
            stats.truncated = true;
            break;
        }
        let path: PathBuf = entry.into_path();
        let Some(path) = path.to_str() else {
            stats.errors = stats.errors.saturating_add(1);
            continue;
        };
        match FileItem::from_path_sync(path) {
            Ok(file) => {
                files.push(file);
                stats.scanned = stats.scanned.saturating_add(1);
            }
            Err(error) => {
                tracing::warn!(?path, %error, "could not inspect scanned file");
                stats.errors = stats.errors.saturating_add(1);
            }
        }
        if (stats.scanned as usize + stats.errors as usize) % PROGRESS_INTERVAL == 0 {
            on_progress(&stats);
        }
    }
    on_progress(&stats);
    Ok(ScanOutput { files, stats })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn non_recursive_scan_only_returns_root_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("watched");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("root.txt"), b"root").unwrap();
        fs::write(root.join("nested/child.txt"), b"child").unwrap();
        let options = ScanOptions::for_folder(None, false);
        let output = scan(&root, &options, || false, |_| {}).unwrap();
        assert_eq!(output.files.len(), 1);
        assert_eq!(output.files[0].name, "root.txt");
    }
}
