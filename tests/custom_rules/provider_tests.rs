//! W2/B3: the YAML `RuleProvider`. Runs the rules in `provider/rules/` on
//! `provider/sample.gd` and checks spans, messages, fixes and safety.

use std::path::{Path, PathBuf};

use gdscript_formatter::linter::api::Config;
use gdscript_formatter::linter::custom::loader::load_rules;
use gdscript_formatter::linter::custom::{YamlRuleProvider, load_providers};
use gdscript_formatter::linter::diagnostic::{
    Diagnostic, FileContext, FixSafety, Position, Severity, apply_edits,
};

fn provider_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("custom_rules")
        .join("provider")
}

fn parse(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_gdscript::LANGUAGE.into())
        .unwrap();
    parser.parse(source, None).unwrap()
}

fn sample_source() -> String {
    std::fs::read_to_string(provider_dir().join("sample.gd"))
        .unwrap()
        .replace("\r\n", "\n")
}

fn check(source: &str) -> Vec<Diagnostic> {
    let config = Config {
        custom_rule_paths: vec![provider_dir().join("rules")],
        ..Config::default()
    };
    let mut providers = load_providers(&config).unwrap();
    assert_eq!(providers.len(), 1);
    let tree = parse(source);
    let context = FileContext {
        path: Path::new("sample.gd"),
        source,
        tree: &tree,
        semantic: None,
        project: None,
    };
    providers[0].check_file(&context)
}

fn find<'a>(diagnostics: &'a [Diagnostic], rule: &str) -> Vec<&'a Diagnostic> {
    let mut found = Vec::new();
    for diagnostic in diagnostics {
        if diagnostic.rule == rule {
            found.push(diagnostic);
        }
    }
    found
}

#[test]
fn no_rule_paths_gives_no_providers() {
    assert!(load_providers(&Config::default()).unwrap().is_empty());
}

#[test]
fn load_error_is_returned_as_a_string() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("custom_rules")
        .join("loader")
        .join("duplicate");
    let config = Config {
        custom_rule_paths: vec![folder.clone()],
        ..Config::default()
    };
    let error = match load_providers(&config) {
        Ok(_) => panic!("expected a load error"),
        Err(error) => error,
    };
    assert_eq!(
        error,
        format!(
            "{}: `id`: duplicate rule id `same-id`, already defined in {}",
            folder.join("b.yml").display(),
            folder.join("a.yml").display()
        )
    );
}

#[test]
fn provider_lists_rule_meta_in_load_order() {
    let config = Config {
        custom_rule_paths: vec![provider_dir().join("rules")],
        ..Config::default()
    };
    let providers = load_providers(&config).unwrap();
    let mut ids = Vec::new();
    for meta in providers[0].rules() {
        ids.push(meta.id.as_str());
    }
    assert_eq!(
        ids,
        vec![
            "no-get-node-in-process",
            "prefer-is-empty",
            "print-args",
            "suggest-node-path"
        ]
    );
}

#[test]
fn span_message_help_and_severity() {
    let source = sample_source();
    let diagnostics = check(&source);
    let found = find(&diagnostics, "prefer-is-empty");
    assert_eq!(found.len(), 1);
    let diagnostic = found[0];
    assert_eq!(diagnostic.severity, Severity::Warning);
    assert_eq!(
        diagnostic.message,
        "Use `items.is_empty()` instead of `len(items) == 0`"
    );
    assert_eq!(
        diagnostic.help.as_deref(),
        Some("`items.is_empty()` states the intent.")
    );
    let start = source.find("len(items) == 0").unwrap();
    assert_eq!(diagnostic.span.start_byte, start);
    assert_eq!(diagnostic.span.end_byte, start + "len(items) == 0".len());
    assert_eq!(diagnostic.span.start, Position { line: 3, column: 5 });
    assert_eq!(
        diagnostic.span.end,
        Position {
            line: 3,
            column: 20
        }
    );
}

#[test]
fn safe_fix_edits_the_matched_range() {
    let source = sample_source();
    let diagnostics = check(&source);
    let diagnostic = find(&diagnostics, "prefer-is-empty")[0];
    assert_eq!(diagnostic.fixes.len(), 1);
    let fix = &diagnostic.fixes[0];
    assert_eq!(fix.safety, FixSafety::Safe);
    assert_eq!(fix.description, "Replace with `items.is_empty()`");
    assert_eq!(fix.edits.len(), 1);
    assert_eq!(fix.edits[0].span, diagnostic.span);
    assert_eq!(fix.edits[0].replacement, "items.is_empty()");
    let fixed = apply_edits(&source, &fix.edits).unwrap();
    assert!(fixed.contains("\tif items.is_empty():\n"), "{}", fixed);
}

#[test]
fn multi_metavariable_keeps_separators_in_message_and_fix() {
    let source = sample_source();
    let diagnostics = check(&source);
    let found = find(&diagnostics, "print-args");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Info);
    assert_eq!(found[0].message, "print called with \"empty\", 1");
    assert_eq!(found[0].fixes[0].safety, FixSafety::Unsafe);
    assert_eq!(
        found[0].fixes[0].edits[0].replacement,
        "push_warning(\"empty\", 1)"
    );
}

