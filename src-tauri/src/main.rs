// Previne uma janela de console extra no Windows em modo release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod hookmode;
mod i18n;
mod selfconfig;
mod server;
mod settings;
mod state;
mod toast;
mod tray;
mod transcript;
mod usage;

use rusqlite::Connection;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

const DEFAULT_PORT: u16 = 47812;
const CLEANUP_INTERVAL_SECS: u64 = 60;
const USAGE_CACHE_TTL_SECS: u64 = 60;
/// Sessão obsoleta (Stale) sem nenhum evento novo por esse tempo é apagada
/// de vez — não fica acumulando pra sempre esperando um SessionEnd que a
/// ferramenta original pode nunca mandar (processo morto, terminal fechado).
/// Sessões `Ended` ficam de fora dessa purga e são retidas indefinidamente
/// (só saem por delete manual): `state::apply_stale_timeout` nunca transforma
/// `Ended` em `Stale`, então o branch abaixo nunca as alcança.
const STALE_PURGE_AFTER_SECS: i64 = 24 * 60 * 60;

pub struct AppState {
    pub conn: Mutex<Connection>,
    pub settings: Mutex<settings::Settings>,
    pub data_dir: PathBuf,
    pub app_handle: AppHandle,
    pub main_window: WebviewWindow,
    pub toast_window: WebviewWindow,
    /// Última posição conhecida do ícone da bandeja — itens de menu e o
    /// toast (que não recebem coordenadas de clique) usam isso pra se
    /// posicionar colados nela. `None` até o primeiro clique/hover na
    /// bandeja (ex.: logo depois do app abrir) — nesse caso quem lê usa
    /// `fallback_tray_pos` em vez de posicionar no canto (0,0) da tela.
    pub last_tray_pos: Mutex<Option<PhysicalPosition<f64>>>,
    /// Contador monotônico: incrementado a cada toast mostrado, evita que
    /// o timer de auto-hide de um toast antigo esconda um toast mais novo.
    pub toast_generation: Mutex<u64>,
    pub tray_menu: TrayMenuItems,
    pub usage_cache: Mutex<Option<(Instant, usage::AccountUsage)>>,
}

pub struct TrayMenuItems {
    pub open: MenuItem<tauri::Wry>,
    pub settings: MenuItem<tauri::Wry>,
    pub reconfigure: MenuItem<tauri::Wry>,
    pub quit: MenuItem<tauri::Wry>,
}

fn needle_dir() -> PathBuf {
    std::env::temp_dir().join("needle")
}

fn db_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_local_data_dir()
        .expect("app_local_data_dir indisponível")
}

/// Sobe um listener TCP na porta padrão, tentando as próximas se ocupada, e
/// grava a porta escolhida num arquivo conhecido pro modo hook ler.
fn bind_available_port() -> (std::net::TcpListener, u16) {
    for offset in 0..20u16 {
        let port = DEFAULT_PORT + offset;
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if let Ok(listener) = std::net::TcpListener::bind(addr) {
            return (listener, port);
        }
    }
    panic!("nenhuma porta disponível pro servidor local do Needle");
}

fn write_port_file(port: u16) {
    let dir = needle_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("port"), port.to_string());
}

/// Posiciona a janela colada no ícone da bandeja (assumindo o padrão mais
/// comum no Windows: barra de tarefas embaixo, área de notificação à
/// direita) em vez de deixar o SO centralizar a janela na tela.
pub(crate) fn position_near_tray(window: &WebviewWindow, click_pos: PhysicalPosition<f64>) {
    if let Ok(size) = window.outer_size() {
        const MARGIN: f64 = 8.0;
        let x = (click_pos.x - size.width as f64).max(0.0);
        let y = (click_pos.y - size.height as f64 - MARGIN).max(0.0);
        let _ = window.set_position(PhysicalPosition::new(x, y));
    }
}

fn show_near_tray(window: &WebviewWindow, click_pos: PhysicalPosition<f64>) {
    position_near_tray(window, click_pos);
    let _ = window.show();
    let _ = window.set_focus();
}

