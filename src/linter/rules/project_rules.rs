//! Cross-file rules, using the run's shared index and conservative signal use.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, FileContext, Severity, Span};
use crate::project::{MemberLookup, ProjectAnalysis};
use crate::semantic::SymbolKind;

pub struct UnusedSignalRule;
impl Rule for UnusedSignalRule {
    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if analysis.dynamic_signal_usage || context.tree.root_node().has_error() {
            return;
        }
        let local = ProjectAnalysis::from_tree(context.tree, context.source);
        if local.dynamic_signal_usage {
            return;
        }
        let Some(semantic) = context.semantic else {
            return;
        };
        for symbol in semantic.symbols() {
            if symbol.kind == SymbolKind::Signal
                && !analysis.used_signals.contains(&symbol.name)
                && !local.used_signals.contains(&symbol.name)
            {
                diagnostics.push(Diagnostic {
                    rule: "unused-signal".to_string(),
                    severity: Severity::Warning,
                    message: format!("Signal '{}' is never emitted or connected in this project.", symbol.name),
                    help: Some("Emit/connect the signal, remove it after review, or add a reasoned ignore for usage outside this project.".to_string()),
                    span: symbol.name_span,
                    fixes: Vec::new(),
                });
            }
        }
    }
}

pub struct UnknownMemberRule;
impl Rule for UnknownMemberRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let (Some(semantic), Some(project)) = (context.semantic, context.project) else {
            return;
        };
        if context.tree.root_node().has_error() {
            return;
        }
        // Only explicitly typed project receivers are checkable. Index their
        // resolved reference positions once instead of rescanning all uses
        // for every attribute (quadratic on large scripts).
        let mut receivers = std::collections::HashMap::new();
        for symbol in semantic.symbols() {
            if symbol
                .declared_type
                .as_ref()
                .is_some_and(|name| project.class_names.contains_key(name))
            {
                for reference in semantic.references_to(symbol.id) {
                    receivers.insert(reference.span.start_byte, symbol.id);
                }
            }
        }
        if receivers.is_empty() {
            return;
        }
        let mut cursor = context.tree.walk();
        loop {
            let node = cursor.node();
            if node.kind() == "attribute" {
                if let (Some(receiver), Some(member)) = (node.named_child(0), node.named_child(1)) {
                    if receiver.kind() == "identifier" {
                        if let Some(symbol) = receivers
                            .get(&receiver.start_byte())
                            .and_then(|id| semantic.symbol(*id))
                        {
                            if let Some(class) = &symbol.declared_type {
                                let member = if matches!(
                                    member.kind(),
                                    "attribute_call" | "attribute_subscript"
                                ) {
                                    member.named_child(0)
                                } else {
                                    Some(member)
                                };
                                if let Some(member) = member {
                                    let name = &context.source[member.byte_range()];
                                    if project.has_member(class, name) == MemberLookup::No {
                                        diagnostics.push(Diagnostic {
                                            rule: "unknown-member-on-project-class".to_string(),
                                            severity: Severity::Warning,
                                            message: format!("Project class '{}' has no declared member '{}'.", class, name),
                                            help: Some(format!("Check '{}' for a spelling mistake or declare the member on {}. Inferred and engine types are not checked.", name, class)),
                                            span: Span::from_node(&member),
                                            fixes: Vec::new(),
                                        });
                                    }
                                }
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
                    return;
                }
            }
        }
    }
}
