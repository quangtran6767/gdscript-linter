# FAQ

## What should an agent run after editing?

Run `gdguard lint --fix --format json PATH`, inspect every remaining issue's
`help`, `range` and `explain`, edit what needs judgment, then lint the project
again. Use exit 2 as a tool/config failure, never as evidence of clean code.
Fix output for disk files is JSON on stdout; fixes and status go to stderr.
See [agent integration](agents.md) for the JSON fields and hook setup.

## Does gdguard infer types or replace Godot warnings?

No type inference is performed. Cross-file member checks use explicit
project `class_name` types and declared inheritance. If a missing base,
cycle, engine base or dynamic accessor makes absence uncertain, no missing
member is reported. Engine completion, static type diagnostics and runtime
validation remain Godot's responsibility. The `parse-error` diagnostic
reports syntax the bundled Tree-sitter parser could not read; the parser
can lag Godot. Empty enums, accepted by Godot 4.7, are tolerated explicitly.

## Why is a signal reported as unused?

Within a Godot project, `unused-signal` checks script emit/connect calls and
`.tscn` connection entries. Matching is conservative by signal name across
the project, so unrelated signals with the same name can hide an unused
one. Dynamic emission prevents a definite unused report. Current-buffer
uses are merged with disk uses. Binary scenes, external consumers and
arbitrary runtime reflection cannot be proven: inspect the result and add
a reasoned ignore if required. No signal is deleted automatically.

Project-wide analysis supersedes `unused-local-signal` inside projects to
avoid duplicate or scene-blind warnings. Outside a project, or when
`unused-signal` is disabled, the local rule remains available.

## How do I add a rule?

Use a [YAML pattern](custom_rules.md) for syntax matches. Use a
[JavaScript visitor](js_rules.md) when the rule needs lexical symbols or
project classes/autoloads. Put files in a configured rule directory, include
valid/invalid test cases, and run `gdguard test-rules .gdguard/rules`.
[Examples](../examples/rules/) and the [type declarations](../integrations/types/gdguard.d.ts)
are included. JS has no OS/network/import APIs and has resource limits;
a runtime failure becomes an error diagnostic. Feature-off builds explicitly
reject configured JS rather than silently skipping checks.

## Will --fix change the meaning of code?

Only fixes labeled safe are automatic by default. Reparse checks roll back
invalid output; they do not prove runtime equivalence. Unsafe changes need
review before `--fix-unsafe`; suggestions require deliberate edits. Keep
version control and inspect diffs. A parser error has no automatic fix.

## Can I use it without an editor plugin?

Yes. CLI and agent loops are the primary workflow. LSP and Godot native
integrations are optional compatibility features; see [editor notes](editors.md).
