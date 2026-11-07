# Handoff w4-a

## Summary
All four W4 tasks for A are done:

- ignore comments v2 (file scope, `-- reason`, and the `unused-ignore` warning);
- `--baseline` and `--update-baseline`;
- the `github` and `sarif` reporters;
- parallel linting with a lint cache in `.gdguard/cache/`.

Only the `lsp` stub still prints "not implemented". With a warm cache, the
1068-file corpus lints in about 32 ms, far under the 1 s target.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| A7 ignores | done | `src/linter/ignore_patterns.rs` was rewritten. It applies to built-in and custom diagnostics, after rule levels. |
| A8 baseline | done | New `src/linter/baseline.rs`, CLI in `src/main.rs`, and `tests/baseline/` (8 tests, including moved lines). |
| A9 reporters | done | `src/linter/output/{github,sarif}.rs`, with snapshots in `tests/output/expected/`. |
| Parallel + cache | done | `lint_files`/`fix_files` in `src/linter.rs`, `src/linter/cache.rs`, `--no-cache`, and `tests/cache/` (7 tests). |

## Files changed
- `src/linter/ignore_patterns.rs`:
  - comments are read from tree-sitter `comment` nodes, so a directive inside a string doesn't count;
  - new `apply_ignores`, which also reports `unused-ignore`.
- `src/linter.rs`:
  - `GDScriptLinter::lint_with_levels` runs levels, then ignores, then `unused-ignore` (levels apply to it too), then sorts;
  - `ConfigCache` records custom rule metadata;
  - `lint_files`/`fix_files` run in parallel (`prepare_jobs`, `run_jobs`, `run_worker`, `run_job`);
  - `LintRunOptions.cache_path` is new.
- `src/linter/config.rs`: `builtin_rule_ids()` now includes `unused-ignore`.
- `src/linter/baseline.rs`: new.
- `src/linter/cache.rs`: filled in. It was a stub.
- `src/linter/output/`:
  - `LintReport` gains `baseline_hidden` and `rule_metas`;
  - new `utf16_column`;
  - `pretty` and `json` show the hidden count;
  - new `github.rs` and `sarif.rs`.
- `src/main.rs`:
  - wires up the baseline, `--update-baseline` and the cache path;
  - removes the github/sarif/baseline stubs.
- `src/cli.rs`: `--no-cache` flag and help line.
- Tests:
  - `tests/lint/{input,expected}/ignore_gdguard.*`;
  - 2 tests in `tests/config/main.rs`;
  - `tests/baseline/`;
  - `tests/cache/`;
  - `tests/cli_tests.rs`: the "not implemented" test became `github_and_sarif_formats_report_issues`;
  - output snapshots.

## Public API added
```rust
// src/linter/ignore_patterns.rs
pub const UNUSED_IGNORE_RULE_ID: &str = "unused-ignore";
pub enum IgnoreScope { File, Line(usize) }
pub struct IgnoreComment { pub scope, pub directive: String, pub rules: Vec<String>,
                           pub reason: Option<String>, pub span: Span }
pub fn parse_directive(comment_text: &str) -> Option<ParsedDirective>;
pub fn parse_ignore_comments(source: &str, tree: &Tree) -> Vec<IgnoreComment>;
pub fn apply_ignores(source, comments, report_unused: bool, disabled_rules: &HashSet<String>,
                     diagnostics: &mut Vec<Diagnostic>, unused: &mut Vec<Diagnostic>);

// src/linter.rs
impl GDScriptLinter { pub fn lint_with_levels(&mut self, source, path, providers, levels) -> Result<Vec<Diagnostic>, String>; }
pub struct LintRunOptions { pub config_path, pub cli, pub cache_path: Option<PathBuf> }  // + cache_path

// src/linter/baseline.rs
pub const BASELINE_VERSION: u32 = 1;
pub const DEFAULT_BASELINE_PATH: &str = ".gdguard/baseline.json";
pub struct BaselineEntry { pub file, pub rule, pub fingerprint, pub message }
pub struct Baseline;  // from_entries, len, is_empty
pub struct BaselineTarget { pub path: PathBuf, pub root: PathBuf, pub is_explicit: bool }
pub fn resolve_baseline(cli_path, config_path, cwd, for_update) -> Result<Option<BaselineTarget>, String>;
pub fn load_target / load_baseline / write_baseline / entries_for_report / apply_baseline / fingerprint / baseline_file_key;

// src/linter/cache.rs
pub const CACHE_VERSION: u32 = 1;
pub const CACHE_PATH: &str = ".gdguard/cache/lint.json";
pub fn stable_hash / file_hash / settings_hash / rules_hash / file_key;
pub struct CacheKey { pub file_hash, pub settings_hash, pub rules_hash }
pub struct LintCache;  // load, len, is_empty, get, insert, save

// src/linter/output/
pub struct LintReport { ..., pub baseline_hidden: usize, pub rule_metas: Vec<RuleMeta> }
pub fn utf16_column(source: &str, byte: usize) -> usize;
pub fn write_github(report, output); pub fn write_sarif(report, output) -> Result<(), String>;
```

