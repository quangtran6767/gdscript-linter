# Linter specification

This document is the source of truth for the linter in this fork. It describes
what the linter does, the shared data contracts every part of the code relies
on, and the file formats users and AI agents see. The work split
lives in `docs/implementation_plan.md`.

Tool name: **`gdguard`** (decided in W0). The binary, the config file
`gdguard.toml`, the rules folder `.gdguard/` and the ignore prefix
`gdguard-ignore` all use it. The Cargo package/crate and primary executable
are `gdguard`; `gdscript-formatter` remains a legacy executable alias with
unchanged formatting behavior. gdtoolkit's Python linter keeps its `gdlint` name.

The primary product is a CLI for coding agents: apply safe fixes, inspect
remaining actionable diagnostics, repair code and re-lint. IDE and Godot
integrations are optional compatibility features.

## Goals

1. Projects can add their own rules without recompiling: YAML syntax patterns and
   sandboxed JavaScript visitors.
2. Issues can be fixed automatically when that's safe, and every remaining
   issue carries enough information for a person or an AI agent to fix it.
3. Output is stable and machine-readable (JSON, SARIF, GitHub annotations) so
   the linter fits into CI and agent loops.
4. Existing projects can adopt it gradually (baseline file, per-folder
   overrides).

Non-goals:

- Type inference. Rules see declared types (`var x: Node`), never inferred
  ones.
- Replacing Godot's own static checker warnings.

## Architecture

```
            ┌──────────────┐
 files ───▶ │ config load  │  gdguard.toml + .editorconfig + CLI flags
            └──────┬───────┘
                   ▼
            ┌──────────────┐   built-in rules (Rust, src/linter/rules/)
 source ──▶ │ parse + run  │◀─ custom rules  (YAML, src/linter/custom/)
            │  providers   │   scripted rules (JS)
            └──────┬───────┘
                   ▼  Vec<Diagnostic>
            ┌──────────────┐
            │ ignore filter│  inline comments, overrides, baseline
            └──────┬───────┘
          ┌────────┴─────────┐
          ▼                  ▼
   ┌─────────────┐    ┌─────────────┐
   │ fix engine  │    │  reporters  │  pretty, compact, json, github, sarif
   └─────────────┘    └─────────────┘
```

Every rule source produces the same `Diagnostic` type. Everything after the
"parse + run" step works the same way whether a diagnostic came from a
built-in rule or a custom rule.

## Shared contracts (`src/linter/diagnostic.rs`)

This module is **frozen after W0**. Changes must be additive (new optional
fields). The project now has one maintainer; preserve compatibility when
implementing the roadmap contracts.

### Positions

- `line` is 1-based.
- `column` is 1-based and counted in **UTF-8 bytes**. This matches tree-sitter
  and the existing linter output.
- `start_byte` and `end_byte` are byte offsets into the file. They are the
  authoritative position. The end is exclusive.
- Reporters that need other units (for example SARIF or LSP UTF-16 columns)
  convert at the edge.

```rust
pub struct Position { pub line: usize, pub column: usize }

pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start: Position,
    pub end: Position,
}

pub enum Severity { Error, Warning, Info }

pub enum FixSafety {
    /// Never changes behavior. Applied by `--fix`.
    Safe,
    /// Probably keeps behavior. Applied only with `--fix-unsafe`.
    Unsafe,
    /// Never applied automatically. Shown to people and agents.
    Suggestion,
}

pub struct Edit { pub span: Span, pub replacement: String }

pub struct Fix {
    pub description: String,
    pub safety: FixSafety,
    /// Edits must not overlap each other.
    pub edits: Vec<Edit>,
}

pub struct Diagnostic {
    pub rule: String,          // "prefer-is-empty", built-in or custom id
    pub severity: Severity,
    pub message: String,       // what is wrong, one sentence
    pub help: Option<String>,  // how to fix it, one or two sentences
    pub span: Span,
    pub fixes: Vec<Fix>,       // usually zero or one
}
// Built-in rules report on the most precise node: the name for naming rules,
// the keyword for `no-else-return`, the part past the limit for
// `max-line-length`.

pub struct RuleMeta {
    pub id: String,
    pub description: String,     // one line, used by --list-rules
    pub rationale: String,       // why the rule exists, used by `explain`
    pub bad_example: String,
    pub good_example: String,
    pub default_severity: Severity,
    pub fixable: bool,
    pub origin: RuleOrigin,      // BuiltIn or Custom { path }
}

/// Everything a rule source needs to check one file.
pub struct FileContext<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub tree: &'a tree_sitter::Tree,
    pub semantic: Option<&'a SemanticModel>,
    pub project: Option<&'a ProjectIndex>,
}

/// Implemented by the custom YAML and JS rule engines. The
/// built-in rules keep their existing `Rule` trait and visitor.
pub trait RuleProvider {
    fn rules(&self) -> &[RuleMeta];
    fn check_file(&mut self, context: &FileContext) -> Vec<Diagnostic>;
}

/// Single-pass edit applier shared by the fix engine and the rule tester.
/// Edits are sorted by start_byte. Overlapping edits return an error.
pub fn apply_edits(source: &str, edits: &[Edit]) -> Result<String, String>;
```

`apply_edits` details: edits are sorted by `(start_byte, end_byte)`, so an
insertion at the start of a replaced range goes first, and edits with the same
range keep their input order. Two edits overlap when one starts before the
previous one ends; touching edits are fine. Ranges past the end of the source
or not on UTF-8 character boundaries are errors too.

Helpers added in W0 (additive):

```rust
impl Position { pub fn from_byte(source: &str, byte: usize) -> Position; }
impl Span {
    pub fn from_node(node: &tree_sitter::Node) -> Span;
    pub fn from_byte_range(source: &str, start_byte: usize, end_byte: usize) -> Span;
}
impl Severity  { pub fn as_str(self) -> &'static str; pub fn from_name(name: &str) -> Option<Severity>; }
impl FixSafety { pub fn as_str(self) -> &'static str; pub fn from_name(name: &str) -> Option<FixSafety>; }
```

All contract types derive `Debug`, `Clone` and `PartialEq` (`Position`, `Span`,
`Severity` and `FixSafety` are also `Copy`), except `FileContext`, which only
borrows.

### Library API (`src/linter/api.rs`)

The CLI, the LSP server and the Godot gdextension all go through this API, so
none of them depend on CLI internals. Signatures are frozen in W0; the bodies
are filled in by Track A.

```rust
/// Loads the nearest gdguard.toml for `path` (or defaults) plus custom rules.
pub fn load_config_for(path: &Path) -> Result<Config, ConfigError>;

pub struct LintResult { pub diagnostics: Vec<Diagnostic> }

/// Lints one in-memory buffer. `path` is used for config overrides and output.
pub fn lint_source(path: &Path, source: &str, config: &Config) -> Result<LintResult, String>;

pub struct FixResult { pub output: String, pub applied: usize, pub remaining: Vec<Diagnostic> }

/// Runs the fix engine on one buffer without touching the disk.
pub fn fix_source(path: &Path, source: &str, config: &Config, unsafe_fixes: bool)
    -> Result<FixResult, String>;
```

