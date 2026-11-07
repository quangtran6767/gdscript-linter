# Handoff w2-b

## Summary
Custom YAML rules now run: `custom::load_providers` returns one
`YamlRuleProvider` that matches every rule on the pipeline's existing tree
(no re-parse) and builds `Diagnostic`s with rendered messages and `Fix`es.
`gdguard test-rules [RULE_DIR]` runs each rule's `tests` (counts, fix output,
parse check, idempotency) and `test-rules examples/rules` passes. While
testing, two ast-grep limitations surfaced and are fixed: string and node
path literals in patterns now match exactly (custom `GdDoc`), and multi-line
fixes are re-indented with tabs or spaces.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| B3 engine | done | `provider.rs`, `doc.rs`; 14 provider tests. |
| B4 `test-rules` | done | `rule_tester.rs`; 13 tests (pass, wrong count, wrong output, non-idempotent, load error, no fix, broken fix, CLI exit codes 0/1/2). |
| Example rules | done | `examples/rules/prefer-is-empty.yml` (safe fix), `no-get-node-in-process.yml` (no fix). |

## Files changed
- `src/linter/custom/mod.rs`: `load_providers` body; `pub mod doc; pub mod provider;`; re-exports `YamlRuleProvider`.
- `src/linter/custom/provider.rs`: new. `YamlRuleProvider`, message/fix rendering.
- `src/linter/custom/doc.rs`: new. `GdDoc`/`GdNode`, the ast-grep document type for GDScript.
- `src/linter/custom/language.rs`: patterns are built with `GdDoc`; `LanguageExt`/`StrDoc` no longer used.
- `src/linter/custom/loader.rs`: `CompiledRule.fixer: Option<Fixer>` became `fix: Option<String>`; `InvalidCase.errors`; literal node paths in patterns are allowed again.
- `src/linter/custom/spike_tests.rs`: uses `GdDoc`; node path tests now assert exact matching; new string literal tests.
- `src/linter/rule_tester.rs`: `test-rules` implementation.
- `examples/rules/`: two rules.
- `tests/custom_rules/`: `provider_tests.rs`, `rule_tester_tests.rs`, fixtures in `provider/` and `rule_tester/`; loader tests updated.

## Public API added
```rust
// src/linter/custom/mod.rs
pub fn load_providers(config: &Config) -> Result<Vec<Box<dyn RuleProvider>>, String>;
    // [] when config.custom_rule_paths is empty, else [YamlRuleProvider].
    // Err is the loader's ConfigError as text: "<path>: `<key>`: <message>".
pub use provider::YamlRuleProvider;

// src/linter/custom/provider.rs
pub struct YamlRuleProvider { /* metas, compiled */ }
impl YamlRuleProvider {
    pub fn new(loaded_rules: Vec<LoadedRule>) -> YamlRuleProvider;
    pub fn check_rule(&self, rule_id: &str, context: &FileContext) -> Vec<Diagnostic>;
}
impl RuleProvider for YamlRuleProvider { /* rules(), check_file() */ }
pub fn render_template(template: &str, environment: &MetaVarEnv<'_, GdDoc>, source: &str) -> String;
pub fn render_fix(template: &str, environment: &MetaVarEnv<'_, GdDoc>, source: &str, match_start: usize) -> String;

// src/linter/custom/doc.rs
#[derive(Clone)] pub struct GdDoc { /* source, tree */ }   // impl ast_grep_core::source::Doc
impl GdDoc {
    pub fn parse(source: &str) -> Result<GdDoc, String>;
    pub fn from_tree(source: &str, tree: &tree_sitter::Tree) -> GdDoc;  // no re-parse
}
#[derive(Clone, Copy)] pub struct GdNode<'r>(pub tree_sitter::Node<'r>);  // impl SgNode

// src/linter/custom/loader.rs (changed from W1)
pub struct CompiledRule { pub matcher: RuleCore, pub fix: Option<String>, pub fix_safety: FixSafety,
                          pub message: String, pub help: Option<String> }
pub struct InvalidCase { pub code: String, pub output: Option<String>, pub errors: Option<usize> }

// src/linter/rule_tester.rs
pub const DEFAULT_RULE_DIR: &str = ".gdguard/rules";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleReport { pub id: String, pub passed: usize, pub failures: Vec<String> }
pub fn run(args: &TestRulesArgs) -> i32;            // 0 pass, 1 failure, 2 load error
pub fn test_rules(rules: Vec<LoadedRule>) -> Vec<RuleReport>;
pub fn exit_code(reports: &[RuleReport]) -> i32;
pub fn format_reports(reports: &[RuleReport]) -> String;
pub fn line_diff(expected: &str, actual: &str) -> String;
```

