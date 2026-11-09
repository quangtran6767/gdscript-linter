use crate::node_kind::GDScriptNodeKind;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tree_sitter::{Node, Parser, Tree};

// Shared contracts (frozen after W0).
pub mod api;
pub mod diagnostic;

pub mod ignore_patterns;
pub mod lib;
pub mod regex_patterns;
pub mod rule_config;
pub mod rule_groups;
pub mod rules;

// Track A modules.
pub mod baseline;
pub mod cache;
pub mod config;
pub mod fix;
pub mod godot_check;
pub mod output;

// Track B modules.
pub mod custom;
pub mod editor;
pub mod explain;
pub mod init;
pub mod rule_tester;
pub mod rules_meta;
#[cfg(feature = "js-rules")]
pub mod script;

#[cfg(test)]
mod tests;

use api::{Config, RuleLevel};
use config::{CliRuleOverrides, CompiledConfig, FileSettings};
use diagnostic::{Diagnostic, FileContext, RuleMeta, RuleProvider, Severity};
use ignore_patterns::{apply_ignores, parse_ignore_comments};
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
    /// Whether `type-annotations` accepts `:=` and implicit loop-variable types.
    pub allow_inferred_types: bool,
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            disabled_rules: HashSet::new(),
            max_line_length: 100,
            indent_size: 4,
            allow_inferred_types: false,
        }
    }
}

pub struct GDScriptLinter {
    config: LinterConfig,
    parser: Parser,
    project: Option<Arc<crate::project::ProjectIndex>>,
    project_analysis: Option<Arc<crate::project::ProjectAnalysis>>,
}