The config types. Every field is `pub` and `Config` derives `Default`, so
tests build it by hand (`Config { custom_rule_paths: vec![dir], ..Config::default() }`).
Maps are `BTreeMap` so iteration order and the config hash are stable.

```rust
pub enum RuleLevel { Off, Info, Warning, Error }   // as_str() / from_name()

pub struct RuleSetting {
    pub level: Option<RuleLevel>,   // None keeps the rule's default severity
    pub options: toml::Table,       // every key of the table form except `level`
}

pub struct ConfigOverride {
    pub files: Vec<String>,                       // globs, relative to the config folder
    pub rules: BTreeMap<String, RuleSetting>,
}

pub struct Config {
    pub config_path: Option<PathBuf>,             // None: no gdguard.toml found
    pub rules: BTreeMap<String, RuleSetting>,     // [rules], built-in and custom ids
    pub custom_rule_paths: Vec<PathBuf>,          // [custom_rules].paths, resolved
    pub exclude: Vec<String>,                     // [lint].exclude globs
    pub baseline: Option<PathBuf>,                // [lint].baseline, resolved
    pub overrides: Vec<ConfigOverride>,           // [[overrides]], later entries win
    pub max_line_length: Option<usize>,           // None: .editorconfig, then 100
    pub indent_size: Option<usize>,               // None: .editorconfig, then 4
    pub fix_format: bool,                         // [fix].format (W11, additive)
    pub fix_unsafe_all: bool,                     // [fix].unsafe = true (W11)
    pub fix_unsafe_rules: Vec<String>,            // [fix].unsafe = [ids] (W11)
}

/// Displays as "<path>: `<key>`: <message>", leaving out the parts that are None.
pub struct ConfigError { pub path: Option<PathBuf>, pub key: Option<String>, pub message: String }
```

Entry points owned by Track B, called by the CLI (frozen in W0 so B never
edits `src/cli.rs` or `src/main.rs`). Each returns the process exit code.

```rust
// src/linter/custom/mod.rs
pub fn load_providers(config: &Config) -> Result<Vec<Box<dyn RuleProvider>>, String>;
// src/linter/explain.rs
pub struct ExplainArgs { pub rule: String }
pub enum ListRulesFormat { Text, Json }
pub fn run(args: &ExplainArgs) -> i32;
pub fn list_rules(format: ListRulesFormat) -> i32;
// src/linter/rule_tester.rs
pub struct TestRulesArgs { pub rule_dir: Option<PathBuf> }
pub fn run(args: &TestRulesArgs) -> i32;
// src/linter/init.rs
pub struct InitArgs { pub force: bool }
pub fn run(args: &InitArgs) -> i32;
```

`lint --format` parses into `output::OutputFormat { Pretty, Compact, Json,
Github, Sarif, Agent }` (`Agent` added in W11) (`src/linter/output/mod.rs`, Track A).

## Configuration (`gdguard.toml`)

The linter finds the config by searching upward from each linted file and
uses the nearest one.

Precedence, highest first: CLI flags, then `gdguard.toml` (overrides before
`[rules]`), then `.editorconfig` (for `max_line_length` only), then defaults.

```toml
# Rule levels: "off" | "info" | "warning" | "error"
[rules]
unused-argument = "error"
max-line-length = { level = "warning", max = 120 }
prefer-is-empty = "error"            # custom rules use the same table

[custom_rules]
paths = [".gdguard/rules"]           # folders of *.yml rule files

[lint]
exclude = ["addons/**", ".godot/**"]
baseline = ".gdguard/baseline.json"  # optional

[fix]
format = true                        # optional, default false (W11)
unsafe = ["prefer-squared-distance"] # optional: true, false or rule ids (W11)

[[overrides]]
files = ["tests/**"]
rules = { private-access = "off" }
```

Unknown rule ids and unknown keys are errors (exit code 2) with a message that
names the file and the key, so typos never fail silently.

A rule's table form may omit `level` to keep the default severity. Built-in
rules accept these options: `max-line-length.max` (a positive integer).
Custom rules accept any options. Globs in `exclude` and `overrides.files`
are relative to the config file's folder; `*` doesn't cross `/`, `**` does,
and a pattern also matches every file under a matching folder
(`exclude = ["addons"]`). `exclude` also applies to files named on the
command line. `--config <PATH>` uses that file for every linted file instead
of searching. The line length comes from `max-line-length = { max = N }`
(also inside an override); there is no separate `max_line_length` key.

Custom rules come only from `[custom_rules].paths`; without a config, no
custom rule runs. They load once per config, run on the same parsed tree
after the built-in rules, and follow the same rules: `[rules]` and override
levels (including `off`), inline ignore comments, and `--disable`, which
accepts built-in and custom ids. A rule file error stops the run with exit
code 2.

## Inline ignore comments

Directives must begin a real tree-sitter comment, not occur in strings.
All forms accept comma/space-separated rule IDs, or no IDs for every rule,
and a reason after ` -- `. The legacy `gdlint-` prefixes are equivalent.

- `gdguard-ignore-file`: whole file, regardless of the comment's location.
- `gdguard-ignore-line`: the comment's line.
- `gdguard-ignore-next-line`: the following line.
- `gdguard-ignore`: the same line after code, otherwise the following line.

```gdscript
# gdguard-ignore-file max-line-length -- generated text
var badName = 1 # gdguard-ignore variable-name -- external field name
# gdguard-ignore-next-line unused-argument -- required callback signature
func callback(event):
    pass
```

Ignores apply to built-in and custom diagnostics after levels. An unused
comment or listed ID produces `unused-ignore` (Warning, Safe fix), except
for disabled rules. Its fix removes the comment or only unused IDs.
`unused-ignore = "off"` or `gdguard-ignore-file unused-ignore` disables it.

## Custom YAML rules

One rule per file in `.gdguard/rules/*.yml`. The schema is a subset of
ast-grep's rule format, so existing ast-grep knowledge (human or AI) carries
over.

```yaml
id: prefer-is-empty                 # required, kebab-case, unique
severity: warning                   # default level, config can override
message: "Use `$X.is_empty()` instead of `len($X) == 0`"  # may use metavariables
help: "is_empty() states the intent and works on Array, Dictionary and String."
rationale: |                        # shown by `explain`
  Comparing len() to zero hides the intent ...
rule:                               # ast-grep rule object
  pattern: len($X) == 0
fix: $X.is_empty()                  # optional rewrite template
fixSafety: safe                     # safe | unsafe | suggestion (default: suggestion)
tests:                              # run by `gdguard test-rules`
  invalid:
    - code: "if len(items) == 0:\n\tpass"
      output: "if items.is_empty():\n\tpass"
  valid:
    - "if items.is_empty():\n\tpass"
```

