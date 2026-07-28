# Persist Ended Sessions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop treating `Ended` sessions as "already gone" — keep them
visible in the panel (and counted in the total-cost banner) until the
user manually deletes them, instead of the current near-instant removal.

**Architecture:** Backend change first (stop excluding/purging `Ended`
sessions, allow manual deletion of them), then a small frontend change
(show the existing delete button for `Ended` too, generalize its label).
No schema change.

**Tech Stack:** Rust (rusqlite, `cargo test`), Vue 3 + TypeScript
(`vue-tsc`, no frontend test runner — manual verification via
`npm run tauri dev`).

## Global Constraints

- No schema migration — same `sessions` table, same columns.
- `Ended` sessions are never auto-purged by any background job going
  forward. The 24h stale-purge (`STALE_PURGE_AFTER_SECS` in `main.rs`)
  continues to apply only to `Stale` sessions — unchanged.
- Manual deletion (the existing delete button + `delete_session` Tauri
  command) must accept both `Stale` and `Ended` sessions, and continue to
  reject every other state (an active session must never disappear by
  accident).
- New/changed user-facing strings go in both `src/locales/pt-BR.json` and
  `src/locales/en.json`.
- Rust changes are verified with `cargo test` (run from `src-tauri/`).
  Frontend changes are verified with `npm run build` (runs
  `vue-tsc --noEmit`) plus manual trace/run — no automated frontend test
  harness exists in this repo.

---

### Task 1: Stop excluding and purging `Ended` sessions (backend)

**Files:**
- Modify: `src-tauri/src/db.rs:124-144` (`list_sessions`), `:171-173`
  (`delete_ended_sessions` — removed), `:229-264` (tests)
- Modify: `src-tauri/src/main.rs:96-137` (`spawn_cleanup_job`),
  `:453-467` (`delete_session` command)
- Modify: `src-tauri/src/tray.rs:24-60` (`on_session_changed`)

**Interfaces:**
- Consumes: `SessionState::{Stale, Ended}` (`src-tauri/src/state.rs`,
  unchanged).
- Produces: `db::list_sessions` now returns rows of every state,
  including `Ended` — Task 2's frontend work only reads the existing
  `Session`/`SessionRow` shape (unchanged fields), so no interface change
  propagates there.

- [ ] **Step 1: Write the failing test for `list_sessions` including `Ended`**

In `src-tauri/src/db.rs`, replace the existing test (currently named
`list_sessions_excludes_ended`, around line 229):

```rust
    #[test]
    fn list_sessions_excludes_ended() {
        let conn = setup();
        upsert_session(&conn, "s1", "/tmp/a", 100, SessionState::Running, None).unwrap();
        upsert_session(&conn, "s2", "/tmp/b", 100, SessionState::Ended, None).unwrap();
        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");
    }
```

with:

```rust
    #[test]
    fn list_sessions_includes_ended() {
        let conn = setup();
        upsert_session(&conn, "s1", "/tmp/a", 100, SessionState::Running, None).unwrap();
        upsert_session(&conn, "s2", "/tmp/b", 200, SessionState::Ended, None).unwrap();
        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, "s2");
        assert_eq!(sessions[0].state, SessionState::Ended);
    }
```

(`s2` has a later `last_event_at` so it's also verifying the existing
`ORDER BY last_event_at DESC` still holds with an `Ended` row present.)

- [ ] **Step 2: Run the test, verify it fails**

Run (from `src-tauri/`): `cargo test list_sessions_includes_ended`
Expected: FAIL — `list_sessions` still filters `WHERE state != 'Ended'`,
so `sessions.len()` is 1, not 2.

- [ ] **Step 3: Remove the `Ended` filter from `list_sessions`**

In `src-tauri/src/db.rs`, `list_sessions` currently reads:

