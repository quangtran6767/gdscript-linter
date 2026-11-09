use crate::linter::diagnostic::{Diagnostic, Edit, Fix, FixSafety, Severity, Span};
use crate::linter::lib::get_node_text;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

/// This rule checks for unused function arguments: if a function argument is
/// not used in the function body, it suggests removing it or prefixing it with
/// an underscore (_). Arguments that start with an underscore are ignored.
pub struct UnusedArgumentRule;

impl UnusedArgumentRule {
    fn is_identifier_used_in_node(node: &Node, identifier: &str, source_code: &str) -> bool {
        let mut cursor = node.walk();

        fn check_usage(
            cursor: &mut tree_sitter::TreeCursor,
            identifier: &str,
            source_code: &str,
        ) -> bool {
            let node = cursor.node();

            if GDScriptNodeKind::get_kind_from_ast_node(node) == GDScriptNodeKind::Identifier {
                let node_text = get_node_text(&node, source_code);
                if node_text == identifier {
                    return true;
                }
            }

            if cursor.goto_first_child() {
                loop {
                    if check_usage(cursor, identifier, source_code) {
                        return true;
                    }
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }

            false
        }

        check_usage(&mut cursor, identifier, source_code)
    }
}

impl Rule for UnusedArgumentRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Function]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        // Parameter name nodes that don't start with an underscore.
        let mut parameter_name_nodes = Vec::new();

        if let Some(params_node) = node.child_by_field_name("parameters") {
            let mut params_cursor = params_node.walk();
            if params_cursor.goto_first_child() {
                loop {
                    let param_node = params_cursor.node();
                    // A parameter is either a plain identifier or a node whose
                    // first child is the name (typed and default parameters).
                    let name_node = match GDScriptNodeKind::get_kind_from_ast_node(param_node) {
                        GDScriptNodeKind::Identifier => Some(param_node),
                        GDScriptNodeKind::Parameter => param_node.child(0),
                        _ => None,
                    };
                    if let Some(name_node) = name_node {
                        let param_name = get_node_text(&name_node, source_code);
                        if !param_name.is_empty() && !param_name.starts_with('_') {
                            parameter_name_nodes.push(name_node);
                        }
                    }
                    if !params_cursor.goto_next_sibling() {
                        break;
                    }
                }
            }
        }

        let Some(body_node) = node.child_by_field_name("body") else {
            return;
        };
        for name_node in parameter_name_nodes {
            let param_name = get_node_text(&name_node, source_code);
            if Self::is_identifier_used_in_node(&body_node, param_name, source_code) {
                continue;
            }
            diagnostics.push(Diagnostic {
                check: None,
                rule: "unused-argument".to_string(),
                severity: Severity::Warning,
                message: format!(
                    "Function argument '{}' is unused. Consider removing it or prefixing with '_'",
                    param_name
                ),
                help: Some(format!(
                    "Remove '{}', or rename it to '_{}' if the function signature must stay the same.",
                    param_name, param_name
                )),
                span: Span::from_node(&name_node),
                fixes: vec![Fix {
                    description: format!("Rename it to '_{}'", param_name),
                    // Unsafe: the rule can miss a use it doesn't see, such as
                    // a lookup by name, and the rename would then break it.
                    safety: FixSafety::Unsafe,
                    edits: vec![Edit {
                        span: Span::from_node(&name_node),
                        replacement: format!("_{}", param_name),
                    }],
                }],
            });
        }
    }
}
