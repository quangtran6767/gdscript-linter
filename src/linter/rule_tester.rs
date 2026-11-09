//! Owner: Track B (W2). `gdguard test-rules [RULE_DIR]`: runs the `tests` of
//! custom rules and checks fixes with `apply_edits`.
//!
//! For each rule:
//! - every `valid` case must produce no diagnostic for that rule;
//! - every `invalid` case must produce at least one (or exactly `errors`);
//! - when an `invalid` case has `output`, the rule's fixes are applied (all
//!   safety levels) and the result must equal `output`, must parse without
//!   errors, and must produce no diagnostic when the rule runs again
//!   (idempotency).

use std::path::{Path, PathBuf};

use crate::linter::api::load_config_for;
use crate::linter::custom::YamlRuleProvider;
use crate::linter::custom::loader::{LoadedRule, RuleTests, load_rules};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, apply_edits};

/// The rules folder used when no folder is given and the config has no
/// `[custom_rules].paths`.
pub const DEFAULT_RULE_DIR: &str = ".gdguard/rules";

/// Arguments of `gdguard test-rules [RULE_DIR]`, parsed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestRulesArgs {
    /// `None` means: the custom rule paths from the config, or
    /// `.gdguard/rules`.
    pub rule_dir: Option<PathBuf>,
}

/// The result of one rule's tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleReport {
    pub id: String,
    pub passed: usize,
    /// One readable explanation per failed case.
    pub failures: Vec<String>,
}

/// Runs `gdguard test-rules` and returns the process exit code: 0 when every
/// test passes, 1 when a test fails, 2 when the rules can't be loaded.
pub fn run(args: &TestRulesArgs) -> i32 {
    let folders = match rule_folders(args) {
        Ok(folders) => folders,
        Err(message) => {
            eprintln!("Error: {}", message);
            return 2;
        }
    };
    let rules = match load_rules(&folders) {
        Ok(rules) => rules,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 2;
        }
    };
    if rules.is_empty()
        && crate::linter::custom::find_js_files(&folders).is_ok_and(|files| files.is_empty())
    {
        let mut names = Vec::with_capacity(folders.len());
        for folder in &folders {
            names.push(folder.display().to_string());
        }
        println!("No rules found in {}", names.join(", "));
        return 0;
    }
    #[cfg(feature = "js-rules")]
    let existing_metas: Vec<_> = rules.iter().map(|rule| rule.meta.clone()).collect();
    #[allow(unused_mut)]
    let mut reports = test_rules(rules);
    let js_files = match crate::linter::custom::find_js_files(&folders) {
        Ok(files) => files,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 2;
        }
    };
    if !js_files.is_empty() {
        #[cfg(feature = "js-rules")]
        {
            let config = crate::linter::api::Config {
                custom_rule_paths: folders,
                ..Default::default()
            };
            let provider = match crate::linter::script::JsRuleProvider::load(
                &config,
                &js_files,
                &existing_metas,
            ) {
                Ok(provider) => provider,
                Err(error) => {
                    eprintln!("Error: {}", error);
                    return 2;
                }
            };
            reports.extend(crate::linter::script::test_rules(provider));
        }
        #[cfg(not(feature = "js-rules"))]
        {
            eprintln!("Error: JavaScript rules require the js-rules feature");
            return 2;
        }
    }
    print!("{}", format_reports(&reports));
    exit_code(&reports)
}

/// The folders to load: the argument, else the config's custom rule paths,
/// else `.gdguard/rules`.
fn rule_folders(args: &TestRulesArgs) -> Result<Vec<PathBuf>, String> {
    if let Some(folder) = &args.rule_dir {
        return Ok(vec![folder.clone()]);
    }
    let config = match load_config_for(Path::new(".")) {
        Ok(config) => config,
        Err(error) => return Err(error.to_string()),
    };
    if !config.custom_rule_paths.is_empty() {
        return Ok(config.custom_rule_paths);
    }
    Ok(vec![PathBuf::from(DEFAULT_RULE_DIR)])
}

