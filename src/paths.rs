//! Where lethe keeps things, and how it tells projects and Claude windows apart.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn expand(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => PathBuf::from(p),
    }
}

/// `~/.lethe`, or `LETHE_HOME`.
pub fn home() -> PathBuf {
    std::env::var_os("LETHE_HOME").map(PathBuf::from).unwrap_or_else(|| expand("~/.lethe"))
}

pub fn db_path() -> PathBuf {
    home().join("lethe.db")
}

pub fn notes_dir() -> PathBuf {
    home().join("notes")
}

pub fn log_path() -> PathBuf {
    home().join("server.log")
}

/// Claude Code's own folder: `~/.claude`, or `CLAUDE_CONFIG_DIR`.
pub fn claude_dir() -> PathBuf {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).unwrap_or_else(|| expand("~/.claude"))
}

/// This program, as installed. Hooks call it by full path, since a hook's
/// PATH may not include `~/.local/bin`.
pub fn exe() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("lethe"))
}

pub fn port() -> u16 {
    std::env::var("LETHE_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(7340)
}

pub fn page_url() -> String {
    format!("http://127.0.0.1:{}", port())
}

/// A project: the git repo a folder is in, or the folder itself outside git.
pub struct Project {
    pub name: String,
    pub path: String,
}

pub fn project_of(cwd: &Path) -> Project {
    let top = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    let path = top.unwrap_or_else(|| cwd.to_string_lossy().into_owned());
    let name = Path::new(&path).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "root".into());
    Project { name, path }
}

/// The Claude Code process this command runs under. It stays the same across
/// `/clear`, so it ties the old session to the new one in the same window.
///
/// Claude sets `CLAUDE_PID` for the commands it runs; otherwise walk up the
/// parent processes until one is `claude`.
pub fn claude_pid() -> Option<i64> {
    if let Some(pid) = std::env::var("CLAUDE_PID").ok().and_then(|p| p.parse().ok()) {
        return Some(pid);
    }
    // SAFETY: getppid has no preconditions.
    let mut pid = unsafe { libc::getppid() } as i64;
    for _ in 0..12 {
        if pid <= 1 {
            return None;
        }
        let out = Command::new("ps").args(["-o", "ppid=,comm=", "-p", &pid.to_string()]).output().ok()?;
        let line = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let (ppid, comm) = line.split_once(char::is_whitespace)?;
        let name = Path::new(comm.trim()).file_name()?.to_string_lossy().into_owned();
        if name == "claude" {
            return Some(pid);
        }
        pid = ppid.trim().parse().ok()?;
    }
    None
}