#[test]
fn rule_without_fix_has_no_fixes_and_inside_limits_matches() {
    let source = sample_source();
    let diagnostics = check(&source);
    let found = find(&diagnostics, "no-get-node-in-process");
    // `get_node("Player")` in `_ready` doesn't match; only `_process` does.
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Error);
    assert!(found[0].fixes.is_empty());
    assert_eq!(found[0].span.start, Position { line: 8, column: 2 });
    assert_eq!(
        &source[found[0].span.start_byte..found[0].span.end_byte],
        "get_node(\"Enemy\")"
    );
}

#[test]
fn fix_without_safety_is_a_suggestion_and_keeps_literal_node_paths() {
    let source = sample_source();
    let diagnostics = check(&source);
    let found = find(&diagnostics, "suggest-node-path");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Severity::Warning);
    let fix = &found[0].fixes[0];
    assert_eq!(fix.safety, FixSafety::Suggestion);
    assert_eq!(fix.edits[0].replacement, "$Player");
    let fixed = apply_edits(&source, &fix.edits).unwrap();
    assert!(fixed.contains("\tvar p = $Player\n"), "{}", fixed);
}

#[test]
fn every_rule_runs_on_one_tree() {
    let source = sample_source();
    let diagnostics = check(&source);
    assert_eq!(diagnostics.len(), 4);
}

#[test]
fn file_without_matches_gives_no_diagnostics() {
    assert!(check("func f():\n\tpass\n").is_empty());
}

#[test]
fn check_rule_runs_one_rule_only() {
    let rules = load_rules(&[provider_dir().join("rules")]).unwrap();
    let provider = YamlRuleProvider::new(rules);
    let source = sample_source();
    let tree = parse(&source);
    let context = FileContext {
        path: Path::new("sample.gd"),
        source: &source,
        tree: &tree,
        semantic: None,
        project: None,
    };
    let diagnostics = provider.check_rule("print-args", &context);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "print-args");
}

#[test]
fn multi_line_fix_is_reindented_and_described_by_rule_id() {
    let folder =
        std::env::temp_dir().join(format!("gdguard-provider-multiline-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("wrap.yml"),
        "id: wrap-print\nmessage: m\nrule:\n  pattern:\n    context: print($A)\n    selector: expression_statement\nfix: |-\n  if OS.is_debug_build():\n  \tprint($A)\n",
    )
    .unwrap();
    let config = Config {
        custom_rule_paths: vec![folder.clone()],
        ..Config::default()
    };
    let mut providers = load_providers(&config).unwrap();
    let source = "func f():\n\tprint(1)\n";
    let tree = parse(source);
    let context = FileContext {
        path: Path::new("a.gd"),
        source,
        tree: &tree,
        semantic: None,
        project: None,
    };
    let diagnostics = providers[0].check_file(&context);
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(diagnostics.len(), 1);
    let fix = &diagnostics[0].fixes[0];
    assert_eq!(fix.description, "Apply the `wrap-print` fix");
    let fixed = apply_edits(source, &fix.edits).unwrap();
    assert_eq!(
        fixed,
        "func f():\n\tif OS.is_debug_build():\n\t\tprint(1)\n"
    );
}

#[test]
fn multi_line_fix_keeps_space_indentation() {
    let folder =
        std::env::temp_dir().join(format!("gdguard-provider-spaces-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("wrap.yml"),
        "id: wrap-print\nmessage: m\nrule:\n  pattern:\n    context: print($A)\n    selector: expression_statement\nfix: |-\n  if OS.is_debug_build():\n      print($A)\n",
    )
    .unwrap();
    let config = Config {
        custom_rule_paths: vec![folder.clone()],
        ..Config::default()
    };
    let mut providers = load_providers(&config).unwrap();
    let source = "func f():\n    if true:\n        print(1)\n";
    let tree = parse(source);
    let context = FileContext {
        path: Path::new("a.gd"),
        source,
        tree: &tree,
        semantic: None,
        project: None,
    };
    let diagnostics = providers[0].check_file(&context);
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(diagnostics.len(), 1);
    let fixed = apply_edits(source, &diagnostics[0].fixes[0].edits).unwrap();
    assert_eq!(
        fixed,
        "func f():\n    if true:\n        if OS.is_debug_build():\n            print(1)\n"
    );
}

#[test]
fn message_uses_metavariable_captured_in_a_relational_rule() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("rules");
    let config = Config {
        custom_rule_paths: vec![examples],
        ..Config::default()
    };
    // This exercises YAML capture rendering, independently of optional JS.
    let yaml = gdscript_formatter::linter::custom::YamlRuleProvider::new(
        gdscript_formatter::linter::custom::loader::load_rules(&config.custom_rule_paths).unwrap(),
    );
    let mut providers: Vec<Box<dyn gdscript_formatter::linter::diagnostic::RuleProvider>> =
        vec![Box::new(yaml)];
    let source = "func _physics_process(delta):\n\tget_node(\"A\").show()\n";
    let tree = parse(source);
    let context = FileContext {
        path: Path::new("a.gd"),
        source,
        tree: &tree,
        semantic: None,
        project: None,
    };
    let diagnostics = providers[0].check_file(&context);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "Don't look up nodes in `_physics_process()`; cache the node in an `@onready` variable"
    );
}
