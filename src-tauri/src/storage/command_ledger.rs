//! Durable exactly-once guard for rules that execute external commands.

use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use uuid::Uuid;

pub struct CommandLedgerRepo {
    pub(crate) conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClaim {
    Claimed,
    AlreadyCompleted,
    InProgress,
    InterruptedNeedsReview,
    InterruptedAwaitingReview,
}

impl CommandLedgerRepo {
    /// A process cannot prove whether an external command completed after a
    /// crash. Turn every running claim into a review item on next startup.
    pub fn reset_in_progress(&self) -> Result<usize> {
        let conn = self.conn.lock();
        Ok(conn.execute(
            "UPDATE command_ledger SET in_progress = 2, updated_at = ?1 WHERE in_progress = 1",
            [Utc::now().to_rfc3339()],
        )?)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn try_begin(
        &self,
        rule_id: Uuid,
        file_path: &str,
        command: &str,
        file_size: u64,
        modified_at: DateTime<Utc>,
        rerun_always: bool,
    ) -> Result<CommandClaim> {
        let file_size = i64::try_from(file_size).map_err(|_| anyhow!("FILE_TOO_LARGE"))?;
        let modified_at = modified_at.to_rfc3339();
        let rule_id = rule_id.to_string();
        let mut conn = self.conn.lock();
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(i64, String, i64)> = transaction
            .query_row(
                "SELECT file_size, modified_at, in_progress FROM command_ledger \
                 WHERE rule_id=?1 AND file_path=?2 AND command=?3",
                rusqlite::params![rule_id, file_path, command],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        if let Some((old_size, old_modified, state)) = previous {
            let same_file = old_size == file_size && old_modified == modified_at;
            let blocked = match state {
                1 => Some(CommandClaim::InProgress),
                2 => Some(CommandClaim::InterruptedNeedsReview),
                3 => Some(CommandClaim::InterruptedAwaitingReview),
                _ if same_file && !rerun_always => Some(CommandClaim::AlreadyCompleted),
                _ => None,
            };
            if let Some(claim) = blocked {
                transaction.commit()?;
                return Ok(claim);
            }
        }

        transaction.execute(
            "INSERT INTO command_ledger \
             (rule_id, file_path, command, file_size, modified_at, in_progress, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6) \
             ON CONFLICT(rule_id, file_path, command) DO UPDATE SET \
               file_size=excluded.file_size, modified_at=excluded.modified_at, \
               in_progress=1, updated_at=excluded.updated_at",
            rusqlite::params![
                rule_id,
                file_path,
                command,
                file_size,
                modified_at,
                Utc::now().to_rfc3339(),
            ],
        )?;
        transaction.commit()?;
        Ok(CommandClaim::Claimed)
    }

    pub fn abort(&self, rule_id: Uuid, file_path: &str, command: &str) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM command_ledger \
             WHERE rule_id=?1 AND file_path=?2 AND command=?3 AND in_progress=1",
            rusqlite::params![rule_id.to_string(), file_path, command],
        )?;
        Ok(())
    }

    pub fn finish(
        &self,
        rule_id: Uuid,
        file_path: &str,
        command: &str,
        file_size: u64,
        modified_at: DateTime<Utc>,
    ) -> Result<()> {
        let file_size = i64::try_from(file_size).map_err(|_| anyhow!("FILE_TOO_LARGE"))?;
        let conn = self.conn.lock();
        let changed = conn.execute(
            "UPDATE command_ledger SET file_size=?1, modified_at=?2, in_progress=0, updated_at=?3 \
             WHERE rule_id=?4 AND file_path=?5 AND command=?6 AND in_progress=1",
            rusqlite::params![
                file_size,
                modified_at.to_rfc3339(),
                Utc::now().to_rfc3339(),
                rule_id.to_string(),
                file_path,
                command,
            ],
        )?;
        if changed != 1 {
            return Err(anyhow!("COMMAND_CLAIM_NOT_FOUND"));
        }
        Ok(())
    }

    pub fn mark_interrupted_reported(
        &self,
        rule_id: Uuid,
        file_path: &str,
        command: &str,
    ) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE command_ledger SET in_progress=3, updated_at=?1 \
             WHERE rule_id=?2 AND file_path=?3 AND command=?4 AND in_progress=2",
            rusqlite::params![
                Utc::now().to_rfc3339(),
                rule_id.to_string(),
                file_path,
                command
            ],
        )?;
        Ok(())
    }

    pub fn allow_interrupted_retry(
        &self,
        rule_id: Uuid,
        file_path: &str,
        command: &str,
    ) -> Result<bool> {
        let conn = self.conn.lock();
        let changed = conn.execute(
            "DELETE FROM command_ledger \
             WHERE rule_id=?1 AND file_path=?2 AND command=?3 AND in_progress IN (2,3)",
            rusqlite::params![rule_id.to_string(), file_path, command],
        )?;
        Ok(changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Database;

    #[test]
    fn completed_fingerprints_skip_and_interrupted_claims_require_review() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(&directory.path().join("db.sqlite")).unwrap();
        database.run_migrations().unwrap();
        let repo = database.command_ledger();
        let rule = Uuid::new_v4();
        let modified = Utc::now();
        assert_eq!(
            repo.try_begin(rule, "/tmp/a", "echo", 1, modified, false)
                .unwrap(),
            CommandClaim::Claimed
        );
        repo.finish(rule, "/tmp/a", "echo", 1, modified).unwrap();
        assert_eq!(
            repo.try_begin(rule, "/tmp/a", "echo", 1, modified, false)
                .unwrap(),
            CommandClaim::AlreadyCompleted
        );
        assert_eq!(
            repo.try_begin(rule, "/tmp/a", "echo", 2, modified, false)
                .unwrap(),
            CommandClaim::Claimed
        );
        repo.reset_in_progress().unwrap();
        assert_eq!(
            repo.try_begin(rule, "/tmp/a", "echo", 2, modified, false)
                .unwrap(),
            CommandClaim::InterruptedNeedsReview
        );
    }
}
