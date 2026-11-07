# Handoff w4-b

## Summary
Claude Code integration: a POSIX `sh` PostToolUse hook that runs
`lint --fix --format json` on each edited `.gd` file and hands the remaining
issues back to Claude (exit 2), plus the settings snippet, a skill and an
AGENTS snippet. New docs: `docs/custom_rules.md` and `docs/agents.md`, and a
"Linter" quick start in `README.md`. Every command shown in the docs was run
and its output copied. Dogfooding on the sample project found one real CLI
bug (`lint` without a path reads stdin) and a few message problems; no false
positives from the rule pack, and every `--fix-unsafe` edit was correct.

## Tasks
| Task | Status | Notes |
|------|--------|-------|
| B8 hook `hooks/lint-gd.sh` | done | jq when present, sed fallback (`GDGUARD_HOOK_NO_JQ=1` forces it). |
| B8 `settings.example.json`, `skill/SKILL.md`, `AGENTS.snippet.md` | done | |
| B8 tests `tests/integrations/` | done | 8 tests; they return early with a note when `sh` (or, for one test, `jq`) is missing. |
| `docs/custom_rules.md` | done | Format, metavariables, `$` escape, every operator (each example is a tested rule), constraints, fixes, `test-rules`, debugging. |
| `docs/agents.md` | done | Loop, hook in 3 steps, JSON schema, CI. |
| README "Linter" section | done | Quick start and links; old formatter/linter content kept. |
| Dogfood sample project | done | See Known issues. |
| Dogfood the user's game | not done | No project path was given. |

## Files changed
- `integrations/claude-code/hooks/lint-gd.sh` (mode 100755), `settings.example.json`, `skill/SKILL.md`, `AGENTS.snippet.md`: new.
- `tests/integrations/main.rs`: new test target.
- `docs/custom_rules.md`, `docs/agents.md`: new.
- `README.md`: "Linting GDScript files" intro and "Basic linting" replaced by a "Linter" section (quick start, commands table, links); the stale output format `filepath:line:rule:severity` is gone. The other linter subsections are unchanged.

## Public API added
None in Rust. Hook interface:
- Input: Claude Code hook JSON on stdin; reads `tool_input.file_path`.
- Env: `GDGUARD` (binary, default `gdguard`), `GDGUARD_HOOK_NO_JQ=1`.
- Exit 0 and no output: clean after safe fixes, not a `.gd` file, file
  missing, or binary missing (then a one-line warning on stderr).
