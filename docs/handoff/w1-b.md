# Handoff w1-b

## Summary
The ast-grep spike is a **GO**: `ast-grep-core`/`ast-grep-config` 0.51.1
build against our tree-sitter 0.27.0 (one shared copy), and every capability
the spec lists works on GDScript (metavariables, `$$$` multi-match, `inside`,
`has`, `not`, `follows`/`precedes`, constraints, fix templates). The YAML
loader reads `*.yml`/`*.yaml` from `custom_rule_paths`, validates every key
with errors that name the file and key path, and returns `LoadedRule`s with
`RuleMeta`, a compiled ast-grep matcher and fixer, and the rule's tests.
`custom::load_providers` still returns `Ok(Vec::new())`.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| B1 spike | done (GO) | `GdScript` language in `language.rs`; 21 spike tests in `spike_tests.rs`, 5 unit tests for the `$` rewrite. |
| B2 loader | done | `loader.rs`; 38 tests in `tests/custom_rules/` (exact messages for every error). |
| Keep `load_providers` empty | done | Unchanged behavior; only `pub mod language; pub mod loader;` added. |

## Files changed
- `Cargo.toml`, `Cargo.lock`: `ast-grep-core = "=0.51.1"`, `ast-grep-config = "=0.51.1"` (allowed by the prompt). No separate YAML crate: the loader uses `ast_grep_config::from_str` (serde-saphyr, pulled in by ast-grep-config).
- `src/linter/custom/mod.rs`: declares `language`, `loader` and the test-only `spike_tests`.
- `src/linter/custom/language.rs`: new. ast-grep `Language` + `LanguageExt` for tree-sitter-gdscript, and the `$` rewrite.
- `src/linter/custom/spike_tests.rs`: new. Spike tests (W1/B1 acceptance).
- `src/linter/custom/loader.rs`: new. Rule file discovery, validation, compilation.
- `tests/custom_rules/main.rs`, `tests/custom_rules/loader_tests.rs`: new integration test target `custom_rules`.
- `tests/custom_rules/loader/{valid,duplicate}/`: fixtures.

## Public API added
```rust
// src/linter/custom/language.rs
pub const EXPANDO_CHAR: char = 'µ';
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GdScript;                       // impl ast_grep_core::Language + tree_sitter::LanguageExt
pub fn to_internal(text: &str) -> Cow<'_, str>;   // user `$` syntax -> ast-grep form
pub fn from_internal(text: &str) -> Cow<'_, str>; // `µ` -> `$`

// src/linter/custom/loader.rs
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleTests { pub valid: Vec<String>, pub invalid: Vec<InvalidCase> }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidCase { pub code: String, pub output: Option<String> }
pub struct CompiledRule {                  // Debug (manual), not Clone
    pub matcher: ast_grep_config::RuleCore,
    pub fixer: Option<ast_grep_config::Fixer>,
    pub fix_safety: FixSafety,
    pub message: String,                   // internal form (`µX`)
    pub help: Option<String>,              // internal form
}
#[derive(Debug)]
pub struct LoadedRule { pub meta: RuleMeta, pub compiled: CompiledRule, pub tests: RuleTests }

pub fn load_rules(rule_paths: &[PathBuf]) -> Result<Vec<LoadedRule>, ConfigError>;
pub fn find_rule_files(folder: &Path) -> Result<Vec<PathBuf>, ConfigError>;
pub fn load_rule_file(path: &Path) -> Result<LoadedRule, ConfigError>;
pub fn load_rule_str(path: &Path, text: &str) -> Result<LoadedRule, ConfigError>;
```

Notes for W2 (engine):
- Match with `GdScript.ast_grep(source)` then `root.root().find_all(&rule.compiled.matcher)`.
  To reuse the pipeline's tree instead of re-parsing: `StrDoc` has pub fields
  `{ src, lang, tree }`, so a doc can be built from `FileContext.tree.clone()`.
- Fix: `node_match.replace_by(fixer)` gives `ast_grep_core::source::Edit { position, deleted_length, inserted_text: Vec<u8> }`; convert with `Span::from_byte_range`.
- Message/help rendering: replace `µNAME` / `µµµNAME` in `compiled.message`
  with the captured text (`get_env().get_match` / `get_multiple_matches`).
  The loader already checked that every metavariable used in `message`,
  `help` and `fix` is defined by the rule.

## Decisions
- **GO on ast-grep.** Spike tests cover all six required checks plus `any`,
  constraints, `follows`/`precedes` and kind+regex.
- **`$` escape rule (final):**
  - A metavariable is `$NAME`, `$$NAME`, `$$$NAME` or `$$$`, where `NAME` is
    uppercase letters, digits and `_`, starts with a letter or `_`, and is not
    followed by another identifier character. Internally its `$` signs become
    `µ` (U+00B5), which GDScript accepts in identifiers, so patterns still
    parse. ast-grep's `meta_var_char` is `µ` too, so a literal `$` is never
    special to ast-grep.
  - Every other `$` is literal: `$Player`, `$"UI/Label"`, `$player`.
  - `\$` is always a literal `$`, for all-caps node paths: `\$HUD`.
  - The same rewrite applies to `pattern`, `fix`, `message` and `help`.
- **Literal node paths are rejected in patterns.** In tree-sitter-gdscript,
  `$Player` is a single `get_node` node whose path text is not a child node,
  so ast-grep matches every node path alike (`$Player.hide()` would match
  `$Enemy.hide()`; see `literal_node_path_in_pattern_matches_every_node_path`).
  Rather than overmatching silently, the loader rejects them and suggests
  `$P` plus `constraints: { P: { regex: '^\$Player$' } }`. `kind: get_node`
  with `regex` also works. Literal node paths are fine in `fix`/`message`.
  This replaces the spec's tentative `$$Player` escape: in ast-grep, `$$X`
  already means "capture including unnamed nodes", and keeping ast-grep
  semantics was more useful than reusing it.