Diagnostics from the provider:
- `rule` = rule id; `severity` = the rule's default (the pipeline applies
  `[rules]` levels); `span` = the matched node.
- `message`/`help`: `$X` → captured text; `$$$X` → source text from the
  first to the last captured node (keeps `, `); uncaptured → empty.
- `fixes`: one `Fix` with one `Edit` over the match span, safety from
  `fixSafety` (default `Suggestion`), description `Replace with `<text>``
  for one short line, else `Apply the `<id>` fix`.
- Order: rules in load order (path order), matches in source order. Not
  sorted with built-in diagnostics; the pipeline sorts.

## Decisions
- **`GdDoc` instead of ast-grep's `StrDoc`.** ast-grep compares text only on
  leaf nodes. In tree-sitter-gdscript, `string`, `string_name`, `node_path`
  and `get_node` have anonymous children (quotes, `$`) but no node for their
  contents, so with `StrDoc` `get_node("Player")` matched
  `get_node("Enemy")` and `$Player` matched `$Enemy`. `GdNode` reports those
  four kinds as leaves, so their whole text is compared. Consequences:
  - Literal node paths in patterns work and are allowed again (W1 rejected
    them). `\$HUD` matches the all-caps path `$HUD`.
  - A metavariable can still capture a whole string (`get_node($NAME)`).
- **Fix rendering is ours, not ast-grep's `Fixer`.** ast-grep re-indents
  multi-line fixes with spaces only (its source has `TODO: support TAB`).
  `render_fix` adds the matched line's leading whitespace (tabs or spaces)
  to every template line after the first; captured text is inserted as is.
- **Tree reuse:** `GdDoc::from_tree` clones the tree-sitter `Tree`
  (reference count), so each file is parsed once by the pipeline only.
- **`test-rules` rules:**
  - Each case runs only its own rule (`check_rule`).
  - `invalid` without `errors` needs ≥ 1 diagnostic; with `errors: N`, exactly N.
  - With `output`: apply the first fix of every diagnostic (any safety,
    since the test checks the template), compare, require the result to
    parse without errors, then re-run the rule and require 0 diagnostics.
  - `output` on a rule without `fix` is a failure.
  - Folder: the argument; else `[custom_rules].paths` from
    `load_config_for(".")`; else `.gdguard/rules`. A missing folder is a load
    error (exit 2). An existing folder without rules prints
    `No rules found in <folder>` and exits 0.
  - `run` treats a `load_config_for` error of exactly `not implemented yet`
    as "no config" so it worked before W2-A; after the merge that branch is
    dead and can be removed.
- **Diff format:** positional line diff (`  |` same, `- ` expected,
  `+ ` actual), enough for short test snippets. `similar` is only a
  dev-dependency, so it isn't used.
- **`no-get-node-in-process`** names the function in its message through
  `$FUNC`, captured inside `inside.has` and limited by a `constraints`
  regex. Captures from relational rules are available to messages (tested).

