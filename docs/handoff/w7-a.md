# W7-A: optional stdio LSP (solo)

Implemented a synchronous lsp-server loop using the existing library API.
It keeps full-text/versioned open buffers, publishes on open/save and after
150 ms debounce, clears closed diagnostics, and reloads config/rules on
watched-file/configuration events. Dynamic watchers register when supported.

Diagnostics include UTF-16 ranges, severity, rule code, help and a docs link.
Code actions include each safe/unsafe/suggestion fix, safe fix-all and an
ignore insertion with a reason placeholder. Edits carry document versions;
requested action-kind filters are honored. Protocol/config errors are
reported without crashing or corrupting stdout. Shutdown/exit is supported.

## API / files

- `src/lsp/mod.rs`: `run_stdio() -> Result<(), String>`,
  `serve(&Connection) -> Result<(), String>`,
  `diagnostic_json(&str, &Diagnostic) -> serde_json::Value`,
  `byte_to_position(&str, usize) -> lsp_types::Position`,
  `position_to_byte(&str, Position) -> usize`,
  `uri_to_path(&str) -> Result<PathBuf, String>`.
- CLI `lsp` now runs the server; the unimplemented-command test became a
  real early-EOF protocol-error check.
- `tests/lsp/`: actual stdio JSON-RPC session with initialize, open,
  diagnostics, versioned code actions, applied safe fix, debounce, config
  reload/error recovery, close and shutdown; emoji/CJK conversion; parity
  with every sample script's library diagnostics.
- `docs/editors.md`: optional VS Code/Neovim/Zed setup. A small Zed adapter
  registers the server alongside Godot; its SDK is separately pinned and
  host compile-checked, outside the CLI dependency graph.

## Validation / scope

585 workspace tests and 569 explicit feature-off tests pass, as do format,
clippy, release build and protocol/native parity. No GUI session in VS Code,
Neovim or Zed was run. The maintainer clarified that CLI/AI-agent usage is
primary; IDE work is compatibility support, not a polish/release gate.

Manual: `cargo test --test lsp`; `gdscript-formatter lsp` from an LSP client.
No frozen library signatures were changed. No release tag created.
