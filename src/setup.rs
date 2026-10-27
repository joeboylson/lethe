//! `lethe setup`, `uninstall` and `doctor`: put the skills and hooks into
//! Claude Code, take them out, and check they're still there.

use crate::{db, paths, service};
use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::fs;
use std::path::PathBuf;

const SKILLS: [(&str, &str); 2] = [
    ("lethe", include_str!("../skills/lethe/SKILL.md")),
    ("recall", include_str!("../skills/recall/SKILL.md")),
];

const EVENTS: [&str; 2] = ["SessionStart", "SessionEnd"];

fn settings_path() -> PathBuf {
    paths::claude_dir().join("settings.json")
}

fn skill_path(name: &str) -> PathBuf {
    paths::claude_dir().join("skills").join(name).join("SKILL.md")
}

fn hook_command() -> String {
    format!("{} hook", paths::exe().display())
}

/// Whether a hook entry is ours, whichever copy of lethe installed it.
fn is_ours(entry: &Value) -> bool {
    entry["hooks"].as_array().is_some_and(|hooks| {
        hooks.iter().any(|h| h["command"].as_str().is_some_and(|c| c.ends_with("lethe hook")))
    })
}

fn read_settings() -> Result<Value> {
    match fs::read_to_string(settings_path()) {
        Ok(text) if !text.trim().is_empty() => serde_json::from_str(&text)
            .with_context(|| format!("{} isn't valid JSON; fix it, then run this again", settings_path().display())),
        _ => Ok(json!({})),
    }
}

fn write_settings(settings: &Value) -> Result<()> {
    let path = settings_path();
    fs::create_dir_all(path.parent().unwrap())?;
    // Written next to it, then moved over it, so Claude never reads half a file.
    let tmp = path.with_extension("json.lethe-tmp");
    fs::write(&tmp, serde_json::to_string_pretty(settings)? + "\n")?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

/// Take lethe's entries out of the settings' hooks, keeping everyone else's.
fn remove_hooks(settings: &mut Value) {
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else { return };
    for event in EVENTS {
        if let Some(list) = hooks.get_mut(event).and_then(Value::as_array_mut) {
            list.retain(|e| !is_ours(e));
            if list.is_empty() {
                hooks.remove(event);
            }
        }
    }
    if hooks.is_empty() {
        settings.as_object_mut().unwrap().remove("hooks");
    }
}

pub fn cmd_setup() -> Result<i32> {
    let mut settings = read_settings()?;
    if !settings.is_object() {
        bail!("{} doesn't hold a JSON object", settings_path().display());
    }
    // Keep one copy of the original, the first time only.
    let backup = settings_path().with_extension("json.before-lethe");
    if settings_path().exists() && !backup.exists() {
        fs::copy(settings_path(), &backup)?;
    }
    remove_hooks(&mut settings);
    let hooks = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .context("settings.json's \"hooks\" isn't an object")?;
    for event in EVENTS {
        let list = hooks.entry(event).or_insert_with(|| json!([]));
        let Some(list) = list.as_array_mut() else { bail!("settings.json's hooks.{event} isn't a list") };
        list.push(json!({ "hooks": [{ "type": "command", "command": hook_command(), "timeout": 10 }] }));
    }
    write_settings(&settings)?;
    println!("Added the SessionStart and SessionEnd hooks to {}", settings_path().display());

    for (name, text) in SKILLS {
        let path = skill_path(name);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, text)?;
        println!("Installed /{name} in {}", path.display());
    }
    db::open()?;
    println!("Notes go in {}", paths::notes_dir().display());
    println!("\nOpen sessions pick this up when they restart. `lethe start` runs the page at {}", paths::page_url());
    Ok(0)
}

pub fn cmd_uninstall() -> Result<i32> {
    if settings_path().exists() {
        let mut settings = read_settings()?;
        remove_hooks(&mut settings);
        write_settings(&settings)?;
        println!("Removed lethe's hooks from {}", settings_path().display());
    }
    for (name, _) in SKILLS {
        let dir = skill_path(name).parent().unwrap().to_path_buf();
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
            println!("Removed /{name}");
        }
    }
    service::stop_quietly();
    println!("Your notes are still in {}; delete that folder to remove them too.", paths::home().display());
    Ok(0)
}

pub fn cmd_doctor() -> Result<i32> {
    let mut problems = 0;
    let mut check = |ok: bool, good: &str, bad: &str| {
        if ok {
            println!("  ok   {good}");
        } else {
            println!("  FIX  {bad}");
            problems += 1;
        }
    };

    let on_path = std::env::var("PATH").unwrap_or_default().split(':').any(|d| PathBuf::from(d).join("lethe").exists());
    check(on_path, "lethe is on PATH", "lethe isn't on PATH: add ~/.local/bin to PATH");

    let settings = read_settings().unwrap_or(json!({}));
    for event in EVENTS {
        let entries = settings["hooks"][event].as_array().cloned().unwrap_or_default();
        let ours: Vec<&Value> = entries.iter().filter(|e| is_ours(e)).collect();
        let current = ours.iter().any(|e| e["hooks"][0]["command"].as_str() == Some(&hook_command()));
        check(
            current && ours.len() == 1,
            &format!("{event} hook is installed"),
            &format!("{event} hook is missing or points at another copy of lethe: run `lethe setup`"),
        );
    }
    for (name, text) in SKILLS {
        let installed = fs::read_to_string(skill_path(name)).ok();
        check(
            installed.as_deref() == Some(text),
            &format!("/{name} is installed and current"),
            &format!("/{name} is missing or out of date: run `lethe setup`"),
        );
    }
    check(db::open().is_ok(), &format!("{} opens", paths::db_path().display()), "the notes database won't open; see `lethe list`");
    let running = service::server_running();
    check(running, &format!("the page is up at {}", paths::page_url()), "the page isn't running: `lethe start`");
    if cfg!(target_os = "macos") {
        check(service::login_item_installed(), "the page starts at login", "the page won't start at login: `lethe start`");
    }
    println!();
    if problems == 0 {
        println!("Everything works.");
        Ok(0)
    } else {
        println!("{problems} thing(s) to fix.");
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_only_ours() {
        let mut s = json!({
            "hooks": {
                "SessionStart": [
                    { "hooks": [{ "type": "command", "command": "other-tool start" }] },
                    { "hooks": [{ "type": "command", "command": "/x/bin/lethe hook" }] }
                ],
                "SessionEnd": [{ "hooks": [{ "type": "command", "command": "/x/bin/lethe hook" }] }]
            },
            "model": "opus"
        });
        remove_hooks(&mut s);
        assert_eq!(s["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
        assert!(s["hooks"].get("SessionEnd").is_none());
        assert_eq!(s["model"], "opus");
    }
}
