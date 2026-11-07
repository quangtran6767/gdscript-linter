# W3 · Track A: fix engine

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W3**, on branch **`w3-a`**.

**At the same time**, Track B is writing the starter rule pack, the `explain`
command, built-in rule metadata and `init`. Don't touch
`src/linter/custom/`, `explain.rs`, `rules_meta.rs`, `init.rs` or
`examples/`.

## Goal

`--fix`, `--fix-unsafe`, `--diff` and `--stdin-filename` work safely for
built-in and custom rules, and some built-ins gain safe fixes.

## Read

- Spec: "Fix engine", "CLI", "Library API".
- All `docs/handoff/w2-*.md` and `w2-merge.md`.

## Tasks

1. **A4, engine** (`src/linter/fix.rs`):
   1. Collect fixes allowed by the safety level (`--fix` means Safe;
      `--fix-unsafe` means Safe and Unsafe).
   2. Sort them by start, and skip any fix that overlaps one already chosen.
      Skipped fixes are retried on the next pass.
   3. Apply them with `apply_edits`, then re-parse. If the tree has an
      `ERROR` or `MISSING` node, find the bad fix by re-applying the pass's
      fixes one at a time. Drop it and report it as a `fix-failed`
      diagnostic (Warning, with the rule id in the message).
   4. Re-lint and repeat, for at most 10 passes, stopping early when nothing
      changes.
   5. If the original file was already formatted (formatting it gives the
      same text), run the formatter on the result. Otherwise leave
      formatting alone.
   - Fill in `api::fix_source`.
2. **CLI:**
   - `--fix` and `--fix-unsafe` write files, then report what's left with
     the normal reporters, and print "Fixed N problems in M files".
   - `--diff` prints a unified diff (move the `similar` crate from
     dev-dependencies to dependencies) and writes nothing.
   - `--stdin-filename <PATH>` reads the source from stdin. Combined with
     `--fix`, it writes the fixed source to stdout.
3. **A6, built-in fixes:**
   - `unnecessary-pass`: Safe, remove the `pass` line;
   - `no-else-return`: Safe, unindent the else body and remove the `else:`;
   - any other built-in where a fix is obviously behavior-preserving.

## You may edit

- `src/linter/fix.rs`, `src/linter.rs`, `src/linter/rules/`
- `src/linter/api.rs` (bodies), `src/linter/output/`
- `src/cli.rs`, `src/main.rs`
- `Cargo.toml` (`similar` only)
- `tests/lint/`, `tests/cli_tests.rs`, new `tests/fix/`

## Acceptance

`tests/fix/` covers:
- a single fix;
- two overlapping fixes, which take two passes;
- a fix that breaks parsing, which is rolled back and reported;
- idempotency: `--fix` twice equals `--fix` once;
- the formatter step on formatted and unformatted input;
- `--diff` writing nothing;
- stdin with `--fix`.

Fixture tests show the expected output of each new built-in fix.

`docs/handoff/w3-a.md` is written.
