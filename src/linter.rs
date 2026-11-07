use crate::node_kind::GDScriptNodeKind;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter::{Node, Parser, Tree};

// Shared contracts (frozen after W0).
pub mod api;
pub mod diagnostic;

pub mod ignore_patterns;
pub mod lib;
pub mod regex_patterns;
pub mod rule_config;
pub mod rules;

// Track A modules.
pub mod baseline;
pub mod cache;
pub mod config;
pub mod fix;
pub mod output;

// Track B modules.
pub mod custom;
pub mod explain;
pub mod init;
pub mod rule_tester;
pub mod rules_meta;

#[cfg(test)]
mod tests;

use api::{Config, RuleLevel};
use config::{CliRuleOverrides, CompiledConfig, FileSettings};
use diagnostic::{Diagnostic, FileContext, RuleProvider, Severity};
use ignore_patterns::{parse_ignore_patterns, should_ignore_rule};
use output::{FileDiagnostics, LintReport, display_path};
use rules::{ALL_RULES, Rule};

/// Formats one diagnostic in the default text output:
/// `path:line:rule:severity: message`.
pub fn format_diagnostic_line(file_path: &str, diagnostic: &Diagnostic) -> String {
    format!(
        "{}:{}:{}:{}: {}",
        file_path,
        diagnostic.span.start.line,
        diagnostic.rule,
        diagnostic.severity.as_str(),
        diagnostic.message
    )
}

#[derive(Debug, Clone)]
pub struct LinterConfig {
    pub disabled_rules: HashSet<String>,
    pub max_line_length: usize,
    pub indent_size: usize,
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            disabled_rules: HashSet::new(),
            max_line_length: 100,
            indent_size: 4,
        }
    }
}

pub struct GDScriptLinter {
    config: LinterConfig,
    parser: Parser,
}

impl GDScriptLinter {
    pub fn new(config: LinterConfig) -> Result<Self, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_gdscript::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set language: {}", e))?;
        GDScriptNodeKind::populate_lookup_table();
        Ok(Self { config, parser })
    }

    /// Parses and lints one file with the built-in rules. Returns the
    /// diagnostics left after inline ignore comments, sorted by position.
    pub fn lint(&mut self, source_code: &str, file_path: &str) -> Result<Vec<Diagnostic>, String> {
        self.lint_with_providers(source_code, file_path, &mut [])
    }

    /// Parses one file once, runs the built-in rules and then every custom
    /// rule provider on the same tree. Inline ignore comments apply to both.
    /// Returns the diagnostics sorted by position.
    pub fn lint_with_providers(
        &mut self,
        source_code: &str,
        file_path: &str,
        providers: &mut [Box<dyn RuleProvider>],
    ) -> Result<Vec<Diagnostic>, String> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or("Failed to parse GDScript code")?;

        let mut diagnostics = Vec::new();
        run_builtin_rules(&self.config, &tree, source_code, &mut diagnostics);
        if !providers.is_empty() {
            let context = FileContext {
                path: Path::new(file_path),
                source: source_code,
                tree: &tree,
            };
            for provider in providers.iter_mut() {
                diagnostics.extend(provider.check_file(&context));
            }
        }

        // Inline ignore comments apply to the line where a diagnostic starts.
        let ignore_map = parse_ignore_patterns(source_code);
        if !ignore_map.is_empty() {
            diagnostics.retain(|diagnostic| {
                !should_ignore_rule(&ignore_map, diagnostic.span.start.line, &diagnostic.rule)
            });
        }

        // Sort by position. Rules that run on the source code like the line
        // length check would otherwise appear at the end.
        diagnostics.sort_by(|a, b| {
            a.span
                .start
                .line
                .cmp(&b.span.start.line)
                .then(a.span.start.column.cmp(&b.span.start.column))
        });

        Ok(diagnostics)
    }

    /// Returns true when `source_code` has a syntax error: an `ERROR` or
    /// `MISSING` node in its tree.
    pub fn has_syntax_errors(&mut self, source_code: &str) -> Result<bool, String> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or("Failed to parse GDScript code")?;
        Ok(tree.root_node().has_error())
    }

    /// Replaces the built-in rule settings, for example when the next file
    /// has a different config.
    pub fn set_config(&mut self, config: LinterConfig) {
        self.config = config;
    }
}

