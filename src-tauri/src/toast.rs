use std::sync::Arc;

use serde::Serialize;
use tauri::Emitter;

use crate::AppState;

/// Quantos segundos o toast fica visível antes de sumir sozinho.
const VISIBLE_SECS: u64 = 5;

#[derive(Clone, Serialize)]
struct ToastPayload<'a> {
    title: &'a str,
    body: &'a str,
}

/// Mostra o toast in-app perto da bandeja — substituto da notificação
/// nativa quando `Settings.notifications_enabled` está desligado (ver
/// `tray::alert_if_needed`, o único chamador). Um toast de cada vez: um
/// novo sempre reposiciona a janela, reemite o conteúdo e reagenda o
/// auto-hide, nunca empilha com um toast anterior ainda visível.
pub fn show(app_state: &Arc<AppState>, title: &str, body: &str) {
    let pos = *app_state.last_tray_pos.lock().unwrap();
    crate::position_near_tray(&app_state.toast_window, pos);

    let _ = app_state
        .toast_window
        .emit("toast-show", ToastPayload { title, body });
    let _ = app_state.toast_window.show();

    let generation = {
        let mut gen_lock = app_state.toast_generation.lock().unwrap();
        *gen_lock += 1;
        *gen_lock
    };

    let app_state = app_state.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(VISIBLE_SECS)).await;
        if *app_state.toast_generation.lock().unwrap() == generation {
            let _ = app_state.toast_window.hide();
        }
    });
}
