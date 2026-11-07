# START: build the whole project, wave by wave, until it's finished

The user pasted this so you would work **through every wave until the end**
without them doing anything else. Two people (A and B) each run this in their
own Claude Code session at the same time, on separate branches. Git keeps the
two sessions in step:

- **A** does A's task each wave, waits for B's branch, then merges both into
  `main` and checks everything (following `MERGE.md`).
- **B** does B's task each wave, then waits until A has merged before
  starting the next wave.

The loop is safe to restart. If the session stops for any reason (limits,
crash, closed terminal), the user pastes the same line again and you continue
where the work left off. All state lives in git: the branches, handoff notes
and `docs/prompts/PROGRESS.md`.

## 0. Once, at the start

1. Find out who you are from the user's message ("I am A" / "I am B"). If it
   isn't there, ask once.
2. If the working tree has uncommitted changes, stop and ask the user what to
   do with them. That's the only time to stop before the loop.
3. Tell the user: "Running all waves as <A/B>. I'll keep going until
   PROGRESS says finished. If I stop, paste the same line again."

## 1. The loop

Repeat until `current_wave` is `finished`:

### 1a. Sync and pick the wave

1. Run `git fetch origin`, `git switch main` and `git pull origin main`.
2. Read `current_wave` in `docs/prompts/PROGRESS.md` and call it WAVE.
3. **Re-read** `docs/prompts/START.md` (this file),
   `docs/prompts/_session_rules.md` and the task file below at the start of
   **every** wave. Don't rely on memory of earlier waves; your context may
   have been compacted.

| WAVE | You are A | You are B |
|------|-----------|-----------|
| `w0`, `sm0` | Branch `WAVE`, task `docs/prompts/WAVE.md` | Nothing to do. Go to **1d (B waits)**. |
| other | Branch `WAVE-a`, task `docs/prompts/WAVE-a.md` | Branch `WAVE-b`, task `docs/prompts/WAVE-b.md` |

### 1b. Do your task

1. **Your work is already done** if `origin/<branch>` contains
   `docs/handoff/<branch>.md` (check with
   `git cat-file -e origin/<branch>:docs/handoff/<branch>.md`). In that case,
   skip to 1c.
2. **Otherwise, set up the branch:**
   - if it exists locally or on `origin`, switch to it and pull, then read
     `git log main..HEAD` to see what's already done (you are resuming);
   - if it doesn't exist, run `git switch -c <branch>` from `main`.
3. Tell the user in one line: "Wave WAVE, track <A/B>: starting."
4. Do the task file completely, following `_session_rules.md`.
5. **Push only when the task is finished.** A pushed handoff note is the
   "done" signal your partner waits for. Before pushing:
   - the handoff note `docs/handoff/<branch>.md` is committed;
   - `cargo fmt --check`, clippy (`-D warnings`) and
     `cargo test --workspace` pass.
   Then run `git push -u origin <branch>`.
6. Tell the user in one line what's done.

### 1c. A merges (only A)

1. In two-person waves, wait until your partner is done, that is, until
   `origin/WAVE-b` contains `docs/handoff/WAVE-b.md` (see "How to wait").
2. Follow `docs/prompts/MERGE.md` completely. It merges, checks, pushes
   `main`, and moves `current_wave` to the next wave.
3. **If MERGE stops** because of failing checks it couldn't fix, or a
   conflict that needs a human: don't continue the loop. Tell the user
   exactly what's wrong, and that your partner's session is waiting. Then
   stop.
4. Go back to 1a.

### 1d. B waits for the merge (only B)

1. Wait until `current_wave` in `origin/main:docs/prompts/PROGRESS.md` is no
   longer WAVE (see "How to wait").
2. Go back to 1a.

## How to wait

- Don't spin in the foreground and don't keep re-running commands by hand.
- Start **one background shell command** that polls every 60 seconds and
  exits when the condition is true. You're notified when it exits.
- **A waiting for B:**

  ```sh
  for i in $(seq 1 720); do
    git fetch -q origin 2>/dev/null
    git cat-file -e origin/WAVE-b:docs/handoff/WAVE-b.md 2>/dev/null && exit 0
    sleep 60
  done; exit 1
  ```

- **B waiting for A:**

  ```sh
  for i in $(seq 1 720); do
    git fetch -q origin 2>/dev/null
    git show origin/main:docs/prompts/PROGRESS.md 2>/dev/null \
      | grep -q '^current_wave: WAVE$' || exit 0
    sleep 60
  done; exit 1
  ```

- If the wait times out (12 hours), tell the user your partner's session
  seems stopped, say what you're waiting for, and stop. Pasting the line again
  resumes.

## When everything is finished

When `current_wave` is `finished`:

- **A** reads every `docs/handoff/*-merge.md` and gives the user a final
  report: what was built, test status, open issues, and the release tags
  they may want to create (`v0.29` to `v1.0`, never created automatically).
- **B** says: "All waves finished. See A's final report."
