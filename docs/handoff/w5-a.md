# W5-A: semantic model (solo)

Implemented the frozen semantic API in `src/semantic/`, with one syntax
walk collecting scopes/declarations/uses, followed by lexical resolution.
The pipeline builds one model when custom providers are present. W6 will
also request it for enabled semantic built-ins.

## Tasks and behavior

- All scope kinds, function/default/typed parameters, locals/constants,
  class members, signals, enums, loop variables and match bindings.
- Class members resolve before their declaration; locals become visible
  after their initializer. Lambdas capture; inner classes do not capture
  outer instance members. `self.x` selects the class member despite local
  shadowing. Named enum members resolve through their qualifier.
- Read/write/call references; compound assignments record both read/write.
  Attribute names on unknown receivers remain unresolved.
- Constructor and property accessor scopes/parameters are included.
- Scope lookup is binary search over sorted scope-boundary intervals.
- 18 snapshot fixtures plus directed assertions cover the required cases,
  declaration visibility, enum qualifiers, accessors, and invalid syntax.

## Public API

No frozen signatures changed. Added only private storage/helpers. See
`sm0.md` for exact public signatures. `FileContext.semantic` is now Some
for custom-provider checks.

## Timing

`cargo run --release --example semantic_benchmark` measures model building
on already-parsed trees against parse + built-in lint time, 100 rounds:

| Corpus | Files | Lint | Model | Overhead |
| --- | --- | --- | --- | --- |
| tests/lint/input | 19 | 1.498 ms | 0.180 ms | 12.0% |
| tests/input | 60 | 6.389 ms | 0.775 ms | 12.1% |

Both are below the 30% target. Numbers vary with machine load. No model is
built for syntax-only lint without custom providers in W5.

## Validation / manual checks

`cargo test --test semantic --test contracts`; full workspace tests,
clippy, formatter regression, release and feature-off builds pass. The
integration report records totals. Inspect `tests/semantic/expected/` for
the exact stable debug format and lexical resolutions.

## Decisions and remaining work

- Unresolved engine/inherited/unknown receiver members remain data, never
  errors. This is lexical analysis, without type inference.
- Signal parameters are signature data, not locals visible to methods.
- No RuleProvider changes were required.
- Project context and semantic built-in consumers are W6 work.
