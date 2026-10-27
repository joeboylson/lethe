---
name: recall
description: Pick up a handoff note written with /lethe. With a note number, loads that note; without one, lists this project's newest notes for the user to choose from. Use when the user runs /recall or asks to pick up a handoff.
argument-hint: "[note number | all]"
allowed-tools: Bash(lethe:*)
---

# recall: pick up a handoff note

The user's argument: `$ARGUMENTS`

- **A note number:** run `lethe show <number> --pickup` and follow what it
  prints. It starts with the pickup steps, then the note.
- **`all`:** run `lethe list --all --limit 15`, show the list, and ask which
  note to pick up.
- **Nothing:** run `lethe list`, show the list as it prints, and ask which note
  to pick up (a number, or `all` for every project's notes). Don't pick one
  for them.

Once they choose, run `lethe show <number> --pickup` and follow it.
