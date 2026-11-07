# W4 · Track A: ignore comments v2, baseline, CI reporters, speed

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W4**, on branch **`w4-a`**.

**At the same time**, Track B is building the Claude Code integration and
writing user docs. Don't touch `integrations/`, `README.md` or
`docs/custom_rules.md` / `docs/agents.md`.

## Goal

Make the linter adoptable on large existing projects and in CI.

## Read

- Spec: "Inline ignore comments", "Baseline", "JSON output", "Performance and
  caching".
- All `docs/handoff/w3-*.md` and `w3-merge.md`.

## Tasks

1. **A7, ignore comments v2** (`src/linter/ignore_patterns.rs`):
   - `gdguard-ignore-file <rules>`;
   - a `-- reason` suffix;
   - the legacy `gdlint-ignore*` prefixes still work;
   - an `unused-ignore` warning (a built-in rule with metadata; send its
     `RuleMeta` text under "Requests" because B owns `rules_meta.rs`);
   - ignores apply to custom-rule diagnostics too.
2. **A8, baseline** (`src/linter/baseline.rs`):
   - `--update-baseline` writes `.gdguard/baseline.json`, sorted and stable
     so diffs are small;
   - `--baseline <PATH>` (or `[lint].baseline`) hides matching issues;
   - fingerprint: `rule`, `file`, trimmed line text, and the occurrence index
     of that text in the file;
   - the summary says "N issues hidden by baseline".
3. **A9, reporters:**
   - `github`: `::error file=…,line=…,col=…,endLine=…,endColumn=…,title=rule::message`;
   - `sarif` 2.1.0, with rules in `tool.driver.rules` (from `RuleMeta`) and
     columns converted to UTF-16 units.
4. **Speed:**
   - lint files in parallel with `std::thread::scope` (as many workers as
     available cores);
   - `src/linter/cache.rs`: `.gdguard/cache/lint.json` keyed by file hash,
     config hash, rules hash and tool version, so unchanged files are
     skipped;
   - `--no-cache`.

## You may edit

- `src/linter/ignore_patterns.rs`, `baseline.rs`, `cache.rs`
- `src/linter/output/`, `src/linter.rs`, `src/linter/rules/`
- `src/cli.rs`, `src/main.rs`
- `tests/lint/`, `tests/cli_tests.rs`, new `tests/baseline/`

## Acceptance

- Tests for:
  - every ignore form, plus `unused-ignore`;
  - a baseline that survives inserted lines above the issue;
  - a new duplicate issue on the same line text that isn't hidden;
  - a GitHub format snapshot;
  - a SARIF snapshot, with a test that checks the SARIF structure;
  - a cache hit and a miss after editing a file, the config or a rule;
  - parallel output that is deterministic (same order every run).
- `docs/handoff/w4-a.md` is written, with a before/after timing on
  `tests/e2e/sample_project` and on the repo's test inputs.
