---
name: gdguard
description: Lint and fix GDScript (.gd) files with gdguard until they are clean. Use after editing GDScript, when the gdguard hook reports issues, or when asked to lint, clean up or fix warnings in a Godot project.
---

# Fixing GDScript with gdguard

gdguard is a GDScript linter with automatic fixes. Work in this loop until
`gdguard lint` exits with code 0.

## The loop

1. **Apply the safe fixes and get the rest as JSON:**

   ```sh
   gdguard lint --fix --format json path/to/file.gd
   ```

   `--fix` rewrites the file with every `safe` fix. stdout is the JSON report
   of what is left; stderr says `Fixed N problems in M files`. Exit code 0
   means clean; 1 means issues remain; 2 means a config or rule-file error
   (read stderr and fix that first).

   With the Claude Code hook installed, this runs by itself after each edit
   and the report arrives as hook feedback.

2. **Fix each remaining issue by hand.** For every entry in `issues`:
   - `message` says what is wrong and `help` says how to fix it.
   - `range.start.line`/`column` (1-based) and `snippet` locate it.
   - `fixes` may hold an `unsafe` or `suggestion` fix with exact `edits`
     (byte ranges and replacement text). Apply it when it is right for this
     code; `unsafe` fixes usually are, `suggestion` fixes need judgment.
   - When unsure why a rule exists or what good code looks like, run the
     command in `explain` (for example `gdguard explain no-else-return`).

3. **Lint again** (step 1) and repeat until the exit code is 0. Info
   diagnostics don't change the exit code; fix them when they are cheap.

## Rules for this loop

- Fix the code, don't hide the issue. Only add an ignore comment when the
  code is right and the rule is wrong for this line, and always give a
  reason after ` -- `:

  ```gdscript
  var _unused = 1  # gdguard-ignore unused-argument -- required by the plugin API
  ```

  Never add an ignore comment without a ` -- reason`.
- Don't change `gdguard.toml` to turn rules off unless the user asks.
- Don't edit `.gdguard/baseline.json` by hand.
- If a fix from a custom rule (`origin: "custom"`) looks wrong, say so; the
  rule file is in `.gdguard/rules/` and has its own tests
  (`gdguard test-rules`).

## Useful commands

```sh
gdguard lint                         # whole project, human-readable
gdguard lint --fix-unsafe file.gd    # also apply unsafe fixes
gdguard lint --diff file.gd          # show what --fix would change
gdguard explain <rule>               # why a rule exists, with examples
gdguard lint --list-rules            # every rule, with severity and fixability
```
