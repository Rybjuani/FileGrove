//! Path canonicalization forensics for a watched root.
//!
//! Usage: `cargo run --example inspect_paths -- <watched-root>` (e.g. `F:\`)
//!
//! For the root and the first 3 regular files found under it, prints one line:
//!   raw | std canonicalize | dunce canonicalize | key | starts_with verdict
//! The verdict replicates `component_starts_with` (Windows flavor):
//! `key_file == key_root || key_file.starts_with(key_root + "\\")`.
//! Errors are printed as `ERR: <reason>` instead of panicking.

use std::path::{Path, PathBuf};

fn main() {
    let raw_root = match std::env::args().nth(1) {
        Some(root) => root,
        None => {
            println!("ERR: missing argv[1] (watched root)");
            return;
        }
    };
    let root = PathBuf::from(&raw_root);
    let root_key = path_key(&root);

    print_entry(&root, "ROOT", &root_key);
    let mut printed = 0;
    match std::fs::read_dir(&root) {
        Ok(entries) => {
            for entry in entries {
                if printed >= 3 {
                    break;
                }
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(error) => {
                        println!("ERR: read_dir entry: {error}");
                        continue;
                    }
                };
                let is_file = match entry.file_type() {
                    Ok(file_type) => file_type.is_file(),
                    Err(error) => {
                        println!("ERR: file_type ({}): {error}", entry.path().display());
                        continue;
                    }
                };
                if !is_file {
                    continue;
                }
                print_entry(&entry.path(), "FILE", &root_key);
                printed += 1;
            }
        }
        Err(error) => println!("ERR: read_dir: {error}"),
    }
    if printed == 0 {
        println!("ERR: no regular files found at root");
    }
}

/// Lowercase canonical path key, same fallback as `canonical_path_key`:
/// `dunce::canonicalize`, or lexical absolute when canonicalization fails.
#[cfg(windows)]
fn path_key(path: &Path) -> String {
    let resolved = dunce::canonicalize(path).unwrap_or_else(|_| lexical_absolute(path));
    resolved.to_string_lossy().to_lowercase()
}

#[cfg(not(windows))]
fn path_key(path: &Path) -> String {
    let resolved = dunce::canonicalize(path).unwrap_or_else(|_| lexical_absolute(path));
    resolved.to_string_lossy().into_owned()
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

fn print_entry(path: &Path, kind: &str, root_key: &str) {
    let std_canonical = match std::fs::canonicalize(path) {
        Ok(canonical) => format!("{}", canonical.display()),
        Err(error) => format!("ERR: {error}"),
    };
    let dunce_canonical = match dunce::canonicalize(path) {
        Ok(canonical) => format!("{}", canonical.display()),
        Err(error) => format!("ERR: {error}"),
    };
    let key = path_key(path);
    // Windows `component_starts_with`: exact key match or key prefix on a
    // component boundary (root key followed by a backslash).
    let verdict = format!("{}", key == root_key || key.starts_with(&format!("{root_key}\\")));
    println!("{kind}: {} | {std_canonical} | {dunce_canonical} | {key} | {verdict}", path.display());
}
