//! `lethe start`, `stop` and `open`: run the page in the background, and
//! again at every login (launchd on macOS, systemd on Linux).

use crate::paths;
use anyhow::{bail, Result};
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LABEL: &str = "dev.lethe.server";

fn macos() -> bool {
    cfg!(target_os = "macos")
}

fn plist_path() -> PathBuf {
    paths::expand(&format!("~/Library/LaunchAgents/{LABEL}.plist"))
}

fn systemd_path() -> PathBuf {
    paths::expand("~/.config/systemd/user/lethe.service")
}

pub fn login_item_installed() -> bool {
    if macos() { plist_path().exists() } else { systemd_path().exists() }
}

/// Whether lethe's page answers on its port.
pub fn server_running() -> bool {
    let addr = format!("127.0.0.1:{}", paths::port());
    let Ok(mut s) = TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_millis(500)) else { return false };
    let _ = s.set_read_timeout(Some(Duration::from_secs(2)));
    let req = format!("GET /api/health HTTP/1.0\r\nHost: {addr}\r\n\r\n");
    if s.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut body = String::new();
    let _ = s.read_to_string(&mut body);
    body.contains("lethe")
}

fn wait_for(mut check: impl FnMut() -> bool, seconds: u64) -> bool {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

fn run_quiet(program: &str, args: &[&str]) {
    let _ = Command::new(program).args(args).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

fn uid() -> u32 {
    // SAFETY: getuid has no preconditions.
    unsafe { libc::getuid() }
}

fn plist() -> String {
    let exe = paths::exe().display().to_string();
    let log = paths::log_path().display().to_string();
    let home = paths::home().display().to_string();
    let port = paths::port();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{exe}</string><string>serve</string></array>
  <key>EnvironmentVariables</key>
  <dict><key>LETHE_HOME</key><string>{home}</string><key>LETHE_PORT</key><string>{port}</string><key>PATH</key><string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string></dict>
  <key>RunAtLoad</key><true/>
  <!-- Restart after a crash, but not after `lethe stop`. -->
  <key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict>
  <key>StandardOutPath</key><string>{log}</string>
  <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#
    )
}

fn systemd_unit() -> String {
    format!(
        "[Unit]\nDescription=lethe: handoff notes for Claude Code\n\n[Service]\nExecStart={} serve\nEnvironment=LETHE_HOME={}\nEnvironment=LETHE_PORT={}\nRestart=on-failure\n\n[Install]\nWantedBy=default.target\n",
        paths::exe().display(),
        paths::home().display(),
        paths::port()
    )
}

pub fn cmd_start() -> Result<i32> {
    if server_running() {
        println!("Already running: {}", paths::page_url());
        return Ok(0);
    }
    fs::create_dir_all(paths::home())?;
    if macos() {
        fs::create_dir_all(plist_path().parent().unwrap())?;
        fs::write(plist_path(), plist())?;
        run_quiet("launchctl", &["bootout", &format!("gui/{}/{LABEL}", uid())]);
        run_quiet("launchctl", &["bootstrap", &format!("gui/{}", uid()), &plist_path().to_string_lossy()]);
    } else if cfg!(target_os = "linux") && Command::new("systemctl").arg("--user").arg("--version").output().is_ok() {
        fs::create_dir_all(systemd_path().parent().unwrap())?;
        fs::write(systemd_path(), systemd_unit())?;
        run_quiet("systemctl", &["--user", "daemon-reload"]);
        run_quiet("systemctl", &["--user", "enable", "--now", "lethe"]);
    } else {
        let log = fs::OpenOptions::new().create(true).append(true).open(paths::log_path())?;
        // SAFETY: setsid only affects the child, so it outlives this terminal.
        unsafe {
            Command::new(paths::exe())
                .arg("serve")
                .stdin(Stdio::null())
                .stdout(log.try_clone()?)
                .stderr(log)
                .pre_exec(|| {
                    libc::setsid();
                    Ok(())
                })
                .spawn()?;
        }
    }
    if !wait_for(server_running, 15) {
        println!("The page didn't start. See {}", paths::log_path().display());
        return Ok(1);
    }
    println!("Running: {}", paths::page_url());
    if login_item_installed() {
        println!("It starts again when you log in. `lethe stop` turns that off.");
    }
    Ok(0)
}

/// Stop the page and its start at login. Notes keep working without it.
pub fn stop_quietly() {
    if macos() {
        run_quiet("launchctl", &["bootout", &format!("gui/{}/{LABEL}", uid())]);
        let _ = fs::remove_file(plist_path());
    } else if systemd_path().exists() {
        run_quiet("systemctl", &["--user", "disable", "--now", "lethe"]);
        let _ = fs::remove_file(systemd_path());
    }
}

pub fn cmd_stop() -> Result<i32> {
    stop_quietly();
    if wait_for(|| !server_running(), 5) {
        println!("Stopped. Handoffs still work; only the page is off.");
        Ok(0)
    } else {
        println!("Something still answers on port {}. Was it started with `lethe serve` in a terminal?", paths::port());
        Ok(1)
    }
}

pub fn cmd_open() -> Result<i32> {
    if !server_running() {
        bail!("the page isn't running: `lethe start`");
    }
    let opener = if macos() { "open" } else { "xdg-open" };
    run_quiet(opener, &[&paths::page_url()]);
    println!("{}", paths::page_url());
    Ok(0)
}
