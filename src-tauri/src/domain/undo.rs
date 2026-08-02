use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One reversible operation in an undo log. Currently only `Move` is
/// reversible (the others either have no Undo or are handled by the OS).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoEntry {
    pub session_id: Uuid,
    pub at: chrono::DateTime<chrono::Utc>,
    pub from_path: String,
    pub to_path: String,
}

/// Aggregate stats for an undo session, surfaced to the UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoSessionStats {
    pub session_id: Uuid,
    pub entry_count: usize,
    pub completed: usize,
    pub failed: usize,
}

/// Persisted summary used to recover undo affordances after navigation or an
/// application restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoSessionSummary {
    pub session_id: Uuid,
    pub at: chrono::DateTime<chrono::Utc>,
    pub entry_count: usize,
    pub from_path: String,
}
