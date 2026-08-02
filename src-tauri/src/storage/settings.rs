//! Settings repository — thin wrapper over the `settings` table.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
use serde_json::Value;

pub struct SettingsRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl SettingsRepo {
    pub fn get_all(&self) -> Result<serde_json::Map<String, Value>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT key, value FROM settings")?;
        let rows = stmt.query_map([], |r| {
            let k: String = r.get(0)?;
            let v: String = r.get(1)?;
            Ok((k, v))
        })?;
        let mut map = serde_json::Map::new();
        for row in rows {
            let (k, v) = row?;
            let parsed: Value = serde_json::from_str(&v).unwrap_or(Value::Null);
            map.insert(k, parsed);
        }
        Ok(map)
    }

    pub fn get(&self, key: &str) -> Result<Option<Value>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?")?;
        let mut rows = stmt.query_map([key], |r| {
            let v: String = r.get(0)?;
            Ok(v)
        })?;
        if let Some(row) = rows.next() {
            let v = row?;
            let parsed: Value = serde_json::from_str(&v)?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }

    pub fn set(&self, key: &str, value: &Value) -> Result<()> {
        let conn = self.conn.lock();
        let serialized = serde_json::to_string(value)?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![key, serialized],
        )?;
        Ok(())
    }

    pub fn set_many(
        &self,
        updates: &serde_json::Map<String, Value>,
    ) -> Result<Vec<(String, Option<Value>)>> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        let mut previous = Vec::with_capacity(updates.len());
        for (k, v) in updates {
            let old_serialized = tx
                .query_row("SELECT value FROM settings WHERE key = ?1", [k], |row| {
                    row.get::<_, String>(0)
                })
                .optional()?;
            let old_value = old_serialized
                .map(|serialized| serde_json::from_str(&serialized))
                .transpose()?;
            previous.push((k.clone(), old_value));
            let serialized = serde_json::to_string(v)?;
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                rusqlite::params![k, serialized],
            )?;
        }
        tx.commit()?;
        Ok(previous)
    }

    pub fn restore(&self, snapshot: &[(String, Option<Value>)]) -> Result<()> {
        let mut conn = self.conn.lock();
        let tx = conn.transaction()?;
        for (key, value) in snapshot {
            if let Some(value) = value {
                let serialized = serde_json::to_string(value)?;
                tx.execute(
                    "INSERT INTO settings (key, value) VALUES (?1, ?2) \
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    rusqlite::params![key, serialized],
                )?;
            } else {
                tx.execute("DELETE FROM settings WHERE key = ?1", [key])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ---- typed accessors used by the bootstrap ----

    pub fn get_minimize_to_tray(&self) -> bool {
        self.get("minimizeToTray")
            .ok()
            .flatten()
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    pub fn get_show_tray_icon(&self) -> bool {
        self.get("showTrayIcon")
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(true)
    }

    pub fn get_launch_at_startup(&self) -> bool {
        self.get("launchAtStartup")
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    pub fn get_notifications(&self) -> bool {
        self.get("notifications")
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(true)
    }

    pub fn get_automation_paused(&self) -> bool {
        self.get("automationPaused")
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    }

    pub fn get_language(&self) -> String {
        self.get("language")
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(str::to_string))
            .unwrap_or_else(|| "en".to_string())
    }

    pub fn get_conflict_behavior_str(&self) -> String {
        self.get("conflictBehavior")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "rename".to_string())
    }

    pub fn get_duplicate_behavior_str(&self) -> String {
        self.get("duplicateBehavior")
            .ok()
            .flatten()
            .and_then(|value| value.as_str().map(String::from))
            .unwrap_or_else(|| "keepBoth".to_string())
    }
}
