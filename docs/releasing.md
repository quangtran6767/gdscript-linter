# Reviewing and preparing a release

Build and validate before packaging:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p gdguard --no-default-features
cargo build --locked --release --workspace
cargo run --bin make_release -- --dry-run
python3 .github/package_release.py --output /tmp/gdguard-release
```

Python 3.11+ is required for the packager. The helper defaults to dry run,
validates existing inputs and prints the exact archive manifest. `--output`
creates archives; it does not edit versions, update dependencies, commit,
tag, push or publish. `--only cli common` can omit optional native assets.
`--binary-dir` and `--platform` select already-built cross-target artifacts.

Inspect ZIP contents and run both extracted CLI binaries. The rule archive
includes `gdguard.d.ts` and all example rules; the native archive includes
the matching platform library, manifest and lint/formatter addons. Cold-start
Godot after copying native assets. Each native archive is platform-specific.

A maintainer chooses a version explicitly. Update root Cargo.toml, Cargo.lock,
addon versions, native package version if appropriate, and CHANGELOG before
creating a matching plain `X.Y.Z` tag. The workflow refuses a tag whose version
differs from the root package. Completed roadmap milestones are not published
release numbers until a maintainer prepares a release.

Pushing an explicitly selected tag triggers six platform/architecture builds,
packages CLI/legacy alias/native libraries and shared addon/rule assets, then
creates a **draft** GitHub release for review. Review all platform jobs and
assets before publishing it. Local Linux validation cannot establish that
all remote targets compile. No tag or release is created as part of roadmap
completion. Nothing is ever sent upstream to GDQuest.
