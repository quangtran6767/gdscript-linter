//! Library API used by the CLI, the LSP server and the Godot gdextension, so
//! none of them depend on CLI internals.
//!
//! FROZEN after W0: the function signatures and the public types below only
//! change additively. Track A fills in the function bodies (W1 to W3).
//! See `docs/specification_linter.md`, section "Library API".

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::linter::config::{self, CliRuleOverrides, CompiledConfig};
use crate::linter::custom;
use crate::linter::diagnostic::Diagnostic;
use crate::linter::fix;
use crate::linter::{GDScriptLinter, LinterConfig, lint_with_settings};

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
///
/// Searches upward from `path` (a file or a folder). Checks every key and
/// level, and that every rule id is a built-in or custom rule.
pub fn load_config_for(path: &Path) -> Result<Config, ConfigError> {
    config::load_config_for(path)
}

/// Lints one in-memory buffer. `path` is used for config overrides and output.
///
/// Applies `config` the same way as the CLI: `[rules]` levels and options,
/// matching `[[overrides]]`, then `.editorconfig` for the line length and
/// indent size. `[lint].exclude` is not checked here: the caller asked for
/// this buffer.
pub fn lint_source(path: &Path, source: &str, config: &Config) -> Result<LintResult, String> {
    let compiled_config = CompiledConfig::new(config.clone()).map_err(|error| error.to_string())?;
    let settings = compiled_config.settings_for_file(path, &CliRuleOverrides::default());
    let mut providers = custom::load_providers(config)?;
    let mut linter = GDScriptLinter::new(LinterConfig::default())?;
    let diagnostics = lint_with_settings(&mut linter, path, source, &settings, &mut providers)?;
    Ok(LintResult { diagnostics })
}

/// Runs the fix engine on one buffer without touching the disk.
///
/// Applies `Safe` fixes, plus `Unsafe` ones when `unsafe_fixes` is true, over
/// several passes; fixes that would break the code are dropped and reported
/// as `fix-failed` in `remaining`. Formats the output when `source` was
/// formatted.
pub fn fix_source(
    path: &Path,
    source: &str,
    config: &Config,
    unsafe_fixes: bool,
) -> Result<FixResult, String> {
    let compiled_config = CompiledConfig::new(config.clone()).map_err(|error| error.to_string())?;
    let settings = compiled_config.settings_for_file(path, &CliRuleOverrides::default());
    let mut providers = custom::load_providers(config)?;
    let mut linter = GDScriptLinter::new(LinterConfig::default())?;
    let fixed = fix::fix_source_with_settings(
        &mut linter,
        path,
        source,
        &settings,
        &mut providers,
        unsafe_fixes,
    )?;
    Ok(FixResult {
        output: fixed.output,
        applied: fixed.applied,
        remaining: fixed.remaining,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::linter::diagnostic::Severity;

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
    fn fix_source_applies_safe_fixes_and_reports_the_rest() {
        let source = "func f(unused):\n\tprint(1)\n\tpass\n";
        let result = fix_source(Path::new("a.gd"), source, &Config::default(), false).unwrap();
        assert_eq!(result.output, "func f(unused):\n\tprint(1)\n");
        assert_eq!(result.applied, 1);
        assert_eq!(result.remaining.len(), 1);
        assert_eq!(result.remaining[0].rule, "unused-argument");
    }

    #[test]
    fn fix_source_applies_unsafe_fixes_only_when_asked() {
        let source = "func f(unused):\n\tprint(1)\n";
        let result = fix_source(Path::new("a.gd"), source, &Config::default(), true).unwrap();
        assert_eq!(result.output, "func f(_unused):\n\tprint(1)\n");
        assert_eq!(result.applied, 1);
        assert!(result.remaining.is_empty());
    }

    fn rule_level(rule_id: &str, level: RuleLevel) -> Config {
        let mut rules = BTreeMap::new();
        rules.insert(
            rule_id.to_string(),
            RuleSetting {
                level: Some(level),
                options: toml::Table::new(),
            },
        );
        Config {
            rules,
            ..Config::default()
        }
    }

    #[test]
    fn lint_source_reports_builtin_rules_with_spans() {
        let source = "func badName(unused):\n\tpass\n";
        let result = lint_source(Path::new("a.gd"), source, &Config::default()).unwrap();
        assert_eq!(result.diagnostics.len(), 2);

        let function_name = &result.diagnostics[0];
        assert_eq!(function_name.rule, "function-name");
        assert_eq!(function_name.severity, Severity::Error);
        assert_eq!(
            &source[function_name.span.start_byte..function_name.span.end_byte],
            "badName"
        );
        assert_eq!(
            function_name.help.as_deref(),
            Some("Rename it to 'bad_name'.")
        );

        let unused_argument = &result.diagnostics[1];
        assert_eq!(unused_argument.rule, "unused-argument");
        assert_eq!(unused_argument.severity, Severity::Warning);
        assert_eq!(unused_argument.span.start.line, 1);
        assert_eq!(unused_argument.span.start.column, 14);
    }

    #[test]
    fn lint_source_applies_rule_levels() {
        let source = "func badName(unused):\n\tpass\n";
        let path = Path::new("a.gd");

        let disabled =
            lint_source(path, source, &rule_level("function-name", RuleLevel::Off)).unwrap();
        assert_eq!(disabled.diagnostics.len(), 1);
        assert_eq!(disabled.diagnostics[0].rule, "unused-argument");

        let as_error = lint_source(
            path,
            source,
            &rule_level("unused-argument", RuleLevel::Error),
        )
        .unwrap();
        assert_eq!(as_error.diagnostics[1].rule, "unused-argument");
        assert_eq!(as_error.diagnostics[1].severity, Severity::Error);

        let as_info =
            lint_source(path, source, &rule_level("function-name", RuleLevel::Info)).unwrap();
        assert_eq!(as_info.diagnostics[0].severity, Severity::Info);
    }

    #[test]
    fn lint_source_uses_max_line_length() {
        let source = "var text = \"0123456789\"\n";
        let path = Path::new("a.gd");
        let default_result = lint_source(path, source, &Config::default()).unwrap();
        assert!(default_result.diagnostics.is_empty());

        let config = Config {
            max_line_length: Some(20),
            ..Config::default()
        };
        let short_result = lint_source(path, source, &config).unwrap();
        assert_eq!(short_result.diagnostics.len(), 1);
        assert_eq!(short_result.diagnostics[0].rule, "max-line-length");
        assert_eq!(short_result.diagnostics[0].span.start.column, 21);
    }

    #[test]
    fn lint_source_respects_ignore_comments() {
        let source = "# gdlint-ignore-next-line function-name\nfunc badName():\n\tpass\n";
        let result = lint_source(Path::new("a.gd"), source, &Config::default()).unwrap();
        assert!(result.diagnostics.is_empty());
    }
}