- **The loader parses every pattern itself** (including nested ones and
  `context` of the object form) and rejects `ERROR`/`MISSING` nodes, because
  ast-grep accepts broken patterns silently.
- **`follows`/`precedes` work on statements.** A pattern like `queue_free()`
  selects the `call` node, which has no siblings. Users need
  `pattern: { context: queue_free(), selector: expression_statement }`. To
  document in `docs/custom_rules.md` (W4-B).
- **Supported rule keys are exactly the spec's v1 list** plus `stopBy` and
  `field` inside relational rules, and `context`/`selector`/`strictness` in
  the pattern object. `nthChild`, `range`, `matches`, `utils`, `transform` and
  `rewriters` get "not supported in gdguard rules".
- **A top-level `rule` needs a positive matcher** (`pattern`, `kind`,
  `regex`, `all` or `any`). ast-grep accepts `rule: { not: ... }`, which
  matches almost every node.
- **Defaults:** `severity` is optional (default `warning`); `message` and
  `rule` are required; `fixSafety` defaults to `suggestion` and is an error
  without `fix`.
- **`RuleMeta` mapping:** `description` = `message` as written (with `$X`);
  `rationale` = `rationale` or `""`; `bad_example` = first `tests.invalid`
  code; `good_example` = first `tests.valid`; `fixable` = has a `fix` with
  safety `safe` or `unsafe` (a suggestion is never applied by `--fix`).
- **`tests.invalid` entries** may be `{ code, output? }` or a bare code
  string.
- **Discovery:** only the folder itself (not recursive), sorted by path. A
  missing folder is an error with key `custom_rules.paths`. The loader stops
  at the first invalid file.
- **Duplicate ids:** the error is on the second file and names the first.
- **Errors use `ConfigError`** (`<path>: \`<key>\`: <message>`), with dotted
  key paths such as `rule.any[1].kinds` or `tests.invalid[0].outptu`. YAML
  syntax errors come from serde-saphyr and include the line.

## Requests for the merge session
- **MSRV:** `ast-grep-config` depends on `serde-saphyr 2.1.1`, which requires
  Rust 1.89. `Cargo.toml` says `rust-version = "1.85"`. Please bump it to
  `"1.89"` (B is only allowed to touch the ast-grep lines).
- **Windows test failures that exist on `main` already** (not caused by this
  branch; checked on a clean `main` worktree): `e2e_tests::lint_sample_project_default_output`
  (paths print as `.\scripts\...`) and 12 `reorder_tests` (CRLF checkout with
  `core.autocrlf=true`). They pass on Linux/macOS. Suggest normalizing `\` to
  `/` in the e2e snapshot comparison and adding a `.gitattributes`
  (`*.gd text eol=lf`), or comparing with line endings normalized.

## Spec updates
Replace the "Metavariables" paragraph in "Custom YAML rules" with:

> Metavariables: `$X` matches one node, `$$$ARGS` matches zero or more nodes,
> `$_` and `$$$` match without capturing. A metavariable name uses uppercase
> letters, digits and `_` and must not be followed by another letter, so
> `$X`, `$ARGS` and `$NODE_1` are metavariables.
>
> GDScript also uses `$` for node paths. Every `$` that doesn't start a
> metavariable is literal: `$Player`, `$"UI/Label"` and `$player` are node
> paths. For an all-caps node path, escape the dollar: `\$HUD`. These rules
> apply the same way in `pattern`, `fix`, `message` and `help`.
>
> Patterns can't contain literal node paths: the GDScript grammar doesn't
> expose the path text as a node, so `$Player` would match every node path.
> The loader rejects them. Match a specific node path with a metavariable and
> a constraint:
>
> ```yaml
> rule:
>   pattern: $P.hide()
> constraints:
>   P:
>     regex: ^\$Player$
> ```
>
> or with `kind: get_node` and `regex: ^\$Player$`. Unique names (`%Health`)
> are `get_node` nodes too.

Add to "Supported in `rule:`": `stopBy` and `field` inside `inside`, `has`,
`follows` and `precedes`, and the object form
`pattern: { context, selector, strictness }`. `follows` and `precedes`
compare siblings, so for statements use
`pattern: { context: <code>, selector: expression_statement }`. A top-level
`rule` must contain `pattern`, `kind`, `regex`, `all` or `any`. Other ast-grep
keys (`nthChild`, `range`, `matches`, `utils`, `transform`, `rewriters`) are
not supported.

Add to the YAML example's comments: `severity` defaults to `warning`;
`message` and `rule` are required; `fixSafety` requires `fix`.
`tests.invalid` entries may also be a bare code string (no output check).
Rule files are found in the folder itself, not in sub-folders.

## CHANGELOG lines
- None user-visible yet. Custom rules are loaded and validated internally but
  not run until W2 connects the engine.

## How to test manually
```sh
cargo test --lib linter::custom          # spike tests + `$` rewrite tests
cargo test --test custom_rules           # loader tests with exact messages
```

## Known issues / follow-ups
- W2-B: build the `RuleProvider` on `LoadedRule`, render messages, and reuse
  the pipeline's tree instead of re-parsing (see "Notes for W2").
- `load_rules` stops at the first error. Collecting every error would be a
  nice follow-up for `test-rules`.
- Rule ids are checked against built-in rule names only. Config validation
  (W2-A) must also accept custom ids in `[rules]`.
