# Handoff w2-a

## Summary
Projects now configure the linter with `gdguard.toml`: upward discovery (or
`--config`), `[rules]` levels and options, `[custom_rules].paths`,
`[lint].exclude`/`baseline`, `[[overrides]]` with globs, full precedence,
and validation errors that name the file and key (exit 2). Output has three
reporters: `pretty` (snippet with carets, help, fixes, summary), `compact`
(the new non-TTY default) and `json` (schema v1, every spec field).

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| A2 discovery | done | Nearest `gdguard.toml` upward from each file; `--config` replaces discovery. Configs are cached per file during a run. |
| A2 parsing | done | Manual walk over the TOML table, so every unknown key gets an exact key path. |
| A2 precedence | done | CLI → overrides (later wins) → `[rules]` → `.editorconfig` → defaults. |
| A2 validation | done | Unknown keys, bad levels, bad types, bad globs, unknown built-in options, unknown rule ids (with "did you mean"). Custom ids come from `custom::load_providers(config).rules()`. |
| A2 `api::load_config_for` | done | |
| A2 per-file levels/options | done | `off` disables, other levels set severity; `max-line-length.max` sets the limit. `lint_source` uses the same code. |
| A3 pretty | done | |
| A3 compact | done | `file:line:col: severity [rule] message` |
| A3 json | done | Exactly the v1 fields, plus `summary.infos` (additive). |
| A3 default format | done | `--format` > `--pretty` > pretty on a TTY > compact. |
| Tests | done | 22 config tests (`tests/config/`), 9 reporter tests with snapshots (`tests/output/expected/`), 6 new CLI tests. |

## Files changed
- `src/linter/config.rs`: new. Discovery, parsing, validation, `CompiledConfig` (globs), per-file settings.
- `src/linter/output/mod.rs`: `FileDiagnostics`, `LintReport`, `ReportSummary` (with `exit_code`), `write_report`, `display_path`, `line_text_at`.
- `src/linter/output/{pretty,compact,json}.rs`, `src/linter/output/tests.rs`: new.
- `src/linter.rs`: removed `LintSummary` and the `lint_files*`/printing methods of `GDScriptLinter`; added `set_config`, `apply_rule_levels`, `lint_with_settings`, `LintRunOptions`, `lint_files`, `lint_stdin`.
- `src/linter/api.rs`: `load_config_for` and `lint_source` bodies (tests updated).
- `src/main.rs`: builds a `LintReport`, picks the format, prints it, exits with `ReportSummary::exit_code`. `--format` and `--config` stubs removed (github/sarif still stubbed).
- `src/cli.rs`: help text for `--format` default and `--pretty`.
- `tests/config/main.rs`: new test target.
- `tests/output/expected/{pretty,pretty_colored,compact,json}.txt`: reporter snapshots.
- `tests/cli_tests.rs`: stub cases for `--format`/`--config` replaced by `--format github`/`sarif`; new format and config tests.

## Public API added

```rust
// src/linter/config.rs
pub const CONFIG_FILE_NAME: &str = "gdguard.toml";
pub fn builtin_rule_ids() -> Vec<&'static str>;          // ALL_RULES names + "load-variable-name"
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliRuleOverrides { pub disabled_rules: HashSet<String>, pub max_line_length: Option<usize> }
#[derive(Debug, Clone)]
pub struct FileSettings {
    pub linter_config: LinterConfig,                     // built-in settings after precedence
    pub levels: BTreeMap<String, RuleLevel>,             // explicit levels, any rule id
    pub rule_options: BTreeMap<String, toml::Table>,     // merged options, any rule id
}
pub fn find_config_file(path: &Path) -> Option<PathBuf>;
pub fn load_config_file(path: &Path) -> Result<Config, ConfigError>;     // parse + key checks
pub fn parse_config(config_path: &Path, text: &str) -> Result<Config, ConfigError>;
pub fn validate_rule_ids(config: &Config) -> Result<(), ConfigError>;  // needs custom providers
pub fn load_config_for(path: &Path) -> Result<Config, ConfigError>;    // nearest file or defaults
pub fn load_and_validate(config_path: &Path) -> Result<Config, ConfigError>;
pub struct CompiledConfig { pub config: Config, /* compiled globs */ }
impl CompiledConfig {
    pub fn new(config: Config) -> Result<CompiledConfig, ConfigError>;
    pub fn is_excluded(&self, file_path: &Path) -> bool;
    pub fn settings_for_file(&self, file_path: &Path, cli: &CliRuleOverrides) -> FileSettings;
}

// src/linter/output/mod.rs
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiagnostics { pub path: String, pub source: String, pub diagnostics: Vec<Diagnostic> }
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LintReport { pub files: Vec<FileDiagnostics>, pub files_checked: usize }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReportSummary { pub errors: usize, pub warnings: usize, pub infos: usize, pub fixable: usize }
impl ReportSummary { pub fn exit_code(&self, max_warnings: Option<usize>) -> i32; }
impl LintReport { pub fn summary(&self) -> ReportSummary; }
pub fn write_report(format: OutputFormat, report: &LintReport, with_colors: bool, output: &mut String)
    -> Result<(), String>;                               // Err for github/sarif until W4
pub fn display_path(path: &Path) -> String;              // "/" separators, no leading "./"
pub fn line_text_at(source: &str, byte: usize) -> &str;
// src/linter/output/pretty.rs, compact.rs, json.rs
pub fn write_pretty(report: &LintReport, with_colors: bool, output: &mut String);
pub fn write_compact(report: &LintReport, output: &mut String);
pub fn write_json(report: &LintReport, output: &mut String) -> Result<(), String>;
pub const JSON_SCHEMA_VERSION: u32 = 1;

// src/linter.rs
impl GDScriptLinter { pub fn set_config(&mut self, config: LinterConfig); }
pub fn apply_rule_levels(levels: &BTreeMap<String, RuleLevel>, diagnostics: &mut Vec<Diagnostic>);
pub fn lint_with_settings(linter: &mut GDScriptLinter, path: &Path, source_code: &str,
    settings: &FileSettings) -> Result<Vec<Diagnostic>, String>;
#[derive(Debug, Clone, Default)]
pub struct LintRunOptions { pub config_path: Option<PathBuf>, pub cli: CliRuleOverrides }
pub fn lint_files(files: &[PathBuf], options: &LintRunOptions) -> Result<LintReport, String>;
pub fn lint_stdin(source_code: String, config_lookup_path: &Path, display_name: &str,
    options: &LintRunOptions) -> Result<LintReport, String>;
```

