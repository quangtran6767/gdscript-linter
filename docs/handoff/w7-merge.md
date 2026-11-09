# W7 solo integration

Completed the practical LSP/native compatibility layer. The maintainer
explicitly prioritizes CLI/AI-agent checking, safe fixes and useful JSON;
this priority is persisted in CLAUDE.md and START.md. No partner wait.

## C6 parity

- Real stdio LSP diagnostics match the library for every sample script.
- Editor payloads match the same library results in Rust tests.
- Real Godot 4.7.2 native Dictionaries match all five sample rule/span sets
  in an isolated project, with Unicode and fixes checked at runtime.
- Headless addon parsing/import and cold-start native tests pass. Late
  hot-loading requires a restart; optional GUI behavior is documented.

## Checks

| Check | Result |
| --- | --- |
| fmt / clippy -D warnings | pass |
| workspace tests | 585 pass |
| explicit root feature-off tests | 569 pass |
| release CLI / host gdextension build | pass |
| Zed adapter host check | pass; optional SDK isolated |
| Godot cold-start/import + native parity/fix smoke | pass, 0 failures |

No tag/release created. W8 must rename the native singleton/library/manifest
and update the smoke's expected artifact names along with the CLI aliases.
Next: cross-file CLI rules, measured 1000-file performance, naming/release
dry run, final agent-focused docs and validation. Avoid further IDE polish.
