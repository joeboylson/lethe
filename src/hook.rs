//! `lethe hook`: Claude Code's SessionStart and SessionEnd hook.
//!
//! It records which session runs in which Claude window. When a window is
//! cleared and the old session left a note waiting, it hands the note to the
//! new session. It never fails: a problem goes to `~/.lethe/hooks.log`.

use crate::db;
use crate::notes;
use crate::paths;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use std::io::{Read, Write};

pub fn cmd_hook() -> i32 {
    let mut raw = String::new();
    let _ = std::io::stdin().read_to_string(&mut raw);
    match handle(&raw) {
        Ok(Some(out)) => println!("{out}"),
        Ok(None) => {}
        Err(e) => log(&format!("{e:#}")),
    }
    0
}

fn log(msg: &str) {
    let _ = std::fs::create_dir_all(paths::home());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(paths::home().join("hooks.log")) {
        let _ = writeln!(f, "{} {msg}", db::now());
    }
}

fn handle(raw: &str) -> Result<Option<String>> {
    let input: Value = serde_json::from_str(raw)?;
    let session = input["session_id"].as_str().unwrap_or_default().to_string();
    if session.is_empty() {
        return Ok(None);
    }
    let conn = db::open()?;
    match input["hook_event_name"].as_str() {
        Some("SessionStart") => session_start(&conn, &session, &input),
        Some("SessionEnd") => {
            session_end(&conn, &session, input["reason"].as_str().unwrap_or("other"))?;
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn session_start(conn: &Connection, session: &str, input: &Value) -> Result<Option<String>> {
    let source = input["source"].as_str().unwrap_or("startup");
    let cwd = input["cwd"].as_str().map(str::to_string).unwrap_or_else(|| {
        std::env::current_dir().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default()
    });
    let pid = paths::claude_pid();
    let project = paths::project_of(std::path::Path::new(&cwd));

    // The session this one replaces: the last one in the same window.
    let previous: Option<String> = match pid {
        Some(pid) => conn
            .query_row(
                "SELECT id FROM sessions WHERE claude_pid = ?1 AND id != ?2 ORDER BY started_at DESC, rowid DESC LIMIT 1",
                params![pid, session],
                |r| r.get(0),
            )
            .optional()?,
        None => None,
    };

    conn.execute(
        "INSERT INTO sessions (id, claude_pid, cwd, project, source, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET claude_pid = excluded.claude_pid, source = excluded.source, ended_at = NULL, end_reason = NULL",
        params![session, pid, cwd, project.path, source, db::now()],
    )?;

    if source != "clear" {
        // A fresh window with this pid means any earlier sessions under it are
        // gone (Claude crashed, or the pid was reused): their notes stop waiting.
        if source == "startup" {
            if let Some(pid) = pid {
                let stale: Vec<String> = conn
                    .prepare("SELECT id FROM sessions WHERE claude_pid = ?1 AND id != ?2 AND ended_at IS NULL")?
                    .query_map(params![pid, session], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?;
                for id in stale {
                    session_end(conn, &id, "gone")?;
                }
            }
        }
        return Ok(None);
    }
    if let Some(previous) = &previous {
        conn.execute(
            "UPDATE sessions SET replaced_by = ?2, ended_at = COALESCE(ended_at, ?3), end_reason = COALESCE(end_reason, 'clear') WHERE id = ?1",
            params![previous, session, db::now()],
        )?;
    }

    // The note waiting in this window. Matched by window as well as by
    // session, so a session that started before `lethe setup` (and so was
    // never recorded) can still hand off.
    let waiting: Option<i64> = conn
        .query_row(
            "SELECT id FROM notes WHERE status = 'waiting' AND session_id != ?1 AND (session_id = ?2 OR claude_pid = ?3)
             ORDER BY id DESC LIMIT 1",
            params![session, previous, pid],
            |r| r.get(0),
        )
        .optional()?;
    let Some(note_id) = waiting else { return Ok(None) };
    let Some(note) = db::note(conn, note_id)? else { return Ok(None) };
    db::mark_picked_up(conn, note_id, Some(session))?;
    Ok(Some(
        json!({
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": notes::pickup_text(&note),
            },
            "systemMessage": format!("lethe: picked up note {} ({})", note.id, note.title),
        })
        .to_string(),
    ))
}

/// A session ended. On `/clear` its waiting note stays for the next session;
/// for any other reason (the window closed) the note stops waiting.
fn session_end(conn: &Connection, session: &str, reason: &str) -> Result<()> {
    conn.execute(
        "UPDATE sessions SET ended_at = COALESCE(ended_at, ?2), end_reason = COALESCE(end_reason, ?3) WHERE id = ?1",
        params![session, db::now(), reason],
    )?;
    if reason != "clear" {
        conn.execute("UPDATE notes SET status = 'cancelled' WHERE session_id = ?1 AND status = 'waiting'", [session])?;
    }
    Ok(())
}
