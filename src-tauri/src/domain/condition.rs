use serde::{Deserialize, Serialize};

/// A single condition that a file must satisfy for a rule to match.
///
/// Multiple conditions are ANDed together (see `match_file_against_rule`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Condition {
    /// File extension matches one of `value` (with or without leading dot).
    Extension { value: Vec<String> },
    /// Filename contains any of `value` (case-insensitive).
    NameContains { value: Vec<String> },
    /// Filename starts with any of `value` (case-insensitive).
    NameStartsWith { value: Vec<String> },
    /// Filename (without extension) ends with any of `value`.
    NameEndsWith { value: Vec<String> },
    /// Filename matches the regex `value[0]` (case-insensitive).
    Regex { value: Vec<String> },
    /// File size > `value[0]` megabytes.
    SizeGreaterThan { value: Vec<String> },
    /// File size < `value[0]` megabytes.
    SizeLessThan { value: Vec<String> },
    /// File mtime older than `value[0]` days.
    OlderThan { value: Vec<String> },
    /// File mtime newer than `value[0]` days.
    NewerThan { value: Vec<String> },
}

impl Condition {
    /// All condition type names — useful for serialization / UI enumeration.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Extension { .. } => "extension",
            Self::NameContains { .. } => "nameContains",
            Self::NameStartsWith { .. } => "nameStartsWith",
            Self::NameEndsWith { .. } => "nameEndsWith",
            Self::Regex { .. } => "regex",
            Self::SizeGreaterThan { .. } => "sizeGreaterThan",
            Self::SizeLessThan { .. } => "sizeLessThan",
            Self::OlderThan { .. } => "olderThan",
            Self::NewerThan { .. } => "newerThan",
        }
    }
}
