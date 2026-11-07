//! Owner: Track A. Loads `gdguard.toml`: upward discovery, `[rules]`,
//! `[custom_rules]`, `[lint]`, `[[overrides]]`, precedence and validation.
//!
//! Precedence for one file, highest first: CLI flags, then `[[overrides]]`
//! (later entries win), then `[rules]`, then `.editorconfig`
//! (`max_line_length` and `indent_size`), then the defaults.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};

use crate::linter::LinterConfig;
use crate::linter::api::{Config, ConfigError, ConfigOverride, RuleLevel, RuleSetting};
use crate::linter::custom;
use crate::linter::rules::ALL_RULES;

pub const CONFIG_FILE_NAME: &str = "gdguard.toml";

/// Diagnostic ids reported by built-in rules. `load-variable-name` is reported
/// by the `variable-name` rule for variables holding a loaded resource.
pub fn builtin_rule_ids() -> Vec<&'static str> {
    let mut ids = Vec::with_capacity(ALL_RULES.len() + 1);
    for rule in ALL_RULES {
        ids.push(rule.name);
    }
    ids.push("load-variable-name");
    ids
}

/// The options a built-in rule accepts in its table form.
fn builtin_rule_options(rule_id: &str) -> &'static [&'static str] {
    match rule_id {
        "max-line-length" => &["max"],
        _ => &[],
    }
}

/// Settings given on the command line. They win over every config file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CliRuleOverrides {
    /// `--disable`.
    pub disabled_rules: HashSet<String>,
    /// `--max-line-length`.
    pub max_line_length: Option<usize>,
}

/// The settings that apply to one file, after precedence.
#[derive(Debug, Clone)]
pub struct FileSettings {
    /// Built-in rule settings: disabled rules (level `off`), line length and
    /// indent size.
    pub linter_config: LinterConfig,
    /// Levels set explicitly for a rule id, built-in or custom. Rules without
    /// an entry keep their default severity.
    pub levels: BTreeMap<String, RuleLevel>,
    /// Options set for a rule id (the table form minus `level`).
    pub rule_options: BTreeMap<String, toml::Table>,
}

/// Finds the nearest `gdguard.toml` for a file or folder, searching upward.
/// Relative paths are resolved against the current directory first.
pub fn find_config_file(path: &Path) -> Option<PathBuf> {
    let absolute_path = make_absolute(path);
    let mut current_directory = if absolute_path.is_dir() {
        Some(absolute_path.as_path())
    } else {
        absolute_path.parent()
    };
    while let Some(directory) = current_directory {
        let candidate = directory.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return Some(candidate);
        }
        current_directory = directory.parent();
    }
    None
}

fn make_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match std::env::current_dir() {
        Ok(current_directory) => current_directory.join(path),
        Err(_) => path.to_path_buf(),
    }
}

/// Reads and parses a config file. Checks the TOML syntax, every key, every
/// level and the options of built-in rules. Unknown rule ids are checked
/// separately by [`validate_rule_ids`], because custom rules must be loaded
/// first.
pub fn load_config_file(path: &Path) -> Result<Config, ConfigError> {
    let absolute_path = make_absolute(path);
    let text = std::fs::read_to_string(&absolute_path).map_err(|error| ConfigError {
        path: Some(absolute_path.clone()),
        key: None,
        message: format!("can't read the config file: {}", error),
    })?;
    parse_config(&absolute_path, &text)
}

