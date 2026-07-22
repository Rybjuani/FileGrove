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
        let conn = self.conn.lock();
        conn.execute("DELETE FROM rules WHERE id = ?", [id.to_string()])?;
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
    let conditions: Vec<Condition> = serde_json::from_str(&conditions_str).unwrap_or_default();
    let action: Action = serde_json::from_str(&action_str).unwrap_or(Action::Trash);
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
