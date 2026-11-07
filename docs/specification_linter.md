# Linter specification

This document is the source of truth for the linter in this fork. It describes
what the linter does, the shared data contracts every part of the code relies
on, and the file formats users and AI agents see. The work split
lives in `docs/implementation_plan.md`.

Tool name: **`gdguard`** (decided in W0). The binary, the config file
`gdguard.toml`, the rules folder `.gdguard/` and the ignore prefix
`gdguard-ignore` all use it. The Cargo package and binary keep the name
`gdscript-formatter` until W8 renames them. Don't use `gdlint` because
gdtoolkit's Python linter already uses that name.

## Goals

1. Projects can add their own rules without recompiling: YAML rules first,
   scripted rules later.
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
            │  providers   │   (later) scripted rules (JS)
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
fields) and need a PR approved by both developers.

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
}

/// Implemented by the custom rule engine (and later the JS engine). The
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
Github, Sarif }` (`src/linter/output/mod.rs`, Track A).

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

[[overrides]]
files = ["tests/**"]
rules = { private-access = "off" }
```

Unknown rule ids and unknown keys are errors (exit code 2) with a message that
names the file and the key, so typos never fail silently.

## Inline ignore comments

```gdscript
# gdguard-ignore-file prefer-is-empty
var x = 1  # gdguard-ignore no-magic-number -- reason text is allowed
# gdguard-ignore-next-line private-access
```

- The legacy `gdlint-ignore*` prefixes keep working.
- Text after ` -- ` is a reason and is ignored by the parser.
- An ignore comment that suppresses nothing produces an `unused-ignore`
  warning.

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
metavariables.

Metavariables: `$X` matches one node, `$$$ARGS` matches zero or more. GDScript
already uses `$` for node paths (`$Player`). The engine parses a pattern's
`$UPPERCASE` names as metavariables. To match a literal node path, write
`$$Player` (escaped) or use `kind: get_node` with a `regex`. W1/B1 confirms the
exact escape after the engine spike.

Validation errors (bad YAML, unknown keys, a pattern that doesn't parse as
GDScript, a duplicate id) report the rule file path and stop with exit code 2.

## Fix engine

1. Collect the fixes of the allowed safety levels (`--fix` means Safe;
   `--fix-unsafe` means Safe and Unsafe).
2. Sort them and drop any fix that overlaps one already chosen. Dropped fixes
   are retried on the next pass.
3. Apply the fixes with `apply_edits`, then re-parse. If the new tree has
   `ERROR` or `MISSING` nodes, undo that pass's fixes one by one to find the
   bad fix, then report it as `fix-failed`. Never write broken code.
4. Re-lint and repeat until nothing changes, for at most 10 passes.
5. Run the formatter on files that changed, only when the file was already
   formatted before the fix. That way we never reformat files the user
   doesn't format.
6. `--diff` prints a unified diff and doesn't write anything.

## CLI

```
gdguard lint [FILES...]                 # default: whole project
  --fix | --fix-unsafe | --diff
  --format pretty|compact|json|github|sarif   (default: pretty on a TTY, compact otherwise)
  --config <PATH>
  --baseline <PATH> | --update-baseline
  --max-warnings <N>
  --stdin-filename <PATH>               # read source from stdin
  --list-rules [--format json]
gdguard explain <RULE>                   # rationale, examples, fixability
gdguard test-rules [RULE_DIR]            # run YAML rule tests
gdguard init [--force]                   # write a starter gdguard.toml and .gdguard/
gdguard lsp                              # language server on stdin/stdout
```

Until a flag or subcommand is implemented, it prints
``"`<flag>` is not implemented yet"`` on stderr and exits with code 2.
`--list-rules` accepts `--format text` (default) or `--format json`.

Exit codes:

- `0`: no errors, and warnings at or under `--max-warnings`.
- `1`: lint problems found.
- `2`: config, usage or rule-file error.

The formatter's existing CLI keeps working unchanged.

## JSON output (schema version 1)

Field names are stable. New optional fields may be added without bumping the
version.

