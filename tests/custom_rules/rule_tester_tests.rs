//! W2/B4: `gdguard test-rules`. Library tests check the reports; CLI tests
//! check the printed output and exit codes.

use std::path::{Path, PathBuf};
use std::process::Command;

use gdguard::linter::custom::loader::load_rules;
use gdguard::linter::rule_tester::{RuleReport, exit_code, format_reports, line_diff, test_rules};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("custom_rules")
        .join("rule_tester")
        .join(name)
}

fn reports_for(name: &str) -> Vec<RuleReport> {
    test_rules(load_rules(&[fixture(name)]).unwrap())
}

fn run_cli(folder: &Path) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .arg("test-rules")
        .arg(folder)
        .output()
        .unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap(),
    )
}

#[test]
fn passing_rule() {
    let reports = reports_for("pass");
    assert_eq!(
        reports,
        vec![RuleReport {
            id: "prefer-is-empty".to_string(),
            passed: 6,
            failures: Vec::new(),
        }]
    );
    assert_eq!(exit_code(&reports), 0);
    assert_eq!(
        format_reports(&reports),
        "✓ prefer-is-empty  6 passed\n\n1 rules, 1 passed, 0 failed\n"
    );
}

#[test]
fn wrong_counts_and_valid_case_that_matches() {
    let reports = reports_for("wrong_count");
    assert_eq!(reports.len(), 1);
    let report = &reports[0];
    assert_eq!(report.passed, 1);
    assert_eq!(
        report.failures,
        vec![
            "valid[0]: expected no diagnostics, got 1:\n    2:2: Remove print(2)\n    code:\n    | func f():\n    | \tprint(2)\n".to_string(),
            "invalid[0]: expected exactly 2 diagnostics, got 1\n    2:2: Remove print(1)\n    code:\n    | func f():\n    | \tprint(1)\n".to_string(),
            "invalid[1]: expected at least 1 diagnostics, got 0\n    code:\n    | func f():\n    | \tpass\n".to_string(),
        ]
    );
    assert_eq!(exit_code(&reports), 1);
}

#[test]
fn wrong_output_shows_a_diff() {
    let reports = reports_for("wrong_output");
    let report = &reports[0];
    assert_eq!(report.passed, 1);
    assert_eq!(
        report.failures,
        vec![
            "invalid[0]: the fixed code differs from `output` (- expected, + actual):\n    | func f(items):\n    - \tif items.empty():\n    + \tif items.is_empty():\n    | \t\tpass\n".to_string()
        ]
    );
}

#[test]
fn non_idempotent_fix() {
    let reports = reports_for("not_idempotent");
    assert_eq!(
        reports[0].failures,
        vec![
            "invalid[0]: the fix is not idempotent: the rule still reports 1 diagnostics on the fixed code:\n    2:2: Wrap print\n    code:\n    | func f():\n    | \tprint(str(1))\n".to_string()
        ]
    );
    assert_eq!(exit_code(&reports), 1);
}

