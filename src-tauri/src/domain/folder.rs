use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Schedule;

/// A folder that the user wants FileGrove to organize.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folder {
    pub id: Uuid,
    pub path: String,
    pub name: String,
    pub mode: FolderMode,
    pub rule_ids: Vec<Uuid>,
    pub watcher_status: WatcherStatus,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub subfolders: SubfolderConfig,
    pub schedule: Option<Schedule>,
    pub exclude_folders: Vec<String>,
    pub exclude_patterns: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FolderMode {
    /// Manual one-shot organize; user clicks "Organize now".
    Single,
    /// Real-time: every new file triggers rules immediately.
    Auto,
    /// Cron-driven: rules fire on a schedule.
    Scheduled,
}

impl FolderMode {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "single" => Some(Self::Single),
            "auto" => Some(Self::Auto),
            "scheduled" => Some(Self::Scheduled),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::Auto => "auto",
            Self::Scheduled => "scheduled",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WatcherStatus {
    Stopped,
    Running,
    Paused,
}

impl WatcherStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Running => "running",
            Self::Paused => "paused",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubfolderConfig {
    pub enabled: bool,
    pub destination_mode: DestinationMode,
    pub first_run_confirmed: bool,
    pub max_depth: MaxDepth,
}

impl Default for SubfolderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            destination_mode: DestinationMode::Flatten,
            first_run_confirmed: false,
            max_depth: MaxDepth::Unlimited,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DestinationMode {
    /// Move all matched files into the destination root, ignoring subfolder structure.
    Flatten,
    /// Preserve the relative subfolder structure under the destination.
    Preserve,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MaxDepth {
    Unlimited,
    Level(u32),
}

impl MaxDepth {
    pub fn includes(self, depth: u32) -> bool {
        match self {
            Self::Unlimited => true,
            Self::Level(max) => depth <= max,
        }
    }
}
