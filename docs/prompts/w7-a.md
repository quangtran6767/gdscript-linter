# W7 · Track A: LSP server

Read `docs/prompts/_session_rules.md` first and follow it the whole session.

You are **Track A**, wave **W7**, on branch **`w7-a`**.

**At the same time**, Track B is building the Godot editor panel in
`gdextension/` and `addons/`. Don't touch those. Both of you use only
`src/linter/api.rs`.

## Goal

`gdguard lsp` gives live diagnostics and quick fixes in VS Code, Neovim, Zed
and any other LSP client.

## Read

- Spec: "LSP server", "Library API".
- All `docs/handoff/w6-*.md` and `w6-merge.md`, plus the `lsp-server` crate
  examples.

## Tasks

1. **`src/lsp/`:** a synchronous main loop using `lsp-server`.
   - **Document sync:** full text; keep open documents in memory.
   - **Diagnostics:** on open, change (debounced 150 ms) and save, call
     `api::lint_source` with the in-memory text. Map severity, rule id
     (`code`), `codeDescription` and `help` (in `relatedInformation` or the
     message).
   - **Code actions:**
     - one quick fix per `Fix`, with the safety in the title, for example
       "Fix: Use is_empty() (unsafe)";
     - "Fix all safe problems" through `api::fix_source`;
     - "Ignore <rule> on this line".
   - **Config:** reload when `gdguard.toml` or the rules change (use
     `workspace/didChangeWatchedFiles`), and re-lint open documents.
   - **Positions:** convert UTF-8 byte columns to UTF-16 in both directions,
     with a test using emoji and CJK text.
   - Never crash. Log errors to stderr and keep serving.
2. **CLI:** `gdguard lsp` uses stdio.
3. **Docs:** the LSP part of `docs/editors.md`, with exact config for VS Code
   (generic LSP client extension), Neovim (`vim.lsp.start`) and Zed (the
   repo has `.zed/`). Explain that it runs alongside Godot's own GDScript
   language server.

## You may edit

- `src/lsp/`, `src/cli.rs`, `src/main.rs`
- `src/linter/api.rs` (bodies, if a fix is needed)
- new `tests/lsp/`, `docs/editors.md` (LSP section only; B writes the Godot
  section)

## Acceptance

`tests/lsp/` drives the server through scripted JSON-RPC messages:
initialize, didOpen, diagnostics published, didChange, a code action, the
edit applied, a config change triggering a re-lint, and shutdown.

`docs/handoff/w7-a.md` is written, including how the user tested it manually
in their editor.