/// Posição de fallback quando `last_tray_pos` ainda é `None` (nenhum clique
/// ou hover na bandeja ainda aconteceu, ex.: logo após o app abrir): o canto
/// inferior direito do monitor primário, de onde `position_near_tray` já
/// subtrai o tamanho da janela e a margem — resultado é um popover colado
/// nesse canto, convenção usual de toast/tray no Windows.
pub(crate) fn fallback_tray_pos(app_handle: &AppHandle) -> PhysicalPosition<f64> {
    app_handle
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| {
            let size = monitor.size();
            PhysicalPosition::new(size.width as f64, size.height as f64)
        })
        .unwrap_or(PhysicalPosition::new(0.0, 0.0))
}

fn spawn_cleanup_job(app_state: Arc<AppState>) {
    tauri::async_runtime::spawn(async move {
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(CLEANUP_INTERVAL_SECS));
        loop {
            interval.tick().await;
            let now = chrono::Utc::now().timestamp();
            let (waiting_timeout, stale_timeout, lang) = {
                let settings = app_state.settings.lock().unwrap();
                (
                    settings.waiting_timeout_secs,
                    settings.stale_timeout_secs,
                    settings.language,
                )
            };

            let conn = app_state.conn.lock().unwrap();
            let sessions = db::list_sessions(&conn).unwrap_or_default();
            for session in &sessions {
                let seconds_since = now - session.last_event_at;
                let after_waiting =
                    state::apply_waiting_timeout(session.state, seconds_since, waiting_timeout);
                let after_stale =
                    state::apply_stale_timeout(after_waiting, seconds_since, stale_timeout);

                if after_stale == state::SessionState::Stale
                    && seconds_since > STALE_PURGE_AFTER_SECS
                {
                    let _ = db::delete_session(&conn, &session.session_id);
                    let _ = app_state
                        .app_handle
                        .emit("session-removed", &session.session_id);
                    continue;
                }

                if after_stale != session.state {
                    let _ = db::set_session_state(&conn, &session.session_id, after_stale);
                    tray::alert_if_needed(
                        &app_state,
                        &session.session_id,
                        &sessions,
                        session.state,
                        after_stale,
                        lang,
                    );
                }
            }
            drop(conn);

            tray::refresh_from_db(&app_state);
        }
    });
}

