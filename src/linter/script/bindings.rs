//! Native lazy tree queries. Owned tree handles keep callbacks sound even if
//! a rule retains a wrapper; each file replaces its native callbacks.
use crate::linter::diagnostic::{FileContext, Span};
use crate::project::{MemberLookup, ProjectIndex};
use crate::semantic::{Reference, SemanticModel};
use rquickjs::{Ctx, Function};
use serde_json::{Value, json};
use std::rc::Rc;

pub fn span_json(span: Span) -> Value {
    json!({"start":{"line":span.start.line,"column":span.start.column,"byte":span.start_byte},
        "end":{"line":span.end.line,"column":span.end.column,"byte":span.end_byte}})
}

pub fn install<'js>(
    ctx: &Ctx<'js>,
    context: &FileContext,
    project: Option<Rc<ProjectIndex>>,
) -> rquickjs::Result<()> {
    let tree = context.tree.clone();
    let source = context.source.to_string();
    ctx.globals().set(
        "__node",
        Function::new(
            ctx.clone(),
            move |path: Vec<u32>, property: String, field: String| -> String {
                let mut node = tree.root_node();
                for index in &path {
                    let Some(child) = node.child(*index) else {
                        return "null".to_string();
                    };
                    node = child;
                }
                let value = match property.as_str() {
                    "type" => json!(node.kind()),
                    "text" => json!(&source[node.byte_range()]),
                    "range" => span_json(Span::from_node(&node)),
                    "children" | "namedChildren" => {
                        let mut children = Vec::with_capacity(node.child_count() as usize);
                        for index in 0..node.child_count() {
                            if let Some(child) = node.child(index) {
                                if property == "children" || child.is_named() {
                                    let mut child_path = path.clone();
                                    child_path.push(index);
                                    children.push(child_path);
                                }
                            }
                        }
                        json!(children)
                    }
                    "field" => {
                        let target = node.child_by_field_name(&field);
                        let mut result = Value::Null;
                        if let Some(target) = target {
                            for index in 0..node.child_count() {
                                if node.child(index) == Some(target) {
                                    let mut child_path = path.clone();
                                    child_path.push(index);
                                    result = json!(child_path);
                                    break;
                                }
                            }
                        }
                        result
                    }
                    _ => Value::Null,
                };
                value.to_string()
            },
        )?,
    )?;
    let semantic = context.semantic.cloned();
    ctx.globals().set(
        "__semantic",
        Function::new(
            ctx.clone(),
            move |method: String, id: usize, name: String| -> String {
                match &semantic {
                    Some(model) => semantic_query(model, &method, id, &name).to_string(),
                    None => "null".to_string(),
                }
            },
        )?,
    )?;
    ctx.globals().set(
        "__project",
        Function::new(
            ctx.clone(),
            move |method: String, name: String, member: String| -> String {
                let Some(project) = &project else {
                    return "null".to_string();
                };
                let value = match method.as_str() {
                    "root" => json!(project.root.to_string_lossy().replace('\\', "/")),
                    "classNames" => {
                        let mut names: Vec<&str> =
                            project.class_names.keys().map(String::as_str).collect();
                        names.sort();
                        json!(names)
                    }
                    "autoloads" => {
                        let mut paths = std::collections::BTreeMap::new();
                        for (name, path) in &project.autoloads {
                            paths.insert(name, path.to_string_lossy().replace('\\', "/"));
                        }
                        json!(paths)
                    }
                    "hasMember" => json!(match project.has_member(&name, &member) {
                        MemberLookup::Yes => "yes",
                        MemberLookup::No => "no",
                        MemberLookup::Unknown => "unknown",
                    }),
                    _ => Value::Null,
                };
                value.to_string()
            },
        )?,
    )?;
    Ok(())
}

fn semantic_query(model: &SemanticModel, method: &str, id: usize, name: &str) -> Value {
    match method {
        "scopeAt" => json!(model.scope_at(id).0),
        "lookup" => json!(
            model
                .lookup(crate::semantic::ScopeId(id as u32), name)
                .map(|id| id.0)
        ),
        "referencesTo" => {
            let mut references = Vec::new();
            for reference in model.references_to(crate::semantic::SymbolId(id as u32)) {
                references.push(reference_json(reference));
            }
            json!(references)
        }
        "symbols" => {
            let mut symbols = Vec::with_capacity(model.symbols().len());
            for symbol in model.symbols() {
                let kind = match symbol.kind {
                    crate::semantic::SymbolKind::Variable => "variable",
                    crate::semantic::SymbolKind::Constant => "constant",
                    crate::semantic::SymbolKind::Parameter => "parameter",
                    crate::semantic::SymbolKind::LoopVariable => "loop_variable",
                    crate::semantic::SymbolKind::Function => "function",
                    crate::semantic::SymbolKind::Signal => "signal",
                    crate::semantic::SymbolKind::Enum => "enum",
                    crate::semantic::SymbolKind::EnumMember => "enum_member",
                    crate::semantic::SymbolKind::InnerClass => "inner_class",
                };
                symbols.push(json!({"id":symbol.id.0,"name":symbol.name,"kind":kind,"scope":symbol.scope.0,
                    "nameSpan":span_json(symbol.name_span),"declarationSpan":span_json(symbol.declaration_span),
                    "declaredType":symbol.declared_type,"annotations":symbol.annotations,"isStatic":symbol.is_static}));
            }
            json!(symbols)
        }
        _ => Value::Null,
    }
}

fn reference_json(reference: &Reference) -> Value {
    json!({"range":span_json(reference.span),"name":reference.name,
        "kind":match reference.kind { crate::semantic::ReferenceKind::Read => "read",crate::semantic::ReferenceKind::Write => "write",crate::semantic::ReferenceKind::Call => "call" },
        "symbol":reference.symbol.map(|id| id.0)})
}
