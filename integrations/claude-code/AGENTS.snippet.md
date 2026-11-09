After editing a `.gd` file, run `gdguard lint --fix --format json <file>`
(`gdscript-formatter` is the current binary name). Read each remaining
issue's `help` and `fixes`; use `gdguard explain <rule>` if unclear. Review
unsafe fixes and suggestions, fix the code, and re-lint until exit 0. Exit 2
is a tool/config/file error. Never add an ignore without a ` -- reason`, or
change the baseline to hide a new issue. After editing rules, run
`gdguard test-rules`.
