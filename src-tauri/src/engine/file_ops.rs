//! Crash-conscious file operations used by organize and undo.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use uuid::Uuid;

use crate::engine::cancellation::CancellationToken;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictBehavior {
    Rename,
    Overwrite,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateBehavior {
    KeepBoth,
    Trash,
    Delete,
}

impl DuplicateBehavior {
    pub fn from_str(value: &str) -> Self {
        match value {
            "trash" => Self::Trash,
            "delete" => Self::Delete,
            _ => Self::KeepBoth,
        }
    }
}

impl ConflictBehavior {
    pub fn from_str(value: &str) -> Self {
        match value {
            "overwrite" => Self::Overwrite,
            "skip" => Self::Skip,
            _ => Self::Rename,
        }
    }
}

#[derive(Debug)]
pub enum MoveOutcome {
    Moved(PathBuf),
    Skipped,
}

#[derive(Debug)]
pub enum PrepareMoveOutcome {
    Ready(PreparedMove),
    Skipped,
}

#[derive(Debug)]
pub struct PreparedMove {
    source: PathBuf,
    destination: PathBuf,
    behavior: ConflictBehavior,
    overwrite_backup: Option<PathBuf>,
    had_destination: bool,
}

impl PreparedMove {
    pub fn source(&self) -> &Path {
        &self.source
    }

    pub fn destination(&self) -> &Path {
        &self.destination
    }

    pub fn behavior(&self) -> ConflictBehavior {
        self.behavior
    }

    pub fn overwrite_backup(&self) -> Option<&Path> {
        self.overwrite_backup.as_deref()
    }

    pub fn had_destination(&self) -> bool {
        self.had_destination
    }
}

pub fn prepare_move(
    source: &Path,
    requested_destination: &Path,
    behavior: ConflictBehavior,
) -> Result<PrepareMoveOutcome> {
    ensure_regular_file(source, "SOURCE_FILE_INVALID")?;
    let parent = requested_destination
        .parent()
        .ok_or_else(|| anyhow!("INVALID_DESTINATION"))?;
    let parent_metadata =
        fs::symlink_metadata(parent).map_err(|_| anyhow!("INVALID_DESTINATION"))?;
    if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
        return Err(anyhow!("INVALID_DESTINATION"));
    }

    let requested_destination_exists = path_exists(requested_destination)?;
    if requested_destination_exists {
        ensure_regular_file(requested_destination, "DESTINATION_UNSAFE")?;
    }

    let destination = match (behavior, requested_destination_exists) {
        (ConflictBehavior::Skip, true) => return Ok(PrepareMoveOutcome::Skipped),
        (ConflictBehavior::Rename, true) => available_renamed_path(requested_destination)?,
        _ => requested_destination.to_path_buf(),
    };
    if destination == source {
        return Err(anyhow!("SOURCE_EQUALS_DESTINATION"));
    }
    let overwrite_backup =
        if behavior == ConflictBehavior::Overwrite && requested_destination_exists {
            Some(unique_managed_path(
                requested_destination,
                "overwrite-backup",
            ))
        } else {
            None
        };

    Ok(PrepareMoveOutcome::Ready(PreparedMove {
        source: source.to_path_buf(),
        destination,
        behavior,
        overwrite_backup,
        // This flag describes the selected destination, not whether the
        // originally requested name happened to be occupied. Rename mode
        // chooses a fresh path and therefore never needs an overwrite backup.
        had_destination: behavior == ConflictBehavior::Overwrite && requested_destination_exists,
    }))
}

pub fn execute_prepared_move(prepared: PreparedMove) -> Result<MoveOutcome> {
    execute_prepared_move_cancellable(prepared, None)
}

pub fn execute_prepared_move_cancellable(
    prepared: PreparedMove,
    cancellation: Option<CancellationToken>,
) -> Result<MoveOutcome> {
    if is_cancelled(cancellation.as_ref()) {
        return Err(anyhow!("MOVE_CANCELLED"));
    }
    ensure_regular_file(&prepared.source, "SOURCE_FILE_INVALID")?;

    let backup_published = if let Some(backup) = prepared.overwrite_backup.as_deref() {
        ensure_regular_file(&prepared.destination, "DESTINATION_UNSAFE")?;
        // The backup lives beside the destination, so rename is atomic and
        // also works on removable Linux filesystems that lack hard links.
        fs::rename(&prepared.destination, backup).context("OVERWRITE_BACKUP_FAILED")?;
        sync_parent(&prepared.destination);
        true
    } else {
        false
    };

    if is_cancelled(cancellation.as_ref()) {
        rollback_after_failure(&prepared, false, backup_published)?;
        return Err(anyhow!("MOVE_CANCELLED"));
    }

    let move_result = publish_then_remove_source(
        &prepared.source,
        &prepared.destination,
        cancellation.as_ref(),
    );
    match move_result {
        Ok(()) => Ok(MoveOutcome::Moved(prepared.destination)),
        Err(error) => {
            let destination_is_ours =
                same_file::is_same_file(&prepared.source, &prepared.destination).unwrap_or(false);
            if let Err(restore_error) =
                rollback_after_failure(&prepared, destination_is_ours, backup_published)
            {
                return Err(anyhow!(
                    "DESTINATION_RESTORE_FAILED: {restore_error}; original error: {error}"
                ));
            }
            Err(error)
        }
    }
}

