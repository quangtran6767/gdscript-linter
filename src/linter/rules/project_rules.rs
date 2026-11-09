//! Cross-file rules, using the run's shared index and conservative signal use.
use super::Rule;
use super::fix_helpers;
use super::godot_classes::ENGINE_CLASS_NAMES;
use super::godot_virtuals::VIRTUAL_METHODS;
use super::resource_paths::project_relative;
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use crate::project::{MemberLookup, ProjectAnalysis, ProjectIndex};
use crate::semantic::SymbolKind;
use std::path::{Path, PathBuf};

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

/// The `class_name` statement at the top level of a script, with its name.
fn class_name_node<'a>(
    tree: &'a tree_sitter::Tree,
    source: &str,
) -> Option<(String, tree_sitter::Node<'a>)> {
    let root = tree.root_node();
    for index in 0..root.named_child_count() {
        let node = root.named_child(index as u32)?;
        if node.kind() == "class_name_statement" {
            let name = node.child_by_field_name("name")?;
            return Some((source[name.byte_range()].to_string(), name));
        }
    }
    None
}

/// The top-level `extends` statement, which may be part of a combined
/// `class_name X extends Y` line.
fn extends_node(tree: &tree_sitter::Tree) -> Option<tree_sitter::Node<'_>> {
    let root = tree.root_node();
    for index in 0..root.named_child_count() {
        let node = root.named_child(index as u32)?;
        if node.kind() == "extends_statement" {
            return Some(node);
        }
    }
    None
}

/// Where to report a script-wide issue: the `extends` or `class_name`
/// statement, else the first line with code.
fn script_anchor(context: &FileContext) -> Span {
    if let Some(node) = extends_node(context.tree) {
        return Span::from_node(&node);
    }
    let root = context.tree.root_node();
    for index in 0..root.named_child_count() {
        if let Some(node) = root.named_child(index as u32) {
            if node.kind() == "class_name_statement" {
                return Span::from_node(&node);
            }
        }
    }
    for index in 0..root.named_child_count() {
        if let Some(node) = root.named_child(index as u32) {
            if node.kind() != "comment" && node.start_byte() < node.end_byte() {
                let end = fix_helpers::line_end(context.source, node.start_byte());
                return Span::from_byte_range(context.source, node.start_byte(), end);
            }
        }
    }
    let end = fix_helpers::line_end(context.source, 0).max(context.source.len().min(1));
    Span::from_byte_range(context.source, 0, end)
}

pub struct DuplicateClassNameRule;
impl Rule for DuplicateClassNameRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let Some(project) = context.project else {
            return;
        };
        let Some((name, node)) = class_name_node(context.tree, context.source) else {
            return;
        };
        let own = project_relative(context.path, &project.root);
        let mut others = Vec::new();
        for path in project.scripts_declaring(&name) {
            if Some(path) != own.as_deref() {
                others.push(format!(
                    "res://{}",
                    path.to_string_lossy().replace('\\', "/")
                ));
            }
        }
        if others.is_empty() {
            return;
        }
        diagnostics.push(Diagnostic {
            rule: "duplicate-class-name".to_string(),
            severity: Severity::Error,
            message: format!(
                "Class name '{}' is also declared in {}.",
                name,
                others.join(", ")
            ),
            help: Some(format!(
                "Rename one of the classes. Godot registers one global class per name and fails to load the other script with `Class \"{}\" hides a global script class`.",
                name
            )),
            span: Span::from_node(&node),
            fixes: Vec::new(),
        });
    }
}

/// `player_controller` -> `PlayerController`. `None` when the result isn't a
/// valid identifier.
fn pascal_case(stem: &str) -> Option<String> {
    let mut name = String::new();
    for part in stem.split(['_', '-', ' ', '.']) {
        let mut characters = part.chars();
        if let Some(first) = characters.next() {
            name.extend(first.to_uppercase());
            name.push_str(characters.as_str());
        }
    }
    let first = name.chars().next()?;
    if !first.is_ascii_alphabetic()
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return None;
    }
    Some(name)
}