/// Applies the levels from the config to a file's diagnostics: `off` removes
/// them, other levels replace their severity.
pub fn apply_rule_levels(levels: &BTreeMap<String, RuleLevel>, diagnostics: &mut Vec<Diagnostic>) {
    if levels.is_empty() {
        return;
    }
    diagnostics.retain(|diagnostic| levels.get(&diagnostic.rule) != Some(&RuleLevel::Off));
    for diagnostic in diagnostics.iter_mut() {
        match levels.get(&diagnostic.rule) {
            Some(RuleLevel::Error) => diagnostic.severity = Severity::Error,
            Some(RuleLevel::Warning) => diagnostic.severity = Severity::Warning,
            Some(RuleLevel::Info) => diagnostic.severity = Severity::Info,
            Some(RuleLevel::Off) | None => {}
        }
    }
}

/// Lints one buffer with the settings resolved for its file. This is the
/// per-file pipeline shared by the CLI and `api::lint_source`.
pub fn lint_with_settings(
    linter: &mut GDScriptLinter,
    path: &Path,
    source_code: &str,
    settings: &FileSettings,
    providers: &mut [Box<dyn RuleProvider>],
) -> Result<Vec<Diagnostic>, String> {
    linter.set_config(settings.linter_config.clone());
    let mut diagnostics =
        linter.lint_with_providers(source_code, &path.to_string_lossy(), providers)?;
    apply_rule_levels(&settings.levels, &mut diagnostics);
    Ok(diagnostics)
}

/// What the `lint` command needs besides the files.
#[derive(Debug, Clone, Default)]
pub struct LintRunOptions {
    /// `--config`: use this file for every linted file instead of searching
    /// upward from each one.
    pub config_path: Option<PathBuf>,
    /// `--disable` and `--max-line-length`.
    pub cli: CliRuleOverrides,
}

/// A loaded config with its custom rule providers.
struct LoadedConfig {
    compiled: CompiledConfig,
    providers: Vec<Box<dyn RuleProvider>>,
}

/// Loads and caches configs during a run, keyed by config file path, so each
/// `gdguard.toml` and its custom rules are read and validated once.
struct ConfigCache {
    configs: HashMap<PathBuf, LoadedConfig>,
}

impl ConfigCache {
    /// Returns the config that applies to `file_path`: the `--config` file,
    /// the nearest `gdguard.toml`, or the defaults.
    fn config_for(
        &mut self,
        file_path: &Path,
        options: &LintRunOptions,
    ) -> Result<&mut LoadedConfig, String> {
        let config_path = match &options.config_path {
            Some(config_path) => Some(config_path.clone()),
            None => config::find_config_file(file_path),
        };
        // The default config uses an empty path as its key.
        let cache_key = match &config_path {
            Some(config_path) => config_path.clone(),
            None => PathBuf::new(),
        };
        if !self.configs.contains_key(&cache_key) {
            let (config, providers) = match &config_path {
                Some(config_path) => {
                    config::load_with_providers(config_path).map_err(|error| error.to_string())?
                }
                None => (Config::default(), Vec::new()),
            };
            check_disabled_rule_ids(&options.cli, &providers)?;
            let compiled = CompiledConfig::new(config).map_err(|error| error.to_string())?;
            self.configs.insert(
                cache_key.clone(),
                LoadedConfig {
                    compiled,
                    providers,
                },
            );
        }
        match self.configs.get_mut(&cache_key) {
            Some(loaded) => Ok(loaded),
            None => Err("config cache lost an entry".to_string()),
        }
    }
}

/// Checks that every `--disable` name is a built-in rule or one of the
/// config's custom rules.
fn check_disabled_rule_ids(
    cli: &CliRuleOverrides,
    providers: &[Box<dyn RuleProvider>],
) -> Result<(), String> {
    let builtin_ids = config::builtin_rule_ids();
    let mut invalid_ids: Vec<&str> = Vec::new();
    for rule_id in &cli.disabled_rules {
        if builtin_ids.contains(&rule_id.as_str()) {
            continue;
        }
        let mut is_custom = false;
        for provider in providers {
            for meta in provider.rules() {
                if meta.id == *rule_id {
                    is_custom = true;
                }
            }
        }
        if !is_custom {
            invalid_ids.push(rule_id);
        }
    }
    if invalid_ids.is_empty() {
        return Ok(());
    }
    invalid_ids.sort();
    Err(format!(
        "Invalid rule names: {}\nUse --list-rules to see all available rules",
        invalid_ids.join(", ")
    ))
}

