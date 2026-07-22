use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Action, Condition};

/// A user-defined rule that maps file conditions to an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub id: Uuid,
    pub name: String,
    pub is_quick_rule: bool,
    pub enabled: bool,
    pub order: i32,
    pub icon: String,
    pub conditions: Vec<Condition>,
    pub action: Action,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl Rule {
    /// A rule is "active" if it's enabled and has at least one condition.
    pub fn is_actionable(&self) -> bool {
        self.enabled && !self.conditions.is_empty()
    }
}

/// Predefined quick rules — see SPEC §2.1 F-04.
#[allow(dead_code)]
pub const QUICK_RULES: &[QuickRuleTemplate] = &[
    QuickRuleTemplate {
        id: "quick-images",
        name: "Images",
        description: "Organizes image files: JPG, PNG, GIF, WebP, SVG, and more",
        icon: "Image",
        extensions: &[
            ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp", ".svg", ".ico", ".heic",
        ],
        default_destination: "Images",
    },
    QuickRuleTemplate {
        id: "quick-documents",
        name: "Documents",
        description: "Organizes documents: PDF, Word, Excel, PowerPoint, and text files",
        icon: "FileText",
        extensions: &[
            ".pdf", ".doc", ".docx", ".xls", ".xlsx", ".ppt", ".pptx", ".txt", ".rtf", ".odt",
        ],
        default_destination: "Documents",
    },
    QuickRuleTemplate {
        id: "quick-videos",
        name: "Videos",
        description: "Organizes video files: MP4, MKV, AVI, MOV, and more",
        icon: "Video",
        extensions: &[
            ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v",
        ],
        default_destination: "Videos",
    },
    QuickRuleTemplate {
        id: "quick-audio",
        name: "Audio",
        description: "Organizes audio files: MP3, WAV, FLAC, AAC, and more",
        icon: "Music",
        extensions: &[".mp3", ".wav", ".flac", ".aac", ".ogg", ".wma", ".m4a"],
        default_destination: "Audio",
    },
    QuickRuleTemplate {
        id: "quick-archives",
        name: "Archives",
        description: "Organizes compressed files: ZIP, RAR, 7Z, TAR, and more",
        icon: "Archive",
        extensions: &[".zip", ".rar", ".7z", ".tar", ".gz", ".bz2"],
        default_destination: "Archives",
    },
    QuickRuleTemplate {
        id: "quick-installers",
        name: "Installers",
        description: "Organizes installer files: EXE, MSI, DMG, and app packages",
        icon: "Download",
        extensions: &[".exe", ".msi", ".dmg", ".app", ".deb", ".rpm"],
        default_destination: "Installers",
    },
];

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct QuickRuleTemplate {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    pub extensions: &'static [&'static str],
    pub default_destination: &'static str,
}
