# Implementation plan

**Solo workflow (2026-11-09):** one maintainer now completes both historical
tracks and integration in order, on the current branch. Track names and
ownership below remain useful task groupings, but do not require another
person, branch, handoff, or waiting. `docs/prompts/START.md` is the entry
point; `docs/prompts/PROGRESS.md` records the exact resume position.

This plan covers the full scope of `docs/specification_linter.md`, from the
first contracts to the 1.0 release. Two developers work in parallel the whole
time:

- **A = you**
- **B = your friend**

You can swap the letters; just keep them fixed once work starts.

The work is split into **waves**. In each wave, A and B each run their own
**Claude Code session at the same time**, on separate branches (`wN-a`,
`wN-b`) and in **separate files**. When both finish, a **merge session**
merges both branches into `main`, does the integration work (the "Sync"
column below), and tests everything.

- **How to run it:** `docs/prompts/README.md` (two copy-paste lines). The
  per-wave task files are `docs/prompts/wN-a.md` and `wN-b.md`.
- **W0 and SM0** are single-session waves that freeze the shared contracts.
  One person runs them before the next parallel wave starts.
- Sessions talk to each other only through handoff notes in
  `docs/handoff/`.

## Overview: who works on what at the same time

| Wave | A (you) works on | B (your friend) works on, at the same time | Sync at end of wave | Release |
|------|------------------|---------------------------------------------|---------------------|---------|
| **W0** | **Together:** core contracts, library API stubs, CLI flag stubs, empty modules | (together) | Merge W0 PR | |
| **W1** | Migrate built-in rules to `Diagnostic`; exit codes; library API body (lint only) | ast-grep spike; YAML rule loader | Merge both | |
| **W2** | Config file (`gdguard.toml`); reporters (pretty, compact, json) | YAML rule engine (`RuleProvider`); `test-rules` command | **C1:** A wires custom rules into the pipeline, B reviews | |
| **W3** | Fix engine (`--fix`, `--fix-unsafe`, `--diff`, stdin); fixes for built-ins | Starter rule pack; `explain` and built-in rule metadata; `init` | **C2:** custom-rule fixes work through `--fix` | **v0.29**: custom rules + autofix |
| **W4** | Ignore comments v2; baseline; github and sarif reporters; parallel linting and cache | Claude Code hook and skill; docs for custom rules and agents; dogfood on your game | **C3:** run on a real project for a few days and fix the bugs found | **v0.30**: ready to adopt |
| **SM0** | **Together:** freeze the semantic model, project index and JS API contracts | (together) | Merge SM0 PR | |
| **W5** | Semantic model (scopes, symbols, references) | JS runtime: sandbox, loader, syntax-only node API, JS in `test-rules` | **C4:** B exposes `context.semantic` to JS, A reviews | |
| **W6** | Semantic built-in rules; project index | JS bindings for project data; `.d.ts` types; JS example rules; JS docs | **C5:** JS rules can use the semantic model and project index | **v0.31**: semantic + JS rules |
| **W7** | LSP server (`gdguard lsp`) | Godot editor panel (gdextension + addon) | **C6:** both run on the same project and the same config | **v0.32**: editors |
| **W8** | Cross-file rules; performance and benchmarks | Rename and release pipeline; CLI docs | Final review | **v1.0** |

Rough size: W1 to W4 are the biggest waves. If each person can put in a few
focused days per wave, the whole roadmap takes weeks, not months. AI agents
speed up the mechanical tasks a lot, especially A1 and B5.

## File ownership

Edit only the files you own **in the current wave**. Everything else is
read-only for you. If you really need a change, make a separate tiny PR and
tell the other person.