Supported in `rule:` for v1: `pattern`, `kind`, `regex`, `inside`, `has`,
`follows`, `precedes`, `all`, `any`, `not`, plus `constraints` on
metavariables. Also `stopBy` and `field` inside `inside`, `has`, `follows`
and `precedes`, and the object form `pattern: { context, selector,
strictness }`. `follows` and `precedes` compare siblings, so for statements
use `pattern: { context: <code>, selector: expression_statement }`. A
top-level `rule` must contain `pattern`, `kind`, `regex`, `all` or `any`.
Other ast-grep keys (`nthChild`, `range`, `matches`, `utils`, `transform`,
`rewriters`) are not supported.

The engine is ast-grep (`ast-grep-core`/`ast-grep-config` 0.51), with a
GDScript `Language` implementation (decided in W1/B1).

Metavariables: `$X` matches one node, `$$$ARGS` matches zero or more nodes,
`$_` and `$$$` match without capturing. A metavariable name uses uppercase
letters, digits and `_` and must not be followed by another letter, so
`$X`, `$ARGS` and `$NODE_1` are metavariables.

GDScript also uses `$` for node paths. Every `$` that doesn't start a
metavariable is literal: `$Player`, `$"UI/Label"` and `$player` are node
paths. For an all-caps node path, escape the dollar: `\$HUD`. These rules
apply the same way in `pattern`, `fix`, `message` and `help`.

Literal node paths, strings, string names (`&"x"`) and node path literals
(`^"x"`) in patterns match by their exact text: `get_node("Player")` doesn't
match `get_node("Enemy")`, and `$Player.hide()` doesn't match
`$Enemy.hide()`. To match several, use a metavariable with a constraint, for
example `$P` with `constraints: { P: { regex: '^\$(Player|Enemy)$' } }`, or
`kind: get_node` with `regex`. Unique names (`%Health`) are `get_node` nodes
too.

Rule file details:

- `severity` defaults to `warning`; `message` and `rule` are required;
  `fixSafety` requires `fix`.
- `tests.invalid` entries may also be a bare code string (no output check).
- Rule files are found in each folder itself, not in sub-folders, sorted by
  path. A duplicate id is reported on the second file and names the first.
- `RuleMeta` mapping: `description` is the `message` as written,
  `bad_example`/`good_example` are the first invalid/valid test, and
  `fixable` is true when the rule has a `fix` with safety `safe` or `unsafe`.
- `tests.invalid` entries may set `errors: N` to require exactly N
  diagnostics (default: at least one).
- Multi-line `fix` templates are written without the surrounding
  indentation; every line after the first gets the indentation of the line
  the match starts on (tabs or spaces).
- In `message`, `help` and `fix`, `$$$ARGS` renders as the source text of
  the captured nodes including their separators.
- A diagnostic's span is the matched node. Its fix is one edit over that
  span, with safety `fixSafety` (default `suggestion`).

Validation errors (bad YAML, unknown keys, a pattern that doesn't parse as
GDScript, a duplicate id) report the rule file path and stop with exit code 2.

When a metavariable is captured in several places (for example in the
pattern and in a `has`), `constraints` check the last capture.

The starter rule pack is in `examples/rules/`.

`gdguard test-rules [RULE_DIR]` loads the rules from `RULE_DIR`, else
`[custom_rules].paths`, else `.gdguard/rules`, and runs each rule's `tests`:
`valid` cases must produce no diagnostic for that rule; `invalid` cases at
least one (or exactly `errors`). When a case has `output`, the rule's fixes
are applied (any safety level); the result must equal `output`, must parse,
and must produce no diagnostic when the rule runs again. It prints
`✓ <id>  N passed` or `✗ <id>  F failed, N passed` with details, then a
summary line. Exit code 0 when everything passes, 1 on a failing test, 2 when
rules can't be loaded.

## Parser diagnostics

`parse-error` (Error, non-fixable) reports the first ERROR/MISSING node the
bundled parser cannot read, with a range and repair/re-lint/Godot guidance.
Like other diagnostic IDs it is configurable and reserved from custom-rule
collisions. A malformed source must not appear clean merely because style
rules find nothing. Empty enums are explicitly tolerated: Godot 4.7 accepts
them although the pinned grammar inserts a missing enum identifier. The
parser may lag newer Godot syntax; Godot remains the authoritative checker.

## Fix engine

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
   never reformat files the user doesn't format. With `[fix].format = true`
   (W11), every checked file is formatted, whether or not a fix applied or
   it was formatted before, using its EditorConfig settings and the
   formatter's structure check (`safe`), repeated (at most 3 runs) until
   the text stops changing so one `--fix` run is idempotent; a file the
   formatter can't parse or whose structure would change stays unformatted. Remaining
   diagnostics refer to the formatted text.
6. A file that already has syntax errors is not fixed, with one exception
   (W9): when the allowed fixes applied together leave no syntax error at
   all (for example `godot3-syntax` replacing `onready`/`tool`), that result
   is accepted and fixing continues from it. Otherwise the file is unchanged.
7. `--diff` prints a unified diff (`a/<path>`, `b/<path>`) and doesn't
   write anything. Exit code 1 when a file would change, else 0.

`fix-failed` is a built-in id: it can be set in `[rules]` like any rule.

`[fix].unsafe` (W11) lets `--fix` apply unsafe fixes too: `true` for every
rule, or a list of rule ids (built-in or custom; an unknown id is a config
error naming `fix.unsafe`, with a suggestion). `--fix-unsafe` still applies
all of them. Suggestions are never applied. The setting is per config file,
not per `[[overrides]]` entry.

Built-in fixes: `unnecessary-pass` (Safe, when `pass` is alone on its line),
`no-else-return` (Safe: `elif` → `if` when every earlier branch returns, and
removing `else:` and dedenting its body; Unsafe when that body declares a
`var` or `const`), `unused-argument` (Unsafe: prefix the name with `_`),
`prefer-uid-path` (Safe: replace the path literal with the UID, keeping the
quote), `missing-class-name` (Unsafe: insert `class_name <Name>`) and
`godot3-syntax` (Safe: `tool` → `@tool`, `onready` → `@onready`; Unsafe:
`export var` → `@export var`, `yield(obj, "sig")` → `await obj.sig` with
`idle_frame` → `process_frame`, `.instance()` → `.instantiate()`,
`connect("sig", target, "method")` → `connect("sig", target.method)`).

