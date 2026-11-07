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

## Rules for agents

- Ask which track (A or B) and which wave you are on if the user hasn't said.
  Edit only the files that track owns in that wave, according to the
  ownership table in `docs/implementation_plan.md`. If a change needs another
  track's file, stop and tell the user.
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
