# In-App Toast Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the user has turned off native Windows notifications
(`Settings.notifications_enabled = false`), show a small custom toast
window near the tray instead, so they're never left with zero warning.

**Architecture:** Task 1 promotes window/position state that today lives
as local variables in `main.rs`'s `setup()` closure into `AppState`, adds
a second Tauri window (`"toast"`), and adds a `open_panel_from_toast`
command — pure plumbing, no new behavior yet. Task 2 adds the `toast`
module (shows/positions/auto-hides the window) and rewrites
`tray.rs`'s notification gate to pick a channel (native vs. toast) based
on the setting. Task 3 adds the frontend: a window-label check in
`main.ts` that mounts a new minimal `ToastView.vue` instead of the usual
`App.vue` when running inside the `toast` window.

**Tech Stack:** Rust (`tauri` 2, `tokio`, `serde`), Vue 3 + TypeScript.

## Global Constraints

- Toast shows **only** when `notifications_enabled == false`. Never both
  channels at once, never silence.
- Content matches what the native notification would have shown: title
  from `i18n::notif_title`, body = session `cwd`.
- One toast at a time — a new one always supersedes/repositions/reschedules
  the auto-hide of any currently-showing toast, never stacks.
- Clicking the toast opens the main panel on the Sessions tab (same as
  clicking the tray icon) — no scrolling/highlighting to a specific
  session; that's out of scope.
- The "hooks configured"/"hooks reconfigured" notifications in `main.rs`
  are untouched — always native, unaffected by this feature (same
  boundary as the previous notifications-toggle feature).
- Verify Rust changes with `cargo test` (from `src-tauri/`) and
  `cargo build` — clean, no warnings. Verify frontend changes with
  `npm run build`. Window creation, positioning, and the auto-hide timer
  are not unit-testable (they need a live Tauri runtime) — manual
  verification is expected for those, same as prior tray/window features
  in this project.

---

### Task 1: Promote window/position state into `AppState`, add the `toast` window

**Files:**
- Modify: `src-tauri/tauri.conf.json` (new window entry)
- Modify: `src-tauri/src/main.rs` (`AppState` struct, `setup()` closure,
  new `open_panel_from_toast` command, `position_near_tray` visibility)

**Interfaces:**
- Produces: `AppState.main_window: WebviewWindow`,
  `AppState.toast_window: WebviewWindow`,
  `AppState.last_tray_pos: Mutex<PhysicalPosition<f64>>`,
  `AppState.toast_generation: Mutex<u64>` — Task 2's `toast::show` reads
  all four of these directly by field name.
- Produces: `pub(crate) fn position_near_tray(window: &WebviewWindow,
  click_pos: PhysicalPosition<f64>)` in `main.rs` (visibility widened
  from private) — Task 2's `toast::show` calls this as
  `crate::position_near_tray(...)`.
- Produces: `#[tauri::command] fn open_panel_from_toast(state:
  tauri::State<Arc<AppState>>)` — Task 3's `ToastView.vue` invokes this
  by name (`"open_panel_from_toast"`) with no arguments.

- [ ] **Step 1: Add the `toast` window to `tauri.conf.json`**

In `src-tauri/tauri.conf.json`, the `app.windows` array currently has one
entry (`"main"`). Add a second entry right after it:

```json
    "windows": [
      {
        "label": "main",
        "title": "Needle",
        "width": 340,
        "height": 480,
        "visible": false,
        "decorations": false,
        "resizable": false,
        "skipTaskbar": true,
        "alwaysOnTop": true,
        "shadow": true,
        "center": false
      },
      {
        "label": "toast",
        "title": "Needle",
        "width": 320,
        "height": 84,
        "visible": false,
        "decorations": false,
        "resizable": false,
        "skipTaskbar": true,
        "alwaysOnTop": true,
        "shadow": true,
        "center": false
      }
    ],
```

- [ ] **Step 2: Widen `position_near_tray`'s visibility and add fields to `AppState`**

In `src-tauri/src/main.rs`, `position_near_tray` currently reads:

```rust
fn position_near_tray(window: &WebviewWindow, click_pos: PhysicalPosition<f64>) {
```

Change to:

```rust
pub(crate) fn position_near_tray(window: &WebviewWindow, click_pos: PhysicalPosition<f64>) {
```

`AppState` currently reads:

