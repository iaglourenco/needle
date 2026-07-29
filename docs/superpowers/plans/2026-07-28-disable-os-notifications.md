# Disable OS Notifications Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user turn off the native Windows notifications Needle
fires when a session needs attention, via a new checkbox in Settings.

**Architecture:** Backend first — a new `notifications_enabled` field on
`Settings` (default `true`, so existing installs keep today's behavior),
and a pure, unit-testable guard function in `tray.rs` gating the one place
that fires OS notifications for session-state transitions. Then a small
frontend follow-up — checkbox + locale strings.

**Tech Stack:** Rust (`serde`, `cargo test`), Vue 3 + TypeScript
(`vue-tsc`, no frontend test runner).

## Global Constraints

- Default `notifications_enabled = true` — existing `settings.json` files
  without this key must keep notifying, exactly like today.
- This toggle governs ONLY session-state notifications (`tray.rs`'s
  `notify_if_needed`, covering `NeedsAttention`/`Error`/`WaitingInput`
  transitions). The "hooks configured"/"hooks reconfigured" notifications
  in `main.rs` are a different category (app lifecycle, not session
  state) and are explicitly out of scope — they always fire regardless
  of this setting.
- No new Tauri command — routed through the existing
  `get_settings`/`save_settings` commands, same as every other `Settings`
  field.
- Verify Rust changes with `cargo test` (from `src-tauri/`) and
  `cargo build`. Verify frontend changes with `npm run build`
  (`vue-tsc --noEmit`) — no automated frontend test harness exists in
  this repo.

---

### Task 1: `notifications_enabled` setting + guard in `tray.rs` (backend)

**Files:**
- Modify: `src-tauri/src/settings.rs` (`Settings` struct, `Default` impl,
  tests)
- Modify: `src-tauri/src/tray.rs` (`notify_if_needed`, new
  `should_send_notification` function, tests)

**Interfaces:**
- Produces: `Settings.notifications_enabled: bool` (Rust, `serde` field
  name `notificationsEnabled` via the struct's existing
  `#[serde(rename_all = "camelCase")]`) — Task 2's frontend
  `Settings.notificationsEnabled: boolean` field maps onto this exact
  wire name.
- Produces: `should_send_notification(enabled: bool, previous_state:
  SessionState, new_state: SessionState) -> bool` (private to `tray.rs`,
  not consumed elsewhere, but pinned by its own unit tests).

- [ ] **Step 1: Write the failing settings tests**

Add these two tests to the existing `#[cfg(test)] mod tests` block in
`src-tauri/src/settings.rs` (alongside the existing tests):

```rust
    #[test]
    fn notifications_enabled_defaults_to_true_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"waitingTimeoutSecs":60,"staleTimeoutSecs":1800,"autostart":false}"#,
        )
        .unwrap();
        let settings = load(dir.path());
        assert!(settings.notifications_enabled);
    }

    #[test]
    fn notifications_enabled_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let settings = Settings {
            notifications_enabled: false,
            ..Settings::default()
        };
        save(dir.path(), &settings).unwrap();
        let loaded = load(dir.path());
        assert!(!loaded.notifications_enabled);
    }
```

- [ ] **Step 2: Run the tests, verify they fail**

Run (from `src-tauri/`): `cargo test notifications_enabled`
Expected: FAIL to compile — `Settings` has no field
`notifications_enabled` yet (`no field notifications_enabled on type
Settings` / similar).

- [ ] **Step 3: Add the field to `Settings`**

In `src-tauri/src/settings.rs`, the struct and its `Default` impl
currently read:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub waiting_timeout_secs: i64,
    pub stale_timeout_secs: i64,
    pub autostart: bool,
    #[serde(default)]
    pub language: Language,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            waiting_timeout_secs: 60,
            stale_timeout_secs: 30 * 60,
            autostart: false,
            language: Language::PtBr,
        }
    }
}
```

Replace with (adds a `default_notifications_enabled` helper — plain
`#[serde(default)]` would use `bool::default() == false`, which would
invert today's always-on behavior for old `settings.json` files, so an
explicit default-`true` function is required):

```rust
fn default_notifications_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub waiting_timeout_secs: i64,
    pub stale_timeout_secs: i64,
    pub autostart: bool,
    #[serde(default)]
    pub language: Language,
    #[serde(default = "default_notifications_enabled")]
    pub notifications_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            waiting_timeout_secs: 60,
            stale_timeout_secs: 30 * 60,
            autostart: false,
            language: Language::PtBr,
            notifications_enabled: true,
        }
    }
}
```

- [ ] **Step 4: Fix the pre-existing test that constructs `Settings` by full literal**

Adding a field to `Settings` breaks any test that builds one field-by-field
without `..Default::default()`. In the same file's test module, the
`save_then_load_roundtrips` test currently has:

```rust
        let settings = Settings {
            waiting_timeout_secs: 90,
            stale_timeout_secs: 600,
            autostart: true,
            language: Language::En,
        };
```

Add the new field so it compiles again:

```rust
        let settings = Settings {
            waiting_timeout_secs: 90,
            stale_timeout_secs: 600,
            autostart: true,
            language: Language::En,
            notifications_enabled: true,
        };
```

(Check whether any other test in this file constructs `Settings` by full
literal without `..Default::default()`/`..Settings::default()` — the
`language_serializes_with_the_exact_wire_tags` test already uses the
spread form and needs no change.)

- [ ] **Step 5: Run the tests, verify they pass**

