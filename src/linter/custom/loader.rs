//! Loads and validates custom YAML rule files (`.gdguard/rules/*.yml`).
//!
//! One rule per file. The loader checks every key before compiling, so each
//! error names the file and the key it is about. The output is a list of
//! [`LoadedRule`]: the rule's `RuleMeta`, its compiled ast-grep matcher and
//! fix, and its `tests:` section for `gdguard test-rules`.
//!
//! Spec: `docs/specification_linter.md`, section "Custom YAML rules".

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use ast_grep_config::{DeserializeEnv, RuleCore, SerializableRule, SerializableRuleCore};
use serde_json::Value;

use super::language::{EXPANDO_CHAR, GdScript, to_internal};
use crate::linter::api::ConfigError;
use crate::linter::diagnostic::{FixSafety, RuleMeta, RuleOrigin, Severity};
use crate::linter::rule_config::get_all_rule_names;

/// The top-level keys of a rule file.
const TOP_LEVEL_KEYS: [&str; 10] = [
    "id",
    "severity",
    "message",
    "help",
    "rationale",
    "rule",
    "constraints",
    "fix",
    "fixSafety",
    "tests",
];

/// The keys allowed in a rule object (`rule:`, a constraint, `not:`, an item
/// of `all:` or `any:`).
const RULE_KEYS: [&str; 10] = [
    "pattern", "kind", "regex", "inside", "has", "follows", "precedes", "all", "any", "not",
];

/// Extra keys allowed in a relational rule (`inside`, `has`, `follows`,
/// `precedes`).
const RELATIONAL_KEYS: [&str; 2] = ["stopBy", "field"];

/// ast-grep rule keys that gdguard doesn't support in v1. They get a clearer
/// error than a plain unknown key.
const UNSUPPORTED_AST_GREP_KEYS: [&str; 6] = [
    "nthChild",
    "range",
    "matches",
    "utils",
    "transform",
    "rewriters",
];

/// The keys of the object form of `pattern`.
const PATTERN_OBJECT_KEYS: [&str; 3] = ["context", "selector", "strictness"];

/// The `tests:` section of a rule file, used by `gdguard test-rules`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuleTests {
    /// Code the rule must not report.
    pub valid: Vec<String>,
    /// Code the rule must report, with the expected source after the fix.
    pub invalid: Vec<InvalidCase>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidCase {
    pub code: String,
    /// The expected source after applying the rule's fix. `None` means the
    /// test only checks that the rule reports something.
    pub output: Option<String>,
    /// The exact number of diagnostics expected. `None` means at least one.
    pub errors: Option<usize>,
}

/// The parts of a rule the engine needs to check a file. Templates are kept
/// in the form ast-grep uses (see `language::to_internal`).
pub struct CompiledRule {
    pub matcher: RuleCore,
    /// The `fix:` template with metavariables in internal form. `None` when
    /// the rule has no fix. The provider renders it (see
    /// `provider::render_fix`).
    pub fix: Option<String>,
    pub fix_safety: FixSafety,
    /// The `message:` template with metavariables in internal form.
    pub message: String,
    /// The `help:` text with metavariables in internal form.
    pub help: Option<String>,
}

impl std::fmt::Debug for CompiledRule {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledRule")
            .field("fix", &self.fix)
            .field("fix_safety", &self.fix_safety)
            .field("message", &self.message)
            .field("help", &self.help)
            .finish_non_exhaustive()
    }
}

/// A validated custom rule.
#[derive(Debug)]
pub struct LoadedRule {
    pub meta: RuleMeta,
    pub compiled: CompiledRule,
    pub tests: RuleTests,
}

/// Loads every `*.yml` and `*.yaml` file in each folder of `rule_paths` (not
/// recursive), in path order. Stops at the first invalid file. Rule ids must
/// be unique across all files and must not clash with a built-in rule.
pub fn load_rules(rule_paths: &[PathBuf]) -> Result<Vec<LoadedRule>, ConfigError> {
    let mut rules: Vec<LoadedRule> = Vec::new();
    let mut id_to_path: HashMap<String, PathBuf> = HashMap::new();
    for folder in rule_paths {
        let files = find_rule_files(folder)?;
        for file in files {
            let rule = load_rule_file(&file)?;
            if let Some(first_path) = id_to_path.get(&rule.meta.id) {
                return Err(error_at(
                    &file,
                    Some("id"),
                    format!(
                        "duplicate rule id `{}`, already defined in {}",
                        rule.meta.id,
                        first_path.display()
                    ),
                ));
            }
            id_to_path.insert(rule.meta.id.clone(), file);
            rules.push(rule);
        }
    }
    Ok(rules)
}

