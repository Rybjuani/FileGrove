//! SQLite database wrapper + refinery migrations.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use super::activity::ActivityRepo;
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
             PRAGMA synchronous = NORMAL;",
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
    pub fn undo(&self) -> UndoRepo {
        UndoRepo {
            conn: self.conn.clone(),
        }
    }
}
