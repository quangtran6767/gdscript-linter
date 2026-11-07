# W6 · Track A: semantic built-in rules and project index

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W6**, on branch **`w6-a`**.

**At the same time**, Track B is exposing `context.project` to JS rules and
writing JS docs. Don't touch `src/linter/script/`, `rules_meta.rs` or
`docs/js_rules.md`.

## Goal

High-value built-in rules that use the semantic model, plus a `ProjectIndex`
for the whole Godot project.

## Read

- Spec: "Semantic model", "Project index".
- `docs/handoff/w5-*.md` and `w5-merge.md`.

## Tasks

1. **Semantic rules** in `src/linter/rules/`. Each is a `Rule` that reads the
   model, with fixture tests.

   | Rule | Fix |
   |------|-----|
   | `unused-variable` (locals and loop variables; `_`-prefixed names are exempt) | Unsafe, removes the declaration if the initializer has no calls |
   | `unused-private-function` (`_name` functions never referenced in the file; Godot virtuals like `_ready` and `_process` are exempt) | Suggestion |
   | `shadowed-variable` (a local shadowing a member or an outer local) | None |
   | `unused-local-signal` (declared, never emitted or connected in this file) | None |
   | `assigned-but-never-read` | None |

   Put each rule's `RuleMeta` text under "Requests" in your handoff, because
   B owns `rules_meta.rs`.
2. **P1, project index** (`src/project/index.rs`):
   - find the root by searching upward for `project.godot`;
   - parse `[autoload]` (strip the `*` prefix);
   - summarize every `.gd` file (`class_name`, `extends`, functions,
     signals, variables);
   - `has_member` follows `extends` chains inside the project and returns
     `Unknown` when the chain reaches an engine class;
   - cache in `.gdguard/cache/index.json`, keyed by file hash;
   - fill in `FileContext.project` (built once per run, shared read-only
     across threads).
3. Don't report "unknown member" anywhere yet; that's W8.

## You may edit

- `src/linter/rules/`, `src/project/`, `src/semantic/`
- `src/linter.rs`, `src/linter/cache.rs`
- `tests/lint/`, `tests/semantic/`, new `tests/project/`

## Acceptance

- Each new rule has fixtures with true positives and with the exemptions
  that must not be flagged.
- Project tests use a fixture project with autoloads, `class_name`, a
  3-level `extends` chain, and an engine base class. The cache is
  invalidated after a file changes.
- `tests/e2e/sample_project` shows no false positives from the new rules.
  Read only; report anything you find.
- `docs/handoff/w6-a.md` is written.
