# W4 solo integration

## Result

Continued `w4-a`, which already held A7/A8/A9/parallel-cache, and completed
W4-B and C3 sequentially. No branches merged or remote/main changes made.
The user changed to solo work on 2026-11-09; saved orchestration, repository
instructions and progress now explicitly remove partner waits and track
ownership restrictions for the solo session.

## Integration

- Added `unused-ignore` metadata, rationale, examples and safe-fix flag.
  Updated the catalog count test to include the newly documented engine ID.
- Updated the JSON snapshot for the intentional additive
  `summary.baseline_hidden` field. This was the sole initial failing e2e
  test recorded in A's handoff.
- Added full sample GitHub and SARIF snapshots (normalizing tool version),
  and structural SARIF assertions.
- Added baseline round-trip, deterministic rewrite and new-issue coverage.
- Added the tested hook, six executable docs examples, and user guides.
- Applied W4 spec/plan/changelog changes and advanced progress to SM0.

## Validation

| Check | Result |
| --- | --- |
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | pass |
| `cargo build --release` | pass |
| Release smoke checks | 20 pass |
| Formatter regression | workspace fixtures and sample `--check` pass; formatter code untouched |
| Sample source hashes before/after smoke | unchanged |
| Hook tests | clean, dirty, safe-to-clean, non-GD/tool error, forced no-jq fallback pass |
| Complete docs YAML examples | all six rules pass `test-rules` |

Release checks include pretty/JSON, diff, safe fix, unsafe fix, baseline,
GitHub/SARIF, rule tests, metadata, init and hook feedback. Fixes and init
used temporary copies/folders. Detailed session output is under
`/tmp/gdguard-w4-{tests.log,clippy.log,smoke.json}` (ephemeral).

## E2E excerpts

```text
5 files: 8 errors, 9 warnings, 2 infos, 4 safe-fixable issues
Fixed 4 problems in 2 files
19 issues hidden by baseline.
```

The baseline test also introduces `scripts/new.gd`: only its new issue is
reported, with 19 existing issues hidden.

## Open issues and next wave

- No real-game path was supplied. Recommend real-game dogfooding before
  v0.30; no release tag was created.
- SARIF fixes are still deferred, as recorded by W4-A.
- Preexisting W3 caveats are retained in `w4-b.md`.
- Next is SM0's semantic/project/JS contracts. Implement both historical
  tracks of every later wave on the current branch, then integrate locally.
- Git metadata is read-only in the ordinary sandbox. Implementation and
  checks are unaffected; committing requires the git-write escalation.
