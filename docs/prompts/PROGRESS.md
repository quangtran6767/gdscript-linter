# Progress

Solo mode since 2026-11-09: one session completes both tracks and
integration on the current branch (`w4-a`). No partner waits.

The solo integration step updates this file after validation.

current_wave: w5

| Wave | Who works | Status |
|------|-----------|--------|
| w0  | A only | done |
| w1  | A and B | done |
| w2  | A and B | done |
| w3  | A and B | done |
| w4  | solo (A + B + integration) | done |
| sm0 | solo | done |
| w5  | solo (A + B + integration) | todo |
| w6  | solo (A + B + integration) | todo |
| w7  | solo (A + B + integration) | todo |
| w8  | solo (A + B + integration) | todo |

## Resume notes

- Current branch: `w4-a`; do not wait for or require `w4-b`.
- W4-A was already committed. W4-B and integration are complete; see
  `docs/handoff/w4-b.md` and `docs/handoff/w4-merge.md`.
- SM0 contracts and validation are complete; see `docs/handoff/sm0.md`
  and `docs/handoff/sm0-merge.md`.
- Next: W5 semantic implementation (`w5-a.md`), JS runtime (`w5-b.md`),
  and C4 integration. Neither W5 implementation has started.
- SM0 models intentionally remain empty; JS execution is not implemented.
- Checks: 548 workspace tests, explicit root feature-off tests, default and
  feature-off clippy/build, strict TS API/example checks, release smoke pass.
- No real-game project path was provided; sample dogfooding passed.
