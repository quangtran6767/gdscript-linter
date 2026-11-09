//! SM0 contract behavior at the empty-model boundary.

use gdscript_formatter::project::{MemberLookup, ProjectIndex};
use gdscript_formatter::semantic::{ScopeId, ScopeKind, SemanticModel, SymbolId};
use std::path::Path;

#[test]
fn semantic_stub_has_a_root_and_unknown_ids_never_panic() {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_gdscript::LANGUAGE.into())
        .expect("grammar");
    let tree = parser.parse("", None).expect("tree");
    let model = SemanticModel::build(&tree, "");
    assert_eq!(model.scopes().len(), 1);
    assert_eq!(model.scopes()[0].kind, ScopeKind::Class);
    assert_eq!(model.scope_at(usize::MAX), ScopeId(0));
    assert_eq!(model.lookup(ScopeId(u32::MAX), "unknown"), None);
    assert_eq!(model.symbol(SymbolId(u32::MAX)), None);
    assert!(model.references_to(SymbolId(u32::MAX)).is_empty());
    assert!(model.unresolved_references().is_empty());
    assert_eq!(model.debug_dump(), "scope 0 Class 0..0 parent=-\n");
}

#[test]
fn empty_project_cannot_claim_a_missing_member() {
    let root = std::env::temp_dir().join(format!("gdguard-contract-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("empty project folder");
    std::fs::write(root.join("project.godot"), "config_version=5\n").expect("project file");
    let index = ProjectIndex::build(&root).expect("empty project index");
    assert_eq!(index.root, Path::new(&root));
    assert!(index.scripts.is_empty());
    assert!(index.class_names.is_empty());
    assert!(index.autoloads.is_empty());
    assert_eq!(
        index.has_member("MissingClass", "member"),
        MemberLookup::Unknown
    );
    std::fs::remove_dir_all(root).expect("remove project folder");
}
