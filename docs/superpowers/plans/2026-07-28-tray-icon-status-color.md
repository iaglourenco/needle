# Tray Icon Status Color Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Windows tray icon itself (not just the tooltip) reflect
the worst aggregate session state — a colored dot when something needs
attention, back to the default Needle logo otherwise.

**Architecture:** Single-file backend change in `src-tauri/src/tray.rs`.
Draw a small filled-circle RGBA bitmap at runtime for the relevant color
(no new asset files, no new dependency — `tauri::image::Image::new_owned`
already ships in the `tauri` crate's `tray-icon` feature), and call
`TrayIcon::set_icon` from the two places that already recompute the worst
state and update the tooltip.

**Tech Stack:** Rust (`tauri` 2 crate, `tray-icon` feature already
enabled), `cargo test`.

## Global Constraints

- No new crate dependency, no new asset files — the icon is generated as
  a raw RGBA buffer in code.
- Colored dot appears only when the worst aggregate state has severity >
  0 (`Running`, `WaitingInput`, `NeedsAttention`, `Error`, `Idle`).
  `Stale`/`Ended` (severity 0) and "no sessions at all" both fall back to
  the default Needle logo.
- Colors match the palette already used in `src/components/SessionItem.vue`
  (`stateColor`): Running `#3b82f6`, WaitingInput `#eab308`,
  NeedsAttention/Error `#ef4444`, Idle `#22c55e`.
- Verify with `cargo test` (run from `src-tauri/`) and `cargo build` — no
  frontend files change in this plan.

---

### Task 1: Draw and wire up the status-colored tray icon

**Files:**
- Modify: `src-tauri/src/tray.rs` (adds 3 new functions + a `#[cfg(test)]`
  module; wires into the two existing call sites that already recompute
  `worst` and set the tooltip)

**Interfaces:**
- Consumes: `SessionState` (`src-tauri/src/state.rs`, unchanged),
  `AppState` (`src-tauri/src/main.rs`, unchanged — uses the existing
  `app_handle` field via the `Manager` trait's `default_window_icon()`
  and `tray_by_id()`, both already used elsewhere in this file).
- Produces: no new public exports — all three new functions
  (`icon_color_for`, `dot_icon`, `icon_for_worst`) are private to
  `tray.rs`, used only by `on_session_changed` and `refresh_from_db` in
  the same file.

- [ ] **Step 1: Write the failing tests**

Add this `#[cfg(test)]` module at the end of `src-tauri/src/tray.rs`
(after the existing `notify_if_needed` function, which is currently the
last item in the file):

```rust
#[cfg(test)]
mod tests {
    use super::*;

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
}
```

- [ ] **Step 2: Run the tests, verify they fail**

Run (from `src-tauri/`): `cargo test tray::`
Expected: FAIL to compile — `icon_color_for` and `dot_icon` don't exist
yet (`cannot find function` errors).

- [ ] **Step 3: Implement `icon_color_for` and `dot_icon`**

Add these two functions to `src-tauri/src/tray.rs`, above the `#[cfg(test)]`
module added in Step 1 (right after `notify_if_needed`):

```rust
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
```

- [ ] **Step 4: Run the tests, verify they pass**

Run: `cargo test tray::`
Expected: PASS (4 tests: `icon_color_for_maps_each_severity_bearing_state`,
`icon_color_for_terminal_states_is_none`, `dot_icon_is_32x32_rgba`,
`dot_icon_paints_center_and_leaves_corners_transparent`).

- [ ] **Step 5: Add `icon_for_worst` and wire it into both call sites**

Add this function right after `dot_icon` (still above the `#[cfg(test)]`
module):

```rust
/// Ícone a exibir na bandeja pro pior estado agregado: círculo colorido
/// se houver sessão com severidade > 0, senão volta pro logo padrão do
/// Needle (sem sessão, ou só sessões `Stale`/`Ended`).
fn icon_for_worst(
    app_state: &AppState,
    worst: Option<SessionState>,
) -> Option<tauri::image::Image<'static>> {
    match worst.and_then(icon_color_for) {
        Some(color) => Some(dot_icon(color)),
        None => app_state.app_handle.default_window_icon().cloned(),
    }
}
```

In `on_session_changed`, this block currently reads:

```rust
    if let Some(tray) = app_state
        .app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
    {
        let _ = tray.set_tooltip(Some(i18n::tray_tooltip(lang, worst)));
    }
```

Change it to:

```rust
    if let Some(tray) = app_state
        .app_handle
        .tray_by_id(&TrayIconId::new(TRAY_ID))
    {
        let _ = tray.set_tooltip(Some(i18n::tray_tooltip(lang, worst)));
        let _ = tray.set_icon(icon_for_worst(app_state, worst));
    }
```

In `refresh_from_db`, this block currently reads:

```rust
    if let Some(tray) = app_state.app_handle.tray_by_id(&TrayIconId::new(TRAY_ID)) {
        let _ = tray.set_tooltip(Some(i18n::tray_tooltip(lang, worst)));
    }
```

Change it to:

```rust
    if let Some(tray) = app_state.app_handle.tray_by_id(&TrayIconId::new(TRAY_ID)) {
        let _ = tray.set_tooltip(Some(i18n::tray_tooltip(lang, worst)));
        let _ = tray.set_icon(icon_for_worst(app_state, worst));
    }
```

- [ ] **Step 6: Run the full Rust suite and build**

Run (from `src-tauri/`): `cargo test`
Expected: all tests pass (previous count + the 4 new ones from Step 1).

Run: `cargo build`
Expected: compiles cleanly, no warnings.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/tray.rs
git commit -m "feat: color the tray icon by worst aggregate session state"
```

- [ ] **Step 8: Note manual verification for the human**

This change can't be verified headlessly — a real Windows tray icon
requires eyes on the actual system tray. In your final report, note that
a human should run `npm run tauri dev`, drive a session (or a few) through
different states (e.g. trigger a `Notification` hook event for
`WaitingInput`/yellow, let one sit idle for `Idle`/green, force a tool
error for `Error`/`NeedsAttention`/red), and confirm the tray icon itself
changes color and reverts to the default Needle logo once no session has
severity > 0.