| Path | Owner | Notes |
|------|-------|-------|
| `src/linter/diagnostic.rs`, `src/linter/api.rs` (signatures) | **Both, frozen** | Additive changes only, in their own PR, approved by both. A fills in the `api.rs` function bodies. |
| `src/linter.rs`, `src/linter/rules/`, `src/linter/ignore_patterns.rs` | A | |
| `src/linter/config.rs`, `fix.rs`, `baseline.rs`, `cache.rs`, `output/` | A | |
| `src/cli.rs`, `src/main.rs` | A | B's commands are stubbed in W0 and call B's functions, so B never edits these |
| `src/semantic/`, `src/project/`, `src/lsp/` | A | |
| `tests/lint/`, `tests/semantic/`, `tests/lsp/` | A | |
| `src/linter/custom/`, `rule_tester.rs`, `explain.rs`, `rules_meta.rs` | B | |
| `src/linter/script/` (JS engine) | B | |
| `examples/rules/`, `integrations/` (hooks, skill, `.d.ts`) | B | |
| `tests/custom_rules/`, `tests/js_rules/` | B | |
| `gdextension/`, `addons/` | **A in W1 only** (API migration), **B from W2 on** | |
| `.github/workflows/release.yml`, `README.md` | B (from W4) | A asks B for changes |
| `docs/configuration.md`, `docs/editors.md` (LSP part) | A | |
| `docs/custom_rules.md`, `docs/js_rules.md`, `docs/agents.md`, `docs/editors.md` (Godot part) | B | |
| `src/linter/init.rs` | B | |
| `docs/specification_linter.md`, `docs/implementation_plan.md`, `CLAUDE.md`, `CHANGELOG.md`, `tests/e2e/` | **Merge session** | Track sessions put their spec text and CHANGELOG lines in their handoff note |
| `docs/handoff/wN-a.md` / `wN-b.md` / `wN-merge.md` | The session with that name | One file per session, so they never conflict |
| `Cargo.toml`, `Cargo.lock` | W0/SM0, plus a track only when its prompt allows | The merge session keeps both sides and rebuilds the lock |

---

## W0: core contracts (together, about 2 hours, one PR)

- [x] Decide the tool name and replace the `gdguard` placeholder in the docs.
- [x] Add dependencies: `serde` (derive), `serde_json`, `toml`, `globset`.
- [x] `src/linter/diagnostic.rs`: every type from the spec, `RuleProvider`,
      `FileContext`, and `apply_edits` **with unit tests**.