```rust
pub struct AppState {
    pub conn: Mutex<Connection>,
    pub settings: Mutex<settings::Settings>,
    pub data_dir: PathBuf,
    pub app_handle: AppHandle,
    pub tray_menu: TrayMenuItems,
    pub usage_cache: Mutex<Option<(Instant, usage::AccountUsage)>>,
}
```

Replace with:

```rust
pub struct AppState {
    pub conn: Mutex<Connection>,
    pub settings: Mutex<settings::Settings>,
    pub data_dir: PathBuf,
    pub app_handle: AppHandle,
    pub main_window: WebviewWindow,
    pub toast_window: WebviewWindow,
    /// Última posição conhecida do ícone da bandeja — itens de menu e o
    /// toast (que não recebem coordenadas de clique) usam isso pra se
    /// posicionar colados nela.
    pub last_tray_pos: Mutex<PhysicalPosition<f64>>,
    /// Contador monotônico: incrementado a cada toast mostrado, evita que
    /// o timer de auto-hide de um toast antigo esconda um toast mais novo.
    pub toast_generation: Mutex<u64>,
    pub tray_menu: TrayMenuItems,
    pub usage_cache: Mutex<Option<(Instant, usage::AccountUsage)>>,
}
```

- [ ] **Step 3: Replace the whole `setup()` closure**

This step reorders window/state creation so `main_window`/`toast_window`
exist before `AppState` is built (they need to go inside it), and moves
`last_tray_pos` from a local `Arc<Mutex<...>>` into `AppState`'s new
field — every tray/menu closure that used to capture the local
`last_tray_pos`/`last_tray_pos_for_event` now reads/writes
`AppState.last_tray_pos` through a cloned `Arc<AppState>` instead.

In `src-tauri/src/main.rs`, the entire `.setup(|app| { ... })` closure
(from `.setup(|app| {` through its matching `})` right before
`.invoke_handler(...)`) currently reads:

```rust
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

            let app_state = Arc::new(AppState {
                conn: Mutex::new(conn),
                settings: Mutex::new(loaded_settings),
                data_dir: data_dir.clone(),
                app_handle: handle.clone(),
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

            let window = app.get_webview_window("main").unwrap();
            let window_for_click = window.clone();
            let window_for_open = window.clone();
            let window_for_settings = window.clone();
            let window_for_blur = window.clone();
            let handle_for_reconfigure = handle.clone();

            // Guarda a última posição conhecida do ícone da bandeja, pra
            // itens de menu (que não recebem coordenadas de clique) também
            // conseguirem abrir a janela colada nela.
            let last_tray_pos = Arc::new(Mutex::new(PhysicalPosition::new(0.0, 0.0)));
            let last_tray_pos_for_event = last_tray_pos.clone();

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
                    let pos = *last_tray_pos.lock().unwrap();
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
                        *last_tray_pos_for_event.lock().unwrap() = position;
                        show_near_tray(&window_for_click, position);
                    }
                    TrayIconEvent::Enter { position, .. } => {
                        *last_tray_pos_for_event.lock().unwrap() = position;
                    }
                    _ => {}
                })
                .build(app)?;

            tray::refresh_from_db(&app_state);

            Ok(())
        })
```

Replace the entire block with:

```rust
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
                toast_window: toast_window.clone(),
                last_tray_pos: Mutex::new(PhysicalPosition::new(0.0, 0.0)),
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
                    let pos = *app_state_for_menu.last_tray_pos.lock().unwrap();
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
                        *app_state_for_tray_events.last_tray_pos.lock().unwrap() = position;
                        show_near_tray(&window_for_click, position);
                    }
                    TrayIconEvent::Enter { position, .. } => {
                        *app_state_for_tray_events.last_tray_pos.lock().unwrap() = position;
                    }
                    _ => {}
                })
                .build(app)?;

            tray::refresh_from_db(&app_state);

            Ok(())
        })
```

- [ ] **Step 4: Add the `open_panel_from_toast` command**

Add this new command in `src-tauri/src/main.rs`, near the other
`#[tauri::command]` functions (e.g. right after `delete_session` at the
end of the file):

```rust
/// Chamado ao clicar no toast in-app: abre o painel principal colado na
/// bandeja e muda pra aba Sessões — mesmo comportamento de clicar no
/// ícone da bandeja.
#[tauri::command]
fn open_panel_from_toast(state: tauri::State<Arc<AppState>>) {
    let pos = *state.last_tray_pos.lock().unwrap();
    show_near_tray(&state.main_window, pos);
    let _ = state.main_window.emit("show-view", "sessions");
}
```