/// 0 when every rule passed, 1 otherwise.
pub fn exit_code(reports: &[RuleReport]) -> i32 {
    for report in reports {
        if !report.failures.is_empty() {
            return 1;
        }
    }
    0
}

/// Runs the tests of every rule, in load order.
pub fn test_rules(rules: Vec<LoadedRule>) -> Vec<RuleReport> {
    // The provider takes ownership of the rules, so keep what the tests need.
    let mut cases = Vec::with_capacity(rules.len());
    for rule in &rules {
        cases.push((
            rule.meta.id.clone(),
            rule.compiled.fix.is_some(),
            rule.tests.clone(),
        ));
    }
    let provider = YamlRuleProvider::new(rules);
    let mut reports = Vec::with_capacity(cases.len());
    for (id, has_fix, tests) in &cases {
        reports.push(test_one_rule(&provider, id, *has_fix, tests));
    }
    reports
}

fn test_one_rule(
    provider: &YamlRuleProvider,
    id: &str,
    has_fix: bool,
    tests: &RuleTests,
) -> RuleReport {
    let mut report = RuleReport {
        id: id.to_string(),
        passed: 0,
        failures: Vec::new(),
    };

    for (index, code) in tests.valid.iter().enumerate() {
        let diagnostics = match check(provider, id, code) {
            Ok(diagnostics) => diagnostics,
            Err(message) => {
                report
                    .failures
                    .push(format!("valid[{}]: {}", index, message));
                continue;
            }
        };
        if diagnostics.is_empty() {
            report.passed += 1;
        } else {
            report.failures.push(format!(
                "valid[{}]: expected no diagnostics, got {}:\n{}{}",
                index,
                diagnostics.len(),
                describe_diagnostics(&diagnostics),
                indent_code(code)
            ));
        }
    }

    for (index, case) in tests.invalid.iter().enumerate() {
        let label = format!("invalid[{}]", index);
        let diagnostics = match check(provider, id, &case.code) {
            Ok(diagnostics) => diagnostics,
            Err(message) => {
                report.failures.push(format!("{}: {}", label, message));
                continue;
            }
        };
        let count_is_right = match case.errors {
            Some(expected) => diagnostics.len() == expected,
            None => !diagnostics.is_empty(),
        };
        if !count_is_right {
            let expected = match case.errors {
                Some(expected) => format!("exactly {}", expected),
                None => "at least 1".to_string(),
            };
            report.failures.push(format!(
                "{}: expected {} diagnostics, got {}\n{}{}",
                label,
                expected,
                diagnostics.len(),
                describe_diagnostics(&diagnostics),
                indent_code(&case.code)
            ));
            continue;
        }
        let Some(expected_output) = &case.output else {
            report.passed += 1;
            continue;
        };
        if !has_fix {
            report
                .failures
                .push(format!("{}: has `output` but the rule has no `fix`", label));
            continue;
        }
        match check_fix(provider, id, &case.code, &diagnostics, expected_output) {
            Ok(()) => report.passed += 1,
            Err(message) => report.failures.push(format!("{}: {}", label, message)),
        }
    }
    report
}

/// Applies the first fix of every diagnostic and checks the result.
fn check_fix(
    provider: &YamlRuleProvider,
    id: &str,
    code: &str,
    diagnostics: &[Diagnostic],
    expected_output: &str,
) -> Result<(), String> {
    let mut edits: Vec<Edit> = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics {
        if let Some(fix) = diagnostic.fixes.first() {
            for edit in &fix.edits {
                edits.push(edit.clone());
            }
        }
    }
    let fixed = match apply_edits(code, &edits) {
        Ok(fixed) => fixed,
        Err(error) => return Err(format!("the fixes can't be applied: {}", error)),
    };
    if fixed != expected_output {
        return Err(format!(
            "the fixed code differs from `output` (- expected, + actual):\n{}",
            line_diff(expected_output, &fixed)
        ));
    }
    let tree = parse(&fixed)?;
    if tree.root_node().has_error() {
        return Err(format!(
            "the fixed code doesn't parse as GDScript:\n{}",
            indent_code(&fixed)
        ));
    }
    let context = FileContext {
        path: Path::new("test.gd"),
        source: &fixed,
        tree: &tree,
        semantic: None,
        project: None,
    };
    let again = provider.check_rule(id, &context);
    if !again.is_empty() {
        return Err(format!(
            "the fix is not idempotent: the rule still reports {} diagnostics on the fixed code:\n{}{}",
            again.len(),
            describe_diagnostics(&again),
            indent_code(&fixed)
        ));
    }
    Ok(())
}

