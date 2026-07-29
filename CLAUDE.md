# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Needle: Windows tray app (Tauri 2 + Vue 3 frontend, Rust backend) that monitors Claude Code sessions. Claude Code fires hooks on session events; Needle's own executable (`needle.exe hook`) receives them, forwards to the running app over local HTTP, and the app tracks per-session state in SQLite, updating the tray icon/panel live.

## Commands

```bash
npm install
npm run tauri dev     # dev mode (Vite + Tauri, hot reload)
npm run build          # vue-tsc --noEmit && vite build (frontend only)
npm run tauri build    # release build, NSIS installer at src-tauri/target/release/bundle/nsis

cd src-tauri
cargo test             # all Rust unit tests
cargo test <name>       # single test by substring, e.g. cargo test transition
```

CI (`.github/workflows/release.yml`) on tag push `v*`: `vue-tsc --noEmit` + `cargo test`, then builds/publishes via `tauri-apps/tauri-action`.

Prerequisites: Rust (MSVC toolchain), Visual Studio Build Tools (C++ workload), Node.js 20+.

## Architecture

### Dual entry point of the same binary

`src-tauri/src/main.rs` branches on `argv[1]` before any Tauri init:
- `needle.exe hook` → `hookmode::run()`: reads a hook JSON payload from stdin, POSTs it to the running app's local HTTP server, exits. No GUI, no Tauri — this is what Claude Code actually invokes on every hook event, so it must stay fast and fail silently (a dead/unreachable app must never block a Claude Code session).
- `needle.exe remove-hooks` → strips Needle's entries from `~/.claude/settings.json` (called by the NSIS uninstaller pre-uninstall hook).
- no args → normal Tauri app: opens the local HTTP server, tray icon, webview window, background cleanup job.

### Event flow

`hookmode` → local HTTP (`server.rs`, axum, POST `/event`) → `state::transition(current, event)` (pure state machine, see `state.rs`) → `db::upsert_session` (SQLite, `db.rs`) → `tray::on_session_changed` (`tray.rs`): re-reads the session's Claude Code transcript for cost/model, updates tray tooltip, emits `session-updated` to the webview, and alerts (`tray::alert_if_needed`) if the new state warrants it — an OS notification when `Settings.notifications_enabled` is on, otherwise an in-app toast (`toast.rs`'s `toast::show`, emits `toast-show` to the dedicated `toast` window, which navigates to the main panel via the `open_panel_from_toast` command when clicked). `session-removed` is emitted only from `main.rs` — on the 24h stale-purge in `spawn_cleanup_job` and on manual deletion via the `delete_session` command — never from `tray.rs`.

Session states (`state.rs`): `Running → WaitingInput → NeedsAttention` (escalates after a timeout with no follow-up event, applied by the periodic cleanup job, not by hook events) · `Idle` · `Error` · `Stale` (no event at all for a timeout, purged entirely after `STALE_PURGE_AFTER_SECS`) · `Ended`. `state::transition` is a pure function of (current state, event) with no time dependency; the timeout-driven transitions (`apply_waiting_timeout`, `apply_stale_timeout`) are separate pure functions invoked only by the cleanup job in `main.rs` (`spawn_cleanup_job`, runs every `CLEANUP_INTERVAL_SECS`). Keep this separation — don't fold wall-clock logic into `transition`.

### Where things live

- **Port discovery**: the server binds the first free port from `DEFAULT_PORT` (47812) upward and writes it to `%TEMP%/needle/port`; `hookmode` reads that file to know where to POST. If the app isn't running, no port file (or a stale one) → hook silently no-ops.
- **Cost/model per session**: not part of the hook payload. `transcript.rs` reads Claude Code's own transcript JSONL (`~/.claude/projects/<sanitized-cwd>/<session_id>.jsonl`) directly and re-derives cost from token usage + a hardcoded `MODEL_PRICING` table — update that table when new models ship.
- **Account-level usage** (5h/7-day %): `usage.rs` calls `https://api.anthropic.com/api/oauth/usage` using the OAuth token Claude Code already stores in `~/.claude/.credentials.json`. Cached in `AppState.usage_cache` for `USAGE_CACHE_TTL_SECS`.
- **Self-configuring hooks**: `selfconfig.rs` rewrites `~/.claude/settings.json`, adding/replacing only hook entries whose command contains "needle" (case-insensitive) — never touches other tools' hooks. Backs up to `settings.json.needle-backup` before any write. Called on every app startup (idempotent, also self-heals the exe path after a move/update) and from the tray "Reconfigure hooks" menu item / Settings UI.
- **Backend i18n vs frontend i18n**: two independent systems. `i18n.rs` hardcodes PT-BR/EN strings for OS-level surfaces (tray tooltip, native menu, notifications) that live outside the webview. The Vue app has its own vue-i18n setup (`src/i18n/`, `src/locales/{en,pt-BR}.json`) for in-webview UI text. When adding user-facing strings, add to the right one — they don't share a source of truth.
- **Frontend ↔ backend bridge**: `src/lib/api.ts` wraps every `#[tauri::command]` from `main.rs`; `src/lib/types.ts` mirrors the Rust structs sent over IPC (kept in sync by hand, not generated). The sessions store (`src/stores/sessions.ts`) is seeded via `list_sessions` then kept live by listening for the `session-updated`/`session-removed` events emitted from `tray.rs`.
- **Window behavior**: single instance (`tauri_plugin_single_instance` refocuses instead of spawning a second process/tray icon), popover-style — hides on blur (`WindowEvent::Focused(false)`), repositions itself next to the tray icon on click using the last known tray click/hover position (tray menu items don't get click coordinates, hence the tracked `last_tray_pos`, held as `AppState.last_tray_pos: Mutex<Option<PhysicalPosition<f64>>>` rather than a `setup()`-local variable — `None` until the first tray click/hover, with callers falling back to the primary monitor's bottom-right corner). A second window, `toast` (label `"toast"`, its own capability file `capabilities/toast.json`), is a small non-interactive-focus popup near the tray that shows the in-app notification alternative and auto-hides itself a few seconds later (`toast.rs`).

### Code comment convention

Existing Rust comments are in Portuguese and explain non-obvious *why* (invariants, race conditions avoided, format quirks) rather than *what* — match that style and language when touching `src-tauri/src`.
