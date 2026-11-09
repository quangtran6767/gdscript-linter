use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::get_node_text;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct StandaloneExpressionRule;

impl Rule for StandaloneExpressionRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::ExpressionStatement]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(expr_child) = node.child(0) else {
            return;
        };
        let expr_kind = GDScriptNodeKind::get_kind_from_ast_node(expr_child);
        if !matches!(
            expr_kind,
            GDScriptNodeKind::BinaryOperator | GDScriptNodeKind::Literal | GDScriptNodeKind::String
        ) {
            return;
        }
        let expr_text = get_node_text(&expr_child, source_code);
        diagnostics.push(Diagnostic {
            check: None,
            rule: "standalone-expression".to_string(),
            severity: Severity::Warning,
            message: format!(
                "Standalone expression '{}' is not assigned or used, the line may have no effect",
                expr_text
            ),
            help: Some("Assign or use the value, or remove the line.".to_string()),
            span: Span::from_node(&expr_child),
            fixes: Vec::new(),
        });
    }
}
