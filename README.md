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
gdguard lint --fix --format json .   # apply safe fixes, report the rest
gdguard explain <rule>               # why a rule fires and how to fix it
gdguard lint --format json .         # re-lint after manual repairs
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

```sh
gdguard lint --list-rules
gdguard test-rules .gdguard/rules
```

Built-in rules check naming, control flow, unreachable code, unused
declarations, shadowing, duplicate dictionary keys and Godot 3 syntax. In a
Godot project they also check `class_name` (every non-autoload script has a
unique one), `load()`/`preload()` paths (they exist, and use `uid://` when
the resource has a UID), scene connections to missing methods, unused
signals across scripts and `.tscn` scenes, and missing members on
explicitly typed project classes. The linter never infers types.

For type errors, add Godot's own check to the same run:

```sh
gdguard lint --godot /path/to/godot --format json .   # or set GDGUARD_GODOT
```

You can add project rules without rebuilding, as
[YAML patterns](docs/custom_rules.md) or
[sandboxed JavaScript](docs/js_rules.md). See the tested
[examples](examples/rules/).

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

[Configuration](docs/configuration.md) ·
[Migrating from gdlint](docs/migration.md) · [FAQ](docs/faq.md) ·
[Specification](docs/specification_linter.md) ·
[Releasing](docs/releasing.md)

## License

MIT, see [LICENSE](LICENSE). Based on
[GDQuest's GDScript Formatter](https://github.com/GDQuest/GDScript-formatter)
and [tree-sitter-gdscript](https://github.com/PrestonKnopp/tree-sitter-gdscript).
