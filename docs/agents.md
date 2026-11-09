# Using the linter with coding agents

The primary executable is `gdguard`; `gdscript-formatter` remains a legacy alias.
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
   project's agent instructions. Run `gdguard init` in the game
   to enable the starter custom rule. Under `[fix]`, set `format = true`
   so each edited file is also formatted, and list the unsafe fixes your
   project accepts, for example
   `unsafe = ["prefer-squared-distance", "prefer-array-method", "prefer-unique-node-name"]`,
   so the hook applies those too.

The hook reads `.tool_input.file_path` (or `.path`) from stdin, skips other
extensions, and runs `lint --fix --format agent` on each `.gd` file, so safe
fixes (and formatting, with `[fix] format = true`) land before the agent
sees anything. A clean file returns silently and costs the agent no
tokens. Remaining issues return on stderr with exit 2, as short text: each
issue's location, rule and message, its source line, help and the fixes
that need review. Set `GDGUARD_HOOK_FORMAT=json` for the full JSON report
instead. Tool failures also return exit 2 with their error, so they do not
look clean. Paths are decoded as JSON and passed as a single
argument, including spaces, quotes and Unicode.

This is feedback after an edit. It does not undo the edit or lint shell
commands that bypass these tools. See the
[official hook reference](https://code.claude.com/docs/en/hooks) for event
and exit-code behavior. The tests run the hook through `sh` with the actual
linter, including its Python fallback without `jq`.

## Recommended loop for any agent

```sh
gdguard lint --fix --format agent scripts/player.gd
gdguard explain unused-argument
gdguard lint --format agent scripts/player.gd
```

`--format agent` prints only what's left to do, in a few lines per issue:

```text
scripts/player.gd:5:6: warning [unused-variable] Variable 'speed' is never used.
  > var speed := 10
  help: Use the value, remove the declaration, or prefix an intentional unused binding with `_`.
  unsafe fix: Remove unused variable 'speed'
1 issue (1 warning). Rule details: gdguard explain <rule>
Unsafe fixes may change behavior: review each, then edit the code or run gdguard lint --fix-unsafe.
```

Use `--format json` when a program reads the result (byte ranges and fix
edits are only in JSON). Read the help and fixes after safe fixes. Review each unsafe fix
and suggestion in context, edit the code, and re-lint until exit 0. Fix
tool/config errors (exit 2) before interpreting diagnostics. Keep ignore
comments specific and include ` -- reason`. Do not extend a baseline to
hide a newly introduced issue. Run `test-rules` after changing custom rules.

### Add Godot's own check

gdguard doesn't check types. Point it at a Godot 4 executable and the same
run also loads the linted scripts in headless Godot, reporting its errors
as `godot-error` in the same JSON:

```sh
export GDGUARD_GODOT=/path/to/Godot_v4.7.2-stable_linux.x86_64
gdguard lint --fix --format json scripts/   # or pass --godot <PATH>
```

The first run after a `class_name` changes runs `godot --import` (a few
seconds); later runs take a fraction of a second per project. Godot's
warnings only appear when the project sets them to error, for example
`debug/gdscript/warnings/unreachable_code=2` in `project.godot`. The check
is skipped for stdin and `--diff`. Godot may write `.godot/` and `.uid`
files, as the editor does.

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
| `check` | Optional granular ID for consolidated rules; old IDs still work in ignores and config. |
| `message`, `help`, `snippet`, `explain` | Explanation, nullable help, first source line, explain command |
| `range.start`, `range.end` | 1-based `line` and UTF-8 byte `column`, 0-based absolute `byte`; exclusive end |
| `fixes[]` | `description`, `safety` (`safe`/`unsafe`/`suggestion`), `edits[]` |
| `edits[].range`, `replacement` | `start.byte`, `end.byte`, replacement text |

`summary.fixable` counts issues with a safe fix. Byte offsets refer to the
reported source, so re-lint after changing it. Apply all edits of one fix
together, in descending byte order, or use `--fix`.

## CI

```sh
gdguard lint --format github .
gdguard lint --format sarif . > gdguard.sarif
gdguard lint --no-cache --format json . > gdguard.json
gdguard test-rules examples/rules
```

GitHub annotations are emitted on stdout. Upload SARIF with your chosen
code-scanning workflow and preserve the linter's exit status. Use a
[baseline](configuration.md#baseline-an-existing-project) for existing
debt. Commit rules, config and the baseline; ignore `.gdguard/cache/`.
