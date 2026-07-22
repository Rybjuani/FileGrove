//! Condition matcher — see SPEC §5.1.
//!
//! All condition values are compared case-insensitively against file
//! properties. A rule matches when **all** its conditions match (AND).

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use regex::Regex;
#[cfg(test)]
use uuid::Uuid;

use crate::domain::{Condition, Rule};

/// A normalized view of a file's observable properties. The engine fills
/// this in before matching so we don't re-stat the file per condition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileItem {
    pub path: String,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl FileItem {
    pub fn from_path_sync(path: &str) -> std::io::Result<Self> {
        let p = Path::new(path);
        let meta = std::fs::metadata(p)?;
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let extension = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| format!(".{}", s.to_lowercase()))
            .unwrap_or_default();
        let modified_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0).unwrap_or_else(Utc::now)
            })
            .unwrap_or_else(Utc::now);
        let created_at = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| {
                DateTime::<Utc>::from_timestamp(d.as_secs() as i64, 0).unwrap_or_else(Utc::now)
            })
            .unwrap_or_else(Utc::now);
        Ok(Self {
            path: path.to_string(),
            name,
            extension,
            size: meta.len(),
            modified_at,
            created_at,
        })
    }

    /// Filename without extension, lowercased.
    pub fn name_without_ext_lower(&self) -> String {
        let stem = Path::new(&self.name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        stem.to_lowercase()
    }
}

/// True iff the file satisfies the condition.
pub fn match_condition(file: &FileItem, cond: &Condition) -> bool {
    let file_name_lower = file.name.to_lowercase();
    let file_ext_lower = file.extension.to_lowercase();
    let name_no_ext = file.name_without_ext_lower();

    match cond {
        Condition::Extension { value } => value.iter().any(|v| {
            let v = v.to_lowercase();
            let v = if let Some(stripped) = v.strip_prefix('.') {
                format!(".{}", stripped)
            } else {
                format!(".{}", v)
            };
            file_ext_lower == v
        }),
        Condition::NameContains { value } => value
            .iter()
            .any(|v| file_name_lower.contains(&v.to_lowercase())),
        Condition::NameStartsWith { value } => value
            .iter()
            .any(|v| file_name_lower.starts_with(&v.to_lowercase())),
        Condition::NameEndsWith { value } => value
            .iter()
            .any(|v| name_no_ext.ends_with(&v.to_lowercase())),
        Condition::Regex { value } => {
            let raw = match value.first() {
                Some(r) => r,
                None => return false,
            };
            // Cache the compiled regex via `once_cell` if perf matters.
            match Regex::new(&format!("(?i){}", raw)) {
                Ok(re) => re.is_match(&file.name),
                Err(_) => false,
            }
        }
        Condition::SizeGreaterThan { value } => value
            .first()
            .and_then(|v| v.parse::<f64>().ok())
            .is_some_and(|mb| file.size > (mb * 1024.0 * 1024.0) as u64),
        Condition::SizeLessThan { value } => value
            .first()
            .and_then(|v| v.parse::<f64>().ok())
            .is_some_and(|mb| file.size < (mb * 1024.0 * 1024.0) as u64),
        Condition::OlderThan { value } => value
            .first()
            .and_then(|v| v.parse::<i64>().ok())
            .is_some_and(|days| {
                let cutoff = Utc::now() - Duration::days(days);
                file.modified_at < cutoff
            }),
        Condition::NewerThan { value } => value
            .first()
            .and_then(|v| v.parse::<i64>().ok())
            .is_some_and(|days| {
                let cutoff = Utc::now() - Duration::days(days);
                file.modified_at > cutoff
            }),
    }
}

/// True iff the file matches the rule (i.e. ALL conditions match).
pub fn match_file_against_rule(file: &FileItem, rule: &Rule) -> bool {
    rule.conditions.iter().all(|c| match_condition(file, c))
}

/// Walk `rules` in priority order (already sorted by caller) and return
/// the first matching rule, or `None`.
pub fn find_first_matching_rule<'a>(file: &FileItem, rules: &'a [Rule]) -> Option<&'a Rule> {
    rules.iter().find(|r| match_file_against_rule(file, r))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Action;

    fn fake_rule(id: &str, conditions: Vec<Condition>) -> Rule {
        Rule {
            id: Uuid::new_v4(),
            name: format!("rule-{}", id),
            is_quick_rule: false,
            enabled: true,
            order: 0,
            icon: "File".to_string(),
            conditions,
            action: Action::Move {
                destination: "/tmp".to_string(),
            },
            created_at: Utc::now(),
        }
    }

    fn fake_file(name: &str, ext: &str, size: u64) -> FileItem {
        FileItem {
            path: format!("/tmp/{}", name),
            name: format!("{}{}", name, ext),
            extension: ext.to_string(),
            size,
            modified_at: Utc::now(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn extension_match_case_insensitive() {
        let r = fake_rule(
            "1",
            vec![Condition::Extension {
                value: vec![".JPG".into(), ".png".into()],
            }],
        );
        assert!(match_file_against_rule(
            &fake_file("photo", ".JPG", 100),
            &r
        ));
        assert!(match_file_against_rule(
            &fake_file("photo", ".jpg", 100),
            &r
        ));
        assert!(match_file_against_rule(&fake_file("img", ".PNG", 100), &r));
        assert!(!match_file_against_rule(&fake_file("doc", ".pdf", 100), &r));
    }

    #[test]
    fn size_match() {
        let r = fake_rule(
            "1",
            vec![Condition::SizeGreaterThan {
                value: vec!["10".into()],
            }],
        );
        assert!(match_file_against_rule(
            &fake_file("big", ".bin", 11 * 1024 * 1024),
            &r
        ));
        assert!(!match_file_against_rule(
            &fake_file("small", ".bin", 5 * 1024 * 1024),
            &r
        ));
    }

    #[test]
    fn all_conditions_anded() {
        let r = fake_rule(
            "1",
            vec![
                Condition::Extension {
                    value: vec![".jpg".into()],
                },
                Condition::SizeGreaterThan {
                    value: vec!["1".into()],
                },
            ],
        );
        assert!(match_file_against_rule(
            &fake_file("photo", ".jpg", 2 * 1024 * 1024),
            &r
        ));
        assert!(!match_file_against_rule(
            &fake_file("photo", ".jpg", 500 * 1024),
            &r
        ));
        assert!(!match_file_against_rule(
            &fake_file("photo", ".png", 5 * 1024 * 1024),
            &r
        ));
    }

    #[test]
    fn invalid_regex_does_not_match() {
        let r = fake_rule(
            "1",
            vec![Condition::Regex {
                value: vec!["[invalid".into()],
            }],
        );
        assert!(!match_file_against_rule(
            &fake_file("photo", ".jpg", 100),
            &r
        ));
    }
}