Then add it to the `invoke_handler` list, which currently reads:

```rust
        .invoke_handler(tauri::generate_handler![
            list_sessions,
            get_settings,
            save_settings,
            get_hook_status,
            reconfigure_hooks,
            remove_hooks_command,
            get_account_usage,
            delete_session,
        ])
```

Change to:

```rust
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
```

- [ ] **Step 5: Build and test**

Run (from `src-tauri/`): `cargo test`
Expected: all pre-existing tests still pass (no logic changed, only
struct/closure restructuring).

Run: `cargo build`
Expected: compiles cleanly, no warnings.

- [ ] **Step 6: Manual smoke check**

Run: `npm run tauri dev`. Confirm the app still starts, the tray icon
still opens the panel on click (both left-click and the "Abrir
Needle"/"Open Needle" menu item), positioned next to the tray exactly as
before — this step only moved where `last_tray_pos` lives, it must not
change that behavior at all.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/main.rs
git commit -m "refactor: promote window/tray-position state into AppState, add toast window"
```

---

### Task 2: `toast` module + channel-selection rewrite in `tray.rs`

**Files:**
- Create: `src-tauri/src/toast.rs`
- Modify: `src-tauri/src/main.rs` (`mod toast;` declaration, rename call
  site in `spawn_cleanup_job`)
- Modify: `src-tauri/src/tray.rs` (`should_send_notification` →
  `should_alert`, `notify_if_needed` → `alert_if_needed`, tests)

**Interfaces:**
- Consumes (from Task 1): `AppState.main_window`, `.toast_window`,
  `.last_tray_pos`, `.toast_generation`; `crate::position_near_tray`.
- Produces: `pub fn toast::show(app_state: &Arc<AppState>, title: &str,
  body: &str)` — Task 2 itself is the only caller (from
  `tray::alert_if_needed`); no later task depends on it directly, but
  Task 3's frontend depends on the `"toast-show"` event name and payload
  shape (`{ title, body }`) that this function emits.
- Produces: `pub(crate) fn tray::alert_if_needed(app_state: &Arc<AppState>,
  session_id: &str, sessions: &[db::SessionRow], previous_state:
  SessionState, new_state: SessionState, lang: Language)` (renamed from
  `notify_if_needed`) — no later task calls this directly.

- [ ] **Step 1: Write the failing test for `should_alert`**

In `src-tauri/src/tray.rs`'s `#[cfg(test)] mod tests` block, the existing
test currently reads:

```rust
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
```

Replace it with (drops the `enabled` parameter entirely — channel choice
is no longer this function's job, see Step 3):

```rust
    #[test]
    fn should_alert_respects_transition() {
        assert!(should_alert(SessionState::Running, SessionState::WaitingInput));
        assert!(!should_alert(SessionState::WaitingInput, SessionState::WaitingInput));
        assert!(!should_alert(SessionState::Running, SessionState::Idle));
        assert!(should_alert(SessionState::Running, SessionState::NeedsAttention));
        assert!(should_alert(SessionState::Running, SessionState::Error));
        assert!(!should_alert(SessionState::Running, SessionState::Stale));
        assert!(!should_alert(SessionState::Running, SessionState::Ended));
    }
```

- [ ] **Step 2: Run the test, verify it fails**

Run (from `src-tauri/`): `cargo test should_alert`
Expected: FAIL to compile — `should_alert` doesn't exist yet (the crate
still only has `should_send_notification`).

- [ ] **Step 3: Rename/restructure the notification gate in `tray.rs`**

In `src-tauri/src/tray.rs`, this block currently reads:

```rust
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
```

Replace with:

```rust
/// Guarda pura: a transição pede atenção, independente de canal (nativo
/// ou toast in-app — a escolha do canal é feita em `alert_if_needed`).
fn should_alert(previous_state: SessionState, new_state: SessionState) -> bool {
    previous_state != new_state && should_notify(new_state)
}

/// `pub(crate)`: além de `on_session_changed`, o job de limpeza periódica
/// (`main.rs::spawn_cleanup_job`) também chama isso diretamente — é o
/// único lugar que promove sessão pra `NeedsAttention` (via
/// `apply_waiting_timeout`), transição que nenhum hook do Claude Code gera
/// sozinho. Escolhe o canal: notificação nativa do Windows se
/// `Settings.notifications_enabled` estiver ligado, senão o toast in-app.
pub(crate) fn alert_if_needed(
    app_state: &Arc<AppState>,
    session_id: &str,
    sessions: &[db::SessionRow],
    previous_state: SessionState,
    new_state: SessionState,
    lang: Language,
) {
    if !should_alert(previous_state, new_state) {
        return;
    }

    let title = i18n::notif_title(lang, new_state);
    let body = sessions
        .iter()
        .find(|s| s.session_id == session_id)
        .map(|s| s.cwd.clone())
        .unwrap_or_default();

    let native_enabled = app_state.settings.lock().unwrap().notifications_enabled;
    if native_enabled {
        let _ = app_state
            .app_handle
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show();
    } else {
        crate::toast::show(app_state, title, &body);
    }
}
```

Also update `on_session_changed`'s signature (needed because it now
forwards to `alert_if_needed`, which needs an `Arc<AppState>` to hand to
the toast's spawned auto-hide task) — this block currently reads:

```rust
pub fn on_session_changed(
    app_state: &AppState,
    session_id: &str,
    previous_state: SessionState,
    new_state: SessionState,
) {
```

Change to:

```rust
pub fn on_session_changed(
    app_state: &Arc<AppState>,
    session_id: &str,
    previous_state: SessionState,
    new_state: SessionState,
) {
```

And its call to the notification function, currently:

```rust
    notify_if_needed(app_state, session_id, &sessions, previous_state, new_state, lang);
```

becomes:

```rust
    alert_if_needed(app_state, session_id, &sessions, previous_state, new_state, lang);
```

Finally, add the missing import at the top of `src-tauri/src/tray.rs`
(needed for the `&Arc<AppState>` parameter types above) — the file's
imports currently start with:

```rust
use tauri::tray::TrayIconId;
```

Add right above it:

```rust
use std::sync::Arc;
use tauri::tray::TrayIconId;
```

- [ ] **Step 4: Run the test, verify it passes**

Run: `cargo test should_alert`
Expected: PASS.

- [ ] **Step 5: Create the `toast` module**

Create `src-tauri/src/toast.rs`:

```rust
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
```

- [ ] **Step 6: Wire up `mod toast;` and rename the cleanup-job call site**

In `src-tauri/src/main.rs`, the module declarations currently read:

```rust
mod db;
mod hookmode;
mod i18n;
mod selfconfig;
mod server;
mod settings;
mod state;
mod tray;
mod transcript;
mod usage;
```

Add the new module (alphabetically, after `transcript`):

```rust
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
```

And in `spawn_cleanup_job`, the call currently reads:

```rust
                if after_stale != session.state {
                    let _ = db::set_session_state(&conn, &session.session_id, after_stale);
                    tray::notify_if_needed(
                        &app_state,
                        &session.session_id,
                        &sessions,
                        session.state,
                        after_stale,
                        lang,
                    );
                }
```

Change the call to the renamed function:

```rust
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
```

- [ ] **Step 7: Run the full Rust suite and build**

Run (from `src-tauri/`): `cargo test`
Expected: all tests pass (same count as before this task — one test was
renamed/reduced in scope, none removed or added).

Run: `cargo build`
Expected: compiles cleanly, no warnings.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/toast.rs src-tauri/src/main.rs src-tauri/src/tray.rs
git commit -m "feat: add in-app toast channel for session alerts"
```

---

### Task 3: Frontend `ToastView.vue` + window-label bootstrap

**Files:**
- Create: `src/components/ToastView.vue`
- Modify: `src/main.ts` (mount `ToastView` when window label is `toast`)
- Modify: `src/lib/api.ts` (new `openPanelFromToast` method)

**Interfaces:**
- Consumes (from Task 2): the `"toast-show"` event, payload
  `{ title: string; body: string }`.
- Consumes (from Task 1): the `"open_panel_from_toast"` Tauri command
  (no arguments, no return value).

- [ ] **Step 1: Add the API method**

In `src/lib/api.ts`, the object currently reads:

```ts
export const api = {
  listSessions: () => invoke<Session[]>("list_sessions"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { newSettings: settings }),
  getHookStatus: () => invoke<HookStatus>("get_hook_status"),
  reconfigureHooks: () => invoke<boolean>("reconfigure_hooks"),
  removeHooks: () => invoke<boolean>("remove_hooks_command"),
  getAccountUsage: () => invoke<AccountUsage>("get_account_usage"),
  deleteSession: (sessionId: string) =>
    invoke<boolean>("delete_session", { sessionId }),
};
```

Add the new method:

```ts
export const api = {
  listSessions: () => invoke<Session[]>("list_sessions"),
  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) =>
    invoke<void>("save_settings", { newSettings: settings }),
  getHookStatus: () => invoke<HookStatus>("get_hook_status"),
  reconfigureHooks: () => invoke<boolean>("reconfigure_hooks"),
  removeHooks: () => invoke<boolean>("remove_hooks_command"),
  getAccountUsage: () => invoke<AccountUsage>("get_account_usage"),
  deleteSession: (sessionId: string) =>
    invoke<boolean>("delete_session", { sessionId }),
  openPanelFromToast: () => invoke<void>("open_panel_from_toast"),
};
```

- [ ] **Step 2: Create `ToastView.vue`**

Create `src/components/ToastView.vue`:

```vue
<script setup lang="ts">
import { onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { api } from "../lib/api";

interface ToastPayload {
  title: string;
  body: string;
}

const title = ref("");
const body = ref("");

onMounted(() => {
  listen<ToastPayload>("toast-show", (event) => {
    title.value = event.payload.title;
    body.value = event.payload.body;
  });
});

async function onClick() {
  await api.openPanelFromToast();
  await getCurrentWindow().hide();
}
</script>

<template>
  <div class="toast" @click="onClick">
    <strong class="title">{{ title }}</strong>
    <p class="body">{{ body }}</p>
  </div>
</template>

<style scoped>
.toast {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  height: 100%;
  box-sizing: border-box;
  padding: 0.6rem 0.75rem;
  font-family: system-ui, sans-serif;
  cursor: pointer;
  background: #1e1e1e;
  color: #fff;
}
.title {
  font-size: 0.85rem;
}
.body {
  margin: 0;
  font-size: 0.75rem;
  opacity: 0.75;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
```

- [ ] **Step 3: Branch on window label in `main.ts`**

`src/main.ts` currently reads:

```ts
import { createApp } from "vue";
import { createPinia } from "pinia";
import App from "./App.vue";
import "./style.css";
import { i18n } from "./i18n";
import { api } from "./lib/api";

async function bootstrap() {
  try {
    const settings = await api.getSettings();
    i18n.global.locale.value = settings.language;
  } catch (err) {
    console.error("failed to load settings, using default locale", err);
  }
  createApp(App).use(createPinia()).use(i18n).mount("#app");
}

bootstrap();
```

Replace with:

```ts
import { createApp } from "vue";
import { createPinia } from "pinia";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.vue";
import ToastView from "./components/ToastView.vue";
import "./style.css";
import { i18n } from "./i18n";
import { api } from "./lib/api";

async function bootstrap() {
  if (getCurrentWindow().label === "toast") {
    createApp(ToastView).mount("#app");
    return;
  }

  try {
    const settings = await api.getSettings();
    i18n.global.locale.value = settings.language;
  } catch (err) {
    console.error("failed to load settings, using default locale", err);
  }
  createApp(App).use(createPinia()).use(i18n).mount("#app");
}

bootstrap();
```

- [ ] **Step 4: Type-check**

Run: `npm run build`
Expected: no TypeScript errors.

- [ ] **Step 5: Manual verification**

This can't be verified headlessly — showing/hiding a second native
window, positioning it near the tray, and confirming click-through
behavior all require eyes on the actual app. Note in your final report
that a human should run `npm run tauri dev` and:
1. Open Settings, uncheck "Notificações do Windows"/"Windows
   notifications", save.
2. Force a session into `WaitingInput` or `Error` (e.g. via a real
   Claude Code session, or by POSTing a matching hook payload to the
   local server) — NOT `NeedsAttention` directly, since that state is
   only reachable via the cleanup job's timeout, not immediately.
3. Confirm the toast window appears near the tray with the right
   title/body, instead of a native Windows toast.
4. Confirm it disappears on its own after ~5 seconds if left alone.
5. Force a second alert-worthy transition and confirm the toast still
   behaves correctly (repositions/updates content, doesn't stack, its
   own 5s timer applies to the new content).
6. Click the toast (before it auto-hides) and confirm the main panel
   opens near the tray on the Sessions tab, and the toast hides.
7. Re-check "Notificações do Windows"/"Windows notifications", save,
   force another alert-worthy transition, confirm the native Windows
   toast fires again instead of the in-app one.

- [ ] **Step 6: Commit**

```bash
git add src/lib/api.ts src/components/ToastView.vue src/main.ts
git commit -m "feat: add ToastView and window-label bootstrap for in-app toast"
```