`declaration-order` (W10, corrected after review) reuses the formatter's
reorder plan and offers a full-file fix unless formatting is disabled by a
`# fmt: off` region or the file has a top-level `@warning_ignore_start` or
`@warning_ignore_restore` annotation (these cover every line between them,
so moving declarations would change which warnings they hide; the
formatter's `--reorder-code` still moves them). For a file already formatted with its EditorConfig
settings, use `format_gdscript` with `reorder_code` so the result matches
`gdguard --reorder-code`. Otherwise move original source slices for each
plan item, including its attached comments/annotations and split inline
extends, without reformatting declaration bodies. Preserve text before the
first item and after the last, including standalone trailing comments.
Use one blank line between items, two before/after functions and inner
classes; preserve CRLF separators when present. The source-slice result must
parse and re-lint without declaration-order, retaining every declaration.

The fix is Unsafe whenever an initializer reads a member variable declared
before it in the source but after it in the new order. Check every variable
pair, including bare identifiers and `self.member`, and all variable kinds
(regular, exported, static and onready), even when a variable's numeric item
index does not change. A pre-existing reference to a later variable that
remains later does not by itself make the reorder unsafe. Constants, enums,
signals, methods and inner classes moving remain Safe.

## CLI

```
gdguard lint [FILES...]                 # default: current directory, recursively
  --fix | --fix-unsafe | --diff
  --format pretty|compact|json|github|sarif|agent   (default: pretty on a TTY, compact otherwise)
  --config <PATH>
  --no-cache
  --baseline <PATH> | --update-baseline
  --max-warnings <N>
  --stdin-filename <PATH>               # read source from stdin
  --godot <PATH>                        # optional Godot check (default: $GDGUARD_GODOT)
  --list-rules [--format json]
gdguard explain <RULE>                   # rationale, examples, fixability
gdguard test-rules [RULE_DIR]            # run YAML and JS rule tests
gdguard init [--force]                   # write a starter gdguard.toml and .gdguard/
gdguard lsp                              # language server on stdin/stdout
```

- `--fix`/`--fix-unsafe` write the files, print the report of the issues
  left, then `Fixed N problems in M files` (on stderr for machine formats).
  M counts files with at least one applied fix. When `[fix].format`
  changed K > 0 files, `, formatted K files` is appended (singular for 1).
  The exit code follows the issues left.
- `--stdin-filename <PATH>` reads the source from stdin; `PATH` finds the
  config and is shown in output. It can't be combined with file arguments.
  With `--fix`, the fixed source goes to stdout and the report to stderr.
- `--godot <PATH>` (or the `GDGUARD_GODOT` environment variable; an empty
  value turns it off) runs the optional Godot check described in "Godot
  check" after linting, or after `--fix`/`--fix-unsafe` wrote the files.
  It is skipped for stdin and `--diff`, where the checked text isn't on
  disk. A Godot executable that can't run or times out exits 2.

`gdguard explain <RULE>` prints the rule's id, origin (built-in or the
custom rule file), default severity, whether it is fixable, its description,
rationale, and bad and good examples. An unknown id exits with 2 and
suggests the closest id.

`lint --list-rules` prints one line per rule (id, default severity, `fix`
when fixable, description, `(custom)` for custom rules). With `--format
json` it prints `{ "version": 1, "rules": [ ... ] }`, each rule with `id`,
`description`, `rationale`, `default_severity`, `fixable`, `origin`
(`built-in` | `custom`), `path` (rule file or `null`), `bad_example` and
`good_example`. Both include the custom rules of the config found from the
current folder.

`gdguard init [--force]` writes `gdguard.toml` (every built-in rule with its
default level, commented) and `.gdguard/rules/prefer-is-empty.yml` in the
Godot project root (the nearest folder with `project.godot`, else the
current folder). If either file exists it writes nothing and exits 1, unless
`--force` is given.

`--pretty` is an alias of `--format pretty`. The compact format is one line
per issue: `file:line:col: severity [rule] message`.

All listed flags/subcommands are implemented. Unknown options exit 2.
`--list-rules` accepts `--format text` (default) or `--format json`.

Exit codes:

- `0`: no errors, and warnings at or under `--max-warnings`.
- `1`: lint problems found.
- `2`: config, usage or rule-file error.

Without `--max-warnings`, any warning gives exit code 1. Info diagnostics
never change the exit code.

The formatter's existing CLI keeps working unchanged.

## Godot check (`src/linter/godot_check.rs`, W9)

gdguard doesn't check types. With `--godot <PATH>`, `lint` loads the linted
files in headless Godot 4 once per project root and reports each
`SCRIPT ERROR` as `godot-error` (Error, not fixable):

1. When `.godot/global_script_class_cache.cfg` doesn't list exactly the
   project's valid `class_name` scripts (or is missing), run
   `godot --headless --path <root> --import` first, so new classes resolve.
   This writes `.godot/` and `.uid` files like opening the editor does.
2. Write a loader script to the system temp folder and run
   `godot --headless --path <root> --script <loader> -- res://a.gd ...`.
   It prints `GDGUARD-BEGIN <path>`/`GDGUARD-END <path>` on stderr around
   `ResourceLoader.load(path, "", CACHE_MODE_IGNORE)` for each file.
   Scripts are compiled, not instantiated (static initializers may run).
3. Each `SCRIPT ERROR: [Parse Error: ]<message>` followed by
   `at: ... (res://file.gd:LINE)` becomes a diagnostic on that line's
   content of the file being loaded. Errors located in another script are
   reported on line 1 as `In res://other.gd:LINE: <message>`. Identical
   errors are reported once. ANSI colors are stripped.

Godot prints warnings only when the project sets them to error
(`debug/gdscript/warnings/<name>=2`). Excluded files and files where
`godot-error` is `off` aren't loaded; rule levels apply. Ignore comments,
the lint cache and `unused-ignore` don't apply to `godot-error`; the
baseline does. One Godot process may run for 300 s.

## JSON output (schema version 1)

Field names are stable. New optional fields may be added without bumping the
version.

```json
{
  "version": 1,
  "tool": { "name": "gdguard", "version": "0.29.0" },
  "summary": { "files": 12, "errors": 1, "warnings": 3, "infos": 0, "fixable": 2, "baseline_hidden": 0 },
  "issues": [
    {
      "file": "scripts/enemy.gd",
      "rule": "no-get-node-in-process",
      "origin": "custom",
      "severity": "error",
      "message": "Don't call get_node() every frame",
      "help": "Declare `@onready var health_bar = $HealthBar` and reuse it",
      "range": {
        "start": { "line": 17, "column": 12, "byte": 402 },
        "end":   { "line": 17, "column": 33, "byte": 423 }
      },
      "snippet": "    var hp = get_node(\"HealthBar\")",
      "explain": "gdguard explain no-get-node-in-process",
      "fixes": [
        {
          "description": "Replace with is_empty()",
          "safety": "safe",
          "edits": [
            { "range": { "start": {"byte": 402}, "end": {"byte": 423} },
              "replacement": "items.is_empty()" }
          ]
        }
      ]
    }
  ]
}
```

