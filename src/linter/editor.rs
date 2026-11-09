//! Godot-facing data and config session, independent of engine initialization.
//! The native wrapper converts these exact payloads to Dictionaries.
use super::api::{self, Config};
use super::diagnostic::{Diagnostic, Edit};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
pub struct EditorFix {
    pub description: String,
    pub safety: String,
    pub edits: Vec<Edit>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct EditorDiagnostic {
    pub rule: String,
    pub check: Option<String>,
    pub severity: String,
    pub message: String,
    pub help: Option<String>,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub start_byte: usize,
    pub end_byte: usize,
    pub fixes: Vec<EditorFix>,
}

impl EditorDiagnostic {
    pub fn check_id(&self) -> &str {
        self.check.as_deref().unwrap_or(&self.rule)
    }
}
#[derive(Debug, Clone, PartialEq)]
pub struct EditorFixResult {
    pub output: String,
    pub applied: usize,
    pub remaining: Vec<EditorDiagnostic>,
}
struct CachedConfig {
    hash: Option<String>,
    config: Config,
}
#[derive(Default)]
pub struct EditorSession {
    configs: HashMap<PathBuf, CachedConfig>,
}
impl EditorSession {
    fn config(&mut self, path: &Path) -> Result<&Config, String> {
        let file = super::config::find_config_file(path);
        let key = file
            .clone()
            .unwrap_or_else(|| path.parent().unwrap_or(Path::new(".")).to_path_buf());
        let hash = match &file {
            Some(file) => Some(super::cache::file_hash(
                &std::fs::read_to_string(file).map_err(|error| error.to_string())?,
            )),
            None => None,
        };
        if self
            .configs
            .get(&key)
            .is_none_or(|cached| cached.hash != hash)
        {
            let config = api::load_config_for(path).map_err(|error| error.to_string())?;
            self.configs
                .insert(key.clone(), CachedConfig { hash, config });
        }
        self.configs
            .get(&key)
            .map(|cached| &cached.config)
            .ok_or_else(|| "lost cached editor config".to_string())
    }
    pub fn lint(&mut self, path: &Path, source: &str) -> Result<Vec<EditorDiagnostic>, String> {
        let config = self.config(path)?;
        let lint = api::lint_source(path, source, config)?;
        Ok(payloads(&lint.diagnostics))
    }
    pub fn fix(
        &mut self,
        path: &Path,
        source: &str,
        unsafe_fixes: bool,
    ) -> Result<EditorFixResult, String> {
        let config = self.config(path)?;
        let fixed = api::fix_source(path, source, config, unsafe_fixes)?;
        Ok(EditorFixResult {
            output: fixed.output,
            applied: fixed.applied,
            remaining: payloads(&fixed.remaining),
        })
    }
}

pub fn payloads(diagnostics: &[Diagnostic]) -> Vec<EditorDiagnostic> {
    let mut payloads = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics {
        let mut fixes = Vec::with_capacity(diagnostic.fixes.len());
        for fix in &diagnostic.fixes {
            fixes.push(EditorFix {
                description: fix.description.clone(),
                safety: fix.safety.as_str().to_string(),
                edits: fix.edits.clone(),
            });
        }
        payloads.push(EditorDiagnostic {
            rule: diagnostic.rule.clone(),
            check: diagnostic.check.clone(),
            severity: diagnostic.severity.as_str().to_string(),
            message: diagnostic.message.clone(),
            help: diagnostic.help.clone(),
            line: diagnostic.span.start.line,
            column: diagnostic.span.start.column,
            end_line: diagnostic.span.end.line,
            end_column: diagnostic.span.end.column,
            start_byte: diagnostic.span.start_byte,
            end_byte: diagnostic.span.end_byte,
            fixes,
        });
    }
    payloads
}

/// Applies reviewed editor edits together and validates the resulting syntax.
pub fn apply_edits_checked(source: &str, edits: &[Edit]) -> Result<String, String> {
    let output = super::diagnostic::apply_edits(source, edits)?;
    let tree = super::rule_tester::parse(&output)?;
    if tree.root_node().has_error() {
        return Err(
            "The edit was rejected because the resulting GDScript does not parse.".to_string(),
        );
    }
    Ok(output)
}
