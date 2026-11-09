# START: continue the solo roadmap on the current branch

The user switched to working alone on 2026-11-09. Complete both historical
tracks sequentially; never wait for a friend or a `wN-b` remote branch.
This overrides the former two-person orchestration and ownership limits in
individual wave prompts. Preserve frozen contracts and formatter behavior.

## Resume

1. Read `CLAUDE.md`, `_session_rules.md`, `PROGRESS.md`, the current wave's
   task prompts, relevant spec sections, and existing handoffs.
2. Inspect `git status` and the current branch. Continue that branch; keep
   user changes and do not reset, switch to main, or overwrite work.
3. Existing handoffs describe completed work, but verify their known issues
   and integration requests. Do not redo completed tasks.
4. Tell the user the wave and next unfinished work, then proceed.

## Product priority (maintainer clarification)

This project is primarily a CLI for AI agents. Finish the CLI, fix/re-lint
loop, useful JSON/help/fixes and predictable errors first. Treat W7 IDE and
Godot integrations as optional compatibility work: keep basic tests/builds,
record limitations, and do not spend time on full IDE polish before W8's
CLI completion/performance/distribution tasks.

## Work through each wave

Read the following task files for the current wave:

| Wave | Task prompts |
| --- | --- |
| w0, sm0 | `<wave>.md` |
| w1 through w8 | `<wave>-a.md`, then `<wave>-b.md` |

1. Finish all incomplete tasks from both prompts, including tests and docs.
   A solo session may edit every file those prompts or `MERGE.md` assign.
2. Perform that wave's integration row and applicable handoff requests from
   `MERGE.md` directly on the current branch. No synthetic branches or merge
   commits are required.
3. Run format, clippy, workspace tests, release build, feature-off build
   from SM0 onward, and release CLI smoke tests. Keep formatter regression
   tests green. Fix failures before marking a wave done.
4. Write/update the track handoffs and `docs/handoff/<wave>-merge.md`, with
   checks, open issues, and exact remaining work. Update the spec, plan and
   changelog. Commit completed work when git writes are available.
5. Mark the wave done and advance `current_wave` only after acceptance
   passes. Repeat: w0, w1, w2, w3, w4, sm0, w5, w6, w7, w8, finished.
6. Do not create release tags automatically. Keep publishing separate from
   implementation unless authorized by the user. Report any unavailable
   network/git operations accurately; never misreport a push or a merge.

When all waves are complete, report what was built, validation, unresolved
issues, and suggested release tags. If interrupted, leave PROGRESS and
handoffs accurate so the next session can resume without waiting on anyone.
