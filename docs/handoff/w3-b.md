# Handoff w3-b

## Summary
The starter rule pack in `examples/rules/` has 8 tested rules (the 7
planned, with `prefer-is-empty` split in two), and `test-rules
examples/rules` is all green. Every built-in rule id has metadata, and
`gdguard explain <rule>`, `lint --list-rules` and `lint --list-rules --format
json` work for built-in and custom rules. `gdguard init` writes a commented
`gdguard.toml` and `.gdguard/rules/prefer-is-empty.yml` in the Godot project
root.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| B5 rule pack | done | 8 rules, 54 test cases; a test enforces rationale, help, ≥ 3 valid and ≥ 3 invalid, and `output` on every invalid case of a fixable rule. |
| B6 metadata | done | `rules_meta.rs`: 19 ids (17 built-ins + `load-variable-name` + `fix-failed`). Every bad example is checked against the real linter. |
| B6 `explain` | done | Built-in and custom, "did you mean", exit 2 on unknown ids. |
| B6 `--list-rules` text and JSON | done | Includes custom rules of the current folder's config. |
| B7 `init` | done | Project root via `project.godot`, no overwrite without `--force` (exit 1). |

### Rule pack
| Rule | Severity | Fix | Safety |
|------|----------|-----|--------|
| `prefer-is-empty` | warning | `len(x) == 0`, `len(x) < 1`, `0 == len(x)` → `x.is_empty()` | safe |
| `prefer-not-is-empty` | warning | `len(x) > 0`, `!= 0`, `>= 1`, `0 < len(x)` → `not x.is_empty()` | safe |
| `no-get-node-in-process` | warning | none (see Decisions) | n/a |
| `prefer-preload-const` | warning | `load("res://…")` → `preload("res://…")` | suggestion |
| `no-godot3-yield` | error | `yield(obj, "sig")` → `await Signal(obj, "sig")` | unsafe |
| `no-string-connect` | error | `connect("sig", target, "method")` → `connect("sig", Callable(target, "method"))` | unsafe |
| `no-print-left` | info | `print(…)`/`prints(…)`/`print_debug(…)` statement → `pass` | suggestion |
| `prefer-is-instance-valid` | info | `x != null` → `is_instance_valid(x)` when `x` is declared with a node type | suggestion |

## Files changed
- `examples/rules/`: 8 rule files (`prefer-is-empty` and `no-get-node-in-process` rewritten; 6 new).
- `src/linter/rules_meta.rs`: metadata table, `builtin_rule_metas`, `builtin_rule_meta`, tests.
- `src/linter/explain.rs`: `explain`, `--list-rules` text and JSON.
- `src/linter/init.rs`: `gdguard init`.
- `src/linter/rule_tester.rs`: removed the dead "not implemented yet" config branch (requested by w2-merge).
- `tests/explain/` (new target): `main.rs` (explain, list-rules), `init_tests.rs`, fixture `project/`.
- `tests/custom_rules/`: example pack test; updated expectations for the new pack.

## Public API added
```rust
// src/linter/rules_meta.rs
pub fn builtin_rule_metas() -> Vec<RuleMeta>;          // sorted by id, 19 entries
pub fn builtin_rule_meta(id: &str) -> Option<RuleMeta>;

// src/linter/explain.rs
pub const LIST_RULES_JSON_VERSION: u32 = 1;
pub fn run(args: &ExplainArgs) -> i32;                 // 0, or 2 for unknown id / config error
pub fn list_rules(format: ListRulesFormat) -> i32;     // 0, or 2 on config error
pub fn all_rule_metas(config: &Config) -> Result<Vec<RuleMeta>, String>;  // built-ins, then custom
pub fn explain(rule_id: &str, metas: &[RuleMeta]) -> Result<String, String>;
pub fn format_explain(meta: &RuleMeta) -> String;
pub fn format_rule_list_text(metas: &[RuleMeta]) -> String;
pub fn format_rule_list_json(metas: &[RuleMeta]) -> String;

// src/linter/init.rs
pub fn run(args: &InitArgs) -> i32;                    // 0 written, 1 file exists, 2 I/O error
pub fn find_project_root(start: &Path) -> PathBuf;
pub fn init_project(start: &Path, args: &InitArgs, output: &mut String) -> i32;
pub fn starter_config() -> String;
```

