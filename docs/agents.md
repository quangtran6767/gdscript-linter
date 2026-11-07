# gdguard for AI agents

gdguard is built so an AI coding agent can lint GDScript, apply the safe fixes,
fix the rest itself, and check its work, in a loop, without a person in
between. This page covers the Claude Code hook, the JSON report the agent
reads, the recommended loop, and CI.

Every command on this page was run with gdguard; outputs are real, with
absolute paths shortened to `<project>`.

## The loop

1. `gdguard lint --fix --format json <files>`: applies every `safe` fix in
   place, then prints the remaining issues as JSON on stdout. stderr gets
   `Fixed N problems in M files`.
2. For each remaining issue, the agent reads `message` and `help`, applies
   the issue's `fixes` (`unsafe` or `suggestion`) when they fit, and runs
   `gdguard explain <rule>` when it needs the rule's rationale and examples.
3. Lint again. Stop when the exit code is 0.

Exit codes: `0` no errors (and warnings within `--max-warnings`), `1` issues
remain, `2` config or rule-file error (fix that first; stderr says which file
and key). Info diagnostics never change the exit code.

Agents should fix the code rather than silence rules. An ignore comment is
only for code that is right, and must give a reason after ` -- `:

```gdscript
func _on_body_entered(body):  # gdguard-ignore unused-argument -- signal signature
	queue_free()
```

The older `gdlint-ignore` prefix works too.

## Claude Code hook (3 steps)

The hook runs the loop's first step after every edit of a `.gd` file and
hands the remaining issues back to Claude.

1. **Put gdguard on the PATH** (or set `GDGUARD` to the binary's path in the
   environment Claude Code runs in).
2. **Copy the hook** into your project:

   ```sh
   mkdir -p .claude/hooks
   cp integrations/claude-code/hooks/lint-gd.sh .claude/hooks/
   ```

3. **Register it** in `.claude/settings.json` (merge with what's there; the
   full file is `integrations/claude-code/settings.example.json`):

   ```json
   {
     "hooks": {
       "PostToolUse": [
         {
           "matcher": "Edit|Write|MultiEdit",
           "hooks": [
             {
               "type": "command",
               "command": "sh \"$CLAUDE_PROJECT_DIR/.claude/hooks/lint-gd.sh\"",
               "timeout": 60
             }
           ]
         }
       ]
     }
   }
   ```

Optional: copy `integrations/claude-code/skill/SKILL.md` to
`.claude/skills/gdguard/SKILL.md` so Claude knows the loop, and paste
`integrations/claude-code/AGENTS.snippet.md` into `CLAUDE.md` or `AGENTS.md`
for other agents.

### What the hook does

The hook reads Claude Code's hook JSON on stdin and takes
`tool_input.file_path`. For a `.gd` file that exists, it runs
`gdguard lint --fix --format json <file>`:

| Result | Hook output | Exit code |
|--------|-------------|-----------|
| Clean after safe fixes | nothing | 0 |
| Issues remain | summary + JSON report on stderr | 2 (Claude sees stderr) |
| Config or rule-file error | the error on stderr | 2 |
| Not a `.gd` file, file missing, gdguard not found | nothing (a warning when gdguard is missing) | 0 |

For example, after Claude edits a file with an unnecessary `pass` (safe fix)
and a badly named variable (no fix):

```console
$ echo '{"tool_name":"Edit","tool_input":{"file_path":"scripts/hud.gd"}}' | sh .claude/hooks/lint-gd.sh
gdguard: scripts/hud.gd still has issues after safe fixes (1 errors, 0 warnings, 0 infos).
Fixed 1 problem in 1 file
Fix them using each issue's "help" and "fixes" (run `gdguard explain <rule>` when unsure), then the hook lints again. JSON report:
{
  "version": 1,
  ...
```

The `pass` is already gone from the file; Claude renames the variable and
the next edit runs the hook again.

The hook is POSIX `sh`. It uses `jq` when installed (and then also lists one
line per issue in the summary); without `jq` it falls back to `sed`. Set
`GDGUARD_HOOK_NO_JQ=1` to force the fallback.

## The JSON report (version 1)

`gdguard lint --format json` prints one document for all files. Field names
are stable; new optional fields may be added without changing `version`.

