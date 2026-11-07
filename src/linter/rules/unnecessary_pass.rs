use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct UnnecessaryPassRule;

impl Rule for UnnecessaryPassRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Body, GDScriptNodeKind::ClassBody]
    }

    fn check_node(&mut self, node: &Node, _source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let mut has_other_statements = false;
        let mut pass_nodes = Vec::new();

        let mut body_cursor = node.walk();
        if body_cursor.goto_first_child() {
            loop {
                let stmt_node = body_cursor.node();
                let stmt_kind = GDScriptNodeKind::get_kind_from_ast_node(stmt_node);
                if stmt_kind == GDScriptNodeKind::PassStatement {
                    pass_nodes.push(stmt_node);
                } else if stmt_kind != GDScriptNodeKind::Comment
                    && stmt_kind != GDScriptNodeKind::Other
                {
                    has_other_statements = true;
                }
                if !body_cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        if !has_other_statements {
            return;
        }
        for pass_node in pass_nodes {
            diagnostics.push(Diagnostic {
                rule: "unnecessary-pass".to_string(),
                severity: Severity::Warning,
                message: "Unnecessary 'pass' statement when other statements are present"
                    .to_string(),
                help: Some("Remove the 'pass' statement.".to_string()),
                span: Span::from_node(&pass_node),
                fixes: Vec::new(),
            });
        }
    }
}