`summary.files` counts every linted file, `summary.fixable` counts issues
with a `safe` fix, `origin` is `builtin` or `custom`, `snippet` is the full
first line of the range, and `help` is `null` when the rule gives none.
`summary.baseline_hidden` is an additive W4 field counting hidden issues.

## Baseline

`--update-baseline` writes every current issue, sorted deterministically, to
`--baseline PATH`, else `[lint].baseline`, else `.gdguard/baseline.json`.
It cannot be combined with `--fix`, `--fix-unsafe` or `--diff`.
`--baseline` or `[lint].baseline` hides matching issues from lint and fix
reports. The default path is not loaded unless configured or passed.
A missing explicit file is an error; a missing configured file hides nothing.

The fingerprint is FNV-1a 64 over the rule ID, file path relative to the
config folder (or cwd), trimmed source line, and occurrence index among
identical (rule, file, text) issues. Moving lines keeps issues hidden;
editing the line or adding another occurrence reveals an issue.
Pretty output shows `N issues hidden by baseline.`; JSON includes
`summary.baseline_hidden`. Commit baselines, not `.gdguard/cache/`.

## CI reporters

`--format github` emits GitHub workflow annotations with UTF-16 columns,
escaped properties/data, and Info mapped to `notice`. `--format sarif`
emits SARIF 2.1.0, `columnKind: utf16CodeUnits`, percent-encoded file URIs,
and metadata in `tool.driver.rules` for the sorted set of reported rules.
SARIF does not export fixes yet.

## AI agent integration

- `--format agent` (W11) is short text for agents. Per issue: the compact
  line `file:line:col: severity [rule] message`, then `  > <trimmed source
  line>` (cut to 120 characters with `...`), `  help: <help>` when present,
  and one `  safe fix: `, `  unsafe fix: ` or `  suggestion: ` line per fix
  with its description. Then one summary line: `N issues (E errors, W
  warnings, I infos)` (zero counts left out, singular for 1), `, H hidden
  by baseline` when H > 0, and `. Rule details: gdguard explain <rule>`.
  A line `Safe fixes: run gdguard lint --fix.` follows when any issue has a
  safe fix, and a line about reviewing unsafe fixes when any has one. A
  clean report prints nothing. With `--fix` it goes to stdout, like
  pretty and compact.
- Run `lint --fix --format json`, review remaining `help` and `fixes`, use
  `explain` when unsure, edit, and re-lint until exit 0. Review unsafe fixes
  and suggestions; never add ignore comments without a ` -- reason`.
- `integrations/claude-code/` includes a POSIX sh PostToolUse hook registered
  for `Edit|Write|MultiEdit`, settings, a skill and an AGENTS snippet.
- The hook decodes stdin JSON with `jq`, or Python 3 without jq. It checks
  `.gd` files, runs safe fixes, returns nothing when clean, or returns a
  short summary plus the `agent` report on stderr with exit 2 when issues
  remain (W11; `GDGUARD_HOOK_FORMAT=json` returns the JSON report instead,
  any other value is a tool error). Tool errors also return feedback with
  exit 2.
- Binary lookup: `GDGUARD_BIN`, otherwise `gdguard`, then
  `gdscript-formatter` (legacy compatibility alias).
- User guides: `docs/configuration.md`, `docs/custom_rules.md`,
  `docs/agents.md`. Complete YAML examples are tested automatically.

## Semantic model (`src/semantic/`)

Built once per file, after parsing, and shared by built-in rules, JS rules and
the LSP. Its API is frozen at the start of wave 5 (SM0 in the plan).

```rust
pub struct ScopeId(pub u32);
pub struct SymbolId(pub u32);

pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub span: Span,
    pub symbols: Vec<SymbolId>, // declaration order
}

pub enum ScopeKind { Class, InnerClass, Function, Lambda, Block, ForLoop, MatchBranch }

pub enum SymbolKind {
    Variable, Constant, Parameter, LoopVariable, Function, Signal,
    Enum, EnumMember, InnerClass,
}

pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub name_span: Span,
    pub declaration_span: Span,
    pub declared_type: Option<String>,   // text of the annotation, never inferred
    pub annotations: Vec<String>,        // "onready", "export", ...
    pub is_static: bool,
}

pub enum ReferenceKind { Read, Write, Call }

pub struct Reference {
    pub span: Span,
    pub name: String,
    pub kind: ReferenceKind,
    /// None means unresolved: inherited member, engine global, autoload, or a typo.
    pub symbol: Option<SymbolId>,
}

impl SemanticModel {
    pub fn build(tree: &tree_sitter::Tree, source: &str) -> SemanticModel;
    pub fn scope_at(&self, byte: usize) -> ScopeId;
    pub fn lookup(&self, scope: ScopeId, name: &str) -> Option<SymbolId>;
    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol>;
    pub fn symbols(&self) -> &[Symbol];
    pub fn scopes(&self) -> &[Scope];
    pub fn references_to(&self, id: SymbolId) -> &[Reference];
    pub fn unresolved_references(&self) -> &[Reference];
    pub fn debug_dump(&self) -> String;
}
```

SM0 freezes the definitions above. IDs are zero-based; scope 0 is always the
file's root Class scope, including empty/error files. `symbol` returns None
for unknown IDs; references to unknown IDs are empty. `scope_at` outside the
source returns root 0. `debug_dump` uses ID-ordered scopes and symbols, then
references ordered by (start byte, end byte, kind), with LF endings and
Rust Debug string escaping. Its exact entry format is documented in
`src/semantic/mod.rs`. W5 builds declarations/scopes in one syntax walk,
then resolves collected references (including forward class members).
Locals are visible after their initializer. Inner classes cannot capture
outer instance members, lambdas capture enclosing scopes, and self.x
resolves to the nearest class member. Compound assignments produce read
and write uses. Named enum members resolve through their enum qualifier.
Scope queries use a binary search over disjoint scope-boundary intervals.
The pipeline builds one model when custom providers or enabled semantic
built-ins need it. W6 supplies the read-only project index in FileContext.

Important limit: an unresolved reference is **not** an error, because it may
come from the parent class, the engine or an autoload. Unused checks for locals, parameters and private members use resolved
lexical references. Project signal checks are conservative name-based
heuristics; runtime or external consumers still require review.

W6 built-in rules: `unused-variable`, `unused-private-function`,
`shadowed-variable`, `unused-local-signal` (declared and never emitted or
connected in the same file), `assigned-but-never-read`. They default to
Warning. Unused locals/loop bindings exempt `_` prefixes; class members
are not treated as unused locals. Assignments with no reads have a separate
warning. Unused-variable fixes are Unsafe, omitted for initializers with
calls; an otherwise-empty body receives pass. Private helper removal is a
Suggestion and exempts known Godot virtuals and string references. Signal
string emit/connect calls, including self calls, count as local use.
Semantic built-ins do not report on a malformed tree.

