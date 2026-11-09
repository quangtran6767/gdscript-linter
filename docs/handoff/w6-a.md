# W6-A: semantic rules and project index (solo)

## Completed

- Added the five planned Warning rules: unused-variable,
  unused-private-function, shadowed-variable, unused-local-signal and
  assigned-but-never-read. Each has metadata and true/negative fixtures.
- Unsafe unused-local removal is offered only without initializer calls;
  an otherwise-empty block becomes pass. Private-helper removal is a
  Suggestion. Underscore locals, Godot virtual callbacks, string callbacks
  and signal string emit/connect forms receive conservative exemptions.
- Added an additive default Rule.check_context method; RuleProvider and
  frozen library signatures are unchanged. Semantic models build once
  when enabled semantic rules or custom providers need them.
- Implemented discovery, autoloads, stable content-hash summaries/cache,
  class/path inheritance and tri-state member queries in src/project/.
- Prepared one Arc<ProjectIndex> per project root for all CLI workers;
  in-memory API/stdin calls prepare their index once per linter run.
- Project file/script-content digests invalidate dependent lint caches.
  Lint cache version is now 3. Broken/unwritable caches remain nonfatal.

## Public API additions

```rust
pub fn project::find_root(path: &Path) -> Option<PathBuf>;
pub fn project::fingerprint(index: &ProjectIndex) -> Result<String, String>;
impl GDScriptLinter {
    pub fn set_project(&mut self, project: Option<Arc<ProjectIndex>>);
}
trait Rule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>);
}
```

ProjectIndex/ScriptSummary and semantic API signatures remain frozen.
ScriptSummary adds serialization derives for its private on-disk cache.

## Decisions / correctness

- Malformed scripts are cached but excluded from resolvable summaries.
  Missing bases, cycles and engine bases remain Unknown.
- Top-level constants/enums/inner classes are member names too.
- A terminal project script returns No for an absent project member;
  implicit Object/RefCounted member names and dynamic _get/_set hooks stay
  Unknown. The engine/virtual exemption data comes from installed
  gdextension-api 0.5.1 (Godot 4.2–4.7), not guessed callback names.
- Local signals intentionally warn about absent *local* use; external or
  scene-only use needs a reasoned exception. The project-wide rule is W8.
- Project summaries reflect disk content. Live unsaved document semantics
  still use the in-memory buffer; project data updates on later runs.

## Dogfood

The sample now has 20 issues: 8 errors, 10 warnings, 2 infos. The one new
issue is its intentionally unused OnDefeated signal. No new false positive
was identified. No real-game path was supplied. Fixtures were not fixed;
safe/unsafe fix smoke runs use temporary copies.

## Validation

Workspace and feature-off tests, clippy, release build, formatter regression,
13 example rules, and project/cache/semantic tests pass; see w6-merge.md.
Manual: cargo test --test project --test semantic; cargo run -- lint
--no-cache --format json tests/e2e/sample_project.

Metadata/spec/changelog requests were applied directly in solo integration.