/// Parses the text of a config file. `config_path` is used for error messages
/// and to resolve relative paths, and should be absolute.
pub fn parse_config(config_path: &Path, text: &str) -> Result<Config, ConfigError> {
    let table: toml::Table = toml::from_str(text).map_err(|error| ConfigError {
        path: Some(config_path.to_path_buf()),
        key: None,
        message: format!("invalid TOML: {}", error.to_string().trim_end()),
    })?;

    let config_directory = match config_path.parent() {
        Some(directory) => directory.to_path_buf(),
        None => PathBuf::new(),
    };
    let mut config = Config {
        config_path: Some(config_path.to_path_buf()),
        ..Config::default()
    };

    for (section_name, section_value) in &table {
        match section_name.as_str() {
            "rules" => {
                let Some(rules_table) = section_value.as_table() else {
                    return Err(key_error(
                        config_path,
                        "rules",
                        "expected a table".to_string(),
                    ));
                };
                config.rules = parse_rules_table(rules_table, "rules", config_path)?;
            }
            "custom_rules" => {
                let Some(custom_rules_table) = section_value.as_table() else {
                    return Err(key_error(
                        config_path,
                        "custom_rules",
                        "expected a table".to_string(),
                    ));
                };
                for (key, value) in custom_rules_table {
                    if key != "paths" {
                        return Err(key_error(
                            config_path,
                            &format!("custom_rules.{}", key),
                            "unknown key; expected: paths".to_string(),
                        ));
                    }
                    let paths = parse_string_array(value, "custom_rules.paths", config_path)?;
                    for rule_path in paths {
                        config
                            .custom_rule_paths
                            .push(config_directory.join(rule_path));
                    }
                }
            }
            "lint" => {
                let Some(lint_table) = section_value.as_table() else {
                    return Err(key_error(
                        config_path,
                        "lint",
                        "expected a table".to_string(),
                    ));
                };
                for (key, value) in lint_table {
                    match key.as_str() {
                        "exclude" => {
                            config.exclude =
                                parse_string_array(value, "lint.exclude", config_path)?;
                            compile_globs(&config.exclude, "lint.exclude", config_path)?;
                        }
                        "baseline" => {
                            let Some(baseline) = value.as_str() else {
                                return Err(key_error(
                                    config_path,
                                    "lint.baseline",
                                    "expected a string".to_string(),
                                ));
                            };
                            config.baseline = Some(config_directory.join(baseline));
                        }
                        _ => {
                            return Err(key_error(
                                config_path,
                                &format!("lint.{}", key),
                                "unknown key; expected one of: exclude, baseline".to_string(),
                            ));
                        }
                    }
                }
            }
            "overrides" => {
                let Some(override_list) = section_value.as_array() else {
                    return Err(key_error(
                        config_path,
                        "overrides",
                        "expected an array of tables: write each one as [[overrides]]".to_string(),
                    ));
                };
                for (override_index, override_value) in override_list.iter().enumerate() {
                    let override_key = format!("overrides[{}]", override_index);
                    let Some(override_table) = override_value.as_table() else {
                        return Err(key_error(
                            config_path,
                            &override_key,
                            "expected a table".to_string(),
                        ));
                    };
                    let mut config_override = ConfigOverride::default();
                    for (key, value) in override_table {
                        match key.as_str() {
                            "files" => {
                                let files_key = format!("{}.files", override_key);
                                config_override.files =
                                    parse_string_array(value, &files_key, config_path)?;
                                compile_globs(&config_override.files, &files_key, config_path)?;
                            }
                            "rules" => {
                                let rules_key = format!("{}.rules", override_key);
                                let Some(rules_table) = value.as_table() else {
                                    return Err(key_error(
                                        config_path,
                                        &rules_key,
                                        "expected a table".to_string(),
                                    ));
                                };
                                config_override.rules =
                                    parse_rules_table(rules_table, &rules_key, config_path)?;
                            }
                            _ => {
                                return Err(key_error(
                                    config_path,
                                    &format!("{}.{}", override_key, key),
                                    "unknown key; expected one of: files, rules".to_string(),
                                ));
                            }
                        }
                    }
                    if config_override.files.is_empty() {
                        return Err(key_error(
                            config_path,
                            &format!("{}.files", override_key),
                            "required: list the glob patterns this override applies to".to_string(),
                        ));
                    }
                    config.overrides.push(config_override);
                }
            }
            _ => {
                return Err(key_error(
                    config_path,
                    section_name,
                    "unknown key; expected one of: rules, custom_rules, lint, overrides"
                        .to_string(),
                ));
            }
        }
    }

    Ok(config)
}

