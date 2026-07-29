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
        let _ = tray.set_icon(icon_for_worst(app_state, worst));
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
        let _ = tray.set_icon(icon_for_worst(app_state, worst));
    }
    for session in &sessions {
        let _ = app_state.app_handle.emit("session-updated", session);
    }
}

/// Guarda pura: só notifica se o usuário não desligou notificações do SO
/// (`Settings.notifications_enabled`) e a transição pede atenção.
fn should_send_notification(
    enabled: bool,
    previous_state: SessionState,
    new_state: SessionState,
) -> bool {
    enabled && previous_state != new_state && should_notify(new_state)
}

/// `pub(crate)`: além de `on_session_changed`, o job de limpeza periódica
/// (`main.rs::spawn_cleanup_job`) também chama isso diretamente — é o
/// único lugar que promove sessão pra `NeedsAttention` (via
/// `apply_waiting_timeout`), transição que nenhum hook do Claude Code gera
/// sozinho.
pub(crate) fn notify_if_needed(
    app_state: &AppState,
    session_id: &str,
    sessions: &[db::SessionRow],
    previous_state: SessionState,
    new_state: SessionState,
    lang: Language,
) {
    let enabled = app_state.settings.lock().unwrap().notifications_enabled;
    if !should_send_notification(enabled, previous_state, new_state) {
        return;
    }

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

/// Cor do ponto na bandeja pro estado dado — mesma paleta usada em
/// `SessionItem.vue` (`stateColor`). `None` pros estados terminais, que
/// não acendem o ícone (ele volta pro logo padrão nesse caso).
fn icon_color_for(state: SessionState) -> Option<(u8, u8, u8)> {
    match state {
        SessionState::Running => Some((0x3b, 0x82, 0xf6)),
        SessionState::WaitingInput => Some((0xea, 0xb3, 0x08)),
        SessionState::NeedsAttention | SessionState::Error => Some((0xef, 0x44, 0x44)),
        SessionState::Idle => Some((0x22, 0xc5, 0x5e)),
        SessionState::Stale | SessionState::Ended => None,
    }
}

/// Desenha um círculo preenchido 32x32 sobre fundo transparente, usado
/// como ícone da bandeja quando há sessão em estado que pede atenção.
fn dot_icon(color: (u8, u8, u8)) -> tauri::image::Image<'static> {
    const SIZE: u32 = 32;
    let (r, g, b) = color;
    let radius = SIZE as f32 / 2.0;
    let center = radius - 0.5;
    let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius - 1.0 {
                let idx = ((y * SIZE + x) * 4) as usize;
                rgba[idx] = r;
                rgba[idx + 1] = g;
                rgba[idx + 2] = b;
                rgba[idx + 3] = 255;
            }
        }
    }
    tauri::image::Image::new_owned(rgba, SIZE, SIZE)
}

/// Ícone a exibir na bandeja pro pior estado agregado: círculo colorido
/// se houver sessão com severidade > 0, senão volta pro logo padrão do
/// Needle (sem sessão, ou só sessões `Stale`/`Ended`).
fn icon_for_worst(
    app_state: &AppState,
    worst: Option<SessionState>,
) -> Option<tauri::image::Image<'static>> {
    match worst.and_then(icon_color_for) {
        Some(color) => Some(dot_icon(color)),
        None => app_state
            .app_handle
            .default_window_icon()
            .map(|icon| icon.clone().to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_send_notification_respects_enabled_flag_and_transition() {
        assert!(should_send_notification(
            true,
            SessionState::Running,
            SessionState::WaitingInput
        ));
        assert!(!should_send_notification(
            false,
            SessionState::Running,
            SessionState::WaitingInput
        ));
        assert!(!should_send_notification(
            true,
            SessionState::WaitingInput,
            SessionState::WaitingInput
        ));
        assert!(!should_send_notification(
            true,
            SessionState::Running,
            SessionState::Idle
        ));
        assert!(should_send_notification(
            true,
            SessionState::Running,
            SessionState::NeedsAttention
        ));
        assert!(should_send_notification(
            true,
            SessionState::Running,
            SessionState::Error
        ));
        assert!(!should_send_notification(
            true,
            SessionState::Running,
            SessionState::Stale
        ));
        assert!(!should_send_notification(
            true,
            SessionState::Running,
            SessionState::Ended
        ));
    }

    #[test]
    fn icon_color_for_maps_each_severity_bearing_state() {
        assert_eq!(
            icon_color_for(SessionState::Running),
            Some((0x3b, 0x82, 0xf6))
        );
        assert_eq!(
            icon_color_for(SessionState::WaitingInput),
            Some((0xea, 0xb3, 0x08))
        );
        assert_eq!(
            icon_color_for(SessionState::NeedsAttention),
            Some((0xef, 0x44, 0x44))
        );
        assert_eq!(icon_color_for(SessionState::Error), Some((0xef, 0x44, 0x44)));
        assert_eq!(icon_color_for(SessionState::Idle), Some((0x22, 0xc5, 0x5e)));
    }

    #[test]
    fn icon_color_for_terminal_states_is_none() {
        assert_eq!(icon_color_for(SessionState::Stale), None);
        assert_eq!(icon_color_for(SessionState::Ended), None);
    }

    #[test]
    fn dot_icon_is_32x32_rgba() {
        let icon = dot_icon((0x3b, 0x82, 0xf6));
        assert_eq!(icon.width(), 32);
        assert_eq!(icon.height(), 32);
        assert_eq!(icon.rgba().len(), 32 * 32 * 4);
    }

    #[test]
    fn dot_icon_paints_center_and_leaves_corners_transparent() {
        let icon = dot_icon((0x3b, 0x82, 0xf6));
        let rgba = icon.rgba();
        let idx = |x: u32, y: u32| ((y * 32 + x) * 4) as usize;

        let center = idx(16, 16);
        assert_eq!(&rgba[center..center + 4], &[0x3b, 0x82, 0xf6, 255]);

        let corner = idx(0, 0);
        assert_eq!(rgba[corner + 3], 0, "corner pixel should be transparent");
    }

    #[test]
    fn dot_icon_is_symmetric_with_correct_extent() {
        let icon = dot_icon((0x3b, 0x82, 0xf6));
        let rgba = icon.rgba();
        let idx = |x: u32, y: u32| ((y * 32 + x) * 4) as usize;
        let filled = |x: u32, y: u32| rgba[idx(x, y) + 3] == 255;

        // 4-fold symmetry catches an off-center circle
        for (x, y) in [(1u32, 15u32), (30, 15), (15, 1), (15, 30)] {
            assert!(filled(x, y), "({x},{y}) should be inside the disc");
        }
        // 1px transparent margin catches a too-large radius
        for (x, y) in [(0u32, 15u32), (31, 15), (15, 0), (15, 31)] {
            assert!(!filled(x, y), "({x},{y}) should be outside the disc");
        }
    }

    #[test]
    fn icon_color_for_matches_severity_gate() {
        use SessionState::*;
        for s in [
            Running,
            WaitingInput,
            NeedsAttention,
            Idle,
            Error,
            Stale,
            Ended,
        ] {
            assert_eq!(icon_color_for(s).is_some(), s.severity() > 0, "{s:?}");
        }
    }
}
