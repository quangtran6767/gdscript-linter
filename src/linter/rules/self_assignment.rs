//! `x = x` on a local variable or parameter. Member self-assignment is left
//! alone: `property = property` is a Godot idiom to run a setter again.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, FileContext, Severity, Span};
use crate::semantic::{ScopeKind, SymbolKind};

pub struct SelfAssignmentRule;
impl Rule for SelfAssignmentRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let Some(model) = context.semantic else {
            return;
        };
        let mut cursor = context.tree.walk();
        loop {
            let node = cursor.node();
            if node.kind() == "assignment" {
                if let (Some(left), Some(right)) = (
                    node.child_by_field_name("left"),
                    node.child_by_field_name("right"),
                ) {
                    let name = &context.source[left.byte_range()];
                    let is_local = left.kind() == "identifier"
                        && right.kind() == "identifier"
                        && name == &context.source[right.byte_range()]
                        && model
                            .lookup(model.scope_at(node.start_byte()), name)
                            .and_then(|id| model.symbol(id))
                            .is_some_and(|symbol| {
                                matches!(
                                    symbol.kind,
                                    SymbolKind::Variable
                                        | SymbolKind::Parameter
                                        | SymbolKind::LoopVariable
                                ) && !matches!(
                                    model.scopes()[symbol.scope.0 as usize].kind,
                                    ScopeKind::Class | ScopeKind::InnerClass
                                )
                            });
                    if is_local {
                        diagnostics.push(Diagnostic {
                            rule: "self-assignment".to_string(),
                            severity: Severity::Warning,
                            message: format!("'{}' is assigned to itself.", name),
                            help: Some(
                                "This line has no effect. Assign the value you meant, or remove the line."
                                    .to_string(),
                            ),
                            span: Span::from_node(&node),
                            fixes: Vec::new(),
                        });
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
