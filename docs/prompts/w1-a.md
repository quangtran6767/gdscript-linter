# W1 · Track A: move built-in rules to `Diagnostic`, exit codes

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W1**, on branch **`w1-a`**.

**At the same time**, Track B is building the YAML custom-rule loader and
testing ast-grep in `src/linter/custom/`. Don't touch that.

## Goal

The linter pipeline and all 17 built-in rules produce the new `Diagnostic`
type (real start and end spans), and the linter has proper exit codes. Old
output must look the same.

## Read

- Spec: "Shared contracts", "Library API", "CLI" (exit codes).
- `src/linter.rs`, `src/linter/rules/*.rs`, `gdextension/src/lib.rs`,
  `tests/integration_tests.rs`.

## Tasks

1. **A1, pipeline:**
   - Change `GDScriptLinter::lint` and the `Rule` trait to produce
     `Vec<Diagnostic>`. Remove `LintIssue`, or keep it only as a thin output
     adapter if that makes the change much smaller.
   - Ignore comments must keep working.
2. **A1, rules:** port all 17 rules in `src/linter/rules/`.
   - Use `Span::from_node` on the most precise node (the name node for
     naming rules, the whole statement for `unnecessary-pass`).
   - Add `help` text when the fix is obvious, for example the expected
     naming style.
3. **gdextension:** update `gdextension/src/lib.rs` to the new types and
   keep its Dictionary output keys the same. Track A owns the gdextension in
   W1 only.
4. **`api::lint_source`:** fill in the body with built-in rules only. Ignore
   `config.custom_rule_paths` for now.
5. **A5, exit codes:** `lint` exits 0 when clean, 1 when there are problems,
   and 2 on usage or config errors. Implement `--max-warnings N`: warnings
   only fail when there are more than N.

## You may edit

- `src/linter.rs`, `src/linter/rules/`, `src/linter/lib.rs`
- `src/linter/ignore_patterns.rs`, `src/linter/rule_config.rs`
- `src/linter/api.rs` (bodies only)
- `src/cli.rs`, `src/main.rs`
- `gdextension/src/lib.rs`
- `tests/lint/`, `tests/integration_tests.rs`, `tests/cli_tests.rs`

## Acceptance

- The `tests/lint/` expectations are unchanged in the default text format.
  If one must change, explain why in the handoff.
- A new test checks that each built-in rule's span start and end are right
  on at least one fixture.
- Tests for `lint_source`, the exit codes (0, 1 and 2) and `--max-warnings`.
- `cargo build -p` for the gdextension crate succeeds.
- `docs/handoff/w1-a.md` is written. List the new `Rule` trait signature
  under "Public API added".