## Requests for the merge session
- **Delete the stale stub cases** in `tests/cli_tests.rs::unimplemented_subcommands_exit_with_code_2_and_name_the_command`:
  `(&["test-rules"], "test-rules")` and `(&["test-rules", "rules"], "test-rules")`.
  They now fail because `test-rules` is implemented (B can't edit that file).
  `test-rules` CLI behavior is covered in `tests/custom_rules/rule_tester_tests.rs`.
- **C1 wiring:** call `custom::load_providers(&config)` once per config and
  `check_file` per file, then apply `[rules]` levels/`off` to custom ids the
  same way as built-ins. `rules()` gives the custom `RuleMeta`s (A's
  validation already uses it).
- **Windows: `tests/reorder_tests.rs` fails on `main` for a different reason
  than CRLF.** It builds the expected path with
  `file_path.to_str().replace("/input/", "/expected/")`; Windows paths use
  `\`, so the replace does nothing and every input is compared with itself
  (12 failures). Re-checkout with LF didn't help. Fix: build the path with
  `Path` components, e.g. `file_path.parent().parent().join("expected").join(file_name)`.
  `tests/integration_tests.rs` may have the same pattern; worth checking.

## Spec updates
In "Custom YAML rules":
- Replace the paragraph starting "Patterns can't contain literal node
  paths" and its example with:

  > Literal node paths, strings, string names (`&"x"`) and node path
  > literals (`^"x"`) in patterns match by their exact text:
  > `get_node("Player")` doesn't match `get_node("Enemy")`, and
  > `$Player.hide()` doesn't match `$Enemy.hide()`. To match several, use a
  > metavariable with a constraint, for example `$P` with
  > `constraints: { P: { regex: '^\$(Player|Enemy)$' } }`, or `kind:
  > get_node` with `regex`.

- Add to "Rule file details":
  - `tests.invalid` entries may set `errors: N` to require exactly N
    diagnostics (default: at least one).
  - Multi-line `fix` templates are written without the surrounding
    indentation; every line after the first gets the indentation of the line
    the match starts on (tabs or spaces).
  - In `message`, `help` and `fix`, `$$$ARGS` renders as the source text of
    the captured nodes including their separators.
- Add a "`gdguard test-rules`" paragraph:

  > `gdguard test-rules [RULE_DIR]` loads the rules from `RULE_DIR`, else
  > `[custom_rules].paths`, else `.gdguard/rules`, and runs each rule's
  > `tests`: `valid` cases must produce no diagnostic for that rule;
  > `invalid` cases at least one (or exactly `errors`). When a case has
  > `output`, the rule's fixes are applied (any safety level); the result
  > must equal `output`, must parse, and must produce no diagnostic when the
  > rule runs again. It prints `✓ <id>  N passed` or
  > `✗ <id>  F failed, N passed` with details, then a summary line. Exit
  > code 0 when everything passes, 1 on a failing test, 2 when rules can't
  > be loaded.

## CHANGELOG lines
- Linter: custom YAML rules in `[custom_rules].paths` run alongside the built-in rules, with metavariables in messages and fixes.
- Linter: new `gdguard test-rules [RULE_DIR]` command runs the `tests` in custom rule files, including fix output and idempotency checks.
- Linter: example custom rules in `examples/rules/` (`prefer-is-empty`, `no-get-node-in-process`).

## How to test manually
```sh
cargo run -- test-rules examples/rules                         # 2 rules pass, exit 0
cargo run -- test-rules tests/custom_rules/rule_tester/wrong_output   # diff, exit 1
cargo run -- test-rules tests/custom_rules/rule_tester/load_error     # exit 2
cargo test --test custom_rules                                 # loader, provider, tester
cargo test --lib linter::custom                                # engine spike tests
```

## Known issues / follow-ups
- Provider diagnostics use the rule's default severity; levels and `off`
  come from the pipeline (C1).
- `load_rules` still stops at the first invalid file.
- The positional line diff can look noisy when a fix adds or removes lines
  in the middle of a long snippet.