/// Lints files from disk. Each file uses its nearest `gdguard.toml` (or
/// `--config`); files matching `[lint].exclude` are skipped. Config errors
/// and unreadable files return an error (exit code 2).
pub fn lint_files(files: &[PathBuf], options: &LintRunOptions) -> Result<LintReport, String> {
    let mut linter = GDScriptLinter::new(LinterConfig::default())?;
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let mut report = LintReport::default();
    for file_path in files {
        let loaded = config_cache.config_for(file_path, options)?;
        if loaded.compiled.is_excluded(file_path) {
            continue;
        }
        let settings = loaded.compiled.settings_for_file(file_path, &options.cli);
        let source_code = fs::read_to_string(file_path)
            .map_err(|error| format!("Failed to read file {}: {}", file_path.display(), error))?;
        let diagnostics = lint_with_settings(
            &mut linter,
            file_path,
            &source_code,
            &settings,
            &mut loaded.providers,
        )?;
        report.files_checked += 1;
        if !diagnostics.is_empty() {
            report.files.push(FileDiagnostics {
                path: display_path(file_path),
                source: source_code,
                diagnostics,
            });
        }
    }
    Ok(report)
}

/// Lints source read from stdin. `config_lookup_path` is the path used to
/// find the config and `.editorconfig`; `display_name` is shown in output.
pub fn lint_stdin(
    source_code: String,
    config_lookup_path: &Path,
    display_name: &str,
    options: &LintRunOptions,
) -> Result<LintReport, String> {
    let mut linter = GDScriptLinter::new(LinterConfig::default())?;
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let loaded = config_cache.config_for(config_lookup_path, options)?;
    let settings = loaded
        .compiled
        .settings_for_file(config_lookup_path, &options.cli);
    let diagnostics = lint_with_settings(
        &mut linter,
        config_lookup_path,
        &source_code,
        &settings,
        &mut loaded.providers,
    )?;
    let mut report = LintReport {
        files: Vec::with_capacity(1),
        files_checked: 1,
    };
    if !diagnostics.is_empty() {
        report.files.push(FileDiagnostics {
            path: display_name.to_string(),
            source: source_code,
            diagnostics,
        });
    }
    Ok(report)
}

/// A file the fix engine changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedFile {
    /// The path to write the fixed source to. For stdin, the lookup path.
    pub path: PathBuf,
    /// The path as shown to the user, like in `FileDiagnostics`.
    pub display_path: String,
    pub original: String,
    pub fixed: String,
    /// How many fixes were applied to this file.
    pub applied: usize,
}

/// The result of `lint --fix`, `--fix-unsafe` or `--diff`, before anything is
/// written.
#[derive(Debug, Clone, Default)]
pub struct FixRunReport {
    /// Diagnostics left after fixing. Each file's `source` is the fixed text.
    pub report: LintReport,
    /// Files whose text changed, in the order they were processed.
    pub changed_files: Vec<FixedFile>,
    /// For stdin: the fixed source, even when it didn't change.
    pub stdin_output: Option<String>,
}

impl FixRunReport {
    /// The number of fixes applied over every file.
    pub fn applied(&self) -> usize {
        let mut applied = 0;
        for changed_file in &self.changed_files {
            applied += changed_file.applied;
        }
        applied
    }
}

/// Runs the fix engine on files from disk without writing them. Files
/// matching `[lint].exclude` are skipped, like in `lint_files`.
pub fn fix_files(
    files: &[PathBuf],
    options: &LintRunOptions,
    allow_unsafe: bool,
) -> Result<FixRunReport, String> {
    let mut linter = GDScriptLinter::new(LinterConfig::default())?;
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let mut fix_report = FixRunReport::default();
    for file_path in files {
        let loaded = config_cache.config_for(file_path, options)?;
        if loaded.compiled.is_excluded(file_path) {
            continue;
        }
        let settings = loaded.compiled.settings_for_file(file_path, &options.cli);
        let source_code = fs::read_to_string(file_path)
            .map_err(|error| format!("Failed to read file {}: {}", file_path.display(), error))?;
        let fixed = fix::fix_source_with_settings(
            &mut linter,
            file_path,
            &source_code,
            &settings,
            &mut loaded.providers,
            allow_unsafe,
        )?;
        add_fixed_source(
            file_path,
            &display_path(file_path),
            source_code,
            fixed,
            &mut fix_report,
        );
    }
    Ok(fix_report)
}

