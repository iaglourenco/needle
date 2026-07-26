use axum::{extract::State, routing::post, Json, Router};
use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;

use crate::state::{transition, HookEvent};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct HookPayload {
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    pub hook_event_name: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub tool_response: Option<Value>,
}

fn is_tool_error(payload: &HookPayload) -> bool {
    match &payload.tool_response {
        Some(Value::Object(map)) => map
            .get("error")
            .map(|v| !v.is_null())
            .unwrap_or(false)
            || map
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        _ => false,
    }
}

fn to_hook_event(payload: &HookPayload) -> Option<HookEvent> {
    match payload.hook_event_name.as_str() {
        "SessionStart" => Some(HookEvent::SessionStart),
        "UserPromptSubmit" => Some(HookEvent::UserPromptSubmit),
        "PreToolUse" => Some(HookEvent::PreToolUse),
        "PostToolUse" => Some(HookEvent::PostToolUse {
            is_error: is_tool_error(payload),
        }),
        "Notification" => Some(HookEvent::Notification),
        "Stop" => Some(HookEvent::Stop),
        "SubagentStop" => Some(HookEvent::SubagentStop),
        "SessionEnd" => Some(HookEvent::SessionEnd),
        "PreCompact" => Some(HookEvent::PreCompact),
        _ => None,
    }
}

fn snippet_for(payload: &HookPayload) -> Option<String> {
    if let Some(message) = &payload.message {
        return Some(truncate(message));
    }
    payload.tool_name.as_ref().map(|name| truncate(name))
}

fn truncate(s: &str) -> String {
    const MAX: usize = 120;
    if s.chars().count() > MAX {
        s.chars().take(MAX).collect::<String>() + "…"
    } else {
        s.to_string()
    }
}

async fn handle_event(State(app_state): State<Arc<AppState>>, Json(payload): Json<HookPayload>) {
    let Some(event) = to_hook_event(&payload) else {
        return;
    };

    let now = chrono::Utc::now().timestamp();
    let cwd = payload.cwd.clone().unwrap_or_default();
    let snippet = snippet_for(&payload);

    let conn = app_state.conn.lock().unwrap();
    let current = crate::db::get_session_state(&conn, &payload.session_id)
        .ok()
        .flatten()
        .unwrap_or(crate::state::SessionState::Idle);
    let new_state = transition(current, event);

    if crate::db::upsert_session(
        &conn,
        &payload.session_id,
        &cwd,
        now,
        new_state,
        snippet.as_deref(),
    )
    .is_err()
    {
        return;
    }

    let payload_json = serde_json::to_string(&payload.tool_response).unwrap_or_default();
    let _ = crate::db::insert_event(
        &conn,
        &payload.session_id,
        &payload.hook_event_name,
        &payload_json,
        now,
    );
    drop(conn);

    crate::tray::on_session_changed(&app_state, &payload.session_id, current, new_state);
}

pub fn router(app_state: Arc<AppState>) -> Router {
    Router::new()
        .route("/event", post(handle_event))
        .with_state(app_state)
}