/// Lists the rule files of one folder, sorted by path.
pub fn find_rule_files(folder: &Path) -> Result<Vec<PathBuf>, ConfigError> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) => {
            return Err(error_at(
                folder,
                Some("custom_rules.paths"),
                format!("can't read the rule folder: {}", error),
            ));
        }
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                return Err(error_at(
                    folder,
                    Some("custom_rules.paths"),
                    format!("can't read the rule folder: {}", error),
                ));
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_yaml = match path.extension() {
            Some(extension) => extension == "yml" || extension == "yaml",
            None => false,
        };
        if is_yaml {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

/// Reads and validates one rule file.
pub fn load_rule_file(path: &Path) -> Result<LoadedRule, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            return Err(error_at(
                path,
                None,
                format!("can't read the file: {}", error),
            ));
        }
    };
    load_rule_str(path, &text)
}

/// Validates the rule in `text`. `path` is used for errors and `RuleOrigin`.
pub fn load_rule_str(path: &Path, text: &str) -> Result<LoadedRule, ConfigError> {
    let document: Value = match ast_grep_config::from_str(text) {
        Ok(document) => document,
        Err(error) => {
            return Err(error_at(path, None, format!("invalid YAML: {}", error)));
        }
    };
    let Value::Object(fields) = document else {
        return Err(error_at(
            path,
            None,
            "a rule file must be a YAML mapping with `id`, `message` and `rule`".to_string(),
        ));
    };
    for key in fields.keys() {
        if !TOP_LEVEL_KEYS.contains(&key.as_str()) {
            return Err(unknown_key_error(path, key, key, &TOP_LEVEL_KEYS));
        }
    }

    let id = required_string(path, &fields, "id")?;
    validate_id(path, &id)?;
    let message = required_string(path, &fields, "message")?;
    let help = optional_string(path, &fields, "help")?;
    let rationale = optional_string(path, &fields, "rationale")?;

    let severity = match optional_string(path, &fields, "severity")? {
        Some(name) => match Severity::from_name(&name) {
            Some(severity) => severity,
            None => {
                return Err(error_at(
                    path,
                    Some("severity"),
                    format!(
                        "unknown severity `{}`, expected `error`, `warning` or `info`",
                        name
                    ),
                ));
            }
        },
        None => Severity::Warning,
    };

    let fix = optional_string(path, &fields, "fix")?;
    let fix_safety = match optional_string(path, &fields, "fixSafety")? {
        Some(name) => {
            let Some(safety) = FixSafety::from_name(&name) else {
                return Err(error_at(
                    path,
                    Some("fixSafety"),
                    format!(
                        "unknown fix safety `{}`, expected `safe`, `unsafe` or `suggestion`",
                        name
                    ),
                ));
            };
            if fix.is_none() {
                return Err(error_at(
                    path,
                    Some("fixSafety"),
                    "`fixSafety` needs a `fix`".to_string(),
                ));
            }
            safety
        }
        None => FixSafety::Suggestion,
    };

    let Some(rule_value) = fields.get("rule") else {
        return Err(error_at(
            path,
            Some("rule"),
            "missing required key".to_string(),
        ));
    };
    validate_rule_object(path, rule_value, "rule", false)?;
    if !has_positive_matcher(rule_value) {
        return Err(error_at(
            path,
            Some("rule"),
            "needs at least one of `pattern`, `kind`, `regex`, `all` or `any`; a rule with only `not` or relational keys matches almost every node".to_string(),
        ));
    }
    let rule: SerializableRule = match serde_json::from_value(rule_value.clone()) {
        Ok(rule) => rule,
        Err(error) => {
            return Err(error_at(path, Some("rule"), error.to_string()));
        }
    };

    let mut constraints: Option<HashMap<String, SerializableRule>> = None;
    if let Some(constraints_value) = fields.get("constraints") {
        let Value::Object(constraint_fields) = constraints_value else {
            return Err(error_at(
                path,
                Some("constraints"),
                "expected a mapping from metavariable name to rule".to_string(),
            ));
        };
        let mut parsed = HashMap::with_capacity(constraint_fields.len());
        for (name, constraint_value) in constraint_fields {
            let key = format!("constraints.{}", name);
            validate_rule_object(path, constraint_value, &key, false)?;
            let constraint: SerializableRule =
                match serde_json::from_value(constraint_value.clone()) {
                    Ok(constraint) => constraint,
                    Err(error) => return Err(error_at(path, Some(&key), error.to_string())),
                };
            parsed.insert(name.clone(), constraint);
        }
        constraints = Some(parsed);
    }

    let core = SerializableRuleCore {
        rule,
        constraints,
        utils: None,
        transform: None,
    };
    let matcher = match core.get_matcher(DeserializeEnv::new(GdScript)) {
        Ok(matcher) => matcher,
        Err(error) => {
            return Err(error_at(path, Some("rule"), error_chain_message(&error)));
        }
    };

    let defined = matcher.defined_vars();
    let internal_message = to_internal(&message).into_owned();
    check_metavariables(path, "message", &internal_message, &defined)?;
    let mut internal_help = None;
    if let Some(help_text) = &help {
        let internal = to_internal(help_text).into_owned();
        check_metavariables(path, "help", &internal, &defined)?;
        internal_help = Some(internal);
    }
    let mut internal_fix = None;
    if let Some(fix_template) = &fix {
        let internal = to_internal(fix_template).into_owned();
        check_metavariables(path, "fix", &internal, &defined)?;
        internal_fix = Some(internal);
    }

    let tests = match fields.get("tests") {
        Some(tests_value) => parse_tests(path, tests_value)?,
        None => RuleTests::default(),
    };

    let mut bad_example = String::new();
    if let Some(first) = tests.invalid.first() {
        bad_example = first.code.clone();
    }
    let mut good_example = String::new();
    if let Some(first) = tests.valid.first() {
        good_example = first.clone();
    }
    let fixable = internal_fix.is_some() && fix_safety != FixSafety::Suggestion;
    let meta = RuleMeta {
        id,
        description: message.clone(),
        rationale: rationale.unwrap_or_default(),
        bad_example,
        good_example,
        default_severity: severity,
        fixable,
        origin: RuleOrigin::Custom {
            path: path.to_path_buf(),
        },
    };
    Ok(LoadedRule {
        meta,
        compiled: CompiledRule {
            matcher,
            fix: internal_fix,
            fix_safety,
            message: internal_message,
            help: internal_help,
        },
        tests,
    })
}

