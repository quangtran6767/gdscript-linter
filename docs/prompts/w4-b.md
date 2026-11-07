# W4 · Track B: AI agent integration and docs

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W4**, on branch **`w4-b`**.

**At the same time**, Track A is building ignore comments v2, the baseline,
the GitHub/SARIF output and the cache in `src/linter/`. Don't touch those
files.

## Goal

An AI agent such as Claude Code can lint, auto-fix and then fix the rest by
itself in a loop. Users can learn the whole feature set from the docs.

## Read

- Spec: "AI agent integration", "JSON output", "CLI".
- All `docs/handoff/w3-*.md` and `w3-merge.md`.

## Tasks

1. **B8, Claude Code integration** in `integrations/claude-code/`:
   - `hooks/lint-gd.sh`: a PostToolUse hook for `Edit|Write|MultiEdit`. It
     reads the hook JSON from stdin, and when the edited file ends in `.gd`
     it:
     - runs `gdguard lint --fix --format json <file>`;
     - returns nothing if the file is clean;
     - otherwise exits 2 with a short summary plus the JSON on stderr, so
       Claude sees the remaining issues and fixes them.
     Use POSIX sh plus `jq` if present, with a fallback that works without
     `jq`.
   - `settings.example.json`: the hook registration to paste into
     `.claude/settings.json`.
   - `skill/SKILL.md`: a skill that teaches the loop. Lint with `--fix`, then
     read `help` and `fixes`, call `gdguard explain <rule>` when unsure,
     re-lint until exit 0, and never add ignore comments without a
     `-- reason`.
   - `AGENTS.snippet.md`: the same loop in a few lines, for any agent.
   - Tests in `tests/integrations/`:
     - run the hook script with a sample hook JSON on a dirty and a clean
       file, and check the exit code and output;
     - mark them ignored when `sh` isn't available.
2. **Docs:**
   - `docs/custom_rules.md`: the YAML format, metavariables and the
     `$NodePath` escape, every `rule:` operator with an example, fixes and
     safety, `test-rules`, and a debugging guide.
   - `docs/agents.md`: the hook setup in 3 steps, the JSON schema, the
     recommended loop, and CI usage.
   - Take over `README.md`: add a "Linter" section with a quick start
     (`init`, `lint`, `--fix`) and links to the docs. Keep the formatter
     content.
3. **Dogfood:**
   - Run the current build on `tests/e2e/sample_project`.
   - If the user gives you a path to their real game project, run on that
     too, without changing their files.
   - Record false positives, crashes and confusing messages under "Known
     issues" with exact repro steps.

## You may edit

- `integrations/`, `docs/custom_rules.md`, `docs/agents.md`, `README.md`
- `examples/rules/` (fixes for problems found while dogfooding)
- `tests/integrations/`

## Acceptance

- Hook tests pass.
- Every code example in the docs was actually run, and the output shown is
  real.
- `docs/handoff/w4-b.md` is written. "Known issues" holds the dogfooding
  results.
