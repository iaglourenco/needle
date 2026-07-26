use tauri::tray::TrayIconId;
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

use crate::db;
use crate::state::{worst_state, SessionState};
use crate::AppState;

pub const TRAY_ID: &str = "main-tray";

fn tooltip_for(state: Option<SessionState>) -> String {
    match state {
        None => "Needle — nenhuma sessão ativa".to_string(),
        Some(SessionState::NeedsAttention) => "Needle — sessão precisa de atenção".to_string(),
        Some(SessionState::Error) => "Needle — erro em uma sessão".to_string(),
        Some(SessionState::WaitingInput) => "Needle — aguardando input".to_string(),
        Some(SessionState::Running) => "Needle — sessões em execução".to_string(),
        Some(SessionState::Idle) => "Needle — sessões ociosas".to_string(),
        Some(SessionState::Stale | SessionState::Ended) => "Needle".to_string(),
    }
}

fn should_notify(new_state: SessionState) -> bool {
    matches!(
        new_state,
        SessionState::NeedsAttention | SessionState::WaitingInput | SessionState::Error
    )
}

/// Recalcula o pior estado agregado entre todas as sessões ativas, atualiza
/// o tooltip da bandeja, emite o evento pro frontend e dispara notificação
/// OS se a sessão acabou de entrar num estado que pede atenção.
pub fn on_session_changed(
    app_state: &AppState,
    session_id: &str,
    previous_state: SessionState,
    new_state: SessionState,
) {
    let conn = app_state.conn.lock().unwrap();
    let sessions = db::list_sessions(&conn).unwrap_or_default();
    drop(conn);

    let worst = worst_state(sessions.iter().map(|s| s.state));

    if let Some(session) = sessions.iter().find(|s| s.session_id == session_id) {
        let _ = app_state.app_handle.emit("session-updated", session);
    }
    if new_state == SessionState::Ended {
        let _ = app_state.app_handle.emit("session-removed", session_id);
    }

    if let Some(tray) = app_state
        .app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
    {
        let _ = tray.set_tooltip(Some(tooltip_for(worst)));
    }

    notify_if_needed(app_state, session_id, &sessions, previous_state, new_state);
}

/// Recalcula tooltip/badge a partir do banco, sem evento específico associado
/// (usado pelo job de limpeza periódica após timeouts).
pub fn refresh_from_db(app_state: &AppState) {
    let conn = app_state.conn.lock().unwrap();
    let sessions = db::list_sessions(&conn).unwrap_or_default();
    drop(conn);

    let worst = worst_state(sessions.iter().map(|s| s.state));
    if let Some(tray) = app_state.app_handle.tray_by_id(&TrayIconId::new(TRAY_ID)) {
        let _ = tray.set_tooltip(Some(tooltip_for(worst)));
    }
    for session in &sessions {
        let _ = app_state.app_handle.emit("session-updated", session);
    }
}

fn notify_if_needed(
    app_state: &AppState,
    session_id: &str,
    sessions: &[db::SessionRow],
    previous_state: SessionState,
    new_state: SessionState,
) {
    if previous_state != new_state && should_notify(new_state) {
        let title = match new_state {
            SessionState::NeedsAttention => "Needle: sessão precisa de atenção",
            SessionState::Error => "Needle: erro numa sessão",
            SessionState::WaitingInput => "Needle: aguardando sua resposta",
            _ => "Needle",
        };
        let body = sessions
            .iter()
            .find(|s| s.session_id == session_id)
            .map(|s| s.cwd.clone())
            .unwrap_or_default();

        let _ = app_state
            .app_handle
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    }
}
