# Handoff w1-a

## Summary
The linter pipeline and all 17 built-in rules now produce `Diagnostic` with
real start and end spans, and `help` text on every rule (naming rules suggest
the corrected name). `LintIssue` and `LintSeverity` are gone; the old text
and pretty output are byte-identical. `lint` has exit codes 0/1/2 and
`--max-warnings`, and `api::lint_source` works with built-in rules.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| A1 pipeline | done | `Rule` trait appends to `&mut Vec<Diagnostic>`; ignore comments filter once after all rules run. |
| A1 rules | done | All 17 ported. Spans on the name node for naming rules, the `pass` statement, the `elif`/`else` keyword, the overflowing part of a long line, etc. |
| gdextension | done | `lint_gdscript` returns the same Dictionary keys (`line`, `column`, `rule`, `severity`, `message`). `cargo build -p gdscript-formatter-gdextension` passes. |
| `api::lint_source` | done | Built-in rules, `config.rules` levels, `max_line_length`, `indent_size`. |
| A5 exit codes | done | 0 clean, 1 problems, 2 usage/I/O errors. `--max-warnings N`. |
| Tests | done | Span test for every rule, `lint_source` tests, exit-code and `--max-warnings` CLI tests (file and stdin). |

## Files changed
- `src/linter.rs`: pipeline on `Diagnostic`; new `run_builtin_rules`, `format_diagnostic_line`, `LintSummary`; `lint_files*` return `LintSummary`; pretty output keeps file order (was `HashMap` order).
- `src/linter/rules.rs`: new `Rule` trait signatures.
- `src/linter/rules/*.rs`: all 17 rules ported, with spans and `help`.
- `src/linter/lib.rs`: removed `get_line_column`; added `to_snake_case`, `to_pascal_case`, `to_constant_case`.
- `src/linter/api.rs`: `lint_source` body and tests (stub test updated).
- `src/linter/tests.rs`: `LintIssue::format` test replaced by a `format_diagnostic_line` test.
- `src/main.rs`: `run_linter(LintRunOptions) -> Result<LintSummary, String>`; exit codes; `--max-warnings` stub removed; `find_gdscript_files` returns an empty list instead of exiting (the formatter still prints the same error and exits 1).
- `gdextension/src/lib.rs`: uses `Diagnostic`.
- `tests/integration_tests.rs`: lint tests use `format_diagnostic_line`; new `builtin_rule_spans` test.
- `tests/lint/spans/all_rules.gd`, `tests/lint/spans/all_rules.txt`: span fixture (every rule, multi-byte line).
- `tests/cli_tests.rs`: `--max-warnings` removed from the stub test; 5 new exit-code tests.

## Public API added

```rust
// src/linter/rules.rs
pub trait Rule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] { &[] }
    fn check_source(&mut self, _source_code: &str, _diagnostics: &mut Vec<Diagnostic>) {}
    fn check_node(&mut self, _node: &Node, _source_code: &str, _diagnostics: &mut Vec<Diagnostic>) {}
    fn finalize(&mut self, _source_code: &str, _diagnostics: &mut Vec<Diagnostic>) {}
}

// src/linter.rs
pub fn format_diagnostic_line(file_path: &str, diagnostic: &Diagnostic) -> String;
    // "path:line:rule:severity: message", the default text output
pub fn run_builtin_rules(config: &LinterConfig, tree: &Tree, source_code: &str,
                         diagnostics: &mut Vec<Diagnostic>);
    // unfiltered, unsorted; GDScriptLinter::lint calls it, then ignores, then sorts

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LintSummary { pub error_count: usize, pub warning_count: usize, pub info_count: usize }
impl LintSummary {
    pub fn add(&mut self, diagnostics: &[Diagnostic]);
    pub fn exit_code(&self, max_warnings: Option<usize>) -> i32;
}

impl GDScriptLinter {
    pub fn lint(&mut self, source_code: &str, file_path: &str) -> Result<Vec<Diagnostic>, String>;
    pub fn lint_files(&mut self, files: &[PathBuf], do_pretty_print: bool) -> Result<LintSummary, String>;
    pub fn lint_files_with_editorconfig(&mut self, files: &[PathBuf], do_pretty_print: bool,
        max_line_length_override: Option<usize>) -> Result<LintSummary, String>;
}
pub fn lint_gdscript_with_config(source: &str, path: &str, config: &LinterConfig) -> Result<Vec<Diagnostic>, String>;
pub fn lint_gdscript(source: &str, path: &str) -> Result<Vec<Diagnostic>, String>;

// src/linter/lib.rs
pub fn to_snake_case(name: &str) -> String;     // "moveTowards" -> "move_towards", keeps leading "_"
pub fn to_pascal_case(name: &str) -> String;    // "bad_class_name" -> "BadClassName"
pub fn to_constant_case(name: &str) -> String;  // "maxSpeed" -> "MAX_SPEED"
```

