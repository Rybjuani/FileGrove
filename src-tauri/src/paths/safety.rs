//! Filesystem boundary checks shared by scanning and destructive actions.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context, Result};

pub fn canonical_path_key(path: &Path) -> String {
    let resolved = dunce::canonicalize(path).unwrap_or_else(|_| lexical_absolute(path));
    let key = resolved.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        key.to_lowercase()
    }
    #[cfg(not(windows))]
    {
        key
    }
}

pub fn paths_overlap(left: &Path, right: &Path) -> bool {
    let left = canonical_or_resolved(left).unwrap_or_else(|_| lexical_absolute(left));
    let right = canonical_or_resolved(right).unwrap_or_else(|_| lexical_absolute(right));
    component_starts_with(&left, &right) || component_starts_with(&right, &left)
}

pub fn validate_scan_root(path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(anyhow!("WATCHED_FOLDER_INVALID"));
    }
    reject_parent_components(path, "WATCHED_FOLDER_INVALID")?;
    let canonical = dunce::canonicalize(path).map_err(|_| anyhow!("WATCHED_FOLDER_INVALID"))?;
    let metadata = std::fs::metadata(&canonical).map_err(|_| anyhow!("WATCHED_FOLDER_INVALID"))?;
    if !metadata.is_dir() {
        return Err(anyhow!("WATCHED_FOLDER_INVALID"));
    }
    if is_protected_root(&canonical) {
        return Err(anyhow!("WATCHED_FOLDER_PROTECTED"));
    }
    if canonical.join(".git").exists()
        || canonical.join(".hg").exists()
        || canonical.join(".svn").exists()
    {
        return Err(anyhow!("WATCHED_FOLDER_IS_REPOSITORY"));
    }
    Ok(canonical)
}

pub fn validate_source_file(scan_root: &Path, source: &Path) -> Result<PathBuf> {
    let root = validate_scan_root(scan_root)?;
    if !source.is_absolute() {
        return Err(anyhow!("SOURCE_FILE_INVALID"));
    }
    reject_parent_components(source, "SOURCE_FILE_INVALID")?;
    let link_metadata =
        std::fs::symlink_metadata(source).map_err(|_| anyhow!("SOURCE_FILE_INVALID"))?;
    if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
        return Err(anyhow!("SOURCE_FILE_INVALID"));
    }
    let canonical = dunce::canonicalize(source).map_err(|_| anyhow!("SOURCE_FILE_INVALID"))?;
    if canonical == root || !component_starts_with(&canonical, &root) {
        return Err(anyhow!("SOURCE_FILE_OUTSIDE_WATCHED_FOLDER"));
    }
    Ok(canonical)
}

pub fn validate_destination_directory(scan_root: &Path, destination: &Path) -> Result<PathBuf> {
    if !destination.is_absolute() {
        return Err(anyhow!("INVALID_DESTINATION"));
    }
    reject_parent_components(destination, "INVALID_DESTINATION_TRAVERSAL")?;
    let root = validate_scan_root(scan_root)?;
    let managed_destination = component_starts_with(destination, &root);
    if managed_destination {
        reject_symlinks_below_root(&root, destination)?;
    }
    let resolved =
        canonical_or_resolved(destination).map_err(|_| anyhow!("INVALID_DESTINATION"))?;
    if destination.exists() {
        let metadata =
            std::fs::symlink_metadata(destination).map_err(|_| anyhow!("INVALID_DESTINATION"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(anyhow!("INVALID_DESTINATION"));
        }
    }
    if is_protected_root(&resolved) {
        return Err(anyhow!("INVALID_DESTINATION_PROTECTED"));
    }
    // A strict child of the watched folder is a supported managed output.
    // The root itself is never a destination because that would turn every
    // matching move into a no-op or a naming conflict.
    if resolved == root {
        return Err(anyhow!("INVALID_DESTINATION_WATCHED_FOLDER"));
    }
    if managed_destination && !component_starts_with(&resolved, &root) {
        return Err(anyhow!("INVALID_DESTINATION"));
    }
    Ok(resolved)
}

/// Return the stable first folder of a relative destination template.
/// Internal outputs require a literal first component so recursive scans and
/// watchers can exclude the entire generated tree before processing files.
pub fn relative_destination_root(template: &str) -> Result<Option<String>> {
    let template = template.trim();
    if template.is_empty() || template.contains('\0') {
        return Err(anyhow!("INVALID_DESTINATION"));
    }
    let path = Path::new(template);
    if path.is_absolute() || path.has_root() {
        reject_parent_components(path, "INVALID_DESTINATION_TRAVERSAL")?;
        return Ok(None);
    }
    let mut first = None;
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                if first.is_none() {
                    let value = value
                        .to_str()
                        .ok_or_else(|| anyhow!("INVALID_DESTINATION"))?;
                    if value.contains('{') || value.contains('}') {
                        return Err(anyhow!("INVALID_DESTINATION_DYNAMIC_ROOT"));
                    }
                    first = Some(value.to_string());
                }
            }
            Component::CurDir
            | Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_) => {
                return Err(anyhow!("INVALID_DESTINATION_TRAVERSAL"));
            }
        }
    }
    first
        .map(Some)
        .ok_or_else(|| anyhow!("INVALID_DESTINATION"))
}

