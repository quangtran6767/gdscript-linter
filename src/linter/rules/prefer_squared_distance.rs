//! `a.distance_to(b) < radius` becomes `a.distance_squared_to(b) < radius *
//! radius`, and `v.length() > speed` becomes `v.length_squared() > speed *
//! speed`, which skips a square root. Only for receivers known to be float
//! vectors: identifiers declared `Vector2`/`Vector3`/`Vector4` (or set to a
//! `Vector2(...)` literal), `position` and `global_position` (inherited or
//! on another object), and sums/differences of those. The threshold must be
//! free of calls so squaring it evaluates nothing twice.
//!
//! The fix is Unsafe: a negative threshold, or float rounding exactly on the
//! boundary, can change the result.

use super::Rule;
use super::receiver_types::{
    contains_kind, identifier_type, is_float_vector, nodes_of_kind, strip_parentheses, text,
};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use crate::semantic::SemanticModel;
use tree_sitter::Node;

pub const RULE_ID: &str = "prefer-squared-distance";

/// Integer thresholds above this aren't squared, to stay far from overflow.
const MAX_SQUARED_INTEGER: i64 = 1_000_000;

pub struct PreferSquaredDistanceRule;

impl Rule for PreferSquaredDistanceRule {
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
            if !matches!(text(context.source, op), "<" | "<=" | ">" | ">=") {
                continue;
            }
            let (measure, threshold) = if let Some(measure) =
                vector_measure(context, model, strip_parentheses(left))
            {
                (measure, right)
            } else if let Some(measure) = vector_measure(context, model, strip_parentheses(right)) {
                (measure, left)
            } else {
                continue;
            };
            let Some(squared_threshold) = squared(context.source, strip_parentheses(threshold))
            else {
                continue;
            };
            if comparison.has_error() || contains_kind(comparison, &["comment"]) {
                continue;
            }
            let method_text = text(context.source, measure.method);
            let squared_method = if method_text == "distance_to" {
                "distance_squared_to"
            } else {
                "length_squared"
            };
            diagnostics.push(Diagnostic {
                rule: RULE_ID.to_string(),
                severity: Severity::Warning,
                message: format!(
                    "Compare `{}()` with the squared threshold instead of `{}()`.",
                    squared_method, method_text
                ),
                help: Some(format!(
                    "Use `{}` and compare against `{}`: it skips a square root. Keep `{}` when you need the real distance.",
                    squared_method, squared_threshold, method_text
                )),
                span: Span::from_node(&comparison),
                fixes: vec![Fix {
                    description: format!(
                        "Use `{}` and `{}`",
                        squared_method, squared_threshold
                    ),
                    safety: FixSafety::Unsafe,
                    edits: vec![
                        Edit {
                            span: Span::from_node(&measure.method),
                            replacement: squared_method.to_string(),
                        },
                        Edit {
                            span: Span::from_node(&threshold),
                            replacement: squared_threshold,
                        },
                    ],
                }],
            });
        }
    }
}

struct Measure<'tree> {
    /// The `distance_to` or `length` identifier.
    method: Node<'tree>,
}

/// `<vector>.distance_to(x)` or `<vector>.length()`.
fn vector_measure<'tree>(
    context: &FileContext,
    model: &SemanticModel,
    node: Node<'tree>,
) -> Option<Measure<'tree>> {
    if node.kind() != "attribute" {
        return None;
    }
    let count = u32::try_from(node.named_child_count()).ok()?;
    if count < 2 {
        return None;
    }
    let call = node.named_child(count - 1)?;
    if call.kind() != "attribute_call" {
        return None;
    }
    let method = call.named_child(0)?;
    let arguments = call.child_by_field_name("arguments")?;
    let expected_arguments = match text(context.source, method) {
        "distance_to" => 1,
        "length" => 0,
        _ => return None,
    };
    if arguments.named_child_count() != expected_arguments {
        return None;
    }
    // The receiver: every child before the call (`a`, or `enemy.global_position`).
    let is_vector = if count == 2 {
        is_vector_expression(context, model, node.named_child(0)?)
    } else {
        is_position_chain(context.source, node, count - 1)
    };
    is_vector.then_some(Measure { method })
}

