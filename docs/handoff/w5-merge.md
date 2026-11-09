# W5 solo integration

Completed both tracks on `w4-a`, without a partner branch or merge. The
semantic API and JS rule API work together through the shared pipeline.

## C4

- Added an e2e JS rule on a temporary sample-project copy that calls
  semantic.scopeAt/lookup/symbols/referencesTo and resolves inventory's
  declared Array type. It produces a real custom diagnostic.
- Fixed the temporary-project test helper to preserve JSON escapes rather
  than replacing every backslash as if JSON were plain path text.
- Added engine metadata/catalog/config support for script-error.
- Protected engine IDs against custom-rule collisions across YAML and JS.
- Extended cache hashing to JS rules and bumped the cache format version.
- Updated the mixed example-pack CLI expectation from 8 YAML rules to 10
  rules. YAML-only tests retain their original assertions.

## Validation

Format, clippy with warnings denied, workspace tests, explicit root
feature-off tests/build and release build pass. Existing formatter fixtures
and sample formatting remain unchanged. Exact test totals and latest
timings are recorded in PROGRESS/session logs.

`cargo run --release --example semantic_benchmark` measures both semantic
and JS overhead; W5-A/B record the results. Semantic model overhead is
approximately 12%, below the 30% target. The complete example pack passes
test-rules including fix/idempotency checks for YAML and JS fixtures.

## Next

W6: semantic built-ins, cached project index, project bindings/examples/docs.
No release tags created. Keep working solo on the current branch and push
completed checkpoints to the already authorized origin/w4-a destination.