New diagnostic id: `unused-ignore`:
- severity: Warning;
- has a Safe fix;
- message: ``` `<directive>` doesn't suppress any issue ```, or ``` `<directive>` for `<ids>` doesn't suppress any issue ``` when only some ids are unused;
- help: "Remove the comment or the rule ids that no longer apply, so ignores don't hide future issues."

## Decisions
- **Ignore directives:**
  - The forms are `gdguard-ignore`, `gdguard-ignore-line`, `gdguard-ignore-next-line` and `gdguard-ignore-file`, plus the same `gdlint-` forms.
  - A directive must be the first thing in the comment.
  - Rule ids are separated by commas and/or spaces.
  - A reason comes after ` -- ` (whitespace before `--`).
  - With no ids, the directive covers every rule.
  - `gdguard-ignore` on a line with code applies to that line. On a line of its own, it applies to the next line.
  - `-file` can be anywhere in the file.
- **`unused-ignore`:**
  - reported once per comment that suppresses nothing, or once per comment for the ids it lists that suppress nothing;
  - the fix removes the comment (the whole line when the comment is alone on it), or rewrites the comment with only the ids that are still used;
  - ids of rules that are disabled (`--disable`, `= "off"`) are never reported, because they can't fire;
  - `# gdguard-ignore-file unused-ignore` turns it off for a file, and `unused-ignore = "off"` turns it off for the project.
- **Baseline:**
  - The fingerprint is FNV-1a over the rule id, the file path (relative to the folder of `gdguard.toml`, or the current folder when there is none), the trimmed line text, and the index among identical (rule, file, text) triples. Moving lines keeps an issue hidden. Editing the line or adding a duplicate line shows the issue.
  - `--update-baseline` writes `--baseline PATH`, else `[lint].baseline`, else `.gdguard/baseline.json`. Entries are sorted, so the output is stable.
  - A missing file named by `[lint].baseline` counts as an empty baseline. A missing `--baseline` file is an error (exit 2).
  - `--update-baseline` with `--fix`, `--fix-unsafe` or `--diff` is a usage error.
- **github:**
  - format: `::<error|warning|notice> file=…,line=…,col=…,endLine=…,endColumn=…,title=<rule>::<message>`;
  - columns are in UTF-16 units, as GitHub expects;
  - info maps to `notice`;
  - `%`, CR, LF, `:` and `,` are escaped as GitHub documents.
- **SARIF 2.1.0:**
  - `tool.driver.rules` lists only the rules that appear in results, sorted by id;
  - built-in metadata comes from `rules_meta`, custom metadata from the loaded providers, with a minimal entry as fallback;
  - `columnKind` is `utf16CodeUnits`;
  - URIs are the displayed paths (relative when the inputs are), percent-encoded;
  - fixes are not exported yet.
- **Parallel linting:**
  - Configs are loaded and checked on the main thread, which keeps errors deterministic.
  - Workers then pull jobs from an `AtomicUsize`, using `available_parallelism()` threads.
  - Each worker has its own `GDScriptLinter` and loads its own custom rule providers, because they aren't `Send`. This means custom YAML/JS rules are compiled once per worker and config.
  - Results go back in input order (file discovery is sorted). The first error in input order wins.
  - `lint_stdin`/`fix_stdin` stay sequential.
- **Cache:**
  - Only `lint` uses the cache, not `--fix`/`--diff`, and only when a `gdguard.toml` is found: `--config`, else the one found from the current folder, else the one of the first file.
  - The cache file is `.gdguard/cache/lint.json` next to that config, and it holds files from every config in the run, keyed by absolute path.
  - An entry is reused when four things are unchanged: the file content hash, the resolved settings for the file (levels, rule options, line length, indent), the custom rule files (paths and bytes), and the tool version. A version change drops the whole cache.
  - Writes go to a temporary file that is then renamed.
  - Entries for deleted files are pruned on save.
  - The folder gets a `.gitignore` containing `*`.
  - An unreadable cache is ignored. A cache that can't be written is silently skipped.
  - `--no-cache` skips both reading and writing.
  - The baseline is applied after the cache, so cached results are the full results.

## Timings
Release build on a 12-core machine, average of 20 runs of `lint --format compact`:

