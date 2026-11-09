# Linter configuration

The current executable is `gdscript-formatter`. `init` creates
`gdguard.toml` and `.gdguard/rules/prefer-is-empty.yml` at the Godot project
root (or current directory without `project.godot`).

```toml
[rules]
unused-argument = "error"
max-line-length = { level = "warning", max = 120 }

[custom_rules]
paths = [".gdguard/rules"]

[lint]
exclude = ["addons/**", ".godot/**"]
baseline = ".gdguard/baseline.json"

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
gdscript-formatter lint --update-baseline .
gdscript-formatter lint --baseline .gdguard/baseline.json .
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

Commit `.gdguard/baseline.json`; review intentional updates as code changes.

## Output, exit codes and cache

`lint --format pretty|compact|json|github|sarif` selects a reporter. The
default is pretty in a terminal, compact otherwise. GitHub output uses
workflow annotations; SARIF 2.1.0 includes metadata for reported rules.
Both convert columns to UTF-16; JSON uses UTF-8 byte positions.

Exit 0 means no errors and at most `--max-warnings N` warnings (default 0).
Infos do not fail. Exit 1 means issues exceed that limit; exit 2 is a
usage/config/file error. `--diff` exits 1 when it would change a file.

Files lint in parallel with deterministic output order. With a config,
lint results cache in `.gdguard/cache/lint.json`. File content, effective
settings, custom rule content and tool version invalidate entries. Fix and
diff runs do not use it. `--no-cache` skips reads and writes. The cache
folder contains a `.gitignore`; do not commit cache files.
