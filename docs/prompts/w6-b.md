# W6 · Track B: JS project API, types, examples, docs

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W6**, on branch **`w6-b`**.

**At the same time**, Track A is building semantic built-in rules and the
`ProjectIndex` in `src/project/` and `src/linter/rules/`. Don't touch those.
Code against the frozen `ProjectIndex` signatures.

## Goal

JS rules can use project-wide data. The JS rule experience is documented and
has good examples.

## Read

- Spec: "Scripted rules in JS", "Project index".
- `docs/handoff/w5-*.md` and `w5-merge.md`.

## Tasks

1. **`context.project`** in `src/linter/script/bindings.rs`: `classNames`,
   `autoloads` and `hasMember(className, member)` returning
   `"yes" | "no" | "unknown"`. It's `null` when there's no `project.godot`.
2. **Finish `integrations/types/gdguard.d.ts`:** add JSDoc examples on the
   main members. Only additive changes; if you need a breaking change, stop
   and ask.
3. **Example JS rules** in `examples/rules/`, each with tests:
   - `require-typed-signal-params` (signal parameters must have types);
   - `no-autoload-mutation-outside-owner` (assigning `AutoloadName.x = …`
     outside the autoload's own script);
   - `prefer-static-typing` (a `var` without a type or `:=`, with an option
     for `allow_inferred`).
4. **`docs/js_rules.md`:**
   - quick start;
   - the full API reference (generated from or kept in sync with the
     `.d.ts`);
   - the visitor model and node kinds (how to find a kind: add a hidden
     `gdguard debug-tree <file>` command in `explain.rs`, or document the
     tree-sitter playground);
   - the sandbox limits;
   - testing;
   - three walk-throughs.

## You may edit

- `src/linter/script/`, `src/linter/explain.rs`, `src/linter/rules_meta.rs`
  (also add A's new rule metadata from `w5-merge.md` or A's requests, if
  they're already merged)
- `examples/rules/`, `integrations/types/`, `docs/js_rules.md`
- `tests/js_rules/`

## Acceptance

- Tests for `context.project` with a fixture project and with no project.
- Every example rule passes `test-rules`.
- Every code block in `docs/js_rules.md` was actually run.
- `docs/handoff/w6-b.md` is written.
