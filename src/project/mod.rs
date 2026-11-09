//! Frozen SM0 project contracts. W6 fills the index and inheritance lookup.

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
        index::build(root)
    }

    /// Unknown includes missing classes, cycles, and chains reaching engine types.
    pub fn has_member(&self, class_name: &str, member: &str) -> MemberLookup {
        index::has_member(self, class_name, member)
    }
}
