# W7-B: optional Godot native/editor integration (solo)

## Completed

- Path-aware native lint/fix methods use the same library/config/custom-rule
  pipeline as CLI/LSP. Config cache entries refresh by content hash.
- Dictionaries include rule/severity/message/help, start/end line/byte
  columns, authoritative start/end bytes, fixes/safety/edits.
- Native `apply_edits(source, edits)` validates reviewed individual fixes
  atomically and rejects malformed ranges or resulting GDScript.
- Kept the deprecated `lint_gdscript(source, config)` API working.
- Added optional `addons/gdguard_lint`: issue panel, severity icons, caret
  navigation, safety-labelled selected fix, safe fix-all, stale-snapshot
  protection, undo grouping, typing/save linting and project settings.
- Panel UI deliberately skips headless processes. Cached editor references
  and parent-owned timer teardown avoid late editor access on shutdown.
- Added editor data/config session in `src/linter/editor.rs`, independent
  of Godot initialization, with Rust conversion and parity tests.

## Public additions

Native singleton methods (names are migrated in W8):

```text
lint(path: String, source: String) -> Array[Dictionary]
fix(path: String, source: String, unsafe_fixes: bool) -> Dictionary
apply_edits(source: String, edits: Array[Dictionary]) -> Dictionary
```

The Rust editor module adds EditorSession::lint/fix, typed EditorDiagnostic/
EditorFix/EditorFixResult payloads, payloads(&[Diagnostic]) and
apply_edits_checked(&str, &[Edit]) -> Result<String, String>. Frozen API
signatures remain unchanged.

## Actual Godot validation

The user supplied Godot 4.7.2. The host extension builds and the isolated
headless smoke passes: native singleton, all five sample-file rule/span
sets, Unicode/end positions/help/fixes, unsafe rename, individual reviewed
edit, invalid edit rejection and legacy method. **0 native failures.**
The addon parses/imports in the headless editor with its UI disabled there.

First-discovery hot-loading aborts with this engine/SDK combination. The
supported installation/test path cold-loads the extension on startup;
restart Godot after installing. The smoke seeds its temporary extension
list to emulate that normal startup. No real project or user editor
settings are changed; XDG data/config/cache and logs are temporary.

Run:

```sh
python3 tests/gdextension/run_godot_smoke.py --godot /path/to/Godot
```

## Optional interactive checklist

- Enable the plugin after a native-extension restart; open a script.
- Select an issue, verify Unicode caret navigation, inspect help.
- Review/apply a selected unsafe/suggestion fix; undo once.
- Apply safe fix-all; edit during pending diagnostics to check stale guards.
- Toggle enabled/lint_on_save/show_warnings settings.

Interactive appearance/navigation/undo were not GUI-tested. Per the user's
CLI-first clarification these are optional compatibility checks, not a
reason to postpone CLI completion. No screenshot assets or IDE polish added.
