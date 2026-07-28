use tauri::tray::TrayIconId;
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

use crate::db;
use crate::i18n;
use crate::settings::Language;
use crate::state::{worst_state, SessionState};
use crate::transcript;
use crate::AppState;

pub const TRAY_ID: &str = "main-tray";

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
    let mut sessions = db::list_sessions(&conn).unwrap_or_default();

    if let Some(session) = sessions.iter_mut().find(|s| s.session_id == session_id) {
        if let Some(usage) = transcript::read_session_usage(&session.cwd, session_id) {
            let _ = db::set_session_usage(&conn, session_id, &usage.model, usage.cost_usd);
            session.model = Some(usage.model);
            session.cost_usd = Some(usage.cost_usd);
        }
    }
    drop(conn);
    let lang = app_state.settings.lock().unwrap().language;

    let worst = worst_state(sessions.iter().map(|s| s.state));

    if let Some(session) = sessions.iter().find(|s| s.session_id == session_id) {
        let _ = app_state.app_handle.emit("session-updated", session);
    }

    if let Some(tray) = app_state
        .app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
    {
        let _ = tray.set_tooltip(Some(i18n::tray_tooltip(lang, worst)));
    }

    notify_if_needed(app_state, session_id, &sessions, previous_state, new_state, lang);
}

/// Recalcula tooltip/badge a partir do banco, sem evento específico associado
/// (usado pelo job de limpeza periódica após timeouts).
pub fn refresh_from_db(app_state: &AppState) {
    let conn = app_state.conn.lock().unwrap();
    let sessions = db::list_sessions(&conn).unwrap_or_default();
    drop(conn);
    let lang = app_state.settings.lock().unwrap().language;

    let worst = worst_state(sessions.iter().map(|s| s.state));
    if let Some(tray) = app_state.app_handle.tray_by_id(&TrayIconId::new(TRAY_ID)) {
        let _ = tray.set_tooltip(Some(i18n::tray_tooltip(lang, worst)));
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
    lang: Language,
) {
    if previous_state != new_state && should_notify(new_state) {
        let title = i18n::notif_title(lang, new_state);
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