Removed (W1-A API): `LintSummary`, `GDScriptLinter::lint_files`,
`GDScriptLinter::lint_files_with_editorconfig`.

CLI: `lint --format pretty|compact|json`, `lint --config <PATH>`. JSON
schema: the spec's v1 fields plus `summary.infos`.

## Decisions
- **Default output is now `compact` when stdout isn't a terminal** (spec).
  The old `file:line:rule:severity: message` text is no longer printed by the
  CLI. `format_diagnostic_line` still exists for the `tests/lint/` fixtures.
  **This breaks the legacy Godot addon's parser** (see Requests).
- **`[rules]` table without `level`** (`max-line-length = { max = 120 }`)
  keeps the default severity.
- **Options are only checked for built-in rules.** `max-line-length`
  accepts `max` (positive integer); the other built-ins accept none. Custom
  rules accept any options (for JS rules later).
- **`Config.max_line_length` / `indent_size` are not read from TOML.** The
  TOML way is `max-line-length = { max = N }` (also per override). The
  `Config` fields stay for API callers and win over `.editorconfig`.
  `indent_size` comes from `.editorconfig` only.
- **Globs:** relative to the config file's folder; `*` doesn't cross `/`,
  `**` does. A pattern matches a file or any of its parent folders, so
  `exclude = ["addons"]` excludes everything under `addons/`. Overrides use
  the same matching.
- **Exclude applies to explicit file arguments too**, like `--exclude`
  already did. `lint_source` doesn't check excludes: the caller asked for
  that buffer.
- **Paths in config errors are absolute** (relative config paths are made
  absolute when loaded) so messages are unambiguous.
- **Unknown rule ids are checked after loading custom providers.** Parsing
  accepts any id; `validate_rule_ids` then rejects ids that are neither
  built-in (including `load-variable-name`) nor provided by
  `load_providers(config)`. "Did you mean" uses edit distance ≤ max(2, len/3).
- **`summary.fixable` counts issues with at least one `Safe` fix** (what
  `--fix` applies). The pretty summary says "N fixable with --fix".
- **JSON `origin`** is `builtin` for `builtin_rule_ids()`, `custom`
  otherwise. Fix edits carry only `byte` in their range, like the spec.
- **Pretty layout:** ` line:col severity [rule] message`, a gutter with the
  source line (tabs shown as 4 spaces), carets under the span (to the end of
  the first line for multi-line spans), `= help:` and `= fix (safety):`
  lines, a blank line between diagnostics, and
  `Found N problems (E errors, W warnings[, I infos]) in F files. X fixable with --fix.`
  Nothing is printed when there are no problems.
- **Display paths** drop a leading `./` and use `/`.
- **`--config` that isn't a file** is a usage error (exit 2) before linting.

