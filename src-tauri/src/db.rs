use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::Path;

use crate::state::SessionState;

pub fn open(path: &Path) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            session_id TEXT PRIMARY KEY,
            cwd TEXT NOT NULL,
            started_at INTEGER NOT NULL,
            last_event_at INTEGER NOT NULL,
            state TEXT NOT NULL,
            last_message_snippet TEXT
        );
        CREATE TABLE IF NOT EXISTS events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            received_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_events_session ON events(session_id);
        ",
    )?;
    Ok(conn)
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
    pub session_id: String,
    pub cwd: String,
    pub started_at: i64,
    pub last_event_at: i64,
    pub state: SessionState,
    pub last_message_snippet: Option<String>,
}

fn state_to_str(state: SessionState) -> &'static str {
    match state {
        SessionState::Running => "Running",
        SessionState::WaitingInput => "WaitingInput",
        SessionState::NeedsAttention => "NeedsAttention",
        SessionState::Idle => "Idle",
        SessionState::Error => "Error",
        SessionState::Stale => "Stale",
        SessionState::Ended => "Ended",
    }
}

fn state_from_str(s: &str) -> SessionState {
    match s {
        "Running" => SessionState::Running,
        "WaitingInput" => SessionState::WaitingInput,
        "NeedsAttention" => SessionState::NeedsAttention,
        "Idle" => SessionState::Idle,
        "Error" => SessionState::Error,
        "Stale" => SessionState::Stale,
        _ => SessionState::Ended,
    }
}

pub fn get_session_state(
    conn: &Connection,
    session_id: &str,
) -> rusqlite::Result<Option<SessionState>> {
    conn.query_row(
        "SELECT state FROM sessions WHERE session_id = ?1",
        params![session_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map(|opt| opt.map(|s| state_from_str(&s)))
}

pub fn upsert_session(
    conn: &Connection,
    session_id: &str,
    cwd: &str,
    now: i64,
    new_state: SessionState,
    snippet: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "
        INSERT INTO sessions (session_id, cwd, started_at, last_event_at, state, last_message_snippet)
        VALUES (?1, ?2, ?3, ?3, ?4, ?5)
        ON CONFLICT(session_id) DO UPDATE SET
            last_event_at = ?3,
            state = ?4,
            last_message_snippet = COALESCE(?5, last_message_snippet)
        ",
        params![session_id, cwd, now, state_to_str(new_state), snippet],
    )?;
    Ok(())
}

pub fn insert_event(
    conn: &Connection,
    session_id: &str,
    event_type: &str,
    payload_json: &str,
    now: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO events (session_id, event_type, payload_json, received_at) VALUES (?1, ?2, ?3, ?4)",
        params![session_id, event_type, payload_json, now],
    )?;
    Ok(())
}

pub fn list_sessions(conn: &Connection) -> rusqlite::Result<Vec<SessionRow>> {
    let mut stmt = conn.prepare(
        "SELECT session_id, cwd, started_at, last_event_at, state, last_message_snippet
         FROM sessions WHERE state != 'Ended' ORDER BY last_event_at DESC",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(SessionRow {
                session_id: row.get(0)?,
                cwd: row.get(1)?,
                started_at: row.get(2)?,
                last_event_at: row.get(3)?,
                state: state_from_str(&row.get::<_, String>(4)?),
                last_message_snippet: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn set_session_state(
    conn: &Connection,
    session_id: &str,
    new_state: SessionState,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE sessions SET state = ?2 WHERE session_id = ?1",
        params![session_id, state_to_str(new_state)],
    )?;
    Ok(())
}

pub fn delete_ended_sessions(conn: &Connection) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM sessions WHERE state = 'Ended'", [])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        open(Path::new(":memory:")).unwrap()
    }

    #[test]
    fn upsert_then_get_roundtrips_state() {
        let conn = setup();
        upsert_session(&conn, "s1", "/tmp/proj", 100, SessionState::Running, None).unwrap();
        assert_eq!(
            get_session_state(&conn, "s1").unwrap(),
            Some(SessionState::Running)
        );
    }

    #[test]
    fn upsert_is_idempotent_and_updates_state() {
        let conn = setup();
        upsert_session(
            &conn,
            "s1",
            "/tmp/proj",
            100,
            SessionState::Running,
            Some("first"),
        )
        .unwrap();
        upsert_session(
            &conn,
            "s1",
            "/tmp/proj",
            200,
            SessionState::Idle,
            Some("second"),
        )
        .unwrap();

        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].state, SessionState::Idle);
        assert_eq!(sessions[0].last_event_at, 200);
        assert_eq!(sessions[0].last_message_snippet.as_deref(), Some("second"));
    }

    #[test]
    fn list_sessions_excludes_ended() {
        let conn = setup();
        upsert_session(&conn, "s1", "/tmp/a", 100, SessionState::Running, None).unwrap();
        upsert_session(&conn, "s2", "/tmp/b", 100, SessionState::Ended, None).unwrap();
        let sessions = list_sessions(&conn).unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "s1");
    }

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
}
