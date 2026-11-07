# How to work

Each of you pastes **one line, once**. Both Claude Code sessions then work
through every wave until the project is finished: they build, test, push,
merge and check, and move on to the next wave.

**You:**
```
Read docs/prompts/START.md and follow it. I am A.
```

**Your friend:**
```
Read docs/prompts/START.md and follow it. I am B.
```

What happens:

- **Each wave:** both sessions work at the same time on their own branches.
  A's session then waits for B's branch, merges both into `main`, runs every
  check, and pushes. B's session waits for that merge, and both continue with
  the next wave.
- **`w0` and `sm0`:** only A works. B's session simply waits.
- **The only question:** A's session asks for the tool's final name, once, at
  the start.
- **If a session stops** (usage limit, crash, closed terminal): paste the
  same line again. It continues where it left off.
- **If a merge fails** in a way Claude can't fix safely, A's session stops
  and explains why. Fix the problem (or tell Claude how), then paste the line
  again.

## Before you paste

1. **Same repo:** both of you work in a clone of the same GitHub repo, and
   you can both push to it.
2. **No permission prompts:** the sessions need to run `cargo`, `git` and
   shell commands without stopping to ask. Use auto mode, or allow those
   commands in `.claude/settings.json`.
3. **Leave it running:** keep both terminals open. A full run takes a long
   time and a lot of tokens; expect several restarts because of usage
   limits.
4. **Check on it:** whenever you like, read `PROGRESS.md` and the merge
   reports in `docs/handoff/*-merge.md`.

---

- `PROGRESS.md`: which wave is current.
- `wN-a.md` / `wN-b.md`: each person's task per wave.
- `MERGE.md`: what A's session does after each wave.
- `_session_rules.md`: the shared rules.
- `../handoff/`: notes the sessions leave for each other.