## Requests for the merge session
1. **Regenerate the e2e snapshots** (they're yours): the default output is
   now compact and pretty has the new layout, so
   `lint_sample_project_default_output` and `lint_sample_project_pretty_output`
   fail on this branch. Same 16 issues. Run
   `UPDATE_SNAPSHOTS=1 cargo test --test e2e_tests` after merging and review.
2. **Fix the legacy Godot addon** (B owns `addons/` from W2, so I couldn't):
   in `addons/GDQuest_GDScript_formatter/plugin.gd`, `lint_code()` must
   pass `--format compact` (so TTY detection never matters), and
   `parse_lint_issue()` must use the compact format:
   ```gdscript
   # Expected format: file:line:col: severity [rule] message
   regex.compile(r"^(.*\.gd):(\d+):(\d+): (\w+) \[([^\]]+)\] (.*)$")
   ...
   return {
       "line": int(result.get_string(2)) - 1,
       "rule": result.get_string(5),
       "severity": result.get_string(4),
       "message": result.get_string(6).strip_edges(),
   }
   ```
   The standalone addon uses the gdextension and is unaffected.
3. **C1:** custom providers run per config, on the tree the built-in rules
   use. Suggested wiring:
   - give `GDScriptLinter` a `providers: Vec<Box<dyn RuleProvider>>` field
     (empty by default) and a `set_providers` method; in
     `GDScriptLinter::lint`, right after
     `run_builtin_rules(&self.config, &tree, source_code, &mut diagnostics);`,
     call `check_file(&FileContext { path, source: source_code, tree: &tree })`
     on each provider and append. `lint` takes `file_path: &str`; build the
     `Path` from it.
   - load providers once per config: in `ConfigCache::config_for`
     (`src/linter.rs`), call `custom::load_providers(&config)` next to
     `CompiledConfig::new` and keep them with the compiled config, then hand
     them to the linter before `lint_with_settings` (a `HashMap` of
     providers keyed like `configs`).
   - `api::lint_source` should do the same for its single call.
   - `config::validate_rule_ids` already calls `load_providers`, so each
     config loads rules twice; fine for now, or return the providers from
     `load_and_validate` to reuse them.
   - Levels for custom ids already work: `apply_rule_levels` runs on all
     diagnostics after the providers.
4. `--disable` still validates names against built-in rules only
   (`rule_config::validate_rule_names`). After C1, consider accepting custom
   ids there too.

## Spec updates
- "Configuration": add after the TOML example:
  > A rule's table form may omit `level` to keep the default severity.
  > Built-in rules accept these options: `max-line-length.max` (a positive
  > integer). Custom rules accept any options. Globs in `exclude` and
  > `overrides.files` are relative to the config file's folder; `*` doesn't
  > cross `/`, `**` does, and a pattern also matches every file under a
  > matching folder (`exclude = ["addons"]`). `exclude` also applies to
  > files named on the command line. `--config <PATH>` uses that file for
  > every linted file instead of searching. The line length comes from
  > `max-line-length = { max = N }` (also inside an override); there is no
  > separate `max_line_length` key.
- "CLI": `--format` default: "pretty on a TTY, compact otherwise; `--pretty`
  is an alias of `--format pretty`". Compact format:
  `file:line:col: severity [rule] message`.
- "JSON output": add `"infos": 0` to `summary`, and: "`summary.files` counts
  every linted file, `summary.fixable` counts issues with a `safe` fix,
  `origin` is `builtin` or `custom`, `snippet` is the full first line of the
  range, and `help` is `null` when the rule gives none."

## CHANGELOG lines
- Linter: projects can configure the linter with a `gdguard.toml` file (rule levels and options, excluded files, per-folder overrides). The linter uses the nearest one above each file, or the file given with `--config`. Unknown keys and rule ids are reported with the file and key.
- Linter: new `--format` option with `pretty` (source snippet with carets, help and fix hints, and a summary), `compact` and `json` (stable schema for tools and AI agents).
- Linter: the default output is now `pretty` in a terminal and `compact` (`file:line:col: severity [rule] message`) otherwise. This replaces the old `file:line:rule:severity: message` output; scripts that parse it should use `--format compact` or `--format json`.

## How to test manually
```sh
cargo test --test config                         # discovery, precedence, errors
cargo test --lib linter::output                  # reporter snapshots, JSON fields
cargo test --test cli_tests                      # formats, --config, exclude
cd tests/e2e/sample_project
cargo run -q -- lint .                           # compact
cargo run -q -- lint --format pretty .           # snippets + summary
cargo run -q -- lint --format json scripts/player.gd
printf '[rules]\nunused-argumnet = "off"\n' > /tmp/bad.toml
cargo run -q -- lint --config /tmp/bad.toml .; echo $?   # did you mean, exit 2
```

## Known issues / follow-ups
- `tests/e2e/` snapshots fail on this branch until the merge regenerates
  them (request 1).
- Pretty counts every character as one column (emoji and CJK are two wide
  in most terminals), so carets can be short on those lines.
- `.editorconfig` lookups happen per file (as before); with many files this
  is the main config cost. W4's cache/perf work can memoize it per folder.
