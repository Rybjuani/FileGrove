//! Shared filtering for recursive scans and filesystem watcher events.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use glob::{MatchOptions, Pattern};
use thiserror::Error;

pub const DEFAULT_EXCLUDE_FOLDERS: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "dist",
    "build",
    ".next",
    ".turbo",
    ".parcel-cache",
    ".cache",
    "coverage",
    "target",
    ".gradle",
    ".idea",
    ".vscode",
    "vendor",
    "Pods",
    ".terraform",
    "venv",
];

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("INVALID_EXCLUDE_FOLDER")]
    InvalidExcludeFolder,
    #[error("INVALID_EXCLUDE_PATTERN: {0}")]
    InvalidExcludePattern(String),
}

/// Compiled per-folder excludes. Matching is intentionally case-insensitive
/// so a configuration behaves identically on Windows and portable drives.
#[derive(Clone)]
pub struct PathFilter {
    root: PathBuf,
    excluded_names: HashSet<String>,
    excluded_relative_paths: Vec<String>,
    patterns: Vec<Pattern>,
}

impl PathFilter {
    pub fn new(
        root: &Path,
        exclude_folders: &[String],
        exclude_patterns: &[String],
    ) -> Result<Self, FilterError> {
        // Callers that perform I/O already pass a validated canonical root.
        // Keeping the spelling supplied here also lets tests and watcher
        // events with non-extended Windows paths strip the prefix correctly.
        let root = root.to_path_buf();
        let mut excluded_names = DEFAULT_EXCLUDE_FOLDERS
            .iter()
            .map(|value| value.to_lowercase())
            .collect::<HashSet<_>>();
        let mut excluded_relative_paths = Vec::new();

        for configured in exclude_folders {
            let normalized = normalize_configured_folder(configured)?;
            if normalized.contains('/') {
                excluded_relative_paths.push(normalized.to_lowercase());
            } else {
                excluded_names.insert(normalized.to_lowercase());
            }
        }

        let mut patterns = Vec::with_capacity(exclude_patterns.len() + 1);
        patterns.push(Pattern::new(".Trash-*").expect("static glob is valid"));
        for configured in exclude_patterns {
            let configured = configured.trim();
            if configured.is_empty() {
                continue;
            }
            patterns.push(
                Pattern::new(&configured.replace('\\', "/"))
                    .map_err(|_| FilterError::InvalidExcludePattern(configured.to_string()))?,
            );
        }

        Ok(Self {
            root,
            excluded_names,
            excluded_relative_paths,
            patterns,
        })
    }

    /// True for a file or directory that must not be processed. Directory
    /// entries are passed through this method before traversal, pruning their
    /// entire subtree rather than merely hiding the files found below it.
    pub fn is_ignored(&self, path: &Path) -> bool {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        if relative.as_os_str().is_empty() {
            return false;
        }

        let components = relative
            .components()
            .filter_map(|component| match component {
                Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .collect::<Vec<_>>();
        if components.is_empty() {
            return true;
        }

        if components.iter().any(|name| is_builtin_ignored_name(name)) {
            return true;
        }
        if components
            .iter()
            .any(|name| self.excluded_names.contains(&name.to_lowercase()))
        {
            return true;
        }

        let normalized = components.join("/");
        let normalized_lower = normalized.to_lowercase();
        if self.excluded_relative_paths.iter().any(|excluded| {
            normalized_lower == *excluded || normalized_lower.starts_with(&format!("{excluded}/"))
        }) {
            return true;
        }

        let file_name = components.last().copied().unwrap_or_default();
        let options = MatchOptions {
            case_sensitive: false,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        };
        if self.patterns.iter().any(|pattern| {
            pattern.matches_with(&normalized, options) || pattern.matches_with(file_name, options)
        }) {
            return true;
        }

        // Never touch or traverse files inside a Git, Mercurial or SVN repository
        is_inside_repository(&self.root, path)
    }
}

fn is_inside_repository(root: &Path, path: &Path) -> bool {
    let mut current = path.parent();
    while let Some(dir) = current {
        if dir == root {
            break;
        }
        if dir.join(".git").exists() || dir.join(".hg").exists() || dir.join(".svn").exists() {
            return true;
        }
        current = dir.parent();
    }
    false
}

/// Backwards-compatible built-in filtering for callers without folder config.
#[cfg(test)]
pub fn should_ignore(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(is_builtin_ignored_name)
        .unwrap_or(true)
}

fn normalize_configured_folder(value: &str) -> Result<String, FilterError> {
    let value = value.trim().replace('\\', "/");
    if value.is_empty() {
        return Err(FilterError::InvalidExcludeFolder);
    }
    let path = Path::new(&value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(FilterError::InvalidExcludeFolder);
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");
    if normalized.is_empty() {
        Err(FilterError::InvalidExcludeFolder)
    } else {
        Ok(normalized)
    }
}

fn is_builtin_ignored_name(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with("._")
        || matches!(name, ".DS_Store" | "Thumbs.db" | "desktop.ini" | "Icon\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn ignores_hidden_and_os_metadata() {
        assert!(should_ignore(Path::new("/tmp/.hidden")));
        assert!(should_ignore(Path::new("/tmp/._photo.jpg")));
        assert!(should_ignore(Path::new("/tmp/Thumbs.db")));
        assert!(should_ignore(Path::new("/tmp/desktop.ini")));
        assert!(!should_ignore(Path::new("/tmp/photo.jpg")));
    }

    #[test]
    fn configured_folders_and_patterns_match_nested_paths() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        fs::create_dir(&root).unwrap();
        let filter = PathFilter::new(
            &root,
            &["private/cache".into(), "Generated".into()],
            &["*.tmp".into(), "exports/**/*.bak".into()],
        )
        .unwrap();

        assert!(filter.is_ignored(&root.join("node_modules/pkg/index.js")));
        assert!(filter.is_ignored(&root.join("PRIVATE/cache/a.txt")));
        assert!(filter.is_ignored(&root.join("nested/generated/a.txt")));
        assert!(filter.is_ignored(&root.join("nested/download.tmp")));
        assert!(filter.is_ignored(&root.join("exports/old/report.bak")));
        assert!(!filter.is_ignored(&root.join("private/other/report.pdf")));
    }

    #[test]
    fn rejects_traversal_in_folder_excludes() {
        let temp = tempfile::tempdir().unwrap();
        assert!(matches!(
            PathFilter::new(temp.path(), &["../outside".into()], &[]),
            Err(FilterError::InvalidExcludeFolder)
        ));
    }
}
