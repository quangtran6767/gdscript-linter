# SM0 solo integration

## Result

Completed the single-session contract wave directly on `w4-a`. No branch
switch, merge, main push or release tag was needed. All newly frozen types
and exact signatures are in `sm0.md` and the source/type declaration files.

## Integration

- Optional semantic/project references compile at every FileContext
  construction site; existing YAML and formatter behavior remains intact.
- Semantic/project, optional JS runtime/bindings, and LSP module boundaries
  are wired before implementation starts in later waves.
- QuickJS is default-enabled and optional. The explicit root package
  feature-off dependency tree contains no QuickJS crate.
- The strict JS type check of the spec's sample rule exposed nullable node
  fields and untyped options. Updated that draft example to declare its
  options and handle missing parameters/name fields.
- Spec, plan and changelog describe the exact SM0 boundary, including the
  empty-model status, and progress advances to W5.

## Validation

| Check | Result |
| --- | --- |
| `cargo fmt --check` | pass |
| `cargo clippy --workspace --all-targets -- -D warnings` | pass |
| `cargo test --workspace` | 548 tests pass |
| `cargo build --release` | pass |
| `cargo build --no-default-features` | pass |
| `cargo clippy --workspace --all-targets --no-default-features -- -D warnings` | pass |
| `cargo test --workspace --no-default-features` | 548 tests pass |
| `cargo test -p gdscript-formatter --no-default-features` | 548 tests pass |
| Strict TypeScript declaration and checked-JS example | pass |
| Release smoke | five pass: sample formatter check, JSON lint, diff, rule tests, explain |

The explicit root feature-off check matters: workspace feature unification
can enable the root package's default features through gdextension.

## E2E output

Release lint JSON remains unchanged: 5 files, 8 errors, 9 warnings, 2 infos,
4 safe-fixable issues, 0 baseline-hidden. Formatter fixtures and the sample
project's formatter check continue to pass.

## Open issues / next wave

- SM0 intentionally implements contracts only. Semantic builders, project
  indexing, JS execution and LSP are pending W5/W6/W7 respectively.
- W5 must use the nullable symbol API and `.d.ts` units exactly. Avoid
  copying a whole syntax tree into JS; keep lazy node access.
- Implement W5-A, W5-B, then C4 on the same branch. No partner wait.
- Current repository/build version remains 0.28.0; no release has been made.
