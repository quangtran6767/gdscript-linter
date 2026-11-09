//! Statements after `return`, `break` or `continue` in the same block.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct UnreachableCodeRule;
impl Rule for UnreachableCodeRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Body]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let mut exit: Option<&str> = None;
        for index in 0..node.named_child_count() {
            let Some(child) = node.named_child(index as u32) else {
                continue;
            };
            if child.kind() == "comment" {
                continue;
            }
            if let Some(exit) = exit {
                let end = node
                    .named_child(node.named_child_count() as u32 - 1)
                    .map_or(child.end_byte(), |last| last.end_byte());
                diagnostics.push(Diagnostic {
                    check: None,
                    rule: "unreachable-code".to_string(),
                    severity: Severity::Warning,
                    message: format!("Unreachable code after '{}'.", exit),
                    help: Some(format!(
                        "Nothing after `{}` in this block runs. Remove the code, or move it before `{}`.",
                        exit, exit
                    )),
                    span: Span::from_byte_range(source_code, child.start_byte(), end),
                    fixes: Vec::new(),
                });
                return;
            }
            exit = match child.kind() {
                "return_statement" => Some("return"),
                "break_statement" => Some("break"),
                "continue_statement" => Some("continue"),
                _ => None,
            };
        }
    }
}
