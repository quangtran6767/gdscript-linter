# W8 solo integration

W8-A and W8-B landed on `w4-a`. The spec, plan and CHANGELOG are updated.
All waves in the roadmap are done. The CLI and agent loop are the release
focus; no tag has been created.

## Checks (2026-11-09)

| Check | Result |
| --- | --- |
| `cargo fmt --check` / clippy `-D warnings` | pass |
| `cargo test --workspace` | 591 pass |
| feature-off (`--no-default-features --features gdscript`) | 575 pass |
| release build, both executable names | pass |
| 1000-file benchmark | cold 76.6 ms, warm 48.4 ms, identical output |
| release packaging tests + `make_release --dry-run` | pass |
| Godot 4.7.2 cold-start/import + native parity smoke | 0 failures, 5 fixtures |
| README commands on a temp sample project | init, lint --fix, explain, list-rules, test-rules, github format, --check, format all behave as documented |

## Spec / changelog

Spec: parser diagnostics section, cross-file rule details, final names,
release and benchmark notes. CHANGELOG: Added/Changed entries for W8.

## Remaining (maintainer decisions)

- Choose the version and push a tag to trigger the draft release.
- Decide whether to send any `docs/upstream_proposal.md` PRs to GDQuest.
- Dogfood on a real game project (none provided so far).