fn error_at(path: &Path, key: Option<&str>, message: String) -> ConfigError {
    ConfigError {
        path: Some(path.to_path_buf()),
        key: key.map(str::to_string),
        message,
    }
}

fn unknown_key_error(path: &Path, key_path: &str, key: &str, allowed: &[&str]) -> ConfigError {
    if UNSUPPORTED_AST_GREP_KEYS.contains(&key) {
        return error_at(
            path,
            Some(key_path),
            format!("`{}` is not supported in gdguard rules", key),
        );
    }
    let mut expected = String::new();
    for (index, name) in allowed.iter().enumerate() {
        if index > 0 {
            expected.push_str(", ");
        }
        expected.push('`');
        expected.push_str(name);
        expected.push('`');
    }
    error_at(
        path,
        Some(key_path),
        format!("unknown key, expected one of {}", expected),
    )
}

/// Joins an error and its sources: ast-grep wraps the useful detail.
fn error_chain_message(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(inner) = source {
        let inner_message = inner.to_string();
        if !message.contains(&inner_message) {
            message.push_str(": ");
            message.push_str(&inner_message);
        }
        source = inner.source();
    }
    message
}

fn required_string(
    path: &Path,
    fields: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<String, ConfigError> {
    match optional_string(path, fields, key)? {
        Some(value) => Ok(value),
        None => Err(error_at(
            path,
            Some(key),
            "missing required key".to_string(),
        )),
    }
}

fn optional_string(
    path: &Path,
    fields: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ConfigError> {
    match fields.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(error_at(path, Some(key), "expected a string".to_string())),
    }
}

/// Checks that `id` is kebab-case and doesn't clash with a built-in rule.
fn validate_id(path: &Path, id: &str) -> Result<(), ConfigError> {
    if !is_kebab_case(id) {
        return Err(error_at(
            path,
            Some("id"),
            format!(
                "rule id `{}` must be kebab-case (lowercase letters, digits and single dashes, starting with a letter)",
                id
            ),
        ));
    }
    for built_in in get_all_rule_names() {
        if built_in == id {
            return Err(error_at(
                path,
                Some("id"),
                format!("rule id `{}` is already used by a built-in rule", id),
            ));
        }
    }
    Ok(())
}

fn is_kebab_case(id: &str) -> bool {
    let bytes = id.as_bytes();
    if bytes.is_empty() || !bytes[0].is_ascii_lowercase() {
        return false;
    }
    let mut previous_was_dash = false;
    for &byte in bytes {
        if byte == b'-' {
            if previous_was_dash {
                return false;
            }
            previous_was_dash = true;
        } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
            previous_was_dash = false;
        } else {
            return false;
        }
    }
    !previous_was_dash
}

