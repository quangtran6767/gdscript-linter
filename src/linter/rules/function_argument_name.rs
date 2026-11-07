use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_snake_case};
use crate::linter::regex_patterns::{PRIVATE_SNAKE_CASE, SNAKE_CASE};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct FunctionArgumentNameRule;

impl FunctionArgumentNameRule {
    fn is_valid_argument_name(name: &str) -> bool {
        SNAKE_CASE.is_match(name) || PRIVATE_SNAKE_CASE.is_match(name)
    }
}

impl Rule for FunctionArgumentNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Function]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(params_node) = node.child_by_field_name("parameters") else {
            return;
        };
        let mut params_cursor = params_node.walk();
        if !params_cursor.goto_first_child() {
            return;
        }
        loop {
            let param_node = params_cursor.node();
            let param_kind = GDScriptNodeKind::get_kind_from_ast_node(param_node);
            // A parameter is either a plain identifier or a node whose first
            // child is the name (typed and default parameters).
            let name_node = match param_kind {
                GDScriptNodeKind::Identifier => Some(param_node),
                GDScriptNodeKind::Parameter => param_node.child(0),
                _ => None,
            };
            if let Some(name_node) = name_node {
                let param_name = get_node_text(&name_node, source_code);
                if !param_name.is_empty() && !Self::is_valid_argument_name(param_name) {
                    let suggestion = to_snake_case(param_name);
                    let mut help = None;
                    if Self::is_valid_argument_name(&suggestion) {
                        help = Some(format!("Rename it to '{}'.", suggestion));
                    }
                    diagnostics.push(Diagnostic {
                        rule: "function-argument-name".to_string(),
                        severity: Severity::Error,
                        message: format!(
                            "Function argument '{}' should be in snake_case or _private_snake_case format",
                            param_name
                        ),
                        help,
                        span: Span::from_node(&name_node),
                        fixes: Vec::new(),
                    });
                }
            }
            if !params_cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
