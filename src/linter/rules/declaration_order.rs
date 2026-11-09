//! Top-level style-guide ordering, shared with the formatter.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use crate::reorder::{DeclarationKind, ReorderItem, ReorderPlan, build_reorder_plan};
use crate::{FormatterConfiguration, format_gdscript};
use tree_sitter::Node;

pub struct DeclarationOrderRule;

impl Rule for DeclarationOrderRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let root = context.tree.root_node();
        if root.has_error() {
            return;
        }
        let plan = build_reorder_plan(root, context.source);
        let mut source_order = Vec::with_capacity(plan.items.len());
        for index in 0..plan.items.len() {
            source_order.push(index);
        }
        source_order.sort_by_key(|&index| {
            let item = &plan.items[index];
            (item.child_index, item.sub_child)
        });
        let mut mismatch = None;
        for index in 0..source_order.len() {
            if source_order[index] != index {
                mismatch = Some(index);
                break;
            }
        }
        let Some(index) = mismatch else {
            return;
        };
        let item = &plan.items[index];
        let before = &plan.items[source_order[index]];
        let node = declaration_node(root, item);
        let before_description = if item.declaration_kind == before.declaration_kind {
            describe_declaration(root, before, context.source)
        } else {
            category_name(before.declaration_kind).to_string()
        };
        let mut diagnostic = Diagnostic {
            rule: "declaration-order".to_string(),
            severity: Severity::Warning,
            message: format!(
                "{} should come before {}",
                describe_declaration(root, item, context.source),
                before_description
            ),
            help: Some(
                "Use the formatter's top-level declaration order within each #region group. Format the file first to enable its reorder fix."
                    .to_string(),
            ),
            span: Span::from_node(&node),
            fixes: Vec::new(),
        };

        // Use the formatter only on already-formatted input: moving code must
        // never quietly reformat declaration bodies. Share its marker scanner
        // so strings containing `# fmt: off` don't suppress a valid fix.
        if crate::parser::find_disabled_regions(context.source).is_empty() {
            let mut config = FormatterConfiguration::default();
            crate::editorconfig::apply_editorconfig_to_formatter_config(&mut config, context.path);
            if crate::linter::fix::is_formatted(context.source, &config) {
                config.reorder_code = true;
                if let Ok(reordered) = format_gdscript(context.source, &config) {
                    diagnostic.fixes.push(Fix {
                        description: "Reorder top-level declarations".to_string(),
                        safety: reorder_safety(root, &plan, &source_order, context.source),
                        edits: vec![Edit {
                            span: Span::from_byte_range(context.source, 0, context.source.len()),
                            replacement: reordered,
                        }],
                    });
                }
            }
        }
        diagnostics.push(diagnostic);
    }
}

fn declaration_node<'tree>(root: Node<'tree>, item: &ReorderItem) -> Node<'tree> {
    let node = root.child(item.child_index).expect("item belongs to root");
    match item.sub_child {
        Some(index) => node
            .child(index)
            .expect("split extends belongs to class_name"),
        None => node,
    }
}

fn describe_declaration(root: Node, item: &ReorderItem, source: &str) -> String {
    let node = declaration_node(root, item);
    let kind = match item.declaration_kind {
        DeclarationKind::ClassAnnotation => "annotation",
        DeclarationKind::ClassName => "class_name",
        DeclarationKind::Extends => "extends",
        DeclarationKind::ClassDocstring => "class docstring",
        DeclarationKind::Signal => "signal",
        DeclarationKind::Enum => "enum",
        DeclarationKind::Constant => "constant",
        DeclarationKind::StaticVariable
        | DeclarationKind::ExportVariable
        | DeclarationKind::RegularVariable
        | DeclarationKind::OnReadyVariable => "variable",
        DeclarationKind::Method => "method",
        DeclarationKind::InnerClass => "inner class",
        DeclarationKind::RegionStartOrEnd => "region marker",
        DeclarationKind::Unknown => "declaration",
    };
    if let Some(name) = node.child_by_field_name("name") {
        format!("{} `{}`", kind, &source[name.byte_range()])
    } else if node.kind() == "constructor_definition" {
        "method `_init`".to_string()
    } else {
        kind.to_string()
    }
}

