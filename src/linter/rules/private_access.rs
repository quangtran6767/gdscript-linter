use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::get_node_text;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct PrivateAccessRule;

impl Rule for PrivateAccessRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Attribute]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let mut attr_cursor = node.walk();
        if !attr_cursor.goto_first_child() {
            return;
        }
        let object_node = attr_cursor.node();
        let object_name = get_node_text(&object_node, source_code);
        if object_name == "super" || object_name == "self" {
            return;
        }
        // Skip the `.` to reach the accessed member.
        if !(attr_cursor.goto_next_sibling() && attr_cursor.goto_next_sibling()) {
            return;
        }
        let member_node = attr_cursor.node();
        match GDScriptNodeKind::get_kind_from_ast_node(member_node) {
            GDScriptNodeKind::AttributeCall => {
                let Some(method_name_node) = member_node.child(0) else {
                    return;
                };
                let method_name = get_node_text(&method_name_node, source_code);
                if !method_name.starts_with('_') {
                    return;
                }
                diagnostics.push(Diagnostic {
                    rule: "private-access".to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Private method '{}' should not be called from outside its class",
                        method_name
                    ),
                    help: Some(
                        "Call a public method instead, or make this one public by removing the leading underscore."
                            .to_string(),
                    ),
                    span: Span::from_node(&method_name_node),
                    fixes: Vec::new(),
                });
            }
            GDScriptNodeKind::Identifier => {
                let variable_name = get_node_text(&member_node, source_code);
                if !variable_name.starts_with('_') {
                    return;
                }
                diagnostics.push(Diagnostic {
                    rule: "private-access".to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Private variable '{}' should not be accessed from outside its class",
                        variable_name
                    ),
                    help: Some(
                        "Use a public property or method instead, or make this one public by removing the leading underscore."
                            .to_string(),
                    ),
                    span: Span::from_node(&member_node),
                    fixes: Vec::new(),
                });
            }
            _ => {}
        }
    }
}
