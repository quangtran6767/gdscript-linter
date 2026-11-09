//! Owner: Track B (W3). `gdguard explain <RULE>` and `lint --list-rules`.
//!
//! Both show built-in rules (from `rules_meta`) and the custom rules of the
//! config found from the current folder.

use std::path::Path;

use crate::linter::api::{Config, load_config_for};
use crate::linter::custom::load_providers;
use crate::linter::diagnostic::{RuleMeta, RuleOrigin};
use crate::linter::rules_meta::builtin_rule_metas;

/// Arguments of `gdguard explain <RULE>`, parsed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainArgs {
    pub rule: String,
}

/// The `--format` value of `lint --list-rules`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListRulesFormat {
    Text,
    Json,
}

/// Version of the `--list-rules --format json` output.
pub const LIST_RULES_JSON_VERSION: u32 = 1;

/// Runs `gdguard explain` and returns the process exit code: 0 when the rule
/// exists, 2 for an unknown rule or a config error.
pub fn run(args: &ExplainArgs) -> i32 {
    let metas = match rule_metas_for_current_folder() {
        Ok(metas) => metas,
        Err(message) => {
            eprintln!("Error: {}", message);
            return 2;
        }
    };
    match explain(&args.rule, &metas) {
        Ok(text) => {
            print!("{}", text);
            0
        }
        Err(message) => {
            eprintln!("Error: {}", message);
            2
        }
    }
}

/// Runs `lint --list-rules` and returns the process exit code.
pub fn list_rules(format: ListRulesFormat) -> i32 {
    let metas = match rule_metas_for_current_folder() {
        Ok(metas) => metas,
        Err(message) => {
            eprintln!("Error: {}", message);
            return 2;
        }
    };
    match format {
        ListRulesFormat::Text => print!("{}", format_rule_list_text(&metas)),
        ListRulesFormat::Json => println!("{}", format_rule_list_json(&metas)),
    }
    0
}

fn rule_metas_for_current_folder() -> Result<Vec<RuleMeta>, String> {
    let config = match load_config_for(Path::new(".")) {
        Ok(config) => config,
        Err(error) => return Err(error.to_string()),
    };
    all_rule_metas(&config)
}

/// Built-in rules sorted by id, then the config's custom rules in load order.
pub fn all_rule_metas(config: &Config) -> Result<Vec<RuleMeta>, String> {
    let mut metas = builtin_rule_metas();
    let providers = load_providers(config)?;
    for provider in &providers {
        for meta in provider.rules() {
            metas.push(meta.clone());
        }
    }
    Ok(metas)
}

/// The text `explain` prints for `rule_id`, or an error with a "did you
/// mean" suggestion when the id is unknown.
pub fn explain(rule_id: &str, metas: &[RuleMeta]) -> Result<String, String> {
    for meta in metas {
        if meta.id == rule_id {
            return Ok(format_explain(meta));
        }
    }
    // Granular names stay explainable without cluttering --list-rules.
    if let Some(meta) = super::rules_meta::builtin_rule_meta(rule_id) {
        return Ok(format_explain(&meta));
    }
    let mut message = format!("unknown rule `{}`", rule_id);
    let mut suggestions = metas.to_vec();
    for group in super::rule_groups::RULE_GROUPS {
        for check in group.checks {
            if let Some(meta) = super::rules_meta::builtin_rule_meta(check) {
                suggestions.push(meta);
            }
        }
    }
    if let Some(suggestion) = closest_rule_id(rule_id, &suggestions) {
        message.push_str(&format!("; did you mean `{}`?", suggestion));
    }
    message.push_str("\nRun `gdguard lint --list-rules` to see every rule.");
    Err(message)
}