pub fn validate_destination_outside_watched_folders<'a>(
    destination: &Path,
    watched_folders: impl IntoIterator<Item = &'a Path>,
) -> Result<()> {
    let destination =
        canonical_or_resolved(destination).map_err(|_| anyhow!("INVALID_DESTINATION"))?;
    for watched in watched_folders {
        let watched =
            canonical_or_resolved(watched).map_err(|_| anyhow!("WATCHED_FOLDER_INVALID"))?;
        if component_starts_with(&destination, &watched) {
            return Err(anyhow!("INVALID_DESTINATION_WATCHED_FOLDER"));
        }
    }
    Ok(())
}

fn reject_parent_components(path: &Path, code: &'static str) -> Result<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        Err(anyhow!(code))
    } else {
        Ok(())
    }
}

fn reject_symlinks_below_root(root: &Path, destination: &Path) -> Result<()> {
    let relative = destination
        .strip_prefix(root)
        .map_err(|_| anyhow!("INVALID_DESTINATION"))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(anyhow!("INVALID_DESTINATION"));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(_) => return Err(anyhow!("INVALID_DESTINATION")),
        }
    }
    Ok(())
}

fn canonical_or_resolved(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return dunce::canonicalize(path).with_context(|| format!("canonicalizing {path:?}"));
    }
    let mut missing = Vec::<OsString>::new();
    let mut ancestor = path;
    while !ancestor.exists() {
        let name = ancestor
            .file_name()
            .ok_or_else(|| anyhow!("path has no existing ancestor"))?;
        missing.push(name.to_os_string());
        ancestor = ancestor
            .parent()
            .ok_or_else(|| anyhow!("path has no existing ancestor"))?;
    }
    let mut resolved = dunce::canonicalize(ancestor)?;
    for component in missing.into_iter().rev() {
        resolved.push(component);
    }
    Ok(resolved)
}

fn lexical_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(windows)]
fn component_starts_with(path: &Path, base: &Path) -> bool {
    canonical_path_key(path) == canonical_path_key(base)
        || canonical_path_key(path).starts_with(&format!("{}\\", canonical_path_key(base)))
}

#[cfg(not(windows))]
fn component_starts_with(path: &Path, base: &Path) -> bool {
    path == base || path.starts_with(base)
}

fn is_protected_root(path: &Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::Prefix;
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            if matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
                && components.next() == Some(Component::RootDir)
                && components.next().is_none()
            {
                return true;
            }
        }
        false
    }
    #[cfg(not(windows))]
    {
        const PROTECTED_TREES: &[&str] = &[
            "/bin", "/boot", "/dev", "/etc", "/lib", "/lib64", "/proc", "/root", "/run", "/sbin",
            "/sys", "/usr", "/var",
        ];
        if path == Path::new("/")
            || path == Path::new("/home")
            || PROTECTED_TREES
                .iter()
                .any(|protected| path.starts_with(Path::new(protected)))
        {
            return true;
        }
        directories::BaseDirs::new().is_some_and(|dirs| path == dirs.home_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn accepts_managed_children_but_rejects_root_and_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("watched");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("file.txt"), b"data").unwrap();
        assert!(validate_source_file(&root, &root.join("file.txt")).is_ok());
        assert_eq!(
            validate_destination_directory(&root, &root.join("sorted")).unwrap(),
            root.join("sorted")
        );
        assert!(validate_destination_directory(&root, &root).is_err());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("file.txt"), root.join("link.txt")).unwrap();
            assert!(validate_source_file(&root, &root.join("link.txt")).is_err());
            fs::create_dir(root.join("real-destination")).unwrap();
            std::os::unix::fs::symlink(
                root.join("real-destination"),
                root.join("linked-destination"),
            )
            .unwrap();
            assert!(
                validate_destination_directory(&root, &root.join("linked-destination")).is_err()
            );
            assert!(validate_destination_directory(
                &root,
                &root.join("linked-destination").join("nested")
            )
            .is_err());
        }
    }

    #[test]
    fn relative_destinations_require_a_static_safe_root() {
        assert_eq!(
            relative_destination_root("Images/{year}/{month}").unwrap(),
            Some("Images".to_string())
        );
        assert_eq!(relative_destination_root("/tmp/Images").unwrap(), None);
        assert!(relative_destination_root("{year}/Images").is_err());
        assert!(relative_destination_root("../Images").is_err());
        assert!(relative_destination_root(".").is_err());
    }
}
