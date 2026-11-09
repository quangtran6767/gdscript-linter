//! Run-wide project facts shared by every file of a lint run: signal usage
//! (conservatively matched by name across scripts and scenes), scene
//! connections with their target scripts, and the project's UID map.
use super::ProjectIndex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A `[connection]` entry of a text scene (`.tscn`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SceneConnection {
    /// The scene declaring the connection, as `res://...`.
    pub scene: String,
    pub signal: String,
    pub from: String,
    pub to: String,
    pub method: String,
    /// The script attached to the `to` node, as `res://...`, when the scene
    /// sets one through an external resource. `None` for nodes without a
    /// script, instanced sub-scenes and built-in scripts.
    pub target_script: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectAnalysis {
    pub used_signals: HashSet<String>,
    pub dynamic_signal_usage: bool,
    /// Method names used by scene connections anywhere in the project.
    pub connected_methods: HashSet<String>,
    /// Every scene connection, in scene path order.
    pub scene_connections: Vec<SceneConnection>,
    /// `uid://...` to `res://...`, from `.uid` files, `.import` files and the
    /// `uid` attribute of text scene and resource headers.
    pub uid_paths: HashMap<String, String>,
    /// `res://...` to `uid://...`, the reverse of `uid_paths`.
    pub path_uids: HashMap<String, String>,
    /// Scene `res://` path to the `res://` path of its root node's script.
    pub scene_root_scripts: HashMap<String, String>,
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
        let files = project_files(&index.root)?;
        let mut scenes = Vec::new();
        for path in &files {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if let Some(target_name) = name.strip_suffix(".uid") {
                let text = std::fs::read_to_string(path).unwrap_or_default();
                let uid = text.trim();
                if uid.starts_with("uid://") {
                    let target = path.with_file_name(target_name);
                    analysis.add_uid(uid, &res_path(&index.root, &target));
                }
            } else if let Some(target_name) = name.strip_suffix(".import") {
                let text = std::fs::read_to_string(path).unwrap_or_default();
                if let Some(uid) = import_uid(&text) {
                    let target = path.with_file_name(target_name);
                    analysis.add_uid(&uid, &res_path(&index.root, &target));
                }
            } else if name.ends_with(".tscn") || name.ends_with(".tres") {
                let text = std::fs::read_to_string(path)
                    .map_err(|error| format!("{}: {}", path.display(), error))?;
                if let Some(header) = text.lines().next() {
                    if let Some(uid) = attribute(header, "uid") {
                        analysis.add_uid(&uid, &res_path(&index.root, path));
                    }
                }
                if name.ends_with(".tscn") {
                    scenes.push((res_path(&index.root, path), text));
                }
            }
        }
        for (scene, text) in &scenes {
            analysis.collect_scene(scene, text);
        }
        Ok(analysis)
    }

    fn add_uid(&mut self, uid: &str, path: &str) {
        self.uid_paths.insert(uid.to_string(), path.to_string());
        self.path_uids.insert(path.to_string(), uid.to_string());
    }

    /// Reads the external scripts, nodes and connections of one text scene.
    fn collect_scene(&mut self, scene: &str, text: &str) {
        let mut resources: HashMap<String, String> = HashMap::new();
        let mut node_scripts: HashMap<String, String> = HashMap::new();
        let mut current_node: Option<String> = None;
        let mut connections = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with('[') {
                current_node = None;
                if trimmed.starts_with("[ext_resource ") {
                    let Some(id) = attribute(trimmed, "id") else {
                        continue;
                    };
                    let path = match attribute(trimmed, "path") {
                        Some(path) => Some(path),
                        None => attribute(trimmed, "uid")
                            .and_then(|uid| self.uid_paths.get(&uid).cloned()),
                    };
                    if let Some(path) = path {
                        resources.insert(id, path);
                    }
                } else if trimmed.starts_with("[node ") {
                    let Some(name) = attribute(trimmed, "name") else {
                        continue;
                    };
                    current_node = Some(match attribute(trimmed, "parent") {
                        None => ".".to_string(),
                        Some(parent) if parent == "." => name,
                        Some(parent) => format!("{}/{}", parent, name),
                    });
                } else if trimmed.starts_with("[connection ") {
                    if let (Some(signal), Some(from), Some(to), Some(method)) = (
                        attribute(trimmed, "signal"),
                        attribute(trimmed, "from"),
                        attribute(trimmed, "to"),
                        attribute(trimmed, "method"),
                    ) {
                        self.used_signals.insert(signal.clone());
                        self.connected_methods.insert(method.clone());
                        connections.push((signal, from, to, method));
                    }
                }
            } else if let Some(node) = &current_node {
                if let Some(value) = trimmed.strip_prefix("script = ") {
                    if let Some(id) = ext_resource_id(value) {
                        node_scripts.insert(node.clone(), id);
                    }
                }
            }
        }
        if let Some(script) = node_scripts.get(".").and_then(|id| resources.get(id)) {
            self.scene_root_scripts
                .insert(scene.to_string(), script.clone());
        }
        for (signal, from, to, method) in connections {
            let target_script = node_scripts
                .get(&to)
                .and_then(|id| resources.get(id))
                .filter(|path| path.ends_with(".gd"))
                .cloned();
            self.scene_connections.push(SceneConnection {
                scene: scene.to_string(),
                signal,
                from,
                to,
                method,
                target_script,
            });
        }
    }
}

