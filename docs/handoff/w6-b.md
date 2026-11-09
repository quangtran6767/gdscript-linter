# W6-B: JS project API, examples and docs (solo)

## Completed

- context.project exposes sorted classNames, relative autoload paths and
  hasMember (yes/no/unknown), or null outside a Godot project.
- Added optional Project.root as an additive type/API member. JS filename
  is absolute; this permits exact autoload-owner comparisons, including a
  regression for another directory with the same owner basename.
- Added require-typed-signal-params, prefer-static-typing (allow_inferred
  option), and no-autoload-mutation-outside-owner examples. All 13 YAML/JS
  example rules pass; real project tests cover the autoload positive case.
- Added docs/js_rules.md with the complete API, node-kind/grammar guide,
  sandbox limits, fix testing and three walkthroughs. Its complete JS rule
  is extracted and run by a test. Added JSDoc examples to the declaration.
- Strict TypeScript checks pass for the declaration and all JS examples.
- Added project/null API tests, owner exemptions, cross-file cache
  invalidation and C5 e2e coverage.

## API

The SM0 API is unchanged except the additive optional field:

```typescript
interface Project {
  readonly root?: string; // absolute filesystem project directory
}
```

The root field fixes an ambiguity in comparing an absolute filename with a
project-relative owner path. A suffix comparison alone incorrectly exempted
unrelated nested scripts with the same filename.

## Manual checks

- cargo run -- test-rules examples/rules
- cargo test --test js_rules --test e2e_tests
- tsc --strict --noEmit --allowJs --checkJs --target ES2021
  integrations/types/gdguard.d.ts examples/rules/*.js

## Follow-ups

No incomplete W6 task. W7 adds LSP/editor integration. No release tagged.
