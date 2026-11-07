# MERGE: merge the finished branches into main and check everything

The user pasted this after both people pushed their branches. Your job is to
merge them into `main`, do the wave's integration work, test everything,
report, and move `PROGRESS.md` to the next wave. Work on your own; stop only
for the cases this file names.

You own `CHANGELOG.md`, `docs/specification_linter.md`,
`docs/implementation_plan.md`, `CLAUDE.md`, `docs/prompts/PROGRESS.md` and
`tests/e2e/`. Follow `docs/rust_coding_guidelines.md` for any code you write.

Below, **WAVE** means the value of `current_wave` in
`docs/prompts/PROGRESS.md`, for example `w2`.

## 1. Check before merging

1. If the working tree has uncommitted changes, stop and ask the user. Then
   run `git fetch origin`, `git switch main` and `git pull origin main`.
2. Read WAVE from `docs/prompts/PROGRESS.md`. The branches to merge are:
   - `w0` / `sm0`: only the branch `WAVE`;
   - otherwise: `WAVE-a` and `WAVE-b`.
3. Every branch must exist on `origin`. If one is missing, stop and say
   which person hasn't pushed yet.
4. Read each branch's handoff note completely (`docs/handoff/<branch>.md`,
   via `git show origin/<branch>:docs/handoff/<branch>.md`). If one is
   missing, stop: that session didn't finish.
5. Read `docs/implementation_plan.md` for this wave and the integration table
   below.

## 2. Merge, one branch at a time

1. `git merge --no-ff origin/WAVE-a` (or `origin/WAVE` for w0/sm0), then run
   `cargo test --workspace`.
2. `git merge --no-ff origin/WAVE-b`, then run `cargo test --workspace`.
   Skip this for w0/sm0.

Handle conflicts like this:

- **`Cargo.toml`:** keep both sides' dependencies. For `Cargo.lock`, take
  either side, then run `cargo build` to regenerate it.
- **Module lists, test lists and other additive files:** keep both sides.
- **Anything else:** understand both sides before resolving. If the two
  tracks made incompatible choices, stop and explain the options to the
  user. Don't guess.

If tests fail after a merge, find which branch causes it by running the tests
on each branch alone.
- Fix it in a separate commit `[WAVE-merge] fix: …` only if the fix is
  small and obvious.
- Otherwise stop and report the failure, the branch, and a suggested fix.

## 3. Integration work

Do the "Requests for the merge session" items from both handoffs, plus the
row for this wave below. Commit each as `[WAVE-merge] …`.

| Wave | Integration work |
|------|------------------|
| w0, sm0 | None. Check that the contracts compile and that the handoff lists every signature. |
| w1 | Make sure B's loader compiles against A's migrated pipeline. Apply B's metavariable spec text. |
| w2 | **C1:** call `custom::load_providers` in the pipeline at the place A's handoff names. Custom levels come from `gdguard.toml`, and config validation accepts custom ids. E2E: copy 2 example rules into `tests/e2e/sample_project/.gdguard/rules/`, add a `gdguard.toml`, and snapshot `lint --format json`. |
| w3 | **C2:** E2E `--fix` on the sample project fixes a custom-rule issue, then `test-rules` on `examples/rules` passes. The CLI wiring for `--list-rules` is connected to `explain`. Release **v0.29** (ask the user before tagging). |
| w4 | **C3:** add `unused-ignore` metadata to `rules_meta.rs`. E2E: baseline round-trip, plus `github`/`sarif` snapshots on the sample project. The hook script runs on a sample file. In the report, recommend that the user dogfood on their game before tagging **v0.30**. Don't wait for it. |
| w5 | **C4:** make sure `context.semantic` in JS uses A's real model. Add an e2e JS rule that uses `semantic.lookup` on the sample project. |
| w6 | **C5:** add A's semantic-rule metadata to `rules_meta.rs` if B didn't. E2E: a JS rule using `context.project` and a semantic built-in on the sample project. Release **v0.31** (ask first). |
| w7 | **C6:** parity test: the LSP diagnostics and gdextension `lint()` give the same set of (rule, span) results on every sample file. Release **v0.32** (ask first). |
| w8 | Apply B's rename requests to A's new files. Full re-read of the spec against the code, then fix the spec. Release **v1.0** (ask first). |

## 4. Test everything

Run all of these and record the results:

1. `cargo fmt --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo build --release`, plus `--no-default-features` from W5 on
5. E2E smoke test on `tests/e2e/sample_project` with the release binary:
   - `lint` (pretty and json);
   - `lint --diff`;
   - `lint --fix` on a temporary copy;
   - `test-rules`;
   - `explain <one rule>`;
   - each feature this wave added.
   Each command must give the expected exit code and sensible output.
6. **Formatter regression:** the formatter tests still pass, and formatting
   `tests/input` gives exactly the same results as on `main` before this
   merge (`git stash` or a worktree of `HEAD~` before your first merge
   commit).
7. Every command in both handoffs' "How to test manually" section.

Extend `tests/e2e/` so this wave's features are covered automatically next
time.

## 5. Docs and report

1. Apply the "Spec updates" from both handoffs to
   `docs/specification_linter.md`. Mark this wave's tasks done in
   `docs/implementation_plan.md`.
2. Add both handoffs' "CHANGELOG lines" under "Unreleased" in
   `CHANGELOG.md`.
3. Write `docs/handoff/WAVE-merge.md` with:
   - the result of the merge (conflicts and how you resolved them);
   - the integration work done;
   - a test results table (command, result);
   - E2E output excerpts;
   - open issues to carry into the next wave;
   - suggested changes to the next wave's prompts.
4. In `docs/prompts/PROGRESS.md`, mark WAVE `done` and set `current_wave`
   to the next row of the table (after `w4` comes `sm0`; after `w8`, write
   `finished`).
5. Commit.
6. **Push only if every check in section 4 passed:** `git push origin main`.
   If anything failed and you couldn't fix it, don't push. Explain the
   problem and stop.
7. Never create release tags yourself. When this wave's row says "Release",
   tell the user the command to run if they want it.
8. Print a short summary:
   - merged OK or not;
   - test status;
   - open issues;
   - the next wave.
9. If you were started from `START.md`, return to its loop (step 1a).
   Don't stop.