pub struct MissingClassNameRule;
impl Rule for MissingClassNameRule {
    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(project) = context.project else {
            return;
        };
        if context.tree.root_node().has_error()
            || context.source.trim().is_empty()
            || class_name_node(context.tree, context.source).is_some()
        {
            return;
        }
        let own = project_relative(context.path, &project.root);
        if let Some(own) = &own {
            let own_res = format!("res://{}", own.to_string_lossy().replace('\\', "/"));
            for autoload in project.autoloads.values() {
                if autoload == own {
                    return;
                }
                let autoload_res =
                    format!("res://{}", autoload.to_string_lossy().replace('\\', "/"));
                if analysis.scene_root_scripts.get(&autoload_res) == Some(&own_res) {
                    return;
                }
            }
        }
        let candidate = context
            .path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(pascal_case)
            .filter(|name| {
                ENGINE_CLASS_NAMES.binary_search(&name.as_str()).is_err()
                    && project.scripts_declaring(name).is_empty()
                    && !project.autoloads.contains_key(name)
            });
        let mut fixes = Vec::new();
        if let Some(name) = &candidate {
            // Godot's style guide order is `@tool`, `class_name`, `extends`.
            let insert_at = if let Some(extends) = extends_node(context.tree) {
                let start = fix_helpers::line_start(context.source, extends.start_byte());
                if context.source[start..extends.start_byte()]
                    .trim()
                    .is_empty()
                {
                    Some(start)
                } else {
                    None
                }
            } else {
                let first_line = &context.source[..fix_helpers::line_end(context.source, 0)];
                if first_line.trim() == "@tool" {
                    Some((first_line.len() + 1).min(context.source.len()))
                } else if first_line.trim_start().starts_with('@') {
                    None
                } else {
                    Some(0)
                }
            };
            if let Some(insert_at) = insert_at {
                let mut replacement = format!("class_name {}\n", name);
                if insert_at == context.source.len() && !context.source.ends_with('\n') {
                    replacement.insert(0, '\n');
                }
                fixes.push(Fix {
                    description: format!("Add `class_name {}`", name),
                    safety: FixSafety::Unsafe,
                    edits: vec![Edit {
                        span: Span::from_byte_range(context.source, insert_at, insert_at),
                        replacement,
                    }],
                });
            }
        }
        let help = match &candidate {
            Some(name) => format!(
                "Add `class_name {}` (or another unique PascalCase name) above `extends`. Autoload scripts are exempt.",
                name
            ),
            None => {
                "Add a unique PascalCase `class_name` above `extends`. Autoload scripts are exempt."
                    .to_string()
            }
        };
        diagnostics.push(Diagnostic {
            rule: "missing-class-name".to_string(),
            severity: Severity::Warning,
            message: "Script has no class_name.".to_string(),
            help: Some(help),
            span: script_anchor(context),
            fixes,
        });
    }
}

/// The engine class a script finally extends, following project scripts by
/// class name and path. `None` when the chain can't be resolved or ends at
/// an implicit `RefCounted`.
fn engine_base(project: &ProjectIndex, path: &Path) -> Option<String> {
    let mut current = project
        .scripts
        .iter()
        .position(|script| script.path == path)?;
    for _ in 0..64 {
        let script = project.scripts.get(current)?;
        let base = script.extends.as_ref()?;
        if let Some(parent) = project.class_names.get(base) {
            current = *parent;
            continue;
        }
        if base.ends_with(".gd") {
            let target = match base.strip_prefix("res://") {
                Some(relative) => PathBuf::from(relative),
                None => script.path.parent().unwrap_or(Path::new("")).join(base),
            };
            let mut normalized = PathBuf::new();
            for component in target.components() {
                match component {
                    std::path::Component::ParentDir => {
                        normalized.pop();
                    }
                    std::path::Component::CurDir => {}
                    other => normalized.push(other.as_os_str()),
                }
            }
            current = project
                .scripts
                .iter()
                .position(|script| script.path == normalized)?;
            continue;
        }
        if ENGINE_CLASS_NAMES.binary_search(&base.as_str()).is_ok() {
            return Some(base.clone());
        }
        return None;
    }
    None
}

pub struct SceneConnectionMissingMethodRule;
impl Rule for SceneConnectionMissingMethodRule {
    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(project) = context.project else {
            return;
        };
        if context.tree.root_node().has_error() {
            return;
        }
        let Some(own) = project_relative(context.path, &project.root) else {
            return;
        };
        let own_res = format!("res://{}", own.to_string_lossy().replace('\\', "/"));
        // Functions of the buffer being linted, which may be newer than disk.
        let mut local_functions = std::collections::HashSet::new();
        let root = context.tree.root_node();
        for index in 0..root.named_child_count() {
            if let Some(node) = root.named_child(index as u32) {
                if node.kind() == "function_definition" {
                    if let Some(name) = node.child_by_field_name("name") {
                        local_functions.insert(&context.source[name.byte_range()]);
                    }
                }
            }
        }
        let mut reported = std::collections::HashSet::new();
        for connection in &analysis.scene_connections {
            if connection.target_script.as_deref() != Some(own_res.as_str())
                || local_functions.contains(connection.method.as_str())
            {
                continue;
            }
            let missing = match project.has_member_in_script(&own, &connection.method) {
                MemberLookup::Yes => false,
                MemberLookup::No => true,
                // Engine methods can't be checked, but engine methods never
                // start with `_` unless they are virtual callbacks.
                MemberLookup::Unknown => {
                    connection.method.starts_with('_')
                        && VIRTUAL_METHODS
                            .binary_search(&connection.method.as_str())
                            .is_err()
                        && engine_base(project, &own).is_some()
                }
            };
            if !missing
                || !reported.insert((
                    connection.scene.as_str(),
                    connection.signal.as_str(),
                    connection.from.as_str(),
                    connection.method.as_str(),
                ))
            {
                continue;
            }
            diagnostics.push(Diagnostic {
                rule: "scene-connection-missing-method".to_string(),
                severity: Severity::Error,
                message: format!(
                    "Scene '{}' connects signal '{}' of node '{}' to method '{}', which this script doesn't define.",
                    connection.scene, connection.signal, connection.from, connection.method
                ),
                help: Some(format!(
                    "Add `func {}(...)` to this script or a base script, or fix the method name in the scene's [connection]. Godot only fails when the signal is emitted.",
                    connection.method
                )),
                span: script_anchor(context),
                fixes: Vec::new(),
            });
        }
    }
}