- [x] `src/linter/api.rs`: `load_config_for`, `lint_source`, `fix_source`
      with the final signatures and stub bodies returning
      `Err("not implemented yet")` (no `todo!()`, so callers don't panic).
      Also a `Config` struct with the fields the spec lists, so B can build it
      by hand in tests.
- [x] Empty modules (each with `mod` lines) for every A and B file in the
      ownership table that W1 to W4 need.
- [x] `src/cli.rs`: parse **every** flag and subcommand from the spec (`lint`
      flags, `explain`, `test-rules`, `init`, `lsp`). B's commands call
      `rule_tester::run`, `explain::run` and `init::run`, which are stubs
      returning exit code 2 for now.
- [x] Agree on `custom::load_providers(config: &Config) -> Result<Vec<Box<dyn RuleProvider>>, String>`.
- [x] `cargo clippy ... -D warnings` and `cargo test --workspace` pass. Merge,
      then both branch off.

## W1: A and B at the same time

**A: `a/diagnostic-migration`, `a/exit-codes`**
- [x] A1. Make the pipeline produce `Diagnostic`. Port all 17 built-in rules
      to real spans and add `help` text where it's obvious.
- [x] Update `gdextension/src/lib.rs` to the new type. This is A's only
      gdextension work.
- [x] Update the `tests/lint/` expected files. The old text output format
      must stay the same.
- [x] A5. Exit codes (0/1/2) and `--max-warnings`.
- [x] Fill in the `api::lint_source` body (with no custom rules yet).

**B: `b/astgrep-spike`, `b/yaml-loader`**
- [x] B1. **Spike, time-boxed to half a day.**
  - Build `ast-grep-core`/`ast-grep-config` 0.51 against tree-sitter-gdscript
    by implementing their `Language` trait.
  - Prove these work: `len($X) == 0`, `get_node($$$A)`, `inside`/`has`, a fix
    template, and a way to escape `$NodePath`.
  - Write the go/no-go decision in the PR description, and update the spec's
    metavariable escape note.
  - On no-go, fall back to a `query:` field (raw tree-sitter query) with the
    same YAML envelope.
- [x] B2. YAML loader:
  - finds rule files in `custom_rules.paths`;
  - validates the schema and checks for duplicate ids;
  - maps each rule to `RuleMeta`;
  - gives errors that name the file and the key.
- [x] Tests for every loader error.

**Merge session:** merge both branches and test.

## W2: A and B at the same time

**A: `a/config`, `a/reporters`**
- [x] A2. `gdguard.toml`:
  - upward discovery;
  - `[rules]` levels and options, `[lint].exclude`, `[[overrides]]`;
  - precedence (CLI, then config, then editorconfig, then defaults);
  - unknown keys and unknown rule ids are errors.
  - Fill in the `api::load_config_for` body.
- [x] A3. Reporters: `pretty` (snippet and carets), `compact`, `json` v1.
      Add snapshot tests per format.

**B: `b/yaml-engine`, `b/test-rules`**
- [x] B3. `custom::load_providers` and the YAML `RuleProvider`:
  - matches become `Diagnostic`s;
  - metavariables are substituted into `message` and `help`;
  - `fix` templates become `Fix`es with their safety level.
- [x] Fixture tests in `tests/custom_rules/`.
- [x] B4. `gdguard test-rules`:
  - runs each rule's `valid` and `invalid` cases and checks the expected
    `output`, using `apply_edits` for a single pass;
  - checks idempotency (fixed output has no issue for that rule);
  - prints readable failures.

- [x] **Merge session (C1):** calls `custom::load_providers` in
`linter.rs`, so custom rule levels come from `gdguard.toml`.
After C1, `gdguard lint` shows both built-in and custom issues.

## W3: A and B at the same time

**A: `a/fix-engine`, `a/builtin-fixes`**
- [x] A4. Fix engine:
  - multiple passes (at most 10) with overlap handling;
  - re-parse after each pass and roll back fixes that break parsing;
  - formatter step, only for files that were already formatted;
  - `--fix`, `--fix-unsafe`, `--diff`, `--stdin-filename`;
  - fill in the `api::fix_source` body.
- [x] Tests: overlapping fixes, rollback, idempotency.
- [x] A6. Safe fixes for `unnecessary-pass` and `no-else-return`, and any
      other easy ones.

**B: `b/rule-pack`, `b/explain`, `b/init`**
- [x] B5. `examples/rules/`, each rule with tests:
  - `prefer-is-empty`
  - `no-get-node-in-process`
  - `prefer-preload-const`
  - `no-godot3-yield`
  - `no-string-connect`
  - `no-print-left`
  - `prefer-is-instance-valid`
- [x] B6. `rules_meta.rs`: rationale and good/bad examples for the 17
      built-ins. Then `gdguard explain <rule>` and
      `--list-rules [--format json]`.
- [x] B7. `gdguard init`: writes a starter `gdguard.toml` and `.gdguard/rules/`.

- [x] **Merge session (C2):** add an end-to-end test where a custom rule's fix is applied by
`--fix`. Tag **v0.29**.

## W4: A and B at the same time

**A: `a/ignores`, `a/baseline`, `a/ci-reporters`, `a/parallel-cache`**
- [x] A7. File-level ignores, `-- reason`, and the `unused-ignore` warning.
- [x] A8. `--baseline` and `--update-baseline`, using line-text
      fingerprints. Test that moving lines keeps an issue suppressed.
- [x] A9. `github` and `sarif` reporters.
- [x] Parallel linting (`std::thread::scope`) and the lint cache in
      `.gdguard/cache/`.

**B: `b/agent-integration`, `b/docs`**
- [x] B8. `integrations/claude-code/`: a PostToolUse hook (lint the edited
      `.gd` file with `--fix --format json` and return what's left), a
      skill/AGENTS snippet, and setup steps.
- [x] Write `docs/custom_rules.md` and `docs/agents.md`, and take over the
      README.
- [x] Dogfood on the sample project and record repros. No real-game path
      was provided; real-game adoption remains a maintainer follow-up.

**Solo integration (C3):** completed reporter snapshots, baseline
round-trip, unused-ignore metadata, hook checks and sample dogfooding.
Real-game dogfooding is recommended before a v0.30 release; no tag created.

## SM0: second contracts session (together, about 1.5 hours, one PR)

- [x] `src/semantic/mod.rs`: types and method signatures from the spec,
      non-panicking empty-model bodies.
- [x] `src/project/mod.rs`: `ScriptSummary`, `ProjectIndex`, `MemberLookup`,
      with signatures only.
- [x] Add `semantic: Option<&SemanticModel>` and
      `project: Option<&ProjectIndex>` to `FileContext` (additive). The
      pipeline fills them in once W5 and W6 land.
- [x] Write the final JS API surface into `integrations/types/gdguard.d.ts`.
      This is the contract B implements and A's data must fit.
- [x] Add the `rquickjs` dependency behind the `js-rules` feature, and
      `lsp-server`/`lsp-types` for W7.

## W5: A and B at the same time

**A: `a/semantic-model`**
- [x] S1. `SemanticModel::build`: scopes for every `ScopeKind`, symbols,
      references (read, write, call), resolution, and `scope_at`.
- [x] `tests/semantic/`: fixtures with expected symbol and reference dumps.
- [x] Build the model once per file in the pipeline and pass it through
      `FileContext`.

**B: `b/js-runtime`**
- [x] J1. `src/linter/script/`:
  - load `.js` rules from the rules folders;
  - QuickJS sandbox with no imports, a memory limit and a time limit;
  - `meta`, then `create(context)`, then visitor callbacks for the subscribed
    node kinds;
  - syntax-only node API, `report`, `fix.*`, `options`.
- [x] Make `test-rules` run JS `tests`.
- [x] Tests in `tests/js_rules/`, including a rule that runs too long or uses
      too much memory and gets stopped.

**Merge session (C4):** adds `context.semantic` to the JS API on top of A's merged
model, and A reviews it.

## W6: A and B at the same time

**A: `a/semantic-rules`, `a/project-index`**
- [x] Built-in rules on the semantic model, each with fixes where they're
      safe:
  - `unused-variable`
  - `unused-private-function`
  - `shadowed-variable`
  - `unused-local-signal`
  - `assigned-but-never-read`
- [x] Add their metadata to `rules_meta.rs`. Coordinate with B, because B
      owns that file; send A's text in the PR description and B adds it.
- [x] P1. `ProjectIndex::build`: parse `project.godot` autoloads, summarize
      the scripts, follow `extends` chains, use the cache, and fill
      `FileContext.project`.

**B: `b/js-project-api`, `b/js-docs`**
- [x] J2. Add `context.project` to the JS API and finish the `.d.ts` types.
- [x] JS example rules in `examples/rules/`: `max-function-params`,
      `require-typed-signal-params`, `no-autoload-mutation-outside-owner`.
- [x] Write `docs/js_rules.md`.

**Merge session (C5):** add an end-to-end test where a JS rule uses semantic and
project data. Tag **v0.31**.

## W7: A and B at the same time

**A: `a/lsp`**
- [x] L1. `gdguard lsp`:
  - diagnostics on open, change and save, debounced;
  - code actions: quick fix, fix all safe, ignore line;
  - reload config and rules when they change;
  - UTF-16 positions.
- [x] `tests/lsp/`, using a scripted client session.
- [x] Write the LSP part of `docs/editors.md` (VS Code, Neovim, Zed).

**B: `b/godot-editor`**
- [x] G1. gdextension `lint(path, source)` and `fix(path, source, unsafe)`
      on top of `api.rs`, returning Dictionaries.
- [x] Addon: a bottom panel with the issue list and jump-to-line, plus
      "Apply fix" and "Fix all safe" buttons, linting on save.
- [x] Write the Godot part of `docs/editors.md`.

**Merge session (C6):** run both on the same project with the same config and check that
the diagnostics are identical. Tag **v0.32**.

## W8: A and B at the same time

**A: `a/cross-file-rules`, `a/perf`**
- [x] P2. `unused-signal` (project-wide) and
      `unknown-member-on-project-class`.
- [x] A lint benchmark in `benchmarks/`, profiling, and a warm-cache target
      of under 1 second for 1000 files.

**B: `b/release`, `b/upstream`**
- [x] R1. Rename the binary, addon and artifacts in `release.yml`, then do a
      release dry run.
- [x] Final docs pass and README.
- [x] ~~Upstream contributions to GDQuest.~~ Dropped: the maintainer decided
      (2026-11-09) that nothing is ever sent upstream.

**Final:** review the spec against code and validate CLI/agent workflows.
No release tag is created automatically. Selecting v1.0 and publishing it
remain explicit maintainer actions; see docs/releasing.md.

## W9: solo, stronger built-in rules (2026-11-09)

Requested by the maintainer after testing W8 against Godot 4.7.2.

- [x] `duplicate-class-name`; duplicate names no longer fail the index.
- [x] `unused-private-function` counts scene connections;
      `scene-connection-missing-method`.
- [x] `missing-resource-path`, `prefer-uid-path` (maintainer: use `uid://`
      for `load`/`preload`), `missing-class-name` (maintainer: every
      script has a `class_name` unless it is an autoload).
- [x] `self-assignment`, `unreachable-code`, `duplicate-dictionary-key`,
      `godot3-syntax`, and the fix-engine exception for fixes that repair
      every syntax error.
- [x] Optional Godot check (`--godot`, `GDGUARD_GODOT`, `godot-error`).

See docs/handoff/w9.md.

## W10: solo, default declaration order and shorter rules docs (2026-11-09)

- [x] Default warning `declaration-order`, using the formatter's reorder plan.
- [x] Safe/unsafe reorder fixes, region/comment/annotation preservation,
      disabled-formatting guard and fix/re-lint regression coverage.
- [x] Rule metadata/reference/spec, shorter README Rules section, snapshots,
      CLI fix test, formatter regression checks and handoff.

See docs/handoff/w10.md. No push, tag or release.

---

## Dependency map (why the waves are in this order)

```
W0 contracts ─┬─ A1 diagnostics ─┬─ A2 config ── C1 ── A4 fix engine ── C2
              │                  └─ A3 reporters
              └─ B1 spike ── B2 loader ── B3 engine ── C1
                                         └─ B4 test-rules (uses apply_edits from W0)
SM0 contracts ─┬─ S1 semantic ── C4 ── A semantic rules
               │                 └─ P1 project index ── C5 ── P2 cross-file rules
               └─ J1 JS runtime ── C4 ── J2 project API ── C5
api.rs (W0, filled W1–W3) ─┬─ L1 LSP (W7)
                           └─ G1 Godot editor (W7)
```

Inside a wave, neither person waits on the other. Each task only depends on
things merged in **earlier** waves or on the frozen contracts.

## Working rules

- **The flow for each wave:**
  1. Each person pastes the START line once. Each session does its task for
     the wave on its own branch and pushes it.
  2. A's session waits for B's branch, then merges, tests, pushes `main` and
     moves `docs/prompts/PROGRESS.md` to the next wave (`MERGE.md`).
  3. Both sessions continue with the next wave, until it's finished.
  See `docs/prompts/README.md`.
- **Shared session rules:** every session follows
  `docs/prompts/_session_rules.md` (scope, commit after each task, tests,
  handoff).
- **A track session is done when:**
  - clippy passes with `-D warnings` and `cargo test --workspace` passes;
  - new behavior has tests;
  - it follows `docs/rust_coding_guidelines.md`;
  - its handoff note is written;
  - its branch is pushed.
- **Review:** after each merge, each person reads the merge report and skims
  the other track's diff. That keeps you both up to date with the whole
  codebase, which also protects you if one of you becomes unavailable.
- **Contracts** (`diagnostic.rs`, `api.rs` signatures, `semantic/` and
  `project/` signatures, `.d.ts`, the JSON and config schemas) change only in
  W0/SM0, or in an additive change the merge session makes on request.
