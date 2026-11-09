use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::get_node_text;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct ComparisonWithItselfRule;

impl Rule for ComparisonWithItselfRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::BinaryOperator]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let (Some(left_node), Some(op_node), Some(right_node)) = (
            node.child_by_field_name("left"),
            node.child_by_field_name("op"),
            node.child_by_field_name("right"),
        ) else {
            return;
        };
        let op = get_node_text(&op_node, source_code);
        if !matches!(op, "==" | "!=" | "<" | ">" | "<=" | ">=") {
            return;
        }
        let left_text = get_node_text(&left_node, source_code);
        let right_text = get_node_text(&right_node, source_code);
        if left_text != right_text {
            return;
        }
        diagnostics.push(Diagnostic {
            check: None,
            rule: "comparison-with-itself".to_string(),
            severity: Severity::Warning,
            message: format!(
                "Redundant comparison '{}' - comparing expression with itself",
                get_node_text(node, source_code)
            ),
            help: Some(
                "The result never changes. Compare with the value you meant to check, or remove the condition."
                    .to_string(),
            ),
            span: Span::from_node(node),
            fixes: Vec::new(),
        });
    }
}