```json
{
  "version": 1,
  "tool": { "name": "gdguard", "version": "0.29.0" },
  "summary": { "files": 12, "errors": 1, "warnings": 3, "fixable": 2 },
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

## Baseline

`--update-baseline` writes all current issues to the baseline file.
`--baseline` hides issues that are in it. The fingerprint is
`hash(rule, file, trimmed text of the issue's line, occurrence index of that
text in the file)`, so issues survive unrelated edits that move lines.

## AI agent integration

- The recommended loop is `lint --fix --format json`, then the agent fixes
  the remaining issues (using `help`, `fixes` with safety `suggestion`, and
  `explain`), then the agent re-lints until the exit code is 0.
- `integrations/claude-code/` ships a PostToolUse hook that lints each edited
  `.gd` file and returns the JSON to the agent, plus a skill/AGENTS snippet
  describing the loop.

## Semantic model (`src/semantic/`)

Built once per file, after parsing, and shared by built-in rules, JS rules and
the LSP. Its API is frozen at the start of wave 5 (SM0 in the plan).

```rust
pub struct ScopeId(pub u32);
pub struct SymbolId(pub u32);

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
    pub fn symbol(&self, id: SymbolId) -> &Symbol;
    pub fn references_to(&self, id: SymbolId) -> &[Reference];
    pub fn unresolved_references(&self) -> &[Reference];
}
```

Important limit: an unresolved reference is **not** an error, because it may
come from the parent class, the engine or an autoload. Rules that report
"unused" are reliable for locals, parameters and private (`_`-prefixed)
members only.

Built-in rules this enables: `unused-variable`, `unused-private-function`,
`shadowed-variable`, `unused-local-signal` (declared and never emitted or
connected in the same file), `assigned-but-never-read`.

## Project index (`src/project/`)

Built from the folder containing `project.godot`. Its API is frozen at SM0.

```rust
pub struct ScriptSummary {
    pub path: PathBuf,            // res:// relative
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

impl ProjectIndex {
    pub fn build(root: &Path) -> Result<ProjectIndex, String>;
    /// Walks `extends` chains inside the project. Stops at engine classes.
    pub fn has_member(&self, class_name: &str, member: &str) -> MemberLookup; // Yes | No | Unknown
}
```

- The index is cached in `.gdguard/cache/index.json`, keyed by each file's
  hash, and only changed files are re-summarized.
- Cross-file rules: `unused-signal` (never emitted or connected anywhere in
  the project), `unknown-member-on-project-class` (only when the whole extends
  chain is project scripts; `Unknown` when it reaches an engine class).

## Scripted rules in JS (`src/linter/script/`)

- `.js` files in the same `custom_rules.paths` folders as YAML rules.
- They run in an embedded QuickJS (`rquickjs`) with no file system, network or
  imports, a 64 MB memory limit and a time limit per file.
- Behind the cargo feature `js-rules`, which is on by default for the CLI.
- API modeled on oxlint/ESLint:

```js
// .gdguard/rules/max-function-params.js
/** @type {import("gdguard").Rule} */
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
        const params = node.field("parameters").namedChildren;
        if (params.length > max) {
          context.report({
            node: node.field("name"),
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
- **`context.project`:** `classNames`, `autoloads`, `hasMember`.
- The engine only calls a rule for the node kinds it subscribed to (the same
  visitor map as the built-ins), so the JS cost scales with matches, not file
  size.
- Types ship as `integrations/types/gdguard.d.ts`. Users write `.js` files
  with JSDoc types; `.ts` files are not supported.
- `gdguard test-rules` runs JS rule `tests` the same way it runs YAML tests.

## LSP server (`gdguard lsp`, `src/lsp/`)

- Synchronous `lsp-server` + `lsp-types` crates (no async runtime), so it
  stays in line with the coding guidelines.
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
- Editor setup docs for VS Code, Neovim and Zed.

## Godot editor integration (`gdextension/`, `addons/`)

- The gdextension exposes `lint(path, source)` and `fix(path, source,
  unsafe)` on top of the library API and returns Dictionaries (rule,
  severity, message, help, range, fixes). It reads `gdguard.toml` from the
  project root.
- The addon adds a bottom panel listing the current script's issues.
  - Clicking an issue jumps to its line.
  - An "Apply fix" button applies one fix; "Fix all safe" applies every safe
    fix.
  - It lints on save and when the script changes.

## Performance and caching

- Files are linted in parallel with `std::thread::scope` (no new dependency).
- The lint cache lives in `.gdguard/cache/lint.json`. Its key is the file
  hash, config hash, rules hash and tool version, so files that haven't
  changed are skipped.
- `benchmarks/` gains a lint benchmark. Linting a 1000-file project from a
  warm cache should take under 1 second.

## Distribution

- Rename the binary, the addon and the release artifacts in
  `.github/workflows/release.yml`.
- Docs are split into `docs/configuration.md`, `docs/custom_rules.md`,
  `docs/js_rules.md`, `docs/agents.md` and `docs/editors.md`; the README
  links to them.
- Offer the general improvements (spans, JSON output, fixes) upstream to
  GDQuest to reduce fork drift.