/// True when the rule object has a key that selects nodes by itself.
fn has_positive_matcher(value: &Value) -> bool {
    let Value::Object(fields) = value else {
        return false;
    };
    for key in ["pattern", "kind", "regex", "all", "any"] {
        if fields.contains_key(key) {
            return true;
        }
    }
    false
}

/// Checks the keys of a rule object and every pattern in it, recursively.
/// `key_path` names the object in errors, for example `rule.inside.has`.
fn validate_rule_object(
    path: &Path,
    value: &Value,
    key_path: &str,
    is_relational: bool,
) -> Result<(), ConfigError> {
    let Value::Object(fields) = value else {
        return Err(error_at(
            path,
            Some(key_path),
            "expected a rule object (a mapping)".to_string(),
        ));
    };
    for (key, child) in fields {
        let child_path = format!("{}.{}", key_path, key);
        let is_rule_key = RULE_KEYS.contains(&key.as_str());
        let is_relational_key = is_relational && RELATIONAL_KEYS.contains(&key.as_str());
        if !is_rule_key && !is_relational_key {
            let mut allowed: Vec<&str> = Vec::with_capacity(RULE_KEYS.len() + 2);
            for name in RULE_KEYS {
                allowed.push(name);
            }
            if is_relational {
                for name in RELATIONAL_KEYS {
                    allowed.push(name);
                }
            }
            return Err(unknown_key_error(path, &child_path, key, &allowed));
        }
        match key.as_str() {
            "pattern" => validate_pattern_value(path, child, &child_path)?,
            "inside" | "has" | "follows" | "precedes" => {
                validate_rule_object(path, child, &child_path, true)?;
            }
            "not" => validate_rule_object(path, child, &child_path, false)?,
            "all" | "any" => {
                let Value::Array(items) = child else {
                    return Err(error_at(
                        path,
                        Some(&child_path),
                        "expected a list of rule objects".to_string(),
                    ));
                };
                for (index, item) in items.iter().enumerate() {
                    let item_path = format!("{}[{}]", child_path, index);
                    validate_rule_object(path, item, &item_path, false)?;
                }
            }
            "stopBy" => {
                // `neighbor`, `end` or a rule object.
                if let Value::Object(_) = child {
                    validate_rule_object(path, child, &child_path, false)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_pattern_value(path: &Path, value: &Value, key_path: &str) -> Result<(), ConfigError> {
    match value {
        Value::String(pattern) => validate_pattern_source(path, pattern, key_path),
        Value::Object(fields) => {
            for key in fields.keys() {
                if !PATTERN_OBJECT_KEYS.contains(&key.as_str()) {
                    let child_path = format!("{}.{}", key_path, key);
                    return Err(unknown_key_error(
                        path,
                        &child_path,
                        key,
                        &PATTERN_OBJECT_KEYS,
                    ));
                }
            }
            match fields.get("context") {
                Some(Value::String(context)) => {
                    let context_path = format!("{}.context", key_path);
                    validate_pattern_source(path, context, &context_path)
                }
                _ => Err(error_at(
                    path,
                    Some(key_path),
                    "the object form of `pattern` needs a `context` string".to_string(),
                )),
            }
        }
        _ => Err(error_at(
            path,
            Some(key_path),
            "expected a pattern string or an object with `context`".to_string(),
        )),
    }
}

/// Parses a pattern as GDScript and rejects syntax errors, which ast-grep
/// would otherwise accept silently.
fn validate_pattern_source(path: &Path, pattern: &str, key_path: &str) -> Result<(), ConfigError> {
    let internal = to_internal(pattern);
    let mut parser = tree_sitter::Parser::new();
    if parser
        .set_language(&tree_sitter_gdscript::LANGUAGE.into())
        .is_err()
    {
        return Err(error_at(
            path,
            Some(key_path),
            "can't load the GDScript grammar".to_string(),
        ));
    }
    let Some(tree) = parser.parse(internal.as_ref(), None) else {
        return Err(error_at(
            path,
            Some(key_path),
            format!("pattern `{}` doesn't parse as GDScript", pattern),
        ));
    };
    if tree.root_node().has_error() {
        return Err(error_at(
            path,
            Some(key_path),
            format!("pattern `{}` doesn't parse as GDScript", pattern),
        ));
    }
    Ok(())
}

/// Checks that every metavariable in `template` (internal form) is defined
/// by the rule's patterns.
fn check_metavariables(
    path: &Path,
    key: &str,
    template: &str,
    defined: &HashSet<&str>,
) -> Result<(), ConfigError> {
    let characters: Vec<char> = template.chars().collect();
    let mut index = 0;
    while index < characters.len() {
        if characters[index] != EXPANDO_CHAR {
            index += 1;
            continue;
        }
        while index < characters.len() && characters[index] == EXPANDO_CHAR {
            index += 1;
        }
        let name_start = index;
        while index < characters.len()
            && (characters[index].is_ascii_uppercase()
                || characters[index].is_ascii_digit()
                || characters[index] == '_')
        {
            index += 1;
        }
        let name: String = characters[name_start..index].iter().collect();
        if name.is_empty() || name.starts_with('_') {
            continue;
        }
        if !defined.contains(name.as_str()) {
            return Err(error_at(
                path,
                Some(key),
                format!(
                    "uses the metavariable `${}`, which the rule's patterns don't define",
                    name
                ),
            ));
        }
    }
    Ok(())
}

fn parse_tests(path: &Path, value: &Value) -> Result<RuleTests, ConfigError> {
    let Value::Object(fields) = value else {
        return Err(error_at(
            path,
            Some("tests"),
            "expected a mapping with `valid` and `invalid`".to_string(),
        ));
    };
    let mut tests = RuleTests::default();
    for (key, child) in fields {
        let key_path = format!("tests.{}", key);
        match key.as_str() {
            "valid" => {
                let Value::Array(items) = child else {
                    return Err(error_at(
                        path,
                        Some(&key_path),
                        "expected a list of code strings".to_string(),
                    ));
                };
                for (index, item) in items.iter().enumerate() {
                    let Value::String(code) = item else {
                        return Err(error_at(
                            path,
                            Some(&format!("{}[{}]", key_path, index)),
                            "expected a code string".to_string(),
                        ));
                    };
                    tests.valid.push(code.clone());
                }
            }
            "invalid" => {
                let Value::Array(items) = child else {
                    return Err(error_at(
                        path,
                        Some(&key_path),
                        "expected a list of `{ code, output }` entries".to_string(),
                    ));
                };
                for (index, item) in items.iter().enumerate() {
                    let item_path = format!("{}[{}]", key_path, index);
                    tests
                        .invalid
                        .push(parse_invalid_case(path, item, &item_path)?);
                }
            }
            _ => {
                return Err(unknown_key_error(
                    path,
                    &key_path,
                    key,
                    &["valid", "invalid"],
                ));
            }
        }
    }
    Ok(tests)
}

fn parse_invalid_case(
    path: &Path,
    value: &Value,
    key_path: &str,
) -> Result<InvalidCase, ConfigError> {
    // A bare string is accepted as `{ code: ... }` with no expected output.
    if let Value::String(code) = value {
        return Ok(InvalidCase {
            code: code.clone(),
            output: None,
            errors: None,
        });
    }
    let Value::Object(fields) = value else {
        return Err(error_at(
            path,
            Some(key_path),
            "expected `{ code, output, errors }` or a code string".to_string(),
        ));
    };
    let mut code = None;
    let mut output = None;
    let mut errors = None;
    for (key, child) in fields {
        let child_path = format!("{}.{}", key_path, key);
        match key.as_str() {
            "code" | "output" => {
                let Value::String(text) = child else {
                    return Err(error_at(
                        path,
                        Some(&child_path),
                        "expected a string".to_string(),
                    ));
                };
                if key == "code" {
                    code = Some(text.clone());
                } else {
                    output = Some(text.clone());
                }
            }
            "errors" => {
                let count = match child.as_u64() {
                    Some(count) if count > 0 => count,
                    _ => {
                        return Err(error_at(
                            path,
                            Some(&child_path),
                            "expected a number greater than 0".to_string(),
                        ));
                    }
                };
                errors = Some(count as usize);
            }
            _ => {
                return Err(unknown_key_error(
                    path,
                    &child_path,
                    key,
                    &["code", "output", "errors"],
                ));
            }
        }
    }
    let Some(code) = code else {
        let child_path = format!("{}.code", key_path);
        return Err(error_at(
            path,
            Some(&child_path),
            "missing required key".to_string(),
        ));
    };
    Ok(InvalidCase {
        code,
        output,
        errors,
    })
}
