//! Storage layer — SQLite-backed persistence for all application state.
//!
//! Tables: settings, folders, rules, activity, undo_entries, command_ledger.

pub mod activity;
pub mod command_ledger;
pub mod db;
pub mod folders;
pub mod rules;
pub mod settings;
pub mod undo;

pub use db::Database;
