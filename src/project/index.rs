//! Project discovery, content-hash summaries and conservative inheritance.
use super::{MemberLookup, ProjectIndex, ScriptSummary};
use crate::linter::cache::file_hash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

const INDEX_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct CachedScript {
    hash: String,
    summary: ScriptSummary,
    valid: bool,
}
#[derive(Default, Serialize, Deserialize)]
struct IndexCache {
    version: u32,
    files: BTreeMap<String, CachedScript>,
}

pub fn find_root(path: &Path) -> Option<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    let mut current = if absolute.is_dir() {
        Some(absolute.as_path())
    } else {
        absolute.parent()
    };
    while let Some(directory) = current {
        if directory.join("project.godot").is_file() {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

pub(super) fn build(root: &Path, with_cache: bool) -> Result<ProjectIndex, String> {
    let root =
        std::fs::canonicalize(root).map_err(|error| format!("{}: {}", root.display(), error))?;
    let project_file = root.join("project.godot");
    let project_text = std::fs::read_to_string(&project_file)
        .map_err(|error| format!("{}: {}", project_file.display(), error))?;
    let cache_path = root.join(".gdguard/cache/index.json");
    let mut previous: IndexCache = if with_cache {
        std::fs::read(&cache_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    } else {
        IndexCache::default()
    };
    if previous.version != INDEX_VERSION {
        previous.files.clear();
    }
    let mut paths = Vec::new();
    collect_scripts(&root, &mut paths)?;
    paths.sort();
    let mut cache = IndexCache {
        version: INDEX_VERSION,
        files: BTreeMap::new(),
    };
    let mut scripts = Vec::with_capacity(paths.len());
    let mut class_names = HashMap::new();
    let mut duplicate_names = HashSet::new();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_gdscript::LANGUAGE.into())
        .map_err(|error| error.to_string())?;
    for path in paths {
        let relative = path
            .strip_prefix(&root)
            .map_err(|error| error.to_string())?
            .to_path_buf();
        let key = relative.to_string_lossy().replace('\\', "/");
        let source = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {}", path.display(), error))?;
        let hash = file_hash(&source);
        let entry = match previous.files.remove(&key) {
            Some(entry) if entry.hash == hash => entry,
            _ => {
                let tree = parser
                    .parse(&source, None)
                    .ok_or("tree-sitter returned no project tree")?;
                let summary = summarize(relative, &source, &tree);
                CachedScript {
                    hash,
                    summary,
                    valid: !tree.root_node().has_error(),
                }
            }
        };
        if entry.valid {
            // A name declared twice is ambiguous: keep both scripts but leave
            // the name unresolved, so lookups through it return Unknown. The
            // `duplicate-class-name` rule reports it.
            if let Some(name) = &entry.summary.class_name {
                if class_names.insert(name.clone(), scripts.len()).is_some() {
                    duplicate_names.insert(name.clone());
                }
            }
            scripts.push(entry.summary.clone());
        }
        cache.files.insert(key, entry);
    }
    for name in &duplicate_names {
        class_names.remove(name);
    }
    // Cache errors cost performance, not correctness. Stable path order and
    // a unique temporary file keep concurrent lint processes independent.
    if let Some(directory) = cache_path.parent().filter(|_| with_cache) {
        if std::fs::create_dir_all(directory).is_ok() {
            let _ = std::fs::write(directory.join(".gitignore"), "*\n");
            if let Ok(bytes) = serde_json::to_vec(&cache) {
                let temporary = directory.join(format!("index-{}.tmp", std::process::id()));
                if std::fs::write(&temporary, bytes).is_ok() {
                    let _ = std::fs::rename(&temporary, &cache_path);
                }
                let _ = std::fs::remove_file(temporary);
            }
        }
    }
    Ok(ProjectIndex {
        root,
        scripts,
        class_names,
        autoloads: parse_autoloads(&project_text),
    })
}

fn collect_scripts(directory: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in std::fs::read_dir(directory)
        .map_err(|error| format!("{}: {}", directory.display(), error))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if kind.is_dir() {
            let name = entry.file_name();
            if !matches!(
                name.to_str(),
                Some(".git" | ".godot" | ".gdguard" | "node_modules" | "target")
            ) {
                collect_scripts(&path, paths)?;
            }
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "gd") {
            paths.push(path);
        }
    }
    Ok(())
}

fn summarize(path: PathBuf, source: &str, tree: &tree_sitter::Tree) -> ScriptSummary {
    let mut summary = ScriptSummary {
        path,
        class_name: None,
        extends: None,
        functions: Vec::new(),
        signals: Vec::new(),
        variables: Vec::new(),
    };
    let root = tree.root_node();
    for index in 0..root.named_child_count() {
        let Some(node) = root.named_child(index as u32) else {
            continue;
        };
        if node.kind() == "class_name_statement" {
            summary.class_name = node
                .child_by_field_name("name")
                .map(|name| source[name.byte_range()].to_string());
            if let Some(extends) = node.child_by_field_name("extends") {
                summary.extends = extends_text(extends, source);
            }
        } else if node.kind() == "extends_statement" {
            summary.extends = extends_text(node, source);
        }
        if let Some(name) = node.child_by_field_name("name") {
            let name = source[name.byte_range()].to_string();
            match node.kind() {
                "function_definition" => summary.functions.push(name),
                "signal_statement" => summary.signals.push(name),
                "variable_statement" | "const_statement" | "enum_definition"
                | "class_definition" => summary.variables.push(name),
                _ => {}
            }
        }
    }
    summary
}

fn extends_text(node: tree_sitter::Node, source: &str) -> Option<String> {
    for index in 0..node.named_child_count() {
        if let Some(child) = node.named_child(index as u32) {
            if child.kind() != "annotations" {
                return Some(
                    source[child.byte_range()]
                        .trim_matches(['\'', '"'])
                        .to_string(),
                );
            }
        }
    }
    None
}

fn parse_autoloads(source: &str) -> HashMap<String, PathBuf> {
    let mut autoloads = HashMap::new();
    let mut in_autoload = false;
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_autoload = line == "[autoload]";
            continue;
        }
        if !in_autoload || line.starts_with([';', '#']) {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            if let Ok(value) = serde_json::from_str::<String>(value.trim()) {
                let value = value.trim_start_matches('*').trim_start_matches("res://");
                autoloads.insert(
                    name.trim().trim_matches('"').to_string(),
                    PathBuf::from(value),
                );
            }
        }
    }
    autoloads
}

