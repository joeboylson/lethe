//! The SQLite file: which sessions ran in which Claude window, and the notes
//! they wrote. The notes themselves are Markdown files under `~/.lethe/notes`.

use crate::paths;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    claude_pid  INTEGER,
    cwd         TEXT,
    project     TEXT,
    source      TEXT,
    started_at  TEXT NOT NULL,
    ended_at    TEXT,
    end_reason  TEXT,
    replaced_by TEXT
);
CREATE INDEX IF NOT EXISTS sessions_pid ON sessions (claude_pid, started_at);

CREATE TABLE IF NOT EXISTS notes (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    project      TEXT NOT NULL,
    project_path TEXT NOT NULL,
    path         TEXT NOT NULL,
    title        TEXT NOT NULL,
    session_id   TEXT,
    claude_pid   INTEGER,
    created_at   TEXT NOT NULL,
    -- waiting: loads into the next session after /clear in the same window
    -- picked_up, cancelled (window closed, or a newer note replaced it), kept (no window)
    status       TEXT NOT NULL,
    picked_up_by TEXT,
    picked_up_at TEXT,
    archived     INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS notes_project ON notes (project_path, created_at);
";

pub fn open() -> Result<Connection> {
    std::fs::create_dir_all(paths::home())?;
    let conn = Connection::open(paths::db_path())?;
    // Hooks from several windows can write at once.
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

pub fn now() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

#[derive(Serialize, Clone, Debug)]
pub struct Note {
    pub id: i64,
    pub project: String,
    pub project_path: String,
    pub path: String,
    pub title: String,
    pub session_id: Option<String>,
    pub created_at: String,
    pub status: String,
    pub picked_up_by: Option<String>,
    pub picked_up_at: Option<String>,
    pub archived: bool,
}

pub const NOTE_COLUMNS: &str =
    "id, project, project_path, path, title, session_id, created_at, status, picked_up_by, picked_up_at, archived";

pub fn note_from(row: &Row) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get(0)?,
        project: row.get(1)?,
        project_path: row.get(2)?,
        path: row.get(3)?,
        title: row.get(4)?,
        session_id: row.get(5)?,
        created_at: row.get(6)?,
        status: row.get(7)?,
        picked_up_by: row.get(8)?,
        picked_up_at: row.get(9)?,
        archived: row.get::<_, i64>(10)? != 0,
    })
}

pub fn note(conn: &Connection, id: i64) -> Result<Option<Note>> {
    Ok(conn
        .query_row(&format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1"), [id], note_from)
        .optional()?)
}

/// Newest first. `project_path` None means every project.
pub fn notes(conn: &Connection, project_path: Option<&str>, archived: bool, limit: i64) -> Result<Vec<Note>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {NOTE_COLUMNS} FROM notes
         WHERE (?1 IS NULL OR project_path = ?1) AND archived = ?2
         ORDER BY created_at DESC, id DESC LIMIT ?3"
    ))?;
    let rows = stmt.query_map(params![project_path, archived as i64, limit], note_from)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The session now running in a Claude window: the last one that started there.
pub fn current_session(conn: &Connection, claude_pid: i64) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT id FROM sessions WHERE claude_pid = ?1 AND ended_at IS NULL ORDER BY started_at DESC, rowid DESC LIMIT 1",
            [claude_pid],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn mark_picked_up(conn: &Connection, note_id: i64, session_id: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE notes SET status = 'picked_up', picked_up_by = ?2, picked_up_at = ?3 WHERE id = ?1",
        params![note_id, session_id, now()],
    )?;
    Ok(())
}
