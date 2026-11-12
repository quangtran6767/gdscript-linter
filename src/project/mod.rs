//! Frozen SM0 project contracts. W6 fills the index and inheritance lookup.

mod analysis;
pub use analysis::{ProjectAnalysis, SceneConnection};
mod implicit_base;
mod index;
pub use index::{find_root, fingerprint};

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScriptSummary {
    /// Path relative to the project root (without a literal res:// prefix).
    pub path: PathBuf,
    pub class_name: Option<String>,
    pub extends: Option<String>,
    pub functions: Vec<String>,
    pub signals: Vec<String>,
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberLookup {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ProjectIndex {
    pub root: PathBuf,
    pub scripts: Vec<ScriptSummary>,
    pub class_names: HashMap<String, usize>,
    pub autoloads: HashMap<String, PathBuf>,
}

impl ProjectIndex {
    /// Summarizes a Godot project, reusing content-hash cache entries.
    pub fn build(root: &Path) -> Result<ProjectIndex, String> {
        index::build(root, true)
    }

    /// Builds without reading or writing project summaries (`lint --no-cache`).
    pub fn build_uncached(root: &Path) -> Result<ProjectIndex, String> {
        index::build(root, false)
    }

    /// Unknown includes missing classes, cycles, and chains reaching engine types.
    pub fn has_member(&self, class_name: &str, member: &str) -> MemberLookup {
        index::has_member(self, class_name, member)
    }

    /// Like `has_member`, starting from the script at `path` (project-relative
    /// or `res://`). Unknown when the path isn't a valid indexed script.
    pub fn has_member_in_script(&self, path: &Path, member: &str) -> MemberLookup {
        index::has_member_in_script(self, path, member)
    }

    /// Looks for a method in the script ancestors of an explicit base.
    /// `path` is the project-relative path of the declaring script, used to
    /// resolve relative bases. Supplying the base supports unsaved buffers.
    /// Only function declarations count; unresolved bases return Unknown.
    pub fn has_method_in_base(&self, path: &Path, base: &str, method: &str) -> MemberLookup {
        index::has_method_in_base(self, path, base, method)
    }

    /// Paths of the valid scripts declaring `class_name`, in path order.
    /// More than one path means the name is ambiguous and unresolved.
    pub fn scripts_declaring(&self, class_name: &str) -> Vec<&Path> {
        let mut paths = Vec::new();
        for script in &self.scripts {
            if script.class_name.as_deref() == Some(class_name) {
                paths.push(script.path.as_path());
            }
        }
        paths
    }
}
