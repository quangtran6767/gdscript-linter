# Continue the project

The project now uses one session on the current branch. Both historical
A/B task lists and integration work are completed sequentially. There is no
partner wait or separate merge session.

Resume with:

```text
Read docs/prompts/START.md and continue the solo roadmap on the current branch.
```

- `PROGRESS.md`: current wave and completed checkpoints.
- `START.md`: resume and execution rules.
- `wN-a.md`, `wN-b.md`, `sm0.md`: task checklists; historical track ownership
  and partner waits are superseded by solo mode.
- `MERGE.md`: integration checklist for each wave. Apply it on the current
  branch in solo mode; skip two-person git synchronization.
- `_session_rules.md`: implementation and validation requirements.
- `../handoff/`: completed work, exact APIs, open issues and resume notes.

User instructions take precedence. Keep formatter behavior and compatible
contracts intact; mark progress only when the wave's checks pass.
