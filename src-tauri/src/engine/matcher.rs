//! Condition matcher — see SPEC §5.1.
//!
//! All condition values are compared case-insensitively against file
//! properties. A rule matches when **all** its conditions match (AND).

use std::hash::{Hash, Hasher};
use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use regex::Regex;
#[cfg(test)]
use uuid::Uuid;

use crate::domain::{Condition, Rule};

/// A normalized view of a file's observable properties. The engine fills
/// this in before matching so we don't re-stat the file per condition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileItem {
    pub path: String,
    pub name: String,
    pub extension: String,
    pub size: u64,
    pub modified_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    /// Platform file-identity fingerprint captured while the file is open.
    /// The handle itself is closed immediately so scanning a large folder
    /// never retains thousands of operating-system file handles.
    #[serde(skip, default)]
    pub identity: Option<u64>,
}

impl FileItem {
    pub fn from_path_sync(path: &str) -> std::io::Result<Self> {
        let p = Path::new(path);
        let handle = same_file::Handle::from_path(p)?;
        let meta = handle.as_file().metadata()?;
        let identity = Some(identity_fingerprint(&handle));
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
            .map(DateTime::<Utc>::from)
            .unwrap_or_else(|_| Utc::now());
        let created_at = meta
            .created()
            .map(DateTime::<Utc>::from)
            // Stable fallback: using `now` here made `{createddate}` change on
            // every scan on filesystems without a birth-time field.
            .unwrap_or(modified_at);
        Ok(Self {
            path: path.to_string(),
            name,
            extension,
            size: meta.len(),
            modified_at,
            created_at,
            identity,
        })
    }

    pub fn is_same_unchanged_file(&self, current: &Self) -> bool {
        let same_identity = match (self.identity, current.identity) {
            (Some(expected), Some(actual)) => expected == actual,
            _ => false,
        };
        same_identity && self.size == current.size && self.modified_at == current.modified_at
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

fn identity_fingerprint(handle: &same_file::Handle) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    handle.hash(&mut hasher);
    hasher.finish()
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
            identity: None,
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

    #[test]
    fn snapshot_does_not_hold_the_file_open_and_detects_replacement() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.txt");
        std::fs::write(&path, b"original").unwrap();
        let planned = FileItem::from_path_sync(path.to_string_lossy().as_ref()).unwrap();

        // A retained OS handle would make large scans expensive and can also
        // interfere with replacement on Windows. The snapshot stores only a
        // compact identity fingerprint.
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"replacement with different contents").unwrap();
        let current = FileItem::from_path_sync(path.to_string_lossy().as_ref()).unwrap();

        assert!(planned.identity.is_some());
        assert!(!planned.is_same_unchanged_file(&current));
    }
}
