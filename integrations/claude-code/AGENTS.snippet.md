## GDScript linting (gdguard)

After editing `.gd` files, run the lint loop until it exits with code 0:

1. `gdguard lint --fix --format json <files>`: applies safe fixes; stdout is
   a JSON report of what remains (exit 0 = clean, 1 = issues, 2 = config
   error).
2. For each issue, follow `help`; apply its `fixes` (`unsafe`/`suggestion`)
   when they fit; run `gdguard explain <rule>` when unsure.
3. Lint again. Repeat until exit code 0.

Fix the code rather than silencing the rule. Only add
`# gdguard-ignore <rule> -- <reason>` when the code is right, and always give
the reason. Don't turn rules off in `gdguard.toml` unless asked.