## Project index (`src/project/`)

Built from the folder containing `project.godot`. Its API is frozen at SM0.

```rust
pub struct ScriptSummary {
    pub path: PathBuf,            // project-relative, without res://
    pub class_name: Option<String>,
    pub extends: Option<String>,  // class name or res:// path
    pub functions: Vec<String>,
    pub signals: Vec<String>,
    pub variables: Vec<String>,
}

pub struct ProjectIndex {
    pub root: PathBuf,
    pub scripts: Vec<ScriptSummary>,
    pub class_names: HashMap<String, usize>,   // name -> index in scripts
    pub autoloads: HashMap<String, PathBuf>,   // from project.godot [autoload]
}

pub enum MemberLookup { Yes, No, Unknown }

impl ProjectIndex {
    pub fn build(root: &Path) -> Result<ProjectIndex, String>;
    /// Walks `extends` chains inside the project. Stops at engine classes.
    pub fn has_member(&self, class_name: &str, member: &str) -> MemberLookup; // Yes | No | Unknown
}
```

W6 builds one index per discovered project root and shares it read-only
across workers. `project.godot` autoload values have their `*res://` prefix
removed; paths stay project-relative. Summaries include top-level functions,
signals and declared members (variables, constants, enums and inner classes).
Malformed scripts are cached but excluded from resolvable summaries until
repaired. W9: a `class_name` declared by more than one valid script no
longer fails the build. Every script stays in `scripts`, the name is left
out of `class_names` (lookups through it return Unknown), and the
`duplicate-class-name` rule reports it in each declaring file.

`has_member` follows class names and project-relative/res:// script paths.
Missing bases, cycles and explicit engine bases return Unknown. A terminal
project script can return No; implicit Object/RefCounted members and dynamic
_get/_set hooks stay Unknown to avoid false missing-member reports. Engine
member/virtual exemptions come from the installed gdextension-api data.

- The index is cached in `.gdguard/cache/index.json`, keyed by each file's
  content hash; only changed files are re-summarized. Malformed/unwritable
  caches are nonfatal. Discovery order and serialized cache order are stable.
  A run-wide project-content digest participates in lint cache keys, so JS
  project rules invalidate when other scripts, project.godot or text scenes change.
- Cross-file rules (Warning, no automatic fixes): `unused-signal` counts
  string emit/connect/disconnect/is_connected calls, signal.emit/connect
  uses and `.tscn` connection entries across the project. A shared immutable
  `ProjectAnalysis` is built once per run/root, and current-buffer signal
  uses are merged with disk uses. Dynamic emission suppresses definite unused
  reports; matching by name may hide unrelated signals of the same name.
  Binary scenes and external/runtime consumers cannot be proven.
- In a project, enabled `unused-signal` supersedes `unused-local-signal` to
  avoid duplicate and scene-blind reports; disabling it restores the local
  rule. Outside a project, only the local rule applies.
- `unknown-member-on-project-class` checks identifier receivers resolved to
  symbols with explicit project class annotations. It reports only a `No`
  from has_member, with a precise member range; `Unknown` is never reported.
  There is no inference. Reference positions are indexed once per file to
  avoid scanning all uses for each attribute.
- Additive W8 APIs: `ProjectIndex::build_uncached(root)` skips both summary
  cache reads/writes; `ProjectAnalysis::{build,from_tree}` supplies shared
  signal usage and current-buffer overlays without changing frozen index fields.
- Additive W9 APIs:
  `ProjectIndex::has_member_in_script(path, member)` (like `has_member`,
  from a project-relative or `res://` script path) and
  `ProjectIndex::scripts_declaring(class_name) -> Vec<&Path>`.
  `ProjectAnalysis` gains `connected_methods` (method names of every scene
  connection), `scene_connections: Vec<SceneConnection>` (`scene`,
  `signal`, `from`, `to`, `method`, and `target_script`: the `res://`
  script of the `to` node when the scene sets one through an external
  resource), `uid_paths`/`path_uids` (`uid://` ↔ `res://`, from `.uid`
  files, the `[remap]` `uid` of `.import` files and the header `uid` of
  `.tscn`/`.tres` files) and `scene_root_scripts` (scene → root script).
  Text scenes are read line by line: `[ext_resource]` ids, `[node]` paths
  (`.` for the root, `parent/name` otherwise) with their `script =
  ExtResource(...)`, and `[connection]` attributes.
- The project digest (lint cache key) covers the path of every project file
  plus the contents of scripts, `project.godot`, `.tscn`, `.tres`, `.uid`
  and `.import` files, so adding a resource or a UID invalidates cached
  results. Tool folders (`.git`, `.godot`, `.gdguard`, `node_modules`,
  `target`) are skipped.

### W9 rules

All need `project.godot` except the four single-file rules at the end. A
file's project path comes from its canonical path (or its folder's, for an
unsaved buffer) relative to the root.

- `duplicate-class-name` (Error): the file's `class_name` is also declared
  by another valid script. The message lists the other `res://` paths.
- `missing-class-name` (Warning, Unsafe fix): a script without a top-level
  `class_name`, unless it is an autoload script or the root script of an
  autoload scene. Skipped for files with syntax errors and empty files.
  Reported on the `extends` statement (else the first code line). The fix
  inserts `class_name <PascalCase file stem>` on the line above `extends`
  (after a first-line `@tool` when there is no `extends`); no fix when the
  name isn't a valid identifier, is an engine class, built-in type or
  singleton (`godot_classes.rs`, Godot 4.7.2 `--dump-extension-api`), is
  declared by a project script, or is an autoload name.
- `scene-connection-missing-method` (Error): a scene connection whose
  target node's script is this file names a method that neither this
  buffer's top-level functions nor `has_member_in_script` find. `No` is
  reported. `Unknown` is reported only when the method starts with `_`, is
  not a Godot virtual method and the script's chain ends at an engine
  class: engine methods never start with `_` unless virtual. Reported on
  the `extends` statement, once per scene, signal, source node and method.
  Nodes without a script, instanced sub-scenes and built-in scripts are
  skipped.
- `unused-private-function` (W9 change): in a project, a private function
  named by any scene connection counts as used (matched by name).
