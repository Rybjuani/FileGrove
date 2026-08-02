//! Folder repository.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::domain::{
    Action, Condition, Folder, FolderMode, Rule, SubfolderConfig, WatcherStatus, QUICK_RULES,
};

pub struct FolderRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl FolderRepo {
    pub fn list(&self) -> Result<Vec<Folder>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, path, name, mode, rule_ids, watcher_status, created_at, \
             subfolders, exclude_folders, exclude_patterns \
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
             subfolders, exclude_folders, exclude_patterns \
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
             subfolders, exclude_folders, exclude_patterns \
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
             created_at, subfolders, exclude_folders, exclude_patterns) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                folder.id.to_string(),
                folder.path,
                folder.name,
                folder.mode.as_str(),
                serde_json::to_string(&folder.rule_ids)?,
                folder.watcher_status.as_str(),
                folder.created_at.to_rfc3339(),
                serde_json::to_string(&folder.subfolders)?,
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
             subfolders=?7, exclude_folders=?8, exclude_patterns=?9 \
             WHERE id=?1",
            rusqlite::params![
                folder.id.to_string(),
                folder.path,
                folder.name,
                folder.mode.as_str(),
                serde_json::to_string(&folder.rule_ids)?,
                folder.watcher_status.as_str(),
                serde_json::to_string(&folder.subfolders)?,
                serde_json::to_string(&folder.exclude_folders)?,
                serde_json::to_string(&folder.exclude_patterns)?,
            ],
        )?;
        Ok(())
    }

    pub fn update_status(&self, id: Uuid, status: WatcherStatus) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE folders SET watcher_status = ?2 WHERE id = ?1",
            rusqlite::params![id.to_string(), status.as_str()],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute("DELETE FROM folders WHERE id = ?", [id.to_string()])?;
        Ok(())
    }

    /// Install the safe built-in move rules and assign them to one folder in
    /// a single transaction. Existing customized quick move rules are reused;
    /// a quick rule changed to a destructive action is never assigned here.
    pub fn apply_recommended_rules(&self, folder_id: Uuid, language: &str) -> Result<Folder> {
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let serialized_ids: String = transaction.query_row(
            "SELECT rule_ids FROM folders WHERE id = ?1",
            [folder_id.to_string()],
            |row| row.get(0),
        )?;
        let mut rule_ids: Vec<Uuid> = serde_json::from_str(&serialized_ids).unwrap_or_default();
        let mut next_order: i32 = transaction.query_row(
            "SELECT COALESCE(MAX(\"order\"), -1) + 1 FROM rules",
            [],
            |row| row.get(0),
        )?;

        for template in QUICK_RULES {
            let existing = {
                let mut statement = transaction.prepare(
                    "SELECT id, action FROM rules \
                     WHERE is_quick_rule = 1 AND icon = ?1 ORDER BY created_at ASC",
                )?;
                let rows = statement.query_map([template.icon], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?;
                let mut existing = None;
                for row in rows {
                    let (id, serialized_action) = row?;
                    let Ok(Action::Move { destination }) =
                        serde_json::from_str::<Action>(&serialized_action)
                    else {
                        continue;
                    };
                    if crate::paths::safety::relative_destination_root(&destination)
                        .ok()
                        .flatten()
                        .is_none()
                    {
                        continue;
                    }
                    if let Ok(id) = Uuid::parse_str(&id) {
                        existing = Some(id);
                        break;
                    }
                }
                existing
            };

            let rule_id = if let Some(rule_id) = existing {
                transaction.execute(
                    "UPDATE rules SET enabled = 1 WHERE id = ?1",
                    [rule_id.to_string()],
                )?;
                rule_id
            } else {
                let (name, destination) = localized_quick_rule(template.icon, language);
                let rule = Rule {
                    id: Uuid::new_v4(),
                    name: name.to_string(),
                    is_quick_rule: true,
                    enabled: true,
                    order: next_order,
                    icon: template.icon.to_string(),
                    conditions: vec![Condition::Extension {
                        value: template
                            .extensions
                            .iter()
                            .map(|extension| (*extension).to_string())
                            .collect(),
                    }],
                    action: Action::Move {
                        destination: destination.to_string(),
                    },
                    created_at: chrono::Utc::now(),
                };
                transaction.execute(
                    "INSERT INTO rules (id, name, is_quick_rule, enabled, \"order\", icon, conditions, action, created_at) \
                     VALUES (?1, ?2, 1, 1, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        rule.id.to_string(),
                        rule.name,
                        rule.order,
                        rule.icon,
                        serde_json::to_string(&rule.conditions)?,
                        serde_json::to_string(&rule.action)?,
                        rule.created_at.to_rfc3339(),
                    ],
                )?;
                next_order = next_order.saturating_add(1);
                rule.id
            };

            if !rule_ids.contains(&rule_id) {
                rule_ids.push(rule_id);
            }
        }

        transaction.execute(
            "UPDATE folders SET rule_ids = ?2 WHERE id = ?1",
            rusqlite::params![folder_id.to_string(), serde_json::to_string(&rule_ids)?],
        )?;
        transaction.commit()?;
        drop(conn);
        self.get(folder_id)?
            .ok_or_else(|| anyhow::anyhow!("FOLDER_NOT_FOUND"))
    }
}

