# JavaScript rules

Register a folder in `[custom_rules].paths` in `gdguard.toml`. Its `.js`
files load alongside YAML rules, with unique kebab-case IDs across both and
built-ins. The current executable is `gdscript-formatter`; `gdguard` is the
planned final name. JS support is enabled by default; a build made with
`--no-default-features` reports an error if JS rules are configured.

## Quick start

Save the following as `.gdguard/rules/no-debug-print.js`:

```js
/** @type {import("gdguard").Rule} */
export default {
  meta: {
    id: "no-debug-print",
    description: "Review debug printing before shipping.",
    rationale: "Debug logs can obscure useful messages in a released game."
  },
  create(context) {
    return {
      expression_statement(node) {
        const call = node.namedChildren[0];
        if (call?.type !== "call" || call.namedChildren[0]?.text !== "print") return;
        context.report({
          node,
          message: "Review this debug print",
          help: "Remove the print after checking its arguments for side effects.",
          fix: fixer => fixer.replace(node, "pass"),
          fixSafety: "suggestion"
        });
      }
    };
  },
  tests: {
    valid: ["func f():\n\tpass\n"],
    invalid: [{code: "func f():\n\tprint(1)\n", output: "func f():\n\tpass\n", errors: 1}]
  }
};
```

Run `gdscript-formatter test-rules .gdguard/rules`, then
`gdscript-formatter lint --format json .`. Suggestions are reviewed by a
person or agent; `--fix` does not apply them. The complete rule above is
extracted and run by the documentation test.

For editor completion, reference
[`integrations/types/gdguard.d.ts`](../integrations/types/gdguard.d.ts).
The JSDoc `import("gdguard")` is a type reference; runtime imports and
TypeScript source files are not supported. Use `Rule<{max?: number}>` to
type custom options, as the max-function-params example does.

## API reference

The declaration file is the frozen source of truth. This page uses its
names and units.

| API | Members and behavior |
| --- | --- |
| `Rule<Options>` | Default export with `meta`, `create(context)` and optional `tests` |
| `RuleMeta` | Required `id`, `description`; optional `severity` (warning), `rationale`, `fixable` (false), `fixSafety` (suggestion) |
| `RuleContext<Options>` | Read-only absolute `filename`, `source`, effective `options`, `fix`, nullable `semantic`/`project`; `report(descriptor)` |
| `Node` | `type`, exact `text`, `range`, nullable `parent`, `children`, `namedChildren`, `field(name)` (nullable) |
| `Position` | 1-based `line`/UTF-8-byte `column`, 0-based absolute `byte` |
| `Range` | Inclusive `start`, exclusive `end`, each a Position |
| `ReportDescriptor` | Exactly one of `node`/`range`; required `message`; optional `help`, `fix`, `fixSafety`, `fixDescription` |
| `Fixer` | `replace(target,text)`, `insertBefore(target,text)`, `insertAfter(target,text)`, `remove(target)` return Edit; target is Node or Range |
| `Edit` | `range`, `replacement`; a report's fix can be one edit, an array, or a callback returning either (or null) |
| `Semantic` | `symbols`, `scopeAt(byte)`, `lookup(scope,name)` returning symbol ID/null, `referencesTo(symbol)` |
| `Symbol` | `id`, `name`, `kind`, `scope`, `nameSpan`, `declarationSpan`, nullable `declaredType`, `annotations`, `isStatic` |
| `Reference` | `range`, `name`, `kind` (read/write/call), nullable `symbol` ID |
| `Project` | Optional absolute `root`, sorted `classNames`, `autoloads` mapping names to project-relative paths, `hasMember(className,member)` returning yes/no/unknown |
| `RuleTests` | `valid` strings; `invalid` strings or objects with `code`, optional exact `errors`, optional fixed `output` |

Symbol kinds are `variable`, `constant`, `parameter`, `loop_variable`,
`function`, `signal`, `enum`, `enum_member`, `inner_class`. A compound
assignment produces a read and a write. `declaredType` contains annotation
text; no inferred type is promised. Unresolved names can be engine members,
autoloads or dynamic values and must not be treated as errors by themselves.

