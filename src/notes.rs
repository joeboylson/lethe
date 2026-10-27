//! The commands the skills run: `lethe context`, `save`, `list`, `show`, `archive`.

use crate::db::{self, Note};
use crate::paths;
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A project's own checks, read when a note is written and when it's picked up.
pub const PROJECT_FILE: &str = ".lethe.md";

/// What a session is told when it picks up a note, by `/clear` or `/recall`.
pub const PICKUP: &str = "\
# Pick up from a lethe handoff

A previous session handed its work over in the note below. Carry on from it
without redoing or undoing anything.

1. Read the project's `CLAUDE.md` if it has one, then all of the note, then the
   files it lists under \"Key files\". If the project has a `.lethe.md`, do the
   checks it lists.
2. Time has passed since the note was written. Check the real state before
   acting: did each run or job the note says was going finish, fail or keep
   going? Background jobs and scheduled checks from the old session are gone;
   start again the ones still needed.
3. Tell the user where things stand: what's running, what finished since the
   note, what's waiting on them, and the first step you'll take. If the note
   and the real state disagree, say so plainly.
4. Then carry on with the note's next steps, in order, within the decisions it
   lists. Don't reopen what the user already decided.
";

fn project_here() -> Result<paths::Project> {
    Ok(paths::project_of(&std::env::current_dir()?))
}

fn read_note(note: &Note) -> String {
    fs::read_to_string(&note.path).unwrap_or_else(|e| format!("(couldn't read {}: {e})", note.path))
}

/// The first heading, or the first line with words in it.
fn title_of(body: &str) -> String {
    let line = body
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with('#'))
        .or_else(|| body.lines().map(str::trim).find(|l| !l.is_empty()))
        .unwrap_or("Untitled");
    let t = line.trim_start_matches('#').trim();
    let t = t.strip_prefix("Handoff:").unwrap_or(t).trim();
    if t.is_empty() { "Untitled".into() } else { t.chars().take(120).collect() }
}

/// `notes/<project>/<YYYY-MM-DD-HHMM>.md`, with `-2`, `-3`… if that minute is taken.
fn note_file(project: &str) -> Result<PathBuf> {
    let dir = paths::notes_dir().join(project);
    fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M").to_string();
    let mut path = dir.join(format!("{stamp}.md"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{stamp}-{n}.md"));
        n += 1;
    }
    Ok(path)
}

/// When a note was written, for people: `Oct 27 14:32`.
pub fn when(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.with_timezone(&chrono::Local).format("%b %-d %H:%M").to_string())
        .unwrap_or_else(|_| rfc3339.to_string())
}

// ---- lethe context ----------------------------------------------------------

pub fn cmd_context() -> Result<i32> {
    let project = project_here()?;
    println!("Project: {} ({})", project.name, project.path);
    let conn = db::open()?;
    match db::notes(&conn, Some(&project.path), false, 1)?.first() {
        Some(n) => println!("Last note: {} ({}): {}", n.path, when(&n.created_at), n.title),
        None => println!("Last note: none yet for this project"),
    }
    let own = Path::new(&project.path).join(PROJECT_FILE);
    match fs::read_to_string(&own) {
        Ok(text) => println!("\nThis project's own checks, from {}:\n\n{}", own.display(), text.trim_end()),
        Err(_) => println!("No {PROJECT_FILE} in this project, so only the general checks apply."),
    }
    Ok(0)
}

// ---- lethe save -------------------------------------------------------------