`--list-rules --format json` (version 1):
```json
{ "version": 1, "rules": [ { "id": "class-name", "description": "...", "rationale": "...",
  "default_severity": "error", "fixable": false, "origin": "built-in", "path": null,
  "bad_example": "...", "good_example": "..." } ] }
```
`origin` is `"built-in"` or `"custom"`; `path` is the rule file (with `/`) for
custom rules. Keys are printed in alphabetical order (serde_json default).

## Decisions
- **`prefer-is-empty` is split into `prefer-is-empty` and
  `prefer-not-is-empty`.** A rule has one fix template, and `len(x) == 0`
  and `len(x) > 0` need different ones (`x.is_empty()` vs
  `not x.is_empty()`).
- **Safe fixes never change precedence.** `$X` is limited by a constraint to
  identifiers, attributes, calls, subscripts, node paths, literals and
  parenthesized expressions, so `len(a + b) == 0` is not reported (it would
  become `a + b.is_empty()`).
- **`no-get-node-in-process` has no fix.** The table asked for a suggestion,
  but the fix needs a new `@onready` variable at the top of the script, and
  an engine fix can only edit the matched code. The help text explains the
  change. It reports `get_node(...)` and `$Path` but not `%Unique` names,
  which Godot caches.
- **`no-godot3-yield` and `no-string-connect` keep strings.** Templates can't
  strip quotes, so the fixes use `Signal(obj, "sig")` and
  `Callable(target, "method")`, which are equivalent Godot 4 code. Only the
  two-argument `yield` and three-argument `connect` (with string signal and
  method) are reported, so Godot 4 calls like
  `connect("died", _on_died, CONNECT_ONE_SHOT)` are not.
- **`no-print-left` replaces the call with `pass`.** An edit can't delete the
  whole line (indent and newline are outside the match), and an empty block
  would not parse. `unnecessary-pass` then flags the leftover `pass` when the
  block has other statements.
- **`prefer-is-instance-valid` checks declared types in the same file.** It
  matches `$X != null` inside a file that declares `X` (`var X: T` or a
  parameter `X: T`) with `T` in a list of ~45 common node classes. It can't
  know your own classes or inferred types; it's `info` with a suggestion fix.
- **Gotcha found while writing it:** when a metavariable is bound twice (in
  the pattern, then in a `has`), `constraints` see the last binding. A
  `kind: identifier` constraint on `X` failed because the declaration's
  `name` node bound `X` too. Worth a line in `docs/custom_rules.md` (W4-B).
- **Built-in metadata:** `fixable` is true for `unnecessary-pass`,
  `no-else-return` and `unused-argument`, matching w3-a's handoff.
  `fix-failed` (A's new id) has metadata with no examples.
  `load-variable-name` has its own entry.
- **`explain` and `--list-rules` load the config from the current folder**
  (`api::load_config_for(".")`) to find custom rules. A config or rule-file
  error exits 2. They don't take `--config` (the frozen entry points take no
  path).
- **`--list-rules` text format changed** (W0 kept the old one until now):
  `  <id>  <severity>  fix  <description>[ (custom)]`, then
  `N rules (B built-in, C custom). Run `gdguard explain <rule>` for details.`
  The header `Available linting rules:` is kept.
