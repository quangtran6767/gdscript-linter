# Handoff notes

Sessions talk to each other only through the notes in this folder.

- Each track session writes `<branch>.md` here when it finishes, for example
  `w2-a.md` or `w2-b.md`, using the template in
  `docs/prompts/_session_rules.md`.
- The merge session writes `<wave>-merge.md`, for example `w2-merge.md`.
- Single-session waves write `w0.md` and `sm0.md`.
- Every file has one owner, the session with that name, so notes never
  conflict when branches merge.
- A pushed handoff note is the "done" signal the other sessions wait for. Push
  it only when the task is finished.

Read every earlier note before you start a wave, especially the "Public API
added" sections: they list what you can now call. `w0.md` records the tool's
final name on its first line (`Tool name: <name>`).
