//! Folder repository.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::domain::{Folder, FolderMode, Schedule, SubfolderConfig, WatcherStatus};

pub struct FolderRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl FolderRepo {
    pub fn list(&self) -> Result<Vec<Folder>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, path, name, mode, rule_ids, watcher_status, created_at, \
             subfolders, schedule, exclude_folders, exclude_patterns \
             FROM folders ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map([], row_to_folder)?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get(&self, id: Uuid) -> Result<Option<Folder>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, path, name, mode, rule_ids, watcher_status, created_at, \
             subfolders, schedule, exclude_folders, exclude_patterns \
             FROM folders WHERE id = ?",
        )?;
        let mut rows = stmt.query_map([id.to_string()], row_to_folder)?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn get_by_path(&self, path: &str) -> Result<Option<Folder>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, path, name, mode, rule_ids, watcher_status, created_at, \
             subfolders, schedule, exclude_folders, exclude_patterns \
             FROM folders WHERE path = ?",
        )?;
        let mut rows = stmt.query_map([path], row_to_folder)?;
        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
    }

    pub fn insert(&self, folder: &Folder) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO folders (id, path, name, mode, rule_ids, watcher_status, \
             created_at, subfolders, schedule, exclude_folders, exclude_patterns) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            rusqlite::params![
                folder.id.to_string(),
                folder.path,
                folder.name,
                folder.mode.as_str(),
                serde_json::to_string(&folder.rule_ids)?,
                folder.watcher_status.as_str(),
                folder.created_at.to_rfc3339(),
                serde_json::to_string(&folder.subfolders)?,
                folder
                    .schedule
                    .as_ref()
                    .map(|s| serde_json::to_string(s).unwrap_or_default()),
                serde_json::to_string(&folder.exclude_folders)?,
                serde_json::to_string(&folder.exclude_patterns)?,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, folder: &Folder) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE folders SET path=?2, name=?3, mode=?4, rule_ids=?5, watcher_status=?6, \
             subfolders=?7, schedule=?8, exclude_folders=?9, exclude_patterns=?10 \
             WHERE id=?1",
            rusqlite::params![
                folder.id.to_string(),
                folder.path,
                folder.name,
                folder.mode.as_str(),
                serde_json::to_string(&folder.rule_ids)?,
                folder.watcher_status.as_str(),
                serde_json::to_string(&folder.subfolders)?,
                folder
                    .schedule
                    .as_ref()
                    .map(|s| serde_json::to_string(s).unwrap_or_default()),
                serde_json::to_string(&folder.exclude_folders)?,
                serde_json::to_string(&folder.exclude_patterns)?,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM folders WHERE id = ?", [id.to_string()])?;
        Ok(())
    }
}

fn row_to_folder(row: &rusqlite::Row<'_>) -> rusqlite::Result<Folder> {
    let id_str: String = row.get(0)?;
    let path: String = row.get(1)?;
    let name: String = row.get(2)?;
    let mode_str: String = row.get(3)?;
    let rule_ids_str: String = row.get(4)?;
    let status_str: String = row.get(5)?;
    let created_at_str: String = row.get(6)?;
    let subfolders_str: String = row.get(7)?;
    let schedule_str: Option<String> = row.get(8)?;
    let exclude_folders_str: String = row.get(9)?;
    let exclude_patterns_str: String = row.get(10)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let mode = FolderMode::from_str(&mode_str).unwrap_or(FolderMode::Single);
    let rule_ids: Vec<Uuid> = serde_json::from_str(&rule_ids_str).unwrap_or_default();
    let watcher_status = match status_str.as_str() {
        "running" => WatcherStatus::Running,
        "paused" => WatcherStatus::Paused,
        _ => WatcherStatus::Stopped,
    };
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());
    let subfolders: SubfolderConfig = serde_json::from_str(&subfolders_str).unwrap_or_default();
    let schedule: Option<Schedule> = schedule_str
        .as_ref()
        .and_then(|s| serde_json::from_str(s).ok());
    let exclude_folders: Vec<String> =
        serde_json::from_str(&exclude_folders_str).unwrap_or_default();
    let exclude_patterns: Vec<String> =
        serde_json::from_str(&exclude_patterns_str).unwrap_or_default();

    Ok(Folder {
        id,
        path,
        name,
        mode,
        rule_ids,
        watcher_status,
        created_at,
        subfolders,
        schedule,
        exclude_folders,
        exclude_patterns,
    })
}
