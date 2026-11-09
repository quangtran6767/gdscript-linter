# W10 fix-up: declaration-order review findings

Review of `6e72dbb` by the maintainer's reviewer. Fix all four, in a new
commit on `main` (don't amend, push or tag).

## 1. Safe fix moves a variable above a member it reads

```gdscript
extends Node

var d = 1
@export var v = d + 1
signal e
```

`lint --fix` (safe) produces `signal e`, `@export var v = d + 1`, `var d = 1`:
`v` is now initialized before `d`. Cause: `reorder_safety` skips items whose
plan index equals their source index (`v` is at index 1 in both), but a
dependency can still cross it.

Correct rule: the fix is **unsafe** when, for any variable `x` whose
initializer references a member variable `y` (bare identifier or `self.y`),
`y` is declared before `x` in the source but after `x` in the new order.
Check every pair, not only items whose index changed. Apply it to every
variable kind, including `static` and `@onready`. Being conservative is fine:
when unsure, mark the fix unsafe.

## 2. A bare identifier initializer is not safe

```gdscript
extends Node

var a = 1
@export var b = a
```

This is reported with a **safe** fix, but `b` moves above `a` and reads it
before it is set. Remove the `value.kind() == "identifier"` exception and the
comment that cites W10 for it. The example in `docs/prompts/w10.md` that
called this fine was wrong; fix that line too, and any test, doc or
`rules_meta.rs` text that repeats it.

## 3. Fix for files that aren't formatted

Today the fix exists only when `is_formatted` is true, so `lint --fix` doesn't
reorder in projects that don't use the formatter. Add the other approach
from `w10.md`:

- When the file is not formatted (or not formattable), build the new text by
  moving the **original source slices** of plan items, each with its attached
  comments and annotations (`child_indices_attached_before_declaration`,
  `..._after_declaration`, split inline `extends`), in plan order. Don't
  reformat anything inside a declaration.
- Spacing between moved items: keep it simple and deterministic. One blank
  line between items, two before and after functions and inner classes
  (matching `blank_lines_around_definitions` default 2), and keep text
  before the first item and after the last item unchanged.
- Keep the formatter path for formatted files, so formatted input still
  equals `gdguard --reorder-code` output.
- Files with `# fmt: off` regions still get no fix.
- The result must parse, re-lint without `declaration-order`, and keep the
  same set of top-level declarations (compare names/kinds in a test).
- Same safety rule as 1 and 2.
- Update the help text: it must no longer say "Format the file first".

Tests: unformatted file with mixed indentation inside functions keeps those
bodies byte-for-byte; comments and annotations move with their declaration;
`#region` groups; inline `class_name X extends Y`; trailing comment after the
last declaration stays.

## 4. Message precision

"variable `b` should come before variables" doesn't say why. Use the
specific kind for the moved item too: `exported variable`, `static variable`,
`onready variable`, `variable`. Example:
"exported variable `b` should come before variables". Update snapshots.

## Done when

- Tests for each example above (1 and 2 must assert `unsafe`), plus
  the item-3 tests.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace`, feature-off tests, release build pass.
- Manual: on temp copies, run `gdguard lint --fix` and `--fix-unsafe` on
  each example above and on an unformatted out-of-order file, then
  re-lint. Show the diffs in `docs/handoff/w10.md`.
- `docs/handoff/w10.md`, `docs/specification_linter.md`, `docs/rules.md`
  and `CHANGELOG.md` updated.
- Commit `[w10-solo] fix declaration-order safety and add unformatted fix`.