/// Parses a `[rules]` table (or an override's `rules`). Each value is a level
/// string or a table with an optional `level` plus options.
fn parse_rules_table(
    rules_table: &toml::Table,
    table_key: &str,
    config_path: &Path,
) -> Result<BTreeMap<String, RuleSetting>, ConfigError> {
    let builtin_ids = builtin_rule_ids();
    let mut rules = BTreeMap::new();
    for (rule_id, value) in rules_table {
        let rule_key = format!("{}.{}", table_key, rule_id);
        let mut setting = RuleSetting::default();
        match value {
            toml::Value::String(level_name) => {
                setting.level = Some(parse_level(level_name, &rule_key, config_path)?);
            }
            toml::Value::Table(rule_table) => {
                for (option_name, option_value) in rule_table {
                    if option_name == "level" {
                        let Some(level_name) = option_value.as_str() else {
                            return Err(key_error(
                                config_path,
                                &format!("{}.level", rule_key),
                                "expected a string".to_string(),
                            ));
                        };
                        setting.level = Some(parse_level(
                            level_name,
                            &format!("{}.level", rule_key),
                            config_path,
                        )?);
                        continue;
                    }
                    setting
                        .options
                        .insert(option_name.clone(), option_value.clone());
                }
            }
            _ => {
                return Err(key_error(config_path,
                    &rule_key,
                    "expected a level string (\"off\", \"info\", \"warning\", \"error\") or a table"
                        .to_string(),
                ));
            }
        }

        // Custom rules are loaded later, so only built-in options are checked
        // here. Custom rule options are passed through to the rule.
        if builtin_ids.contains(&rule_id.as_str()) {
            let allowed_options = builtin_rule_options(rule_id);
            for (option_name, option_value) in &setting.options {
                let option_key = format!("{}.{}", rule_key, option_name);
                if !allowed_options.contains(&option_name.as_str()) {
                    let message = if allowed_options.is_empty() {
                        format!("unknown option; `{}` has no options besides level", rule_id)
                    } else {
                        format!(
                            "unknown option; `{}` accepts: level, {}",
                            rule_id,
                            allowed_options.join(", ")
                        )
                    };
                    return Err(key_error(config_path, &option_key, message));
                }
                if rule_id == "max-line-length" && option_name == "max" {
                    let is_positive_integer = match option_value.as_integer() {
                        Some(number) => number > 0,
                        None => false,
                    };
                    if !is_positive_integer {
                        return Err(key_error(
                            config_path,
                            &option_key,
                            "expected a positive integer".to_string(),
                        ));
                    }
                }
            }
        }
        rules.insert(rule_id.clone(), setting);
    }
    Ok(rules)
}

fn key_error(config_path: &Path, key: &str, message: String) -> ConfigError {
    ConfigError {
        path: Some(config_path.to_path_buf()),
        key: Some(key.to_string()),
        message,
    }
}

fn parse_level(level_name: &str, key: &str, config_path: &Path) -> Result<RuleLevel, ConfigError> {
    match RuleLevel::from_name(level_name) {
        Some(level) => Ok(level),
        None => Err(ConfigError {
            path: Some(config_path.to_path_buf()),
            key: Some(key.to_string()),
            message: format!(
                "invalid level \"{}\"; expected \"off\", \"info\", \"warning\" or \"error\"",
                level_name
            ),
        }),
    }
}

fn parse_string_array(
    value: &toml::Value,
    key: &str,
    config_path: &Path,
) -> Result<Vec<String>, ConfigError> {
    let type_error = ConfigError {
        path: Some(config_path.to_path_buf()),
        key: Some(key.to_string()),
        message: "expected an array of strings".to_string(),
    };
    let Some(items) = value.as_array() else {
        return Err(type_error);
    };
    let mut strings = Vec::with_capacity(items.len());
    for item in items {
        let Some(text) = item.as_str() else {
            return Err(type_error);
        };
        strings.push(text.to_string());
    }
    Ok(strings)
}

/// Compiles glob patterns. `*` doesn't cross `/`; `**` does.
fn compile_globs(
    patterns: &[String],
    key: &str,
    config_path: &Path,
) -> Result<GlobSet, ConfigError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .build()
            .map_err(|error| ConfigError {
                path: Some(config_path.to_path_buf()),
                key: Some(key.to_string()),
                message: format!("invalid glob pattern \"{}\": {}", pattern, error.kind()),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| ConfigError {
        path: Some(config_path.to_path_buf()),
        key: Some(key.to_string()),
        message: format!("invalid glob patterns: {}", error),
    })
}

