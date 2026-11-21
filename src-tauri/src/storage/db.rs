//! SQLite database wrapper + refinery migrations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use super::activity::ActivityRepo;
use super::command_ledger::CommandLedgerRepo;
use super::folders::FolderRepo;
use super::rules::RuleRepo;
use super::settings::SettingsRepo;
use super::undo::UndoRepo;

refinery::embed_migrations!("src/storage/migrations");

/// The single shared database handle. All repos borrow from it.
pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    /// Open the database at `<app_data_dir>/app.db`, creating it if missing.
    pub fn open_default(app: &AppHandle) -> Result<Self> {
        let dir = app
            .path()
            .app_data_dir()
            .context("app_data_dir not available")?;
        std::fs::create_dir_all(&dir).ok();
        let path: PathBuf = dir.join("app.db");
        Self::open(&path)
    }

    pub fn open(path: &PathBuf) -> Result<Self> {
        tracing::info!(?path, "opening database");
        let conn =
            Connection::open(path).with_context(|| format!("opening sqlite at {:?}", path))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; \
             PRAGMA foreign_keys = ON; \
             PRAGMA synchronous = FULL;",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Run all embedded migrations.
    pub fn run_migrations(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        migrations::runner()
            .run(&mut *conn)
            .map_err(|e| anyhow::anyhow!("migration failed: {e}"))?;
        migrate_duplicate_quick_rules(&mut conn)?;
        tracing::info!("migrations applied");
        Ok(())
    }

    pub fn settings(&self) -> SettingsRepo {
        SettingsRepo {
            conn: self.conn.clone(),
        }
    }
    pub fn folders(&self) -> FolderRepo {
        FolderRepo {
            conn: self.conn.clone(),
        }
    }
    pub fn rules(&self) -> RuleRepo {
        RuleRepo {
            conn: self.conn.clone(),
        }
    }
    pub fn activity(&self) -> ActivityRepo {
        ActivityRepo {
            conn: self.conn.clone(),
        }
    }
    pub fn command_ledger(&self) -> CommandLedgerRepo {
        CommandLedgerRepo {
            conn: self.conn.clone(),
        }
    }
    pub fn undo(&self) -> UndoRepo {
        UndoRepo {
            conn: self.conn.clone(),
        }
    }
}

