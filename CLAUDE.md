# CLAUDE.md

Fork of GDQuest's GDScript formatter, being extended into a linter with custom
rules (YAML and JS), autofix, a semantic model, an LSP server and a Godot
editor integration.

- Design and contracts: `docs/specification_linter.md`. Treat it as the
  source of truth.
- Waves, tasks and file ownership: `docs/implementation_plan.md`.
- Code style: `docs/rust_coding_guidelines.md` (plain loops, few abstractions,
  no `unwrap()` outside tests).

- How to work (two copy-paste prompts): `docs/prompts/README.md`.
  Every session follows `docs/prompts/_session_rules.md`.
- Notes between sessions: `docs/handoff/`.

## Product priority

The maintainer clarified on 2026-11-09 that this is primarily a CLI for AI
agents: after edits, run lint, apply safe fixes, and use actionable diagnostics
and JSON to finish remaining work. Prioritize correctness, stable output,
clear errors and the agent fix/re-lint loop. IDE/LSP/Godot integrations are
optional compatibility features; extensive IDE polish must not delay CLI
completion or release validation.

## Rules for agents

- This is now a solo workflow (user instruction, 2026-11-09). Continue the
  current branch, use `docs/prompts/PROGRESS.md` to find the wave, and finish
  both historical tracks plus their integration work sequentially. Never
  wait for a friend, another track's branch, or a separate merge session.
  Track ownership tables describe the old collaboration arrangement; the
  solo session owns both tracks and integration files.
- Never change frozen contracts without the user explicitly approving it:
  `src/linter/diagnostic.rs`, the signatures in `src/linter/api.rs`,
  `src/semantic/` and `src/project/` signatures,
  `integrations/types/gdguard.d.ts`, and the JSON or config schemas. Changes
  must be additive.
- Before you call a task done, run:
  `cargo clippy --workspace --all-targets -- -D warnings` and
  `cargo test --workspace`.
- New behavior needs tests in the matching `tests/` folder, and spec or docs
  updates in the same change.
- Keep the formatter's behavior and CLI unchanged unless the task says
  otherwise.