fn origin_text(origin: &RuleOrigin) -> String {
    match origin {
        RuleOrigin::BuiltIn => "built-in".to_string(),
        RuleOrigin::Custom { path } => format!("custom ({})", display_path(path)),
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// Indents every line of a code example by four spaces.
fn indent_example(code: &str) -> String {
    let mut text = String::with_capacity(code.len() + 32);
    let code = code.strip_suffix('\n').unwrap_or(code);
    for line in code.split('\n') {
        if !line.is_empty() {
            text.push_str("    ");
            text.push_str(line);
        }
        text.push('\n');
    }
    text
}

/// The full `explain` text of one rule.
pub fn format_explain(meta: &RuleMeta) -> String {
    let mut text = String::new();
    text.push_str(&meta.id);
    text.push('\n');
    text.push_str(&format!("  Origin:   {}\n", origin_text(&meta.origin)));
    text.push_str(&format!("  Severity: {}\n", meta.default_severity.as_str()));
    let fixable = if meta.fixable { "yes" } else { "no" };
    text.push_str(&format!("  Fixable:  {}\n", fixable));
    text.push('\n');
    text.push_str(&meta.description);
    text.push('\n');
    if !meta.rationale.is_empty() {
        text.push_str("\nWhy:\n");
        let rationale = meta.rationale.trim_end();
        for line in rationale.split('\n') {
            if !line.is_empty() {
                text.push_str("  ");
                text.push_str(line);
            }
            text.push('\n');
        }
    }
    if !meta.bad_example.is_empty() {
        text.push_str("\nBad:\n");
        text.push_str(&indent_example(&meta.bad_example));
    }
    if !meta.good_example.is_empty() {
        text.push_str("\nGood:\n");
        text.push_str(&indent_example(&meta.good_example));
    }
    text
}

/// One line per rule: id, default severity, `fix` when fixable, and the
/// description; custom rules are marked.
pub fn format_rule_list_text(metas: &[RuleMeta]) -> String {
    let mut id_width = 0;
    let mut custom_count = 0;
    for meta in metas {
        id_width = id_width.max(meta.id.len());
        if let RuleOrigin::Custom { .. } = meta.origin {
            custom_count += 1;
        }
    }
    let mut text = String::from("Available linting rules:\n");
    for meta in metas {
        let fix = if meta.fixable { "fix" } else { "   " };
        let custom = match meta.origin {
            RuleOrigin::Custom { .. } => " (custom)",
            RuleOrigin::BuiltIn => "",
        };
        text.push_str(&format!(
            "  {:<id_width$}  {:<7}  {}  {}{}\n",
            meta.id,
            meta.default_severity.as_str(),
            fix,
            meta.description,
            custom,
            id_width = id_width
        ));
    }
    text.push_str(&format!(
        "\n{} rules ({} built-in, {} custom). Run `gdguard explain <rule>` for details.\n",
        metas.len(),
        metas.len() - custom_count,
        custom_count
    ));
    text
}

/// The `--list-rules --format json` document:
/// `{ "version": 1, "rules": [ { "id", "description", "rationale",
/// "default_severity", "fixable", "origin", "path", "bad_example",
/// "good_example" } ] }`. `origin` is `"built-in"` or `"custom"`; `path` is
/// the rule file for custom rules and `null` for built-in rules.
pub fn format_rule_list_json(metas: &[RuleMeta]) -> String {
    let mut rules = Vec::with_capacity(metas.len());
    for meta in metas {
        let (origin, path) = match &meta.origin {
            RuleOrigin::BuiltIn => ("built-in", serde_json::Value::Null),
            RuleOrigin::Custom { path } => {
                ("custom", serde_json::Value::String(display_path(path)))
            }
        };
        rules.push(serde_json::json!({
            "id": meta.id,
            "description": meta.description,
            "rationale": meta.rationale,
            "default_severity": meta.default_severity.as_str(),
            "fixable": meta.fixable,
            "origin": origin,
            "path": path,
            "bad_example": meta.bad_example,
            "good_example": meta.good_example,
        }));
    }
    let document = serde_json::json!({
        "version": LIST_RULES_JSON_VERSION,
        "rules": rules,
    });
    match serde_json::to_string_pretty(&document) {
        Ok(text) => text,
        // A `Value` built from strings and booleans always serializes.
        Err(error) => format!("{{\"error\": \"{}\"}}", error),
    }
}

/// The known id closest to `rule_id`, when it is close enough to be a typo.
fn closest_rule_id<'a>(rule_id: &str, metas: &'a [RuleMeta]) -> Option<&'a str> {
    let mut best: Option<&'a str> = None;
    let mut best_distance = usize::MAX;
    for meta in metas {
        let distance = edit_distance(rule_id, &meta.id);
        if distance < best_distance {
            best_distance = distance;
            best = Some(meta.id.as_str());
        }
    }
    let maximum_distance = (rule_id.chars().count() / 3).max(2);
    if best_distance <= maximum_distance {
        return best;
    }
    None
}

/// Levenshtein distance, counted in characters.
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
