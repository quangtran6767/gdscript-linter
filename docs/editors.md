# Optional editor integrations

The CLI and AI-agent fix/re-lint loop are the primary product. These
integrations reuse the same library; an editor is not required. Keep
Godot's own language server enabled for engine completion and type checks.

## Language server

Run `gdscript-formatter lsp` (the final binary name is `gdguard`). It speaks
LSP over stdin/stdout; protocol output is never mixed with log output.
It publishes on open/save and 150 ms after full-text changes. Diagnostics
include rule ID, severity, range, help and a documentation link.

Code actions provide individual safe/unsafe/suggestion fixes, safe fix-all,
and an ignore insertion with a reason placeholder to fill in. Edits carry
the document version. No file is modified until the client applies an edit.
Config/rule/project changes re-lint open buffers when the client sends
watched-file events; dynamic watcher registration is requested when supported.

### VS Code

Install [Generic LSP Client (v2)](https://marketplace.visualstudio.com/items?itemName=zsol.vscode-glspc)
and add these settings, using the installed executable path:

```json
{
  "glspc.server.command": "/path/to/gdguard",
  "glspc.server.commandArguments": ["lsp"],
  "glspc.server.languageId": ["gdscript"]
}
```

The extension documents these settings in its setup guide. Leave the Godot
Tools extension active as the engine language server. Review the Generic
LSP Client output channel if startup fails.

### Neovim

Add a GDScript FileType hook using the built-in
[`vim.lsp.start`](https://neovim.io/doc/user/lsp.html#vim.lsp.start()):

```lua
vim.api.nvim_create_autocmd("FileType", {
  pattern = "gdscript",
  callback = function(args)
    local marker = vim.fs.find({"project.godot", "gdguard.toml"}, {
      path = vim.api.nvim_buf_get_name(args.buf), upward = true
    })[1]
    vim.lsp.start({
      name = "gdguard",
      cmd = {"/path/to/gdguard", "lsp"},
      root_dir = vim.fs.dirname(marker or vim.api.nvim_buf_get_name(args.buf))
    }, {bufnr = args.buf})
  end
})
```

Use the usual diagnostic/code-action commands. The existing Godot client
may remain attached to the same buffer.

### Zed

A server name must be registered by an extension; adding an arbitrary
`lsp` settings key alone does not register it. This repository includes the
small optional adapter in `integrations/zed/`, following
[Zed's language-server extension API](https://zed.dev/docs/extensions/languages#language-servers).
Install GDScript language support, put gdguard on PATH, and use **Install Dev
Extension** on that folder. Then merge this into project/user settings:

```json
{
  "languages": {
    "GDScript": {"language_servers": ["gdguard", "..."]}
  }
}
```

The adapter only locates the installed CLI and runs `lsp`; it does not
download a binary. Its pinned SDK is isolated from the CLI workspace. Host
compile-checking is automated; actual Zed/VS Code/Neovim GUI sessions have
not been exercised here. They are optional compatibility checks.

## Godot native integration

Build `cargo build -p gdscript-formatter-gdextension` for the host. The
native extension adds `lint(path, source)`, `fix(path, source, unsafe_fixes)`
and reviewed `apply_edits(source, edits)` to the formatter singleton.
The legacy `lint_gdscript(source, config)` method remains available but is
deprecated; use the path-aware methods for gdguard.toml and custom rules.

Copy the platform library into the standalone addon's bin directory using
its `.gdextension` manifest, and copy `addons/gdguard_lint/` into the game.
Restart Godot after copying the native extension, then enable **gdguard Lint**
in Project Settings → Plugins. It is optional and
separate from formatter installation. The plugin reports a clear status
when the native extension is missing.

The bottom panel lists severity, line/column, rule and message. Select an
issue to jump to the caret, inspect help in its tooltip, or apply its first
fix (the button displays safety). **Fix all safe** uses the same guarded fix
engine as the CLI. Edits are validated against the current buffer snapshot,
use byte offsets for Unicode safety, and become one undo operation.

Project settings: `gdguard/enabled`, `gdguard/lint_on_save`,
`gdguard/show_warnings`. Changes while typing are debounced at 300 ms.
Config caching refreshes when gdguard.toml content changes. Diagnostics and
fixes are returned as Dictionaries with UTF-8 byte positions and explicit
end positions, help and safety metadata.

### Validation

- Rust conversion/config-cache tests: `cargo test --test gdextension`.
- LSP JSON-RPC and CLI parity: `cargo test --test lsp`.
- Real Godot smoke: `python3 tests/gdextension/run_godot_smoke.py --godot /path/to/Godot`.
  It creates an isolated temporary project and redirects Godot settings,
  caches and logs there. Godot 4.7.2 was supplied for this work.

Native parity/fix tests have been exercised in Godot. The panel intentionally skips UI initialization in headless mode. Interactive
panel appearance, navigation and undo remain optional manual checks rather than
CLI release gates.

Screenshot placeholders for future editor documentation: issue list with
severity icons; selected unsafe fix; safe fix-all result.