pub fn move_file(
    source: &Path,
    destination: &Path,
    behavior: ConflictBehavior,
) -> Result<MoveOutcome> {
    match prepare_move(source, destination, behavior)? {
        PrepareMoveOutcome::Ready(prepared) => execute_prepared_move(prepared),
        PrepareMoveOutcome::Skipped => Ok(MoveOutcome::Skipped),
    }
}

pub fn trash_file(path: &Path) -> Result<()> {
    ensure_regular_file(path, "SOURCE_FILE_INVALID")?;
    trash::delete(path).map_err(|error| anyhow!("TRASH_FAILED: {error}"))
}

/// Compare two regular files byte-for-byte. Equal size alone is never enough
/// to classify a file as a duplicate.
#[cfg(test)]
pub fn files_identical(left: &Path, right: &Path) -> Result<bool> {
    files_identical_cancellable(left, right, None)
}

pub fn files_identical_cancellable(
    left: &Path,
    right: &Path,
    cancellation: Option<&CancellationToken>,
) -> Result<bool> {
    ensure_regular_file(left, "SOURCE_FILE_INVALID")?;
    ensure_regular_file(right, "DESTINATION_UNSAFE")?;
    if fs::metadata(left)?.len() != fs::metadata(right)?.len() {
        return Ok(false);
    }

    let mut left = File::open(left)?;
    let mut right = File::open(right)?;
    let mut left_buffer = [0u8; 64 * 1024];
    let mut right_buffer = [0u8; 64 * 1024];
    loop {
        if is_cancelled(cancellation) {
            return Err(anyhow!("MOVE_CANCELLED"));
        }
        let left_read = left.read(&mut left_buffer)?;
        let right_read = right.read(&mut right_buffer)?;
        if left_read != right_read || left_buffer[..left_read] != right_buffer[..right_read] {
            return Ok(false);
        }
        if left_read == 0 {
            return Ok(true);
        }
    }
}

/// Restore an overwrite backup without ever replacing an unrelated path.
pub fn restore_retained_backup(backup: &Path, destination: &Path) -> Result<()> {
    ensure_regular_file(backup, "UNDO_OVERWRITE_BACKUP_MISSING")?;
    if path_exists(destination)? {
        return Err(anyhow!("UNDO_DESTINATION_ALREADY_EXISTS"));
    }
    match publish_hard_link(backup, destination) {
        Ok(()) => {
            if let Err(error) = fs::remove_file(backup) {
                let _ = fs::remove_file(destination);
                return Err(anyhow!("DESTINATION_RESTORE_FAILED: {error}"));
            }
        }
        Err(error)
            if error.kind() != std::io::ErrorKind::AlreadyExists && !destination.exists() =>
        {
            copy_without_overwrite(backup, destination).context("DESTINATION_RESTORE_FAILED")?;
            if let Err(error) = fs::remove_file(backup) {
                let _ = fs::remove_file(destination);
                return Err(anyhow!("DESTINATION_RESTORE_FAILED: {error}"));
            }
        }
        Err(error) => return Err(anyhow!("DESTINATION_RESTORE_FAILED: {error}")),
    }
    sync_parent(destination);
    Ok(())
}

fn publish_then_remove_source(
    source: &Path,
    destination: &Path,
    cancellation: Option<&CancellationToken>,
) -> Result<()> {
    match fs::hard_link(source, destination) {
        Ok(()) => {
            sync_parent(destination);
            if is_cancelled(cancellation) {
                fs::remove_file(destination).context("MOVE_ROLLBACK_FAILED")?;
                return Err(anyhow!("MOVE_CANCELLED"));
            }
            fs::remove_file(source).context("MOVE_SOURCE_REMOVE_FAILED")?;
            sync_parent(source);
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(anyhow!("DESTINATION_ALREADY_EXISTS"))
        }
        Err(_) => copy_publish_then_remove(source, destination, cancellation),
    }
}

