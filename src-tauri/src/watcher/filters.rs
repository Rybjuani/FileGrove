//! File filtering rules — see SPEC §10.3.
//!
//! Always ignore: hidden files (starting with `.`), macOS metadata files
//! (`._*`, `Icon\r`), `Thumbs.db`, `desktop.ini`, `.DS_Store`.

use std::path::Path;

/// True iff the file should be ignored by the watcher.
pub fn should_ignore(path: &Path) -> bool {
    let name = match path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return true,
    };

    if name.starts_with('.') {
        return true;
    }
    if name.starts_with("._") {
        return true;
    }
    matches!(name, ".DS_Store" | "Thumbs.db" | "desktop.ini" | "Icon\r")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ignores_hidden() {
        assert!(should_ignore(&PathBuf::from("/tmp/.hidden")));
        assert!(should_ignore(&PathBuf::from("/tmp/.DS_Store")));
    }

    #[test]
    fn ignores_macos_metadata() {
        assert!(should_ignore(&PathBuf::from("/tmp/._photo.jpg")));
        assert!(should_ignore(&PathBuf::from("/tmp/Icon\r")));
    }

    #[test]
    fn ignores_os_junk() {
        assert!(should_ignore(&PathBuf::from("/tmp/Thumbs.db")));
        assert!(should_ignore(&PathBuf::from("/tmp/desktop.ini")));
    }

    #[test]
    fn allows_regular_files() {
        assert!(!should_ignore(&PathBuf::from("/tmp/photo.jpg")));
        assert!(!should_ignore(&PathBuf::from("/tmp/report.pdf")));
    }
}
