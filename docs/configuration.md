# Linter configuration

See [the built-in rules](rules.md) for every rule id, its default level and
its fix.

The executable is `gdguard`. `init` creates
`gdguard.toml` and `.gdguard/rules/prefer-is-empty.yml` at the Godot project
root (or current directory without `project.godot`).

```toml
[rules]
unused-code = "error"
naming-convention = "error"
type-annotations = { level = "warning", allow_inferred = false }
max-line-length = { level = "warning", max = 120 }

[custom_rules]
paths = [".gdguard/rules"]

[lint]
exclude = ["addons/**", ".godot/**"]
baseline = ".gdguard/baseline.json"

[fix]
format = true
unsafe = ["prefer-squared-distance", "prefer-array-method"]

[[overrides]]
files = ["tests/**"]
rules = { private-access = "off" }
```

For each file, the nearest config found upward wins. `--config PATH` uses
one config for the entire run. CLI flags override per-folder overrides,
then `[rules]`, then `.editorconfig` for line length, then defaults. Rule
levels are `off`, `info`, `warning`, `error`; table forms can set options
without changing the default level. Unknown rule IDs, keys, or built-in
options are errors. Custom options are passed through.

Paths and globs are relative to the config directory. `*` does not cross
`/`; `**` does. A matched directory excludes its descendants. Excludes
apply to explicitly named files too. Custom rules need a registered folder.

## Consolidated rules and typing

Use `naming-convention` for naming, `unused-code` for unused declarations and redundant `pass`, and `redundant-expression` for self-comparisons, self-assignments and ineffective expressions. Reports use these group IDs; JSON also includes the original `check` ID. Legacy IDs continue to work in config, overrides, `--disable`, ignores, unsafe-fix allowlists and baselines. Within one rules table, a specific check setting overrides its group. Later matching override tables win over both. CLI disables win last.

`type-annotations` is a warning by default and requires explicit types for functions/lambdas, parameters, variables, constants, loop variables, signal parameters and Array/Dictionary arguments. Use `allow_inferred = true` to accept `:=` declarations/default parameters and implicit loop-variable typing, or set the level to `off` for a gradual migration. Explicit `Variant` is allowed. This checks annotation presence; Godot checks type correctness. No type fixes are applied, even with `--fix-unsafe`.

## Fix and format automatically

With `[fix] format = true`, `lint --fix` and `--fix-unsafe` also format
every file they check with the formatter, even when no fix applied or the
file wasn't formatted before. Use it in projects where agents edit code:
the post-edit hook then leaves each edited file fixed and formatted, so
agents never spend a turn on formatting. The formatter runs with its
structure check, so it can't change what the code does; a file it can't
parse stays as it is. `--diff` shows the formatting too. The summary
reads `Fixed 2 problems in 1 file, formatted 3 files`. The default is
`false`: the fix engine then only keeps already formatted files formatted.
Formatting during fixes uses the resolved `max-line-length` setting,
including matching rule overrides and `--max-line-length`. Other formatting
preferences come from `.editorconfig`.

`unsafe` makes `lint --fix` apply unsafe fixes as well: `true` for every
rule, or a list of rule ids. Unsafe fixes can change behavior in rare
cases (each rule's page in [the rule reference](rules.md) says how), so
list the rules your project accepts. With it, an agent's edits are fixed
by the hook without the agent spending a turn. Unknown ids are config
errors. Suggestions are never applied.

## Ignore comments

```gdscript
# gdguard-ignore-file max-line-length -- generated text
var badName = 1 # gdguard-ignore variable-name -- matches an external field
# gdguard-ignore-next-line unused-argument -- required callback signature
func callback(event):
    pass
```

`gdguard-ignore-line` covers its own line; `gdguard-ignore-next-line`
covers the next. `gdguard-ignore` covers its own line after code, or the
next line when alone. `gdguard-ignore-file` can appear anywhere and covers
the whole file. Separate rule IDs with commas or spaces. With no IDs, all
rules are suppressed. Legacy `gdlint-ignore*` prefixes work too.

The directive must start a comment; strings do not count. Text after
` -- ` is a reason. Add a reason to document intentional exceptions.
`unused-ignore` warns about unused comments or rule IDs and offers a safe
fix. Disabled rules are exempt. Disable it with
`unused-ignore = "off"`, or a file ignore of `unused-ignore`.

## Baseline an existing project

```sh
gdguard lint --update-baseline .
gdguard lint --baseline .gdguard/baseline.json .
```

`--update-baseline` writes all current issues, using `--baseline PATH`,
then `[lint].baseline`, then `.gdguard/baseline.json`. To load the default
file on future runs, set `[lint].baseline` as above or use `--baseline`.
A missing explicit `--baseline` is an error; a missing configured baseline
hides nothing. Update cannot be combined with fixes or `--diff`.

Fingerprints include the rule, relative file path, trimmed source line and
occurrence index among matching issues. Inserting unrelated lines keeps
issues hidden; changing the issue line or introducing another occurrence
reveals an issue. Baselines apply to lint and fix reports. Pretty output
reports the hidden count; JSON adds `summary.baseline_hidden`.

A baseline hides issues from reports but doesn't stop fixes: `--fix`
still applies the safe fixes of baselined issues in the files it checks.
With `[fix].format`, format the project (`gdguard .`) before
`--update-baseline`: reformatting a baselined line changes its text, and
the issue then shows up again.

Commit `.gdguard/baseline.json`; review intentional updates as code changes.

## Output, exit codes and cache

`lint --format pretty|compact|json|github|sarif|agent` selects a reporter.
The default is pretty in a terminal, compact otherwise. `agent` is short
text for coding agents: each issue's compact line, its source line, help
and fixes, then a summary line; see [the agent guide](agents.md). GitHub output uses
workflow annotations; SARIF 2.1.0 includes metadata for reported rules.
Both convert columns to UTF-16; JSON uses UTF-8 byte positions.

Exit 0 means no errors and at most `--max-warnings N` warnings (default 0).
Infos do not fail. Exit 1 means issues exceed that limit; exit 2 is a
usage/config/file error. `--diff` exits 1 when it would change a file.

`--godot <PATH>` (or `GDGUARD_GODOT`) also loads the files in headless
Godot 4 and reports its errors as `godot-error`; see
[the agent guide](agents.md#add-godots-own-check). Set the path per
machine, not in `gdguard.toml`. Turn the rule off like any other:
`godot-error = "off"`.

Files lint in parallel with deterministic output order. With a config,
lint results cache in `.gdguard/cache/lint.json`. File content, effective
settings, custom rule content and tool version invalidate entries. Fix and
diff runs do not use it. Project results also depend on every project
file's path and on scripts, scenes, resources, `.uid` and `.import` files. `--no-cache` skips reads and writes. The cache
folder contains a `.gitignore`; do not commit cache files.
