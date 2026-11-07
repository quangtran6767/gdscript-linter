use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct NoElseReturnRule;

impl NoElseReturnRule {
    fn body_ends_with_return(body_node: &Node) -> bool {
        let mut cursor = body_node.walk();
        let mut last_statement = None;

        if cursor.goto_first_child() {
            loop {
                let child_node = cursor.node();
                // Skip whitespace and comments
                let child_kind = GDScriptNodeKind::get_kind_from_ast_node(child_node);
                if child_kind != GDScriptNodeKind::Comment && child_kind != GDScriptNodeKind::Other
                {
                    last_statement = Some(child_node);
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }

        if let Some(last_stmt) = last_statement {
            return GDScriptNodeKind::get_kind_from_ast_node(last_stmt)
                == GDScriptNodeKind::ReturnStatement;
        }

        false
    }

    /// The span of the `elif` or `else` keyword that starts a clause, so
    /// reporters underline the keyword and not the whole block.
    fn keyword_span(clause_node: &Node) -> Span {
        match clause_node.child(0) {
            Some(keyword_node) => Span::from_node(&keyword_node),
            None => Span::from_node(clause_node),
        }
    }
}

impl Rule for NoElseReturnRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::IfStatement]
    }

    fn check_node(&mut self, node: &Node, _source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let mut if_body_ends_with_return = false;
        if let Some(body_node) = node.child_by_field_name("body") {
            if_body_ends_with_return = Self::body_ends_with_return(&body_node);
        }

        let mut all_branches_return = if_body_ends_with_return;

        let mut stmt_cursor = node.walk();
        if !stmt_cursor.goto_first_child() {
            return;
        }
        loop {
            let child_node = stmt_cursor.node();
            let child_kind = GDScriptNodeKind::get_kind_from_ast_node(child_node);
            if child_kind == GDScriptNodeKind::ElifStatement {
                if if_body_ends_with_return {
                    diagnostics.push(Diagnostic {
                        rule: "no-else-return".to_string(),
                        severity: Severity::Warning,
                        message: "Unnecessary 'elif' after 'if' block that ends with 'return'. Use 'if' instead".to_string(),
                        help: Some(
                            "Replace 'elif' with 'if': the block before it always returns."
                                .to_string(),
                        ),
                        span: Self::keyword_span(&child_node),
                        fixes: Vec::new(),
                    });
                }

                if let Some(elif_body) = child_node.child_by_field_name("body")
                    && !Self::body_ends_with_return(&elif_body)
                {
                    all_branches_return = false;
                }
            } else if child_kind == GDScriptNodeKind::ElseStatement && all_branches_return {
                diagnostics.push(Diagnostic {
                    rule: "no-else-return".to_string(),
                    severity: Severity::Warning,
                    message: "Unnecessary 'else' after 'if'/'elif' blocks that end with 'return'"
                        .to_string(),
                    help: Some(
                        "Remove the 'else' line and dedent its body: the blocks before it always return."
                            .to_string(),
                    ),
                    span: Self::keyword_span(&child_node),
                    fixes: Vec::new(),
                });
            }
            if !stmt_cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
