# Custom YAML rules

Put one rule in each `.yml` or `.yaml` file and register the folder in
`gdguard.toml` (paths are relative to that config):

```toml
[custom_rules]
paths = [".gdguard/rules"]

[rules]
prefer-is-empty = "warning"
```

Run `gdguard init` to create
a starter config and rule. Existing formatter commands still work.

## First rule: patterns, alternatives and constraints

Save this as `.gdguard/rules/prefer-is-empty.yml`:

```yaml
id: prefer-is-empty
severity: warning
message: "Use `$X.is_empty()`"
help: "is_empty() states the collection check directly."
rationale: "A named collection check is easier to read than comparing a length."
rule:
  any:
    - pattern: len($X) == 0
    - pattern: len($X) < 1
constraints:
  X:
    kind: identifier
fix: $X.is_empty()
fixSafety: safe
tests:
  invalid:
    - code: "var empty = len(items) == 0\n"
      output: "var empty = items.is_empty()\n"
      errors: 1
  valid:
    - "var empty = items.is_empty()\n"
    - "var empty = len(items) == 1\n"
```

Run `gdguard test-rules .gdguard/rules`, then
`gdguard lint --fix .`. The tests below run every complete YAML
example in this page with the actual rule tester.

Required fields are `id`, `message` and `rule`. IDs are unique kebab-case,
including across built-ins. `severity` defaults to `warning`; `info` and
`error` are also accepted. `help`, `rationale`, `fix`, `fixSafety` and `tests`
are optional. Unknown fields and invalid patterns are errors with a path.
Folders are read in sorted order, without recursion.

`$X` matches and captures one node; `$$$ARGS` captures zero or more;
`$_` and `$$$` do not capture. Names use uppercase letters, digits and `_`.
Captures interpolate into `message`, `help` and `fix`; multiple-node
captures retain separators. When captured more than once, constraints
check the last capture.

GDScript node paths such as `$NodePath`, `$Player`, `$player` and
`$"UI/Label"` stay literal. Escape an all-caps node path as `\$HUD` in YAML
plain or single-quoted strings. In double-quoted YAML use `\\$HUD`.
String, string-name, node-path and unique-name literals match their text.

## Rule operators

`pattern` matches syntax, rather than raw text. Its object form accepts
`context` (a parseable snippet), `selector` (the root node kind to match),
and `strictness` (the ast-grep pattern strictness). `kind` selects a
tree-sitter node kind; `regex` filters a node's complete source text.
Several keys on a rule are combined with AND. `all` combines every child,
`any` accepts at least one, and `not` excludes a match.

This combines `all`, `kind`, `regex` and `not`:

```yaml
id: no-debug-name
message: "Remove the debug variable"
rule:
  all:
    - kind: identifier
    - regex: ^debug_
  not:
    regex: ^debug_allowed$
tests:
  invalid:
    - "func f():\n\treturn debug_value\n"
  valid:
    - "func f():\n\treturn debug_allowed\n"
```

`inside` checks an ancestor. It checks the immediate parent by default;
`stopBy: end` searches all ancestors. This flags calls inside `_process`:

```yaml
id: no-process-print
message: "Avoid printing every frame"
rule:
  pattern: print($$$ARGS)
  inside:
    kind: function_definition
    has:
      field: name
      regex: ^_process$
    stopBy: end
tests:
  invalid:
    - "func _process(delta):\n\tprint(delta)\n"
  valid:
    - "func _ready():\n\tprint(1)\n"
```

`has` checks a child (`stopBy: end` searches descendants). `field` limits a
relational matcher to a named grammar field, such as the function `name`
above. This finds functions containing a print:

```yaml
id: function-with-print
message: "Review this function's logging"
rule:
  kind: function_definition
  has:
    pattern: print($$$ARGS)
    stopBy: end
tests:
  invalid:
    - "func f():\n\tprint(1)\n"
  valid:
    - "func f():\n\tpass\n"
```

`follows` and `precedes` inspect siblings: the previous or next sibling by
default, or search farther with `stopBy: end`. Select an
`expression_statement` when comparing statements; a call's siblings are
parts of that expression, rather than other statements.

```yaml
id: free-after-emit
message: "Review freeing the emitter"
rule:
  pattern:
    context: queue_free()
    selector: expression_statement
  follows:
    pattern:
      context: emit_signal($$$ARGS)
      selector: expression_statement
tests:
  invalid:
    - "func f():\n\temit_signal(\"done\")\n\tqueue_free()\n"
  valid:
    - "func f():\n\tqueue_free()\n"
```

```yaml
id: emit-before-free
message: "Review this final emission"
rule:
  pattern:
    context: emit_signal($$$ARGS)
    selector: expression_statement
  precedes:
    pattern:
      context: queue_free()
      selector: expression_statement
tests:
  invalid:
    - "func f():\n\temit_signal(\"done\")\n\tqueue_free()\n"
  valid:
    - "func f():\n\temit_signal(\"done\")\n"
```

Each top-level rule needs a positive matcher (`pattern`, `kind`, `regex`,
`all` or `any`). The same operators work in `constraints` keyed by capture
name without `$`. Relational rules accept `stopBy` (also a rule object) and
`field`. Unsupported ast-grep features include `utils`, `matches`,
`nthChild`, `range`, `transform` and `rewriters`.

## Fixes and tests

`fix` replaces the matched node. Multiline templates omit surrounding
indentation: each later line receives the match line's indentation.
`fixSafety` is `safe`, `unsafe`, or `suggestion` (the default). A safety
without a fix is invalid. `--fix` applies safe fixes; `--fix-unsafe` also
applies unsafe ones; suggestions always require review. Use `--diff` to
preview safe fixes. Fixes that break parsing are rejected as `fix-failed`.

`tests.valid` contains code strings; invalid cases are code strings or
objects with `code`, optional `errors` (exact count), and optional `output`.
Without `errors`, at least one issue must be reported. `output` checks the
fix of any safety level, parsing, and idempotency. Other rules do not affect
that rule's tests. `test-rules` exits 0 on success, 1 for failing cases,
and 2 when loading rules fails.

The [example pack](../examples/rules/) includes eight tested rules. Run
`gdguard test-rules examples/rules` from this repository.

## Debugging a rule

1. Run `test-rules` on a small rule folder; include the failing source as an
   invalid case. A load error names the file and key to fix.
2. Start with `kind` or a simple `pattern`, then add constraints. Check the
   node kind in the [GDScript grammar](https://github.com/PrestonKnopp/tree-sitter-gdscript).
3. If a relational rule misses code, check its root selector, grammar
   field, and `stopBy`. Constraints see the last capture of a repeated name.
4. Use `lint --format json` to inspect byte ranges and fixes. Use
   `explain <id>` and `lint --list-rules --format json` from the configured
   project directory to verify loading. These catalog commands use the
   current directory's config and show default severities.
5. Keep a negative case for every near miss, and an `output` case for each
   fix. Prefer suggestions when safety depends on runtime types.
