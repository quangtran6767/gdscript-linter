//! Run-wide signal usage, conservatively matched by name across scripts/scenes.
use super::ProjectIndex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct ProjectAnalysis {
    pub used_signals: HashSet<String>,
    pub dynamic_signal_usage: bool,
}
impl ProjectAnalysis {
    pub fn from_tree(tree: &tree_sitter::Tree, source: &str) -> Self {
        let mut analysis = Self::default();
        analysis.collect_tree(tree, source);
        analysis
    }
    fn collect_tree(&mut self, tree: &tree_sitter::Tree, source: &str) {
        let mut cursor = tree.walk();
        loop {
            let node = cursor.node();
            if matches!(node.kind(), "call" | "attribute_call") {
                if let Some(name) = node.named_child(0) {
                    let method = &source[name.byte_range()];
                    if matches!(
                        method,
                        "emit_signal" | "connect" | "disconnect" | "is_connected"
                    ) {
                        if let Some(argument) = node
                            .child_by_field_name("arguments")
                            .and_then(|arguments| arguments.named_child(0))
                        {
                            if argument.kind() == "string" {
                                self.used_signals.insert(
                                    source[argument.byte_range()]
                                        .trim_matches(['\'', '"'])
                                        .to_string(),
                                );
                            } else if method == "emit_signal" {
                                self.dynamic_signal_usage = true;
                            }
                        }
                    }
                    if node.kind() == "attribute_call"
                        && matches!(method, "emit" | "connect" | "disconnect" | "is_connected")
                    {
                        let receiver = node.prev_named_sibling();
                        if let Some(receiver) = receiver {
                            if receiver.kind() == "identifier" {
                                self.used_signals
                                    .insert(source[receiver.byte_range()].to_string());
                            } else if method == "emit" {
                                self.dynamic_signal_usage = true;
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
                    break;
                }
            }
            if cursor.node() == tree.root_node() {
                break;
            }
        }
    }

    pub fn build(index: &ProjectIndex) -> Result<Self, String> {
        let mut analysis = Self::default();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_gdscript::LANGUAGE.into())
            .map_err(|error| error.to_string())?;
        for script in &index.scripts {
            let source = std::fs::read_to_string(index.root.join(&script.path))
                .map_err(|error| error.to_string())?;
            let tree = parser
                .parse(&source, None)
                .ok_or("failed to parse signal usage")?;
            analysis.collect_tree(&tree, &source);
        }
        let pattern =
            regex::Regex::new(r#"\bsignal\s*=\s*"([^"]+)""#).map_err(|error| error.to_string())?;
        for path in scene_files(&index.root)? {
            let source = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {}", path.display(), error))?;
            for line in source.lines() {
                if line.trim_start().starts_with("[connection ") {
                    if let Some(captures) = pattern.captures(line) {
                        analysis.used_signals.insert(captures[1].to_string());
                    }
                }
            }
        }
        Ok(analysis)
    }
}

pub(crate) fn scene_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    collect(root, &mut paths)?;
    paths.sort();
    Ok(paths)
}
fn collect(root: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(root).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some(".git" | ".godot" | ".gdguard" | "node_modules" | "target")
            ) {
                collect(&entry.path(), paths)?;
            }
        } else if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "tscn")
        {
            paths.push(entry.path());
        }
    }
    Ok(())
}