#[test]
fn output_without_fix_fails() {
    let folder =
        std::env::temp_dir().join(format!("gdguard-rule-tester-no-fix-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("r.yml"),
        "id: no-fix\nmessage: m\nrule:\n  pattern: print($A)\ntests:\n  invalid:\n    - code: print(1)\n      output: print(2)\n",
    )
    .unwrap();
    let reports = test_rules(load_rules(std::slice::from_ref(&folder)).unwrap());
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(
        reports[0].failures,
        vec!["invalid[0]: has `output` but the rule has no `fix`".to_string()]
    );
}

#[test]
fn fix_that_breaks_the_code_fails() {
    let folder =
        std::env::temp_dir().join(format!("gdguard-rule-tester-broken-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("r.yml"),
        "id: broken-fix\nmessage: m\nrule:\n  pattern: len($X) == 0\nfix: \"$X.is_empty(\"\ntests:\n  invalid:\n    - code: \"var a = len(b) == 0\\n\"\n      output: \"var a = b.is_empty(\\n\"\n",
    )
    .unwrap();
    let reports = test_rules(load_rules(std::slice::from_ref(&folder)).unwrap());
    std::fs::remove_dir_all(&folder).unwrap();
    assert_eq!(reports[0].failures.len(), 1);
    assert!(
        reports[0].failures[0]
            .starts_with("invalid[0]: the fixed code doesn't parse as GDScript:\n"),
        "{}",
        reports[0].failures[0]
    );
}

#[test]
fn format_reports_marks_failed_rules() {
    let reports = vec![
        RuleReport {
            id: "a".to_string(),
            passed: 2,
            failures: Vec::new(),
        },
        RuleReport {
            id: "b".to_string(),
            passed: 1,
            failures: vec!["valid[0]: oops".to_string()],
        },
    ];
    assert_eq!(
        format_reports(&reports),
        "✓ a  2 passed\n✗ b  1 failed, 1 passed\n  valid[0]: oops\n\n2 rules, 1 passed, 1 failed\n"
    );
}

#[test]
fn line_diff_marks_changed_lines() {
    assert_eq!(
        line_diff("a\nb\nc\n", "a\nx\nc\n"),
        "    | a\n    - b\n    + x\n    | c\n"
    );
    assert_eq!(line_diff("a\n", "a\nb\n"), "    | a\n    - \n    + b\n");
}

// CLI

#[test]
#[cfg(feature = "js-rules")]
fn cli_passes_on_example_rules() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("rules");
    let (stdout, stderr, code) = run_cli(&examples);
    assert_eq!(code, 0, "{}{}", stdout, stderr);
    assert_eq!(
        stdout,
        "\
✓ no-get-node-in-process  7 passed
✓ no-godot3-yield  6 passed
✓ no-print-left  6 passed
✓ no-string-connect  6 passed
✓ prefer-is-empty  7 passed
✓ prefer-is-instance-valid  7 passed
✓ prefer-not-is-empty  7 passed
✓ prefer-preload-const  7 passed
✓ max-function-params  3 passed
✓ no-autoload-mutation-outside-owner  2 passed
✓ no-empty-function  3 passed
✓ prefer-static-typing  3 passed
✓ require-typed-signal-params  3 passed

13 rules, 13 passed, 0 failed
"
    );
}

#[test]
fn cli_exits_1_on_failure() {
    let (stdout, _, code) = run_cli(&fixture("wrong_output"));
    assert_eq!(code, 1);
    assert!(
        stdout.starts_with("✗ prefer-is-empty  1 failed, 1 passed\n"),
        "{}",
        stdout
    );
}

#[test]
fn cli_exits_2_on_load_error() {
    let folder = fixture("load_error");
    let (stdout, stderr, code) = run_cli(&folder);
    assert_eq!(code, 2);
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        format!(
            "Error: {}: `fixSafety`: `fixSafety` needs a `fix`\n",
            folder.join("bad.yml").display()
        )
    );
}

#[test]
fn cli_with_no_rules_exits_0() {
    let folder = fixture("empty");
    let (stdout, _, code) = run_cli(&folder);
    assert_eq!(code, 0);
    assert_eq!(stdout, format!("No rules found in {}\n", folder.display()));
}

#[test]
fn cli_missing_folder_exits_2() {
    let (_, stderr, code) = run_cli(&fixture("does-not-exist"));
    assert_eq!(code, 2);
    assert!(stderr.contains("can't read the rule folder"), "{}", stderr);
}

// Example rule pack (W3/B5)

#[test]
fn every_example_rule_is_documented_and_well_tested() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("rules");
    let rules = load_rules(&[examples]).unwrap();
    assert_eq!(rules.len(), 8);
    for rule in &rules {
        let id = &rule.meta.id;
        assert!(!rule.meta.rationale.is_empty(), "`{}` has no rationale", id);
        assert!(rule.compiled.help.is_some(), "`{}` has no help", id);
        assert!(rule.tests.valid.len() >= 3, "`{}` needs 3 valid tests", id);
        assert!(
            rule.tests.invalid.len() >= 3,
            "`{}` needs 3 invalid tests",
            id
        );
        if rule.compiled.fix.is_some() {
            for case in &rule.tests.invalid {
                assert!(
                    case.output.is_some(),
                    "`{}` has a fix, so every invalid test should check its output",
                    id
                );
            }
        }
    }
}
