# W5 · Track B: JS rule runtime

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W5**, on branch **`w5-b`**.

**At the same time**, Track A is implementing `SemanticModel` in
`src/semantic/`. Don't touch it. Code `context.semantic` against the frozen
signatures; it returns empty data until the merge.

## Goal

`.js` rules in the rules folders run in a safe sandbox with an
oxlint/ESLint-style API matching `integrations/types/gdguard.d.ts`, and
`test-rules` tests them.

## Read

- Spec: "Scripted rules in JS".
- `docs/handoff/sm0.md`, `integrations/types/gdguard.d.ts`, and the
  `rquickjs` docs.

## Tasks

1. **Runtime** (`src/linter/script/runtime.rs`), behind feature `js-rules`:
   - one QuickJS runtime per worker thread, never shared across threads;
   - a 64 MB memory limit and an interrupt handler with a time limit per file
     (default 2 s, configurable later);
   - no `import` or `require` (module loader disabled); `console.log` goes to
     stderr in verbose mode only.
2. **Loader:**
   - find `*.js` files in `custom_rules.paths`;
   - evaluate each one's `export default`;
   - validate `meta`, giving errors with the file path just like the YAML
     loader;
   - check ids for uniqueness across YAML, JS and built-in rules.
3. **Bindings** (`bindings.rs`):
   - `Node` wraps tree-sitter nodes lazily. Never copy the whole tree into
     JS.
   - `context`: `filename`, `source`, `options` (from config, as JSON),
     `report({ node | range, message, help, fix })`,
     `fix.replace`/`insertBefore`/`insertAfter`/`remove`, and `semantic`
     (calls into `SemanticModel`).
   - Visitor dispatch: call `create(context)` once per file, then call each
     handler only for the node kinds it subscribed to.
4. **Provider:** `JsRuleProvider` implements `RuleProvider`, and
   `custom::load_providers` returns it next to the YAML provider.
5. **`test-rules`:** run JS rules' `tests` the same way as YAML tests,
   including `output` and idempotency.
6. Write 2 example JS rules in `examples/rules/` (`max-function-params`,
   `no-empty-function`) with tests.

## You may edit

- `src/linter/script/`, `src/linter/custom/`, `src/linter/rule_tester.rs`
- `examples/rules/`
- new `tests/js_rules/`

## Acceptance

Tests for:
- report with a span;
- each fixer method;
- `options` from config;
- a syntax error in a rule file;
- a missing `meta`;
- a duplicate id;
- an infinite loop that is stopped by the time limit with a clear error;
- memory exhaustion that is stopped;
- `import` that is rejected;
- one runtime per thread (run the linter on several files in parallel).

`cargo build --no-default-features` still works without JS.

`docs/handoff/w5-b.md` is written, with the JS overhead per file measured.
