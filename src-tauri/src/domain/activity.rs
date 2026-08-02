use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// One row in the activity log — every file action records one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEntry {
    pub id: Uuid,
    pub at: chrono::DateTime<chrono::Utc>,
    pub r#type: ActivityType,
    pub file_name: String,
    pub file_path: String,
    pub folder_id: Option<Uuid>,
    pub rule_id: Option<Uuid>,
    pub rule_name: Option<String>,
    /// For `Move`: the final destination path.
    pub destination: Option<String>,
    /// For `RunCommand`: the executed command string.
    pub command: Option<String>,
    /// For `RunCommand`: process exit code.
    pub exit_code: Option<i32>,
    /// For `RunCommand`: captured stdout+stderr (truncated to 1 MB).
    pub output: Option<String>,
    /// For `RunCommand`: wall-clock duration in ms.
    pub duration_ms: Option<u32>,
    /// For `RunCommand`: did the command time out?
    pub timed_out: Option<bool>,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ActivityType {
    Move,
    Trash,
    Delete,
    RunCommand,
}

impl ActivityType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Trash => "trash",
            Self::Delete => "delete",
            Self::RunCommand => "runCommand",
        }
    }
}
