//! Shared helpers for the W11 rules that need to know what a value is:
//! the declared (or plainly literal) type of an identifier, and simple
//! syntax-tree walks. No type inference beyond literal initializers.

use tree_sitter::{Node, Tree};

use crate::linter::diagnostic::FileContext;
use crate::semantic::{SemanticModel, SymbolKind};

/// Packed arrays: all have `size()` and `is_empty()`.
const PACKED_ARRAYS: &[&str] = &[
    "PackedByteArray",
    "PackedInt32Array",
    "PackedInt64Array",
    "PackedFloat32Array",
    "PackedFloat64Array",
    "PackedStringArray",
    "PackedVector2Array",
    "PackedVector3Array",
    "PackedColorArray",
    "PackedVector4Array",
];

/// Every node of `kind` in the tree, in source order.
pub fn nodes_of_kind<'tree>(tree: &'tree Tree, kind: &str) -> Vec<Node<'tree>> {
    let mut nodes = Vec::new();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if node.kind() == kind {
            nodes.push(node);
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return nodes;
            }
        }
    }
}

/// True when `node` or one of its descendants has one of `kinds`.
pub fn contains_kind(node: Node, kinds: &[&str]) -> bool {
    if kinds.contains(&node.kind()) {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_kind(child, kinds) {
            return true;
        }
    }
    false
}

/// Strips any number of enclosing parentheses.
pub fn strip_parentheses(mut node: Node) -> Node {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = node.named_child(0) else {
            break;
        };
        node = inner;
    }
    node
}

pub fn text<'source>(source: &'source str, node: Node) -> &'source str {
    &source[node.byte_range()]
}

/// The type of the identifier `node` at this point of the file: its
/// declared type (`var a: Array[int]`, `a: Vector2` parameters, typed loop
/// variables), or for an inferred or untyped declaration, the type of a
/// plain literal initializer (`[]`, `{}`, `""`, `Vector2(...)`). None when
/// the identifier doesn't resolve to a script variable, constant, parameter
/// or loop variable, or its type isn't written down.
pub fn identifier_type(context: &FileContext, model: &SemanticModel, node: Node) -> Option<String> {
    if node.kind() != "identifier" {
        return None;
    }
    let name = text(context.source, node);
    let symbol_id = model.lookup(model.scope_at(node.start_byte()), name)?;
    let symbol = model.symbol(symbol_id)?;
    if !matches!(
        symbol.kind,
        SymbolKind::Variable
            | SymbolKind::Constant
            | SymbolKind::Parameter
            | SymbolKind::LoopVariable
    ) {
        return None;
    }
    if let Some(declared) = &symbol.declared_type {
        return Some(declared.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    let declaration = context.tree.root_node().descendant_for_byte_range(
        symbol.declaration_span.start_byte,
        symbol.declaration_span.end_byte,
    )?;
    let value = declaration.child_by_field_name("value")?;
    literal_type(context.source, value)
}

/// The type of a plain literal value, or None.
fn literal_type(source: &str, value: Node) -> Option<String> {
    match value.kind() {
        "array" => Some("Array".to_string()),
        "dictionary" => Some("Dictionary".to_string()),
        "string" => Some("String".to_string()),
        "call" => {
            let function = value.named_child(0)?;
            let name = text(source, function);
            if function.kind() == "identifier"
                && (is_float_vector(name) || PACKED_ARRAYS.contains(&name))
            {
                Some(name.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `Array`, `Array[T]`, `Dictionary`, `Dictionary[K, V]` or a packed array:
/// types with `size()` and `is_empty()`.
pub fn is_sized_collection(type_text: &str) -> bool {
    is_array(type_text)
        || type_text == "Dictionary"
        || type_text.starts_with("Dictionary[")
        || PACKED_ARRAYS.contains(&type_text)
}

/// `Array` or `Array[T]`: types with `any`, `all`, `filter` and `map`.
pub fn is_array(type_text: &str) -> bool {
    type_text == "Array" || type_text.starts_with("Array[")
}

/// The element type of `Array[T]`, or None for an untyped array.
pub fn array_element_type(type_text: &str) -> Option<&str> {
    type_text
        .strip_prefix("Array[")
        .and_then(|rest| rest.strip_suffix(']'))
        .map(str::trim)
}

/// `Vector2`, `Vector3` or `Vector4`: float vectors with `distance_to`,
/// `distance_squared_to`, `length` and `length_squared`.
pub fn is_float_vector(type_text: &str) -> bool {
    matches!(type_text, "Vector2" | "Vector3" | "Vector4")
}