```console
$ gdguard lint --fix --format json scripts/enemy.gd
{
  "version": 1,
  "tool": {
    "name": "gdguard",
    "version": "0.28.0"
  },
  "summary": {
    "files": 1,
    "errors": 0,
    "warnings": 1,
    "infos": 0,
    "fixable": 0
  },
  "issues": [
    {
      "file": "scripts/enemy.gd",
      "rule": "unused-argument",
      "origin": "builtin",
      "severity": "warning",
      "message": "Function argument 'delta' is unused. Consider removing it or prefixing with '_'",
      "help": "Remove 'delta', or rename it to '_delta' if the function signature must stay the same.",
      "range": {
        "start": {
          "line": 1,
          "column": 15,
          "byte": 14
        },
        "end": {
          "line": 1,
          "column": 20,
          "byte": 19
        }
      },
      "snippet": "func _process(delta):",
      "explain": "gdguard explain unused-argument",
      "fixes": [
        {
          "description": "Rename it to '_delta'",
          "safety": "unsafe",
          "edits": [
            {
              "range": {
                "start": {
                  "byte": 14
                },
                "end": {
                  "byte": 19
                }
              },
              "replacement": "_delta"
            }
          ]
        }
      ]
    }
  ]
}
Fixed 0 problems in 0 files
```

(`Fixed ...` is on stderr.)

| Field | Meaning |
|-------|---------|
| `summary.files` | files linted |
| `summary.errors`, `warnings`, `infos` | issue counts by severity |
| `summary.fixable` | issues with a `safe` fix (after `--fix`, usually 0) |
| `issues[].file` | path as given on the command line or found in the folder |
| `issues[].rule` | rule id; `gdguard explain <rule>` describes it |
| `issues[].origin` | `builtin` or `custom` (a rule from `.gdguard/rules/`) |
| `issues[].severity` | `error`, `warning` or `info` |
| `issues[].message` | what is wrong |
| `issues[].help` | how to fix it, or `null` |
| `issues[].range` | `start`/`end` with 1-based `line`, 1-based `column` in UTF-8 bytes, and 0-based `byte` offset; `end` is exclusive |
| `issues[].snippet` | the full first line of the range |
| `issues[].explain` | the command that explains the rule |
| `issues[].fixes[]` | `description`, `safety` (`safe`, `unsafe`, `suggestion`) and `edits` (byte `range` + `replacement`); edits of one fix don't overlap |

To apply a fix's edits by hand, replace the bytes `start.byte..end.byte` with
`replacement`, working from the last edit to the first so earlier offsets
stay valid. `gdguard lint --fix-unsafe` applies `safe` and `unsafe` fixes for
you; `suggestion` fixes are never applied automatically.

## Learning about a rule

```console
$ gdguard explain prefer-is-empty
prefer-is-empty
  Origin:   custom (<project>/.gdguard/rules/prefer-is-empty.yml)
  Severity: warning
  Fixable:  yes

Use `$X.is_empty()` instead of comparing `len($X)` to zero

Why:
  Comparing `len()` to zero hides the intent of the check. `is_empty()` reads
  as a question about the collection and doesn't count its elements.
  See also `prefer-not-is-empty` for `len(x) > 0` and `len(x) != 0`.
...
```

`gdguard lint --list-rules --format json` lists every rule (built-in and
custom) with its description, rationale, severity, fixability and examples,
which an agent can read once per session.

## CI

Lint the project and test the custom rules on every push. With GitHub
Actions:

```yaml
name: gdguard
on: [push, pull_request]
jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # Install gdguard here (download a release binary or `cargo install`).
      - run: gdguard test-rules
      - run: gdguard lint --format compact .
```

`gdguard lint` exits 1 when there are errors or more warnings than
`--max-warnings` allows (default 0), which fails the job. To adopt gdguard on
an existing project without fixing everything first, allow some warnings:

```console
$ gdguard lint --max-warnings 5 .
scripts/enemy.gd:1:15: warning [unused-argument] Function argument 'delta' is unused. Consider removing it or prefixing with '_'
scripts/player.gd:6:15: warning [unused-argument] Function argument 'delta' is unused. Consider removing it or prefixing with '_'
```

(exit code 0: two warnings, at most five allowed).

Pass the folder (`.`) explicitly: with no path, `gdguard lint` currently
reads GDScript from stdin instead of linting the project (in CI, with an
empty stdin, it passes without checking anything).
