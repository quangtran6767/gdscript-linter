//! Frozen SM0 project contracts. W6 fills the index and inheritance lookup.

mod index;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
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
    /// SM0 returns an empty index. W6 discovers scripts and caches summaries.
    pub fn build(root: &Path) -> Result<ProjectIndex, String> {
        Ok(ProjectIndex {
            root: root.to_path_buf(),
            scripts: Vec::new(),
            class_names: HashMap::new(),
            autoloads: HashMap::new(),
        })
    }

    /// Unknown includes missing classes and inheritance reaching engine types.
    pub fn has_member(&self, _class_name: &str, _member: &str) -> MemberLookup {
        MemberLookup::Unknown
    }
}
