//! `Array[...]` and `Dictionary[...]` types stay on one line. Godot 4.7
//! rejects a line break inside the brackets of a variable, constant, return
//! or loop-variable type ("Expected type for collection after ..."), while
//! the bundled parser accepts it. Parameter and lambda types are exempt:
//! Godot accepts them inside the parameter list. Backslash continuations
//! are accepted by Godot and not reported.

use super::Rule;
use super::receiver_types::{nodes_of_kind, text};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};

pub const RULE_ID: &str = "single-line-container-type";

pub struct SingleLineContainerTypeRule;

impl Rule for SingleLineContainerTypeRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        for type_node in nodes_of_kind(context.tree, "type") {
            if !is_checked_position(type_node) {
                continue;
            }
            let Some(subscript) = type_node.named_child(0) else {
                continue;
            };
            if subscript.kind() != "subscript" {
                continue;
            }
            let Some(container) = subscript.named_child(0) else {
                continue;
            };
            let container_name = text(context.source, container);
            if container_name != "Array" && container_name != "Dictionary" {
                continue;
            }
            let type_text = text(context.source, type_node);
            if !has_bare_newline(type_text) {
                continue;
            }
            let mut fixes = Vec::new();
            if !type_text.contains('#') {
                fixes.push(Fix {
                    description: "Join the type on one line".to_string(),
                    safety: FixSafety::Safe,
                    edits: vec![Edit {
                        span: Span::from_node(&type_node),
                        replacement: join_type(type_text),
                    }],
                });
            }
            diagnostics.push(Diagnostic {
                check: None,
                rule: RULE_ID.to_string(),
                severity: Severity::Error,
                message: format!(
                    "Keep the {} type on one line; Godot rejects a line break inside its brackets.",
                    container_name
                ),
                help: Some(
                    "Write Array[T] or Dictionary[K, V] without a line break inside the brackets. Move comments outside the type."
                        .to_string(),
                ),
                span: Span::from_node(&type_node),
                fixes,
            });
        }
    }
}

/// Variable, constant, function return and loop variable types, outside
/// parameter lists and lambdas.
fn is_checked_position(type_node: tree_sitter::Node) -> bool {
    let Some(parent) = type_node.parent() else {
        return false;
    };
    if !matches!(
        parent.kind(),
        "variable_statement" | "const_statement" | "function_definition" | "for_statement"
    ) {
        return false;
    }
    let mut ancestor = Some(parent);
    while let Some(node) = ancestor {
        if matches!(
            node.kind(),
            "lambda" | "parameters" | "typed_parameter" | "typed_default_parameter"
        ) {
            return false;
        }
        ancestor = node.parent();
    }
    true
}

/// True when `type_text` has a line break that isn't a `\` continuation.
fn has_bare_newline(type_text: &str) -> bool {
    let bytes = type_text.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        let mut before = index;
        if before > 0 && bytes[before - 1] == b'\r' {
            before -= 1;
        }
        if before == 0 || bytes[before - 1] != b'\\' {
            return true;
        }
    }
    false
}

/// The type without whitespace or `\` continuations, with one space after
/// each comma: `Dictionary[String,\n\tint]` gives `Dictionary[String, int]`.
fn join_type(type_text: &str) -> String {
    let mut joined = String::with_capacity(type_text.len());
    for character in type_text.chars() {
        if character.is_whitespace() || character == '\\' {
            continue;
        }
        joined.push(character);
        if character == ',' {
            joined.push(' ');
        }
    }
    joined
}
