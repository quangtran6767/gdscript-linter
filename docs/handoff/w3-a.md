# Handoff w3-a

## Summary
The fix engine works: `lint --fix`, `--fix-unsafe`, `--diff` and
`--stdin-filename` are implemented for built-in and custom rules, and
`api::fix_source` has its body. Fixes run in up to 10 passes with overlap
handling; a fix that breaks parsing is rolled back and reported as
`fix-failed`; files that were formatted are formatted again after fixing.
Three built-in rules gained fixes.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| A4 engine | done | `src/linter/fix.rs`, `fix_files`/`fix_stdin` in `src/linter.rs`, `api::fix_source`. |
| CLI `--fix`, `--fix-unsafe`, `--diff`, `--stdin-filename` | done | `src/main.rs`. `similar` moved to `[dependencies]`. |
| A6 built-in fixes | done | `unnecessary-pass` (Safe), `no-else-return` (Safe; Unsafe when the else body declares names), `unused-argument` (Unsafe, `_` prefix). |
| Tests | done | `tests/fix/main.rs` (21 tests) and fixtures in `tests/fix/builtin/`; `src/linter/rules/fix_helpers.rs` unit tests; 2 `api` tests. |

## Files changed
- `src/linter/fix.rs`: the engine (`fix_source_with_settings`, `is_formatted`, `unified_diff`, `MAX_FIX_PASSES`, `FIX_FAILED_RULE_ID`).
- `src/linter.rs`: `GDScriptLinter::has_syntax_errors`; `FixedFile`, `FixRunReport`, `fix_files`, `fix_stdin`.
- `src/linter/api.rs`: `fix_source` body; the stub test became two real tests.
- `src/linter/config.rs`: `builtin_rule_ids()` includes `fix-failed`, so it can be configured, disabled, and is `origin: builtin` in JSON.
- `src/linter/rules.rs`, new `src/linter/rules/fix_helpers.rs`: line helpers for fixes.
- `src/linter/rules/{unnecessary_pass,no_else_return,unused_argument}.rs`: fixes.
- `src/main.rs`: input handling split into `prepare_lint_input` (files, piped stdin, `--stdin-filename`); `run_fix_command`.
- `Cargo.toml`: `similar = "=2.7.0"` moved from dev-dependencies to dependencies.
- `tests/cli_tests.rs`: removed the `--fix`, `--fix-unsafe`, `--diff` and `--stdin-filename` cases from the "not implemented" test.
- `tests/fix/`: new.

## Public API added
```rust
// src/linter/fix.rs
pub const MAX_FIX_PASSES: usize = 10;
pub const FIX_FAILED_RULE_ID: &str = "fix-failed";
pub struct FixedSource { pub output: String, pub applied: usize, pub remaining: Vec<Diagnostic> }
pub fn fix_source_with_settings(linter: &mut GDScriptLinter, path: &Path, source: &str,
    settings: &FileSettings, providers: &mut [Box<dyn RuleProvider>], allow_unsafe: bool)
    -> Result<FixedSource, String>;
pub fn is_formatted(source: &str, config: &FormatterConfiguration) -> bool;
pub fn unified_diff(display_path: &str, original: &str, fixed: &str) -> String;

// src/linter.rs
impl GDScriptLinter { pub fn has_syntax_errors(&mut self, source_code: &str) -> Result<bool, String>; }
pub struct FixedFile { pub path: PathBuf, pub display_path: String, pub original: String,
                       pub fixed: String, pub applied: usize }
pub struct FixRunReport { pub report: LintReport, pub changed_files: Vec<FixedFile>,
                          pub stdin_output: Option<String> }   // + fn applied(&self) -> usize
pub fn fix_files(files: &[PathBuf], options: &LintRunOptions, allow_unsafe: bool) -> Result<FixRunReport, String>;
pub fn fix_stdin(source_code: String, config_lookup_path: &Path, display_name: &str,
                 options: &LintRunOptions, allow_unsafe: bool) -> Result<FixRunReport, String>;

// src/linter/rules/fix_helpers.rs
pub fn line_start(source: &str, byte: usize) -> usize;
pub fn line_end(source: &str, byte: usize) -> usize;
pub fn line_indentation(source: &str, byte: usize) -> &str;
pub fn remove_lines_edit(source: &str, start_byte: usize, end_byte: usize) -> Option<Edit>;
```