- `missing-resource-path` (Error): a plain string literal passed first to
  `load()`, `preload()`, `ResourceLoader.load()` or
  `ResourceLoader.load_threaded_request()` names a missing file
  (`res://` or a path relative to the script's folder) or an unknown UID.
  Computed paths, `user://` and raw or escaped strings are skipped.
- `prefer-uid-path` (Warning, Safe fix): the same calls with a `res://` or
  relative path whose file has a known UID. Paths without a known UID
  (JSON, text, resources never saved by Godot 4) aren't reported, so every
  report has a fix.
- `self-assignment` (Warning): `x = x` where `x` resolves to a local
  variable, parameter or loop variable. Members are skipped because
  `property = property` re-runs a setter.
- `unreachable-code` (Warning): the first non-comment statement after
  `return`, `break` or `continue` in the same block, spanning to the end of
  the block.
- `duplicate-dictionary-key` (Error): a dictionary literal repeats a key.
  Strings compare by content (either quote), Lua-style keys by name,
  identifiers and attribute chains by text, numbers and literals by text.
  Calls and escaped strings are skipped. Reported on the later key.
- `godot3-syntax` (Error): `yield(...)`, `.instance()` with no arguments
  and `connect("sig", target, "method")` from the tree; from source lines
  (outside strings and comments, so they work despite parse errors) a
  top-level `tool` line, `onready var`, `export var`, `export(...)`,
  `setget` on a `var` line, and `remote`/`master`/`puppet`/`*sync` before
  `func` or `var`. Fixes are listed in "Fix engine".

### W10 rule

- `declaration-order` (Warning, enabled by default, syntax-only): compare the
  source order of top-level declarations with `reorder::build_reorder_plan`.
  Follow the formatter's exact ordering within each region group, including
  method priorities and public/private ordering. Report once per file, spanning
  the first declaration that belongs earlier and naming what it precedes.
  Inner class bodies are not checked; files with parse errors are skipped.
  Disabled formatting regions do not suppress the diagnostic, but prevent
  its fix. Messages distinguish regular, exported, static and onready
  variables. `[rules] declaration-order = "off"` disables it. See "Fix engine"
  for availability and safety; default formatter behavior is unchanged.
- Invalidate pre-W10 lint caches even when the package version is unchanged.
  Cache keys include formatter EditorConfig settings while declaration-order
  is enabled, so cached fixes follow the correct formatter/source-slice path.
  The review fix invalidates original W10 caches containing unsafe edits
  incorrectly classified as Safe.

### W11 rules

Base rules learned from a working game project (mark-my-spell). Cache
version 7 drops results cached before them.

- `single-line-container-type` (Error, Safe fix, syntax-only): a `type`
  node whose parent is a `variable_statement`, `const_statement`,
  `function_definition` (return type) or `for_statement`, outside lambdas
  and parameter lists, holding `Array[...]` or `Dictionary[...]` with a line
  break not preceded by `\`. Godot 4.7.2 rejects these ("Expected type for
  collection after ..."); it accepts parameter and lambda types and `\`
  continuations. The fix removes whitespace and `\` inside the type and
  puts one space after each comma; no fix when the type contains a comment.
- `prefer-is-empty-call` (Info, Safe fix, semantic): `x.size()` or
  `len(x)` for Array, `Array[T]`, Dictionary, `Dictionary[K, V]` or a
  packed array, and `x.length()` or `len(x)` for String, compared with the
  integer `0` or `1`, either operand order. `== 0`, `<= 0`, `< 1` give
  `x.is_empty()`; `!= 0`, `> 0`, `>= 1` give `not x.is_empty()`,
  parenthesized when the parent is a non-logical binary operator or a unary
  operator other than `not`. `x` must be an identifier whose symbol has
  that declared type or a literal initializer (`[]`, `{}`, a string,
  `Packed*Array(...)`). Comparisons containing comments are skipped.
- `prefer-squared-distance` (Warning, Unsafe fix, semantic): `<`, `<=`, `>`
  or `>=` with `V.distance_to(x)` or `V.length()` on either side, where V
  is a float vector: an identifier declared (or literal-initialized) as
  `Vector2`/`Vector3`/`Vector4`, an unresolved `position`/`global_position`,
  an identifier chain ending in one of them, or `+`/`-` of such vectors. The
  other operand must be pure (identifiers, attribute reads, numbers,
  `+ - * /`, parentheses) and not a non-positive literal. The fix renames
  the method to `distance_squared_to`/`length_squared` and replaces the
  threshold T with `T * T` (integers up to 1,000,000 are squared to a
  literal; compound T is parenthesized). Unsafe: a negative threshold or
  rounding on the boundary can change the result.
- `prefer-array-method` (Info, Unsafe fix, semantic): a `for E in S` where
  S is an identifier typed `Array`/`Array[T]` and the body is one statement
  with no comments, `await`, assignments, lambdas, functions, loops, `match`,
  `break`/`continue` or errors. Boolean scan: `if P: return true` (no
  elif/else, P on one line) directly followed by `return false` gives
  `return S.any(func(E[: T]): return P)`; reversed booleans give `.all`
  with P negated (`not X` → `X`, De Morgan over `and`/`or`, `==` ↔ `!=`,
  otherwise `not (P)`). Accumulator: a `var R` with an empty `[]` value (and
  an Array type, if typed) directly before the loop, R not mentioned in P or
  X, R ≠ S; `R.append(E)` gives `R.assign(S)`, `R.append(X)` gives
  `R.assign(S.map(func(E[: T]): return X))`, `if P: R.append(E)` gives
  `R.assign(S.filter(func(E[: T]): return P))`. The lambda parameter keeps
  the loop variable's type, else the array's element type. Reported on the
  `for` keyword.
- `prefer-unique-node-name` (Info, Unsafe fix, project): `$path` or
  `$"path"` (relative, no `..`, `%` or `:`) outside inner classes, when the
  script is the root script of at least one scene, is attached to no other
  node, and every such scene owns a node at that path with
  `unique_name_in_owner = true`. The fix gives `%Name` (`%"Name"` when the
  last segment isn't an identifier). `ProjectAnalysis` gains (additive)
  `scene_unique_nodes` (scene → owned unique node paths; nodes under an
  instanced node are left out, inherited scenes have no entry) and
  `non_root_scripts` (scripts on non-root nodes or on an inherited scene's
  root).

## Scripted rules in JS (`src/linter/script/`)

- `.js` files in the same `custom_rules.paths` folders as YAML rules.
- They run in an embedded QuickJS (`rquickjs`) with no file system, network or
  imports, a 64 MB memory limit and a time limit per file.
- Behind cargo feature `js-rules`, enabled by default alongside `gdscript`.
  SM0 pins `rquickjs = 0.9.0` (optional); `--no-default-features` omits it.
  W5 implements execution. Builds without it reject configured JS files
  with a clear feature error instead of silently skipping them.
- API modeled on oxlint/ESLint:

```js
// .gdguard/rules/max-function-params.js
/** @type {import("gdguard").Rule<{max?: number}>} */
export default {
  meta: {
    id: "max-function-params",
    severity: "warning",
    description: "Functions should take at most N parameters",
    rationale: "Long parameter lists are hard to call correctly ...",
    fixable: false,
  },
  create(context) {
    const max = context.options.max ?? 5;          // from gdguard.toml
    return {
      function_definition(node) {                   // tree-sitter node kind
        const params = node.field("parameters")?.namedChildren ?? [];
        if (params.length > max) {
          context.report({
            node: node.field("name") ?? node,
            message: `Function has ${params.length} parameters (max ${max})`,
            help: "Group related parameters into a Resource or Dictionary.",
          });
        }
      },
    };
  },
  tests: {
    valid: ["func a(x, y):\n\tpass"],
    invalid: [{ code: "func a(a, b, c, d, e, f):\n\tpass", errors: 1 }],
  },
};
```

- **Node API:** `type`, `text`, `range`, `parent`, `children`,
  `namedChildren`, `field(name)`.
- **`context`:** `filename`, `source`, `options`, `report()`,
  `fix.replace(node, text)`, `fix.insertBefore(node, text)`,
  `fix.remove(node)`, plus the two data APIs below.
- **`context.semantic`:** `scopeAt`, `lookup`, `referencesTo`, `symbols`.
- **`context.project`:** `classNames`, `autoloads`, `hasMember`, plus optional
  absolute `root` (additive in W6 for exact owner comparisons). filename in
  JS context is absolute; diagnostic display paths keep CLI conventions.
- The runtime limits the JS heap to 64 MB, stack to 512 KB and file checks
  to 2 seconds with an interrupt handler. Rule-load evaluation has the same
  deadline. Runtime failures or invalid reports become `script-error`
  (Error, non-fixable) instead of aborting the worker. Metadata/load errors
  include the rule path and stop loading. `console.log` is quiet unless
  `GDGUARD_VERBOSE` is set, and always uses stderr.
- Each worker/config owns its runtime; no runtime crosses thread boundaries.
  Node properties query native tree handles lazily, with no tree graph
  serialized into JS. Rule options honor per-folder config overrides.
- The engine only calls a rule for the node kinds it subscribed to (the same
  visitor map as the built-ins), so the JS cost scales with matches, not file
  size.
- Types ship as `integrations/types/gdguard.d.ts` (SM0 frozen contract).
  `Node.range` and edits use `{start,end}` positions containing `line`,
  `column` and `byte`, with 1-based UTF-8 columns and exclusive ends.
  `report` accepts exactly one of `node` or `range`; fixes accept an edit,
  an edit array or a fixer callback. All four fixer methods accept a node
  or range. Report-level `fixSafety` overrides `meta.fixSafety` (default
  suggestion). Metadata requires id/description; severity defaults warning.
  Semantic lookup returns numeric symbol IDs or null. Semantic symbols use
  camelCase property names; reference kinds are read/write/call and symbol
  kinds are the snake_case names in the declaration file. Project autoloads
  map names to project-relative script paths; classNames is sorted. The
  optional root field is the absolute project folder.
  `semantic` is null only when the caller supplies no model; `project` is
  null outside a project. Users write `.js` files
  with JSDoc types; `.ts` files are not supported.
- `gdguard test-rules` runs JS rule `tests` the same way it runs YAML tests.

## LSP server (`gdguard lsp`, `src/lsp/`)

- Synchronous pinned `lsp-server = 0.7.9` + `lsp-types = 0.97.0` crates
  (no async runtime). The stdio implementation uses full-text document
  synchronization, open/save publication, and 150 ms change debouncing.
- Publishes diagnostics on open and change (debounced at 150 ms) and on save.
- Code actions:
  - each fix, as a quick fix;
  - "Fix all safe problems in file";
  - "Ignore `<rule>` on this line" (inserts `# gdguard-ignore-next-line`).
- Reloads config and rules when `gdguard.toml` or `.gdguard/rules/**`
  changes.
- Converts positions to UTF-16 at the edge.
- It runs alongside Godot's own GDScript language server. It doesn't replace
  it.
- Protocol errors are logged to stderr; malformed requests return structured
  errors and configuration failures publish a config-error diagnostic.
  Code actions carry the document version and include fix safety in titles.
  Ignore insertions include a reason placeholder for review.
- Editor setup docs for VS Code, Neovim and Zed are in docs/editors.md;
  a small optional Zed adapter registers the second GDScript server.
- CLI/AI-agent workflows are the primary product. IDE features are optional
  compatibility integrations, not CLI release prerequisites.

## Godot editor integration (`gdextension/`, `addons/`)

- The gdextension exposes `lint(path, source)` and `fix(path, source,
  unsafe)` on top of the library API and returns Dictionaries (rule,
  severity, message, help, range, fixes). It reads `gdguard.toml` from the
  project root. The legacy lint_gdscript method remains deprecated but
  available. Native apply_edits validates reviewed individual edits and
  rejects malformed ranges or GDScript; editor config caches use content
  hashes and reload changed config files.
- The optional addons/gdguard_lint addon adds a bottom panel listing the current script's issues.
  - Clicking an issue jumps to its line.
  - An "Apply fix" button applies one fix; "Fix all safe" applies every safe
    fix.
  - It lints on save and when the script changes.

## Performance and caching

- Files lint in parallel with `std::thread::scope`, preserving input order.
  Each worker has its own linter and custom providers; configuration errors
  are resolved before workers start, in deterministic order.
- With a config, lint caches results in `.gdguard/cache/lint.json` beside
  it, keyed by absolute file path and content/settings/custom-rule hashes,
  plus tool version. Baseline filtering occurs after reading cached results.
- `--no-cache` skips reading and writing. Fix/diff and stdin do not cache.
  Unreadable caches are ignored; unwritable caches do not fail linting.
  Writes use a temporary file and rename; deleted-file entries are pruned.
  The cache folder gets a `.gitignore` containing `*`.
- `python3 benchmarks/lint_project.py` generates a temporary 1000-file
  project, measures median cold/warm runs, asserts identical output, and
  checks targets below 3 s cold / 1 s warm. See docs/handoff/w8-a.md for
  the measured machine and results, and docs/handoff/w9.md for the W9 run.

## Distribution

- Primary binary/crate `gdguard`, alias `gdscript-formatter`, native library
  `gdguard_gdextension`, singleton `GDGuard`; addon folders gdguard,
  gdguard_standalone and gdguard_lint.
- `.github/package_release.py` and `make_release --dry-run` review existing
  build inputs without changing versions, git or tags. Explicit --output
  creates CLI/alias, optional native, addon and types/example-rule ZIPs.
- The tag-triggered workflow builds six OS/architecture targets and creates
  a draft release for maintainer review. Version tags remain explicit user
  actions; roadmap completion does not publish a release.
- Docs are split into `docs/configuration.md`, `docs/custom_rules.md`,
  `docs/js_rules.md`, `docs/agents.md` and `docs/editors.md`. The README
  covers only the CLI and agent loop; `docs/editors.md` is not linked from it.
- docs/migration.md, docs/faq.md and docs/releasing.md cover adoption,
  analysis limits and packaging. Nothing from this fork is contributed
  upstream to GDQuest (maintainer decision, 2026-11-09).
