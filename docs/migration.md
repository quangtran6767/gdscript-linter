# Migrating to gdguard

## From gdlint / gdtoolkit

Install gdguard alongside existing tooling, then run `gdguard init` in the
project and `gdguard lint --format json .`. Review diagnostics before adding
it as a required CI check. The tools have different parsers, rules and
configuration; identical rule names do not guarantee identical behavior.

Translate the settings you need into `gdguard.toml`; `.gdlintrc` is not read.
Use `gdguard lint --list-rules --format json` and `gdguard explain RULE` as
the authoritative catalog. gdtoolkit rule names not present in this catalog
need a custom YAML/JS rule or should remain checked by the other tool.
Unknown rule IDs in config fail with exit 2 instead of being ignored.

Legacy `# gdlint-ignore`, `-line` and `-next-line` directives remain accepted.
New code should use `# gdguard-ignore-next-line rule-id -- reason`. Review
`unused-ignore` before removing old exceptions. Use a reviewed
[baseline](configuration.md#baseline-an-existing-project) for existing debt;
do not baseline new issues merely to get a passing agent run.

Prefer JSON to parsing terminal text. JSON schema version 1 uses UTF-8 byte
positions; it differs from other tools' output. CI must handle 0 (pass),
1 (lint problems), and 2 (tool/config/file error). `--fix` applies safe
fixes; it is not a replacement for code review or Godot validation.

## From this fork's gdscript-formatter executable

`gdguard` is now the Cargo package, Rust crate and primary executable. The
`gdscript-formatter` binary remains an alias with the same commands and exit
codes; help identifies the product as gdguard. Replace Rust imports with
`gdguard::` and Cargo `-p gdscript-formatter` with `-p gdguard`.

Formatter flags, `.editorconfig` keys including `gdscript_formatter_*`,
`# fmt: off/on`, and formatting behavior are retained. Running either
executable without a subcommand still formats. Add `lint` explicitly for
linting. Legacy pre-commit hook IDs are retained alongside gdguard IDs.

The old upstream text reporter was replaced by `file:line:col: severity
[rule] message`; use `--format json` for stable automation. Config remains
`gdguard.toml` and custom rules remain `.gdguard/rules/`.

Optional addon folders are now `addons/gdguard/`,
`addons/gdguard_standalone/`, and `addons/gdguard_lint/`. Disable/remove old
formatter addons before copying the renamed ones. The native library is
`gdguard_gdextension` and singleton `GDGuard`; restart Godot after installing.
The deprecated `lint_gdscript` method remains on the new singleton.
Do not install two formatter plugins at once.
