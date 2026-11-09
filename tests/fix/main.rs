//! Tests for the fix engine (`src/linter/fix.rs`) and `lint --fix`,
//! `--fix-unsafe`, `--diff` and `--stdin-filename`.
//!
//! Built-in fix fixtures live in `tests/fix/builtin/`: `<name>.input.gd` is
//! fixed and compared with `<name>.expected.gd`.

#![allow(clippy::unwrap_used)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use gdguard::linter::api::{self, Config};
use gdguard::linter::diagnostic::Severity;
use gdguard::linter::{LintRunOptions, fix_files};

/// A new empty folder under the system temp folder. Removed on drop.
struct TempProject {
    root: PathBuf,
}

impl TempProject {
    fn new() -> TempProject {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "gdguard-fix-test-{}-{}",
            std::process::id(),
            timestamp
        ));
        fs::create_dir_all(&root).unwrap();
        TempProject { root }
    }

    fn write(&self, relative_path: &str, content: &str) -> PathBuf {
        let path = self.root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
        path
    }

    fn read(&self, relative_path: &str) -> String {
        fs::read_to_string(self.root.join(relative_path)).unwrap()
    }

    /// Writes a `gdguard.toml` using the custom rules in `rules/`, and each
    /// `(file name, yaml)` rule.
    fn with_rules(&self, rules: &[(&str, &str)]) {
        self.write("gdguard.toml", "[custom_rules]\npaths = [\"rules\"]\n");
        for (file_name, yaml) in rules {
            self.write(&format!("rules/{}", file_name), yaml);
        }
    }

    fn config(&self) -> Config {
        api::load_config_for(&self.root.join("a.gd")).unwrap()
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const PREFER_IS_EMPTY: &str = "id: prefer-is-empty\nmessage: Use is_empty()\nrule:\n  pattern: len($X) == 0\nfix: $X.is_empty()\nfixSafety: safe\n";

/// Both matches of `unwrap(unwrap(1))` overlap, so the inner one waits for
/// the next pass.
const UNWRAP: &str =
    "id: unwrap\nmessage: Remove unwrap\nrule:\n  pattern: unwrap($X)\nfix: $X\nfixSafety: safe\n";

/// Its fix leaves an unclosed parenthesis.
const BREAK_CODE: &str = "id: break-code\nmessage: Broken fix\nrule:\n  pattern: break_me($X)\nfix: ($X\nfixSafety: safe\n";

/// Its fix adds spaces the formatter removes.
const SPACED_CALL: &str = "id: spaced-call\nmessage: Use new_call\nrule:\n  pattern: old_call($X)\nfix: new_call( $X )\nfixSafety: safe\n";

const UNSAFE_RENAME: &str = "id: unsafe-rename\nmessage: Rename\nrule:\n  pattern: risky()\nfix: careful()\nfixSafety: unsafe\n";

const SUGGESTION_ONLY: &str =
    "id: suggestion-only\nmessage: Maybe\nrule:\n  pattern: maybe()\nfix: surely()\n";

fn fix(project: &TempProject, source: &str, unsafe_fixes: bool) -> api::FixResult {
    api::fix_source(
        &project.root.join("a.gd"),
        source,
        &project.config(),
        unsafe_fixes,
    )
    .unwrap()
}

fn rules_of(result: &api::FixResult) -> Vec<String> {
    let mut rules = Vec::new();
    for diagnostic in &result.remaining {
        rules.push(diagnostic.rule.clone());
    }
    rules
}

// Engine

#[test]
fn applies_a_single_fix() {
    let project = TempProject::new();
    project.with_rules(&[("prefer-is-empty.yml", PREFER_IS_EMPTY)]);
    let result = fix(
        &project,
        "func f(items):\n\tif len(items) == 0:\n\t\tprint(1)\n",
        false,
    );
    assert_eq!(
        result.output,
        "func f(items):\n\tif items.is_empty():\n\t\tprint(1)\n"
    );
    assert_eq!(result.applied, 1);
    assert!(result.remaining.is_empty());
}

#[test]
fn overlapping_fixes_take_two_passes() {
    let project = TempProject::new();
    project.with_rules(&[("unwrap.yml", UNWRAP)]);
    let result = fix(&project, "var a = unwrap(unwrap(1))\n", false);
    assert_eq!(result.output, "var a = 1\n");
    assert_eq!(result.applied, 2);
    assert!(result.remaining.is_empty());
}

#[test]
fn fix_that_breaks_parsing_is_rolled_back_and_reported() {
    let project = TempProject::new();
    project.with_rules(&[
        ("break-code.yml", BREAK_CODE),
        ("prefer-is-empty.yml", PREFER_IS_EMPTY),
    ]);
    let source = "var a = break_me(1)\nvar b = len([]) == 0\n";
    let result = fix(&project, source, false);

    // The good fix is kept, the bad one isn't.
    assert_eq!(
        result.output,
        "var a = break_me(1)\nvar b = [].is_empty()\n"
    );
    assert_eq!(result.applied, 1);
    assert_eq!(rules_of(&result), vec!["break-code", "fix-failed"]);
    let failed = &result.remaining[1];
    assert_eq!(
        failed.message,
        "The fix for `break-code` was not applied because it would break the code"
    );
    assert_eq!(failed.severity, Severity::Warning);
    assert_eq!(
        &result.output[failed.span.start_byte..failed.span.end_byte],
        "break_me(1)"
    );
}

#[test]
fn fix_failed_follows_rule_levels() {
    let project = TempProject::new();
    project.write(
        "gdguard.toml",
        "[rules]\nfix-failed = \"error\"\n\n[custom_rules]\npaths = [\"rules\"]\n",
    );
    project.write("rules/break-code.yml", BREAK_CODE);
    let result = fix(&project, "var a = break_me(1)\n", false);
    assert_eq!(rules_of(&result), vec!["break-code", "fix-failed"]);
    assert_eq!(result.remaining[1].severity, Severity::Error);
}

#[test]
fn fixing_twice_gives_the_same_result_as_once() {
    let project = TempProject::new();
    project.with_rules(&[
        ("unwrap.yml", UNWRAP),
        ("prefer-is-empty.yml", PREFER_IS_EMPTY),
    ]);
    let source = "func f(items):\n\tprint(unwrap(unwrap(items)))\n\tif len(items) == 0:\n\t\tprint(1)\n\telse:\n\t\tpass\n\tpass\n";
    let once = fix(&project, source, true);
    let twice = fix(&project, &once.output, true);
    assert_eq!(twice.output, once.output);
    assert_eq!(twice.applied, 0);
}

#[test]
fn safety_levels_decide_which_fixes_apply() {
    let project = TempProject::new();
    project.with_rules(&[
        ("unsafe-rename.yml", UNSAFE_RENAME),
        ("suggestion-only.yml", SUGGESTION_ONLY),
    ]);
    let source = "func f():\n\trisky()\n\tmaybe()\n";

    let safe = fix(&project, source, false);
    assert_eq!(safe.output, source);
    assert_eq!(safe.applied, 0);
    assert_eq!(rules_of(&safe), vec!["unsafe-rename", "suggestion-only"]);

    let unsafe_result = fix(&project, source, true);
    assert_eq!(unsafe_result.output, "func f():\n\tcareful()\n\tmaybe()\n");
    assert_eq!(rules_of(&unsafe_result), vec!["suggestion-only"]);
}

#[test]
fn formats_the_result_only_when_the_input_was_formatted() {
    let project = TempProject::new();
    project.with_rules(&[("spaced-call.yml", SPACED_CALL)]);

    let formatted = fix(&project, "func f():\n\told_call(1)\n", false);
    assert_eq!(formatted.output, "func f():\n\tnew_call(1)\n");

    // `var  a=1` isn't formatted, so the formatter must not run.
    let unformatted = fix(&project, "var  a=1\n\n\nfunc f():\n\told_call(1)\n", false);
    assert_eq!(
        unformatted.output,
        "var  a=1\n\n\nfunc f():\n\tnew_call( 1 )\n"
    );
}

#[test]
fn source_with_syntax_errors_is_left_unchanged() {
    let project = TempProject::new();
    project.with_rules(&[("prefer-is-empty.yml", PREFER_IS_EMPTY)]);
    let source = "var b = len([]) == 0\nfunc (:\n";
    let result = fix(&project, source, false);
    assert_eq!(result.output, source);
    assert_eq!(result.applied, 0);
}

#[test]
fn fix_files_skips_excluded_files_and_reports_changes() {
    let project = TempProject::new();
    project.write(
        "gdguard.toml",
        "[custom_rules]\npaths = [\"rules\"]\n\n[lint]\nexclude = [\"vendor\"]\n",
    );
    project.write("rules/prefer-is-empty.yml", PREFER_IS_EMPTY);
    let fixed = project.write("a.gd", "var b = len([]) == 0\n");
    let clean = project.write("clean.gd", "var b = 1\n");
    let excluded = project.write("vendor/c.gd", "var b = len([]) == 0\n");

    let report = fix_files(&[fixed, clean, excluded], &LintRunOptions::default(), false).unwrap();
    assert_eq!(report.applied(), 1);
    assert_eq!(report.changed_files.len(), 1);
    assert_eq!(report.changed_files[0].fixed, "var b = [].is_empty()\n");
    assert_eq!(report.report.files_checked, 2);
    assert!(report.report.files.is_empty());
    // fix_files never writes.
    assert_eq!(project.read("a.gd"), "var b = len([]) == 0\n");
}

// Built-in fixes

fn check_builtin_fixture(name: &str, unsafe_fixes: bool) {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fix/builtin");
    let input = fs::read_to_string(folder.join(format!("{}.input.gd", name))).unwrap();
    let expected = fs::read_to_string(folder.join(format!("{}.expected.gd", name))).unwrap();
    let result = api::fix_source(
        &folder.join(format!("{}.input.gd", name)),
        &input,
        &Config::default(),
        unsafe_fixes,
    )
    .unwrap();
    assert_eq!(result.output, expected, "fixture {}", name);
    let again = api::fix_source(
        &folder.join(format!("{}.expected.gd", name)),
        &result.output,
        &Config::default(),
        unsafe_fixes,
    )
    .unwrap();
    assert_eq!(
        again.output, result.output,
        "fixture {} is not idempotent",
        name
    );
}

#[test]
fn builtin_fix_unnecessary_pass() {
    check_builtin_fixture("unnecessary_pass", false);
}

#[test]
fn builtin_fix_no_else_return() {
    check_builtin_fixture("no_else_return", false);
}

#[test]
fn builtin_fix_unused_argument_is_unsafe() {
    check_builtin_fixture("unused_argument", true);
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fix/builtin");
    let input = fs::read_to_string(folder.join("unused_argument.input.gd")).unwrap();
    let safe_only = api::fix_source(
        &folder.join("unused_argument.input.gd"),
        &input,
        &Config::default(),
        false,
    )
    .unwrap();
    assert_eq!(safe_only.output, input);
}

#[test]
fn no_else_return_body_with_declarations_is_an_unsafe_fix() {
    let source = "func f(value):\n\tif value:\n\t\treturn 1\n\telse:\n\t\tvar doubled = value * 2\n\t\treturn doubled\n";
    let result = api::fix_source(Path::new("a.gd"), source, &Config::default(), true).unwrap();
    assert_eq!(
        result.output,
        "func f(value):\n\tif value:\n\t\treturn 1\n\tvar doubled = value * 2\n\treturn doubled\n"
    );
}

// CLI

struct CommandOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

fn run(directory: &Path, args: &[&str], stdin: Option<&str>) -> CommandOutput {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut child_stdin = child.stdin.take().unwrap();
        if let Some(input) = stdin {
            child_stdin.write_all(input.as_bytes()).unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    CommandOutput {
        exit_code: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

const PASS_SOURCE: &str = "func f():\n\tprint(1)\n\tpass\n";

#[test]
fn cli_fix_writes_files_and_reports_the_rest() {
    let project = TempProject::new();
    project.write("a.gd", PASS_SOURCE);
    project.write("b.gd", "func badName():\n\tprint(1)\n\tpass\n");

    let output = run(
        &project.root,
        &["lint", "--fix", "--format", "compact", "a.gd", "b.gd"],
        None,
    );
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert_eq!(
        output.stdout,
        "b.gd:1:6: error [function-name] Function name 'badName' should be in snake_case, _private_snake_case format\nFixed 2 problems in 2 files\n"
    );
    assert_eq!(project.read("a.gd"), "func f():\n\tprint(1)\n");
    assert_eq!(project.read("b.gd"), "func badName():\n\tprint(1)\n");

    let again = run(&project.root, &["lint", "--fix", "a.gd"], None);
    assert_eq!(again.exit_code, 0);
    assert_eq!(again.stdout, "Fixed 0 problems in 0 files\n");
}

#[test]
fn cli_fix_with_json_keeps_stdout_parseable() {
    let project = TempProject::new();
    project.write("a.gd", PASS_SOURCE);
    let output = run(
        &project.root,
        &["lint", "--fix", "--format", "json", "a.gd"],
        None,
    );
    assert_eq!(output.exit_code, 0, "stderr: {}", output.stderr);
    let report: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(report["summary"]["files"], 1);
    assert_eq!(output.stderr, "Fixed 1 problem in 1 file\n");
}

#[test]
fn cli_fix_unsafe_applies_unsafe_fixes() {
    let project = TempProject::new();
    project.write("a.gd", "func f(unused):\n\tprint(1)\n");
    let safe = run(&project.root, &["lint", "--fix", "a.gd"], None);
    assert_eq!(safe.exit_code, 1);
    assert_eq!(project.read("a.gd"), "func f(unused):\n\tprint(1)\n");

    let output = run(&project.root, &["lint", "--fix-unsafe", "a.gd"], None);
    assert_eq!(output.exit_code, 0, "stdout: {}", output.stdout);
    assert_eq!(project.read("a.gd"), "func f(_unused):\n\tprint(1)\n");
}

#[test]
fn cli_diff_prints_a_unified_diff_and_writes_nothing() {
    let project = TempProject::new();
    project.write("a.gd", PASS_SOURCE);
    project.write("clean.gd", "var a = 1\n");

    let output = run(&project.root, &["lint", "--diff", "a.gd", "clean.gd"], None);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert_eq!(
        output.stdout,
        "--- a/a.gd\n+++ b/a.gd\n@@ -1,3 +1,2 @@\n func f():\n \tprint(1)\n-\tpass\n"
    );
    assert_eq!(project.read("a.gd"), PASS_SOURCE);

    let clean = run(&project.root, &["lint", "--diff", "clean.gd"], None);
    assert_eq!(clean.exit_code, 0);
    assert_eq!(clean.stdout, "");
}

#[test]
fn cli_stdin_fix_writes_the_fixed_source_to_stdout() {
    let project = TempProject::new();
    project.with_rules(&[("prefer-is-empty.yml", PREFER_IS_EMPTY)]);
    let source = "func badName(items):\n\tif len(items) == 0:\n\t\tprint(1)\n\tpass\n";

    let output = run(
        &project.root,
        &["lint", "--fix", "--stdin-filename", "scripts/player.gd"],
        Some(source),
    );
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        output.stdout,
        "func badName(items):\n\tif items.is_empty():\n\t\tprint(1)\n"
    );
    assert_eq!(
        output.stderr,
        "scripts/player.gd:1:6: error [function-name] Function name 'badName' should be in snake_case, _private_snake_case format\nFixed 2 problems in 1 file\n"
    );
    // Nothing is written to disk.
    assert!(!project.root.join("scripts/player.gd").exists());
}

#[test]
fn cli_stdin_filename_without_fix_lints_and_finds_the_config() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[rules]\nunnecessary-pass = \"error\"\n");
    let output = run(
        &project.root,
        &[
            "lint",
            "--format",
            "compact",
            "--stdin-filename",
            "scripts/a.gd",
        ],
        Some(PASS_SOURCE),
    );
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        output.stdout,
        "scripts/a.gd:3:2: error [unnecessary-pass] Unnecessary 'pass' statement when other statements are present\n"
    );
}

#[test]
fn cli_stdin_diff_prints_a_diff() {
    let project = TempProject::new();
    let output = run(
        &project.root,
        &["lint", "--diff", "--stdin-filename", "a.gd"],
        Some(PASS_SOURCE),
    );
    assert_eq!(output.exit_code, 1);
    assert_eq!(
        output.stdout,
        "--- a/a.gd\n+++ b/a.gd\n@@ -1,3 +1,2 @@\n func f():\n \tprint(1)\n-\tpass\n"
    );
}

#[test]
fn cli_stdin_filename_rejects_file_arguments() {
    let project = TempProject::new();
    project.write("a.gd", PASS_SOURCE);
    let output = run(
        &project.root,
        &["lint", "--stdin-filename", "b.gd", "a.gd"],
        Some(PASS_SOURCE),
    );
    assert_eq!(output.exit_code, 2);
    assert_eq!(
        output.stderr,
        "Error: --stdin-filename reads from stdin and can't be used with file paths\n"
    );
}
