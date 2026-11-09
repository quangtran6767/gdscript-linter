use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_snake_case};
use crate::linter::regex_patterns::{PRIVATE_SNAKE_CASE, SNAKE_CASE};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct SignalNameRule;

impl SignalNameRule {
    fn is_valid_signal_name(name: &str) -> bool {
        SNAKE_CASE.is_match(name) || PRIVATE_SNAKE_CASE.is_match(name)
    }
}

impl Rule for SignalNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Signal]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = get_node_text(&name_node, source_code);
        if Self::is_valid_signal_name(name) {
            return;
        }
        let suggestion = to_snake_case(name);
        let mut help = None;
        if Self::is_valid_signal_name(&suggestion) {
            help = Some(format!("Rename it to '{}'.", suggestion));
        }
        diagnostics.push(Diagnostic {
            check: None,
            rule: "signal-name".to_string(),
            severity: Severity::Error,
            message: format!("Signal name '{}' should be in snake_case format", name),
            help,
            span: Span::from_node(&name_node),
            fixes: Vec::new(),
        });
    }
}
