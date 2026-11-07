# W2 · Track B: YAML rule engine and `test-rules`

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W2**, on branch **`w2-b`**.

**At the same time**, Track A is building `gdguard.toml` loading and the
output formats in `src/linter/config.rs`, `src/linter/output/` and
`src/linter.rs`. Don't touch them. Build `Config` by hand in your tests.

## Goal

YAML rules run as a `RuleProvider` and produce correct `Diagnostic`s and
`Fix`es, and `gdguard test-rules` verifies rules using their own `tests`.

## Read

- Spec: "Custom YAML rules", "Shared contracts", "CLI".
- `docs/handoff/w1-b.md` (your engine choice) and `docs/handoff/w1-a.md`.

## Tasks

1. **B3, engine** (`src/linter/custom/provider.rs`):
   - Implement `custom::load_providers(config)`: call the W1 loader and
     return one `YamlRuleProvider` that holds all loaded rules.
   - `check_file`: run each rule on `context.tree`/`context.source`. For each
     match, build a `Diagnostic`:
     - `span` comes from the matched node;
     - `severity` comes from the rule's default (A's config overrides it
       later);
     - `$X` and `$$$X` in `message` and `help` are replaced with the
       captured text;
     - `fix` becomes one `Fix` with the right `FixSafety` (default
       `Suggestion`), with metavariables substituted.
   - Parse the tree once per file, never once per rule.
2. **B4, `test-rules`** (`src/linter/rule_tester.rs`):
   - `rule_tester::run(args)` loads the rules from the given folder (default:
     the paths from config, or `.gdguard/rules`).
   - For each rule:
     - each `valid` case must produce 0 diagnostics for that rule;
     - each `invalid` case must produce at least 1 (or exactly `errors` if
       set);
     - if `output` is set, apply the rule's fixes with `apply_edits` and
       compare.
   - **Idempotency:** re-run the rule on the fixed output; it must produce 0
     diagnostics.
   - Output one line per rule (`✓ id  N passed` or `✗ id ...`) with a
     readable diff on failure. Exit 0 when everything passes, 1 on failure,
     2 on load errors.
3. Add 2 rules in `examples/rules/` to drive the tests:
   `prefer-is-empty` (safe fix) and `no-get-node-in-process` (no fix).

## You may edit

- `src/linter/custom/`, `src/linter/rule_tester.rs`
- `examples/rules/`
- `tests/custom_rules/`

## Acceptance

- Provider tests check spans, metavariable substitution, fix edits and
  safety on fixtures in `tests/custom_rules/`.
- `test-rules` tests cover pass, wrong count, wrong output, a non-idempotent
  fix, and a load error.
- `cargo run -- test-rules examples/rules` passes.
- `docs/handoff/w2-b.md` is written. "Public API added" must list
  `load_providers` and the provider type.
