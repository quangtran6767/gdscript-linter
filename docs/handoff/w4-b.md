# Handoff w4-b (completed solo on w4-a)

## Summary

Completed the missing agent integration and user documentation on the
existing branch. The user now works alone; no partner branch is needed.

## Tasks

| Task | Status | Notes |
| --- | --- | --- |
| B8 hook, settings, skill, snippet | done | POSIX sh, jq/Python fallback, real CLI tests |
| User docs and README | done | Configuration, custom rules, agent loop, CI |
| Dogfood | done | Sample project, fixes only on temp copies; no real-game path supplied |

## Files changed

- `integrations/claude-code/`: hook, settings, skill and agent instructions.
- `docs/custom_rules.md`: supported YAML operators, captures, node paths,
  safety, tests and debugging; six complete executable rule examples.
- `docs/configuration.md`: discovery, levels, overrides, ignores, baseline,
  reporter selection, cache and exit status.
- `docs/agents.md`: setup, JSON fields/units, recommended loop and CI.
- `README.md`: linter quick start and links, preserving formatter content.
- `tests/integrations/`: five real-hook tests plus documentation rule tests.

## Public API added

- Hook stdin: JSON `tool_input.file_path` (fallback `tool_input.path`).
- Hook environment: optional `GDGUARD_BIN` executable path. Default lookup
  tries `gdguard`, then `gdscript-formatter`.
- Hook exit 0: silently clean/skipped. Exit 2: remaining diagnostics or tool
  error on stderr. stdout is always empty.

## Decisions

- Use Python 3's JSON parser when jq is unavailable; no regex path decoding.
- Document the current executable name explicitly; W8 owns final naming.
- Settings call `sh` explicitly; no executable-bit dependency for the hook.
- Windows shell tests are ignored unless run on a POSIX test platform.
- Validate complete YAML examples automatically using `test-rules`.

## Requests for the merge session

Completed directly by solo integration; no separate merge needed.

## Spec updates

Applied the hook contract, executable fallback, docs links and W4 feature
details to `docs/specification_linter.md`.

## CHANGELOG lines

- Claude Code PostToolUse hook, skill and agent instructions with jq/Python
  decoding, plus user guides for configuration, rules, agents and CI.

## How to test manually

```sh
cargo test --test integrations
cargo run -- lint --format json tests/e2e/sample_project
cargo run -- test-rules examples/rules
```

For an actual game, build release, copy the hook/settings as described in
`docs/agents.md`, then edit a `.gd` file. Remaining issues should be returned
to Claude after safe fixes.

## Known issues / follow-ups

- Sample dogfood: 5 files, 8 errors, 9 warnings, 2 infos, 4 safe-fixable
  issues. Safe fixes apply four fixes in two files; lint, diff, fix, unsafe
  fix and repeated lint do not crash. Source fixtures remain unchanged.
- The sample's issues are intentional; no new false positive identified.
- Real-game dogfooding has not run: the user supplied no path. Recommend it
  before v0.30 without blocking later roadmap work.
- Existing W3 caveats remain: explain/list-rules use cwd config and show
  default severities; `no-else-return` may flag a later elif when an earlier
  elif falls through; unsafe unused-argument renames can collide with an
  existing `_name`. See `w3-merge.md` for repro context.
