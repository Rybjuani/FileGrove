//! Activity log repository.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::domain::activity::{ActivityEntry, ActivityType};

pub struct ActivityRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Default)]
pub struct ActivityFilters {
    pub folder_id: Option<Uuid>,
    pub rule_id: Option<Uuid>,
    pub success: Option<bool>,
    pub r#type: Option<ActivityType>,
    pub since: Option<chrono::DateTime<chrono::Utc>>,
    pub until: Option<chrono::DateTime<chrono::Utc>>,
}

impl ActivityRepo {
    /// Convert delete rows left pending by a previous process into an honest
    /// review state. File existence is not proof of whether every external
    /// filesystem effect completed, so startup never guesses success.
    pub fn mark_interrupted_deletes_for_review(&self) -> Result<usize> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE activity SET error = 'DELETE_INTERRUPTED_REVIEW_REQUIRED', success = 0 \
             WHERE type = 'delete' AND error = 'DELETE_IN_PROGRESS'",
            [],
        )?;
        Ok(changed)
    }

    pub fn list(
        &self,
        filters: &ActivityFilters,
        limit: u32,
        offset: u32,
    ) -> Result<(Vec<ActivityEntry>, u64)> {
        let conn = self.conn.lock();
        let mut where_clauses: Vec<String> = vec![];
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![];

        if let Some(f) = filters.folder_id {
            where_clauses.push("folder_id = ?".to_string());
            params.push(Box::new(f.to_string()));
        }
        if let Some(r) = filters.rule_id {
            where_clauses.push("rule_id = ?".to_string());
            params.push(Box::new(r.to_string()));
        }
        if let Some(s) = filters.success {
            where_clauses.push("success = ?".to_string());
            params.push(Box::new(if s { 1i32 } else { 0i32 }));
        }
        if let Some(t) = filters.r#type {
            where_clauses.push("type = ?".to_string());
            params.push(Box::new(t.as_str().to_string()));
        }
        if let Some(s) = filters.since {
            where_clauses.push("at >= ?".to_string());
            params.push(Box::new(s.to_rfc3339()));
        }
        if let Some(u) = filters.until {
            where_clauses.push("at <= ?".to_string());
            params.push(Box::new(u.to_rfc3339()));
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };

        let count_sql = format!("SELECT COUNT(*) FROM activity {}", where_sql);
        let list_sql = format!(
            "SELECT id, at, type, file_name, file_path, folder_id, rule_id, rule_name, \
             destination, command, exit_code, output, duration_ms, timed_out, success, error \
             FROM activity {} ORDER BY at DESC LIMIT ? OFFSET ?",
            where_sql
        );

        let total: u64 = {
            let params_refs: Vec<&dyn rusqlite::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            conn.query_row(&count_sql, params_refs.as_slice(), |r| {
                let c: i64 = r.get(0)?;
                Ok(c as u64)
            })?
        };

        params.push(Box::new(limit));
        params.push(Box::new(offset));
        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let mut stmt = conn.prepare(&list_sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), row_to_activity)?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok((out, total))
    }

    pub fn insert(&self, entry: &ActivityEntry) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO activity (id, at, type, file_name, file_path, folder_id, rule_id, \
             rule_name, destination, command, exit_code, output, duration_ms, timed_out, \
             success, error) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            rusqlite::params![
                entry.id.to_string(),
                entry.at.to_rfc3339(),
                entry.r#type.as_str(),
                entry.file_name,
                entry.file_path,
                entry.folder_id.map(|i| i.to_string()),
                entry.rule_id.map(|i| i.to_string()),
                entry.rule_name,
                entry.destination,
                entry.command,
                entry.exit_code,
                entry.output,
                entry.duration_ms,
                entry.timed_out.map(|t| if t { 1i32 } else { 0i32 }),
                entry.success as i32,
                entry.error,
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, entry: &ActivityEntry) -> Result<()> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE activity SET at=?2, type=?3, file_name=?4, file_path=?5, folder_id=?6, \
             rule_id=?7, rule_name=?8, destination=?9, command=?10, exit_code=?11, output=?12, \
             duration_ms=?13, timed_out=?14, success=?15, error=?16 WHERE id=?1",
            rusqlite::params![
                entry.id.to_string(),
                entry.at.to_rfc3339(),
                entry.r#type.as_str(),
                entry.file_name,
                entry.file_path,
                entry.folder_id.map(|id| id.to_string()),
                entry.rule_id.map(|id| id.to_string()),
                entry.rule_name,
                entry.destination,
                entry.command,
                entry.exit_code,
                entry.output,
                entry.duration_ms,
                entry.timed_out.map(i32::from),
                i32::from(entry.success),
                entry.error,
            ],
        )?;
        if changed != 1 {
            anyhow::bail!("ACTIVITY_NOT_FOUND");
        }
        Ok(())
    }

    pub fn delete(&self, ids: &[Uuid]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let id_strs: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
        let params: Vec<&dyn rusqlite::ToSql> =
            id_strs.iter().map(|s| s as &dyn rusqlite::ToSql).collect();

        // If an unresolved command-review row is removed, make its durable
        // claim announce itself again on the next processing pass. Otherwise
        // clearing History could leave a command blocked with no visible way
        // for the user to approve or reject a retry.
        let review_sql = format!(
            "SELECT rule_id, file_path, command FROM activity \
             WHERE id IN ({placeholders}) \
               AND error LIKE 'COMMAND_INTERRUPTED_REVIEW_REQUIRED%'"
        );
        let reviews: Vec<(String, String, String)> = {
            let mut statement = transaction.prepare(&review_sql)?;
            let rows = statement.query_map(params.as_slice(), |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
            let mut collected = Vec::new();
            for row in rows {
                collected.push(row?);
            }
            collected
        };
        for (rule_id, file_path, command) in reviews {
            transaction.execute(
                "UPDATE command_ledger SET in_progress = 2 \
                 WHERE rule_id = ?1 AND file_path = ?2 AND command = ?3 AND in_progress = 3",
                rusqlite::params![rule_id, file_path, command],
            )?;
        }

        let delete_sql = format!("DELETE FROM activity WHERE id IN ({placeholders})");
        transaction.execute(&delete_sql, params.as_slice())?;
        transaction.commit()?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        transaction.execute(
            "UPDATE command_ledger SET in_progress = 2 WHERE in_progress = 3",
            [],
        )?;
        transaction.execute("DELETE FROM activity", [])?;
        transaction.commit()?;
        Ok(())
    }
}

