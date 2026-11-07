use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_pascal_case};
use crate::linter::regex_patterns::PASCAL_CASE;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct EnumNameRule;

impl EnumNameRule {
    fn is_valid_enum_name(name: &str) -> bool {
        PASCAL_CASE.is_match(name)
    }
}

impl Rule for EnumNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Enum]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(name_node) = node.child_by_field_name("name") else {
            return;
        };
        let name = get_node_text(&name_node, source_code);
        if Self::is_valid_enum_name(name) {
            return;
        }
        let suggestion = to_pascal_case(name);
        let mut help = None;
        if Self::is_valid_enum_name(&suggestion) {
            help = Some(format!("Rename it to '{}'.", suggestion));
        }
        diagnostics.push(Diagnostic {
            rule: "enum-name".to_string(),
            severity: Severity::Error,
            message: format!("Enum name '{}' should be in PascalCase format", name),
            help,
            span: Span::from_node(&name_node),
            fixes: Vec::new(),
        });
    }
}
