#![allow(clippy::unwrap_used)]
use gdguard::semantic::{ReferenceKind, ScopeKind, SemanticModel, SymbolKind};
use std::path::Path;

fn model(source: &str) -> SemanticModel {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_gdscript::LANGUAGE.into())
        .unwrap();
    SemanticModel::build(&parser.parse(source, None).unwrap(), source)
}

#[test]
fn semantic_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/semantic");
    let mut count = 0;
    for entry in std::fs::read_dir(root.join("input")).unwrap() {
        let path = entry.unwrap().path();
        let source = std::fs::read_to_string(&path).unwrap();
        let semantic = model(&source);
        let snapshot = root
            .join("expected")
            .join(path.file_name().unwrap())
            .with_extension("txt");
        if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
            std::fs::write(&snapshot, semantic.debug_dump()).unwrap();
        }
        assert_eq!(
            semantic.debug_dump(),
            std::fs::read_to_string(&snapshot).unwrap(),
            "{}",
            path.display()
        );
        count += 1;
    }
    assert!(count >= 15);
}

#[test]
fn references_resolve_to_the_correct_shadow_and_self_member() {
    let source = "var value = 1\nfunc f(value):\n\tself.value = value\n\treturn self.value\n";
    let semantic = model(source);
    let member = &semantic.symbols()[0];
    let parameter = &semantic.symbols()[2];
    assert_eq!(member.kind, SymbolKind::Variable);
    assert_eq!(parameter.kind, SymbolKind::Parameter);
    assert_eq!(semantic.references_to(member.id).len(), 2);
    assert_eq!(semantic.references_to(parameter.id).len(), 1);
    assert_eq!(
        semantic.references_to(member.id)[0].kind,
        ReferenceKind::Write
    );
    assert_eq!(
        semantic.lookup(semantic.scope_at(source.find("return").unwrap()), "value"),
        Some(parameter.id)
    );
}

#[test]
fn inner_classes_cannot_capture_outer_instance_members() {
    let semantic = model(include_str!("input/inner_class.gd"));
    let outer = &semantic.symbols()[0];
    assert!(semantic.references_to(outer.id).is_empty());
    assert!(
        semantic
            .unresolved_references()
            .iter()
            .any(|reference| reference.name == "outer_member")
    );
    assert_eq!(semantic.references_to(semantic.symbols()[1].id).len(), 1);
}

#[test]
fn lambdas_capture_and_loop_bindings_do_not_escape() {
    let semantic = model(include_str!("input/lambda.gd"));
    let captured = semantic
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "captured")
        .unwrap();
    assert_eq!(semantic.references_to(captured.id).len(), 1);
    assert!(
        semantic
            .scopes()
            .iter()
            .any(|scope| scope.kind == ScopeKind::Lambda)
    );
    let semantic = model(include_str!("input/for.gd"));
    let item = semantic
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "item")
        .unwrap();
    assert_eq!(semantic.references_to(item.id).len(), 1);
    assert!(
        semantic
            .unresolved_references()
            .iter()
            .any(|reference| reference.name == "item")
    );
}

#[test]
fn locals_do_not_capture_their_own_initializer_or_earlier_uses() {
    let semantic = model(include_str!("input/initializer.gd"));
    let member = &semantic.symbols()[0];
    let local = semantic
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "value" && symbol.scope.0 != 0)
        .unwrap();
    assert_eq!(semantic.references_to(member.id).len(), 2);
    assert_eq!(semantic.references_to(local.id).len(), 1);
}

#[test]
fn compound_assignment_is_both_read_and_write() {
    let semantic = model(include_str!("input/compound.gd"));
    let count = semantic
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "count")
        .unwrap();
    let references = semantic.references_to(count.id);
    assert_eq!(references.len(), 3);
    assert!(
        references
            .iter()
            .any(|reference| reference.kind == ReferenceKind::Write)
    );
}

#[test]
fn enum_members_resolve_only_through_the_named_enum() {
    let semantic = model(include_str!("input/enums.gd"));
    let idle = semantic
        .symbols()
        .iter()
        .find(|symbol| symbol.name == "IDLE")
        .unwrap();
    assert_eq!(semantic.references_to(idle.id).len(), 1);
    assert_eq!(semantic.lookup(gdguard::semantic::ScopeId(0), "IDLE"), None);
}

#[test]
fn unused_local_fixes_are_unsafe_and_do_not_remove_calls() {
    use gdguard::linter::api::{Config, fix_source, lint_source};
    let path = Path::new("test.gd");
    let config = Config::default();
    let source = "func f():\n\tvar unused = 1\n";
    assert_eq!(
        fix_source(path, source, &config, false).unwrap().output,
        source
    );
    let fixed = fix_source(path, source, &config, true).unwrap();
    assert_eq!(fixed.applied, 1);
    assert!(fixed.output.contains("pass"));
    let source = "func f():\n\tvar unused = load(\"res://data.tres\")\n";
    let diagnostic = lint_source(path, source, &config)
        .unwrap()
        .diagnostics
        .into_iter()
        .find(|diagnostic| diagnostic.check_id() == "unused-variable")
        .unwrap();
    assert!(diagnostic.fixes.is_empty());
}

#[test]
fn semantic_rule_exemptions_cover_virtuals_underscores_and_string_signals() {
    use gdguard::linter::api::{Config, lint_source};
    let source =
        "signal changed\nfunc _ready():\n\tvar _intentional = 1\n\tself.emit_signal(\"changed\")\n";
    let diagnostics = lint_source(Path::new("test.gd"), source, &Config::default())
        .unwrap()
        .diagnostics;
    assert!(!diagnostics.iter().any(|diagnostic| matches!(
        diagnostic.check_id(),
        "unused-variable" | "unused-private-function" | "unused-local-signal"
    )));
}

#[test]
fn static_initializer_is_an_engine_callback() {
    use gdguard::linter::api::{Config, lint_source};
    let source = "class_name Config\nextends RefCounted\n\nstatic var ready_flag := false\n\n\nstatic func _static_init() -> void:\n\tready_flag = true\n\n\nstatic func _unused_helper() -> void:\n\tpass\n";
    let diagnostics = lint_source(Path::new("config.gd"), source, &Config::default())
        .unwrap()
        .diagnostics;
    let unused: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.check_id() == "unused-private-function")
        .collect();
    assert_eq!(unused.len(), 1);
    assert!(unused[0].message.contains("_unused_helper"));
}
