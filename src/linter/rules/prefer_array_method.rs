//! Loops over an Array that rebuild `any`, `all`, `filter`, `map` or a copy.
//! The source must be an identifier known to be an `Array`/`Array[T]`.
//!
//! - `for e in items: if P: return true` + `return false` →
//!   `return items.any(func(e): return P)` (`all` with `not` when reversed).
//! - `var result: Array[T] = []` right before `for e in items:` whose only
//!   statement is `result.append(X)` → `result.assign(items.map(func(e):
//!   return X))`, or `result.assign(items)` when X is `e`; with
//!   `if P: result.append(e)` → `result.assign(items.filter(func(e):
//!   return P))`.
//!
//! The fix is Unsafe: the lambda captures local variables by value and the
//! code reads differently in a debugger. Loops with comments, `await`,
//! assignments, nested functions, or expressions over several lines aren't
//! reported.

use super::Rule;
use super::receiver_types::{
    array_element_type, contains_kind, identifier_type, is_array, nodes_of_kind, strip_parentheses,
    text,
};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use crate::semantic::SemanticModel;
use tree_sitter::Node;

pub const RULE_ID: &str = "prefer-array-method";

/// Node kinds that make a loop body unsafe to move into a lambda.
const BLOCKING_KINDS: &[&str] = &[
    "comment",
    "await_expression",
    "assignment",
    "augmented_assignment",
    "lambda",
    "function_definition",
    "for_statement",
    "while_statement",
    "match_statement",
    "break_statement",
    "continue_statement",
    "variable_statement",
    "ERROR",
];

pub struct PreferArrayMethodRule;

impl Rule for PreferArrayMethodRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let Some(model) = context.semantic else {
            return;
        };
        for loop_node in nodes_of_kind(context.tree, "for_statement") {
            let Some(found) = check_loop(context, model, loop_node) else {
                continue;
            };
            diagnostics.push(Diagnostic {
                rule: RULE_ID.to_string(),
                severity: Severity::Info,
                message: format!("This loop can be `{}`.", found.method),
                help: Some(format!(
                    "Replace the loop with `{}`.",
                    found.replacement.trim_start()
                )),
                span: Span::from_byte_range(
                    context.source,
                    loop_node.start_byte(),
                    loop_node.start_byte() + 3,
                ),
                fixes: vec![Fix {
                    description: format!("Use `{}`", found.method),
                    safety: FixSafety::Unsafe,
                    edits: vec![Edit {
                        span: Span::from_byte_range(context.source, found.start, found.end),
                        replacement: found.replacement,
                    }],
                }],
            });
        }
    }
}

struct Found {
    method: &'static str,
    start: usize,
    end: usize,
    replacement: String,
}

fn check_loop(context: &FileContext, model: &SemanticModel, loop_node: Node) -> Option<Found> {
    let source = context.source;
    let element = loop_node.child_by_field_name("left")?;
    let items = loop_node.child_by_field_name("right")?;
    let body = loop_node.child_by_field_name("body")?;
    if element.kind() != "identifier" || items.kind() != "identifier" {
        return None;
    }
    let items_type = identifier_type(context, model, items)?;
    if !is_array(&items_type) {
        return None;
    }
    if body.named_child_count() != 1 {
        return None;
    }
    let statement = body.named_child(0)?;
    let element_name = text(source, element);
    let parameter = match loop_node.child_by_field_name("type") {
        Some(element_type) => format!("{}: {}", element_name, text(source, element_type)),
        None => match array_element_type(&items_type) {
            Some(element_type) => format!("{}: {}", element_name, element_type),
            None => element_name.to_string(),
        },
    };
    let items_name = text(source, items);

    if let Some(found) = boolean_scan(source, loop_node, statement, items_name, &parameter) {
        return Some(found);
    }
    accumulator(
        source,
        loop_node,
        statement,
        items_name,
        element_name,
        &parameter,
    )
}

