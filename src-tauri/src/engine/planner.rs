//! Plan computation — see SPEC §5.2.
//!
//! Given a list of files and a list of rules, compute the action to take
//! for each file. Rules are sorted by `order` ascending and the first
//! matching rule wins.

use uuid::Uuid;

use crate::domain::{Action, Rule};
use crate::engine::matcher::{find_first_matching_rule, FileItem};

/// The action (or lack thereof) decided for a single file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlannedAction {
    pub file: FileItem,
    pub rule_id: Option<Uuid>,
    pub rule_name: Option<String>,
    pub action: Option<Action>,
}

/// Compute the plan for a batch of files against a rule set.
///
/// `rules` is filtered to enabled rules and sorted by `order` before
/// matching. Files that don't match any rule get `action = None`.
pub fn compute_organize_plan(files: Vec<FileItem>, mut rules: Vec<Rule>) -> Vec<PlannedAction> {
    rules.retain(|r| r.enabled);
    rules.sort_by_key(|r| r.order);

    files
        .into_iter()
        .map(|file| {
            let matched = find_first_matching_rule(&file, &rules);
            PlannedAction {
                file: file.clone(),
                rule_id: matched.map(|r| r.id),
                rule_name: matched.map(|r| r.name.clone()),
                action: matched.map(|r| r.action.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Action, Condition};

    fn rule(id: &str, order: i32, cond: Condition) -> Rule {
        Rule {
            id: Uuid::new_v4(),
            name: format!("rule-{}", id),
            is_quick_rule: false,
            enabled: true,
            order,
            icon: "File".to_string(),
            conditions: vec![cond],
            action: Action::Move {
                destination: format!("/dest/{}", id),
            },
            created_at: chrono::Utc::now(),
        }
    }

    fn file(name: &str, ext: &str) -> FileItem {
        FileItem {
            path: format!("/tmp/{}{}", name, ext),
            name: format!("{}{}", name, ext),
            extension: ext.to_string(),
            size: 100,
            modified_at: chrono::Utc::now(),
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn first_match_wins() {
        let rules = vec![
            rule(
                "a",
                1,
                Condition::Extension {
                    value: vec![".jpg".into()],
                },
            ),
            rule(
                "b",
                2,
                Condition::Extension {
                    value: vec![".jpg".into(), ".png".into()],
                },
            ),
        ];
        let plan = compute_organize_plan(vec![file("photo", ".jpg")], rules);
        assert_eq!(plan.len(), 1);
        assert!(plan[0].rule_name.as_deref() == Some("rule-a"));
    }

    #[test]
    fn no_match_means_no_action() {
        let rules = vec![rule(
            "a",
            1,
            Condition::Extension {
                value: vec![".jpg".into()],
            },
        )];
        let plan = compute_organize_plan(vec![file("doc", ".pdf")], rules);
        assert!(plan[0].action.is_none());
        assert!(plan[0].rule_id.is_none());
    }

    #[test]
    fn disabled_rules_are_skipped() {
        let mut r = rule(
            "a",
            1,
            Condition::Extension {
                value: vec![".jpg".into()],
            },
        );
        r.enabled = false;
        let plan = compute_organize_plan(vec![file("photo", ".jpg")], vec![r]);
        assert!(plan[0].action.is_none());
    }
}
