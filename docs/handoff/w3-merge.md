# Handoff w3-merge

## Merge result
- `origin/w3-a` (fix engine, built-in fixes) and `origin/w3-b` (rule pack,
  `explain`, `--list-rules`, `init`) merged into `main` with `--no-ff`. No
  conflicts.
- After `w3-a`: only the two e2e snapshot tests failed (new fixes in the
  sample output), as A's handoff announced.
- After `w3-b`: the same two, plus three stale tests in `tests/cli_tests.rs`
  that B listed (stubs for `explain`/`init`, the `--list-rules --format json`
  stub, and the old `--list-rules` text).
- B had already synced `rules_meta.rs` with A's fixes (`fixable` for
  `unnecessary-pass`, `no-else-return`, `unused-argument`, and a
  `fix-failed` entry), so A's request 3 was done before the merge.

## Integration work
| Commit | What |
|--------|------|
| `tests: explain, init and list-rules json are implemented ...` | Removed the `explain`/`init` stub cases and the `--list-rules --format json` stub test; the text test now checks the sorted format (`class-name` first, `duplicated-load` present). |
| `C2: e2e --fix/--fix-unsafe/--diff/stdin ...` | See below. |

**C2 / E2E** (`tests/e2e/`, `tests/e2e_tests.rs`):
- `.gdguard/rules/` in the sample project synced with B's rewritten
  `examples/rules/` (`prefer-is-empty`, `no-get-node-in-process`), plus
  `prefer-not-is-empty`, which flags `player.gd`'s `len(inventory) > 0`.
  `sample_project_rules_match_the_example_rules` fails if they drift.
- `diff_on_sample_project_shows_safe_fixes_and_writes_nothing`: snapshot
  `fix.diff` (built-in `no-else-return`, `unnecessary-pass`; custom
  `prefer-is-empty`, `prefer-not-is-empty`), exit 1, no file changed.
- `fix_on_sample_project_applies_builtin_and_custom_fixes`: on a temp copy,
  "Fixed 4 problems in 2 files", the fixed issues leave the report, the
  project stays formatted (`--check`), and a second `--fix` gives "Fixed 0
  problems in 0 files".
- `fix_unsafe_on_sample_project_renames_unused_arguments`.
- `stdin_fix_on_sample_project_uses_its_config`: fixed source on stdout,
  report and "Fixed 2 problems in 1 file" on stderr.
- `test_rules_passes_on_the_example_rule_pack` (8 rules).
- `explain_and_list_rules_include_sample_custom_rules`: built-in and custom
  `explain`, typo exit 2, `--list-rules --format json` lists the 3 custom ids.
- Snapshots `lint.txt`, `lint_pretty.txt`, `lint.json` regenerated: new
  fixes, B's new messages, the `prefer-not-is-empty` issue. Reviewed.
- `lint_exit_codes_on_sample_project`: player.gd now has 2 warnings, so the
  `--max-warnings` cases use 2 (pass) and 1 (fail).

`--list-rules` was already routed to `explain::list_rules` in `main.rs`
since W0; B's implementation is live through it.

## Test results
| Command | Result |
|---------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | pass (506 tests) |
| `cargo build --release` | pass |
| Formatter regression: `tests/input`, old/new binaries, default and `--reorder-code` | identical (both exit 2 on the parse-error fixtures, as before) |
| Smoke `lint --pretty .` (temp copy) | 19 problems (8 errors, 9 warnings, 2 infos), 4 fixable, exit 1 |
| Smoke `lint --format json .` | valid, `fixable: 4` |
| Smoke `lint --diff .` | exit 1 |
| Smoke `lint --fix .` | "Fixed 4 problems in 2 files", exit 1 (errors left) |
| Smoke `test-rules` | 3 rules pass, exit 0 |
| Smoke `explain no-else-return` / `explain unused-arguments` | exit 0 / "did you mean", exit 2 |
| Smoke `--list-rules` (text, json) | 22 rules (19 built-in, 3 custom) |
| Smoke `init` / again / `--force`, then lint a file | exit 0 / 1 / 0; the starter config and rule lint `len([]) == 0` |
| A manual: `cargo test --test fix`, `--fix-unsafe --stdin-filename` | pass |
| B manual: `test-rules examples/rules`, `cargo test --test explain`, `--test custom_rules` | 8 rules pass; 18 and 67 tests pass |

## E2E output excerpts
```
$ gdguard lint --fix --format compact .     # copy of tests/e2e/sample_project
...
scripts/ui/hud.gd:17:98: warning [max-line-length] Line is too long. Found 118 characters, maximum allowed is 100
Fixed 4 problems in 2 files
```
```
--- a/scripts/player.gd
+++ b/scripts/player.gd
@@ -25,4 +25,4 @@
 func has_items() -> bool:
 	# The custom `prefer-not-is-empty` rule flags `len(...) > 0`.
-	return len(inventory) > 0
+	return not inventory.is_empty()
```
```
$ gdguard lint --list-rules | tail -1
22 rules (19 built-in, 3 custom). Run `gdguard explain <rule>` for details.
```

## Release v0.29
Not tagged (the merge session never tags). The repo's release script is
`cargo run --bin make_release`: it bumps `Cargo.toml`, runs `cargo update`
and a release build, commits, and creates the tag `0.29.0` (this repo's tags
have no `v`). It requires a `0.29.0` section in `CHANGELOG.md`, so rename
`## Unreleased` to `## Release 0.29.0 (<date>)` first.

## Open issues for the next waves
- `explain` and `--list-rules` use the config of the current folder and
  ignore `lint --config` (B).
- `--list-rules` shows custom rules' default severity, not the level set in
  `gdguard.toml` (fine for a catalog; worth a note in docs).
- `no-else-return` still reports an `elif` after a returning `if` when an
  earlier `elif` doesn't return (no fix is offered then) (A).
- `unused-argument`'s fix doesn't check that `_name` is free (Unsafe) (A).
- `gdguard-ignore*` comments: still only `gdlint-ignore*` (A7, W4).
- `--disable` ids are only validated when a config loads.
- `load_rules` stops at the first invalid rule file (B).
- `prefer-is-instance-valid` knows a fixed list of node classes (B).

## Suggested changes to the next wave's prompts
- **W4-A (baseline, ignores, github/sarif):** when `--format github|sarif`
  and `--baseline`/`--update-baseline` land, delete their cases from
  `unimplemented_lint_flags_exit_with_code_2_and_name_the_flag`
  (`tests/cli_tests.rs`); after that only `lsp` remains in the "not
  implemented" tests. `run_fix_command` already sends the "Fixed" line to
  stderr for `github`/`sarif`. Baseline filtering should apply to
  `fix_files` reports too (`FixRunReport.report`).
- **W4-A (`unused-ignore`):** the `fix-failed` pattern (an engine id in
  `config::builtin_rule_ids()` plus metadata in `rules_meta.rs`) is the
  model; ask B for the `rules_meta.rs` entry or let C3 add it.
- **W4-B docs:** document the constraints gotcha (last capture wins), the
  `--fix` stdout/stderr split, and `init`.
