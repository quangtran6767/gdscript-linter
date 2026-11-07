# W1 · Track B: ast-grep spike and YAML rule loader

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W1**, on branch **`w1-b`**.

**At the same time**, Track A is changing every built-in rule and
`src/linter.rs` to the new `Diagnostic` type. Don't touch those files. Use
only the frozen types in `src/linter/diagnostic.rs` and `src/linter/api.rs`.

## Goal

Decide the matching engine for custom rules, then load and validate YAML rule
files into `RuleMeta` plus an internal compiled form ready for W2's engine.

## Read

- Spec: "Custom YAML rules", "Shared contracts", "Configuration".
- The ast-grep docs, for the `Language` trait and the rule config format.

## Tasks

1. **B1, spike (time limit: about half a day of work):**
   - Add `ast-grep-core` and `ast-grep-config` at `=0.51.1` (allowed for
     this task). Implement ast-grep's `Language` trait for
     `tree_sitter_gdscript` in `src/linter/custom/language.rs`.
   - Prove each of these with a test in `src/linter/custom/spike_tests.rs`:
     - `len($X) == 0` matches and captures `X`;
     - `get_node($$$A)` matches calls with any arguments;
     - `inside` (a call inside `func _process`) works;
     - `has` and `not` work;
     - a fix template `$X.is_empty()` rewrites correctly;
     - GDScript's own `$NodePath` syntax works: find a way to escape it, or
       a `kind`+`regex` alternative.
   - **On no-go** (it won't compile or can't match GDScript reasonably):
     remove the crates and implement the fallback, a `query:` field with a
     raw tree-sitter query in the same YAML envelope. Explain why in the
     handoff.
2. **B2, loader** (`src/linter/custom/loader.rs`):
   - Find `*.yml` and `*.yaml` files in each folder of
     `config.custom_rule_paths`.
   - Parse the fields from the spec: `id`, `severity`, `message`, `help`,
     `rationale`, `rule`, `constraints`, `fix`, `fixSafety`, `tests`.
   - Validate:
     - `id` is kebab-case and unique across all files and built-in names (use
       `rule_config::get_all_rule_names()`);
     - the `rule` compiles and its patterns parse as GDScript;
     - there are no unknown keys;
     - `fixSafety` is one of the allowed values.
   - Every error names the file and the key or line.
   - Output: `Vec<LoadedRule { meta: RuleMeta, compiled: <engine type>, tests: RuleTests }>`.
3. Keep `custom::load_providers` returning `Ok(Vec::new())`. W2 connects the
   engine.

## You may edit

- `src/linter/custom/` (any new files inside)
- `tests/custom_rules/`
- `examples/rules/` (fixtures only)
- `Cargo.toml` and `Cargo.lock` (only the ast-grep crates and a YAML parser
  if you need one)

## Acceptance

- Spike tests pass, or the fallback is implemented and tested.
- Loader tests cover:
  - a valid rule;
  - each validation error, with an exact message check;
  - a duplicate id across two files;
  - an id that clashes with a built-in.
- `docs/handoff/w1-b.md` is written.
  - "Decisions" must state go or no-go and the `$NodePath` escape rule.
  - "Spec updates" must give the exact text for the metavariable section.
