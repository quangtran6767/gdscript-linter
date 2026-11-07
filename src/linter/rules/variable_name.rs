use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_snake_case};
use crate::linter::regex_patterns::{PASCAL_CASE, PRIVATE_SNAKE_CASE, SNAKE_CASE};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct VariableNameRule;

impl VariableNameRule {
    fn is_valid_variable_name(name: &str) -> bool {
        SNAKE_CASE.is_match(name) || PRIVATE_SNAKE_CASE.is_match(name)
    }

    fn is_valid_load_variable_name(name: &str) -> bool {
        PASCAL_CASE.is_match(name) || SNAKE_CASE.is_match(name) || PRIVATE_SNAKE_CASE.is_match(name)
    }

    fn is_load_call(node: &Node, source_code: &str) -> bool {
        if GDScriptNodeKind::get_kind_from_ast_node(*node) == GDScriptNodeKind::Call
            && let Some(function_node) = node.child(0)
        {
            let function_name = get_node_text(&function_node, source_code);
            return function_name == "load" || function_name == "preload";
        }
        false
    }
}

impl Rule for VariableNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[
            GDScriptNodeKind::Variable,
            GDScriptNodeKind::ExportVariable,
            GDScriptNodeKind::OnReadyVariable,
        ]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = get_node_text(&name_node, source_code);

        let is_load_var = if let Some(value_node) = node.child_by_field_name("value") {
            Self::is_load_call(&value_node, source_code)
        } else {
            false
        };

        // Variables holding a loaded resource may also use PascalCase, like
        // classes. They report under their own rule id, which is enabled and
        // disabled together with `variable-name`.
        let (rule, message) = if is_load_var {
            if Self::is_valid_load_variable_name(name) {
                return;
            }
            (
                "load-variable-name",
                format!(
                    "Variable name '{}' should be in PascalCase, snake_case or _private_snake_case format",
                    name
                ),
            )
        } else {
            if Self::is_valid_variable_name(name) {
                return;
            }
            (
                "variable-name",
                format!(
                    "Variable name '{}' should be in snake_case or _private_snake_case format",
                    name
                ),
            )
        };

        let suggestion = to_snake_case(name);
        let mut help = None;
        if Self::is_valid_variable_name(&suggestion) {
            help = Some(format!("Rename it to '{}'.", suggestion));
        }
        diagnostics.push(Diagnostic {
            rule: rule.to_string(),
            severity: Severity::Error,
            message,
            help,
            span: Span::from_node(&name_node),
            fixes: Vec::new(),
        });
    }
}
