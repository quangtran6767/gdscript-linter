# Handoff w0-merge

## Merge result
- Merged `origin/w0` into `main` with `--no-ff`. No conflicts (single-session
  wave, `main` hadn't moved).
- `cargo test --workspace` passed right after the merge (294 tests).

## Integration work
- W0 row of `MERGE.md`: "Check that the contracts compile and that the
  handoff lists every signature." Done: both crates build, clippy is clean,
  and `docs/handoff/w0.md` lists every public type and function in
  `diagnostic.rs`, `api.rs`, the Track B entry points and `OutputFormat`,
  with their derives.
- W0's handoff had no requests. Its spec updates were already applied in the
  branch (W0 may edit the spec).

## Test results
| Command | Result |
|---------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (was failing on `main` before W0; W0 fixed the gdextension) |
| `cargo test --workspace` | pass, 294 tests, 0 failures |
| `cargo build --release` | pass |
| E2E smoke (release binary, copy of `tests/e2e/sample_project`) | pass, see below |
| Formatter regression: format `tests/input` with the pre-merge and post-merge release binaries | identical output (default and `--reorder-code`); both exit 2 because some inputs have intentional parse errors |
| Handoff "How to test manually" commands | pass |

## E2E output excerpts
```
$ gdguard lint .                       -> 16 issues, exit 1
./scripts/enemies/enemy.gd:3:signal-name:error: Signal name 'OnDefeated' should be in snake_case format
./scripts/enemies/enemy.gd:5:enum-name:error: Enum name 'state' should be in PascalCase format
$ gdguard lint --pretty .              -> grouped output, exit 1
$ gdguard lint --format json .         -> `--format` is not implemented yet, exit 2
$ gdguard lint --diff .                -> `--diff` is not implemented yet, exit 2
$ gdguard lint --fix .                 -> `--fix` is not implemented yet, exit 2 (copy unchanged)
$ gdguard test-rules                   -> `test-rules` is not implemented yet, exit 2
$ gdguard explain unused-argument      -> `explain` is not implemented yet, exit 2
$ gdguard lint --list-rules            -> old rule list, exit 0
$ gdguard init / lsp                   -> not implemented yet, exit 2
```
(`gdguard` = `target/release/gdscript-formatter` until W8.)

## Open issues for the next waves
- The pretty lint output orders files through a `HashMap`, so it isn't
  stable. W2-A's reporters should sort files; only then can e2e snapshot
  pretty output.
- The e2e sample project triggers 14 of 17 built-in rules (not
  `class-name`, `constant-name`, `function-argument-name`).
- `tests/e2e_tests.rs` uses `env!("CARGO_BIN_EXE_gdscript-formatter")`; the
  W8 rename must update it.

## Suggested changes to the next wave's prompts
- W1-A: when `--max-warnings` is implemented, delete its
  `exit_not_implemented` block in `src/main.rs` and its case in
  `tests/cli_tests.rs::unimplemented_lint_flags_exit_with_code_2_and_name_the_flag`.
  Same for every later flag (W2-A `--format`/`--config`, W3-A
  `--fix`/`--fix-unsafe`/`--diff`/`--stdin-filename`, W4-A baseline flags),
  and for the subcommand cases in
  `unimplemented_subcommands_exit_with_code_2_and_name_the_command` (W2-B
  `test-rules`, W3-B `explain`/`init`, W7-A `lsp`) and
  `list_rules_json_is_not_implemented_yet` (W3-B). B owns those
  implementations but not `tests/cli_tests.rs`, so B should put its CLI tests
  in its own test folders and list the stale stub test cases under
  "Requests" for the merge session to delete.
- Every wave: `Config.max_line_length` and `indent_size` are `Option<usize>`
  (`None` = not set in `gdguard.toml`).