New diagnostic id: `fix-failed` (Warning, no fix). Message:
``The fix for `<rule>` was not applied because it would break the code``,
help: "Fix this issue by hand. If the fix comes from a custom rule, check
its `fix` template." Its span is the span of the issue whose fix failed.

## Decisions
- **One fix per diagnostic:** the first fix the safety level allows.
  `Suggestion` fixes are never applied.
- **Overlap:** fixes are sorted by (start, end) of all their edits; a fix is
  skipped when it starts before the previous kept fix ends, or starts at the
  same byte (so the order of two insertions never depends on rule order).
- **Finding the bad fix:** when a pass breaks parsing, the pass's fixes are
  added one at a time from the original text; each one that breaks parsing
  (or makes `apply_edits` fail) is dropped. Dropped fixes are remembered by
  content (rule, replaced text, replacement), not position, so they are not
  retried on later passes after other fixes moved the code. `fix-failed` is
  reported on the issues still present at the end whose fix was dropped, so
  its span always fits the final text. It goes through `[rules]` levels
  like other diagnostics (`fix-failed = "off"` works).
- **Input with syntax errors** is returned unchanged with its diagnostics:
  the engine can't tell whether a fix made it worse, and the formatter
  refuses such files too.
- **Formatter step:** uses `FormatterConfiguration::default()` plus the
  file's `.editorconfig`. "Formatted" means formatting the original gives
  the same text. If formatting the result fails, the unformatted result is
  kept. The step runs only when at least one fix was applied.
- **`--diff`:** prints only the diffs (no report), `--- a/<path>` /
  `+++ b/<path>` headers (absolute paths are shown as is), exit 1 when a file
  would change, else 0. With `--fix-unsafe` it includes unsafe fixes.
- **`--fix` output:** writes files, then the normal report of what's left,
  then `Fixed N problems in M files` (singular for 1). The line goes to
  stdout for `pretty`/`compact` and to stderr for `json` (and later
  `github`/`sarif`) so stdout stays parseable. Exit code follows the issues
  left.
- **`--stdin-filename <PATH>`:** reads stdin even on a terminal; `PATH`
  (relative to the current folder) finds `gdguard.toml` and `.editorconfig`
  and is the name shown. Combined with file arguments it's a usage error
  (exit 2); `-` alone is allowed. With `--fix`, the fixed source (always,
  even unchanged) goes to stdout and the report plus the `Fixed` line to
  stderr without colors. With `--diff`, the diff goes to stdout.
- **`no-else-return` fixes:**
  - `elif` → `if` only when every earlier branch returns. The rule still
    reports an `elif` whenever the `if` body returns (unchanged behavior);
    in the other cases the diagnostic has no fix (see known issues).
  - `else`: removes the `else:` line and dedents every body line that
    starts with the body's indentation; blank and less-indented lines stay.
    `else: return x` on one line keeps only the body. No fix when the
    `else` line has a comment or other code, or the body has a multi-line
    string. Unsafe when the body declares `var`/`const` at its top level.
- **`unnecessary-pass`:** only when `pass` is alone on its line (a trailing
  comment blocks the fix).
- **`unused-argument`:** Unsafe because the rule can miss uses it doesn't
  see.

## Requests for the merge session
1. **Regenerate the e2e snapshots** `lint_pretty.txt` and `lint.json`: the
   sample project now shows the new fixes (`fix (safe)`/`fix (unsafe)`
   lines, `fixable` 1 → 3). No other change.