```rust
pub fn list_sessions(conn: &Connection) -> rusqlite::Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, cwd, started_at, last_event_at, state, last_message_snippet, model, cost_usd
         FROM sessions WHERE state != 'Ended' ORDER BY last_event_at DESC",
    )?;
```

Change the query string to drop the `WHERE` clause:

```rust
pub fn list_sessions(conn: &Connection) -> rusqlite::Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, cwd, started_at, last_event_at, state, last_message_snippet, model, cost_usd
         FROM sessions ORDER BY last_event_at DESC",
    )?;
```

- [ ] **Step 4: Run the test, verify it passes**

Run: `cargo test list_sessions_includes_ended`
Expected: PASS.

- [ ] **Step 5: Remove `delete_ended_sessions` (function + its test)**

In `src-tauri/src/db.rs`, delete this function entirely (around line
171):

```rust
pub fn delete_ended_sessions(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM sessions WHERE state = 'Ended'", [])
}
```

And delete its test (around line 253):

```rust
    #[test]
    fn delete_ended_sessions_removes_only_ended() {
        let conn = setup();
        upsert_session(&conn, "s1", "/tmp/a", 100, SessionState::Running, None).unwrap();
        upsert_session(&conn, "s2", "/tmp/b", 100, SessionState::Ended, None).unwrap();
        let deleted = delete_ended_sessions(&conn).unwrap();
        assert_eq!(deleted, 1);

        let mut stmt = conn.prepare("SELECT COUNT(*) FROM sessions").unwrap();
        let count: i64 = stmt.query_row([], |r| r.get(0)).unwrap();
        assert_eq!(count, 1);
    }
```

- [ ] **Step 6: Remove the auto-purge call in the cleanup job**

In `src-tauri/src/main.rs`, `spawn_cleanup_job` currently has, right
before the loop iteration ends (around line 131):

```rust
            db::delete_ended_sessions(&conn).ok();
            drop(conn);

            tray::refresh_from_db(&app_state);
```

Remove the `delete_ended_sessions` line, keeping the rest:

```rust
            drop(conn);

            tray::refresh_from_db(&app_state);
```

- [ ] **Step 7: Let `delete_session` accept `Ended` as well as `Stale`**

In `src-tauri/src/main.rs`, the command and its doc comment currently
read (around line 453):

```rust
/// Apaga uma sessão manualmente. Só permitido pra sessões já `Stale` — o
/// botão de delete na UI só existe pra elas, e essa checagem no backend
/// evita que uma sessão ativa suma por engano.
#[tauri::command]
fn delete_session(state: tauri::State<Arc<AppState>>, session_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().unwrap();
    let current = db::get_session_state(&conn, &session_id).map_err(|e| e.to_string())?;
    if current != Some(state::SessionState::Stale) {
        return Ok(false);
    }
    db::delete_session(&conn, &session_id).map_err(|e| e.to_string())?;
    drop(conn);
    let _ = state.app_handle.emit("session-removed", &session_id);
    Ok(true)
}
```

Replace with:

```rust
/// Apaga uma sessão manualmente. Só permitido pra sessões em estado
/// terminal (`Stale` ou `Ended`) — o botão de delete na UI só existe pra
/// elas, e essa checagem no backend evita que uma sessão ativa suma por
/// engano.
#[tauri::command]
fn delete_session(state: tauri::State<Arc<AppState>>, session_id: String) -> Result<bool, String> {
    let conn = state.conn.lock().unwrap();
    let current = db::get_session_state(&conn, &session_id).map_err(|e| e.to_string())?;
    if !matches!(
        current,
        Some(state::SessionState::Stale) | Some(state::SessionState::Ended)
    ) {
        return Ok(false);
    }
    db::delete_session(&conn, &session_id).map_err(|e| e.to_string())?;
    drop(conn);
    let _ = state.app_handle.emit("session-removed", &session_id);
    Ok(true)
}
```

- [ ] **Step 8: Stop emitting `session-removed` when a session ends**

In `src-tauri/src/tray.rs`, `on_session_changed` currently has (around
line 48):

