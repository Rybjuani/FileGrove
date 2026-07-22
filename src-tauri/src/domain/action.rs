use serde::{Deserialize, Serialize};

/// What to do with a file when its rule matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    /// Move file to `destination` (a path that may contain tokens like `{year}`).
    Move { destination: String },
    /// Send the file to the OS trash (recoverable).
    Trash,
    /// Permanently delete the file. **No undo.**
    Delete,
    /// Execute a shell command against the file.
    RunCommand {
        command: String,
        #[serde(rename = "timeoutMs")]
        #[serde(default = "default_command_timeout_ms")]
        timeout_ms: u32,
        #[serde(rename = "rerunAlways")]
        #[serde(default)]
        rerun_always: bool,
    },
}

fn default_command_timeout_ms() -> u32 {
    30_000
}

impl Action {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Move { .. } => "move",
            Self::Trash => "trash",
            Self::Delete => "delete",
            Self::RunCommand { .. } => "runCommand",
        }
    }
}
