# SM0: semantic, project and JS contracts (single session, branch `sm0`)

Read `docs/prompts/_session_rules.md` first and follow it, **except** that
this single session may also edit `docs/specification_linter.md`,
`CHANGELOG.md`, `Cargo.toml` and `src/linter/diagnostic.rs` (additive only).

No other session runs during SM0. You freeze the contracts that let W5 and W6
run in parallel.

## Read

- Spec: "Semantic model", "Project index", "Scripted rules in JS", "LSP
  server".
- All files in `docs/handoff/`.

## Tasks

1. **`src/semantic/mod.rs`:**
   - every type from the spec (`ScopeId`, `SymbolId`, `ScopeKind`,
     `SymbolKind`, `Symbol`, `ReferenceKind`, `Reference`, `SemanticModel`);
   - every method signature, with a body that returns an empty model or
     `None`, never a panic;
   - `pub fn debug_dump(&self) -> String`, a stable text format for snapshot
     tests. Specify the format in a doc comment.
2. **`src/project/mod.rs`:** `ScriptSummary`, `ProjectIndex`, `MemberLookup`
   (`Yes`/`No`/`Unknown`), and the `build`/`has_member` signatures with stub
   bodies.
3. **`FileContext` (additive):** add `semantic: Option<&SemanticModel>` and
   `project: Option<&ProjectIndex>`. Update every place that constructs it,
   passing `None` for both.
4. **`integrations/types/gdguard.d.ts`:** the complete JS rule API from the
   spec, with JSDoc on every member. This is the contract for W5 and W6:
   - `Rule`, `RuleMeta`, `RuleContext`;
   - `Node`, with `type`, `text`, `range`, `parent`, `children`,
     `namedChildren` and `field()`;
   - `Fixer`, `Semantic`, `Project`, `Range`, `ReportDescriptor`.
5. **Dependencies:**
   - `rquickjs` behind a new cargo feature `js-rules` (on by default for the
     binary);
   - `lsp-server` and `lsp-types` for W7;
   - all pinned.
6. **Empty modules** with owner doc comments:
   - A: `src/semantic/builder.rs`, `src/project/index.rs`, `src/lsp/mod.rs`
   - B: `src/linter/script/mod.rs`, `runtime.rs`, `bindings.rs`
   - Wire every `mod` line now.
7. **Spec:** update the four sections above to match the exact signatures
   you wrote.

## Acceptance

- Everything compiles with and without `--features js-rules`.
- clippy and the tests are green.
- `docs/handoff/sm0.md` lists every new signature exactly.
