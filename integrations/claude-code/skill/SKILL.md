---
name: gdguard
description: Lint and fix GDScript files with gdguard after editing them.
---

The post-edit hook already runs `gdguard lint --fix` on each edited `.gd`
file and only speaks when issues remain. Without the hook, after editing
GDScript run `gdguard lint --fix --format agent <file>`. The legacy
compatibility alias is `gdscript-formatter`.

1. Read each remaining issue's source line, `help` and fixes. Only safe
   fixes are applied by `--fix`. Use `--format json` if you need byte
   ranges and fix edits.
2. Run `gdguard explain <rule>` when the intent is unclear. Review unsafe
   fixes and suggestions before applying them; preserve game behavior.
3. Fix the remaining code and re-lint until exit 0. Exit 1 means issues
   remain; exit 2 means a usage, configuration, or file error to resolve.
4. Never add an ignore comment without a ` -- reason`. Prefer fixing the
   code. Keep intentional exceptions specific to a rule and location.

Do not edit a baseline to conceal a newly introduced issue. Run
`gdguard test-rules` after changing custom rules.
