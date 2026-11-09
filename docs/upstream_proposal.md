# Possible contributions to GDQuest

These are proposals for maintainer review. No upstream PR has been opened.
The fork's CLI/agent product and custom-rule roadmap can ship independently.

1. **Precise spans and diagnostic data.** Extract source positions, exclusive
   end ranges, structured messages and help from the linter. Preserve existing
   formatter output and expose additive library types. Start with naming and
   control-flow span fixtures, including multibyte source, without changing
   reporter defaults.
2. **Machine-readable reporters.** Add opt-in JSON output on top of those
   diagnostics. Document a versioned schema, deterministic sorting, byte
   positions and exit codes. Include subprocess snapshots, escaping and
   config failure tests. Offer GitHub/SARIF separately if maintainers want them.
3. **Guarded fix engine.** Extract edit validation, conflict handling,
   deterministic application, parse rollback and idempotency tests. Start
   with unnecessary-pass and no-else-return. Keep lint fixes distinct from
   formatting, and preserve formatter regression outputs.

Each proposal should be rebased onto the then-current upstream branch and
reviewed for its API/dependency policy. Avoid bundling the rename, QuickJS,
project index, agent hooks or editor integrations into general formatter PRs.
Discuss acceptance before doing that rebase; the split above describes
possible boundaries and is not evidence of upstream approval.