/// Checks that every rule id in `[rules]` and in the overrides is a built-in
/// rule or a custom rule loaded from `config.custom_rule_paths`.
pub fn validate_rule_ids(config: &Config) -> Result<(), ConfigError> {
    let mut known_ids: Vec<String> = Vec::new();
    for id in builtin_rule_ids() {
        known_ids.push(id.to_string());
    }
    let providers = custom::load_providers(config).map_err(|message| ConfigError {
        path: None,
        key: None,
        message,
    })?;
    for provider in &providers {
        for meta in provider.rules() {
            known_ids.push(meta.id.clone());
        }
    }

    let mut rule_tables: Vec<(String, &BTreeMap<String, RuleSetting>)> =
        Vec::with_capacity(config.overrides.len() + 1);
    rule_tables.push(("rules".to_string(), &config.rules));
    for (override_index, config_override) in config.overrides.iter().enumerate() {
        rule_tables.push((
            format!("overrides[{}].rules", override_index),
            &config_override.rules,
        ));
    }
    for (table_key, rules) in rule_tables {
        for rule_id in rules.keys() {
            if known_ids.contains(rule_id) {
                continue;
            }
            let mut message = format!("unknown rule id \"{}\"", rule_id);
            if let Some(suggestion) = closest_name(rule_id, &known_ids) {
                message.push_str(&format!("; did you mean \"{}\"?", suggestion));
            }
            return Err(ConfigError {
                path: config.config_path.clone(),
                key: Some(format!("{}.{}", table_key, rule_id)),
                message,
            });
        }
    }
    Ok(())
}

/// Returns the candidate closest to `name` by edit distance, when it is close
/// enough to be a likely typo.
fn closest_name<'a>(name: &str, candidates: &'a [String]) -> Option<&'a str> {
    let mut best: Option<&'a str> = None;
    let mut best_distance = usize::MAX;
    for candidate in candidates {
        let distance = edit_distance(name, candidate);
        if distance < best_distance {
            best_distance = distance;
            best = Some(candidate.as_str());
        }
    }
    let maximum_distance = (name.chars().count() / 3).max(2);
    if best_distance <= maximum_distance {
        return best;
    }
    None
}

/// Levenshtein distance between two strings, counted in characters.
fn edit_distance(left: &str, right: &str) -> usize {
    let left_characters: Vec<char> = left.chars().collect();
    let right_characters: Vec<char> = right.chars().collect();
    let mut previous_row: Vec<usize> = Vec::with_capacity(right_characters.len() + 1);
    for column in 0..=right_characters.len() {
        previous_row.push(column);
    }
    let mut current_row = vec![0; right_characters.len() + 1];
    for row in 1..=left_characters.len() {
        current_row[0] = row;
        for column in 1..=right_characters.len() {
            let substitution_cost = if left_characters[row - 1] == right_characters[column - 1] {
                0
            } else {
                1
            };
            current_row[column] = (previous_row[column] + 1)
                .min(current_row[column - 1] + 1)
                .min(previous_row[column - 1] + substitution_cost);
        }
        std::mem::swap(&mut previous_row, &mut current_row);
    }
    previous_row[right_characters.len()]
}

/// Loads the config for `path`: the nearest `gdguard.toml`, or the defaults
/// when there is none. Validates rule ids against built-in and custom rules.
pub fn load_config_for(path: &Path) -> Result<Config, ConfigError> {
    let Some(config_path) = find_config_file(path) else {
        return Ok(Config::default());
    };
    load_and_validate(&config_path)
}

/// Loads one config file and validates its rule ids.
pub fn load_and_validate(config_path: &Path) -> Result<Config, ConfigError> {
    let config = load_config_file(config_path)?;
    validate_rule_ids(&config)?;
    Ok(config)
}

/// A config with its glob patterns compiled, ready to answer per-file
/// questions. Build it once per config and reuse it for every file.
pub struct CompiledConfig {
    pub config: Config,
    /// Folder that relative globs resolve against: the config file's folder,
    /// or the current directory for the default config.
    base_directory: PathBuf,
    exclude: GlobSet,
    overrides: Vec<GlobSet>,
}