fn main() {
    // O Needle é chamado pelo próprio Claude Code como `needle hook`, sem
    // GUI: lê o evento do stdin, repassa pro app rodando e sai. Isso evita
    // depender de Node ou de qualquer runtime externo pro hook funcionar.
    if std::env::args().nth(1).as_deref() == Some("hook") {
        hookmode::run();
        return;
    }

    // Chamado pelo hook NSIS_HOOK_PREUNINSTALL antes do desinstalador
    // apagar o executável: remove as entradas do Needle de
    // ~/.claude/settings.json pra não deixar hook morto configurado.
    if std::env::args().nth(1).as_deref() == Some("remove-hooks") {
        let _ = selfconfig::remove_hooks();
        return;
    }

    tauri::Builder::default()
        // Precisa ser o primeiro plugin: se o Needle já estiver rodando,
        // uma segunda tentativa de abrir só foca a janela existente em vez
        // de criar um segundo processo (e um segundo ícone na bandeja).
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = db_path(&handle);
            std::fs::create_dir_all(&data_dir).expect("falha criando diretório de dados");
            let conn = db::open(&data_dir.join("needle.sqlite")).expect("falha abrindo SQLite");
            let loaded_settings = settings::load(&data_dir);

            let open_item = MenuItem::with_id(
                app,
                "open",
                i18n::menu_label(loaded_settings.language, "open"),
                true,
                None::<&str>,
            )?;
            let settings_item = MenuItem::with_id(
                app,
                "settings",
                i18n::menu_label(loaded_settings.language, "settings"),
                true,
                None::<&str>,
            )?;
            let reconfigure_item = MenuItem::with_id(
                app,
                "reconfigure",
                i18n::menu_label(loaded_settings.language, "reconfigure"),
                true,
                None::<&str>,
            )?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(
                app,
                "quit",
                i18n::menu_label(loaded_settings.language, "quit"),
                true,
                None::<&str>,
            )?;
            let menu = Menu::with_items(
                app,
                &[
                    &open_item,
                    &settings_item,
                    &reconfigure_item,
                    &separator,
                    &quit_item,
                ],
            )?;

            let window = app.get_webview_window("main").unwrap();
            let toast_window = app.get_webview_window("toast").unwrap();

            let app_state = Arc::new(AppState {
                conn: Mutex::new(conn),
                settings: Mutex::new(loaded_settings),
                data_dir: data_dir.clone(),
                app_handle: handle.clone(),
                main_window: window.clone(),
                toast_window,
                last_tray_pos: Mutex::new(None),
                toast_generation: Mutex::new(0),
                tray_menu: TrayMenuItems {
                    open: open_item.clone(),
                    settings: settings_item.clone(),
                    reconfigure: reconfigure_item.clone(),
                    quit: quit_item.clone(),
                },
                usage_cache: Mutex::new(None),
            });
            app.manage(app_state.clone());

            if loaded_settings.autostart {
                let _ = handle.autolaunch().enable();
            }

            let (listener, port) = bind_available_port();
            write_port_file(port);
            listener
                .set_nonblocking(true)
                .expect("falha configurando listener non-blocking");

            let router_state = app_state.clone();
            tauri::async_runtime::spawn(async move {
                let listener = tokio::net::TcpListener::from_std(listener)
                    .expect("falha convertendo listener pro runtime async");
                axum::serve(listener, server::router(router_state))
                    .await
                    .expect("servidor HTTP do Needle caiu");
            });

            spawn_cleanup_job(app_state.clone());

            // Auto-configuração: registra o próprio executável como hook do
            // Claude Code em ~/.claude/settings.json. Idempotente — corrige
            // o caminho sozinho se o app for movido ou atualizado.
            if let Ok(exe_path) = std::env::current_exe() {
                let exe_path = exe_path.display().to_string();
                match selfconfig::ensure_hooks_registered(&exe_path) {
                    Ok(true) => {
                        use tauri_plugin_notification::NotificationExt;
                        let (title, body) =
                            i18n::hooks_configured_notification(loaded_settings.language);
                        let _ = handle.notification().builder().title(title).body(body).show();
                    }
                    Ok(false) => {}
                    Err(err) => eprintln!("falha ao auto-configurar hooks: {err}"),
                }
            }

            let app_state_for_menu = app_state.clone();
            let app_state_for_tray_events = app_state.clone();

            let window_for_click = window.clone();
            let window_for_open = window.clone();
            let window_for_settings = window.clone();
            let window_for_blur = window.clone();
            let handle_for_reconfigure = handle.clone();

            // Fecha a janela quando ela perde o foco, como um popover normal
            // de bandeja — evita a sensação de janela "perdida" no meio da
            // tela depois de clicar em outro lugar.
            window.on_window_event(move |event| {
                if let WindowEvent::Focused(false) = event {
                    let _ = window_for_blur.hide();
                }
            });

            TrayIconBuilder::with_id(tray::TRAY_ID)
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .tooltip("Needle")
                .on_menu_event(move |app, event| {
                    let pos = app_state_for_menu
                        .last_tray_pos
                        .lock()
                        .unwrap()
                        .unwrap_or_else(|| fallback_tray_pos(app));
                    match event.id.as_ref() {
                        "quit" => app.exit(0),
                        "open" => {
                            show_near_tray(&window_for_open, pos);
                            let _ = window_for_open.emit("show-view", "sessions");
                        }
                        "settings" => {
                            show_near_tray(&window_for_settings, pos);
                            let _ = window_for_settings.emit("show-view", "settings");
                        }
                        "reconfigure" => {
                            if let Ok(exe_path) = std::env::current_exe() {
                                let exe_path = exe_path.display().to_string();
                                let _ = selfconfig::ensure_hooks_registered(&exe_path);
                            }
                            use tauri_plugin_notification::NotificationExt;
                            let lang = app_state_for_menu.settings.lock().unwrap().language;
                            let _ = handle_for_reconfigure
                                .notification()
                                .builder()
                                .title("Needle")
                                .body(i18n::hooks_reconfigured_body(lang))
                                .show();
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |_tray, event| match event {
                    // Um clique físico dispara Down e depois Up; reagir só
                    // ao Up evita abrir-e-fechar a janela no mesmo clique.
                    // Sempre mostra (em vez de alternar): perder o foco já
                    // esconde a janela (ver on_window_event acima), então um
                    // toggle aqui entraria em corrida com o hide do blur.
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        position,
                        ..
                    } => {
                        *app_state_for_tray_events.last_tray_pos.lock().unwrap() = Some(position);
                        show_near_tray(&window_for_click, position);
                    }
                    TrayIconEvent::Enter { position, .. } => {
                        *app_state_for_tray_events.last_tray_pos.lock().unwrap() = Some(position);
                    }
                    _ => {}
                })
                .build(app)?;

            tray::refresh_from_db(&app_state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            get_settings,
            save_settings,
            get_hook_status,
            reconfigure_hooks,
            remove_hooks_command,
            get_account_usage,
            delete_session,
            open_panel_from_toast,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao rodar app Needle");
}

#[tauri::command]
fn list_sessions(state: tauri::State<Arc<AppState>>) -> Vec<db::SessionRow> {
    let conn = state.conn.lock().unwrap();
    db::list_sessions(&conn).unwrap_or_default()
}

#[tauri::command]
fn get_settings(state: tauri::State<Arc<AppState>>) -> settings::Settings {
    *state.settings.lock().unwrap()
}

#[tauri::command]
fn save_settings(
    app: AppHandle,
    state: tauri::State<Arc<AppState>>,
    new_settings: settings::Settings,
) -> Result<(), String> {
    let previous_language = state.settings.lock().unwrap().language;
    settings::save(&state.data_dir, &new_settings).map_err(|e| e.to_string())?;
    *state.settings.lock().unwrap() = new_settings;

    if new_settings.language != previous_language {
        let _ = state
            .tray_menu
            .open
            .set_text(i18n::menu_label(new_settings.language, "open"));
        let _ = state
            .tray_menu
            .settings
            .set_text(i18n::menu_label(new_settings.language, "settings"));
        let _ = state
            .tray_menu
            .reconfigure
            .set_text(i18n::menu_label(new_settings.language, "reconfigure"));
        let _ = state
            .tray_menu
            .quit
            .set_text(i18n::menu_label(new_settings.language, "quit"));
        tray::refresh_from_db(&state);
    }

    let autolaunch = app.autolaunch();
    let result = if new_settings.autostart {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
fn get_hook_status() -> selfconfig::HookStatus {
    let exe_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    selfconfig::status(&exe_path)
}

#[tauri::command]
fn reconfigure_hooks() -> Result<bool, String> {
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    selfconfig::ensure_hooks_registered(&exe_path.display().to_string()).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_hooks_command() -> Result<bool, String> {
    selfconfig::remove_hooks().map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_account_usage(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<usage::AccountUsage, String> {
    {
        let cache = state.usage_cache.lock().unwrap();
        if let Some((fetched_at, cached)) = cache.as_ref() {
            if fetched_at.elapsed().as_secs() < USAGE_CACHE_TTL_SECS {
                return Ok(cached.clone());
            }
        }
    }

    let fresh = usage::fetch_account_usage().await?;
    *state.usage_cache.lock().unwrap() = Some((Instant::now(), fresh.clone()));
    Ok(fresh)
}

/// Apaga uma sessão manualmente. Só permitido pra sessões em estado
/// terminal (`Stale` ou `Ended`) — o botão de delete na UI só existe pra
/// elas, e essa checagem no backend evita que uma sessão ativa suma por
/// engano.
#[tauri::command]
fn delete_session(state: tauri::State<Arc<AppState>>, session_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().unwrap();
    let current = db::get_session_state(&conn, &session_id).map_err(|e| e.to_string())?;
    if !state::is_manually_deletable(current) {
        return Ok(false);
    }
    db::delete_session(&conn, &session_id).map_err(|e| e.to_string())?;
    drop(conn);
    let _ = state.app_handle.emit("session-removed", &session_id);
    Ok(true)
}

/// Chamado ao clicar no toast in-app: abre o painel principal colado na
/// bandeja e muda pra aba Sessões — mesmo comportamento de clicar no
/// ícone da bandeja.
#[tauri::command]
fn open_panel_from_toast(state: tauri::State<Arc<AppState>>) {
    let pos = state
        .last_tray_pos
        .lock()
        .unwrap()
        .unwrap_or_else(|| fallback_tray_pos(&state.app_handle));
    show_near_tray(&state.main_window, pos);
    let _ = state.main_window.emit("show-view", "sessions");
}
