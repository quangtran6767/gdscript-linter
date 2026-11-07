# W8 · Track B: rename, release, docs, upstream

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W8**, on branch **`w8-b`**.

**At the same time**, Track A is adding cross-file rules and performance work
in `src/linter/rules/`, `src/project/`, `src/semantic/` and `benchmarks/`.
Don't touch those.

## Goal

The tool ships under its final name, with release binaries, an addon and
complete docs.

## Tasks

1. **Rename** to the name recorded in `docs/handoff/w0.md` (`Tool name:`).
   Don't ask the user:
   - the Cargo package and binary name;
   - the gdextension library name;
   - the addon folder and `plugin.cfg`;
   - every user-facing string except the formatter's legacy CLI aliases.
     Keep a `gdscript-formatter` alias binary or document the migration.
   - **Only rename identifiers and strings.** Never move files that Track A
     is editing in this wave. List those renames for the merge session under
     "Requests".
2. **Release:**
   - update `.github/workflows/release.yml` and `src/bin/make_release.rs`
     for the new names;
   - attach the `.d.ts` and the example rules as release assets;
   - do a dry run locally if possible, and describe it.
3. **Docs:**
   - a final pass over the README and every `docs/*.md`;
   - a "Migrating from gdlint/gdtoolkit" page;
   - a FAQ, including type inference limits, Godot's own warnings, and how
     to write a rule;
   - every example re-run.
4. **Upstream:** write `docs/upstream_proposal.md`, listing which general
   improvements (spans, JSON output, fix engine) could go to GDQuest and how
   to split them into PRs. Don't open the PRs; the user decides.

## You may edit

- `Cargo.toml` (package/bin names), `gdextension/`, `addons/`
- `.github/`, `src/bin/make_release.rs`
- `README.md`, `docs/` (except the spec, plan and handoffs of others)
- `integrations/`, `examples/`

## Acceptance

- `cargo build --release` produces the new binary name.
- All tests pass.
- The CLI help shows the new name.
- `docs/handoff/w8-b.md` is written, with release dry-run notes.
