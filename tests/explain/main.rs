//! W3/B6: `gdguard explain` and `lint --list-rules` (Track B).

#![allow(clippy::unwrap_used)]

mod init_tests;

use std::path::{Path, PathBuf};
use std::process::Command;

use gdguard::linter::api::Config;
use gdguard::linter::explain::{
    all_rule_metas, explain, format_explain, format_rule_list_json, format_rule_list_text,
};
use gdguard::linter::rule_config::get_all_rule_names;
use gdguard::linter::rules_meta::builtin_rule_meta;

fn project_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("explain")
        .join("project")
}

fn project_config() -> Config {
    Config {
        custom_rule_paths: vec![project_dir().join("rules")],
        ..Config::default()
    }
}

/// Runs the binary in `directory` and returns stdout, stderr and exit code.
fn run_in(directory: &Path, args: &[&str]) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    (
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap(),
        output.status.code().unwrap(),
    )
}

/// A fresh empty folder, so the binary finds no gdguard.toml.
fn empty_dir(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("gdguard-explain-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

// explain

#[test]
fn explain_built_in_rule() {
    let meta = builtin_rule_meta("unnecessary-pass").unwrap();
    assert_eq!(
        format_explain(&meta),
        "\
unnecessary-pass
  Origin:   built-in
  Severity: warning
  Fixable:  yes

No `pass` in a block that has other statements.

Why:
  `pass` only exists to fill an empty block. Next to other statements it does nothing.

Bad:
    func f():
    \tprint(1)
    \tpass

Good:
    func f():
    \tprint(1)
"
    );
}

#[test]
fn explain_custom_rule() {
    let metas = all_rule_metas(&project_config()).unwrap();
    let text = explain("no-print-left", &metas).unwrap();
    let path = project_dir()
        .join("rules")
        .join("no-print-left.yml")
        .display()
        .to_string()
        .replace('\\', "/");
    assert_eq!(
        text,
        format!(
            "\
no-print-left
  Origin:   custom ({})
  Severity: info
  Fixable:  no

Remove the debug print

Why:
  Debug prints left in shipped code clutter the output.

Bad:
    func f():
    \tprint(1)

Good:
    func f():
    \tpush_warning(\"x\")
",
            path
        )
    );
}

#[test]
fn explain_unknown_rule_suggests_the_closest_id() {
    let metas = all_rule_metas(&project_config()).unwrap();
    assert_eq!(
        explain("unused-arguments", &metas).unwrap_err(),
        "unknown rule `unused-arguments`; did you mean `unused-argument`?\nRun `gdguard lint --list-rules` to see every rule."
    );
    assert_eq!(
        explain("no-print-lefts", &metas).unwrap_err(),
        "unknown rule `no-print-lefts`; did you mean `no-print-left`?\nRun `gdguard lint --list-rules` to see every rule."
    );
    assert_eq!(
        explain("something-else-entirely", &metas).unwrap_err(),
        "unknown rule `something-else-entirely`\nRun `gdguard lint --list-rules` to see every rule."
    );
}

#[test]
fn all_rule_metas_lists_built_ins_then_custom_rules() {
    let metas = all_rule_metas(&project_config()).unwrap();
    // Built-ins, load-variable-name, fix-failed, unused-ignore, parse-error,
    // script-error, godot-error, and one custom rule.
    assert_eq!(metas.len(), get_all_rule_names().len() + 6);
    assert_eq!(metas[0].id, "declaration-order");
    assert_eq!(metas[metas.len() - 1].id, "no-print-left");
}

#[test]
fn all_rule_metas_reports_rule_file_errors() {
    let broken = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("custom_rules")
        .join("rule_tester")
        .join("load_error");
    let config = Config {
        custom_rule_paths: vec![broken.clone()],
        ..Config::default()
    };
    assert_eq!(
        all_rule_metas(&config).unwrap_err(),
        format!(
            "{}: `fixSafety`: `fixSafety` needs a `fix`",
            broken.join("bad.yml").display()
        )
    );
}

// --list-rules

#[test]
fn list_rules_text() {
    let metas = all_rule_metas(&project_config()).unwrap();
    let text = format_rule_list_text(&metas);
    assert!(text.starts_with("Available linting rules:\n  declaration-order                warning  fix  Top-level declarations follow the formatter's style-guide order.\n"), "{}", text);
    assert!(text.contains("\n  no-else-return                   warning  fix  No `else`/`elif` after an `if` block that ends with `return`.\n"), "{}", text);
    assert!(
        text.contains(
            "\n  no-print-left                    info          Remove the debug print (custom)\n"
        ),
        "{}",
        text
    );
    assert!(text.ends_with(&format!(
        "\n{} rules ({} built-in, 1 custom). Run `gdguard explain <rule>` for details.\n",
        metas.len(),
        metas.len() - 1
    )));
}

#[test]
fn list_rules_json() {
    let metas = all_rule_metas(&project_config()).unwrap();
    let document: serde_json::Value = serde_json::from_str(&format_rule_list_json(&metas)).unwrap();
    assert_eq!(document["version"], 1);
    let rules = document["rules"].as_array().unwrap();
    assert_eq!(rules.len(), metas.len());
    assert_eq!(rules[0]["id"], "declaration-order");
    assert_eq!(
        rules
            .iter()
            .find(|rule| rule["id"] == "duplicate-class-name")
            .expect("duplicate-class-name metadata")
            .clone(),
        serde_json::json!({
            "id": "duplicate-class-name",
            "description": "Each class_name is declared by one script in the project.",
            "rationale": "Godot registers one global class per name. The other script fails to load with `Class \"X\" hides a global script class`, and the editor doesn't always say which file wins.",
            "default_severity": "error", "fixable": false,
            "origin": "built-in", "path": null,
            "bad_example": "class_name ProjectData\nextends Node\n",
            "good_example": "class_name ExampleData\nextends Node\n",
        })
    );
    let custom = &rules[rules.len() - 1];
    assert_eq!(custom["id"], "no-print-left");
    assert_eq!(custom["origin"], "custom");
    assert_eq!(custom["default_severity"], "info");
    assert!(
        custom["path"]
            .as_str()
            .unwrap()
            .ends_with("rules/no-print-left.yml")
    );
}

// CLI

#[test]
fn cli_explain_built_in_rule() {
    let directory = empty_dir("builtin");
    let (stdout, stderr, code) = run_in(&directory, &["explain", "unnecessary-pass"]);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(code, 0, "{}", stderr);
    assert!(
        stdout.starts_with("unnecessary-pass\n  Origin:   built-in\n"),
        "{}",
        stdout
    );
}

#[test]
fn cli_explain_custom_rule_from_the_project_config() {
    let (stdout, stderr, code) = run_in(&project_dir(), &["explain", "no-print-left"]);
    assert_eq!(code, 0, "{}", stderr);
    assert!(
        stdout.starts_with("no-print-left\n  Origin:   custom ("),
        "{}",
        stdout
    );
}

#[test]
fn cli_explain_unknown_rule_exits_2() {
    let directory = empty_dir("unknown");
    let (stdout, stderr, code) = run_in(&directory, &["explain", "unused-arguments"]);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(code, 2);
    assert_eq!(stdout, "");
    assert_eq!(
        stderr,
        "Error: unknown rule `unused-arguments`; did you mean `unused-argument`?\nRun `gdguard lint --list-rules` to see every rule.\n"
    );
}

#[test]
fn cli_list_rules_text_and_json_include_custom_rules() {
    let (stdout, _, code) = run_in(&project_dir(), &["lint", "--list-rules"]);
    assert_eq!(code, 0);
    assert!(stdout.contains("  no-print-left "), "{}", stdout);

    let (json, _, code) = run_in(
        &project_dir(),
        &["lint", "--list-rules", "--format", "json"],
    );
    assert_eq!(code, 0);
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    let rules = document["rules"].as_array().unwrap();
    assert_eq!(rules[rules.len() - 1]["id"], "no-print-left");
}

/// docs/rules.md documents every built-in rule with its default level, and
/// nothing else, so the README's rule reference can't drift from the code.
#[test]
fn rules_reference_lists_every_built_in_rule_with_its_level() {
    let document = include_str!("../../docs/rules.md");
    let metas = gdguard::linter::rules_meta::builtin_rule_metas();
    for meta in &metas {
        assert!(
            document.contains(&format!("\n### `{}`\n", meta.id)),
            "docs/rules.md has no section for `{}`",
            meta.id
        );
        let row = format!(
            "| [`{}`](#{}) | {} |",
            meta.id,
            meta.id,
            meta.default_severity.as_str()
        );
        assert!(
            document.contains(&row),
            "docs/rules.md table row for `{}` is missing or has the wrong level",
            meta.id
        );
    }
    let mut headings = 0;
    for line in document.lines() {
        if line.starts_with("### `") {
            headings += 1;
        }
    }
    assert_eq!(headings, metas.len(), "docs/rules.md lists unknown rules");
    assert!(document.contains(&format!("gdguard has {} built-in rules", metas.len())));
}
