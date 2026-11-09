# Progress

Solo mode since 2026-11-09: one session completes both tracks and
integration on the current branch (`w4-a`). No partner waits.

The solo integration step updates this file after validation.

current_wave: done

| Wave | Who works | Status |
|------|-----------|--------|
| w0  | A only | done |
| w1  | A and B | done |
| w2  | A and B | done |
| w3  | A and B | done |
| w4  | solo (A + B + integration) | done |
| sm0 | solo | done |
| w5  | solo (A + B + integration) | done |
| w6  | solo (A + B + integration) | done |
| w7  | solo (A + B + integration) | done |
| w8  | solo (A + B + integration) | done |
| w9  | solo (maintainer-requested rules) | done |

## Resume notes

- Current branch: `w4-a`; do not wait for or require `w4-b`.
- W4-A was already committed. W4-B and integration are complete; see
  `docs/handoff/w4-b.md` and `docs/handoff/w4-merge.md`.
- SM0 contracts and validation are complete; see `docs/handoff/sm0.md`
  and `docs/handoff/sm0-merge.md`.
- W5 complete: semantic builder (18 fixtures), optional JS rules, C4 e2e.
  See `docs/handoff/w5-{a,b,merge}.md`.
- W5 checks: 564 workspace tests; 553 feature-off tests; clippy, release,
  feature-off build and release semantic smoke pass. Semantic overhead ~12%.
- W6 complete: five semantic rules, shared cached project index, JS project
  bindings/examples/docs and C5. See `docs/handoff/w6-{a,b,merge}.md`.
- W6 checks: 579 workspace tests; 563 feature-off tests; clippy, strict TS,
  release build and 12 release smoke checks pass. 13 example rules pass.
- W7 practical compatibility complete: LSP, native API/panel and C6 parity.
  585 workspace / 569 feature-off tests; fmt/clippy/release; real Godot 4.7.2
  cold-start/import and native smoke pass (0 failures, 5 parity fixtures).
- User priority: CLI + AI-agent fix/re-lint/JSON. Optional IDE polish must not
  delay completion. Native hot-loading requires a restart; GUI QA optional.
- W8 complete: cross-file rules, parse-error, 1000-file benchmark (cold
  77 ms / warm 48 ms), rename to gdguard (alias gdscript-formatter), release
  dry run, CLI-first docs and short README. See docs/handoff/w8-*.md.
- W8 checks: 591 workspace / 575 feature-off tests; fmt/clippy/release;
  Godot 4.7.2 native smoke 0 failures.
- W9 (maintainer request after testing W8): ten new built-in rule ids,
  scene-aware unused-private-function, non-fatal duplicate class_name,
  fix-engine repair exception and the optional Godot check. 608 workspace
  / 592 feature-off tests. See docs/handoff/w9.md.
- Next: the maintainer tests the tool first. Do not start any task, tag or
  release until they say so. Never send anything upstream to GDQuest.
- User explicitly authorized commits and pushes to origin/w4-a on 2026-11-09.
- No real-game project path was provided; sample dogfooding passed.