fn category_name(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::ClassAnnotation => "class annotations",
        DeclarationKind::ClassName => "class_name",
        DeclarationKind::Extends => "extends",
        DeclarationKind::ClassDocstring => "the class docstring",
        DeclarationKind::Signal => "signals",
        DeclarationKind::Enum => "enums",
        DeclarationKind::Constant => "constants",
        DeclarationKind::StaticVariable => "static variables",
        DeclarationKind::ExportVariable => "exported variables",
        DeclarationKind::RegularVariable => "variables",
        DeclarationKind::OnReadyVariable => "onready variables",
        DeclarationKind::Method => "methods",
        DeclarationKind::InnerClass => "inner classes",
        DeclarationKind::RegionStartOrEnd => "region markers",
        DeclarationKind::Unknown => "other declarations",
    }
}

fn reorder_safety(
    root: Node,
    plan: &ReorderPlan,
    source_order: &[usize],
    source: &str,
) -> FixSafety {
    for source_index in 0..source_order.len() {
        let index = source_order[source_index];
        let item = &plan.items[index];
        if index == source_index
            || !matches!(
                item.declaration_kind,
                DeclarationKind::StaticVariable
                    | DeclarationKind::ExportVariable
                    | DeclarationKind::RegularVariable
                    | DeclarationKind::OnReadyVariable
            )
        {
            continue;
        }
        let node = declaration_node(root, item);
        let Some(value) = node.child_by_field_name("value") else {
            continue;
        };
        // W10 explicitly permits a bare member alias (`var b = a`) while
        // requiring unsafe opt-in for computed initializers (`a + 1`).
        if value.kind() == "identifier" {
            continue;
        }
        for later in &plan.items[index + 1..] {
            // Only member variables have declaration-ordered initialization;
            // methods and compile-time declarations can move safely.
            if !matches!(
                later.declaration_kind,
                DeclarationKind::StaticVariable
                    | DeclarationKind::ExportVariable
                    | DeclarationKind::RegularVariable
                    | DeclarationKind::OnReadyVariable
            ) {
                continue;
            }
            let later_node = declaration_node(root, later);
            let Some(name) = later_node.child_by_field_name("name") else {
                continue;
            };
            if references_member(value, &source[name.byte_range()], source) {
                return FixSafety::Unsafe;
            }
        }
    }
    FixSafety::Safe
}

fn references_member(node: Node, member: &str, source: &str) -> bool {
    if node.kind() == "identifier" {
        return &source[node.byte_range()] == member;
    }
    // Attribute names on other objects aren't references to this script's
    // members. Walk the receiver and arguments, and count `self.member`.
    if node.kind() == "attribute" {
        let receiver = node.named_child(0).expect("attribute has a receiver");
        if references_member(receiver, member, source) {
            return true;
        }
        let is_self = &source[receiver.byte_range()] == "self";
        for index in 1..node.named_child_count() {
            let child = node
                .named_child(index as u32)
                .expect("attribute child exists");
            if is_self && index == 1 {
                let name = if child.kind() == "identifier" {
                    Some(child)
                } else {
                    child.named_child(0)
                };
                if name.is_some_and(|name| &source[name.byte_range()] == member) {
                    return true;
                }
            }
            if let Some(arguments) = child.child_by_field_name("arguments") {
                if references_member(arguments, member, source) {
                    return true;
                }
            }
        }
        return false;
    }
    for index in 0..node.named_child_count() {
        let child = node
            .named_child(index as u32)
            .expect("expression child exists");
        if references_member(child, member, source) {
            return true;
        }
    }
    false
}