| Input | Before (sequential) | After, no cache | After, warm cache |
|-------|---------------------|-----------------|-------------------|
| `tests/e2e/sample_project` (5 files) | 2.6 ms | 2.7 ms | 2.0 ms |
| `tests/input` | 8.2 ms | 4.1 ms | (no config) |
| corpus, 1068 files (6 copies of the test inputs) | 116 ms | 47 ms | 32 ms |

On the corpus, a cold run, a warm run and a `--no-cache` run give
byte-identical output.

## Requests for the merge session
1. **Regenerate the e2e snapshot** `tests/e2e/.../lint.json`. JSON now has
   `summary.baseline_hidden: 0`. This is the only test failing on `w4-a`
   (`lint_sample_project_json_output`).
2. **Ignore `tests/e2e/sample_project/.gdguard/cache/`.** e2e runs now
   create it, because the sample project has a `gdguard.toml`. Its
   `.gitignore` keeps it out of git, so either pass `--no-cache` in
   `tests/e2e_tests.rs` or leave it as is. The cache can't change results.
3. **B's `rules_meta.rs`:** add metadata for `unused-ignore` (an engine
   diagnostic, Warning, fixable) so `explain unused-ignore`, `--list-rules`
   and SARIF have it. Bump the `BUILTIN_RULES` count from 19 to 20 in B's
   test.
4. **e2e (C3):** on a copy of the sample project:
   - a `--update-baseline` then `lint` round trip should give exit 0 and "N issues hidden by baseline.";
   - snapshots of `--format github` and `--format sarif` (replace the version number in SARIF).
5. **`docs/configuration.md`/README (B from W4):** document the ignore
   comments, `--baseline`/`--update-baseline`, `[lint].baseline`,
   `--format github|sarif`, `--no-cache`, and committing
   `.gdguard/baseline.json` but not `.gdguard/cache/`.

## Spec updates
- **Ignore comments:**
  - Replace the section with the directive rules under Decisions above.
  - Add that `unused-ignore` is a built-in engine diagnostic (Warning, Safe fix), never reported for disabled rules.
- **Baseline:**
  - Fingerprint = FNV-1a(rule, path relative to the config folder, trimmed line text, occurrence index).
  - Default path is `.gdguard/baseline.json`, and `[lint].baseline` is used automatically.
  - `pretty` shows "N issues hidden by baseline." and JSON adds `summary.baseline_hidden`. This is an additive change to json v1.
- **Reporters:**
  - github: workflow commands with UTF-16 columns.
  - sarif: 2.1.0; only the rules found; `utf16CodeUnits`.
- **Performance and caching:**
  - Workers keep the input order.
  - The cache only exists with a config and only for `lint`.
  - The key is file, settings, custom rule files and tool version.
  - New `--no-cache` flag.
  - Warm 1068 files take 32 ms.

## CHANGELOG lines
- Linter: ignore comments now support `gdguard-ignore-file`, several rule ids, and a reason after ` -- `. They work for custom rules too and are no longer matched inside strings.
- Linter: new `unused-ignore` warning, with a safe fix, for ignore comments that suppress nothing.
- Linter: new `lint --baseline <PATH>` and `--update-baseline` options hide existing issues so only new ones are reported. `[lint].baseline` in `gdguard.toml` is used automatically.
- Linter: new `--format github` (GitHub Actions annotations) and `--format sarif` (SARIF 2.1.0 for code scanning).
- Linter: files are linted in parallel, and results are cached in `.gdguard/cache/` when the project has a `gdguard.toml`. Use `--no-cache` to turn the cache off.

## How to test manually
```sh
cargo test --test baseline --test cache --test config
cp -r tests/e2e/sample_project /tmp/s && cd /tmp/s
gdscript-formatter lint --update-baseline .        # "Wrote N issues to .gdguard/baseline.json"
gdscript-formatter lint --format pretty .          # "No problems found. N issues hidden by baseline."
gdscript-formatter lint --no-cache --format github --baseline /dev/null . 2>&1 | head
gdscript-formatter lint --format sarif . | head -30
ls .gdguard/cache                                  # .gitignore lint.json
```

## Known issues / follow-ups
- Custom rule providers are compiled once per worker thread. With many JS
  rules and many cores, a cold run pays that cost up to N times. If it
  shows up in profiles, share the compiled YAML rules (they are `Send`) and
  keep only JS per thread.
- `--fix` doesn't use the cache. It could skip files whose cached result
  has no fixable diagnostic.
- SARIF doesn't include `fixes` yet.
- `baseline` and `cache` hash with FNV-1a 64, which is fine for change
  detection but is not cryptographic.
