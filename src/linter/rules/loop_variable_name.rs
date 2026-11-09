use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_snake_case};
use crate::linter::regex_patterns::{PRIVATE_SNAKE_CASE, SNAKE_CASE};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct LoopVariableNameRule;

impl LoopVariableNameRule {
    fn is_valid_loop_variable_name(name: &str) -> bool {
        SNAKE_CASE.is_match(name) || PRIVATE_SNAKE_CASE.is_match(name)
    }
}

impl Rule for LoopVariableNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::ForStatement]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        // In GDScript, for loops have the pattern: for <variable> in <iterable>:
        // The variable is an identifier or a typed parameter like
        // "for i: int in range(10):", whose first child is the name.
        let Some(left_node) = node.child_by_field_name("left") else {
            return;
        };
        let name_node = match GDScriptNodeKind::get_kind_from_ast_node(left_node) {
            GDScriptNodeKind::Identifier => left_node,
            GDScriptNodeKind::Parameter => {
                let Some(name_child) = left_node.child(0) else {
                    return;
                };
                name_child
            }
            _ => return,
        };
        let variable_name = get_node_text(&name_node, source_code);
        if variable_name.is_empty() || Self::is_valid_loop_variable_name(variable_name) {
            return;
        }
        let suggestion = to_snake_case(variable_name);
        let mut help = None;
        if Self::is_valid_loop_variable_name(&suggestion) {
            help = Some(format!("Rename it to '{}'.", suggestion));
        }
        diagnostics.push(Diagnostic {
            check: None,
            rule: "loop-variable-name".to_string(),
            severity: Severity::Error,
            message: format!(
                "Loop variable '{}' should be in snake_case_or _private_snake_case format",
                variable_name
            ),
            help,
            span: Span::from_node(&name_node),
            fixes: Vec::new(),
        });
    }
}
