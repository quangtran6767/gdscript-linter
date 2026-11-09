# Handoff SM0 (solo on w4-a)

## Summary

Frozen the semantic model, project index, optional FileContext references,
and complete JS type contract. Empty models are non-panicking. W5 and W6
implement the bodies; SM0 does not claim semantic analysis or JS execution.

## Tasks

| Task | Status | Notes |
| --- | --- | --- |
| Semantic types and signatures | done | Root scope, nullable symbol lookup, stable debug dump |
| Project types and signatures | done | Empty index, Unknown member lookup |
| FileContext additions | done | All 8 construction sites pass None for both |
| JS declaration file | done | JSDoc for public members; strict TypeScript check |
| Dependencies/features | done | Exact pins; JS enabled by default, optional |
| Owner modules and wiring | done | Semantic/project builders, LSP, script runtime/bindings |
| Spec | done | Exact frozen definitions and pending implementation status |

## Files changed

- `src/semantic/{mod,builder}.rs`: semantic contracts and snapshot format.
- `src/project/{mod,index}.rs`: index and tri-state member contracts.
- `src/lsp/mod.rs`, `src/linter/script/`: next-wave module boundaries.
- `src/lib.rs`, `src/linter.rs`: module wiring.
- `src/linter/diagnostic.rs`: optional model references.
- `src/linter/rule_tester.rs`, `tests/custom_rules/provider_tests.rs`:
  initialize new optional references.
- `integrations/types/gdguard.d.ts`: complete JS API with JSDoc.
- `Cargo.toml`, `Cargo.lock`: pinned dependencies and feature.
- `tests/contracts/main.rs`: empty-file/unknown-ID and empty-project checks.
- Spec, implementation plan, changelog, progress and this handoff.

## Public API added

All of these are frozen for W5/W6. Private storage may change.

```rust
// src/semantic/mod.rs
pub struct ScopeId(pub u32);
pub struct SymbolId(pub u32);
pub enum ScopeKind { Class, InnerClass, Function, Lambda, Block, ForLoop, MatchBranch }
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub span: Span,
    pub symbols: Vec<SymbolId>,
}
pub enum SymbolKind {
    Variable, Constant, Parameter, LoopVariable, Function, Signal,
    Enum, EnumMember, InnerClass,
}
pub struct Symbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub scope: ScopeId,
    pub name_span: Span,
    pub declaration_span: Span,
    pub declared_type: Option<String>,
    pub annotations: Vec<String>,
    pub is_static: bool,
}
pub enum ReferenceKind { Read, Write, Call }
pub struct Reference {
    pub span: Span,
    pub name: String,
    pub kind: ReferenceKind,
    pub symbol: Option<SymbolId>,
}
impl SemanticModel {
    pub fn build(tree: &tree_sitter::Tree, source: &str) -> SemanticModel;
    pub fn scope_at(&self, byte: usize) -> ScopeId;
    pub fn lookup(&self, scope: ScopeId, name: &str) -> Option<SymbolId>;
    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol>;
    pub fn symbols(&self) -> &[Symbol];
    pub fn scopes(&self) -> &[Scope];
    pub fn references_to(&self, id: SymbolId) -> &[Reference];
    pub fn unresolved_references(&self) -> &[Reference];
    pub fn debug_dump(&self) -> String;
}

// src/project/mod.rs
pub struct ScriptSummary {
    pub path: PathBuf,
    pub class_name: Option<String>,
    pub extends: Option<String>,
    pub functions: Vec<String>,
    pub signals: Vec<String>,
    pub variables: Vec<String>,
}
pub enum MemberLookup { Yes, No, Unknown }
pub struct ProjectIndex {
    pub root: PathBuf,
    pub scripts: Vec<ScriptSummary>,
    pub class_names: HashMap<String, usize>,
    pub autoloads: HashMap<String, PathBuf>,
}
impl ProjectIndex {
    pub fn build(root: &Path) -> Result<ProjectIndex, String>;
    pub fn has_member(&self, class_name: &str, member: &str) -> MemberLookup;
}

// Added to FileContext<'a>:
pub semantic: Option<&'a crate::semantic::SemanticModel>;
pub project: Option<&'a crate::project::ProjectIndex>;
```

