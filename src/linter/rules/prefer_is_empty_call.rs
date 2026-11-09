//! `items.size() > 0` and `len(items) == 0` become `not items.is_empty()`
//! and `items.is_empty()`, when `items` is an identifier whose type is
//! known (declared, or a literal initializer) to be an Array, Dictionary,
//! packed array or String. Other receivers stay silent: a project class may
//! have its own `size()`.

use super::Rule;
use super::receiver_types::{
    contains_kind, identifier_type, is_sized_collection, nodes_of_kind, strip_parentheses, text,
};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use tree_sitter::Node;

pub const RULE_ID: &str = "prefer-is-empty-call";

pub struct PreferIsEmptyCallRule;

impl Rule for PreferIsEmptyCallRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let Some(model) = context.semantic else {
            return;
        };
        for comparison in nodes_of_kind(context.tree, "binary_operator") {
            let (Some(left), Some(op), Some(right)) = (
                comparison.child_by_field_name("left"),
                comparison.child_by_field_name("op"),
                comparison.child_by_field_name("right"),
            ) else {
                continue;
            };
            // Put the size on the left: `0 < a.size()` reads as `a.size() > 0`.
            let op_text = text(context.source, op);
            let (size_side, number_side, op) = if is_number(context.source, left) {
                let Some(flipped) = flip(op_text) else {
                    continue;
                };
                (right, left, flipped)
            } else {
                (left, right, op_text)
            };
            if !is_number(context.source, number_side) {
                continue;
            }
            let number = text(context.source, strip_parentheses(number_side));
            let is_empty_check = match (op, number) {
                ("==", "0") | ("<=", "0") | ("<", "1") => true,
                ("!=", "0") | (">", "0") | (">=", "1") => false,
                _ => continue,
            };
            let Some(receiver) = size_receiver(context.source, strip_parentheses(size_side)) else {
                continue;
            };
            let Some(receiver_type) = identifier_type(context, model, receiver) else {
                continue;
            };
            if !accepts(&receiver_type, size_side, context.source) {
                continue;
            }
            if comparison.has_error() || contains_kind(comparison, &["comment"]) {
                continue;
            }
            let receiver_text = text(context.source, receiver);
            let mut replacement = if is_empty_check {
                format!("{}.is_empty()", receiver_text)
            } else {
                format!("not {}.is_empty()", receiver_text)
            };
            if !is_empty_check && needs_parentheses(context.source, comparison) {
                replacement = format!("({})", replacement);
            }
            diagnostics.push(Diagnostic {
                rule: RULE_ID.to_string(),
                severity: Severity::Info,
                message: format!(
                    "Use `{}` to check whether `{}` is empty.",
                    replacement, receiver_text
                ),
                help: Some(format!(
                    "Replace `{}` with `{}`.",
                    text(context.source, comparison),
                    replacement
                )),
                span: Span::from_node(&comparison),
                fixes: vec![Fix {
                    description: format!("Replace with `{}`", replacement),
                    safety: FixSafety::Safe,
                    edits: vec![Edit {
                        span: Span::from_node(&comparison),
                        replacement,
                    }],
                }],
            });
        }
    }
}

/// The comparison with its operands swapped: `0 < x` is `x > 0`.
fn flip(op: &str) -> Option<&'static str> {
    match op {
        "==" => Some("=="),
        "!=" => Some("!="),
        "<" => Some(">"),
        ">" => Some("<"),
        "<=" => Some(">="),
        ">=" => Some("<="),
        _ => None,
    }
}

fn is_number(source: &str, node: Node) -> bool {
    strip_parentheses(node).kind() == "integer" && !text(source, node).starts_with('-')
}

/// The receiver identifier of `x.size()`, `x.length()` or `len(x)`.
fn size_receiver<'tree>(source: &str, node: Node<'tree>) -> Option<Node<'tree>> {
    match node.kind() {
        "attribute" => {
            if node.named_child_count() != 2 {
                return None;
            }
            let receiver = node.named_child(0)?;
            let call = node.named_child(1)?;
            if receiver.kind() != "identifier" || call.kind() != "attribute_call" {
                return None;
            }
            let method = text(source, call.named_child(0)?);
            let arguments = call.child_by_field_name("arguments")?;
            if !matches!(method, "size" | "length") || arguments.named_child_count() != 0 {
                return None;
            }
            Some(receiver)
        }
        "call" => {
            let function = node.named_child(0)?;
            let arguments = node.child_by_field_name("arguments")?;
            if function.kind() != "identifier"
                || text(source, function) != "len"
                || arguments.named_child_count() != 1
            {
                return None;
            }
            let argument = arguments.named_child(0)?;
            (argument.kind() == "identifier").then_some(argument)
        }
        _ => None,
    }
}

/// `size()` (and `len`) for collections, `length()` (and `len`) for String.
fn accepts(receiver_type: &str, size_side: Node, source: &str) -> bool {
    let size_text = text(source, strip_parentheses(size_side));
    let is_len = size_text.starts_with("len");
    if receiver_type == "String" {
        return is_len || size_text.ends_with(".length()");
    }
    is_sized_collection(receiver_type) && (is_len || size_text.ends_with(".size()"))
}

/// `not x.is_empty()` binds looser than comparisons and arithmetic, so it
/// needs parentheses inside them. Inside `and`/`or`, as a condition or a
/// statement, it doesn't.
fn needs_parentheses(source: &str, comparison: Node) -> bool {
    let Some(parent) = comparison.parent() else {
        return false;
    };
    match parent.kind() {
        "binary_operator" => {
            let op = parent
                .child_by_field_name("op")
                .map(|op| text(source, op))
                .unwrap_or("");
            !matches!(op, "and" | "or" | "&&" | "||")
        }
        "unary_operator" => {
            let op = parent.child(0).map(|op| text(source, op)).unwrap_or("");
            !matches!(op, "not" | "!")
        }
        _ => false,
    }
}