/// Runs the fix engine on source read from stdin. `config_lookup_path` finds
/// the config; `display_name` is shown in output and diffs.
pub fn fix_stdin(
    source_code: String,
    config_lookup_path: &Path,
    display_name: &str,
    options: &LintRunOptions,
    allow_unsafe: bool,
) -> Result<FixRunReport, String> {
    let mut linter = GDScriptLinter::new(LinterConfig::default())?;
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let loaded = config_cache.config_for(config_lookup_path, options)?;
    let settings = loaded
        .compiled
        .settings_for_file(config_lookup_path, &options.cli);
    let fixed = fix::fix_source_with_settings(
        &mut linter,
        config_lookup_path,
        &source_code,
        &settings,
        &mut loaded.providers,
        allow_unsafe,
    )?;
    let mut fix_report = FixRunReport {
        stdin_output: Some(fixed.output.clone()),
        ..FixRunReport::default()
    };
    add_fixed_source(
        config_lookup_path,
        display_name,
        source_code,
        fixed,
        &mut fix_report,
    );
    Ok(fix_report)
}

fn add_fixed_source(
    path: &Path,
    display_name: &str,
    original: String,
    fixed: fix::FixedSource,
    fix_report: &mut FixRunReport,
) {
    fix_report.report.files_checked += 1;
    if !fixed.remaining.is_empty() {
        fix_report.report.files.push(FileDiagnostics {
            path: display_name.to_string(),
            source: fixed.output.clone(),
            diagnostics: fixed.remaining,
        });
    }
    if fixed.output != original {
        fix_report.changed_files.push(FixedFile {
            path: path.to_path_buf(),
            display_path: display_name.to_string(),
            original,
            fixed: fixed.output,
            applied: fixed.applied,
        });
    }
}

/// Runs every enabled built-in rule on a parsed file and appends their
/// diagnostics, unfiltered and unsorted.
///
/// We build a map from node kinds to the rules that care about them, so we
/// walk the tree only once and call each rule only on the nodes it targets.
/// Source-only rules run first, then the tree walk, then each rule's
/// `finalize` for rules that report after collecting state.
pub fn run_builtin_rules(
    config: &LinterConfig,
    tree: &Tree,
    source_code: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut checkers: Vec<Box<dyn Rule>> = Vec::with_capacity(ALL_RULES.len());
    for current_rule in ALL_RULES {
        if !config.disabled_rules.contains(current_rule.name) {
            checkers.push((current_rule.create)(config));
        }
    }

    let mut node_kind_map: HashMap<GDScriptNodeKind, Vec<usize>> = HashMap::new();
    let mut source_only_rules = Vec::new();
    for (current_index, checker) in checkers.iter().enumerate() {
        let kinds = checker.get_target_ast_nodes();
        if kinds.is_empty() {
            source_only_rules.push(current_index);
        } else {
            for &kind in kinds {
                node_kind_map.entry(kind).or_default().push(current_index);
            }
        }
    }

    for &current_index in &source_only_rules {
        checkers[current_index].check_source(source_code, diagnostics);
    }
    visit_each_node(
        &tree.root_node(),
        source_code,
        &mut checkers,
        &node_kind_map,
        diagnostics,
    );
    for checker in checkers.iter_mut() {
        checker.finalize(source_code, diagnostics);
    }
}

/// This uses the visitor pattern to walk the parsed tree sitter AST only once.
/// We call each rule only when we encounter an AST node it cares about.
fn visit_each_node(
    node: &Node,
    source_code: &str,
    checkers: &mut [Box<dyn Rule>],
    node_kind_map: &HashMap<GDScriptNodeKind, Vec<usize>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(matching_rules) =
        node_kind_map.get(&GDScriptNodeKind::get_kind_from_ast_node(*node))
    {
        for &rule_index in matching_rules {
            checkers[rule_index].check_node(node, source_code, diagnostics);
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            visit_each_node(
                &cursor.node(),
                source_code,
                checkers,
                node_kind_map,
                diagnostics,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

pub fn lint_gdscript_with_config(
    source_code: &str,
    file_path: &str,
    config: &LinterConfig,
) -> Result<Vec<Diagnostic>, String> {
    let mut linter = GDScriptLinter::new(config.clone())?;
    linter.lint(source_code, file_path)
}

pub fn lint_gdscript(source_code: &str, file_path: &str) -> Result<Vec<Diagnostic>, String> {
    let config = LinterConfig::default();
    lint_gdscript_with_config(source_code, file_path, &config)
}
