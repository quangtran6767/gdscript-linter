# Session rules (every Claude Code session reads this first)

You are one of two Claude Code sessions building this linter **at the same
time**, in separate branches. Another session is working on the other track
right now. When you both finish, a merge session merges both branches into
`main` and tests the result. Everything below exists so that merge is easy.

## 1. Read before you code

1. `CLAUDE.md`
2. `docs/specification_linter.md`: the sections your wave prompt names, plus
   "Shared contracts".
3. `docs/implementation_plan.md`: your wave's section and the "File
   ownership" table.
4. `docs/rust_coding_guidelines.md`: follow it strictly (plain loops, no
   closures where a loop works, no `unwrap()` outside tests, `with_capacity`
   when you know the size).
5. Every file in `docs/handoff/` from earlier waves, especially the "Public
   API added" sections, which tell you what you can now use.

## 2. Branch rules

- `START.md` already created your branch (`wN-a`, `wN-b`, or `w0`/`sm0`).
  Check it with `git branch --show-current`. If you're on `main`, stop and
  tell the user.
- Commit only to that branch. Never commit to `main`, merge, rebase onto the
  other track's branch, or force-push.
- Don't push until your whole task is finished. Your partner's session
  treats a pushed handoff note as "done".
- Commit after each finished task with this message format:
  `[w2-a] A2: config file discovery and precedence`.

## 3. Scope rules (these prevent merge conflicts)

- **Edit only the files your wave prompt lists under "You may edit".** You
  may read anything.
- **Never edit these.** The merge session owns them:
  - `CHANGELOG.md`
  - `docs/specification_linter.md`
  - `docs/implementation_plan.md`
  - `CLAUDE.md`
  - `tests/e2e/`
  - the other track's files
- **Frozen contracts** must not change: `src/linter/diagnostic.rs`, the
  signatures in `src/linter/api.rs`, the `src/semantic/` and `src/project/`
  signatures, and `integrations/types/gdguard.d.ts`. If a contract is
  genuinely missing something, work around it with a local helper, and
  describe the needed additive change in your handoff under "Requests".
- **Dependencies:** don't add crates unless your prompt allows it. If you add
  one, say why in the handoff.
- **If you need something from the other track,** don't build it yourself.
  Use a minimal local stub or a test double, and record the request in the
  handoff.

## 4. How to work

1. **Plan:** list the tasks from your prompt, the files you'll touch, and the
   tests you'll write. Keep it short, then start. Don't wait for approval
   unless the prompt says to.
2. **Implement** one task at a time. After each task:
   - `cargo fmt`
   - `cargo clippy --workspace --all-targets -- -D warnings`
   - `cargo test --workspace`
   - commit
3. **Tests are part of the task.** Every new behavior gets tests: unit tests
   next to the code, fixtures in the `tests/` folder your prompt names.
4. **Never weaken tests to pass.** Never delete or loosen an existing test to
   make it pass. If an expected output has to change because of your task,
   change it and explain why in the handoff.
5. **When something is unclear,** pick the option most consistent with the
   spec, write it down under "Decisions" in the handoff, and keep going. Stop
   and ask the user only when you're truly blocked, for example a contract
   that can't work, or a spike result that changes the plan.
6. **Keep the formatter unchanged.** Its tests must still pass.

## 5. Finishing

You're done only when:

- [ ] every task in your prompt is done, or listed as not done with a reason;
- [ ] `cargo fmt --check`, clippy (`-D warnings`) and `cargo test --workspace`
      all pass;
- [ ] `docs/handoff/<branch>.md` is written using the template below and
      committed;
- [ ] the branch is pushed with `git push -u origin <branch>`.

Then print a five-line summary for the user: what's done, what's not, and
anything the merge session must do.

## 6. Handoff template (`docs/handoff/<branch>.md`)

```markdown
# Handoff <branch>

## Summary
Two or three sentences.

## Tasks
| Task | Status (done/partial/not done) | Notes |

## Files changed
- path: one line on what changed

## Public API added
Exact signatures other code may call (functions, structs, CLI flags, JSON fields).

## Decisions
Choices you made where the spec was silent or unclear, and why.

## Requests for the merge session
Contract additions, wiring, or spec text to apply. Be exact.

## Spec updates
Text the merge session should add to or change in docs/specification_linter.md.

## CHANGELOG lines
- One line per user-visible change.

## How to test manually
Commands that show the feature working.

## Known issues / follow-ups
```
