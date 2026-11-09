# W6 solo integration

Completed both tracks and C5 on the current branch. No partner wait, main
merge or release tag was required.

## Integration and intentional output changes

- Added semantic metadata and built-in registration; all metadata examples
  are checked by the existing unit tests.
- C5 runs a JS rule on a temporary sample copy and checks project class
  names, autoloads, Yes/Unknown member results and semantic built-in output.
- Sample snapshots/baseline counts now include its unused OnDefeated signal
  (20 issues rather than 19). Other new lint fixture diagnostics are real
  unused/shadowed declarations, retained explicitly in the goldens.
- Extended the all-rules span fixture with new semantic-rule triggers;
  every registered built-in still has a nonempty, byte-consistent span.
- Updated catalog ordering/padding assertions for the new sorted IDs.
  Existing class-name JSON metadata is still checked exactly.
- Project-wide cache invalidation is tested by changing Data's members in
  another script between cached lint runs.

## Checks

Format, clippy (-D warnings), workspace tests, explicit root feature-off
tests, strict declaration/JS checks, release build, 13 example rules and
sample formatter regression pass. Exact test totals are recorded in
PROGRESS. Release smoke verifies JSON/pretty, diff/fixes on temp copies,
baseline round trip, metadata and the example pack.

## Next

W7 implements the stdio language server and Godot panel, plus diagnostics
parity tests. Existing project summaries are disk-based; live document
semantics must continue to use the in-memory buffer. Manual GUI/editor
checks should be reported honestly if the applications are unavailable.
