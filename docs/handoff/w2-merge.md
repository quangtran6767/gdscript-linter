# Handoff w2-merge

## Merge result
- `origin/w2-a` (config, reporters) and `origin/w2-b` (YAML provider,
  `test-rules`) merged into `main` with `--no-ff`. No conflicts.
- After `w2-a`: only the two e2e snapshot tests failed, as A's handoff
  announced (default output is now compact, pretty has the new layout).
- After `w2-b`: the same two, plus
  `unimplemented_subcommands_exit_with_code_2_and_name_the_command`, which
  still expected `test-rules` to be a stub (B's request 1).

## Integration work
| Commit | What |
|--------|------|
| `tests: drop test-rules stub cases` | Removed the two `test-rules` cases from the stub test (B request). |
| `C1: run custom rule providers in the lint pipeline` | See below. |
| `e2e: custom rules and gdguard.toml ...` | Sample project config, rules, new tests, regenerated snapshots. |
| `addon: run lint with --format compact ...` | `addons/GDQuest_GDScript_formatter/plugin.gd` passes `--format compact` and parses `file:line:col: severity [rule] message` (A request 2). Regex checked against every line of `tests/e2e/expected/lint.txt`; the file stays formatted. |
| `fix: build reorder test expected paths from components` | `tests/reorder_tests.rs` built the expected path with `replace("/input/", "/expected/")`, a no-op on Windows (B request). `tests/integration_tests.rs` doesn't have the problem. |

**C1 details** (`src/linter.rs`, `src/linter/config.rs`, `src/linter/api.rs`, `src/main.rs`):
- `GDScriptLinter::lint_with_providers(source, path, &mut [Box<dyn RuleProvider>])`
  parses once, runs the built-in rules, then each provider's `check_file`
  on the same tree, then inline ignores and the position sort. `lint()`
  calls it with no providers (gdextension and old callers unchanged).
- `lint_with_settings` gained a `providers` parameter; it applies
  `[rules]`/override levels to built-in and custom ids alike.
- `config::load_with_providers(path)` loads the config, its providers, and
  validates rule ids against both, so rules load **once** per config (no
  more double load in validation). `ConfigCache` keeps a `LoadedConfig
  { compiled, providers }` per config file.
- `api::lint_source` calls `custom::load_providers(config)` for its call.
- `--disable` now accepts custom ids (A request 4): names are checked when
  each config loads, against built-in ids plus that config's custom rules.
  Same error text and exit code 2 as before. `rule_config::validate_rule_names`
  is no longer called by the CLI.
- New tests in `tests/config/main.rs`: built-in and custom diagnostics
  sorted together, custom levels from `[rules]` and `[[overrides]]` (`off`
  too), `--disable` and `gdlint-ignore` on a custom id, invalid rule file →
  error, unknown `--disable` id, `api::lint_source` with custom rules.

**E2E** (`tests/e2e/`):
- `sample_project/.gdguard/rules/`: copies of `examples/rules/prefer-is-empty.yml`
  and `no-get-node-in-process.yml`.
- `sample_project/gdguard.toml`: `[custom_rules].paths`, raises
  `no-get-node-in-process` to `error` and lowers `unused-argument` to `info`.
- `player.gd` comments updated (no code change).
- New tests: `lint_sample_project_json_output` (snapshot `lint.json`, tool
  version replaced by `<version>`), `custom_rules_report_with_config_levels`,
  `test_rules_passes_on_sample_project_rules`. `lint_exit_codes_on_sample_project`
  now uses `--disable no-get-node-in-process` (custom id) for the
  `--max-warnings` cases.
- `run_in_sample_project_raw` added: the `\` → `/` normalization broke JSON
  escapes; JSON paths already use `/`.

## Test results
| Command | Result |
|---------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | pass (449 tests) |
| `cargo build --release` | pass |
| Formatter regression: `tests/input` with old/new binaries, default and `--reorder-code` | identical (both exit 2 on the parse-error fixtures, as before) |
| Smoke `lint --pretty .` (temp copy) | 18 problems (8 errors, 8 warnings, 2 infos), exit 1 |
| Smoke `lint --format json .` | valid JSON, origins `builtin` and `custom`, exit 1 |
| Smoke `lint --diff .` / `lint --fix .` / `explain prefer-is-empty` | "not implemented yet", exit 2 (W3/W4) |
| Smoke `test-rules` (uses `[custom_rules].paths`) | 2 rules pass, exit 0 |
| Smoke `--disable prefer-is-empty,no-get-node-in-process` | custom ids accepted, exit 0 (only infos left) |
| Smoke `--disable nope` | "Invalid rule names: nope", exit 2 |
| Smoke broken rule file | `<path>/broken.yml: \`rule\`: missing required key`, exit 2 |
| Smoke `[rules] prefer-is-empti` | "did you mean \"prefer-is-empty\"?", exit 2 |
| A manual: `--config bad.toml` | did you mean, exit 2 |
| B manual: `test-rules examples/rules` / `wrong_output` / `load_error` | exit 0 / 1 / 2 |

## E2E output excerpts
```
$ gdguard lint --format compact scripts/player.gd     # in tests/e2e/sample_project
scripts/player.gd:12:15: info [unused-argument] Function argument 'delta' is unused. ...
scripts/player.gd:14:19: error [no-get-node-in-process] Don't call `get_node()` in `_process()`; cache the node in an `@onready` variable
scripts/player.gd:16:5: warning [prefer-is-empty] Use `inventory.is_empty()` instead of `len(inventory) == 0`
scripts/player.gd:20:31: info [unused-argument] Function argument 'source' is unused. ...
```
```
 16:5 warning [prefer-is-empty] Use `inventory.is_empty()` instead of `len(inventory) == 0`
   |
16 |     if len(inventory) == 0:
   |        ^^^^^^^^^^^^^^^^^^^
   = help: `is_empty()` states the intent and works on Array, Dictionary and String.
   = fix (safe): Replace with `inventory.is_empty()`

Found 18 problems (8 errors, 8 warnings, 2 infos) in 3 files. 1 fixable with --fix.
```
```
$ gdguard test-rules
✓ no-get-node-in-process  6 passed
✓ prefer-is-empty  6 passed

2 rules, 2 passed, 0 failed
```

## Open issues for the next waves
- `gdguard-ignore*` comments aren't parsed yet (only `gdlint-ignore*`);
  that is A7 (W4). Tests use the legacy prefix.
- `--disable` with an unknown id is only reported when a config loads, so a
  run where every file is excluded doesn't report it.
- `rule_tester::run` still has the "`not implemented yet` means no config"
  branch (B noted it is dead after W2-A); B can delete it.
- Pretty carets assume one column per character (A's note: emoji/CJK).
- `.editorconfig` is read per file (W4 cache work).
- `load_rules` stops at the first invalid rule file (B).
- Custom rules default `fixSafety` is `suggestion`, so `--fix` only applies
  them when the rule says `safe` (`prefer-is-empty` does).

## Suggested changes to the next wave's prompts
- **W3-A (fix engine):** run fixes through `lint_with_settings` with the
  config's providers (`LoadedConfig.providers` in `ConfigCache`), so custom
  rule fixes are applied and re-linted each pass like built-in ones. When
  `--fix`, `--fix-unsafe` and `--diff` land, delete their cases from
  `unimplemented_lint_flags_exit_with_code_2_and_name_the_flag` in
  `tests/cli_tests.rs` (or ask the merge session to). `api::fix_source`
  should load providers like `api::lint_source` does.
- **W3-B (explain / rules_meta):** `explain <custom-id>` and
  `--list-rules --format json` need the custom `RuleMeta`s:
  `config::load_with_providers` (or `custom::load_providers`) gives them.
  Delete `explain` from `unimplemented_subcommands_...` and the
  `list_rules_json_is_not_implemented_yet` test when implemented.
- **C2 (merge):** the sample project is ready for an e2e `--fix`:
  `scripts/player.gd:16` has the one safe custom fix (`prefer-is-empty`).