/// `a.b.position` style chains: the children before `end` are plain
/// identifiers and the last one is `position` or `global_position`.
fn is_position_chain(source: &str, attribute: Node, end: u32) -> bool {
    if end == 0 {
        return false;
    }
    for index in 0..end {
        let Some(child) = attribute.named_child(index) else {
            return false;
        };
        if child.kind() != "identifier" {
            return false;
        }
    }
    let Some(last) = attribute.named_child(end - 1) else {
        return false;
    };
    matches!(text(source, last), "position" | "global_position")
}

fn is_vector_expression(context: &FileContext, model: &SemanticModel, node: Node) -> bool {
    let node = strip_parentheses(node);
    match node.kind() {
        "identifier" => {
            if let Some(type_text) = identifier_type(context, model, node) {
                return is_float_vector(&type_text);
            }
            // Inherited `position` / `global_position` of Node2D, Node3D or
            // Control, when the script doesn't declare its own.
            let name = text(context.source, node);
            matches!(name, "position" | "global_position")
                && model
                    .lookup(model.scope_at(node.start_byte()), name)
                    .is_none()
        }
        "attribute" => {
            let Ok(count) = u32::try_from(node.named_child_count()) else {
                return false;
            };
            count >= 2 && is_position_chain(context.source, node, count)
        }
        "binary_operator" => {
            let op = node
                .child_by_field_name("op")
                .map(|op| text(context.source, op))
                .unwrap_or("");
            let (Some(left), Some(right)) = (
                node.child_by_field_name("left"),
                node.child_by_field_name("right"),
            ) else {
                return false;
            };
            matches!(op, "+" | "-")
                && is_vector_expression(context, model, left)
                && is_vector_expression(context, model, right)
        }
        _ => false,
    }
}

/// The squared threshold text, when the threshold has no calls and isn't a
/// non-positive literal: `40` gives `1600`, `40.0` gives `40.0 * 40.0`,
/// `radius` gives `radius * radius` and `r + 2.0` gives `(r + 2.0) * (r + 2.0)`.
fn squared(source: &str, threshold: Node) -> Option<String> {
    if !is_pure(source, threshold) {
        return None;
    }
    let threshold_text = text(source, threshold);
    match threshold.kind() {
        "integer" => {
            let value: i64 = threshold_text.replace('_', "").parse().ok()?;
            if value <= 0 || value > MAX_SQUARED_INTEGER {
                return None;
            }
            Some((value * value).to_string())
        }
        "float" => {
            let value: f64 = threshold_text.replace('_', "").parse().ok()?;
            if value <= 0.0 || !value.is_finite() {
                return None;
            }
            Some(format!("{} * {}", threshold_text, threshold_text))
        }
        "identifier" | "attribute" => Some(format!("{} * {}", threshold_text, threshold_text)),
        _ => Some(format!("({}) * ({})", threshold_text, threshold_text)),
    }
}

/// Identifiers, attribute reads, numbers and arithmetic on them: evaluating
/// it twice gives the same value and has no side effects.
fn is_pure(source: &str, node: Node) -> bool {
    match node.kind() {
        "identifier" | "integer" | "float" => true,
        "parenthesized_expression" => node
            .named_child(0)
            .is_some_and(|inner| is_pure(source, inner)),
        "attribute" => {
            let mut cursor = node.walk();
            let mut children = node.named_children(&mut cursor);
            children.all(|child| child.kind() == "identifier")
        }
        "binary_operator" => {
            let op = node
                .child_by_field_name("op")
                .map(|op| text(source, op))
                .unwrap_or("");
            matches!(op, "+" | "-" | "*" | "/")
                && node
                    .child_by_field_name("left")
                    .is_some_and(|left| is_pure(source, left))
                && node
                    .child_by_field_name("right")
                    .is_some_and(|right| is_pure(source, right))
        }
        _ => false,
    }
}