/// The value of `name="..."` in a scene or resource header line.
fn attribute(line: &str, name: &str) -> Option<String> {
    let pattern = format!("{}=\"", name);
    let mut search_from = 0;
    while let Some(offset) = line[search_from..].find(&pattern) {
        let start = search_from + offset;
        let preceded_by_space = start == 0 || line.as_bytes()[start - 1] == b' ';
        let value_start = start + pattern.len();
        if preceded_by_space {
            let mut value = String::new();
            let mut escaped = false;
            for character in line[value_start..].chars() {
                if escaped {
                    value.push(character);
                    escaped = false;
                } else if character == '\\' {
                    escaped = true;
                } else if character == '"' {
                    return Some(value);
                } else {
                    value.push(character);
                }
            }
            return None;
        }
        search_from = value_start;
    }
    None
}

/// The id in `ExtResource("1_abc")` (Godot 4) or `ExtResource( 1 )` (Godot 3).
fn ext_resource_id(value: &str) -> Option<String> {
    let inner = value
        .trim()
        .strip_prefix("ExtResource(")?
        .split(')')
        .next()?
        .trim()
        .trim_matches('"');
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// The `uid` of the `[remap]` section of an `.import` file.
fn import_uid(text: &str) -> Option<String> {
    let mut in_remap = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_remap = line == "[remap]";
            continue;
        }
        if in_remap {
            if let Some(value) = line.strip_prefix("uid=") {
                let uid = value.trim().trim_matches('"');
                if uid.starts_with("uid://") {
                    return Some(uid.to_string());
                }
            }
        }
    }
    None
}

/// `res://` path of a file under the project root.
pub(crate) fn res_path(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    format!("res://{}", relative.to_string_lossy().replace('\\', "/"))
}

/// Every file of the project, sorted, without tool and cache folders.
pub(crate) fn project_files(root: &Path) -> Result<Vec<PathBuf>, String> {
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
        } else if kind.is_file() {
            paths.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn reads_scene_attributes_and_resource_ids() {
        let line =
            r#"[ext_resource type="Script" uid="uid://abc" path="res://a \"b\".gd" id="1_x"]"#;
        assert_eq!(attribute(line, "path").unwrap(), "res://a \"b\".gd");
        assert_eq!(attribute(line, "id").unwrap(), "1_x");
        assert_eq!(attribute(line, "uid").unwrap(), "uid://abc");
        assert_eq!(attribute(line, "missing"), None);
        assert_eq!(
            attribute(r#"[node xname="a" name="b"]"#, "name").unwrap(),
            "b"
        );
        assert_eq!(ext_resource_id(r#"ExtResource("1_x")"#).unwrap(), "1_x");
        assert_eq!(ext_resource_id("ExtResource( 2 )").unwrap(), "2");
        assert_eq!(ext_resource_id("SubResource(\"x\")"), None);
        assert_eq!(
            import_uid("[remap]\nimporter=\"texture\"\nuid=\"uid://q1\"\n").unwrap(),
            "uid://q1"
        );
        assert_eq!(import_uid("[deps]\nuid=\"uid://q1\"\n"), None);
    }

    #[test]
    fn resolves_connection_target_scripts_by_node_path() {
        let mut analysis = ProjectAnalysis::default();
        analysis.collect_scene(
            "res://ui.tscn",
            concat!(
                "[gd_scene format=3]\n",
                "[ext_resource type=\"Script\" path=\"res://ui.gd\" id=\"1\"]\n",
                "[ext_resource type=\"Script\" path=\"res://button.gd\" id=\"2\"]\n",
                "[node name=\"UI\" type=\"Control\"]\n",
                "script = ExtResource(\"1\")\n",
                "[node name=\"Box\" type=\"VBoxContainer\" parent=\".\"]\n",
                "[node name=\"Button\" type=\"Button\" parent=\"Box\"]\n",
                "script = ExtResource(\"2\")\n",
                "[connection signal=\"pressed\" from=\"Box/Button\" to=\".\" method=\"_on_pressed\"]\n",
                "[connection signal=\"ready\" from=\".\" to=\"Box/Button\" method=\"_on_ready\"]\n",
                "[connection signal=\"ready\" from=\".\" to=\"Box\" method=\"hide\"]\n",
            ),
        );
        let targets: Vec<Option<&str>> = analysis
            .scene_connections
            .iter()
            .map(|connection| connection.target_script.as_deref())
            .collect();
        assert_eq!(
            targets,
            vec![Some("res://ui.gd"), Some("res://button.gd"), None]
        );
        assert_eq!(analysis.scene_root_scripts["res://ui.tscn"], "res://ui.gd");
        assert!(analysis.connected_methods.contains("_on_pressed"));
        assert!(analysis.used_signals.contains("pressed"));
    }
}
