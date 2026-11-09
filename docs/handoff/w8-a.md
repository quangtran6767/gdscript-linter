# W8-A: cross-file rules and performance (solo)

## Summary

Added the two project-wide CLI rules, an actionable `parse-error`
diagnostic, and a single-command 1000-file benchmark. All targets are met
with a wide margin, so no hotspot work was needed beyond sharing the
project analysis once per run.

## Tasks

- [x] `unused-signal`: counts `emit_signal("x")`, `x.emit()`,
  `x.connect(...)`, `connect("x", ...)`, `disconnect`/`is_connected`, and
  `.tscn` `[connection]` entries across the project. Dynamic emission
  (non-literal names) suppresses reports. Current-buffer uses override stale
  disk uses. In a project it supersedes `unused-local-signal`; outside a
  project only the local rule runs.
- [x] `unknown-member-on-project-class`: reports only when `has_member`
  returns `No` for an identifier receiver with an explicit project
  `class_name` annotation. `Unknown` (engine bases, inferred types) is never
  reported.
- [x] `parse-error` (Error, no fix): first ERROR/MISSING node, with a range
  and repair guidance, so malformed files never look clean. Empty enums are
  tolerated because Godot 4.7 accepts them.
- [x] Benchmark: `python3 benchmarks/lint_project.py` (options `--binary`,
  `--files`, `--runs`, `--keep`).

## Benchmark (2026-11-09)

Machine: Intel i5-11400H (12 threads), Linux, release build.

| Case | Result |
| --- | --- |
| 1000 files, cold median (5 runs) | 76.6 ms (target < 3 s) |
| 1000 files, warm median (5 runs) | 48.4 ms (target < 1 s) |
| cold and warm output identical | yes |
| 5000 project-class attribute accesses, single file | 85.9 ms |

## Changed files

- `src/project/analysis.rs` (new), `src/project/mod.rs`,
  `src/project/index.rs`
- `src/linter/rules/project_rules.rs` (new), `src/linter/rules.rs`,
  `src/linter.rs`, `src/linter/cache.rs`
- `benchmarks/lint_project.py` (new), `src/bin/benchmark.rs`
- `tests/project/main.rs`, `tests/project/fixtures/cross/`,
  `tests/lint/spans/`

## Added public APIs (additive)

- `ProjectIndex::build_uncached(root: &Path) -> Result<ProjectIndex, String>`
- `ProjectAnalysis { used_signals: HashSet<String>, dynamic_signal_usage: bool }`
- `ProjectAnalysis::build(&ProjectIndex) -> Result<Self, String>`
- `ProjectAnalysis::from_tree(&tree_sitter::Tree, &str) -> Self`

## Decisions

- Signal matching is by name. Same-named signals on unrelated classes can
  hide each other; this errs toward no false positives.
- `--no-cache`, stdin and fix/diff runs skip project-summary cache reads and
  writes. Text scenes are part of cache invalidation.

## Tests

`tests/project/main.rs`: cross-file fixture (scene-only connection, other
script emit, engine-base `Unknown` not reported), dynamic emission, live
buffer override. `tests/cli_tests.rs`: parse-error JSON.

## Known issues

Binary `.scn` scenes and runtime or external signal consumers can't be seen.