pub(super) fn has_member(index: &ProjectIndex, class_name: &str, member: &str) -> MemberLookup {
    let Some(current) = index.class_names.get(class_name).copied() else {
        return MemberLookup::Unknown;
    };
    lookup_from(index, current, member)
}

pub(super) fn has_member_in_script(
    index: &ProjectIndex,
    path: &Path,
    member: &str,
) -> MemberLookup {
    let path = path
        .to_str()
        .and_then(|text| text.strip_prefix("res://"))
        .map(PathBuf::from)
        .unwrap_or_else(|| path.to_path_buf());
    let Some(current) = index.scripts.iter().position(|script| script.path == path) else {
        return MemberLookup::Unknown;
    };
    lookup_from(index, current, member)
}

fn lookup_from(index: &ProjectIndex, mut current: usize, member: &str) -> MemberLookup {
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current) {
            return MemberLookup::Unknown;
        }
        let Some(script) = index.scripts.get(current) else {
            return MemberLookup::Unknown;
        };
        if script
            .functions
            .iter()
            .chain(&script.signals)
            .chain(&script.variables)
            .any(|name| name == member)
        {
            return MemberLookup::Yes;
        }
        let Some(base) = &script.extends else {
            if super::implicit_base::MEMBERS.binary_search(&member).is_ok()
                || script
                    .functions
                    .iter()
                    .any(|name| name == "_get" || name == "_set")
            {
                return MemberLookup::Unknown;
            }
            return MemberLookup::No;
        };
        if let Some(parent) = index.class_names.get(base) {
            current = *parent;
            continue;
        }
        if base.contains('/') || base.ends_with(".gd") {
            let path = if let Some(relative) = base.strip_prefix("res://") {
                PathBuf::from(relative)
            } else {
                script.path.parent().unwrap_or(Path::new("")).join(base)
            };
            let mut normalized = PathBuf::new();
            for component in path.components() {
                match component {
                    Component::ParentDir => {
                        normalized.pop();
                    }
                    Component::CurDir => {}
                    other => normalized.push(other.as_os_str()),
                }
            }
            if let Some(parent) = index
                .scripts
                .iter()
                .position(|script| script.path == normalized)
            {
                current = parent;
                continue;
            }
        }
        return MemberLookup::Unknown;
    }
}

/// Run-wide dependency digest for project-aware lint cache entries.
pub fn fingerprint(index: &ProjectIndex) -> Result<String, String> {
    let mut parts = Vec::with_capacity(index.scripts.len() * 2 + 1);
    parts.push(std::fs::read(index.root.join("project.godot")).map_err(|error| error.to_string())?);
    for script in &index.scripts {
        parts.push(script.path.to_string_lossy().as_bytes().to_vec());
        let bytes =
            std::fs::read(index.root.join(&script.path)).map_err(|error| error.to_string())?;
        parts.push(bytes);
    }
    // Scenes, UID sources and the set of existing files feed scene, UID and
    // resource-path rules, so any change to them invalidates cached results.
    for file in super::analysis::project_files(&index.root)? {
        parts.push(file.to_string_lossy().as_bytes().to_vec());
        let name = file.to_string_lossy();
        if name.ends_with(".tscn")
            || name.ends_with(".tres")
            || name.ends_with(".uid")
            || name.ends_with(".import")
        {
            parts.push(std::fs::read(&file).map_err(|error| error.to_string())?);
        }
    }
    let mut slices = Vec::with_capacity(parts.len());
    for part in &parts {
        slices.push(part.as_slice());
    }
    Ok(crate::linter::cache::stable_hash(&slices))
}