JS signatures and field types are exact in
[`integrations/types/gdguard.d.ts`](../../integrations/types/gdguard.d.ts),
declaring module `gdguard`:

- `Position`, `Range`, `Node`, `Edit`, `Fixer`, `FixSafety`, `Severity`,
  `Fix`, `ReportDescriptor`, `SymbolKind`, `Symbol`, `Reference`, `Semantic`,
  `Project`, `RuleContext<Options>`, `RuleMeta`, `RuleTests`, `Visitors`,
  `Rule<Options>`.
- `Node.field(name: string): Node | null`.
- `Fixer.replace(target: Node | Range, text: string): Edit`.
- `Fixer.insertBefore(target: Node | Range, text: string): Edit`.
- `Fixer.insertAfter(target: Node | Range, text: string): Edit`.
- `Fixer.remove(target: Node | Range): Edit`.
- `Semantic.scopeAt(byte: number): number`.
- `Semantic.lookup(scope: number, name: string): number | null`.
- `Semantic.referencesTo(symbol: number): readonly Reference[]`.
- `Project.hasMember(className: string, member: string): "yes" | "no" | "unknown"`.
- `RuleContext.report(descriptor: ReportDescriptor): void`.
- `Rule.create(context: RuleContext<Options>): Visitors`.

## Decisions

- `symbol()` returns `Option<&Symbol>` rather than the draft spec's `&Symbol`:
  the draft could not fulfill SM0's no-panic/empty-model requirement for
  unknown IDs. This finalizes a new contract, not an existing implementation.
- Add `Scope`, `scopes()` and `symbols()` now so semantic rules and JS can
  enumerate declarations without requesting a breaking change later.
- Scope 0 is a valid file root, even for an empty/error tree. Out-of-range
  scope_at queries return it; unknown symbol queries return None/empty.
- Project-relative PathBuf values omit the literal `res://` prefix.
- JS fixes accept an edit, edit array or callback; default safety is
  suggestion, overridable by rule metadata or report descriptor. Explicit
  range and node locations are mutually exclusive.
- All runtime-facing byte offsets use UTF-8; JavaScript string indices
  remain UTF-16. JS model properties are camelCase; kind values snake_case.
- Pinned `rquickjs = 0.9.0` optional (no module-loader/parallel features),
  `lsp-server = 0.7.9`, `lsp-types = 0.97.0` are compatible with the root
  package's declared Rust 1.89. Newer versions were not needed for the
  roadmap contracts. No claim that the whole workspace supports 1.89:
  gdextension already declares Rust 1.94.

## Requests / next implementation work

- W5-A implements `src/semantic/builder.rs` and wires one model per file.
- W5-B implements script runtime/bindings, discovery/provider/test-rules and
  example JS rules. Empty script modules intentionally have no runtime API.
- W6 implements project discovery/index and fills FileContext.project.
- W7 implements the LSP module; the CLI still reports it unimplemented.
- Keep signatures compatible; the solo session owns both historical tracks.

## Spec updates and changelog

Applied the exact contracts and units above, optional feature/dependency
details, explicit SM0 stub status, and internal changelog entry.

## How to test manually

```sh
cargo test --test contracts
cargo build --no-default-features
cargo test -p gdscript-formatter --no-default-features
tsc --strict --noEmit integrations/types/gdguard.d.ts
```

## Known issues / follow-ups

- Semantic and project models are deliberately empty until W5/W6.
- JS rules are not executed yet. Feature presence is not runtime support.
- No real-game dogfooding or release tags created in this wave.
