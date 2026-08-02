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
#[serde(rename_all = "camelCase")]
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
    rules.retain(Rule::is_actionable);
    rules.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.created_at.cmp(&right.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });

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
            identity: None,
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

    #[test]
    fn rules_without_conditions_are_never_actionable() {
        let mut r = rule(
            "a",
            1,
            Condition::Extension {
                value: vec![".jpg".into()],
            },
        );
        r.conditions.clear();
        let plan = compute_organize_plan(vec![file("photo", ".jpg")], vec![r]);
        assert!(plan[0].action.is_none());
    }

    #[test]
    fn preview_plan_uses_the_renderer_camel_case_contract() {
        let plan = compute_organize_plan(
            vec![file("photo", ".jpg")],
            vec![rule(
                "a",
                1,
                Condition::Extension {
                    value: vec![".jpg".into()],
                },
            )],
        );
        let serialized = serde_json::to_value(&plan[0]).unwrap();

        assert!(serialized.get("ruleId").is_some());
        assert!(serialized.get("ruleName").is_some());
        assert!(serialized.get("rule_id").is_none());
        assert!(serialized["file"].get("modifiedAt").is_some());
    }
}
