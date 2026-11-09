# Session rules: solo continuation

The user now works alone (2026-11-09). Historical A/B prompts are task
checklists for one session. Follow `START.md`; never wait for another person.

## Read first

- `CLAUDE.md`, `PROGRESS.md`, and the current wave's prompts.
- Relevant specification sections and shared contracts.
- The wave in `docs/implementation_plan.md`.
- `docs/rust_coding_guidelines.md`.
- Earlier handoffs, particularly public APIs and integration requests.

## Scope and branches

Continue the current branch. The solo session owns both historical tracks
and integration files, including specs, plan, changelog and e2e tests. Keep
user changes. Do not reset, force-push, or create release tags. Do not create
branches or merges merely to imitate the former two-person workflow.

Frozen signatures and schemas must remain compatible; use additive changes
where the wave authorizes them. Add dependencies only where the wave calls
for them, with a reason in its handoff. Keep formatter behavior unchanged.

## Implementation and checks

Briefly identify the tasks, files and tests, then implement without waiting
for approval. Choose spec-consistent defaults and record decisions. New
behavior needs meaningful tests and docs. Do not weaken existing tests.

Run `cargo fmt --check`,
`cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`, release build and applicable feature-off checks.
Check the CLI using the sample project, applying fixes only to temp copies.
Commit completed tasks as `[<wave>-solo] <task>: <description>` when git
writes are available. A handoff alone does not prove tests pass.

## Handoffs

Use `docs/handoff/<wave>-a.md`, `<wave>-b.md` and `<wave>-merge.md` (or the
single-wave name for SM0). Include summary, task status, changed files,
exact added public APIs, decisions, integration requests, spec updates,
changelog lines, manual test commands, and known issues. Keep any remaining
work explicit. `PROGRESS.md` advances only after wave acceptance passes.
