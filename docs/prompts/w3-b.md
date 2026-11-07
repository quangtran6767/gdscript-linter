# W3 · Track B: rule pack, `explain`, rule metadata, `init`

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W3**, on branch **`w3-b`**.

**At the same time**, Track A is building the fix engine in
`src/linter/fix.rs` and adding fixes to built-in rules in
`src/linter/rules/`. Don't touch those. `test-rules` already checks fixes
with `apply_edits`, so you don't need A's engine.

## Goal

A useful, tested starter rule pack; `explain` and `--list-rules` for every
rule; and `init` to set up a project in one command.

## Read

- Spec: "Custom YAML rules", "CLI".
- All `docs/handoff/w2-*.md` and `w2-merge.md`.

## Tasks

1. **B5, rule pack** in `examples/rules/`. Each rule has `rationale`,
   `help`, at least 3 `valid` and 3 `invalid` tests, and a fix where it's
   safe.

   | Rule | Fix |
   |------|-----|
   | `prefer-is-empty` (`len(x) == 0` and `len(x) > 0`/`!= 0`) | `not x.is_empty()`, Safe |
   | `no-get-node-in-process` (`get_node`/`$` inside `_process`/`_physics_process`) | Suggestion |
   | `prefer-preload-const` (`load("res://…")` with a literal inside loops or `_process`) | Suggestion |
   | `no-godot3-yield` (`yield(` → `await`) | Unsafe, simple forms only |
   | `no-string-connect` (`connect("sig", self, "method")` → `sig.connect(method)`) | Unsafe |
   | `no-print-left` (`print(`, `prints(`, `print_debug(`) | Suggestion, removes the line |
   | `prefer-is-instance-valid` (`x != null` on a variable typed as a Node subclass, when matchable) | Suggestion |

   Drop a rule only if the engine really can't express it, and explain why
   in the handoff.
2. **B6, metadata** (`src/linter/rules_meta.rs`):
   - `RuleMeta` for all 17 built-in rules: description, rationale, bad and
     good example. Use `get_all_rule_names()` to make sure none are missing,
     and add a test for that.
   - `explain::run`: `gdguard explain <rule>` prints the id, origin, default
     severity, fixable, rationale and bad/good examples for built-in and
     custom rules. Unknown ids give a "did you mean" suggestion and exit 2.
   - `--list-rules` and `--list-rules --format json`: implement
     `explain::list_rules(format)`. The CLI already calls it (from W0).
3. **B7, `init`** (`src/linter/init.rs`):
   - `gdguard init` writes a commented `gdguard.toml` (all built-in rules
     listed with their default level) and `.gdguard/rules/` with
     `prefer-is-empty.yml` as an example.
   - It never overwrites existing files: it says so and exits 1, unless
     `--force` is given.
   - It finds the Godot project root via `project.godot` when one exists.

## You may edit

- `examples/rules/`, `src/linter/rules_meta.rs`, `src/linter/explain.rs`
- `src/linter/init.rs`, `src/linter/custom/`
- `tests/custom_rules/`, new `tests/explain/`

## Acceptance

- `cargo run -- test-rules examples/rules` is all green.
- Tests for `explain` (built-in, custom, unknown id), list-rules text and
  JSON, and `init` (fresh folder, existing files, `--force`).
- `docs/handoff/w3-b.md` is written. Include a table of the rules with their
  fix safety.
