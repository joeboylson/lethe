//! lethe: hand a Claude Code session's work to the next one across `/clear`.

mod db;
mod hook;
mod notes;
mod paths;
mod server;
mod service;
mod setup;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lethe", version = concat!(env!("CARGO_PKG_VERSION"), " (built ", env!("LETHE_BUILT"), ")"))]
#[command(about = "Hand a Claude Code session's work to the next one across /clear")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Install the /lethe and /recall skills and the hooks into Claude Code
    Setup,
    /// Take lethe's skills and hooks out of Claude Code (notes stay)
    Uninstall,
    /// Check everything is in place
    Doctor,
    /// Run the page in the background, and again at every login
    Start,
    /// Stop the page and its start at login
    Stop,
    /// Open the page in the browser
    Open,
    /// Run the page in this terminal
    Serve,
    /// Save a note (from stdin, or --file) for this session's next /clear
    Save {
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// The project, its last note and its own checks, for writing a note
    Context,
    /// This project's newest notes
    List {
        /// Every project's
        #[arg(long)]
        all: bool,
        #[arg(long, default_value_t = 5)]
        limit: i64,
    },
    /// Print a note
    Show {
        id: i64,
        /// Mark it picked up by this session, with the pickup steps first
        #[arg(long)]
        pickup: bool,
    },
    /// Hide a note from lists and the page
    Archive {
        id: i64,
        /// Bring it back
        #[arg(long)]
        undo: bool,
    },
    /// Claude Code's SessionStart and SessionEnd hook
    #[command(hide = true)]
    Hook,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Setup => setup::cmd_setup(),
        Cmd::Uninstall => setup::cmd_uninstall(),
        Cmd::Doctor => setup::cmd_doctor(),
        Cmd::Start => service::cmd_start(),
        Cmd::Stop => service::cmd_stop(),
        Cmd::Open => service::cmd_open(),
        Cmd::Serve => server::cmd_serve(),
        Cmd::Save { file } => notes::cmd_save(file),
        Cmd::Context => notes::cmd_context(),
        Cmd::List { all, limit } => notes::cmd_list(all, limit),
        Cmd::Show { id, pickup } => notes::cmd_show(id, pickup),
        Cmd::Archive { id, undo } => notes::cmd_archive(id, undo),
        Cmd::Hook => Ok(hook::cmd_hook()),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("lethe: {e:#}");
            std::process::exit(1);
        }
    }
}
