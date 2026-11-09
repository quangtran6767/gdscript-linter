# Progress

Solo mode since 2026-11-09: one session completes both tracks and
integration on the current branch (`w4-a`). No partner waits.

The solo integration step updates this file after validation.

current_wave: w6

| Wave | Who works | Status |
|------|-----------|--------|
| w0  | A only | done |
| w1  | A and B | done |
| w2  | A and B | done |
| w3  | A and B | done |
| w4  | solo (A + B + integration) | done |
| sm0 | solo | done |
| w5  | solo (A + B + integration) | done |
| w6  | solo (A + B + integration) | todo |
| w7  | solo (A + B + integration) | todo |
| w8  | solo (A + B + integration) | todo |

## Resume notes

- Current branch: `w4-a`; do not wait for or require `w4-b`.
- W4-A was already committed. W4-B and integration are complete; see
  `docs/handoff/w4-b.md` and `docs/handoff/w4-merge.md`.
- SM0 contracts and validation are complete; see `docs/handoff/sm0.md`
  and `docs/handoff/sm0-merge.md`.
- W5 complete: semantic builder (18 fixtures), optional JS rules, C4 e2e.
  See `docs/handoff/w5-{a,b,merge}.md`.
- W5 checks: 564 workspace tests; 553 feature-off tests; clippy, release,
  feature-off build and release semantic smoke pass. Semantic overhead ~12%.
- Next: both W6 tasks and C5 integration, then W7 and W8 without stopping.
- User explicitly authorized commits and pushes to origin/w4-a on 2026-11-09.
- No real-game project path was provided; sample dogfooding passed.
