---
name: lethe
description: Write a handoff note so a fresh session can pick up exactly where this one stops. After it's saved, the user types /clear and the new session loads the note on its own. Use when the user runs /lethe or asks for a handoff before clearing.
allowed-tools: Bash(lethe:*)
---

# lethe: hand this session's work to the next one

Write a handoff note, save it with `lethe`, then stop. When the user types
`/clear`, the new session in this window gets the note automatically.

## 1. Find out where you are

Run `lethe context`. It prints the project, the last note written for it (read
that one, so you can say what changed since), and the project's own checks from
its `.lethe.md`, if it has one.

## 2. Check the real state

Don't write from memory. Run the checks and use what they say:
- `git status` and the current branch, and any open pull request for it
- every check listed in the project's `.lethe.md`
- any background jobs or scheduled checks this session started (watchers,
  `/loop`, cron): they stop when the session is cleared, so say which ones
  existed and what they were waiting for

## 3. Write the note

How it's written:
- **Short and direct.** Say the thing first. Active voice, no filler.
- **No jargon.** Plain words a newcomer would follow. If a technical term can't
  be avoided, say what it means once, in a few words.
- **No made-up terms.** Use the real names of things (scripts, files, branches,
  tickets), never labels this session invented.
- **At most 100 lines.** Point to files instead of copying what they hold.

It starts with one heading that names the work, `# <what this is about>`, then:

1. **Goal right now:** one or two sentences on what the user is trying to get
   to, in their words where possible.
2. **Running now:** every run, deployment and background job still going, how
   far along, how to check it, and what should happen when it finishes.
3. **Done since the last note:** what changed, and the results that matter,
   each next to what it's compared with.
4. **The user's decisions:** what they chose or ruled out this session and why,
   so the next session doesn't ask again. Carry over their standing ways of
   working from the last note, even if this session didn't touch them.
5. **Waiting on the user:** open questions and anything only they can do.
6. **Next steps:** what to do first, in order, and what each one is for.
7. **Gotchas:** traps found the hard way that aren't written down elsewhere yet
   (and add them where they belong too: `CLAUDE.md`, a script's header, or memory).
8. **Key files:** the few files the next session should read first.

## 4. Save it

Pass the whole note to `lethe save` in one command:

```bash
lethe save <<'LETHE_NOTE'
# <what this is about>
...
LETHE_NOTE
```

## 5. Then

Show the user the saved note's path and a three-line summary, and tell them to
type `/clear` to hand off. Don't start anything new after saving.