/// Repair databases created by releases that seeded the same built-in rules
/// with fresh UUIDs whenever a folder received recommended rules.
fn migrate_duplicate_quick_rules(conn: &mut Connection) -> Result<()> {
    use crate::domain::Condition;
    use crate::domain::QUICK_RULES;
    use std::collections::{BTreeSet, HashMap};

    let tx = conn.transaction()?;
    let rows = {
        let mut stmt = tx.prepare(
            "SELECT id, name, is_quick_rule, icon, conditions, action FROM rules ORDER BY created_at, id",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };
    // First associate every legacy built-in row with its template. The quick
    // flag and icon identify current built-ins; names and extension sets catch
    // older rows that lost the flag or icon. Neither destination nor locale is
    // part of the identity.
    let mut groups: HashMap<usize, Vec<(String, String)>> = HashMap::new();
    for (id, name, is_quick_rule, icon, conditions_json, _action_json) in rows {
        let conditions = serde_json::from_str::<Vec<Condition>>(&conditions_json).ok();
        let extension_set: BTreeSet<String> = conditions
            .as_ref()
            .into_iter()
            .flatten()
            .filter_map(|condition| match condition {
                Condition::Extension { value } => Some(value),
                _ => None,
            })
            .flatten()
            .map(|extension| extension.trim().to_lowercase())
            .collect();

        let canonical_index = QUICK_RULES.iter().position(|template| {
            crate::storage::folders::quick_rule_id(template.id).to_string() == id
        });
        let icon_index = QUICK_RULES
            .iter()
            .position(|template| template.icon == icon);
        let normalized_name = normalize_builtin_name(&name);
        let name_index = QUICK_RULES.iter().position(|template| {
            let (spanish_name, spanish_destination) =
                crate::storage::folders::localized_quick_rule(template.icon, "es");
            [
                template.name,
                template.default_destination,
                spanish_name,
                spanish_destination,
            ]
            .iter()
            .any(|candidate| normalize_builtin_name(candidate) == normalized_name)
        });
        let extension_index = QUICK_RULES.iter().position(|template| {
            let expected: BTreeSet<String> = template
                .extensions
                .iter()
                .map(|extension| extension.to_lowercase())
                .collect();
            !extension_set.is_empty() && extension_set == expected
        });

        let index = canonical_index
            .or_else(|| (is_quick_rule != 0).then_some(icon_index).flatten())
            .or(name_index)
            .or(extension_index)
            .or_else(|| {
                // A flagged legacy rule can have a changed icon and a slightly
                // changed extension list. Use the unique strongest overlap.
                if is_quick_rule == 0 || extension_set.is_empty() {
                    return None;
                }
                let mut scores: Vec<(usize, usize)> = QUICK_RULES
                    .iter()
                    .enumerate()
                    .map(|(index, template)| {
                        let overlap = template
                            .extensions
                            .iter()
                            .filter(|extension| extension_set.contains(**extension))
                            .count();
                        (index, overlap)
                    })
                    .filter(|(_, overlap)| *overlap > 0)
                    .collect();
                scores.sort_by_key(|(_, overlap)| std::cmp::Reverse(*overlap));
                if scores.first().is_some_and(|best| {
                    scores.get(1).map_or(true, |runner_up| best.1 > runner_up.1)
                }) {
                    scores.first().map(|(index, _)| *index)
                } else {
                    None
                }
            });
        if let Some(index) = index {
            groups.entry(index).or_default().push((id, name));
        }
    }

    let mut replacements = HashMap::new();
    let mut canonical_rows = Vec::new();
    for (index, rows) in groups {
        let template = &QUICK_RULES[index];
        let canonical_id = crate::storage::folders::quick_rule_id(template.id).to_string();
        let survivor = rows
            .iter()
            .find(|(id, _)| id == &canonical_id)
            .map(|(id, _)| id.clone())
            .or_else(|| rows.first().map(|(id, _)| id.clone()))
            .expect("a built-in group has at least one row");
        canonical_rows.push((survivor.clone(), canonical_id.clone(), template.icon));
        for (id, _) in rows {
            if id != canonical_id {
                replacements.insert(id, canonical_id.clone());
            }
        }
    }

    // Move the chosen survivor onto the stable UUID. If a canonical row already
    // exists it remains authoritative; otherwise the oldest legacy row keeps
    // its enabled state, conditions, destination and ordering.
    for (survivor, canonical_id, icon) in &canonical_rows {
        if survivor != canonical_id {
            tx.execute(
                "UPDATE rules SET id = ?1, is_quick_rule = 1, icon = ?2 WHERE id = ?3",
                rusqlite::params![canonical_id, icon, survivor],
            )?;
        } else {
            tx.execute(
                "UPDATE rules SET is_quick_rule = 1, icon = ?2 WHERE id = ?1",
                rusqlite::params![canonical_id, icon],
            )?;
        }
    }

    if !replacements.is_empty() {
        let folders = {
            let mut stmt = tx.prepare("SELECT id, rule_ids FROM folders")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        for (folder_id, serialized) in folders {
            let Ok(mut ids) = serde_json::from_str::<Vec<String>>(&serialized) else {
                continue;
            };
            for id in &mut ids {
                if let Some(replacement) = replacements.get(id) {
                    *id = replacement.clone();
                }
            }
            let mut unique = BTreeSet::new();
            ids.retain(|id| unique.insert(id.clone()));
            tx.execute(
                "UPDATE folders SET rule_ids = ?1 WHERE id = ?2",
                rusqlite::params![serde_json::to_string(&ids)?, folder_id],
            )?;
        }
        for duplicate_id in replacements.keys() {
            tx.execute("DELETE FROM rules WHERE id = ?1", [duplicate_id])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn normalize_builtin_name(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .replace("ã¡", "a")
        .replace("ã©", "e")
        .replace("ã­", "i")
        .replace("ã³", "o")
        .replace("ãº", "u")
        .replace("ã±", "n")
        .replace('á', "a")
        .replace('é', "e")
        .replace('í', "i")
        .replace('ó', "o")
        .replace('ú', "u")
        .replace('ñ', "n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Condition;

    #[test]
    fn duplicate_quick_rule_migration_keeps_folder_assignments() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(&directory.path().join("duplicates.db")).unwrap();
        database.run_migrations().unwrap();
        let connection = database.conn.lock();
        let extensions = r#"[{"type":"extension","value":[".jpg",".png"]}]"#;
        let action = r#"{"type":"move","destination":"Images"}"#;
        for (id, name) in [("rule-a", "Images"), ("rule-b", "Imágenes")] {
            connection
                .execute(
                    "INSERT INTO rules (id, name, is_quick_rule, enabled, \"order\", icon, conditions, action, created_at) \
                     VALUES (?1, ?2, 1, 1, 0, 'Image', ?3, ?4, ?5)",
                    rusqlite::params![id, name, extensions, action, chrono::Utc::now().to_rfc3339()],
                )
                .unwrap();
        }
        connection
            .execute(
                "INSERT INTO folders (id, path, name, mode, rule_ids, watcher_status, created_at, subfolders) \
                 VALUES ('folder', '/watched', 'Watched', 'single', '[\"rule-a\",\"rule-b\"]', 'stopped', ?1, '{}')",
                [chrono::Utc::now().to_rfc3339()],
            )
            .unwrap();
        drop(connection);

        database.run_migrations().unwrap();

        let connection = database.conn.lock();
        let remaining: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM rules WHERE is_quick_rule = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let assignments: String = connection
            .query_row(
                "SELECT rule_ids FROM folders WHERE id = 'folder'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 1);
        assert_eq!(
            assignments,
            format!(
                r#"["{}"]"#,
                crate::storage::folders::quick_rule_id("quick-images")
            )
        );
    }

    #[test]
    fn built_in_rules_with_changed_destinations_or_conditions_collapse_to_canonical_ids() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(&directory.path().join("built-ins.db")).unwrap();
        database.run_migrations().unwrap();
        let mut legacy_ids = Vec::new();
        {
            let connection = database.conn.lock();
            for (index, template) in crate::domain::QUICK_RULES.iter().enumerate() {
                let (spanish_name, _) =
                    crate::storage::folders::localized_quick_rule(template.icon, "es");
                let variants = [
                    (
                        format!("quick-{index}"),
                        spanish_name,
                        1,
                        template.icon,
                        serde_json::to_string(&vec![Condition::Extension {
                            value: vec![template.extensions[0].to_string()],
                        }])
                        .unwrap(),
                        "Old destination",
                    ),
                    (
                        format!("named-{index}"),
                        template.name,
                        0,
                        "File",
                        "[]".to_string(),
                        "Another destination",
                    ),
                    (
                        format!("extensions-{index}"),
                        "Legacy rule",
                        0,
                        "File",
                        serde_json::to_string(&vec![Condition::Extension {
                            value: template
                                .extensions
                                .iter()
                                .map(|extension| extension.to_string())
                                .collect(),
                        }])
                        .unwrap(),
                        "Third destination",
                    ),
                ];
                for (id, name, is_quick, icon, conditions, destination) in variants {
                    legacy_ids.push(id.clone());
                    let action = serde_json::json!({"type":"move", "destination":destination});
                    connection
                        .execute(
                            "INSERT INTO rules (id, name, is_quick_rule, enabled, \"order\", icon, conditions, action, created_at) \
                             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7, ?8)",
                            rusqlite::params![
                                id,
                                name,
                                is_quick,
                                index as i32,
                                icon,
                                conditions,
                                action.to_string(),
                                chrono::Utc::now().to_rfc3339()
                            ],
                        )
                        .unwrap();
                }
            }
            connection
                .execute(
                    "INSERT INTO folders (id, path, name, mode, rule_ids, watcher_status, created_at, subfolders) \
                     VALUES ('folder', '/watched', 'Watched', 'single', ?1, 'stopped', ?2, '{}')",
                    rusqlite::params![
                        serde_json::to_string(&legacy_ids).unwrap(),
                        chrono::Utc::now().to_rfc3339()
                    ],
                )
                .unwrap();
        }

        database.run_migrations().unwrap();
        database.run_migrations().unwrap();

        let connection = database.conn.lock();
        let mut expected_ids: Vec<String> = crate::domain::QUICK_RULES
            .iter()
            .map(|template| crate::storage::folders::quick_rule_id(template.id).to_string())
            .collect();
        expected_ids.sort();
        let mut actual_ids = {
            let mut statement = connection
                .prepare("SELECT id FROM rules WHERE is_quick_rule = 1 ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .collect::<std::result::Result<Vec<_>, _>>()
                .unwrap()
        };
        actual_ids.sort();
        let assignments: String = connection
            .query_row(
                "SELECT rule_ids FROM folders WHERE id = 'folder'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let mut assigned_ids: Vec<String> = serde_json::from_str(&assignments).unwrap();
        assigned_ids.sort();
        assert_eq!(actual_ids, expected_ids);
        assert_eq!(assigned_ids, expected_ids);
    }

    #[test]
    fn migrations_are_idempotent_and_create_the_complete_local_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("filegrove.db");
        let database = Database::open(&path).unwrap();

        database.run_migrations().unwrap();
        database.run_migrations().unwrap();

        let connection = database.conn.lock();
        let journal_mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        let foreign_keys: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");
        assert_eq!(foreign_keys, 1);

        for table in [
            "settings",
            "folders",
            "rules",
            "activity",
            "undo_entries",
            "command_ledger",
        ] {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "missing table {table}");
        }

        let theme: String = connection
            .query_row("SELECT value FROM settings WHERE key='theme'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(theme, "\"dark\"");
        let duplicate_behavior: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key='duplicateBehavior'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(duplicate_behavior, "\"keepBoth\"");

        let launch_at_startup: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key='launchAtStartup'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let onboarding_completed: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key='onboardingCompleted'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(launch_at_startup, "false");
        assert_eq!(onboarding_completed, "false");

        let fresh_folder_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM folders", [], |row| row.get(0))
            .unwrap();
        let fresh_rule_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM rules", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fresh_folder_count, 0);
        assert_eq!(fresh_rule_count, 0);

        let latest_migration: i64 = connection
            .query_row(
                "SELECT MAX(version) FROM refinery_schema_history",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(latest_migration, 5);
    }

    #[test]
    fn scheduled_folders_become_inactive_manual_folders_without_losing_schedule_data() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("legacy-filegrove.db");
        let database = Database::open(&path).unwrap();

        {
            let mut connection = database.conn.lock();
            migrations::runner()
                .set_target(refinery::Target::Version(4))
                .run(&mut *connection)
                .unwrap();
            connection
                .execute(
                    "INSERT INTO folders (
                        id, path, name, mode, rule_ids, watcher_status, created_at,
                        subfolders, schedule, exclude_folders, exclude_patterns
                     ) VALUES (
                        'legacy-scheduled', '/tmp/legacy-scheduled', 'Legacy', 'scheduled',
                        '[]', 'running', '2026-08-02T00:00:00Z', '{}',
                        '{\"enabled\":true,\"daysOfWeek\":[1],\"time\":\"09:00\"}', '[]', '[]'
                     )",
                    [],
                )
                .unwrap();
        }

        database.run_migrations().unwrap();

        let connection = database.conn.lock();
        let (mode, watcher_status, schedule): (String, String, String) = connection
            .query_row(
                "SELECT mode, watcher_status, schedule FROM folders WHERE id='legacy-scheduled'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(mode, "single");
        assert_eq!(watcher_status, "stopped");
        assert_eq!(
            schedule,
            "{\"enabled\":true,\"daysOfWeek\":[1],\"time\":\"09:00\"}"
        );
    }
}
