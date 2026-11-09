//! Builds scopes/declarations in one syntax walk, then resolves collected
//! uses. Delayed resolution makes class members visible before declaration.

use super::{
    Reference, ReferenceKind, Scope, ScopeId, ScopeKind, ScopePoint, SemanticModel, Symbol,
    SymbolId, SymbolKind,
};
use crate::linter::diagnostic::Span;
use std::collections::HashMap;
use tree_sitter::{Node, Tree};

struct PendingReference {
    reference: Reference,
    scope: ScopeId,
    class_member: bool,
    qualified: bool,
    receiver: Option<String>,
}

struct Builder<'a> {
    source: &'a str,
    model: SemanticModel,
    pending: Vec<PendingReference>,
}

pub(super) fn build(tree: &Tree, source: &str) -> SemanticModel {
    let root = Scope {
        id: ScopeId(0),
        kind: ScopeKind::Class,
        parent: None,
        span: Span::from_byte_range(source, 0, source.len()),
        symbols: Vec::new(),
    };
    let mut builder = Builder {
        source,
        model: SemanticModel {
            scopes: vec![root],
            symbols: Vec::new(),
            references: Vec::new(),
            unresolved: Vec::new(),
            names: vec![HashMap::new()],
            scope_points: Vec::new(),
            source_len: source.len(),
            qualified_members: HashMap::new(),
        },
        pending: Vec::new(),
    };
    builder.children(tree.root_node(), ScopeId(0));
    for pending in builder.pending {
        let mut scope = pending.scope;
        if pending.class_member {
            while !matches!(
                builder.model.scopes[scope.0 as usize].kind,
                ScopeKind::Class | ScopeKind::InnerClass
            ) {
                match builder.model.scopes[scope.0 as usize].parent {
                    Some(parent) => scope = parent,
                    None => break,
                }
            }
        }
        let mut reference = pending.reference;
        if pending.qualified && !pending.class_member {
            if let Some(receiver) = &pending.receiver {
                if let Some(owner) =
                    builder
                        .model
                        .lookup_at(scope, receiver, reference.span.start_byte)
                {
                    reference.symbol = builder
                        .model
                        .qualified_members
                        .get(&owner)
                        .and_then(|members| members.get(&reference.name))
                        .copied();
                }
            }
        }
        if !pending.qualified || pending.class_member {
            reference.symbol =
                builder
                    .model
                    .lookup_at(scope, &reference.name, reference.span.start_byte);
        }
        match reference.symbol {
            Some(symbol) => builder.model.references[symbol.0 as usize].push(reference),
            None => builder.model.unresolved.push(reference),
        }
    }
    // Scope starts/ends are properly nested. End events precede starts at
    // equal offsets, so sibling boundaries select the following scope.
    let mut events = Vec::with_capacity(builder.model.scopes.len() * 2);
    for scope in &builder.model.scopes[1..] {
        if scope.span.start_byte < scope.span.end_byte {
            events.push((scope.span.start_byte, false, scope.id));
            events.push((scope.span.end_byte, true, scope.id));
        }
    }
    events.sort_by_key(|event| (event.0, !event.1, event.2));
    let mut active = vec![ScopeId(0)];
    builder.model.scope_points.push(ScopePoint {
        byte: 0,
        scope: ScopeId(0),
    });
    for (byte, end, scope) in events {
        if end {
            if let Some(position) = active.iter().position(|id| *id == scope) {
                active.remove(position);
            }
        } else {
            active.push(scope);
        }
        let current = active.last().copied().unwrap_or(ScopeId(0));
        builder.model.scope_points.push(ScopePoint {
            byte,
            scope: current,
        });
    }
    builder.model
}

