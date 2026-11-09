---
name: gdguard
description: Lint and fix GDScript files with gdguard after editing them.
---

After editing GDScript, run `gdguard lint --fix --format json <file>`.
The existing binary is also available as `gdscript-formatter`.

1. Read the remaining `issues`, their `help`, and `fixes`. Byte offsets are
   UTF-8, the end is exclusive. Only safe fixes are applied by `--fix`.
2. Run `gdguard explain <rule>` when the intent is unclear. Review unsafe
   fixes and suggestions before applying them; preserve game behavior.
3. Fix the remaining code and re-lint until exit 0. Exit 1 means issues
   remain; exit 2 means a usage, configuration, or file error to resolve.
4. Never add an ignore comment without a ` -- reason`. Prefer fixing the
   code. Keep intentional exceptions specific to a rule and location.

Do not edit a baseline to conceal a newly introduced issue. Run
`gdguard test-rules` after changing custom rules.