```rust
    if let Some(session) = sessions.iter().find(|s| s.session_id == session_id) {
        let _ = app_state.app_handle.emit("session-updated", session);
    }
    if new_state == SessionState::Ended {
        let _ = app_state.app_handle.emit("session-removed", session_id);
    }

    if let Some(tray) = app_state
```

Remove the `if new_state == SessionState::Ended { ... }` block (and the
blank line right after it), leaving:

```rust
    if let Some(session) = sessions.iter().find(|s| s.session_id == session_id) {
        let _ = app_state.app_handle.emit("session-updated", session);
    }

    if let Some(tray) = app_state
```

- [ ] **Step 9: Run the full Rust suite and build**

Run (from `src-tauri/`): `cargo test`
Expected: all tests pass (same count as before, minus the one removed in
Step 5, plus the one added in Step 1 — net unchanged count).

Run: `cargo build`
Expected: compiles with no warnings about unused `delete_ended_sessions`
or unused imports in `tray.rs`/`main.rs`.

- [ ] **Step 10: Commit**

```bash
git add src-tauri/src/db.rs src-tauri/src/main.rs src-tauri/src/tray.rs
git commit -m "feat: stop purging ended sessions, keep them until manually deleted"
```

---

### Task 2: Show the delete button for `Ended` sessions (frontend)

**Files:**
- Modify: `src/components/SessionItem.vue:74-93` (delete button `v-if`)
- Modify: `src/locales/pt-BR.json:17` (`sessions.delete`)
- Modify: `src/locales/en.json:17` (`sessions.delete`)

**Interfaces:**
- Consumes: `Session.state` (`src/lib/types.ts`, unchanged) and the
  `delete_session` Tauri command (Task 1, now accepts `Ended` too) via
  `api.deleteSession` (`src/lib/api.ts`, unchanged signature).

- [ ] **Step 1: Extend the delete button's visibility condition**

In `src/components/SessionItem.vue`, the button currently is:

```vue
        <button
          v-if="session.state === 'Stale'"
          type="button"
          class="delete-btn"
```

Change the condition to:

```vue
        <button
          v-if="session.state === 'Stale' || session.state === 'Ended'"
          type="button"
          class="delete-btn"
```

- [ ] **Step 2: Generalize the delete button's label (pt-BR)**

In `src/locales/pt-BR.json`, the `sessions` block has:

```json
    "delete": "Apagar sessão obsoleta"
```

Change to:

```json
    "delete": "Apagar sessão"
```

- [ ] **Step 3: Generalize the delete button's label (en)**

In `src/locales/en.json`, the `sessions` block has:

```json
    "delete": "Delete stale session"
```

Change to:

```json
    "delete": "Delete session"
```

- [ ] **Step 4: Type-check**

Run: `npm run build`
Expected: no TypeScript errors.

- [ ] **Step 5: Manual verification**

Run: `npm run tauri dev` (with the Task 1 backend changes also built —
`npm run tauri dev` rebuilds the Rust side too).

Check, in order:
1. Let a real Claude Code session in some project run to completion (or
   trigger a `SessionEnd` hook manually) — confirm the session stays
   visible in the panel afterward with state "Encerrada"/"Ended" and
   gray dot, instead of disappearing.
2. Hover over that ended session — confirm the delete button (trash
   icon) now appears, same as it does for `Stale` sessions.
3. Confirm the `UsageBanner` total cost tile includes that session's
   cost (compare the sum against the per-session cost labels visible in
   the list).
4. Click delete on the ended session — confirm it's removed from the
   list and the total cost tile updates accordingly.
5. Confirm an active (`Running`/`WaitingInput`/etc.) session still has no
   delete button.

- [ ] **Step 6: Commit**

```bash
git add src/components/SessionItem.vue src/locales/pt-BR.json src/locales/en.json
git commit -m "feat: allow deleting ended sessions, generalize delete label"
```
