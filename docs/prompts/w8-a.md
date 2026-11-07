# W8 · Track A: cross-file rules and performance

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W8**, on branch **`w8-a`**.

**At the same time**, Track B is renaming the tool, preparing releases and
finishing the docs. Don't touch `release.yml`, `README.md` or `docs/`
(except your handoff), and don't do any renaming.

## Goal

Rules that use the whole project, plus measured, documented performance.

## Read

- Spec: "Project index", "Performance and caching".
- All `docs/handoff/w7-*.md` and `w7-merge.md`.

## Tasks

1. **P2, cross-file rules:**
   - `unused-signal`: a signal never emitted or connected anywhere in the
     project. Check `emit_signal("name")`, `name.emit()`,
     `name.connect(...)` and `connect("name", ...)`. Scene `.tscn` `[connection]`
     entries count as usage, so parse them minimally.
   - `unknown-member-on-project-class`: `x.member` where `x` has a declared
     type that is a project `class_name` and `has_member` returns `No`.
     Never report on `Unknown`.
   - Put their `RuleMeta` text under "Requests".
2. **Performance:**
   - add a lint benchmark to `src/bin/benchmark.rs` or `benchmarks/`;
   - make a generated 1000-file project (script in `benchmarks/`);
   - profile and fix the top hotspots;
   - targets: cold run under 3 s and warm cache under 1 s for 1000 files on
     a typical laptop. Record the real numbers.

## You may edit

- `src/linter/rules/`, `src/project/`, `src/semantic/`
- `src/linter.rs`, `src/linter/cache.rs`
- `src/bin/benchmark.rs`, `benchmarks/`
- `tests/lint/`, `tests/project/`

## Acceptance

- Fixture projects for both rules, including signals connected only in a
  `.tscn`, and an `Unknown` case that isn't reported.
- The benchmark runs with a single command, and the numbers are in the
  handoff.
- `docs/handoff/w8-a.md` is written.