Run: `cargo test` (from `src-tauri/`, whole crate — this also confirms
Step 4's fix compiles)
Expected: PASS, including the two new tests from Step 1.

- [ ] **Step 6: Write the failing `tray.rs` guard test**

Add this test to `tray.rs`'s existing `#[cfg(test)] mod tests` block
(added in a previous feature — it already has `use super::*;`):

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
    }
```

- [ ] **Step 7: Run the test, verify it fails**

Run: `cargo test should_send_notification`
Expected: FAIL to compile — `should_send_notification` doesn't exist yet.

- [ ] **Step 8: Extract `should_send_notification` and wire the guard into `notify_if_needed`**

In `src-tauri/src/tray.rs`, `notify_if_needed` currently reads:

```rust
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
```

Replace with:

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

fn notify_if_needed(
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

- [ ] **Step 9: Run the test, verify it passes**

Run: `cargo test should_send_notification`
Expected: PASS.

- [ ] **Step 10: Run the full Rust suite and build**

Run (from `src-tauri/`): `cargo test`
Expected: all tests pass (previous count + 3 new: 2 in `settings.rs`, 1
in `tray.rs`).

Run: `cargo build`
Expected: compiles cleanly, no warnings.

- [ ] **Step 11: Commit**

```bash
git add src-tauri/src/settings.rs src-tauri/src/tray.rs
git commit -m "feat: add setting to disable Windows session notifications"
```

---

### Task 2: Settings checkbox (frontend)

**Files:**
- Modify: `src/lib/types.ts` (`Settings` interface)
- Modify: `src/components/SettingsView.vue` (new checkbox)
- Modify: `src/locales/pt-BR.json`, `src/locales/en.json`
  (`settings.notificationsEnabled` key)

**Interfaces:**
- Consumes: `Settings.notificationsEnabled: boolean` maps onto Task 1's
  Rust `notifications_enabled` field via the existing camelCase JSON
  wire format (same mechanism already used for every other `Settings`
  field — no new serialization code needed on either side).

- [ ] **Step 1: Add the field to the TypeScript `Settings` type**

In `src/lib/types.ts`, the interface currently reads:

```ts
export interface Settings {
  waitingTimeoutSecs: number;
  staleTimeoutSecs: number;
  autostart: boolean;
  language: "pt-BR" | "en";
}
```

Add the new field:

```ts
export interface Settings {
  waitingTimeoutSecs: number;
  staleTimeoutSecs: number;
  autostart: boolean;
  notificationsEnabled: boolean;
  language: "pt-BR" | "en";
}
```

- [ ] **Step 2: Add the locale key (pt-BR)**

In `src/locales/pt-BR.json`, the `settings` block currently has:

```json
    "generalTitle": "Geral",
    "autostart": "Iniciar com o sistema",
    "language": "Idioma da aplicação",
```

Add a new key right after `autostart`:

```json
    "generalTitle": "Geral",
    "autostart": "Iniciar com o sistema",
    "notificationsEnabled": "Notificações do Windows",
    "language": "Idioma da aplicação",
```

- [ ] **Step 3: Add the locale key (en)**

In `src/locales/en.json`, the `settings` block currently has:

```json
    "generalTitle": "General",
    "autostart": "Start with the system",
    "language": "Application language",
```

Add a new key right after `autostart`:

```json
    "generalTitle": "General",
    "autostart": "Start with the system",
    "notificationsEnabled": "Windows notifications",
    "language": "Application language",
```

- [ ] **Step 4: Add the checkbox to `SettingsView.vue`**

In `src/components/SettingsView.vue`, the "Geral"/General section
currently has:

```vue
    <section>
      <h2>{{ t("settings.generalTitle") }}</h2>
      <label class="checkbox">
        <input v-model="settings.autostart" type="checkbox" />
        {{ t("settings.autostart") }}
      </label>
      <label>
        {{ t("settings.language") }}
        <select v-model="settings.language" @change="locale = settings.language">
          <option value="pt-BR">Português (Brasil)</option>
          <option value="en">English</option>
        </select>
      </label>
    </section>
```

Add a second checkbox right after the autostart one:

```vue
    <section>
      <h2>{{ t("settings.generalTitle") }}</h2>
      <label class="checkbox">
        <input v-model="settings.autostart" type="checkbox" />
        {{ t("settings.autostart") }}
      </label>
      <label class="checkbox">
        <input v-model="settings.notificationsEnabled" type="checkbox" />
        {{ t("settings.notificationsEnabled") }}
      </label>
      <label>
        {{ t("settings.language") }}
        <select v-model="settings.language" @change="locale = settings.language">
          <option value="pt-BR">Português (Brasil)</option>
          <option value="en">English</option>
        </select>
      </label>
    </section>
```

- [ ] **Step 5: Type-check**

Run: `npm run build`
Expected: no TypeScript errors.

- [ ] **Step 6: Manual verification**

This can't be verified headlessly — confirming a Windows toast does or
doesn't appear requires eyes on the actual OS notification. Note in your
final report that a human should run `npm run tauri dev`, open Settings,
uncheck "Notificações do Windows"/"Windows notifications", click Save,
then force a session into `WaitingInput`/`NeedsAttention`/`Error` (e.g.
via a real Claude Code session or by POSTing a matching hook payload to
the local server) and confirm no Windows toast appears — while the tray
icon color/tooltip keep updating normally. Then re-check the box, save,
and confirm notifications resume.

- [ ] **Step 7: Commit**

```bash
git add src/lib/types.ts src/components/SettingsView.vue src/locales/pt-BR.json src/locales/en.json
git commit -m "feat: add checkbox to toggle Windows session notifications"
```
