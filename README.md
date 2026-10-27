# lethe: hand a Claude Code session to the next one

Write a handoff note, type `/clear`, and the fresh session picks up where the
old one stopped. Every note lands in one place, with a page to read them.

```
/lethe  ──► note saved, "waiting"  ──►  /clear  ──►  new session starts with the note
/recall ──► pick any earlier note by number
```

In the myth, Lethe is the river of forgetting. `/clear` forgets; lethe remembers
the part that matters.

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/joeboylson/lethe/main/install.sh | sh
lethe setup     # installs /lethe and /recall, and two hooks, into Claude Code
lethe start     # the page, at http://127.0.0.1:7340, and again at every login
```

It works on macOS and Linux. Run the install line again to update. Sessions
already open pick up the hooks when they restart.

## Every day

In Claude Code:

- **`/lethe`** checks the real state (git, running jobs, the project's own
  checks), writes the note and saves it. Then type **`/clear`**: the new session
  in that window starts with the note and the steps to pick it up.
- **`/recall`** lists this project's newest notes to choose from.
  `/recall 12` picks up note 12; `/recall all` lists every project's.

A note waits for the next `/clear` in the window that wrote it, however long
that takes. A newer `/lethe` replaces it, and closing the window cancels it
(it's still there for `/recall`). A `/clear` without a waiting note starts
fresh, as usual.

On the page: notes by project, who wrote each one and who picked it up, a
button that copies `/recall <n>` to paste into any session, and Archive.

## A project's own checks

Put a `.lethe.md` at a project's root to add checks that only make sense there.
`/lethe` runs them before writing, and the session that picks up runs them too.

```md
- `scripts/bg.sh status`: which runs are going, finished or failed
- `free -g`: memory
- the tail of `experiments/LOG.md`
```

## Commands

```
lethe setup | uninstall | doctor     put into Claude Code, take out, check
lethe start | stop | open | serve    the page
lethe list [--all]                   newest notes
lethe show <n> [--pickup]            print a note (and mark it picked up)
lethe archive <n> [--undo]
lethe save [--file f] | context      what /lethe runs
```

## How it works

Claude Code runs `lethe hook` when a session starts and ends. lethe records
each session against its Claude window (the `claude` process, which survives
`/clear`). `/lethe` saves the note against the session in that window. When the
window is cleared, the new session's start hook finds the waiting note and adds
it to the session's opening context.

Everything is in `~/.lethe/`: notes as plain Markdown in
`notes/<project>/<YYYY-MM-DD-HHMM>.md`, and `lethe.db` (SQLite) with the list of
notes and sessions. A project is the git repo you're in, or the folder outside git.
`lethe setup` adds to `~/.claude/settings.json` without touching other hooks,
and keeps the original as `settings.json.before-lethe`.

## Building

```sh
./install.sh            # builds the page and lethe from this copy, installs it
cd web && npm run dev   # the page, against a running `lethe serve`
```

The page is React with the Slag workbench style, built into the binary. The
server is Rust (axum, SQLite). Pushing a `v*` tag publishes a release.