impl<'a> Builder<'a> {
    fn text(&self, node: Node) -> &'a str {
        &self.source[node.byte_range()]
    }

    fn scope(&mut self, node: Node, kind: ScopeKind, parent: ScopeId) -> ScopeId {
        let id = ScopeId(self.model.scopes.len() as u32);
        self.model.scopes.push(Scope {
            id,
            kind,
            parent: Some(parent),
            span: Span::from_node(&node),
            symbols: Vec::new(),
        });
        self.model.names.push(HashMap::new());
        id
    }

    fn declare(
        &mut self,
        declaration: Node,
        name: Node,
        kind: SymbolKind,
        scope: ScopeId,
    ) -> SymbolId {
        let id = SymbolId(self.model.symbols.len() as u32);
        let mut annotations = Vec::new();
        let mut is_static = false;
        for index in 0..declaration.named_child_count() {
            if let Some(child) = declaration.named_child(index as u32) {
                if child.kind() == "static_keyword" {
                    is_static = true;
                }
                if child.kind() == "annotations" {
                    for annotation_index in 0..child.named_child_count() {
                        if let Some(annotation) = child.named_child(annotation_index as u32) {
                            if let Some(identifier) = annotation.named_child(0) {
                                annotations.push(self.text(identifier).to_string());
                            }
                        }
                    }
                }
            }
        }
        let declared_type = declaration
            .child_by_field_name("type")
            .filter(|node| node.kind() == "type")
            .map(|node| self.text(node).to_string());
        let symbol = Symbol {
            id,
            name: self.text(name).to_string(),
            kind,
            scope,
            name_span: Span::from_node(&name),
            declaration_span: Span::from_node(&declaration),
            declared_type,
            annotations,
            is_static,
        };
        self.model.names[scope.0 as usize].insert(symbol.name.clone(), id);
        self.model.scopes[scope.0 as usize].symbols.push(id);
        self.model.symbols.push(symbol);
        self.model.references.push(Vec::new());
        id
    }

    fn reference(
        &mut self,
        node: Node,
        scope: ScopeId,
        kind: ReferenceKind,
        class_member: bool,
        qualified: bool,
    ) {
        if self.text(node) == "self" {
            return;
        }
        self.pending.push(PendingReference {
            reference: Reference {
                span: Span::from_node(&node),
                name: self.text(node).to_string(),
                kind,
                symbol: None,
            },
            scope,
            class_member,
            qualified,
            receiver: None,
        });
    }

    fn children(&mut self, node: Node, scope: ScopeId) {
        for index in 0..node.named_child_count() {
            if let Some(child) = node.named_child(index as u32) {
                self.visit(child, scope);
            }
        }
    }

    fn parameters(&mut self, node: Node, scope: ScopeId) {
        for index in 0..node.named_child_count() {
            if let Some(parameter) = node.named_child(index as u32) {
                let parameter = if parameter.kind() == "variadic_parameter" {
                    parameter.named_child(0).unwrap_or(parameter)
                } else {
                    parameter
                };
                let name = if parameter.kind() == "identifier" {
                    Some(parameter)
                } else {
                    parameter.named_child(0)
                };
                if let Some(name) = name {
                    self.declare(parameter, name, SymbolKind::Parameter, scope);
                }
                if let Some(value) = parameter.child_by_field_name("value") {
                    self.visit(value, scope);
                }
            }
        }
    }

    fn visit(&mut self, node: Node, scope: ScopeId) {
        match node.kind() {
            "variable_statement" | "const_statement" => {
                if let Some(name) = node.child_by_field_name("name") {
                    let kind = if node.kind() == "const_statement" {
                        SymbolKind::Constant
                    } else {
                        SymbolKind::Variable
                    };
                    self.declare(node, name, kind, scope);
                }
                if let Some(value) = node.child_by_field_name("value") {
                    self.visit(value, scope);
                }
                if let Some(setget) = node.child_by_field_name("setget") {
                    self.children(setget, scope);
                }
            }
            "function_definition" | "lambda" | "constructor_definition" => {
                if node.kind() == "constructor_definition" {
                    for index in 0..node.child_count() {
                        if let Some(name) = node.child(index) {
                            if self.text(name) == "_init" {
                                self.declare(node, name, SymbolKind::Function, scope);
                                break;
                            }
                        }
                    }
                }
                if let Some(name) = node.child_by_field_name("name") {
                    self.declare(node, name, SymbolKind::Function, scope);
                }
                let kind = if node.kind() == "lambda" {
                    ScopeKind::Lambda
                } else {
                    ScopeKind::Function
                };
                let inner = self.scope(node, kind, scope);
                if let Some(parameters) = node.child_by_field_name("parameters") {
                    self.parameters(parameters, inner);
                }
                if let Some(body) = node.child_by_field_name("body") {
                    self.children(body, inner);
                }
            }
            "get_body" | "set_body" => {
                let inner = self.scope(node, ScopeKind::Function, scope);
                for index in 0..node.named_child_count() {
                    if let Some(child) = node.named_child(index as u32) {
                        if child.kind() == "parameters" {
                            self.parameters(child, inner);
                        }
                    }
                }
                if let Some(body) = node.child_by_field_name("body") {
                    self.children(body, inner);
                }
            }
            "class_definition" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.declare(node, name, SymbolKind::InnerClass, scope);
                }
                let inner = self.scope(node, ScopeKind::InnerClass, scope);
                if let Some(body) = node.child_by_field_name("body") {
                    self.children(body, inner);
                }
            }
            "signal_statement" => {
                if let Some(name) = node.child_by_field_name("name") {
                    self.declare(node, name, SymbolKind::Signal, scope);
                }
                // Signal parameters are signature metadata, not lexical locals.
            }
            "enum_definition" => {
                let owner = node
                    .child_by_field_name("name")
                    .map(|name| self.declare(node, name, SymbolKind::Enum, scope));
                if let Some(body) = node.child_by_field_name("body") {
                    for index in 0..body.named_child_count() {
                        if let Some(member) = body.named_child(index as u32) {
                            if let Some(name) = member.child_by_field_name("left") {
                                let id = self.declare(member, name, SymbolKind::EnumMember, scope);
                                if let Some(owner) = owner {
                                    let text = self.text(name).to_string();
                                    self.model.names[scope.0 as usize].remove(&text);
                                    self.model
                                        .qualified_members
                                        .entry(owner)
                                        .or_default()
                                        .insert(text, id);
                                }
                            }
                            if let Some(value) = member.child_by_field_name("right") {
                                self.visit(value, scope);
                            }
                        }
                    }
                }
            }
            "if_statement" | "elif_clause" | "else_clause" | "while_statement" => {
                if let Some(condition) = node.child_by_field_name("condition") {
                    self.visit(condition, scope);
                }
                if let Some(body) = node.child_by_field_name("body") {
                    let inner = self.scope(body, ScopeKind::Block, scope);
                    self.children(body, inner);
                }
                for index in 0..node.named_child_count() {
                    if let Some(child) = node.named_child(index as u32) {
                        if matches!(child.kind(), "elif_clause" | "else_clause") {
                            self.visit(child, scope);
                        }
                    }
                }
            }
            "for_statement" => {
                if let Some(value) = node.child_by_field_name("right") {
                    self.visit(value, scope);
                }
                let inner = self.scope(node, ScopeKind::ForLoop, scope);
                if let Some(name) = node.child_by_field_name("left") {
                    self.declare(node, name, SymbolKind::LoopVariable, inner);
                }
                if let Some(body) = node.child_by_field_name("body") {
                    self.children(body, inner);
                }
            }
            "pattern_section" => {
                let inner = self.scope(node, ScopeKind::MatchBranch, scope);
                self.children(node, inner);
            }
            "pattern_binding" => {
                if let Some(name) = node.named_child(0) {
                    self.declare(node, name, SymbolKind::Variable, scope);
                }
            }
            "assignment" | "augmented_assignment" => {
                if let Some(left) = node.child_by_field_name("left") {
                    self.use_expression(left, scope, ReferenceKind::Write);
                    if node.kind() == "augmented_assignment" {
                        self.use_expression(left, scope, ReferenceKind::Read);
                    }
                }
                if let Some(right) = node.child_by_field_name("right") {
                    self.visit(right, scope);
                }
            }
            "call" => {
                if let Some(callee) = node.named_child(0) {
                    self.use_expression(callee, scope, ReferenceKind::Call);
                }
                if let Some(arguments) = node.child_by_field_name("arguments") {
                    self.children(arguments, scope);
                }
            }
            "attribute" => self.use_expression(node, scope, ReferenceKind::Read),
            "identifier" => self.reference(node, scope, ReferenceKind::Read, false, false),
            "name"
            | "type"
            | "inferred_type"
            | "annotations"
            | "annotation"
            | "class_name_statement"
            | "extends_statement"
            | "string"
            | "string_name"
            | "node_path"
            | "get_node"
            | "comment" => {}
            _ => self.children(node, scope),
        }
    }

    fn use_expression(&mut self, node: Node, scope: ScopeId, kind: ReferenceKind) {
        if node.kind() == "identifier" {
            self.reference(node, scope, kind, false, false);
        } else if node.kind() == "attribute" {
            let first = node.named_child(0);
            let is_self = first.is_some_and(|first| self.text(first) == "self");
            if let Some(first) = first {
                self.visit(first, scope);
            }
            for index in 1..node.named_child_count() {
                if let Some(member) = node.named_child(index as u32) {
                    if member.kind() == "identifier" {
                        self.reference(member, scope, kind, is_self && index == 1, true);
                        if index == 1 && !is_self {
                            if let Some(receiver) = first {
                                if receiver.kind() == "identifier" {
                                    let name = self.text(receiver).to_string();
                                    if let Some(pending) = self.pending.last_mut() {
                                        pending.receiver = Some(name);
                                    }
                                }
                            }
                        }
                    } else {
                        if let Some(name) = member.named_child(0) {
                            let use_kind = if member.kind() == "attribute_call" {
                                ReferenceKind::Call
                            } else {
                                kind
                            };
                            self.reference(name, scope, use_kind, is_self && index == 1, true);
                        }
                        if let Some(arguments) = member.child_by_field_name("arguments") {
                            self.children(arguments, scope);
                        }
                    }
                }
            }
        } else {
            self.visit(node, scope);
        }
    }
}