- **`init`:** writes to the nearest folder with `project.godot` (else the
  current folder). It checks both files before writing anything, so a
  refusal leaves the folder untouched. Messages go to stdout on success and
  stderr on refusal. `gdguard.toml` lists every built-in rule with its
  default level and description (`max-line-length` in table form with
  `max = 100`), except `load-variable-name` (follows `variable-name`) and
  `fix-failed`; it also sets `prefer-is-empty = "warning"`, `[custom_rules]`,
  `[lint].exclude` and commented `baseline`/`[[overrides]]`. The example rule
  is `include_str!` of `examples/rules/prefer-is-empty.yml`, so it can't
  drift. A test loads the written config with `config::load_with_providers`
  and runs the rule's tests.

## Requests for the merge session
In `tests/cli_tests.rs` (B can't edit it), three tests are stale now:
- `unimplemented_subcommands_exit_with_code_2_and_name_the_command`: remove
  the `(&["explain", "unused-argument"], "explain")` and
  `(&["init", "--force"], "init")` cases.
- `list_rules_json_is_not_implemented_yet`: delete the test (covered by
  `tests/explain/main.rs::cli_list_rules_text_and_json_include_custom_rules`).
- `list_rules_text_output_is_unchanged`: update it for the new format; for
  example, assert the output starts with `Available linting rules:\n  class-name`
  and contains `\n  duplicated-load ` (rules are now sorted by id).

If w3-a adds or renames built-in diagnostic ids, add them to
`BUILTIN_RULES` in `rules_meta.rs`; `every_built_in_rule_has_metadata` only
sees `get_all_rule_names()`.

## Spec updates
Under "CLI", after the command list:

> `gdguard explain <RULE>` prints the rule's id, origin (built-in or the
> custom rule file), default severity, whether it is fixable, its
> description, rationale, and bad and good examples. An unknown id exits
> with 2 and suggests the closest id.
>
> `lint --list-rules` prints one line per rule (id, default severity, `fix`
> when fixable, description, `(custom)` for custom rules). With `--format
> json` it prints `{ "version": 1, "rules": [ ... ] }`, each rule with `id`,
> `description`, `rationale`, `default_severity`, `fixable`, `origin`
> (`built-in` | `custom`), `path` (rule file or `null`), `bad_example` and
> `good_example`. Both include the custom rules of the config found from the
> current folder.
>
> `gdguard init [--force]` writes `gdguard.toml` (every built-in rule with
> its default level, commented) and `.gdguard/rules/prefer-is-empty.yml` in
> the Godot project root (the nearest folder with `project.godot`, else the
> current folder). If either file exists it writes nothing and exits 1,
> unless `--force` is given.

Under "Custom YAML rules", add: "The starter rule pack is in
`examples/rules/`." and the constraints gotcha: "When a metavariable is
captured in several places (for example in the pattern and in a `has`),
`constraints` check the last capture."

## CHANGELOG lines
- Linter: `gdguard explain <rule>` shows a rule's rationale, examples, severity and fixability, for built-in and custom rules.
- Linter: `lint --list-rules` shows descriptions, severities and fixability, and includes custom rules; `--list-rules --format json` prints them as JSON.
- Linter: `gdguard init` sets up a project with a commented `gdguard.toml` and an example custom rule.
- Linter: starter custom rule pack in `examples/rules/`: `prefer-is-empty`, `prefer-not-is-empty`, `no-get-node-in-process`, `prefer-preload-const`, `no-godot3-yield`, `no-string-connect`, `no-print-left`, `prefer-is-instance-valid`.

## How to test manually
```sh
cargo run -- test-rules examples/rules            # 8 rules, all pass
cargo run -- explain no-else-return               # built-in
cargo run -- explain unused-arguments             # did you mean, exit 2
cargo run -- lint --list-rules
cargo run -- lint --list-rules --format json
cd "$(mktemp -d)" && touch project.godot && gdguard init && gdguard init   # second run exits 1
cargo test --test explain                         # explain, list-rules, init
cargo test --test custom_rules                    # pack, tester, provider, loader
```

## Known issues / follow-ups
- `explain` and `--list-rules` ignore `lint --config`; they use the config
  of the current folder.
- `prefer-is-instance-valid` only knows a fixed list of node classes.
- Rationale lines in `explain` aren't wrapped.
