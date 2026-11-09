# W5-B: JavaScript rule runtime (solo)

Implemented `.js` loading, metadata validation, lazy node/fixer/semantic
bindings, per-file limits, visitor dispatch and embedded rule testing.
The two example rules and their tests pass alongside the eight YAML rules.

## Files and API

- `src/linter/script/{mod,runtime,bindings}.rs`, `bridge.js`: sandbox,
  provider, validation and native lazy queries.
- `src/linter/custom/`: mixed discovery, ID validation across sources;
  reserved engine IDs are now protected in YAML as well as JS.
- `src/linter/rule_tester.rs`: embedded JS tests, output and idempotency.
- `src/linter/cache.rs`: JS content contributes to rule hashes; cache
  version 2 invalidates older engine results.
- `examples/rules/{max-function-params,no-empty-function}.js`.
- `tests/js_rules/`: report/range/fix methods, callback fixes, options and
  overrides, real semantic lookup, invalid modules/metadata/duplicate IDs,
  time/memory limits, invalid UTF-8 ranges, parallel determinism and tests.

```rust
pub fn custom::find_js_files(folders: &[PathBuf]) -> Result<Vec<PathBuf>, String>;
pub struct JsRule { pub meta: RuleMeta, pub tests: RuleTests }
impl JsRuleProvider {
    pub fn load(config: &Config, files: &[PathBuf], existing: &[RuleMeta]) -> Result<Self, String>;
    pub fn check_rule(&mut self, index: usize, context: &FileContext) -> Result<Vec<Diagnostic>, String>;
}
pub fn script::test_rules(provider: JsRuleProvider) -> Vec<RuleReport>;
```

The provider implements unchanged `RuleProvider`. The full JS API remains
the frozen `integrations/types/gdguard.d.ts`.

## Decisions

- Each worker/config owns its runtime, never shared across threads. This
  follows the existing per-config worker provider cache.
- Heap limit 64 MB, stack 512 KB, default file/load deadline 2 seconds.
  No module loader or filesystem/network/require APIs are installed.
- Tree handles are cheap Rust clones; JS properties query individual
  native nodes lazily using child paths. No tree graph is copied into JS.
- Runtime/invalid-report failures produce `script-error` (Error); load
  failures return path-qualified errors. Rule options include overrides.
- `GDGUARD_VERBOSE` enables console.log on stderr; otherwise it is quiet.
- Feature-off builds reject configured JS files explicitly. YAML-only
  behavior continues to work. Feature-specific example CLI tests are gated;
  the YAML capture test now constructs its YAML provider directly.

## Overhead

The release benchmark, 500 checks of sample player.gd with providers reused:
113.4 us/file for model + built-ins, 432.2 us/file including both example
JS rules; extra 318.8 us/file. Cold loading costs are separate.

## Manual checks

`cargo run -- test-rules examples/rules` reports 10 rules passing.
`cargo test --test js_rules`; `cargo build --no-default-features`.

## Requests / follow-ups

C4 semantic wiring and script-error metadata were completed directly in
solo integration. W6 adds actual project data (the existing bridge returns
null until the pipeline supplies it), three more examples and the full JS
guide. No frozen contracts were changed.