Removed: `linter::LintIssue`, `linter::LintSeverity`, `linter::lib::get_line_column`.

CLI: `lint --max-warnings <N>` works. Exit codes: 0 when there are no errors
and at most N warnings (N defaults to 0, so any warning fails, as before);
1 otherwise; 2 for usage errors (invalid `--disable` names, no `.gd` files,
unreadable files, bad flag values). Info diagnostics never fail.

## Decisions
- **Rules write into `&mut Vec<Diagnostic>`** instead of returning a `Vec`,
  following the guidelines' output-parameter convention. Ignore comments are
  applied once with `retain` after every rule ran, instead of after each
  call. Same result, less code.
- **Kept `run_builtin_rules` separate from `GDScriptLinter::lint`** so the C1
  merge can run custom providers on the same tree (see Requests).
- **Spans** (the most precise node): naming rules use the name node;
  `function-argument-name`, `unused-argument` and `loop-variable-name` use
  the parameter's name node (not the whole `x: int = 1`); `no-else-return`
  uses the `elif`/`else` keyword; `comparison-with-itself` the whole binary
  expression; `standalone-expression` the expression; `unnecessary-pass` the
  `pass`; `private-access` the member name; `duplicated-load` the path
  string; `max-line-length` from the first character past the limit to the
  end of the line (tabs count as `indent_size`, like the formatter). Start
  positions equal the old `(line, column)` everywhere except
  `max-line-length`, whose old column was a display column (`max + 1`) and is
  now a byte column. The text output only prints the line, so it's unchanged.
- **`help`** on every rule. Naming rules suggest the converted name
  ("Rename it to 'bad_name'.") only when the suggestion passes the rule's own
  regex.
- **`load-variable-name`** (emitted by the `variable-name` rule for
  `var X = load(...)`) stays a separate diagnostic id, as before. It is not
  in `ALL_RULES`, so it can't be disabled on its own. Track B's
  `rules_meta.rs` and Track A's W2 config validation should know this id.
- **`lint_source` applies `config.rules` levels** (`off` disables, other
  levels set the severity) because it's tiny and makes `Config` meaningful.
  Options (`max-line-length.max`), overrides and `.editorconfig` are W2.
- **Exit code 2 for "no GDScript files found" and read errors in `lint`.**
  The formatter keeps exit 1 for these, unchanged.
- **Pretty output keeps the CLI's file order** (sorted) instead of
  `HashMap` order. Same format, now stable.

## Requests for the merge session
- **C1 hook point (W2):** in `GDScriptLinter::lint` (`src/linter.rs`), right
  after `run_builtin_rules(&self.config, &tree, source_code, &mut diagnostics);`,
  build a `FileContext { path, source: source_code, tree: &tree }` and append
  each provider's `check_file` output to `diagnostics`. Ignore filtering and
  sorting already happen after that line. W2-A may move this; read
  `w2-a.md` first.
- When B's `test-rules` (W2), `explain`/`init`/`--list-rules --format json`
  (W3) land, delete their cases from the stub tests in `tests/cli_tests.rs`
  (`unimplemented_subcommands_exit_with_code_2_and_name_the_command`,
  `list_rules_json_is_not_implemented_yet`), since B can't edit that file.

## Spec updates
- In "CLI", under the exit codes, add: "Without `--max-warnings`, any warning
  gives exit code 1. Info diagnostics never change the exit code."
- In "Shared contracts", after `Diagnostic`: "Built-in rules report on the
  most precise node: the name for naming rules, the keyword for
  `no-else-return`, the part past the limit for `max-line-length`."

## CHANGELOG lines
- Linter: every built-in rule now reports a precise start and end position and a `help` text with how to fix the issue (naming rules suggest the corrected name).
- Linter: `lint` exit codes are now 0 (no problems), 1 (problems found) and 2 (usage, config or file errors). Invalid `--disable` rule names and "no GDScript files found" now exit with 2 instead of 1.
- Linter: new `--max-warnings <N>` option: warnings only fail the run when there are more than N.
- Linter: `lint --pretty` now lists files in a stable, sorted order.

## How to test manually
```sh
cargo test --test integration_tests builtin_rule_spans   # spans of every rule
cargo test --lib linter::api                             # lint_source
cargo test --test cli_tests                              # exit codes, --max-warnings
cd tests/e2e/sample_project
cargo run -q -- lint .; echo $?                          # 16 issues, 1
cargo run -q -- lint --max-warnings 100 scripts/player.gd; echo $?   # 2 warnings, 0
cargo run -q -- lint --disable nope .; echo $?           # 2
cargo build -p gdscript-formatter-gdextension
```

## Known issues / follow-ups
- `Span::from_byte_range` scans from the start of the file, so a file with
  many long lines costs O(lines × size) in `max-line-length`. Fine for real
  scripts; revisit in W8 perf work if it shows up.
- The gdextension still exposes only `line`/`column`/`rule`/`severity`/
  `message`. W7-B adds spans, help and fixes.