impl GDScriptLinter {
    pub fn new(config: LinterConfig) -> Result<Self, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_gdscript::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set language: {}", e))?;
        GDScriptNodeKind::populate_lookup_table();
        Ok(Self {
            config,
            parser,
            project: None,
            project_analysis: None,
        })
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
        let no_levels = BTreeMap::new();
        self.lint_with_levels(source_code, file_path, providers, &no_levels)
    }

    /// Like `lint_with_providers`, and applies the config's rule `levels`
    /// before ignore comments, so a comment for a rule that is `off` counts
    /// as unused. `unused-ignore` warnings are reported unless the rule is
    /// disabled in the linter config.
    pub fn lint_with_levels(
        &mut self,
        source_code: &str,
        file_path: &str,
        providers: &mut [Box<dyn RuleProvider>],
        levels: &BTreeMap<String, RuleLevel>,
    ) -> Result<Vec<Diagnostic>, String> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or("Failed to parse GDScript code")?;

        let needs_semantic = !providers.is_empty() || rules::semantic_rules_enabled(&self.config);
        let semantic = if needs_semantic {
            Some(crate::semantic::SemanticModel::build(&tree, source_code))
        } else {
            None
        };
        let context = FileContext {
            path: Path::new(file_path),
            source: source_code,
            tree: &tree,
            semantic: semantic.as_ref(),
            project: self.project.as_deref(),
        };
        let mut diagnostics = Vec::new();
        run_builtin_rules_in_context(
            &self.config,
            &context,
            self.project_analysis.as_deref(),
            &mut diagnostics,
        );
        for provider in providers.iter_mut() {
            diagnostics.extend(provider.check_file(&context));
        }

        apply_rule_levels(levels, &mut diagnostics);

        // Inline ignore comments apply to the line where a diagnostic starts.
        let ignore_comments = parse_ignore_comments(source_code, &tree);
        let report_unused = !self
            .config
            .disabled_rules
            .contains(ignore_patterns::UNUSED_IGNORE_RULE_ID);
        let mut unused_ignores = Vec::new();
        apply_ignores(
            source_code,
            &ignore_comments,
            report_unused,
            &self.config.disabled_rules,
            &mut diagnostics,
            &mut unused_ignores,
        );
        apply_rule_levels(levels, &mut unused_ignores);
        diagnostics.extend(unused_ignores);

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

    /// Sets the shared, immutable project index prepared for this run.
    pub fn set_project(&mut self, project: Option<Arc<crate::project::ProjectIndex>>) {
        self.project = project;
    }

    pub fn set_project_analysis(&mut self, analysis: Option<Arc<crate::project::ProjectAnalysis>>) {
        self.project_analysis = analysis;
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
    diagnostics.retain(|diagnostic| {
        levels
            .get(diagnostic.check_id())
            .or_else(|| levels.get(&diagnostic.rule))
            != Some(&RuleLevel::Off)
    });
    for diagnostic in diagnostics.iter_mut() {
        match levels
            .get(diagnostic.check_id())
            .or_else(|| levels.get(&diagnostic.rule))
        {
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
    load_project_context(linter, path, true)?;
    linter.set_config(settings.linter_config.clone());
    linter.lint_with_levels(
        source_code,
        &path.to_string_lossy(),
        providers,
        &settings.levels,
    )
}

fn load_project_context(
    linter: &mut GDScriptLinter,
    path: &Path,
    with_cache: bool,
) -> Result<(), String> {
    if linter.project.is_none() {
        if let Some(root) = crate::project::find_root(path) {
            let project = if with_cache {
                crate::project::ProjectIndex::build(&root)?
            } else {
                crate::project::ProjectIndex::build_uncached(&root)?
            };
            linter.project_analysis =
                Some(Arc::new(crate::project::ProjectAnalysis::build(&project)?));
            linter.project = Some(Arc::new(project));
        }
    }
    Ok(())
}

/// What the `lint` command needs besides the files.
#[derive(Debug, Clone, Default)]
pub struct LintRunOptions {
    /// `--config`: use this file for every linted file instead of searching
    /// upward from each one.
    pub config_path: Option<PathBuf>,
    /// `--disable` and `--max-line-length`.
    pub cli: CliRuleOverrides,
    /// The lint cache file (`.gdguard/cache/lint.json`), or None to lint
    /// every file (`--no-cache`, stdin, no config). Only `lint_files` uses it.
    pub cache_path: Option<PathBuf>,
    /// `--godot` or `GDGUARD_GODOT`: the Godot 4 executable used by
    /// `add_godot_diagnostics`. `None` skips the Godot check.
    pub godot_path: Option<PathBuf>,
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
    /// The metadata of every custom rule loaded so far, sorted by id, each
    /// id once.
    fn custom_rule_metas(&self) -> Vec<RuleMeta> {
        let mut metas: Vec<RuleMeta> = Vec::new();
        for loaded in self.configs.values() {
            for provider in &loaded.providers {
                for meta in provider.rules() {
                    metas.push(meta.clone());
                }
            }
        }
        metas.sort_by(|left, right| left.id.cmp(&right.id));
        metas.dedup_by(|left, right| left.id == right.id);
        metas
    }

    /// A copy of every loaded config, for worker threads (the providers stay
    /// here: they aren't `Send`).
    fn configs_by_key(&self) -> HashMap<PathBuf, Config> {
        let mut configs = HashMap::with_capacity(self.configs.len());
        for (key, loaded) in &self.configs {
            configs.insert(key.clone(), loaded.compiled.config.clone());
        }
        configs
    }

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

/// Lints files from disk, in parallel. Each file uses its nearest
/// `gdguard.toml` (or `--config`); files matching `[lint].exclude` are
/// skipped. With `options.cache_path`, unchanged files reuse their cached
/// result. Output order follows `files`, whatever the thread timing. Config
/// errors and unreadable files return an error (exit code 2).
pub fn lint_files(files: &[PathBuf], options: &LintRunOptions) -> Result<LintReport, String> {
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let mut lint_cache = None;
    if let Some(cache_path) = &options.cache_path {
        lint_cache = Some(cache::LintCache::load(cache_path));
    }
    let jobs = prepare_jobs(files, options, &mut config_cache, lint_cache.is_some())?;
    let configs = config_cache.configs_by_key();
    let outcomes = run_jobs(&jobs, &configs, JobMode::Lint, lint_cache.as_ref())?;

    let mut report = LintReport::default();
    for (job, outcome) in jobs.iter().zip(outcomes) {
        let JobOutcome::Linted {
            source,
            diagnostics,
            new_cache_entry,
        } = outcome
        else {
            return Err("the lint worker returned a fix result".to_string());
        };
        if let (Some(lint_cache), Some((file_key, cache_key))) = (&mut lint_cache, new_cache_entry)
        {
            lint_cache.insert(file_key, cache_key, &diagnostics);
        }
        report.files_checked += 1;
        if !diagnostics.is_empty() {
            report.files.push(FileDiagnostics {
                path: display_path(&job.path),
                source,
                diagnostics,
            });
        }
    }
    if let Some(lint_cache) = &mut lint_cache {
        // A cache that can't be written only costs speed on the next run.
        let _ = lint_cache.save();
    }
    report.rule_metas = config_cache.custom_rule_metas();
    Ok(report)
}

/// Runs the optional Godot check (`options.godot_path`) on files from disk and
/// adds its `godot-error` diagnostics to `report`. Excluded files and files
/// where `godot-error` is off are skipped; rule levels apply. Does nothing
/// without a Godot path. Godot failures return an error (exit code 2).
pub fn add_godot_diagnostics(
    report: &mut LintReport,
    files: &[PathBuf],
    options: &LintRunOptions,
) -> Result<(), String> {
    let Some(executable) = &options.godot_path else {
        return Ok(());
    };
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let mut targets = Vec::new();
    for file_path in files {
        let loaded = config_cache.config_for(file_path, options)?;
        if loaded.compiled.is_excluded(file_path) {
            continue;
        }
        let settings = loaded.compiled.settings_for_file(file_path, &options.cli);
        if settings
            .linter_config
            .disabled_rules
            .contains(godot_check::GODOT_ERROR_RULE_ID)
        {
            continue;
        }
        targets.push((file_path.clone(), settings));
    }
    let mut paths = Vec::with_capacity(targets.len());
    for (path, _) in &targets {
        paths.push(path.clone());
    }
    let mut found = godot_check::check_files(executable, &paths)?;
    for (path, settings) in &targets {
        let Some(mut diagnostics) = found.remove(path) else {
            continue;
        };
        apply_rule_levels(&settings.levels, &mut diagnostics);
        let shown_path = display_path(path);
        let mut index = None;
        for (file_index, file) in report.files.iter().enumerate() {
            if file.path == shown_path {
                index = Some(file_index);
            }
        }
        let index = if let Some(index) = index {
            index
        } else {
            let source = fs::read_to_string(path)
                .map_err(|error| format!("Failed to read file {}: {}", path.display(), error))?;
            report.files.push(FileDiagnostics {
                path: shown_path,
                source,
                diagnostics: Vec::new(),
            });
            report.files.len() - 1
        };
        let file = &mut report.files[index];
        file.diagnostics.extend(diagnostics);
        file.diagnostics.sort_by(|a, b| {
            a.span
                .start
                .line
                .cmp(&b.span.start.line)
                .then(a.span.start.column.cmp(&b.span.start.column))
        });
    }
    // Keep the report in input order, as `lint_files` does.
    let mut order = HashMap::new();
    for (position, path) in files.iter().enumerate() {
        order.entry(display_path(path)).or_insert(position);
    }
    report
        .files
        .sort_by_key(|file| order.get(&file.path).copied().unwrap_or(usize::MAX));
    Ok(())
}

/// The `ConfigCache` key of the config for `file_path`: its path, or an
/// empty path for the defaults.
fn config_key_for(file_path: &Path, options: &LintRunOptions) -> PathBuf {
    let config_path = match &options.config_path {
        Some(config_path) => Some(config_path.clone()),
        None => config::find_config_file(file_path),
    };
    match config_path {
        Some(config_path) => config_path,
        None => PathBuf::new(),
    }
}

/// One file to lint or fix, prepared on the main thread.
struct FileJob {
    path: PathBuf,
    /// The key of its config in `ConfigCache`.
    config_key: PathBuf,
    settings: FileSettings,
    /// `(file key, settings hash, rules hash)` when the cache is on.
    cache_inputs: Option<(String, String, String)>,
    project: Option<Arc<crate::project::ProjectIndex>>,
    project_analysis: Option<Arc<crate::project::ProjectAnalysis>>,
}

#[derive(Clone, Copy)]
enum JobMode {
    Lint,
    Fix { allow_unsafe: bool },
}

enum JobOutcome {
    Linted {
        source: String,
        diagnostics: Vec<Diagnostic>,
        /// Set on a cache miss, to store the result.
        new_cache_entry: Option<(String, cache::CacheKey)>,
    },
    Fixed {
        source: String,
        fixed: fix::FixedSource,
    },
}

/// Resolves the config and settings of every file, in order, skipping
/// excluded files. Loads and validates each config once.
fn prepare_jobs(
    files: &[PathBuf],
    options: &LintRunOptions,
    config_cache: &mut ConfigCache,
    with_cache: bool,
) -> Result<Vec<FileJob>, String> {
    let mut projects: HashMap<PathBuf, Arc<crate::project::ProjectIndex>> = HashMap::new();
    let mut analyses: HashMap<PathBuf, Arc<crate::project::ProjectAnalysis>> = HashMap::new();
    let mut project_hashes: HashMap<PathBuf, String> = HashMap::new();
    let mut rules_hashes: HashMap<PathBuf, String> = HashMap::new();
    let mut jobs = Vec::with_capacity(files.len());
    for file_path in files {
        let config_key = config_key_for(file_path, options);
        let loaded = config_cache.config_for(file_path, options)?;
        if loaded.compiled.is_excluded(file_path) {
            continue;
        }
        let settings = loaded.compiled.settings_for_file(file_path, &options.cli);
        let project_root = crate::project::find_root(file_path);
        if let Some(root) = &project_root {
            if !projects.contains_key(root) {
                let project = if with_cache {
                    crate::project::ProjectIndex::build(root)?
                } else {
                    crate::project::ProjectIndex::build_uncached(root)?
                };
                analyses.insert(
                    root.clone(),
                    Arc::new(crate::project::ProjectAnalysis::build(&project)?),
                );
                if with_cache {
                    project_hashes.insert(root.clone(), crate::project::fingerprint(&project)?);
                }
                projects.insert(root.clone(), Arc::new(project));
            }
        }
        let project = project_root
            .as_ref()
            .and_then(|root| projects.get(root))
            .cloned();
        let cache_inputs = if with_cache {
            if !rules_hashes.contains_key(&config_key) {
                rules_hashes.insert(
                    config_key.clone(),
                    cache::rules_hash(&loaded.compiled.config),
                );
            }
            let mut rules_hash = match rules_hashes.get(&config_key) {
                Some(rules_hash) => rules_hash.clone(),
                None => String::new(),
            };
            if let Some(project_hash) = project_root
                .as_ref()
                .and_then(|root| project_hashes.get(root))
            {
                rules_hash = cache::stable_hash(&[rules_hash.as_bytes(), project_hash.as_bytes()]);
            }
            let mut settings_hash = cache::settings_hash(&settings);
            if !settings
                .linter_config
                .disabled_rules
                .contains("declaration-order")
            {
                // Reorder fixes use different paths for formatted and raw
                // files, including settings the other lint rules ignore.
                let mut formatter = crate::FormatterConfiguration::default();
                crate::editorconfig::apply_editorconfig_to_formatter_config(
                    &mut formatter,
                    file_path,
                );
                let formatting = format!(
                    "{:?}",
                    (
                        formatter.printer.max_line_length,
                        formatter.printer.use_spaces,
                        formatter.printer.indent_size,
                        formatter.printer.continuation_indent_level,
                        formatter.printer.insert_final_newline,
                        formatter.printer.trim_trailing_whitespace,
                        formatter.printer.indent_blank_lines,
                        formatter.blank_lines_around_definitions,
                        formatter.quote_style,
                    )
                );
                settings_hash =
                    cache::stable_hash(&[settings_hash.as_bytes(), formatting.as_bytes()]);
            }
            Some((cache::file_key(file_path), settings_hash, rules_hash))
        } else {
            None
        };
        jobs.push(FileJob {
            path: file_path.clone(),
            config_key,
            settings,
            cache_inputs,
            project,
            project_analysis: project_root
                .as_ref()
                .and_then(|root| analyses.get(root))
                .cloned(),
        });
    }
    Ok(jobs)
}

/// Runs the jobs on as many threads as there are cores, and returns their
/// outcomes in job order. The first error in job order wins.
fn run_jobs(
    jobs: &[FileJob],
    configs: &HashMap<PathBuf, Config>,
    mode: JobMode,
    lint_cache: Option<&cache::LintCache>,
) -> Result<Vec<JobOutcome>, String> {
    let core_count = match std::thread::available_parallelism() {
        Ok(count) => count.get(),
        Err(_) => 1,
    };
    let worker_count = core_count.min(jobs.len()).max(1);
    let next_job = AtomicUsize::new(0);

    let mut slots: Vec<Option<Result<JobOutcome, String>>> = Vec::with_capacity(jobs.len());
    for _ in 0..jobs.len() {
        slots.push(None);
    }
    if worker_count == 1 {
        for (index, result) in run_worker(jobs, configs, mode, lint_cache, &next_job) {
            slots[index] = Some(result);
        }
    } else {
        let mut worker_results = Vec::with_capacity(worker_count);
        std::thread::scope(|scope| {
            let mut handles = Vec::with_capacity(worker_count);
            for _ in 0..worker_count {
                handles
                    .push(scope.spawn(|| run_worker(jobs, configs, mode, lint_cache, &next_job)));
            }
            for handle in handles {
                worker_results.push(handle.join());
            }
        });
        for worker_result in worker_results {
            let Ok(results) = worker_result else {
                return Err("a lint worker thread panicked".to_string());
            };
            for (index, result) in results {
                slots[index] = Some(result);
            }
        }
    }

    let mut outcomes = Vec::with_capacity(jobs.len());
    for (index, slot) in slots.into_iter().enumerate() {
        match slot {
            Some(Ok(outcome)) => outcomes.push(outcome),
            Some(Err(error)) => return Err(error),
            None => return Err(format!("no result for {}", jobs[index].path.display())),
        }
    }
    Ok(outcomes)
}

/// One worker: takes the next job until none are left. Each worker has its
/// own linter and its own custom rule providers (they aren't `Send`).
fn run_worker(
    jobs: &[FileJob],
    configs: &HashMap<PathBuf, Config>,
    mode: JobMode,
    lint_cache: Option<&cache::LintCache>,
    next_job: &AtomicUsize,
) -> Vec<(usize, Result<JobOutcome, String>)> {
    let mut results = Vec::new();
    let mut linter = match GDScriptLinter::new(LinterConfig::default()) {
        Ok(linter) => linter,
        Err(error) => {
            let index = next_job.fetch_add(1, Ordering::Relaxed);
            if index < jobs.len() {
                results.push((index, Err(error)));
            }
            return results;
        }
    };
    let mut providers_by_config: HashMap<PathBuf, Vec<Box<dyn RuleProvider>>> = HashMap::new();
    loop {
        let index = next_job.fetch_add(1, Ordering::Relaxed);
        if index >= jobs.len() {
            break;
        }
        let result = run_job(
            &mut linter,
            &mut providers_by_config,
            &jobs[index],
            configs,
            mode,
            lint_cache,
        );
        results.push((index, result));
    }
    results
}

fn run_job(
    linter: &mut GDScriptLinter,
    providers_by_config: &mut HashMap<PathBuf, Vec<Box<dyn RuleProvider>>>,
    job: &FileJob,
    configs: &HashMap<PathBuf, Config>,
    mode: JobMode,
    lint_cache: Option<&cache::LintCache>,
) -> Result<JobOutcome, String> {
    let source = fs::read_to_string(&job.path)
        .map_err(|error| format!("Failed to read file {}: {}", job.path.display(), error))?;

    linter.set_project(job.project.clone());
    linter.set_project_analysis(job.project_analysis.clone());

    let mut cache_key: Option<(String, cache::CacheKey)> = None;
    if let JobMode::Lint = mode
        && let Some((file_key, settings_hash, rules_hash)) = &job.cache_inputs
    {
        let key = cache::CacheKey {
            file_hash: cache::file_hash(&source),
            settings_hash: settings_hash.clone(),
            rules_hash: rules_hash.clone(),
        };
        if let Some(lint_cache) = lint_cache
            && let Some(diagnostics) = lint_cache.get(file_key, &key, &source)
        {
            return Ok(JobOutcome::Linted {
                source,
                diagnostics,
                new_cache_entry: None,
            });
        }
        cache_key = Some((file_key.clone(), key));
    }

    if !providers_by_config.contains_key(&job.config_key) {
        let providers = match configs.get(&job.config_key) {
            Some(config) => custom::load_providers(config)?,
            None => Vec::new(),
        };
        providers_by_config.insert(job.config_key.clone(), providers);
    }
    let Some(providers) = providers_by_config.get_mut(&job.config_key) else {
        return Err("lost the custom rules of a config".to_string());
    };

    match mode {
        JobMode::Lint => {
            let diagnostics =
                lint_with_settings(linter, &job.path, &source, &job.settings, providers)?;
            Ok(JobOutcome::Linted {
                source,
                diagnostics,
                new_cache_entry: cache_key,
            })
        }
        JobMode::Fix { allow_unsafe } => {
            let fixed = fix::fix_source_with_settings(
                linter,
                &job.path,
                &source,
                &job.settings,
                providers,
                allow_unsafe,
            )?;
            Ok(JobOutcome::Fixed { source, fixed })
        }
    }
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
    load_project_context(&mut linter, config_lookup_path, false)?;
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
        baseline_hidden: 0,
        rule_metas: Vec::new(),
    };
    if !diagnostics.is_empty() {
        report.files.push(FileDiagnostics {
            path: display_name.to_string(),
            source: source_code,
            diagnostics,
        });
    }
    report.rule_metas = config_cache.custom_rule_metas();
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
    /// True when `[fix].format` formatted this file.
    pub formatted: bool,
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

/// Runs the fix engine on files from disk, in parallel, without writing
/// them. Files matching `[lint].exclude` are skipped, like in `lint_files`.
/// The cache isn't used.
pub fn fix_files(
    files: &[PathBuf],
    options: &LintRunOptions,
    allow_unsafe: bool,
) -> Result<FixRunReport, String> {
    let mut config_cache = ConfigCache {
        configs: HashMap::new(),
    };
    let jobs = prepare_jobs(files, options, &mut config_cache, false)?;
    let configs = config_cache.configs_by_key();
    let outcomes = run_jobs(&jobs, &configs, JobMode::Fix { allow_unsafe }, None)?;
    let mut fix_report = FixRunReport::default();
    for (job, outcome) in jobs.iter().zip(outcomes) {
        let JobOutcome::Fixed { source, fixed } = outcome else {
            return Err("the fix worker returned a lint result".to_string());
        };
        add_fixed_source(
            &job.path,
            &display_path(&job.path),
            source,
            fixed,
            &mut fix_report,
        );
    }
    fix_report.report.rule_metas = config_cache.custom_rule_metas();
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
    load_project_context(&mut linter, config_lookup_path, false)?;
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
    fix_report.report.rule_metas = config_cache.custom_rule_metas();
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
            formatted: fixed.formatted,
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
    let semantic = if rules::semantic_rules_enabled(config) {
        Some(crate::semantic::SemanticModel::build(tree, source_code))
    } else {
        None
    };
    let context = FileContext {
        path: Path::new(""),
        source: source_code,
        tree,
        semantic: semantic.as_ref(),
        project: None,
    };
    run_builtin_rules_in_context(config, &context, None, diagnostics);
}

fn run_builtin_rules_in_context(
    config: &LinterConfig,
    context: &FileContext,
    project_analysis: Option<&crate::project::ProjectAnalysis>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let tree = context.tree;
    let source_code = context.source;
    if !config.disabled_rules.contains("parse-error") {
        if let Some(node) = first_parse_error(tree, source_code) {
            diagnostics.push(Diagnostic {
                check: None,
                rule: "parse-error".to_string(),
                severity: diagnostic::Severity::Error,
                message: if node.is_missing() {
                    format!("Missing {} in GDScript syntax.", node.kind())
                } else {
                    "The GDScript parser could not read this syntax.".to_string()
                },
                help: Some("Repair the syntax near this range and run lint again. Confirm valid syntax and additional type diagnostics with Godot; this parser may lag new language features.".to_string()),
                span: diagnostic::Span::from_node(&node),
                fixes: Vec::new(),
            });
        }
    }
    let mut checkers: Vec<Box<dyn Rule>> = Vec::with_capacity(ALL_RULES.len());
    for current_rule in ALL_RULES {
        if current_rule.name == "unused-local-signal"
            && project_analysis.is_some()
            && !config.disabled_rules.contains("unused-signal")
        {
            continue;
        }
        if !config.disabled_rules.contains(current_rule.name)
            && !config
                .disabled_rules
                .contains(rule_groups::rule_id(current_rule.name))
        {
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
        checker.check_context(context, diagnostics);
        if let Some(analysis) = project_analysis {
            checker.check_project_context(context, analysis, diagnostics);
        }
    }
    rule_groups::consolidate(diagnostics);
}

/// The grammar requires at least one enum item, while Godot 4.7 accepts
/// empty enums. Ignore that known recovery node, without hiding other errors.
fn first_parse_error<'tree>(tree: &'tree Tree, source: &str) -> Option<tree_sitter::Node<'tree>> {
    if !tree.root_node().has_error() {
        return None;
    }
    let mut cursor = tree.walk();
    loop {
        let node = cursor.node();
        if node.is_error() || node.is_missing() {
            let empty_enum = node.is_missing()
                && node.kind() == "identifier"
                && node.parent().is_some_and(|parent| {
                    parent.kind() == "enumerator"
                        && parent.parent().is_some_and(|list| {
                            list.kind() == "enumerator_list"
                                && source[list.byte_range()]
                                    .chars()
                                    .all(|ch| ch == '{' || ch == '}' || ch.is_whitespace())
                        })
                });
            if !empty_enum {
                return Some(node);
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
                return None;
            }
        }
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
