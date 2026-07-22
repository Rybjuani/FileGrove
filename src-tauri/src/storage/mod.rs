//! Storage layer — SQLite-backed persistence for all application state.
//!
//! Tables: settings, folders, rules, activity, undo_entries.

pub mod activity;
pub mod db;
pub mod folders;
pub mod rules;
pub mod settings;
pub mod undo;

pub use db::Database;