pub(crate) fn parse(code: &str) -> Result<tree_sitter::Tree, String> {
    let mut parser = tree_sitter::Parser::new();
    if let Err(error) = parser.set_language(&tree_sitter_gdscript::LANGUAGE.into()) {
        return Err(format!("can't load the GDScript grammar: {}", error));
    }
    match parser.parse(code, None) {
        Some(tree) => Ok(tree),
        None => Err("tree-sitter returned no tree".to_string()),
    }
}

/// Runs only rule `id` on `code`.
fn check(provider: &YamlRuleProvider, id: &str, code: &str) -> Result<Vec<Diagnostic>, String> {
    let tree = parse(code)?;
    let context = FileContext {
        path: Path::new("test.gd"),
        source: code,
        tree: &tree,
        semantic: None,
        project: None,
    };
    Ok(provider.check_rule(id, &context))
}

fn describe_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut text = String::new();
    for diagnostic in diagnostics {
        text.push_str(&format!(
            "    {}:{}: {}\n",
            diagnostic.span.start.line, diagnostic.span.start.column, diagnostic.message
        ));
    }
    text
}

/// Shows test code under a failure, indented, with a "code:" header.
fn indent_code(code: &str) -> String {
    let mut text = String::from("    code:\n");
    let code = code.strip_suffix('\n').unwrap_or(code);
    for line in code.split('\n') {
        text.push_str("    | ");
        text.push_str(line);
        text.push('\n');
    }
    text
}

/// A simple line diff: equal lines start with two spaces, expected lines with
/// `- ` and actual lines with `+ `. Lines are compared by position, which is
/// enough for the short snippets in rule tests.
pub fn line_diff(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let actual_lines: Vec<&str> = actual.split('\n').collect();
    // Both usually end with a newline, which would show as an empty last line.
    let mut line_count = expected_lines.len().max(actual_lines.len());
    if expected_lines.last() == Some(&"") && actual_lines.last() == Some(&"") {
        line_count -= 1;
    }
    let mut text = String::new();
    for index in 0..line_count {
        let expected_line = expected_lines.get(index);
        let actual_line = actual_lines.get(index);
        if expected_line == actual_line {
            if let Some(line) = expected_line {
                text.push_str(&format!("    | {}\n", line));
            }
            continue;
        }
        if let Some(line) = expected_line {
            text.push_str(&format!("    - {}\n", line));
        }
        if let Some(line) = actual_line {
            text.push_str(&format!("    + {}\n", line));
        }
    }
    text
}

/// One line per rule (`✓ id  N passed` or `✗ id  F failed, N passed`), the
/// failures under it, then a summary line.
pub fn format_reports(reports: &[RuleReport]) -> String {
    let mut text = String::new();
    let mut failed_rules = 0;
    for report in reports {
        if report.failures.is_empty() {
            text.push_str(&format!("✓ {}  {} passed\n", report.id, report.passed));
            continue;
        }
        failed_rules += 1;
        text.push_str(&format!(
            "✗ {}  {} failed, {} passed\n",
            report.id,
            report.failures.len(),
            report.passed
        ));
        for failure in &report.failures {
            text.push_str("  ");
            text.push_str(failure);
            if !failure.ends_with('\n') {
                text.push('\n');
            }
        }
    }
    text.push_str(&format!(
        "\n{} rules, {} passed, {} failed\n",
        reports.len(),
        reports.len() - failed_rules,
        failed_rules
    ));
    text
}