2. **C2:** `lint --fix` on a copy of `tests/e2e/sample_project` changes
   `player.gd` (`prefer-is-empty`, custom) and `enemies/enemy.gd`
   (`no-else-return`, `unnecessary-pass`): "Fixed 3 problems in 2 files",
   the project stays formatted, a second `--fix` gives "Fixed 0 problems in
   0 files". `lint --diff .` gives exit 1 and those hunks.
3. **B's `rules_meta.rs`:** set `fixable: true` for `unnecessary-pass`,
   `no-else-return` and `unused-argument`, and add metadata for
   `fix-failed` (an engine diagnostic, not a `Rule`) so `explain fix-failed`
   works.

## Spec updates
Replace the "Fix engine" list with:

1. Collect, for each diagnostic, its first fix allowed by the safety level
   (`--fix`: Safe; `--fix-unsafe`: Safe and Unsafe; Suggestion never).
2. Sort them by position and skip any fix that overlaps one already chosen
   or starts at the same byte. Skipped fixes are retried on the next pass.
3. Apply the fixes with `apply_edits`, then re-parse. If the new tree has
   `ERROR` or `MISSING` nodes, add the pass's fixes one at a time to find
   the ones that break the code. They are dropped, never retried, and
   reported as `fix-failed` (Warning; the message names the rule) on the
   issues they belonged to. Never write broken code.
4. Re-lint and repeat until no fix is left, for at most 10 passes.
5. If at least one fix was applied and the original file was formatted
   (formatting it gives the same text), format the result. That way we
   never reformat files the user doesn't format.
6. A file that already has syntax errors is not fixed.
7. `--diff` prints a unified diff (`a/<path>`, `b/<path>`) and doesn't
   write anything. Exit code 1 when a file would change, else 0.

Add to "CLI":
- `--fix`/`--fix-unsafe` write the files, print the report of the issues
  left, then `Fixed N problems in M files` (on stderr for machine formats).
  The exit code follows the issues left.
- `--stdin-filename <PATH>` reads the source from stdin; `PATH` finds the
  config and is shown in output. It can't be combined with file arguments.
  With `--fix`, the fixed source goes to stdout and the report to stderr.

## CHANGELOG lines
- Linter: new `lint --fix` and `--fix-unsafe` options apply automatic fixes, over several passes, and never write code that doesn't parse (a fix that would break the code is reported as `fix-failed`). Files that were formatted stay formatted.
- Linter: new `lint --diff` option prints the fixes as a unified diff without writing files.
- Linter: new `lint --stdin-filename <PATH>` option reads the source from stdin and uses PATH for the config and output; with `--fix` it prints the fixed source.
- Linter: `unnecessary-pass` and `no-else-return` now have safe fixes, and `unused-argument` has an unsafe fix (prefix the name with `_`).

## How to test manually
```sh
cargo test --test fix                     # engine, built-in fixtures, CLI
cp -r tests/e2e/sample_project /tmp/s && cd /tmp/s
gdscript-formatter lint --diff .; echo $?           # 3 hunks, exit 1
gdscript-formatter lint --fix .                     # "Fixed 3 problems in 2 files"
gdscript-formatter lint --fix .                     # "Fixed 0 problems in 0 files"
gdscript-formatter --check .                        # still formatted
printf 'func f(x):\n\tprint(1)\n\tpass\n' | gdscript-formatter lint --fix-unsafe --stdin-filename a.gd
```

## Known issues / follow-ups
- `no-else-return` reports an `elif` after an `if` that returns even when
  an earlier `elif` doesn't return; "use 'if' instead" is wrong there. The
  diagnostic has no fix in that case. Consider reporting only when every
  earlier branch returns.
- `unused-argument`'s fix doesn't check that `_name` is free (another
  parameter named `_name`); it's Unsafe.
- The `else` fix can leave a comment that was less indented than the body
  where it was; it stays valid GDScript.
- `--disable` ids are still validated only when a config loads (W2 merge
  note).
