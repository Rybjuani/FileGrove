//! Rule repository.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::domain::{Action, Condition, Rule};

pub struct RuleRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl RuleRepo {
    pub fn list(&self) -> Result<Vec<Rule>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, name, is_quick_rule, enabled, \"order\", icon, conditions, action, created_at \
             FROM rules ORDER BY \"order\" ASC",
        )?;
        let rows = stmt.query_map([], row_to_rule)?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn get_by_ids(&self, ids: &[Uuid]) -> Result<Vec<Rule>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let conn = self.conn.lock();
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let sql = format!(
            "SELECT id, name, is_quick_rule, enabled, \"order\", icon, conditions, action, created_at \
             FROM rules WHERE id IN ({})",
            placeholders
        );
        let id_strs: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
        let params: Vec<&dyn rusqlite::ToSql> =
            id_strs.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params.as_slice(), row_to_rule)?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn insert(&self, rule: &Rule) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO rules (id, name, is_quick_rule, enabled, \"order\", icon, conditions, action, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                rule.id.to_string(),
                rule.name,
                rule.is_quick_rule as i32,
                rule.enabled as i32,
                rule.order,
                rule.icon,
                serde_json::to_string(&rule.conditions)?,
                serde_json::to_string(&rule.action)?,
                rule.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn update(&self, rule: &Rule) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE rules SET name=?2, enabled=?3, \"order\"=?4, icon=?5, conditions=?6, action=?7 \
             WHERE id=?1",
            rusqlite::params![
                rule.id.to_string(),
                rule.name,
                rule.enabled as i32,
                rule.order,
                rule.icon,
                serde_json::to_string(&rule.conditions)?,
                serde_json::to_string(&rule.action)?,
            ],
        )?;
        Ok(())
    }

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let id_string = id.to_string();
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM rules WHERE id = ?)",
            [&id_string],
            |row| row.get(0),
        )?;
        if !exists {
            anyhow::bail!("RULE_NOT_FOUND");
        }

        // Folder assignments live in JSON arrays rather than a relation, so
        // clean them in the same transaction as the rule deletion.
        let folder_updates: Vec<(String, String)> = {
            let mut statement = transaction.prepare("SELECT id, rule_ids FROM folders")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            let mut updates = Vec::new();
            for row in rows {
                let (folder_id, serialized) = row?;
                let mut rule_ids: Vec<Uuid> = serde_json::from_str(&serialized).unwrap_or_default();
                let previous_len = rule_ids.len();
                rule_ids.retain(|rule_id| *rule_id != id);
                if rule_ids.len() != previous_len {
                    updates.push((folder_id, serde_json::to_string(&rule_ids)?));
                }
            }
            updates
        };
        for (folder_id, serialized) in folder_updates {
            transaction.execute(
                "UPDATE folders SET rule_ids = ?1 WHERE id = ?2",
                rusqlite::params![serialized, folder_id],
            )?;
        }
        transaction.execute("DELETE FROM rules WHERE id = ?", [&id_string])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn toggle(&self, id: Uuid) -> Result<bool> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE rules SET enabled = NOT enabled WHERE id = ?",
            [id.to_string()],
        )?;
        let new_value: bool = {
            let mut stmt = conn.prepare("SELECT enabled FROM rules WHERE id = ?")?;
            let mut rows = stmt.query_map([id.to_string()], |r| {
                let v: i32 = r.get(0)?;
                Ok(v != 0)
            })?;
            rows.next().map(|r| r.unwrap_or(false)).unwrap_or(false)
        };
        Ok(new_value)
    }

    /// Reorder the complete rule list atomically so a partial database error
    /// can never change which destructive rule wins for a file.
    pub fn reorder(&self, ids: &[Uuid]) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut unique = std::collections::HashSet::with_capacity(ids.len());
        if ids.iter().any(|id| !unique.insert(*id)) {
            anyhow::bail!("INVALID_RULE_ORDER");
        }

        let mut conn = self.conn.lock();
        let transaction = conn.transaction()?;
        let total: usize = transaction.query_row("SELECT COUNT(*) FROM rules", [], |row| {
            row.get::<_, i64>(0).map(|value| value as usize)
        })?;
        if total != ids.len() {
            anyhow::bail!("RULE_SET_CHANGED");
        }
        for (order, id) in ids.iter().enumerate() {
            let changed = transaction.execute(
                "UPDATE rules SET \"order\" = ?2 WHERE id = ?1",
                rusqlite::params![id.to_string(), i32::try_from(order)?],
            )?;
            if changed != 1 {
                anyhow::bail!("RULE_NOT_FOUND");
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

fn row_to_rule(row: &rusqlite::Row<'_>) -> rusqlite::Result<Rule> {
    let id_str: String = row.get(0)?;
    let name: String = row.get(1)?;
    let is_quick: i32 = row.get(2)?;
    let enabled: i32 = row.get(3)?;
    let order: i32 = row.get(4)?;
    let icon: String = row.get(5)?;
    let conditions_str: String = row.get(6)?;
    let action_str: String = row.get(7)?;
    let created_at_str: String = row.get(8)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let conditions: Vec<Condition> = serde_json::from_str(&conditions_str).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(error))
    })?;
    // Never substitute a destructive action for corrupt persisted data. A
    // malformed action must make the row unreadable so the engine cannot run
    // it until it has been deliberately repaired.
    let action: Action = serde_json::from_str(&action_str).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
        .map(|d| d.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    Ok(Rule {
        id,
        name,
        is_quick_rule: is_quick != 0,
        enabled: enabled != 0,
        order,
        icon,
        conditions,
        action,
        created_at,
    })
}