fn row_to_activity(row: &rusqlite::Row<'_>) -> rusqlite::Result<ActivityEntry> {
    let id_str: String = row.get(0)?;
    let at_str: String = row.get(1)?;
    let type_str: String = row.get(2)?;
    let file_name: String = row.get(3)?;
    let file_path: String = row.get(4)?;
    let folder_id_str: Option<String> = row.get(5)?;
    let rule_id_str: Option<String> = row.get(6)?;
    let rule_name: Option<String> = row.get(7)?;
    let destination: Option<String> = row.get(8)?;
    let command: Option<String> = row.get(9)?;
    let exit_code: Option<i32> = row.get(10)?;
    let output: Option<String> = row.get(11)?;
    let duration_ms: Option<u32> = row.get(12)?;
    let timed_out: Option<i32> = row.get(13)?;
    let success: i32 = row.get(14)?;
    let error: Option<String> = row.get(15)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let at = chrono::DateTime::parse_from_rfc3339(&at_str)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());
    let r#type = match type_str.as_str() {
        "trash" => ActivityType::Trash,
        "delete" => ActivityType::Delete,
        "runCommand" => ActivityType::RunCommand,
        _ => ActivityType::Move,
    };
    let folder_id = folder_id_str.as_ref().and_then(|s| Uuid::parse_str(s).ok());
    let rule_id = rule_id_str.as_ref().and_then(|s| Uuid::parse_str(s).ok());

    Ok(ActivityEntry {
        id,
        at,
        r#type,
        file_name,
        file_path,
        folder_id,
        rule_id,
        rule_name,
        destination,
        command,
        exit_code,
        output,
        duration_ms,
        timed_out: timed_out.map(|t| t != 0),
        success: success != 0,
        error,
    })
}
