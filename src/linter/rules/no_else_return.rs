use crate::linter::diagnostic::{Diagnostic, Edit, Fix, FixSafety, Severity, Span};
use crate::linter::rules::Rule;
use crate::linter::rules::fix_helpers::{line_end, line_indentation, line_start};
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

    /// Replaces `elif` with `if`. Only behavior-preserving when every branch
    /// before the `elif` returns, so the caller checks that.
    fn elif_fix(clause_node: &Node) -> Option<Fix> {
        let keyword_node = clause_node.child(0)?;
        if keyword_node.kind() != "elif" {
            return None;
        }
        Some(Fix {
            description: "Replace 'elif' with 'if'".to_string(),
            safety: FixSafety::Safe,
            edits: vec![Edit {
                span: Span::from_node(&keyword_node),
                replacement: "if".to_string(),
            }],
        })
    }

    /// Removes the `else:` line and dedents its body by one level. Returns
    /// None when the layout is unusual: code or a comment on the `else`
    /// line, a body indented differently, or a multi-line string in the body
    /// (dedenting would change the string).
    ///
    /// Safe, unless the body declares variables or constants at its top
    /// level: they move to the enclosing scope, where a later declaration
    /// with the same name would clash. Then the fix is Unsafe.
    fn else_fix(source_code: &str, clause_node: &Node) -> Option<Fix> {
        let body_node = clause_node.child_by_field_name("body")?;
        let else_start = clause_node.start_byte();
        let else_line_start = line_start(source_code, else_start);
        if !source_code[else_line_start..else_start].trim().is_empty() {
            return None;
        }
        if Self::contains_multiline_string(&body_node) {
            return None;
        }
        let safety = if Self::declares_names(&body_node) {
            FixSafety::Unsafe
        } else {
            FixSafety::Safe
        };
        let description = "Remove the 'else' and dedent its body".to_string();

        // The body node can start right after the colon, so measure from its
        // first and last statements.
        let first_statement = body_node.named_child(0)?;
        let last_index = u32::try_from(body_node.named_child_count().checked_sub(1)?).ok()?;
        let last_statement = body_node.named_child(last_index)?;
        // `else: return value` on one line: keep only the body.
        if first_statement.start_position().row == clause_node.start_position().row {
            if last_statement.end_position().row != clause_node.start_position().row {
                return None;
            }
            return Some(Fix {
                description,
                safety,
                edits: vec![Edit {
                    span: Span::from_byte_range(
                        source_code,
                        else_start,
                        first_statement.start_byte(),
                    ),
                    replacement: String::new(),
                }],
            });
        }

        let else_line_end = line_end(source_code, else_start);
        if source_code[else_start..else_line_end].trim() != "else:" {
            return None;
        }
        let else_indentation = line_indentation(source_code, else_start);
        let body_indentation = line_indentation(source_code, first_statement.start_byte());
        if body_indentation.len() <= else_indentation.len()
            || !body_indentation.starts_with(else_indentation)
        {
            return None;
        }
        let full_indentation = body_indentation;
        let body_start = else_line_end + 1;
        let body_end = line_end(source_code, last_statement.end_byte());
        if body_start > body_end {
            return None;
        }

        let mut dedented = String::with_capacity(body_end - body_start);
        for line in source_code[body_start..body_end].split_inclusive('\n') {
            match line.strip_prefix(full_indentation) {
                Some(rest) if !line.trim().is_empty() => {
                    dedented.push_str(else_indentation);
                    dedented.push_str(rest);
                }
                // Blank lines, and lines indented less than the body (such as
                // comments), stay as they are.
                _ => dedented.push_str(line),
            }
        }
        Some(Fix {
            description,
            safety,
            edits: vec![Edit {
                span: Span::from_byte_range(source_code, else_line_start, body_end),
                replacement: dedented,
            }],
        })
    }

    fn contains_multiline_string(node: &Node) -> bool {
        if node.kind() == "string" && node.start_position().row != node.end_position().row {
            return true;
        }
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                if Self::contains_multiline_string(&cursor.node()) {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        false
    }

    /// True when a direct statement of `body_node` declares a variable or a
    /// constant.
    fn declares_names(body_node: &Node) -> bool {
        let mut cursor = body_node.walk();
        if cursor.goto_first_child() {
            loop {
                let kind = cursor.node().kind();
                if kind == "variable_statement" || kind == "const_statement" {
                    return true;
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
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

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
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
                    // The rule reports every `elif` after an `if` that
                    // returns, but `elif` → `if` only keeps behavior when
                    // every earlier branch returns.
                    let mut fixes = Vec::new();
                    if all_branches_return && let Some(fix) = Self::elif_fix(&child_node) {
                        fixes.push(fix);
                    }
                    diagnostics.push(Diagnostic {
                        check: None,
                        rule: "no-else-return".to_string(),
                        severity: Severity::Warning,
                        message: "Unnecessary 'elif' after 'if' block that ends with 'return'. Use 'if' instead".to_string(),
                        help: Some(
                            "Replace 'elif' with 'if': the block before it always returns."
                                .to_string(),
                        ),
                        span: Self::keyword_span(&child_node),
                        fixes,
                    });
                }

                if let Some(elif_body) = child_node.child_by_field_name("body")
                    && !Self::body_ends_with_return(&elif_body)
                {
                    all_branches_return = false;
                }
            } else if child_kind == GDScriptNodeKind::ElseStatement && all_branches_return {
                let mut fixes = Vec::new();
                if let Some(fix) = Self::else_fix(source_code, &child_node) {
                    fixes.push(fix);
                }
                diagnostics.push(Diagnostic {
                    check: None,
                    rule: "no-else-return".to_string(),
                    severity: Severity::Warning,
                    message: "Unnecessary 'else' after 'if'/'elif' blocks that end with 'return'"
                        .to_string(),
                    help: Some(
                        "Remove the 'else' line and dedent its body: the blocks before it always return."
                            .to_string(),
                    ),
                    span: Self::keyword_span(&child_node),
                    fixes,
                });
            }
            if !stmt_cursor.goto_next_sibling() {
                break;
            }
        }
    }
}