fn localized_quick_rule(icon: &str, language: &str) -> (&'static str, &'static str) {
    if language == "es" {
        match icon {
            "Image" => ("Imágenes", "Imágenes"),
            "FileText" => ("Documentos", "Documentos"),
            "Video" => ("Vídeos", "Vídeos"),
            "Music" => ("Audio", "Audio"),
            "Archive" => ("Archivos comprimidos", "Archivos comprimidos"),
            "Download" => ("Instaladores", "Instaladores"),
            _ => ("Archivos", "Archivos"),
        }
    } else {
        match icon {
            "Image" => ("Images", "Images"),
            "FileText" => ("Documents", "Documents"),
            "Video" => ("Videos", "Videos"),
            "Music" => ("Audio", "Audio"),
            "Archive" => ("Archives", "Archives"),
            "Download" => ("Installers", "Installers"),
            _ => ("Files", "Files"),
        }
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
    let exclude_folders_str: String = row.get(8)?;
    let exclude_patterns_str: String = row.get(9)?;

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
        exclude_folders,
        exclude_patterns,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[test]
    fn recommended_rules_are_safe_localized_and_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(&directory.path().join("filegrove.db")).unwrap();
        database.run_migrations().unwrap();
        let folder = Folder {
            id: Uuid::new_v4(),
            path: directory
                .path()
                .join("watched")
                .to_string_lossy()
                .into_owned(),
            name: "Prueba".into(),
            mode: FolderMode::Single,
            rule_ids: vec![],
            watcher_status: WatcherStatus::Stopped,
            created_at: chrono::Utc::now(),
            subfolders: SubfolderConfig::default(),
            exclude_folders: vec![],
            exclude_patterns: vec![],
        };
        database.folders().insert(&folder).unwrap();

        let first = database
            .folders()
            .apply_recommended_rules(folder.id, "es")
            .unwrap();
        assert_eq!(first.rule_ids.len(), QUICK_RULES.len());
        let rules = database.rules().get_by_ids(&first.rule_ids).unwrap();
        assert_eq!(rules.len(), QUICK_RULES.len());
        assert!(rules.iter().all(|rule| {
            rule.enabled
                && rule.is_quick_rule
                && matches!(
                    &rule.action,
                    Action::Move { destination }
                        if crate::paths::safety::relative_destination_root(destination)
                            .ok()
                            .flatten()
                            .is_some()
                )
        }));
        assert!(rules.iter().any(|rule| rule.name == "Imágenes"));

        let second = database
            .folders()
            .apply_recommended_rules(folder.id, "es")
            .unwrap();
        assert_eq!(second.rule_ids, first.rule_ids);
        assert_eq!(database.rules().list().unwrap().len(), QUICK_RULES.len());
    }
}
