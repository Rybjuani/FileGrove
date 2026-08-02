use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::domain::{Action, Condition, Rule};
use crate::engine::cancellation::CancellationRegistry;
use crate::AppState;

#[tauri::command]
pub async fn list_rules(state: State<'_, AppState>) -> Result<Value, String> {
    match state.db.rules().list() {
        Ok(rules) => Ok(json!({ "success": true, "data": rules })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn get_by_ids(state: State<'_, AppState>, ids: Vec<Uuid>) -> Result<Value, String> {
    match state.db.rules().get_by_ids(&ids) {
        Ok(rules) => Ok(json!({ "success": true, "data": rules })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn create(state: State<'_, AppState>, rule: Rule) -> Result<Value, String> {
    let mut rule = rule;
    if rule.id == Uuid::nil() {
        rule.id = Uuid::new_v4();
    }
    rule.name = rule.name.trim().to_string();
    rule.created_at = chrono::Utc::now();
    if let Err(error) = validate_rule(&rule) {
        return Ok(json!({ "success": false, "error": error }));
    }
    match state.db.rules().insert(&rule) {
        Ok(()) => Ok(json!({ "success": true, "data": rule })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn update(
    state: State<'_, AppState>,
    id: Uuid,
    updates: serde_json::Value,
) -> Result<Value, String> {
    let _folder_leases = match acquire_rule_folder_leases(&state, Some(id)) {
        Ok(leases) => leases,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    let mut current = match state.db.rules().get_by_ids(&[id]) {
        Ok(rules) if !rules.is_empty() => rules.into_iter().next().unwrap(),
        _ => return Ok(json!({ "success": false, "error": "RULE_NOT_FOUND" })),
    };

    // Patch fields from the updates map.
    if let Some(obj) = updates.as_object() {
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            current.name = name.trim().to_string();
        }
        if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
            current.enabled = enabled;
        }
        if let Some(order_value) = obj.get("order") {
            let Some(order) = order_value
                .as_i64()
                .and_then(|order| i32::try_from(order).ok())
            else {
                return Ok(json!({ "success": false, "error": "INVALID_RULE_ORDER" }));
            };
            current.order = order;
        }
        if let Some(icon) = obj.get("icon").and_then(|v| v.as_str()) {
            current.icon = icon.to_string();
        }
        if let Some(conditions) = obj.get("conditions") {
            let Ok(conditions) = serde_json::from_value::<Vec<_>>(conditions.clone()) else {
                return Ok(json!({ "success": false, "error": "INVALID_RULE_CONDITIONS" }));
            };
            current.conditions = conditions;
        }
        if let Some(action) = obj.get("action") {
            let Ok(action) = serde_json::from_value::<_>(action.clone()) else {
                return Ok(json!({ "success": false, "error": "INVALID_RULE_ACTION" }));
            };
            current.action = action;
        }
    } else {
        return Ok(json!({ "success": false, "error": "INVALID_RULE" }));
    }

    if let Err(error) = validate_rule(&current) {
        return Ok(json!({ "success": false, "error": error }));
    }

    match state.db.rules().update(&current) {
        Ok(()) => Ok(json!({ "success": true, "data": current })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

fn validate_rule(rule: &Rule) -> Result<(), &'static str> {
    if rule.name.is_empty() || rule.name.chars().count() > 100 {
        return Err("INVALID_RULE_NAME");
    }
    if rule.conditions.is_empty() || rule.conditions.len() > 10 {
        return Err("INVALID_RULE_CONDITIONS");
    }
    if rule.icon.trim().is_empty() || rule.icon.chars().count() > 64 {
        return Err("INVALID_RULE_ICON");
    }
    for condition in &rule.conditions {
        let values = match condition {
            Condition::Extension { value }
            | Condition::NameContains { value }
            | Condition::NameStartsWith { value }
            | Condition::NameEndsWith { value }
            | Condition::Regex { value }
            | Condition::SizeGreaterThan { value }
            | Condition::SizeLessThan { value }
            | Condition::OlderThan { value }
            | Condition::NewerThan { value } => value,
        };
        if values.is_empty()
            || values.len() > 50
            || values.iter().any(|value| value.trim().is_empty())
            || values.iter().any(|value| value.chars().count() > 512)
        {
            return Err("INVALID_RULE_CONDITIONS");
        }
        if let Condition::Regex { value } = condition {
            if value.len() != 1 || regex::Regex::new(&value[0]).is_err() {
                return Err("INVALID_RULE_REGEX");
            }
        }
        match condition {
            Condition::SizeGreaterThan { value } | Condition::SizeLessThan { value }
                if value.len() != 1
                    || value[0]
                        .parse::<f64>()
                        .ok()
                        .map_or(true, |number| !number.is_finite() || number < 0.0) =>
            {
                return Err("INVALID_RULE_CONDITIONS");
            }
            Condition::OlderThan { value } | Condition::NewerThan { value }
                if value.len() != 1 || value[0].parse::<u32>().is_err() =>
            {
                return Err("INVALID_RULE_CONDITIONS");
            }
            _ => {}
        }
    }
    match &rule.action {
        Action::Move { destination } if destination.trim().is_empty() => Err("INVALID_DESTINATION"),
        Action::Move { destination } if !std::path::Path::new(destination).is_absolute() => {
            Err("INVALID_DESTINATION")
        }
        Action::Move { destination }
            if std::path::Path::new(destination)
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir)) =>
        {
            Err("INVALID_DESTINATION_TRAVERSAL")
        }
        Action::RunCommand {
            command,
            timeout_ms,
            ..
        } if command.trim().is_empty()
            || command.chars().count() > 32_768
            || !(100..=600_000).contains(timeout_ms) =>
        {
            Err("COMMAND_UNAVAILABLE")
        }
        _ => Ok(()),
    }
}

#[tauri::command]
pub async fn delete_rule(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    let _folder_leases = match acquire_rule_folder_leases(&state, Some(id)) {
        Ok(leases) => leases,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    match state.db.rules().delete(id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
    }
}

#[tauri::command]
pub async fn toggle(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    let _folder_leases = match acquire_rule_folder_leases(&state, Some(id)) {
        Ok(leases) => leases,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    match state.db.rules().toggle(id) {
        Ok(new_value) => Ok(json!({ "success": true, "data": { "enabled": new_value } })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn reorder_rules(state: State<'_, AppState>, ids: Vec<Uuid>) -> Result<Value, String> {
    if ids.len() > 10_000 {
        return Ok(json!({ "success": false, "error": "TOO_MANY_RULES" }));
    }
    let _folder_leases = match acquire_rule_folder_leases(&state, None) {
        Ok(leases) => leases,
        Err(error) => return Ok(json!({ "success": false, "error": error })),
    };
    match state.db.rules().reorder(&ids) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(error) => Ok(json!({ "success": false, "error": error.to_string() })),
    }
}

struct RuleFolderLeases<'a> {
    registry: &'a CancellationRegistry,
    operation_ids: Vec<Uuid>,
}

impl Drop for RuleFolderLeases<'_> {
    fn drop(&mut self) {
        for operation_id in self.operation_ids.drain(..) {
            self.registry.finish(operation_id);
        }
    }
}

fn acquire_rule_folder_leases<'a>(
    state: &'a AppState,
    rule_id: Option<Uuid>,
) -> Result<RuleFolderLeases<'a>, String> {
    let mut folders = state
        .db
        .folders()
        .list()
        .map_err(|error| error.to_string())?;
    if let Some(rule_id) = rule_id {
        folders.retain(|folder| folder.rule_ids.contains(&rule_id));
    }
    folders.sort_by_key(|folder| {
        crate::paths::safety::canonical_path_key(std::path::Path::new(&folder.path))
    });

    let mut leases = RuleFolderLeases {
        registry: &state.cancellations,
        operation_ids: Vec::with_capacity(folders.len()),
    };
    for folder in folders {
        let operation_id = Uuid::new_v4();
        let folder_key =
            crate::paths::safety::canonical_path_key(std::path::Path::new(&folder.path));
        if !state
            .cancellations
            .begin_folder_operation(operation_id, folder_key)
        {
            return Err("ORGANIZE_ALREADY_RUNNING".to_string());
        }
        leases.operation_ids.push(operation_id);
    }
    Ok(leases)
}
