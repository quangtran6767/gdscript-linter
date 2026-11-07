# Handoff w1-merge

## Merge result
- `origin/w1-a` merged with `--no-ff`: no conflicts. Tests after: 305 passed.
- `origin/w1-b` merged with `--no-ff`: no conflicts (the tracks touched
  disjoint files; `Cargo.toml`/`Cargo.lock` changed only on B's side).
  Tests after: 369 passed.
- Both tracks stayed inside their "You may edit" lists.

## Integration work
- **B's loader compiles against A's migrated pipeline.** Yes: the loader
  only uses the frozen contracts plus `rule_config::get_all_rule_names()`,
  which A didn't change.
- **B's metavariable spec text** applied, plus the supported-keys and rule
  file details from B's handoff, and A's two spec lines (exit codes without
  `--max-warnings`; precise spans).
- **B's request, MSRV:** `rust-version` bumped from 1.85 to 1.89
  (`serde-saphyr` via `ast-grep-config` needs it). Side effect: with
  `rust-version >= 1.88`, clippy's `collapsible_if` starts suggesting
  let-chains and flagged 20 nested `if`s in the formatter. Fixed with
  `collapsible_if = "allow"` in `[lints.clippy]` (commented) instead of
  rewriting formatter code. Commit: `[w1-merge] fix: ...`.
- **B's request, Windows tests:** the e2e test now replaces `\` with `/`
  in stdout, so the snapshots match on Windows. The `.gitattributes`
  suggestion was not applied: the repository already has
  `* text=auto eol=lf`, which should give LF checkouts even with
  `core.autocrlf=true`. If the `reorder_tests` still fail on Windows, the
  clone probably predates that file; `git rm --cached -r . && git reset
  --hard` re-applies it.
- **A's requests** are for later waves (C1 hook point in W2, deleting stub
  test cases when B's commands land); carried below.
- **E2E extended:** `lint_exit_codes_on_sample_project` (0/1/2,
  `--max-warnings`) and a `lint --pretty` snapshot
  (`tests/e2e/expected/lint_pretty.txt`), now possible because A made the
  pretty file order stable.

## Test results
| Command | Result |
|---------|--------|
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass (after the `collapsible_if` fix) |
| `cargo test --workspace` | pass, 371 tests, 0 failures |
| `cargo build --release` | pass |
| `cargo build -p gdscript-formatter-gdextension` | pass |
| E2E smoke (release binary, copy of the sample project) | pass, see below |
| Formatter regression on `tests/input` (pre-merge vs post-merge release binary, default and `--reorder-code`) | identical |
| w1-a manual commands (span test, `lint_source` tests, CLI exit codes) | pass |
| w1-b manual commands (`cargo test --lib linter::custom`: 26, `cargo test --test custom_rules`: 38) | pass |

## E2E output excerpts
```
$ gdguard lint .                              -> 16 issues, exit 1 (same as W0)
$ gdguard lint --pretty .                     -> files in sorted order, exit 1
$ gdguard lint --max-warnings 2 scripts/player.gd   -> 2 warnings printed, exit 0
$ gdguard lint --disable nope .               -> Error: Invalid rule names: nope, exit 2
$ gdguard lint --format json|--diff|--fix .   -> not implemented yet, exit 2
$ gdguard test-rules / explain <rule>         -> not implemented yet, exit 2
```

## Open issues for the next waves
- **C1 (this merge session at the end of W2):** call
  `custom::load_providers` in the pipeline. A's suggested hook point is in
  `GDScriptLinter::lint`, right after `run_builtin_rules(...)`; read
  `w2-a.md` first, since W2-A may move it.
- **Stub tests B can't edit:** when B's `test-rules` (W2) and `explain`,
  `init`, `--list-rules --format json` (W3) land, the merge session deletes
  their cases from `tests/cli_tests.rs`
  (`unimplemented_subcommands_exit_with_code_2_and_name_the_command`,
  `list_rules_json_is_not_implemented_yet`).
- **`load-variable-name`** is a diagnostic id emitted by the `variable-name`
  rule but not in `ALL_RULES`. B's loader only rejects custom ids that
  clash with `ALL_RULES` names, so a custom rule could reuse
  `load-variable-name`. W2-A config validation and W3-B `rules_meta.rs`
  should treat it as a known built-in id.
- B's loader stops at the first invalid rule file; collecting every error
  would help `test-rules`.

## Suggested changes to the next wave's prompts
- **W2-B:** build the provider on B's `LoadedRule`, and reuse
  `FileContext.tree` (B's handoff explains how: `StrDoc` has public fields)
  instead of re-parsing.
- **W2-A:** `[rules]` validation must accept custom ids from
  `load_providers(...).rules()` and the extra built-in id
  `load-variable-name`. When `--format`/`--config` land, delete their cases
  from `unimplemented_lint_flags_exit_with_code_2_and_name_the_flag`.
- **W4-B docs:** document that `follows`/`precedes` need
  `pattern: { context, selector: expression_statement }` for statements, and
  the node-path constraint pattern.
