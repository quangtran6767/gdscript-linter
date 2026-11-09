//! Project rules for the string paths passed to `load()`, `preload()` and
//! `ResourceLoader.load()`: the target must exist, and a `res://` path with a
//! known UID should use the UID instead.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};
use crate::project::{ProjectAnalysis, ProjectIndex};
use std::path::{Component, Path, PathBuf};

/// A string literal path given to a resource loading call.
pub(crate) struct LoadPath<'a> {
    pub node: tree_sitter::Node<'a>,
    /// The path without quotes.
    pub path: String,
    /// The quote character, `"` or `'`.
    pub quote: char,
}

/// Finds `load("...")`, `preload("...")`, `ResourceLoader.load("...")` and
/// `ResourceLoader.load_threaded_request("...")` with a plain string literal
/// as their first argument. Raw strings, string names and computed paths are
/// skipped.
pub(crate) fn load_paths<'a>(tree: &'a tree_sitter::Tree, source: &str) -> Vec<LoadPath<'a>> {
    let mut paths = Vec::new();
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        let is_load = match node.kind() {
            "call" => node
                .named_child(0)
                .is_some_and(|callee| matches!(&source[callee.byte_range()], "load" | "preload")),
            "attribute_call" => {
                let receiver_is_loader = node
                    .prev_named_sibling()
                    .is_some_and(|receiver| &source[receiver.byte_range()] == "ResourceLoader");
                receiver_is_loader
                    && node.named_child(0).is_some_and(|callee| {
                        matches!(
                            &source[callee.byte_range()],
                            "load" | "load_threaded_request"
                        )
                    })
            }
            _ => false,
        };
        if is_load {
            if let Some(argument) = node
                .child_by_field_name("arguments")
                .and_then(|arguments| arguments.named_child(0))
            {
                if argument.kind() == "string" {
                    let text = &source[argument.byte_range()];
                    let quote = text.chars().next().unwrap_or('"');
                    if matches!(quote, '"' | '\'')
                        && text.len() >= 2
                        && !text.starts_with("\"\"\"")
                        && !text.starts_with("'''")
                        && !text[1..text.len() - 1].contains('\\')
                    {
                        paths.push(LoadPath {
                            node: argument,
                            path: text[1..text.len() - 1].to_string(),
                            quote,
                        });
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
                return paths;
            }
        }
    }
}

/// The path of `file` relative to the project root, when it is inside it.
pub(crate) fn project_relative(file: &Path, root: &Path) -> Option<PathBuf> {
    if file.as_os_str().is_empty() {
        return None;
    }
    let absolute = if file.is_absolute() {
        file.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(file)
    };
    // Unsaved buffers (stdin, LSP) may not exist on disk: canonicalize the
    // folder instead.
    let absolute = if let Ok(path) = std::fs::canonicalize(&absolute) {
        path
    } else {
        let parent = std::fs::canonicalize(absolute.parent()?).ok()?;
        parent.join(absolute.file_name()?)
    };
    absolute.strip_prefix(root).ok().map(Path::to_path_buf)
}

/// Resolves a load path to `res://...`: `res://` paths as they are, relative
/// paths against the script's folder. `None` for UIDs, `user://` and other
/// schemes, or a relative path without a known script location.
fn resolve_res_path(path: &str, script: Option<&Path>) -> Option<String> {
    if path.starts_with("res://") {
        return Some(path.to_string());
    }
    if path.contains("://") || path.starts_with('/') || path.is_empty() {
        return None;
    }
    let folder = script?.parent().unwrap_or(Path::new(""));
    let mut normalized = PathBuf::new();
    for component in folder.join(path).components() {
        match component {
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    Some(format!(
        "res://{}",
        normalized.to_string_lossy().replace('\\', "/")
    ))
}

pub struct MissingResourcePathRule;
impl Rule for MissingResourcePathRule {
    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(project) = context.project else {
            return;
        };
        let script = project_relative(context.path, &project.root);
        for load in load_paths(context.tree, context.source) {
            let missing = if load.path.starts_with("uid://") {
                !analysis.uid_paths.contains_key(&load.path)
            } else {
                match resolve_res_path(&load.path, script.as_deref()) {
                    Some(res) => !res_exists(project, &res),
                    None => false,
                }
            };
            if !missing {
                continue;
            }
            let help = if load.path.starts_with("uid://") {
                "Use the UID of an existing resource (from its `.uid` file, `.import` file or scene header), or open the project in Godot to regenerate UIDs."
            } else {
                "Check the path for a typo, or create the resource. Paths are case-sensitive in exported games."
            };
            diagnostics.push(Diagnostic {
                rule: "missing-resource-path".to_string(),
                severity: Severity::Error,
                message: format!("Resource '{}' does not exist in this project.", load.path),
                help: Some(help.to_string()),
                span: Span::from_node(&load.node),
                fixes: Vec::new(),
            });
        }
    }
}

fn res_exists(project: &ProjectIndex, res: &str) -> bool {
    let relative = res.trim_start_matches("res://");
    project.root.join(relative).is_file()
}

pub struct PreferUidPathRule;
impl Rule for PreferUidPathRule {
    fn check_project_context(
        &mut self,
        context: &FileContext,
        analysis: &ProjectAnalysis,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some(project) = context.project else {
            return;
        };
        let script = project_relative(context.path, &project.root);
        for load in load_paths(context.tree, context.source) {
            let Some(res) = resolve_res_path(&load.path, script.as_deref()) else {
                continue;
            };
            // Only paths with a known UID are reported, so every report has
            // a fix. Files without UIDs (JSON, text, never-saved resources)
            // keep their path.
            let Some(uid) = analysis.path_uids.get(&res) else {
                continue;
            };
            let span = Span::from_node(&load.node);
            diagnostics.push(Diagnostic {
                rule: "prefer-uid-path".to_string(),
                severity: Severity::Warning,
                message: format!("Load '{}' by its UID '{}'.", load.path, uid),
                help: Some(
                    "UID paths keep working when files are moved or renamed. Replace the path with the UID."
                        .to_string(),
                ),
                span,
                fixes: vec![Fix {
                    description: format!("Replace with {}{}{}", load.quote, uid, load.quote),
                    safety: FixSafety::Safe,
                    edits: vec![Edit {
                        span,
                        replacement: format!("{}{}{}", load.quote, uid, load.quote),
                    }],
                }],
            });
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn resolves_relative_paths_against_the_script_folder() {
        let script = Path::new("scenes/player/player.gd");
        assert_eq!(
            resolve_res_path("bullet.tscn", Some(script)).unwrap(),
            "res://scenes/player/bullet.tscn"
        );
        assert_eq!(
            resolve_res_path("../shared/icon.png", Some(script)).unwrap(),
            "res://scenes/shared/icon.png"
        );
        assert_eq!(resolve_res_path("../../../x.png", Some(script)), None);
        assert_eq!(resolve_res_path("bullet.tscn", None), None);
        assert_eq!(resolve_res_path("user://save.tres", Some(script)), None);
        assert_eq!(resolve_res_path("uid://abc", Some(script)), None);
        assert_eq!(
            resolve_res_path("res://a.png", None).unwrap(),
            "res://a.png"
        );
    }
}
