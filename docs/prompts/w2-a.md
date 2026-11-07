# W2 · Track A: config file and reporters

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W2**, on branch **`w2-a`**.

**At the same time**, Track B is building the YAML rule engine
(`RuleProvider`) and the `test-rules` command in `src/linter/custom/` and
`src/linter/rule_tester.rs`. Don't wire custom rules into the pipeline; the
merge session does that.

## Goal

Projects configure the linter with `gdguard.toml`, and output works in
`pretty`, `compact` and `json` (schema v1).

## Read

- Spec: "Configuration", "Inline ignore comments", "CLI", "JSON output".
- `docs/handoff/w1-a.md` and `docs/handoff/w1-b.md`.

## Tasks

1. **A2, config** (`src/linter/config.rs`):
   - **Discovery:** find the nearest `gdguard.toml` by searching upward from
     each file. `--config <PATH>` overrides discovery.
   - **Parsing:**
     - `[rules]` accepts a level string or a table with `level` plus
       options;
     - `[custom_rules].paths` resolves relative to the config file;
     - `[lint].exclude` and `[lint].baseline`;
     - `[[overrides]]` uses `globset`.
   - **Precedence:** CLI, then overrides, then `[rules]`, then
     `.editorconfig` (`max_line_length` only), then defaults.
   - **Validation:**
     - unknown keys are errors;
     - unknown rule ids are errors, except ids found by the custom loader:
       call `custom::load_providers` and read their `rules()`. It returns
       nothing until the merge, which is fine; write the code so it works
       once providers exist;
     - a bad level is an error.
   - Errors are `ConfigError` with the file path and key, and exit code 2.
   - Fill in `api::load_config_for`.
   - Apply per-file levels (`off` removes the rule) and rule options
     (`max-line-length.max`) to built-in rules.
2. **A3, reporters** (`src/linter/output/`):
   - `pretty`: file header, `line:col severity rule`, source snippet with
     carets under the span, `= help:` line, and a summary line with the
     fixable count.
   - `compact`: `file:line:col: severity [rule] message`.
   - `json`: exactly the v1 schema in the spec (`version`, `tool`,
     `summary`, `issues[]` with `range`, `snippet`, `explain`, `fixes`).
   - Default format: `pretty` on a TTY, `compact` otherwise. The old
     `--pretty` flag maps to `pretty`.

## You may edit

- `src/linter/config.rs`, `src/linter/output/`, `src/linter.rs`
- `src/linter/api.rs` (bodies)
- `src/cli.rs`, `src/main.rs`, `src/editorconfig.rs`
- `tests/lint/`, `tests/cli_tests.rs`
- new `tests/config/` and `tests/output/`

## Acceptance

- Config tests:
  - discovery in nested folders;
  - each precedence level;
  - overrides by glob;
  - each error type, with the message checked.
- Reporter snapshot tests for all three formats, including multi-byte
  characters in the snippet.
- The JSON output passes a test that checks every field named in the spec.
- `docs/handoff/w2-a.md` is written. Under "Requests", say exactly where the
  merge session should call `custom::load_providers` in the pipeline.