- Exit 2: issues remain (stderr: `gdguard: <file> still has issues after safe
  fixes (E errors, W warnings, I infos).`, one line per issue when jq is
  available, the `Fixed N problems` line, an instruction line, then the JSON
  report), or gdguard exited with 2 (stderr: `gdguard couldn't lint <file>
  (exit code 2):` and gdguard's error).

## Decisions
- **The hook runs on the file only**, from the current folder; gdguard finds
  `gdguard.toml` by searching upward from the file.
- **A missing binary doesn't block** the agent (exit 0 with a warning), so a
  project without gdguard installed stays usable.
- **"Fixed 0 problems" is filtered** out of the hook's stderr.
- **CI example uses `--format compact`**: `--format github` isn't on `main`
  yet (A's W4 work), and every doc example had to be run.
- **The `gdguard-ignore` examples** (skill, AGENTS snippet, agents.md) use
  the new prefix from the spec, because that's what agents should write.
  On `main` today only `gdlint-ignore` works; agents.md says the old prefix
  works too.
- **Doc outputs** are real; absolute temp paths are shortened to `<project>`
  and `\` is shown as `/`.

## Requests for the merge session
- After A's ignore comments v2 lands, check that
  `func _on_body_entered(body):  # gdguard-ignore unused-argument -- signal signature`
  suppresses the warning (it's the example in `docs/agents.md`, the skill and
  the AGENTS snippet). Today it doesn't, because `gdguard-ignore` isn't parsed yet.
- After `--format github` lands, change the CI example in `docs/agents.md`
  to `gdguard lint --format github .` and paste its real output.
- `--list-rules --format json` (B, `explain.rs`) prints
  `"origin": "built-in"`, but lint JSON and the spec use `"builtin"`. Please
  change `("built-in", ...)` to `("builtin", ...)` in
  `format_rule_list_json` and the matching assertions in
  `tests/explain/main.rs` (2 places). `explain.rs` isn't in w4-b's
  editable files, so I left it alone.
- When W8 renames the binary, replace the "run them as
  `gdscript-formatter`" note at the top of the README "Linter" section.

## Spec updates
Under "AI agent integration", add:

> The hook (`integrations/claude-code/hooks/lint-gd.sh`) reads
> `tool_input.file_path` from the hook JSON and, for an existing `.gd` file,
> runs `gdguard lint --fix --format json <file>`. It exits 0 silently when
> the file is clean (or not a `.gd` file, or gdguard isn't installed) and
> exits 2 with a summary and the JSON report on stderr when issues remain or
> gdguard reports a config error. `GDGUARD` sets the binary. Setup and the
> JSON fields are documented in `docs/agents.md`.

## CHANGELOG lines
- Linter: Claude Code integration in `integrations/claude-code/`: a PostToolUse hook that lints and fixes each edited `.gd` file and returns the remaining issues to Claude, a skill and an AGENTS.md snippet.
- Docs: `docs/custom_rules.md` (writing, fixing, testing and debugging custom rules) and `docs/agents.md` (agent loop, hook setup, JSON report, CI); README has a linter quick start.

## How to test manually
```sh
cargo test --test integrations
cargo build
D=$(mktemp -d) && printf 'config_version=5\n' > $D/project.godot
printf 'func f():\n\tprint(1)\n\tpass\n\tvar targetName = 1\n' > $D/a.gd
echo "{\"tool_input\":{\"file_path\":\"$D/a.gd\"}}" | GDGUARD=target/debug/gdscript-formatter sh integrations/claude-code/hooks/lint-gd.sh; echo $?   # 2, pass removed
```

## Known issues / follow-ups (dogfooding)
Run on a copy of `tests/e2e/sample_project` with the debug build at
`main` + this branch, with and without the 8 rules of `examples/rules/`.

1. **`gdguard lint` with no path reads stdin instead of linting the
   project** (spec: "default: whole project"). Repro: `cd
   tests/e2e/sample_project && gdscript-formatter lint` waits for input in a
   terminal; with an empty stdin (`gdscript-formatter lint </dev/null`) it
   prints nothing and exits 0, so a CI job would pass without checking
   anything. Owner: CLI (A). Docs currently tell people to pass `.`.
2. **Typo in the `loop-variable-name` message**: `Loop variable 'MyIndex'
   should be in snake_case_or _private_snake_case format` ("case_or"). Repro:
   `gdscript-formatter lint --format compact tests/e2e/sample_project/scripts/ui/hud.gd`.
   Owner: A (`src/linter/rules/loop_variable_name.rs`); it's also in
   `tests/e2e/expected/*.txt`.
3. **Mixed path separators in rule-file errors on Windows**:
   `Error: C:\...\demo\.gdguard/rules\prefer-signal-emit.yml: ...` (the config
   path uses `/`, the joined file name `\`). Repro: a rule file with an
   undefined metavariable in `message`, then `gdguard test-rules` on Windows.
   Owner: B (`custom/loader.rs`), not editable in W4; normalize with the same
   `display_path` the reporters use.
4. **`explain` shows absolute paths for custom rules**
   (`Origin: custom (C:/Users/.../.gdguard/rules/prefer-is-empty.yml)`).
   Relative to the config folder would read better. Owner: B (`explain.rs`).
5. **`--fix` always prints `Fixed 0 problems in 0 files`** on stderr when
   nothing was fixed. Harmless; the hook filters it. Owner: A.
6. **Pretty output** looked right (columns, carets, help and fix lines); no
   crash in any command (`lint`, `--fix`, `--fix-unsafe`, `--diff`,
   `--format json|compact|pretty`, `explain`, `--list-rules`, `test-rules`,
   `init`).
7. **Rule pack on the sample project**: no false positives. Hits were
   `no-get-node-in-process` (player.gd:14), `prefer-is-empty` (16),
   `prefer-not-is-empty` (28), `no-print-left` (player.gd:17, hud.gd:12).
   `--fix-unsafe` fixed 6 problems in 2 files (else removal, pass removal,
   `_delta`/`_source`, `is_empty()`, `not is_empty()`); all edits were
   correct and the files still parse.
8. **Not yet dogfooded on a real game.** Give the next session a project path
   to run on (read-only).
