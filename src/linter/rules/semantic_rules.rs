//! Rules using the shared lexical model. No type inference or engine-member
//! assumptions are made; only local/private declarations are considered.
use super::{Rule, fix_helpers::remove_lines_edit, godot_virtuals::VIRTUAL_METHODS};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity};
use crate::semantic::{ReferenceKind, ScopeKind, Symbol, SymbolKind};

pub struct SemanticRule {
    pub id: &'static str,
}

impl Rule for SemanticRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        // In a project, scene connections can use private functions: that
        // rule waits for the project analysis.
        if self.id == "unused-private-function" && context.project.is_some() {
            return;
        }
        self.report(context, None, diagnostics);
    }

    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &crate::project::ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if self.id == "unused-private-function" && context.project.is_some() {
            self.report(context, Some(analysis), diagnostics);
        }
    }
}

impl SemanticRule {
    fn report(
        &self,
        context: &FileContext,
        analysis: Option<&crate::project::ProjectAnalysis>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(model) = context.semantic else {
            return;
        };
        if context.tree.root_node().has_error() {
            return;
        }
        for symbol in model.symbols() {
            let scope = &model.scopes()[symbol.scope.0 as usize];
            let local = !matches!(scope.kind, ScopeKind::Class | ScopeKind::InnerClass);
            let references = model.references_to(symbol.id);
            let read = references
                .iter()
                .any(|reference| reference.kind != ReferenceKind::Write);
            let written = references
                .iter()
                .any(|reference| reference.kind == ReferenceKind::Write);
            let exempt = symbol.name.starts_with('_');
            let report = match self.id {
                "unused-variable" => {
                    local
                        && !exempt
                        && matches!(symbol.kind, SymbolKind::Variable | SymbolKind::LoopVariable)
                        && references.is_empty()
                }
                "assigned-but-never-read" => {
                    local
                        && !exempt
                        && matches!(symbol.kind, SymbolKind::Variable | SymbolKind::LoopVariable)
                        && written
                        && !read
                }
                "unused-private-function" => {
                    symbol.kind == SymbolKind::Function
                        && exempt
                        && references.is_empty()
                        && VIRTUAL_METHODS
                            .binary_search(&symbol.name.as_str())
                            .is_err()
                        && !context.source.contains(&format!("\"{}\"", symbol.name))
                        && !context.source.contains(&format!("'{}'", symbol.name))
                        && !analysis.is_some_and(|analysis| {
                            analysis.connected_methods.contains(&symbol.name)
                        })
                }
                "shadowed-variable" => {
                    local
                        && !exempt
                        && matches!(
                            symbol.kind,
                            SymbolKind::Variable | SymbolKind::Parameter | SymbolKind::LoopVariable
                        )
                        && scope
                            .parent
                            .and_then(|parent| model.lookup(parent, &symbol.name))
                            .is_some()
                }
                "unused-local-signal" => {
                    symbol.kind == SymbolKind::Signal
                        && references.is_empty()
                        && !signal_used_by_string(context, &symbol.name)
                }
                _ => false,
            };
            if !report {
                continue;
            }
            let (message, help) = match self.id {
                "unused-variable" => (
                    format!("Variable '{}' is never used.", symbol.name),
                    "Use the value, remove the declaration, or prefix an intentional unused binding with `_`.",
                ),
                "assigned-but-never-read" => (
                    format!("Variable '{}' is assigned but never read.", symbol.name),
                    "Read the assigned value or remove the unnecessary assignment.",
                ),
                "unused-private-function" => (
                    format!(
                        "Private function '{}' is never referenced in this file.",
                        symbol.name
                    ),
                    "Remove it after checking scene connections, strings, and external callbacks, or document the intentional callback with a reasoned ignore.",
                ),
                "shadowed-variable" => (
                    format!("Variable '{}' shadows an outer declaration.", symbol.name),
                    "Rename the local binding to make the intended declaration clear.",
                ),
                _ => (
                    format!("Signal '{}' is never used in this file.", symbol.name),
                    "Emit or connect the signal, or document intentional external/scene use with a reasoned ignore.",
                ),
            };
            let mut fixes = Vec::new();
            if self.id == "unused-variable" && symbol.kind == SymbolKind::Variable {
                if let Some(node) = declaration_node(context, symbol) {
                    if node.kind() == "variable_statement" && !has_calls(node) {
                        // Keep an otherwise-empty block parseable. An unsafe
                        // removal must still pass the normal fix-engine guard.
                        let only_statement = node.parent().is_some_and(|body| {
                            if body.kind() != "body" {
                                return false;
                            }
                            let mut count = 0;
                            for index in 0..body.named_child_count() {
                                if let Some(child) = body.named_child(index as u32) {
                                    if child.kind() != "comment" {
                                        count += 1;
                                    }
                                }
                            }
                            count == 1
                        });
                        let edit = if only_statement {
                            Some(Edit {
                                span: symbol.declaration_span,
                                replacement: "pass".to_string(),
                            })
                        } else {
                            remove_lines_edit(
                                context.source,
                                symbol.declaration_span.start_byte,
                                symbol.declaration_span.end_byte,
                            )
                        };
                        if let Some(edit) = edit {
                            fixes.push(Fix {
                                description: format!("Remove unused variable '{}'", symbol.name),
                                safety: FixSafety::Unsafe,
                                edits: vec![edit],
                            });
                        }
                    }
                }
            }
            if self.id == "unused-private-function" {
                if let Some(edit) = remove_lines_edit(
                    context.source,
                    symbol.declaration_span.start_byte,
                    symbol.declaration_span.end_byte,
                ) {
                    fixes.push(Fix {
                        description: format!(
                            "Remove private function '{}' after checking external callbacks",
                            symbol.name
                        ),
                        safety: FixSafety::Suggestion,
                        edits: vec![edit],
                    });
                }
            }
            diagnostics.push(Diagnostic {
                check: None,
                rule: self.id.to_string(),
                severity: Severity::Warning,
                message,
                help: Some(help.to_string()),
                span: symbol.name_span,
                fixes,
            });
        }
    }
}

fn declaration_node<'a>(
    context: &'a FileContext,
    symbol: &Symbol,
) -> Option<tree_sitter::Node<'a>> {
    context.tree.root_node().descendant_for_byte_range(
        symbol.declaration_span.start_byte,
        symbol.declaration_span.end_byte,
    )
}

fn has_calls(node: tree_sitter::Node) -> bool {
    let mut cursor = node.walk();
    loop {
        if matches!(cursor.node().kind(), "call" | "attribute_call" | "get_node") {
            return true;
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return false;
            }
        }
    }
}

fn signal_used_by_string(context: &FileContext, name: &str) -> bool {
    let mut cursor = context.tree.walk();
    loop {
        let node = cursor.node();
        if node.kind() == "call"
            || (node.kind() == "attribute_call"
                && node
                    .parent()
                    .and_then(|parent| parent.named_child(0))
                    .is_some_and(|receiver| &context.source[receiver.byte_range()] == "self"))
        {
            if let Some(callee) = node.named_child(0) {
                let text = &context.source[callee.byte_range()];
                if matches!(
                    text,
                    "emit_signal" | "connect" | "disconnect" | "is_connected"
                ) {
                    if let Some(argument) = node
                        .child_by_field_name("arguments")
                        .and_then(|arguments| arguments.named_child(0))
                    {
                        if argument.kind() == "string"
                            && context.source[argument.byte_range()].trim_matches(['\'', '"'])
                                == name
                        {
                            return true;
                        }
                    }
                }
            }
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return false;
            }
        }
    }
}
