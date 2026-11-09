use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_constant_case};
use crate::linter::regex_patterns::{CONSTANT_CASE, PASCAL_CASE, PRIVATE_CONSTANT_CASE};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct ConstantNameRule;

impl ConstantNameRule {
    fn is_valid_constant_name(name: &str) -> bool {
        CONSTANT_CASE.is_match(name) || PRIVATE_CONSTANT_CASE.is_match(name)
    }

    fn is_valid_load_constant_name(name: &str) -> bool {
        // Load constants can use PascalCase or CONSTANT_CASE
        PASCAL_CASE.is_match(name)
            || CONSTANT_CASE.is_match(name)
            || PRIVATE_CONSTANT_CASE.is_match(name)
    }

    fn is_preload_call(node: &Node, source_code: &str) -> bool {
        if GDScriptNodeKind::get_kind_from_ast_node(*node) == GDScriptNodeKind::Call
            && let Some(function_node) = node.child(0)
        {
            let function_name = get_node_text(&function_node, source_code);
            return function_name == "preload";
        }

        false
    }
}

impl Rule for ConstantNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Const]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = get_node_text(&name_node, source_code);

        let is_preload_const = if let Some(value_node) = node.child_by_field_name("value") {
            Self::is_preload_call(&value_node, source_code)
        } else {
            false
        };

        let message = if is_preload_const {
            if Self::is_valid_load_constant_name(name) {
                return;
            }
            format!(
                "Preload constant name '{}' should be in PascalCase or CONSTANT_CASE format",
                name
            )
        } else {
            if Self::is_valid_constant_name(name) {
                return;
            }
            format!("Constant name '{}' should be in CONSTANT_CASE format", name)
        };

        let suggestion = to_constant_case(name);
        let mut help = None;
        if Self::is_valid_constant_name(&suggestion) {
            help = Some(format!("Rename it to '{}'.", suggestion));
        }
        diagnostics.push(Diagnostic {
            check: None,
            rule: "constant-name".to_string(),
            severity: Severity::Error,
            message,
            help,
            span: Span::from_node(&name_node),
            fixes: Vec::new(),
        });
    }
}