/// `for e in items: if P: return true` followed by `return false`.
fn boolean_scan(
    source: &str,
    loop_node: Node,
    statement: Node,
    items_name: &str,
    parameter: &str,
) -> Option<Found> {
    let (condition, value) = single_if(source, statement)?;
    if value.kind() != "return_statement" {
        return None;
    }
    let returned = text(source, value.named_child(0)?);
    let next = loop_node.next_named_sibling()?;
    if next.kind() != "return_statement" {
        return None;
    }
    let after = text(source, next.named_child(0)?);
    let condition_text = text(source, condition);
    let (method, lambda_body) = match (returned, after) {
        ("true", "false") => ("any", condition_text.to_string()),
        ("false", "true") => ("all", negate(source, condition)),
        _ => return None,
    };
    Some(Found {
        method,
        start: loop_node.start_byte(),
        end: next.end_byte(),
        replacement: format!(
            "return {}.{}(func({}): return {})",
            items_name, method, parameter, lambda_body
        ),
    })
}

/// `var result := []` right before a loop that appends to it.
fn accumulator(
    source: &str,
    loop_node: Node,
    statement: Node,
    items_name: &str,
    element_name: &str,
    parameter: &str,
) -> Option<Found> {
    let declaration = loop_node.prev_named_sibling()?;
    if declaration.kind() != "variable_statement" {
        return None;
    }
    let result = declaration.child_by_field_name("name")?;
    let value = declaration.child_by_field_name("value")?;
    if value.kind() != "array" || value.named_child_count() != 0 {
        return None;
    }
    if let Some(declared) = declaration.child_by_field_name("type")
        && declared.kind() == "type"
        && !is_array(text(source, declared))
    {
        return None;
    }
    let result_name = text(source, result);
    if result_name == items_name {
        return None;
    }

    let (method, call) = if statement.kind() == "expression_statement" {
        let appended = appended_value(source, statement, result_name)?;
        let appended_text = text(source, appended);
        if appended_text == element_name {
            ("assign", items_name.to_string())
        } else {
            if mentions(source, appended, result_name) {
                return None;
            }
            (
                "map",
                format!(
                    "{}.map(func({}): return {})",
                    items_name, parameter, appended_text
                ),
            )
        }
    } else {
        let (condition, appended_statement) = single_if(source, statement)?;
        let appended = appended_value(source, appended_statement, result_name)?;
        if text(source, appended) != element_name || mentions(source, condition, result_name) {
            return None;
        }
        (
            "filter",
            format!(
                "{}.filter(func({}): return {})",
                items_name,
                parameter,
                text(source, condition)
            ),
        )
    };
    Some(Found {
        method,
        start: loop_node.start_byte(),
        end: loop_node.end_byte(),
        replacement: format!("{}.assign({})", result_name, call),
    })
}

/// `if P: <one statement>` with no `elif`/`else`: (P, the statement).
fn single_if<'tree>(source: &str, statement: Node<'tree>) -> Option<(Node<'tree>, Node<'tree>)> {
    if statement.kind() != "if_statement" || contains_kind(statement, BLOCKING_KINDS) {
        return None;
    }
    let condition = statement.child_by_field_name("condition")?;
    let body = statement.child_by_field_name("body")?;
    if body.named_child_count() != 1 || statement.named_child_count() != 2 {
        return None;
    }
    if text(source, condition).contains('\n') {
        return None;
    }
    Some((condition, body.named_child(0)?))
}

