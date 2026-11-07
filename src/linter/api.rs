//! Library API used by the CLI, the LSP server and the Godot gdextension, so
//! none of them depend on CLI internals.
//!
//! FROZEN after W0: the function signatures and the public types below only
//! change additively. Track A fills in the function bodies (W1 to W3).
//! See `docs/specification_linter.md`, section "Library API".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::linter::diagnostic::Diagnostic;

/// The level of a rule in `gdguard.toml`: `"off"`, `"info"`, `"warning"` or
/// `"error"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleLevel {
    Off,
    Info,
    Warning,
    Error,
}

impl RuleLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleLevel::Off => "off",
            RuleLevel::Info => "info",
            RuleLevel::Warning => "warning",
            RuleLevel::Error => "error",
        }
    }

    pub fn from_name(name: &str) -> Option<RuleLevel> {
        match name {
            "off" => Some(RuleLevel::Off),
            "info" => Some(RuleLevel::Info),
            "warning" => Some(RuleLevel::Warning),
            "error" => Some(RuleLevel::Error),
            _ => None,
        }
    }
}

/// One entry of the `[rules]` table (or of an override's `rules` table).
///
/// `unused-argument = "error"` gives `level: Some(Error)` and no options.
/// `max-line-length = { level = "warning", max = 120 }` gives
/// `level: Some(Warning)` and `options = { max = 120 }`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RuleSetting {
    /// `None` keeps the rule's default severity.
    pub level: Option<RuleLevel>,
    /// Every key of the table form except `level`. Passed to the rule as is
    /// (JS rules see it as `context.options`).
    pub options: toml::Table,
}

/// One `[[overrides]]` entry: rule settings for files matching `files`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConfigOverride {
    /// Glob patterns, relative to the folder of the config file.
    pub files: Vec<String>,
    pub rules: BTreeMap<String, RuleSetting>,
}

/// The resolved linter configuration for one project or folder.
///
/// Everything is `pub` and `Default` so tests can build a `Config` by hand:
/// `Config { custom_rule_paths: vec![dir], ..Config::default() }`.
/// Maps are `BTreeMap` so iteration order, and the config hash used by the
/// lint cache, are stable.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Config {
    /// The `gdguard.toml` this config was loaded from. `None` means defaults
    /// (no config file found). Relative globs resolve against its folder.
    pub config_path: Option<PathBuf>,
    /// `[rules]`: rule id to level and options, for built-in and custom rules.
    pub rules: BTreeMap<String, RuleSetting>,
    /// `[custom_rules].paths`: folders of rule files, already resolved
    /// relative to the config file's folder.
    pub custom_rule_paths: Vec<PathBuf>,
    /// `[lint].exclude`: glob patterns of files to skip.
    pub exclude: Vec<String>,
    /// `[lint].baseline`: path of the baseline file, resolved relative to the
    /// config file's folder.
    pub baseline: Option<PathBuf>,
    /// `[[overrides]]`, in file order. Later entries win.
    pub overrides: Vec<ConfigOverride>,
    /// `None` means: use `.editorconfig`, then the default (100).
    pub max_line_length: Option<usize>,
    /// `None` means: use `.editorconfig`, then the default (4).
    pub indent_size: Option<usize>,
}

/// A config or rule-file error. The CLI prints it and exits with code 2.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConfigError {
    /// The file the error is in, when there is one.
    pub path: Option<PathBuf>,
    /// The key the error is about, for example `rules.unused-argumnet`.
    pub key: Option<String>,
    pub message: String,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(path) = &self.path {
            write!(formatter, "{}: ", path.display())?;
        }
        if let Some(key) = &self.key {
            write!(formatter, "`{}`: ", key)?;
        }
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ConfigError {}

impl From<&str> for ConfigError {
    fn from(message: &str) -> Self {
        ConfigError {
            path: None,
            key: None,
            message: message.to_string(),
        }
    }
}

impl From<String> for ConfigError {
    fn from(message: String) -> Self {
        ConfigError {
            path: None,
            key: None,
            message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LintResult {
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FixResult {
    /// The source after applying fixes.
    pub output: String,
    /// How many fixes were applied.
    pub applied: usize,
    /// Diagnostics left after fixing.
    pub remaining: Vec<Diagnostic>,
}

/// Loads the nearest gdguard.toml for `path` (or defaults) plus custom rules.
pub fn load_config_for(_path: &Path) -> Result<Config, ConfigError> {
    Err("not implemented yet".into())
}

/// Lints one in-memory buffer. `path` is used for config overrides and output.
pub fn lint_source(_path: &Path, _source: &str, _config: &Config) -> Result<LintResult, String> {
    Err("not implemented yet".into())
}

/// Runs the fix engine on one buffer without touching the disk.
pub fn fix_source(
    _path: &Path,
    _source: &str,
    _config: &Config,
    _unsafe_fixes: bool,
) -> Result<FixResult, String> {
    Err("not implemented yet".into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn config_can_be_built_by_hand() {
        let mut rules = BTreeMap::new();
        rules.insert(
            "unused-argument".to_string(),
            RuleSetting {
                level: Some(RuleLevel::Error),
                options: toml::Table::new(),
            },
        );
        let config = Config {
            rules,
            custom_rule_paths: vec![PathBuf::from(".gdguard/rules")],
            ..Config::default()
        };
        assert_eq!(config.custom_rule_paths.len(), 1);
        assert_eq!(config.max_line_length, None);
        assert!(config.config_path.is_none());
    }

    #[test]
    fn rule_level_names_round_trip() {
        for level in [
            RuleLevel::Off,
            RuleLevel::Info,
            RuleLevel::Warning,
            RuleLevel::Error,
        ] {
            assert_eq!(RuleLevel::from_name(level.as_str()), Some(level));
        }
        assert_eq!(RuleLevel::from_name("warn"), None);
    }

    #[test]
    fn config_error_display_names_file_and_key() {
        let error = ConfigError {
            path: Some(PathBuf::from("gdguard.toml")),
            key: Some("rules.unused-argumnet".to_string()),
            message: "unknown rule id".to_string(),
        };
        assert_eq!(
            error.to_string(),
            "gdguard.toml: `rules.unused-argumnet`: unknown rule id"
        );
    }

    #[test]
    fn stubs_return_errors_instead_of_panicking() {
        let path = Path::new("a.gd");
        let config = Config::default();
        assert!(load_config_for(path).is_err());
        assert!(lint_source(path, "", &config).is_err());
        assert!(fix_source(path, "", &config, false).is_err());
    }
}