fn copy_publish_then_remove(
    source: &Path,
    destination: &Path,
    cancellation: Option<&CancellationToken>,
) -> Result<()> {
    let temp = unique_managed_path(destination, "move-temp");
    let result = (|| -> Result<()> {
        let mut input = File::open(source).context("MOVE_SOURCE_OPEN_FAILED")?;
        let source_metadata = input.metadata().context("MOVE_SOURCE_METADATA_FAILED")?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .context("MOVE_TEMP_CREATE_FAILED")?;
        let mut buffer = vec![0u8; 1024 * 1024];
        loop {
            if is_cancelled(cancellation) {
                return Err(anyhow!("MOVE_CANCELLED"));
            }
            let read = input.read(&mut buffer).context("MOVE_COPY_READ_FAILED")?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .context("MOVE_COPY_WRITE_FAILED")?;
        }
        output.sync_all().context("MOVE_COPY_SYNC_FAILED")?;
        fs::set_permissions(&temp, source_metadata.permissions())
            .context("MOVE_COPY_PERMISSIONS_FAILED")?;
        if is_cancelled(cancellation) {
            return Err(anyhow!("MOVE_CANCELLED"));
        }
        // The temporary file lives beside the destination, so this link is
        // same-filesystem and refuses to overwrite a racing external file.
        publish_hard_link(&temp, destination).context("MOVE_PUBLISH_FAILED")?;
        fs::remove_file(&temp).context("MOVE_TEMP_REMOVE_FAILED")?;
        sync_parent(destination);
        if is_cancelled(cancellation) {
            fs::remove_file(destination).context("MOVE_ROLLBACK_FAILED")?;
            return Err(anyhow!("MOVE_CANCELLED"));
        }
        if let Err(error) = fs::remove_file(source) {
            if let Err(rollback_error) = fs::remove_file(destination) {
                return Err(anyhow!(
                    "DESTINATION_RESTORE_FAILED: {rollback_error}; source removal: {error}"
                ));
            }
            return Err(anyhow!("MOVE_SOURCE_REMOVE_FAILED: {error}"));
        }
        sync_parent(source);
        Ok(())
    })();
    if temp.exists() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn rollback_after_failure(
    prepared: &PreparedMove,
    remove_destination: bool,
    backup_published: bool,
) -> Result<()> {
    if remove_destination && prepared.destination.exists() {
        fs::remove_file(&prepared.destination).context("MOVE_ROLLBACK_FAILED")?;
    }
    if backup_published {
        let backup = prepared
            .overwrite_backup
            .as_deref()
            .ok_or_else(|| anyhow!("OVERWRITE_BACKUP_MISSING"))?;
        if prepared.destination.exists() {
            return Err(anyhow!("DESTINATION_OCCUPIED_DURING_RESTORE"));
        }
        restore_retained_backup(backup, &prepared.destination)?;
    }
    Ok(())
}

fn publish_hard_link(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::hard_link(source, destination)
}

fn copy_without_overwrite(source: &Path, destination: &Path) -> Result<()> {
    let mut input = File::open(source)?;
    let metadata = input.metadata()?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    std::io::copy(&mut input, &mut output)?;
    output.sync_all()?;
    fs::set_permissions(destination, metadata.permissions())?;
    Ok(())
}

fn ensure_regular_file(path: &Path, code: &'static str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| anyhow!(code))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(anyhow!(code));
    }
    Ok(())
}

fn path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn available_renamed_path(requested: &Path) -> Result<PathBuf> {
    let parent = requested
        .parent()
        .ok_or_else(|| anyhow!("INVALID_DESTINATION"))?;
    let stem = requested
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("INVALID_DESTINATION"))?;
    let extension = requested.extension().and_then(|value| value.to_str());
    for index in 1..=100_000u32 {
        let name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let candidate = parent.join(name);
        if !path_exists(&candidate)? {
            return Ok(candidate);
        }
    }
    Err(anyhow!("DESTINATION_NAME_EXHAUSTED"))
}

fn unique_managed_path(destination: &Path, kind: &str) -> PathBuf {
    let name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    destination.with_file_name(format!(".{name}.filegrove-{kind}-{}", Uuid::new_v4()))
}

fn is_cancelled(cancellation: Option<&CancellationToken>) -> bool {
    cancellation.is_some_and(CancellationToken::is_cancelled)
}

fn sync_parent(path: &Path) {
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_conflicts_preserve_both_files() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("report.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&destination, b"old").unwrap();
        let outcome = move_file(&source, &destination, ConflictBehavior::Rename).unwrap();
        let MoveOutcome::Moved(actual) = outcome else {
            panic!("move was skipped")
        };
        assert_eq!(fs::read(destination).unwrap(), b"old");
        assert_eq!(fs::read(actual).unwrap(), b"new");
        assert!(!source.exists());
    }

    #[test]
    fn renamed_conflict_is_not_recorded_as_an_overwrite() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source.txt");
        let destination = temp.path().join("report.txt");
        fs::write(&source, b"new").unwrap();
        fs::write(&destination, b"old").unwrap();

        let PrepareMoveOutcome::Ready(prepared) =
            prepare_move(&source, &destination, ConflictBehavior::Rename).unwrap()
        else {
            panic!("rename conflict was skipped")
        };

        assert_eq!(prepared.destination(), temp.path().join("report (1).txt"));
        assert!(!prepared.had_destination());
        assert!(prepared.overwrite_backup().is_none());
    }

    #[test]
    fn duplicate_detection_compares_complete_content() {
        let temporary = tempfile::tempdir().unwrap();
        let left = temporary.path().join("left.bin");
        let right = temporary.path().join("right.bin");
        fs::write(&left, b"same bytes").unwrap();
        fs::write(&right, b"same bytes").unwrap();
        assert!(files_identical(&left, &right).unwrap());

        fs::write(&right, b"other data").unwrap();
        assert!(!files_identical(&left, &right).unwrap());

        fs::write(&right, b"short").unwrap();
        assert!(!files_identical(&left, &right).unwrap());
    }
}
