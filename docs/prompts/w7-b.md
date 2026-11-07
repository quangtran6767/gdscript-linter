# W7 · Track B: Godot editor integration

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track B**, wave **W7**, on branch **`w7-b`**.

**At the same time**, Track A is building the LSP server in `src/lsp/`.
Don't touch it. Both of you use only `src/linter/api.rs`.

## Goal

Godot users see lint issues inside the Godot editor and can apply fixes with
one click.

## Read

- Spec: "Godot editor integration", "Library API".
- `gdextension/`, `addons/GDQuest_GDScript_formatter/` (how the formatter
  addon works today), and all `docs/handoff/w6-*.md` and `w6-merge.md`.

## Tasks

1. **gdextension** (`gdextension/src/lib.rs`):
   - `lint(path: String, source: String) -> Array[Dictionary]`, where each
     Dictionary has `rule`, `severity`, `message`, `help`, `line`,
     `column`, `end_line`, `end_column` and `fixes` (each with
     `description`, `safety` and `edits`);
   - `fix(path, source, unsafe: bool) -> Dictionary` with `output`,
     `applied` and `remaining`;
   - the config is loaded with `api::load_config_for` from the project root
     (`ProjectSettings.globalize_path("res://")`) and cached until
     `gdguard.toml` changes;
   - keep the old `lint_gdscript` method working, marked deprecated.
2. **Addon:** extend the existing addon (or add `addons/<name>_lint/` if
   that's cleaner; say which in the handoff).
   - A bottom panel listing the current script's issues as severity icon,
     `line:col`, rule and message.
   - Clicking an issue moves the script editor caret to it.
   - An "Apply fix" button for issues with a fix (it says "unsafe" when the
     fix is unsafe), and a "Fix all safe" button.
   - Lint on save and after a short pause in typing.
   - Project settings: enable/disable, lint on save, and show warnings.
3. **Docs:** the Godot section of `docs/editors.md`, covering install,
   settings and screenshots placeholders.

## You may edit

- `gdextension/`, `addons/`
- `docs/editors.md` (Godot section only)
- new `tests/gdextension/` (Rust tests for the Dictionary conversion)

## Acceptance

- Rust tests for the Dictionary conversion (spans, fixes, unicode text).
- The gdextension builds for the host platform.
- A manual test checklist in the handoff that the user can follow in Godot
  4.x. List anything you couldn't verify without running Godot.
- `docs/handoff/w7-b.md` is written.