impl CompiledConfig {
    pub fn new(config: Config) -> Result<CompiledConfig, ConfigError> {
        let config_path = match &config.config_path {
            Some(config_path) => config_path.clone(),
            None => PathBuf::from(CONFIG_FILE_NAME),
        };
        let base_directory = match &config.config_path {
            Some(config_path) => match config_path.parent() {
                Some(directory) => make_absolute(directory),
                None => make_absolute(Path::new(".")),
            },
            None => make_absolute(Path::new(".")),
        };
        let exclude = compile_globs(&config.exclude, "lint.exclude", &config_path)?;
        let mut overrides = Vec::with_capacity(config.overrides.len());
        for (override_index, config_override) in config.overrides.iter().enumerate() {
            overrides.push(compile_globs(
                &config_override.files,
                &format!("overrides[{}].files", override_index),
                &config_path,
            )?);
        }
        Ok(CompiledConfig {
            config,
            base_directory,
            exclude,
            overrides,
        })
    }

    /// True when `[lint].exclude` matches the file or one of its folders.
    pub fn is_excluded(&self, file_path: &Path) -> bool {
        self.matches(&self.exclude, file_path)
    }

    /// Matches the path relative to the base folder, and each of its parent
    /// folders, so `addons` and `addons/**` both exclude `addons/a/b.gd`.
    fn matches(&self, globs: &GlobSet, file_path: &Path) -> bool {
        if globs.is_empty() {
            return false;
        }
        let absolute_path = make_absolute(file_path);
        let Ok(relative_path) = absolute_path.strip_prefix(&self.base_directory) else {
            return false;
        };
        let mut current_path = Some(relative_path);
        while let Some(path) = current_path {
            if path.as_os_str().is_empty() {
                break;
            }
            if globs.is_match(path) {
                return true;
            }
            current_path = path.parent();
        }
        false
    }

    /// Resolves the settings for one file, applying the precedence order.
    pub fn settings_for_file(&self, file_path: &Path, cli: &CliRuleOverrides) -> FileSettings {
        let mut linter_config = LinterConfig::default();
        crate::editorconfig::apply_editorconfig_to_linter_config(&mut linter_config, file_path);
        if let Some(max_line_length) = self.config.max_line_length {
            linter_config.max_line_length = max_line_length;
        }
        if let Some(indent_size) = self.config.indent_size {
            linter_config.indent_size = indent_size;
        }

        // `[rules]`, then each matching override in order. An override's
        // level replaces the level; its options are merged key by key.
        let mut levels: BTreeMap<String, RuleLevel> = BTreeMap::new();
        let mut rule_options: BTreeMap<String, toml::Table> = BTreeMap::new();
        merge_rule_settings(&self.config.rules, &mut levels, &mut rule_options);
        for (override_index, config_override) in self.config.overrides.iter().enumerate() {
            if self.matches(&self.overrides[override_index], file_path) {
                merge_rule_settings(&config_override.rules, &mut levels, &mut rule_options);
            }
        }

        if let Some(options) = rule_options.get("max-line-length")
            && let Some(max) = options.get("max")
            && let Some(max) = max.as_integer()
            && max > 0
        {
            linter_config.max_line_length = max as usize;
        }

        for disabled_rule in &cli.disabled_rules {
            levels.insert(disabled_rule.clone(), RuleLevel::Off);
        }
        if let Some(max_line_length) = cli.max_line_length {
            linter_config.max_line_length = max_line_length;
        }

        for (rule_id, level) in &levels {
            if *level == RuleLevel::Off {
                linter_config.disabled_rules.insert(rule_id.clone());
            }
        }
        FileSettings {
            linter_config,
            levels,
            rule_options,
        }
    }
}

fn merge_rule_settings(
    rules: &BTreeMap<String, RuleSetting>,
    levels: &mut BTreeMap<String, RuleLevel>,
    rule_options: &mut BTreeMap<String, toml::Table>,
) {
    for (rule_id, setting) in rules {
        if let Some(level) = setting.level {
            levels.insert(rule_id.clone(), level);
        }
        if setting.options.is_empty() {
            continue;
        }
        let options = rule_options.entry(rule_id.clone()).or_default();
        for (option_name, option_value) in &setting.options {
            options.insert(option_name.clone(), option_value.clone());
        }
    }
}
