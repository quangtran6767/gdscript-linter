//! `$Panel/Button` becomes `%Button` when the script is the root script of
//! its scenes and every one of them marks that node `unique_name_in_owner`.
//! `%Button` keeps working when the scene tree is rearranged.
//!
//! The fix is Unsafe: the script could also be used from code or another
//! scene the project scan can't see.

use super::Rule;
use super::receiver_types::{nodes_of_kind, text};
use super::resource_paths::project_relative;
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use crate::project::ProjectAnalysis;

pub const RULE_ID: &str = "prefer-unique-node-name";

pub struct PreferUniqueNodeNameRule;

impl Rule for PreferUniqueNodeNameRule {
    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(project) = context.project else {
            return;
        };
        let Some(own) = project_relative(context.path, &project.root) else {
            return;
        };
        let own_res = format!("res://{}", own.to_string_lossy().replace('\\', "/"));
        if analysis.non_root_scripts.contains(&own_res) {
            return;
        }
        let mut scenes = Vec::new();
        for (scene, script) in &analysis.scene_root_scripts {
            if script != &own_res {
                continue;
            }
            // A root scene without unique-node data is inherited: unknown.
            let Some(unique_nodes) = analysis.scene_unique_nodes.get(scene) else {
                return;
            };
            scenes.push(unique_nodes);
        }
        if scenes.is_empty() {
            return;
        }

        for get_node in nodes_of_kind(context.tree, "get_node") {
            if is_in_inner_class(get_node) {
                continue;
            }
            let Some(path) = node_path(text(context.source, get_node)) else {
                continue;
            };
            if !scenes
                .iter()
                .all(|unique_nodes| unique_nodes.contains(path))
            {
                continue;
            }
            let name = path.rsplit('/').next().unwrap_or(path);
            let replacement = if is_identifier(name) {
                format!("%{}", name)
            } else {
                format!("%\"{}\"", name)
            };
            diagnostics.push(Diagnostic {
                rule: RULE_ID.to_string(),
                severity: Severity::Info,
                message: format!(
                    "`{}` is a unique node: use `{}`.",
                    text(context.source, get_node),
                    replacement
                ),
                help: Some(format!(
                    "The scene marks `{}` with unique_name_in_owner, so `{}` finds it even if the tree changes.",
                    path, replacement
                )),
                span: Span::from_node(&get_node),
                fixes: vec![Fix {
                    description: format!("Use `{}`", replacement),
                    safety: FixSafety::Unsafe,
                    edits: vec![Edit {
                        span: Span::from_node(&get_node),
                        replacement,
                    }],
                }],
            });
        }
    }
}

/// The node path of `$A/B` or `$"A/B"`, when it's a plain relative path.
fn node_path(get_node_text: &str) -> Option<&str> {
    let path = get_node_text.strip_prefix('$')?;
    let path = if let Some(quoted) = path.strip_prefix('"') {
        quoted.strip_suffix('"')?
    } else if let Some(quoted) = path.strip_prefix('\'') {
        quoted.strip_suffix('\'')?
    } else {
        path
    };
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('%')
        || path.contains(':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(path)
}

fn is_in_inner_class(node: tree_sitter::Node) -> bool {
    let mut ancestor = node.parent();
    while let Some(current) = ancestor {
        if current.kind() == "class_definition" {
            return true;
        }
        ancestor = current.parent();
    }
    false
}

fn is_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}