pub fn cmd_save(file: Option<PathBuf>) -> Result<i32> {
    let body = match file {
        Some(f) => fs::read_to_string(&f).with_context(|| format!("couldn't read {}", f.display()))?,
        None => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s)?;
            s
        }
    };
    if body.trim().is_empty() {
        bail!("the note is empty: pass it on stdin or with --file");
    }
    let project = project_here()?;
    let conn = db::open()?;
    let pid = paths::claude_pid();
    let session = match pid {
        Some(pid) => db::current_session(&conn, pid)?,
        None => None,
    }
    .or_else(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok());

    let path = note_file(&project.name)?;
    fs::write(&path, body.trim_end().to_string() + "\n")?;

    // Only a note written inside Claude can wait for that window's /clear.
    let status = if pid.is_some() && session.is_some() { "waiting" } else { "kept" };
    if status == "waiting" {
        // A newer note replaces this session's waiting one.
        conn.execute("UPDATE notes SET status = 'cancelled' WHERE session_id = ?1 AND status = 'waiting'", [&session])?;
    }
    conn.execute(
        "INSERT INTO notes (project, project_path, path, title, session_id, claude_pid, created_at, status)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![project.name, project.path, path.to_string_lossy(), title_of(&body), session, pid, db::now(), status],
    )?;
    let id = conn.last_insert_rowid();
    println!("Saved note {id}: {}", path.display());
    if status == "waiting" {
        println!("Type /clear and the new session picks it up.");
    } else {
        println!("This doesn't look like a Claude Code session, so nothing will pick it up on /clear. Use /recall {id}.");
    }
    Ok(0)
}

// ---- lethe list -------------------------------------------------------------

fn status_word(n: &Note) -> String {
    match n.status.as_str() {
        "waiting" => "waiting for /clear".into(),
        "picked_up" => format!("picked up {}", n.picked_up_at.as_deref().map(when).unwrap_or_default()),
        _ => "not picked up".into(),
    }
}

pub fn cmd_list(all: bool, limit: i64) -> Result<i32> {
    let conn = db::open()?;
    let project = project_here()?;
    let notes = db::notes(&conn, if all { None } else { Some(&project.path) }, false, limit)?;
    if notes.is_empty() {
        if all {
            println!("No notes yet. Write one with /lethe.");
        } else {
            println!("No notes for {} yet. `lethe list --all` shows every project.", project.name);
        }
        return Ok(0);
    }
    if !all {
        println!("Notes for {}:\n", project.name);
    }
    for n in &notes {
        let place = if all { format!("{}: ", n.project) } else { String::new() };
        println!("  {:>4}  {}  {place}{}  ({})", n.id, when(&n.created_at), n.title, status_word(n));
    }
    if !all {
        println!("\n  all   notes from every project");
    }
    Ok(0)
}

// ---- lethe show -------------------------------------------------------------

/// The text a session reads to pick up a note.
pub fn pickup_text(note: &Note) -> String {
    format!(
        "{PICKUP}\n---\n\nNote {} for {}, written {} ({}):\n\n{}",
        note.id,
        note.project,
        when(&note.created_at),
        note.path,
        read_note(note)
    )
}

pub fn cmd_show(id: i64, pickup: bool) -> Result<i32> {
    let conn = db::open()?;
    let Some(note) = db::note(&conn, id)? else { bail!("there's no note {id}; `lethe list` shows them") };
    if pickup {
        let session = paths::claude_pid()
            .map(|pid| db::current_session(&conn, pid))
            .transpose()?
            .flatten()
            .or_else(|| std::env::var("CLAUDE_CODE_SESSION_ID").ok());
        db::mark_picked_up(&conn, id, session.as_deref())?;
        print!("{}", pickup_text(&note));
    } else {
        print!("{}", read_note(&note));
    }
    Ok(0)
}

// ---- lethe archive ----------------------------------------------------------

pub fn set_archived(conn: &Connection, id: i64, archived: bool) -> Result<bool> {
    Ok(conn.execute("UPDATE notes SET archived = ?2 WHERE id = ?1", params![id, archived as i64])? > 0)
}

pub fn cmd_archive(id: i64, undo: bool) -> Result<i32> {
    let conn = db::open()?;
    if !set_archived(&conn, id, !undo)? {
        bail!("there's no note {id}");
    }
    println!("{} note {id}.", if undo { "Restored" } else { "Archived" });
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::title_of;

    #[test]
    fn titles() {
        assert_eq!(title_of("# Handoff: fix the tunnel\n\nbody"), "fix the tunnel");
        assert_eq!(title_of("\n\nplain first line\nmore"), "plain first line");
        assert_eq!(title_of("   "), "Untitled");
    }
}
