//! Undo log repository.

use std::sync::Arc;

use anyhow::Result;
use parking_lot::Mutex;
use rusqlite::Connection;
use uuid::Uuid;

use crate::domain::undo::{UndoEntry, UndoSessionStats};

pub struct UndoRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

impl UndoRepo {
    pub fn append(&self, entry: &UndoEntry) -> Result<()> {
        let conn = self.conn.lock();
        let next_seq: i64 = conn.query_row(
            "SELECT COALESCE(MAX(seq), -1) + 1 FROM undo_entries WHERE session_id = ?",
            [entry.session_id.to_string()],
            |r| r.get(0),
        )?;
        conn.execute(
            "INSERT INTO undo_entries (session_id, at, from_path, to_path, seq) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                entry.session_id.to_string(),
                entry.at.to_rfc3339(),
                entry.from_path,
                entry.to_path,
                next_seq,
            ],
        )?;
        Ok(())
    }

    /// Read entries in reverse insertion order (newest first) for a session.
    pub fn read_reverse(&self, session_id: Uuid) -> Result<Vec<UndoEntry>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT session_id, at, from_path, to_path FROM undo_entries \
             WHERE session_id = ? ORDER BY seq DESC",
        )?;
        let rows = stmt.query_map([session_id.to_string()], |r| {
            let session_id_str: String = r.get(0)?;
            let at_str: String = r.get(1)?;
            let from_path: String = r.get(2)?;
            let to_path: String = r.get(3)?;
            let session_id = Uuid::parse_str(&session_id_str).unwrap_or_default();
            let at = chrono::DateTime::parse_from_rfc3339(&at_str)
                .map(|d| d.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now());
            Ok(UndoEntry {
                session_id,
                at,
                from_path,
                to_path,
            })
        })?;
        let mut out = vec![];
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn stats(&self, session_id: Uuid) -> Result<UndoSessionStats> {
        let conn = self.conn.lock();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM undo_entries WHERE session_id = ?",
            [session_id.to_string()],
            |r| r.get(0),
        )?;
        Ok(UndoSessionStats {
            session_id,
            entry_count: count as usize,
            completed: 0,
            failed: 0,
        })
    }

    pub fn delete_session(&self, session_id: Uuid) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM undo_entries WHERE session_id = ?",
            [session_id.to_string()],
        )?;
        Ok(())
    }
}