`source` is a JS string, so JS indices count UTF-16 units. Source ranges
and edits use UTF-8 bytes. Use node ranges and the fixer rather than
converting a string index directly. Reports/edits are validated for file
bounds, UTF-8 boundaries and non-overlapping changes.

`fixSafety` on a report overrides metadata. Safe fixes run under `--fix`;
unsafe fixes require `--fix-unsafe`; suggestions never run automatically.
Fix output is checked for parsing and embedded output tests also require
idempotency. Config levels/ignores/baselines work the same as for YAML.

## Visitor model and node kinds

`create` is called once per rule/file. Its returned object maps exact
node-kind strings to synchronous handlers. A handler is called only for
subscribed kinds, in tree order. Nodes query the native tree lazily; retain
neither nodes nor file context across checks.

Use [the pinned GDScript grammar](https://github.com/PrestonKnopp/tree-sitter-gdscript/tree/872e2e96905892c954da4101c74b884d0155f4d8)
(`src/node-types.json` lists fields), or the tree-sitter playground with that
grammar, to identify kinds. A declaration's `name` field is a `name` node;
identifier *uses* are usually `identifier`. Signals have `parameters`;
typed parameters have `type`. Attribute access/calls are separate from
simple calls. Always handle missing fields, particularly on broken source.

## Sandbox and errors

Each worker/config owns its QuickJS runtime, never shared across threads.
Limits: 64 MB JS heap, 512 KB stack, 2 seconds per file and per rule-load
evaluation. No filesystem, network, module loader or require API is
installed. `console.log` is quiet by default; set `GDGUARD_VERBOSE=1` to
write logs to stderr without corrupting JSON/LSP stdout.

Invalid metadata/syntax/duplicate IDs include the rule path and stop loading
(exit 2). Handler failures, bad ranges/edits or resource exhaustion report
`script-error` (Error) in the file. Repair the rule before trusting that
file's lint result. Rule code is not an async workflow: visitors must be
synchronous and return their reports during the check.

## Three walk-throughs

1. **Parameter limits:** [max-function-params.js](../examples/rules/max-function-params.js)
   subscribes to `function_definition`, reads named parameter children,
   compares with `context.options.max`, and reports on the function name.
   Config options and per-folder overrides reach the same context. Its
   embedded positive/negative cases run without unrelated built-in rules.
2. **Typed signals and declarations:**
   [require-typed-signal-params.js](../examples/rules/require-typed-signal-params.js)
   visits signal signatures and finds missing type fields.
   [prefer-static-typing.js](../examples/rules/prefer-static-typing.js)
   accepts `:=` by default, or requires annotations with
   `allow_inferred = false`. A semantic rule can additionally inspect
   `semantic.symbols` and `referencesTo(id)` without rebuilding the model.
3. **Autoload ownership:**
   [no-autoload-mutation-outside-owner.js](../examples/rules/no-autoload-mutation-outside-owner.js)
   visits assignment/compound assignment, finds an autoload receiver using
   `project.autoloads`, and compares the current filename with the owner
   path. It returns quietly when `project` is null. Project fixture tests
   cover actual mutation and the owner exemption; ordinary embedded tests
   verify the no-project case.

## Testing and debugging

Run `gdscript-formatter test-rules examples/rules` from this repository.
All 13 example rules pass. `output` tests apply fixes at every safety level,
require the expected text to parse, and re-run the owning rule to verify no
issue remains. A wrong count/output or repeated diagnostic fails (exit 1).

Reduce a rule to one subscription and an embedded case, then inspect node
`type`, `text` and fields. Check `lint --list-rules --format json` or
`explain <id>` from the configured directory to confirm loading. Runtime
errors name the rule; syntax/metadata errors name the file. Project data
comes from cached, content-hashed on-disk scripts; malformed scripts are
excluded from resolvable class names, and inheritance cycles stay unknown.
