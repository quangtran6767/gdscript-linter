# Using the linter with coding agents

The executable currently ships as `gdscript-formatter`; `gdguard` is the
planned name used by JSON `explain` hints. Substitute the installed name.
The hook tries `gdguard`, then `gdscript-formatter`, or an absolute path in
`GDGUARD_BIN`.

## Claude Code setup in three steps

1. Build with `cargo build --release`. Put the binary in your PATH, or set
   `GDGUARD_BIN` to its absolute path. Install `jq` or Python 3 for decoding
   hook JSON; the hook runs under POSIX `sh` (Git Bash on Windows).
2. Copy `integrations/claude-code/hooks/lint-gd.sh` to your game's
   `.claude/hooks/lint-gd.sh`. Merge the `hooks` object from
   [settings.example.json](../integrations/claude-code/settings.example.json)
   into `.claude/settings.json`. It registers `PostToolUse` for
   `Edit|Write|MultiEdit`. Reload the session if needed and inspect `/hooks`.
3. Copy [the skill](../integrations/claude-code/skill/SKILL.md) into
   `.claude/skills/gdguard/SKILL.md`, or append the
   [agent snippet](../integrations/claude-code/AGENTS.snippet.md) to your
   project's agent instructions. Run `gdscript-formatter init` in the game
   to enable the starter custom rule.

The hook reads `.tool_input.file_path` (or `.path`) from stdin, skips other
extensions, and runs `lint --fix --format json` on each `.gd` file. A clean
file returns silently. Remaining issues return a short summary and JSON on
stderr with exit 2. Tool failures also return exit 2 with their error, so
they do not look clean. Paths are decoded as JSON and passed as a single
argument, including spaces, quotes and Unicode.

This is feedback after an edit. It does not undo the edit or lint shell
commands that bypass these tools. See the
[official hook reference](https://code.claude.com/docs/en/hooks) for event
and exit-code behavior. The tests run the hook through `sh` with the actual
linter, including its Python fallback without `jq`.

## Recommended loop for any agent

```sh
gdscript-formatter lint --fix --format json scripts/player.gd
gdscript-formatter explain unused-argument
gdscript-formatter lint --format json scripts/player.gd
```

Read `issues[].help` and `fixes` after safe fixes. Review each unsafe fix
and suggestion in context, edit the code, and re-lint until exit 0. Fix
tool/config errors (exit 2) before interpreting diagnostics. Keep ignore
comments specific and include ` -- reason`. Do not extend a baseline to
hide a newly introduced issue. Run `test-rules` after changing custom rules.

`--fix` sends a JSON report of remaining issues to stdout and the fixed
count to stderr. With `--stdin-filename` and `--fix`, stdout instead holds
the fixed source and diagnostics go to stderr; use disk files for JSON.

## JSON schema version 1

The complete [contract](specification_linter.md#json-output-schema-version-1)
is stable: readers should accept additional fields.

| Field | Meaning |
| --- | --- |
| `version` | Schema integer, currently 1 |
| `tool.name`, `tool.version` | Tool identity and build version |
| `summary` | `files`, `errors`, `warnings`, `infos`, `fixable`, `baseline_hidden` counts |
| `issues[].file`, `rule`, `origin`, `severity` | File, rule ID, `builtin`/`custom`, `error`/`warning`/`info` |
| `message`, `help`, `snippet`, `explain` | Explanation, nullable help, first source line, explain command |
| `range.start`, `range.end` | 1-based `line` and UTF-8 byte `column`, 0-based absolute `byte`; exclusive end |
| `fixes[]` | `description`, `safety` (`safe`/`unsafe`/`suggestion`), `edits[]` |
| `edits[].range`, `replacement` | `start.byte`, `end.byte`, replacement text |

`summary.fixable` counts issues with a safe fix. Byte offsets refer to the
reported source, so re-lint after changing it. Apply all edits of one fix
together, in descending byte order, or use `--fix`.

## CI

```sh
gdscript-formatter lint --format github .
gdscript-formatter lint --format sarif . > gdguard.sarif
gdscript-formatter lint --no-cache --format json . > gdguard.json
gdscript-formatter test-rules examples/rules
```

GitHub annotations are emitted on stdout. Upload SARIF with your chosen
code-scanning workflow and preserve the linter's exit status. Use a
[baseline](configuration.md#baseline-an-existing-project) for existing
debt. Commit rules, config and the baseline; ignore `.gdguard/cache/`.
