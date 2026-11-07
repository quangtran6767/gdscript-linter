use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::linter::lib::get_node_text;
use crate::linter::rules::Rule;
use crate::node_kind::GDScriptNodeKind;
use std::collections::HashMap;
use tree_sitter::Node;

pub struct DuplicatedLoadRule {
    /// Path string literal (with quotes) to the spans of every load of it.
    pub load_paths: HashMap<String, Vec<Span>>,
}

impl Rule for DuplicatedLoadRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Call]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, _diagnostics: &mut Vec<Diagnostic>) {
        let Some(function_node) = node.child(0) else {
            return;
        };
        let function_name = get_node_text(&function_node, source_code);
        if function_name != "load" && function_name != "preload" {
            return;
        }
        let Some(args_node) = node.child_by_field_name("arguments") else {
            return;
        };
        let mut args_cursor = args_node.walk();
        if !args_cursor.goto_first_child() {
            return;
        }
        loop {
            let arg_node = args_cursor.node();
            if GDScriptNodeKind::get_kind_from_ast_node(arg_node) == GDScriptNodeKind::String {
                let path = get_node_text(&arg_node, source_code);
                self.load_paths
                    .entry(path.to_string())
                    .or_default()
                    .push(Span::from_node(&arg_node));
            }
            if !args_cursor.goto_next_sibling() {
                break;
            }
        }
    }

    fn finalize(&mut self, _source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        for (path, spans) in &self.load_paths {
            if spans.len() < 2 {
                continue;
            }
            for span in spans {
                diagnostics.push(Diagnostic {
                    rule: "duplicated-load".to_string(),
                    severity: Severity::Warning,
                    message: format!(
                        "Duplicated load of '{}'. Consider extracting to a constant.",
                        path
                    ),
                    help: Some(format!(
                        "Load it once at the top of the script, for example `const Resource = preload({})`, and reuse the constant.",
                        path
                    )),
                    span: *span,
                    fixes: Vec::new(),
                });
            }
        }
        self.load_paths.clear();
    }
}
