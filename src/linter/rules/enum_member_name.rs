use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::{get_node_text, to_constant_case};
use crate::linter::regex_patterns::CONSTANT_CASE;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct EnumMemberNameRule;

impl EnumMemberNameRule {
    fn is_valid_enum_member_name(name: &str) -> bool {
        CONSTANT_CASE.is_match(name)
    }
}

impl Rule for EnumMemberNameRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Enum]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let Some(body_node) = node.child_by_field_name("body") else {
            return;
        };
        let mut enum_cursor = body_node.walk();
        if !enum_cursor.goto_first_child() {
            return;
        }
        loop {
            let enum_member = enum_cursor.node();
            if GDScriptNodeKind::get_kind_from_ast_node(enum_member) == GDScriptNodeKind::Enumerator
                && let Some(element_name_node) = enum_member.child_by_field_name("left")
            {
                let element_name = get_node_text(&element_name_node, source_code);
                // Skip empty enum member names (happens with empty enums)
                if !element_name.is_empty() && !Self::is_valid_enum_member_name(element_name) {
                    let suggestion = to_constant_case(element_name);
                    let mut help = None;
                    if Self::is_valid_enum_member_name(&suggestion) {
                        help = Some(format!("Rename it to '{}'.", suggestion));
                    }
                    diagnostics.push(Diagnostic {
                        check: None,
                        rule: "enum-member-name".to_string(),
                        severity: Severity::Error,
                        message: format!(
                            "Enum element name '{}' should be in CONSTANT_CASE format",
                            element_name
                        ),
                        help,
                        span: Span::from_node(&element_name_node),
                        fixes: Vec::new(),
                    });
                }
            }
            if !enum_cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
