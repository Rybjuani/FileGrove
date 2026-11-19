//! Safe and fast file deduplication engine.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::paths::safety::validate_scan_root;
use crate::watcher::filters::PathFilter;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub modified_at: String,
    pub is_original: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub original: DuplicateFile,
    pub duplicates: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    pub groups: Vec<DuplicateGroup>,
    pub total_duplicates: usize,
    pub total_wasted_bytes: u64,
}

/// Scan a folder for duplicate files.
/// Fast two-stage detection:
/// 1. Group files by exact byte size (O(1) filter — unique sizes are never hashed).
/// 2. For files sharing the exact same size, stream SHA256 to confirm byte-for-byte identity.
pub fn scan_for_duplicates(
    folder_path: &Path,
    exclude_folders: &[String],
    exclude_patterns: &[String],
) -> Result<DuplicateReport> {
    let root = validate_scan_root(folder_path)?;
    let filter = PathFilter::new(&root, exclude_folders, exclude_patterns)
        .map_err(|e| anyhow!(e.to_string()))?;

    let mut files_by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    let walker = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            if entry.depth() == 0 {
                return true;
            }
            if filter.is_ignored(entry.path()) {
                return false;
            }
            if entry.file_type().is_dir()
                && (entry.path().join(".git").exists()
                    || entry.path().join(".hg").exists()
                    || entry.path().join(".svn").exists())
            {
                return false;
            }
            true
        });

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file()
            || entry.path_is_symlink()
            || filter.is_ignored(entry.path())
        {
            continue;
        }
        if let Ok(metadata) = entry.metadata() {
            let size = metadata.len();
            if size > 0 {
                files_by_size
                    .entry(size)
                    .or_default()
                    .push(entry.into_path());
            }
        }
    }

    let mut groups = Vec::new();
    let mut total_duplicates = 0usize;
    let mut total_wasted_bytes = 0u64;

    for (size, paths) in files_by_size {
        if paths.len() < 2 {
            continue;
        }

        let mut files_by_hash: HashMap<String, Vec<(PathBuf, std::time::SystemTime)>> =
            HashMap::new();

        for path in paths {
            if let Ok(hash) = compute_file_sha256(&path) {
                let mtime = std::fs::metadata(&path)
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                files_by_hash.entry(hash).or_default().push((path, mtime));
            }
        }

        for (hash, mut candidates) in files_by_hash {
            if candidates.len() < 2 {
                continue;
            }

            // Earliest modified or created is considered the original
            candidates.sort_by_key(|(_, mtime)| *mtime);

            let (original_path, original_mtime) = candidates.remove(0);
            let original_dt = chrono::DateTime::<Utc>::from(original_mtime);
            let original = DuplicateFile {
                path: dunce::canonicalize(&original_path)
                    .unwrap_or(original_path.clone())
                    .to_string_lossy()
                    .into_owned(),
                name: original_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
                size,
                modified_at: original_dt.to_rfc3339(),
                is_original: true,
            };

            let mut duplicates = Vec::new();
            for (dup_path, dup_mtime) in candidates {
                let dup_dt = chrono::DateTime::<Utc>::from(dup_mtime);
                duplicates.push(DuplicateFile {
                    path: dunce::canonicalize(&dup_path)
                        .unwrap_or(dup_path.clone())
                        .to_string_lossy()
                        .into_owned(),
                    name: dup_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string(),
                    size,
                    modified_at: dup_dt.to_rfc3339(),
                    is_original: false,
                });
                total_duplicates += 1;
                total_wasted_bytes += size;
            }

            groups.push(DuplicateGroup {
                hash,
                size,
                original,
                duplicates,
            });
        }
    }

    // Sort by largest wasted bytes first
    groups.sort_by(|a, b| {
        (b.size * b.duplicates.len() as u64).cmp(&(a.size * a.duplicates.len() as u64))
    });

    Ok(DuplicateReport {
        groups,
        total_duplicates,
        total_wasted_bytes,
    })
}

/// Compute SHA256 in 64 KB streaming chunks.
fn compute_file_sha256(path: &Path) -> Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_exact_duplicates_and_preserves_original() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("watched");
        fs::create_dir(&root).unwrap();

        fs::write(root.join("file1.txt"), b"duplicate content").unwrap();
        // Slightly later write
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(root.join("file2.txt"), b"duplicate content").unwrap();
        fs::write(root.join("unique.txt"), b"unique content").unwrap();

        let report = scan_for_duplicates(&root, &[], &[]).unwrap();
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.total_duplicates, 1);
        assert_eq!(report.groups[0].original.name, "file1.txt");
        assert_eq!(report.groups[0].duplicates[0].name, "file2.txt");
    }

    #[test]
    fn counts_all_copies_and_wasted_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("watched");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("a.txt"), b"same").unwrap();
        fs::write(root.join("b.txt"), b"same").unwrap();
        fs::write(root.join("c.txt"), b"same").unwrap();

        let report = scan_for_duplicates(&root, &[], &[]).unwrap();
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.total_duplicates, 2);
        assert_eq!(report.total_wasted_bytes, 8);
    }

    #[test]
    fn skips_files_inside_nested_repositories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("watched");
        let repo = root.join("project");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::write(root.join("keep.txt"), b"same").unwrap();
        fs::write(repo.join("ignored.txt"), b"same").unwrap();

        let report = scan_for_duplicates(&root, &[], &[]).unwrap();
        assert_eq!(report.groups.len(), 0);
    }

    #[test]
    fn rejects_a_repository_as_scan_root() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(root.join("a.txt"), b"same").unwrap();

        assert!(scan_for_duplicates(&root, &[], &[]).is_err());
    }
}
