# gdguard

A GDScript linter, fixer and formatter for the command line, built for AI
agents. After editing a Godot project, run it to apply safe fixes, read the
remaining diagnostics as JSON, repair them and run it again.

## Install

```sh
cargo build --locked --release --bin gdguard
cp target/release/gdguard ~/.local/bin/   # or anywhere on PATH
cd my-game && gdguard init                 # creates gdguard.toml
```

Requires Rust 1.89+. Prebuilt binaries are attached to tagged
[releases](https://github.com/quangtran6767/gdscript-linter/releases).

## Agent loop

```sh
gdguard lint --fix --format agent .  # apply safe fixes, report the rest briefly
gdguard explain <rule>               # why a rule fires and how to fix it
gdguard lint --format json .         # full JSON for programs
```

- Each JSON diagnostic has the file, rule, severity, message, help, source
  range, snippet and fix edits with a safety level.
- `--fix` applies only safe fixes. Review unsafe fixes before you pass
  `--fix-unsafe`.
- Exit codes: **0** for no problems, **1** for lint problems, **2** for
  usage, config or file errors. `--max-warnings N` lets up to N warnings pass.
- Fix any `parse-error` first. Byte offsets refer to the linted snapshot, so
  re-lint after each edit.
- `--stdin-filename path.gd` lints a buffer from stdin.

See [docs/agents.md](docs/agents.md) for the JSON contract and a Claude Code
hook.

## Rules

**[All built-in rules, with examples →](docs/rules.md)** ·
`gdguard lint --list-rules` · `gdguard explain <rule>`

The 29 built-in rules combine related naming, unused-code and expression checks.
They cover explicit type annotations (including function return types),
declaration order, likely bugs,
Godot 3 syntax and Godot idioms (`is_empty()`, squared distances, Array
methods, `%UniqueNode`), plus project checks (`class_name`, `load`/`preload`
paths, scene connections, unused signals). To let `lint --fix` apply
selected unsafe fixes too, set `[fix] unsafe = [...]` in `gdguard.toml`.
For type correctness, add Godot's own check:
`gdguard lint --godot /path/to/godot .` (or set `GDGUARD_GODOT`).

Add project rules without rebuilding, as [YAML](docs/custom_rules.md) or
[JavaScript](docs/js_rules.md) ([examples](examples/rules/)).

## Formatting

Without a subcommand, `gdguard` formats files exactly like the original
formatter, and `gdscript-formatter` is kept as an alias.

```sh
gdguard --check .
gdguard scripts/player.gd
```

Configure the formatter with `.editorconfig`, and use `# fmt: off` / `# fmt: on`
to skip a section.

## CI

```sh
gdguard lint --format github .
gdguard lint --format sarif . > gdguard.sarif
```

Commit `gdguard.toml`, your rules and any baseline. Ignore `.gdguard/cache/`.

## More

[Built-in rules](docs/rules.md) ·
[Configuration](docs/configuration.md) ·
[Migrating from gdlint](docs/migration.md) · [FAQ](docs/faq.md) ·
[Specification](docs/specification_linter.md) ·
[Releasing](docs/releasing.md)

## License

MIT, see [LICENSE](LICENSE). Based on
[GDQuest's GDScript Formatter](https://github.com/GDQuest/GDScript-formatter)
and [tree-sitter-gdscript](https://github.com/PrestonKnopp/tree-sitter-gdscript).