/// X in the statement `result.append(X)`, when it fits on one line.
fn appended_value<'tree>(
    source: &str,
    statement: Node<'tree>,
    result_name: &str,
) -> Option<Node<'tree>> {
    if statement.kind() != "expression_statement" || contains_kind(statement, BLOCKING_KINDS) {
        return None;
    }
    let attribute = statement.named_child(0)?;
    if attribute.kind() != "attribute" || attribute.named_child_count() != 2 {
        return None;
    }
    let receiver = attribute.named_child(0)?;
    let call = attribute.named_child(1)?;
    if text(source, receiver) != result_name || call.kind() != "attribute_call" {
        return None;
    }
    if text(source, call.named_child(0)?) != "append" {
        return None;
    }
    let arguments = call.child_by_field_name("arguments")?;
    if arguments.named_child_count() != 1 {
        return None;
    }
    let value = arguments.named_child(0)?;
    if text(source, value).contains('\n') {
        return None;
    }
    Some(value)
}

/// True when the identifier `name` appears anywhere in `node`.
fn mentions(source: &str, node: Node, name: &str) -> bool {
    if node.kind() == "identifier" && text(source, node) == name {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if mentions(source, child, name) {
            return true;
        }
    }
    false
}

/// How loosely an expression binds, to know when it needs parentheses.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    Or,
    And,
    Tight,
}

/// `not P`, simplified: `not X` gives `X`, `A or B` gives `not A and not B`,
/// `A and B` gives `not A or not B`, `a == b` gives `a != b` (and back).
/// Other comparisons keep `not (...)`: `not (a < b)` isn't `a >= b` for NaN.
fn negate(source: &str, condition: Node) -> String {
    negated(source, condition).0
}

fn negated(source: &str, condition: Node) -> (String, Binding) {
    let inner = strip_parentheses(condition);
    match inner.kind() {
        "unary_operator" => {
            if let Some(op) = inner.child(0)
                && matches!(text(source, op), "not" | "!")
                && let Some(operand) = inner.named_child(0)
            {
                let operand_inner = strip_parentheses(operand);
                return (
                    text(source, operand_inner).to_string(),
                    binding(source, operand_inner),
                );
            }
        }
        "binary_operator" => {
            let op = inner
                .child_by_field_name("op")
                .map(|op| text(source, op))
                .unwrap_or("");
            let (Some(left), Some(right)) = (
                inner.child_by_field_name("left"),
                inner.child_by_field_name("right"),
            ) else {
                return (format!("not ({})", text(source, condition)), Binding::Tight);
            };
            match op {
                "or" | "||" => {
                    let left = wrap(negated(source, left), Binding::And);
                    let right = wrap(negated(source, right), Binding::And);
                    return (format!("{} and {}", left, right), Binding::And);
                }
                "and" | "&&" => {
                    let left = wrap(negated(source, left), Binding::Or);
                    let right = wrap(negated(source, right), Binding::Or);
                    return (format!("{} or {}", left, right), Binding::Or);
                }
                "==" | "!=" => {
                    let swapped = if op == "==" { "!=" } else { "==" };
                    return (
                        format!("{} {} {}", text(source, left), swapped, text(source, right)),
                        Binding::Tight,
                    );
                }
                _ => {}
            }
        }
        _ => {}
    }
    let condition_text = text(source, condition);
    if matches!(
        inner.kind(),
        "identifier" | "attribute" | "call" | "true" | "false"
    ) || inner != condition
    {
        (format!("not {}", condition_text), Binding::Tight)
    } else {
        (format!("not ({})", condition_text), Binding::Tight)
    }
}

/// How `node` binds: `or`, `and`, or tighter.
fn binding(source: &str, node: Node) -> Binding {
    if node.kind() != "binary_operator" {
        return Binding::Tight;
    }
    match node.child_by_field_name("op").map(|op| text(source, op)) {
        Some("or" | "||") => Binding::Or,
        Some("and" | "&&") => Binding::And,
        _ => Binding::Tight,
    }
}

/// Parenthesizes `expression` when it binds looser than `context` needs:
/// inside `and`, an `or` needs parentheses; inside `or`, nothing does.
fn wrap((text, binding): (String, Binding), context: Binding) -> String {
    if context == Binding::And && binding == Binding::Or {
        format!("({})", text)
    } else {
        text
    }
}
