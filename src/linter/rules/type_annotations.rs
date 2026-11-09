//! Checks annotation presence, without inferring or inventing types.

use super::Rule;
use super::receiver_types::text;
use crate::linter::diagnostic::{Diagnostic, FileContext, Severity, Span};
use tree_sitter::Node;

pub const RULE_ID: &str = "type-annotations";

pub struct TypeAnnotationsRule {
    pub allow_inferred: bool,
}

impl Rule for TypeAnnotationsRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        if crate::linter::first_parse_error(context.tree, context.source).is_some() {
            return;
        }
        let mut cursor = context.tree.walk();
        loop {
            let node = cursor.node();
            match node.kind() {
                "function_definition" | "lambda" => {
                    if node.child_by_field_name("return_type").is_none() {
                        let name = node.child_by_field_name("name");
                        let label = match name {
                            Some(name) => format!("Function '{}'", text(context.source, name)),
                            None => "Lambda".to_string(),
                        };
                        // An anonymous lambda has no name; its `func` token
                        // provides a useful single-line ignore location.
                        let location = name.or_else(|| node.child(0)).unwrap_or(node);
                        report(
                            location,
                            format!("{} needs a return type.", label),
                            "Declare `-> Type`, or `-> void` when no value is returned.",
                            diagnostics,
                        );
                    }
                    if let Some(parameters) = node.child_by_field_name("parameters") {
                        self.check_parameters(parameters, context.source, "Parameter", diagnostics);
                    }
                }
                "signal_statement" => {
                    if let Some(parameters) = node.child_by_field_name("parameters") {
                        self.check_parameters(
                            parameters,
                            context.source,
                            "Signal parameter",
                            diagnostics,
                        );
                    }
                }
                "variable_statement" | "const_statement" => {
                    if !self.has_type(node) {
                        if let Some(name) = node.child_by_field_name("name") {
                            let label = if node.kind() == "const_statement" {
                                "Constant"
                            } else {
                                "Variable"
                            };
                            report(
                                name,
                                format!(
                                    "{} '{}' needs a type annotation.",
                                    label,
                                    text(context.source, name)
                                ),
                                "Declare `: Type`. Set type-annotations.allow_inferred = true to also accept `:=`.",
                                diagnostics,
                            );
                        }
                    }
                }
                "for_statement" => {
                    // A loop has no `:=` syntax. Its element type remains
                    // required even when inferred declarations are allowed.
                    if node.child_by_field_name("type").is_none() {
                        if let Some(name) = node.child_by_field_name("left") {
                            report(
                                name,
                                format!(
                                    "Loop variable '{}' needs a type annotation.",
                                    text(context.source, name)
                                ),
                                "Declare the element type: `for item: Type in items:`.",
                                diagnostics,
                            );
                        }
                    }
                }
                "type" => check_container_types(node, context.source, diagnostics),
                _ => {}
            }
            if cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    return;
                }
            }
        }
    }
}

impl TypeAnnotationsRule {
    fn has_type(&self, node: Node) -> bool {
        node.child_by_field_name("type")
            .is_some_and(|annotation| annotation.kind() != "inferred_type" || self.allow_inferred)
    }

    fn check_parameters(
        &self,
        parameters: Node,
        source: &str,
        label: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let mut cursor = parameters.walk();
        for mut parameter in parameters.named_children(&mut cursor) {
            if parameter.kind() == "comment" {
                continue;
            }
            if parameter.kind() == "variadic_parameter" {
                let Some(inner) = parameter.named_child(0) else {
                    continue;
                };
                parameter = inner;
            }
            if self.has_type(parameter) {
                continue;
            }
            let name = if parameter.kind() == "identifier" {
                parameter
            } else {
                let Some(name) = parameter.named_child(0) else {
                    continue;
                };
                name
            };
            report(
                name,
                format!(
                    "{} '{}' needs a type annotation.",
                    label,
                    text(source, name)
                ),
                "Declare the parameter as `name: Type`; use an explicit `Variant` for intentional dynamic values.",
                diagnostics,
            );
        }
    }
}

fn check_container_types(node: Node, source: &str, diagnostics: &mut Vec<Diagnostic>) {
    if node.kind() == "attribute" {
        // A project may define a qualified type such as Types.Array.
        return;
    }
    if node.kind() == "identifier" {
        let name = text(source, node);
        if matches!(name, "Array" | "Dictionary") {
            report(
                node,
                format!(
                    "{} needs {}.",
                    name,
                    if name == "Array" {
                        "an element type"
                    } else {
                        "key and value types"
                    }
                ),
                "Use `Array[Type]` or `Dictionary[KeyType, ValueType]`; use `Variant` explicitly for mixed values.",
                diagnostics,
            );
        }
        return;
    }
    if node.kind() == "subscript" {
        // The base is already parameterized; inspect its arguments only.
        if let Some(arguments) = node.child_by_field_name("arguments") {
            let mut cursor = arguments.walk();
            for argument in arguments.named_children(&mut cursor) {
                check_container_types(argument, source, diagnostics);
            }
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        check_container_types(child, source, diagnostics);
    }
}

fn report(node: Node, message: String, help: &str, diagnostics: &mut Vec<Diagnostic>) {
    diagnostics.push(Diagnostic {
        rule: RULE_ID.to_string(),
        check: None,
        severity: Severity::Warning,
        message,
        help: Some(help.to_string()),
        span: Span::from_node(&node),
        fixes: Vec::new(),
    });
}
