use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::domain::Rule;
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
    rule.created_at = chrono::Utc::now();
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
    let mut current = match state.db.rules().get_by_ids(&[id]) {
        Ok(rules) if !rules.is_empty() => rules.into_iter().next().unwrap(),
        _ => return Ok(json!({ "success": false, "error": "RULE_NOT_FOUND" })),
    };

    // Patch fields from the updates map.
    if let Some(obj) = updates.as_object() {
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            current.name = name.to_string();
        }
        if let Some(enabled) = obj.get("enabled").and_then(|v| v.as_bool()) {
            current.enabled = enabled;
        }
        if let Some(order) = obj.get("order").and_then(|v| v.as_i64()) {
            current.order = order as i32;
        }
        if let Some(icon) = obj.get("icon").and_then(|v| v.as_str()) {
            current.icon = icon.to_string();
        }
        if let Some(conditions) = obj.get("conditions") {
            if let Ok(c) = serde_json::from_value::<Vec<_>>(conditions.clone()) {
                current.conditions = c;
            }
        }
        if let Some(action) = obj.get("action") {
            if let Ok(a) = serde_json::from_value::<_>(action.clone()) {
                current.action = a;
            }
        }
    }

    match state.db.rules().update(&current) {
        Ok(()) => Ok(json!({ "success": true, "data": current })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn delete_rule(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    match state.db.rules().delete(id) {
        Ok(()) => Ok(json!({ "success": true })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}

#[tauri::command]
pub async fn toggle(state: State<'_, AppState>, id: Uuid) -> Result<Value, String> {
    match state.db.rules().toggle(id) {
        Ok(new_value) => Ok(json!({ "success": true, "data": { "enabled": new_value } })),
        Err(e) => Ok(json!({ "success": false, "error": e.to_string() })),
    }
}
