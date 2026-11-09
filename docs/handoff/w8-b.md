# W8-B: rename, release, docs, upstream (solo)

## Summary

The tool ships as `gdguard`. `gdscript-formatter` stays as a compatibility
executable with unchanged formatter behavior. Release packaging is scripted
and dry-run tested. Docs are CLI-first; the README was shortened to the
CLI/agent loop on maintainer request (2026-11-09) and no longer covers IDE
or extension setup (that stays in `docs/editors.md`).

## Tasks

- [x] Rename: Cargo package/crate and primary binary `gdguard`
  (`src/main.rs`), legacy binary `gdscript-formatter` (`src/legacy_main.rs`),
  native library `gdguard_gdextension`, singleton `GDGuard`, addon folders
  `addons/gdguard`, `addons/gdguard_standalone`, `addons/gdguard_lint`
  (old `GDQuest_GDScript_formatter*` folders removed). Pre-commit hooks:
  `gdguard`, `gdguard-lint`, plus legacy IDs.
- [x] Release: `.github/workflows/release.yml` builds six OS/arch targets
  on tags and creates a draft release. `.github/package_release.py` builds
  CLI + alias, native, addon, and `.d.ts` + example-rule archives.
  `cargo run --bin make_release -- --dry-run` reviews inputs without touching
  versions, git or tags.
- [x] Docs: README, `docs/migration.md`, `docs/faq.md`,
  `docs/releasing.md`; every `docs/*.md` reviewed for the new names. Local
  links checked.
- [x] Upstream: `docs/upstream_proposal.md`. No PRs opened.

## Release dry run (2026-11-09)

- `cargo build --locked --release --workspace`: produces `gdguard`,
  `gdscript-formatter`, `make_release`, `libgdguard_gdextension.so`.
- `python3 .github/test_package_release.py`: 2 tests OK.
- `make_release --dry-run`: lists binaries, addon, types and 13 example
  rules; nothing written.

## Decisions

- No version tag or release is created; that is a maintainer action
  (`docs/releasing.md`).
- Formatter EditorConfig keys and legacy hook IDs are kept for migration.

## Manual commands

```sh
./target/release/gdguard --help
./target/release/gdscript-formatter --version
cargo run --bin make_release -- --dry-run
```
