# W5 · Track A: semantic model

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W5**, on branch **`w5-a`**.

**At the same time**, Track B is building the JS rule runtime in
`src/linter/script/`. B codes against the frozen `SemanticModel` signatures,
so don't change them.

## Goal

`SemanticModel::build` correctly describes scopes, symbols and references for
any GDScript file, and the pipeline builds it once per file.

## Read

- Spec: "Semantic model".
- `docs/handoff/sm0.md`, plus the tree-sitter-gdscript grammar node kinds in
  `src/node_kind.rs`.

## Tasks

1. **`src/semantic/builder.rs`, a single tree walk that builds:**
   - **Scopes** for class, inner class, function, lambda, block (`if`,
     `elif`, `else`, `while`), `for` and `match` branch.
   - **Symbols:**
     - `var`, `const`, `signal`, `func`, `enum` and its members, inner
       `class`;
     - parameters, including default values and types;
     - the `for` variable;
     - the match pattern bindings (`var x`);
     - each with `declared_type` (annotation text), `annotations` and
       `is_static`.
   - **References:** every identifier use, classified as Read, Write (`=`,
     compound assignment) or Call. Resolve each by lexical lookup.
     - Class members are visible in methods. Inner classes do not see the
       outer class's instance members.
     - Lambdas capture their outer scope.
     - `self.x` resolves to the class member `x`.
     - Anything else unresolved stays `None`.
   - `scope_at(byte)` uses a sorted range search, not a linear scan.
2. **Pipeline:** build the model once per file in `src/linter.rs` and pass
   `Some(&model)` in `FileContext`. Skip building it only when no enabled
   built-in rule needs it **and** no custom provider is loaded. Don't change
   the `RuleProvider` trait. If you think it needs a `needs_semantic()`
   method, write that under "Requests".
3. **Speed:** the model build must add under 30% to lint time on
   `tests/lint/input` and `tests/input`. Measure it and report the numbers.

## You may edit

- `src/semantic/`, `src/linter.rs`
- new `tests/semantic/` (fixtures `*.gd` plus expected `debug_dump` output)

## Acceptance

At least 15 semantic fixtures, covering:
- shadowing in nested blocks;
- lambdas capturing outer variables;
- an inner class not seeing outer instance members;
- `self.x`;
- match bindings;
- `for` variables;
- compound assignment;
- static functions;
- signals used in `emit`/`connect`;
- `@onready`/`@export`;
- unresolved inherited members;
- an empty file;
- a file with parse errors, which must not panic.

`docs/handoff/w5-a.md` is written, with the timing numbers.
