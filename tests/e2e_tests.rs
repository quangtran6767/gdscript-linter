//! End-to-end tests: run the built binary on `tests/e2e/sample_project`, a
//! small Godot 4 project, and compare the output with snapshots in
//! `tests/e2e/expected/`.
//!
//! The merge session owns this file and `tests/e2e/`. Each wave extends it
//! with the features it adds (custom rules, `--fix`, JSON and SARIF output,
//! baseline, JS rules, ...).
//!
//! To update the snapshots after an intended output change, run:
//! `UPDATE_SNAPSHOTS=1 cargo test --test e2e_tests`, then review the diff.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn sample_project_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/e2e/sample_project")
}

struct CommandOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Runs the binary inside the sample project with `args`. Backslashes in
/// stdout become `/`, so text snapshots match on Windows.
fn run_in_sample_project(args: &[&str]) -> CommandOutput {
    let mut output = run_in_sample_project_raw(args);
    // Windows prints `.\scripts\a.gd`; snapshots use `/` so they work on
    // every platform.
    output.stdout = output.stdout.replace('\\', "/");
    output
}

/// Runs the binary inside the sample project with `args`, leaving stdout as
/// is. JSON output needs this: its escapes contain backslashes, and its paths
/// already use `/`.
fn run_in_sample_project_raw(args: &[&str]) -> CommandOutput {
    let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .current_dir(sample_project_path())
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("should run the binary");
    CommandOutput {
        exit_code: output
            .status
            .code()
            .expect("the binary should exit with a code"),
        stdout: String::from_utf8(output.stdout).expect("stdout should be valid UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr should be valid UTF-8"),
    }
}

/// Compares `actual` with the snapshot file, or rewrites the snapshot when
/// the `UPDATE_SNAPSHOTS` environment variable is set.
fn assert_snapshot(snapshot_name: &str, actual: &str) {
    let snapshot_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/e2e/expected")
        .join(snapshot_name);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        fs::write(&snapshot_path, actual).expect("should write snapshot");
        return;
    }
    let expected = fs::read_to_string(&snapshot_path).unwrap_or_else(|error| {
        panic!(
            "should read snapshot {}: {} (run with UPDATE_SNAPSHOTS=1 to create it)",
            snapshot_path.display(),
            error
        )
    });
    if expected != actual {
        let diff = similar::TextDiff::from_lines(expected.as_str(), actual);
        panic!(
            "snapshot {} doesn't match (run with UPDATE_SNAPSHOTS=1 to update it):\n{}",
            snapshot_name,
            diff.unified_diff().header("expected", "actual")
        );
    }
}

#[test]
fn lint_sample_project_default_output() {
    let output = run_in_sample_project(&["lint", "."]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert_eq!(output.stderr, "");
    assert_snapshot("lint.txt", &output.stdout);
}

#[test]
fn sample_project_is_formatted() {
    // The W3 fix engine only reformats files that were already formatted, so
    // the sample scripts must stay formatted for its e2e tests to be stable.
    let output = run_in_sample_project(&["--check", "."]);
    assert_eq!(output.exit_code, 0, "stderr: {}", output.stderr);
}

// W1: exit codes (0 clean, 1 problems, 2 usage errors) and `--max-warnings`.
#[test]
fn lint_exit_codes_on_sample_project() {
    let clean = run_in_sample_project(&["lint", "scripts/clean.gd"]);
    assert_eq!(clean.exit_code, 0, "stdout: {}", clean.stdout);

    // player.gd has one error (the custom `no-get-node-in-process`, raised to
    // `error` in gdguard.toml), two warnings and two infos. Without the error,
    // only the warnings count against `--max-warnings`; infos never fail.
    let player = run_in_sample_project(&["lint", "scripts/player.gd"]);
    assert_eq!(player.exit_code, 1);
    let player_allowed = run_in_sample_project(&[
        "lint",
        "--disable",
        "no-get-node-in-process",
        "--max-warnings",
        "2",
        "scripts/player.gd",
    ]);
    assert_eq!(
        player_allowed.exit_code, 0,
        "stderr: {}",
        player_allowed.stderr
    );
    let player_strict = run_in_sample_project(&[
        "lint",
        "--disable",
        "no-get-node-in-process",
        "--max-warnings",
        "1",
        "scripts/player.gd",
    ]);
    assert_eq!(player_strict.exit_code, 1);

    // enemy.gd has errors, which always fail.
    let enemy =
        run_in_sample_project(&["lint", "--max-warnings", "100", "scripts/enemies/enemy.gd"]);
    assert_eq!(enemy.exit_code, 1);

    let bad_rule = run_in_sample_project(&["lint", "--disable", "not-a-rule", "."]);
    assert_eq!(bad_rule.exit_code, 2);
}

// W1: pretty output now lists files in a stable order, so it can be
// snapshotted.
#[test]
fn lint_sample_project_pretty_output() {
    let output = run_in_sample_project(&["lint", "--pretty", "."]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert_snapshot("lint_pretty.txt", &output.stdout);
}

// W2: custom YAML rules from `.gdguard/rules/` run with the built-in rules,
// with levels from `gdguard.toml`. JSON output is snapshotted.
#[test]
fn lint_sample_project_json_output() {
    let output = run_in_sample_project_raw(&["lint", "--format", "json", "."]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert_eq!(output.stderr, "");
    let report: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("json output should parse");
    assert_eq!(report["version"], 1);
    // The tool version changes with each release; the snapshot doesn't.
    let stdout = output.stdout.replace(
        &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
        "\"version\": \"<version>\"",
    );
    assert_snapshot("lint.json", &stdout);
}

#[test]
fn custom_rules_report_with_config_levels() {
    let output = run_in_sample_project(&["lint", "--format", "compact", "scripts/player.gd"]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert!(output.stdout.contains(
        "scripts/player.gd:14:19: error [no-get-node-in-process] Don't look up nodes in `_process()`"
    ));
    assert!(output.stdout.contains(
        "scripts/player.gd:16:5: warning [prefer-is-empty] Use `inventory.is_empty()` instead of comparing `len(inventory)` to zero"
    ));
    assert!(output.stdout.contains("info [unused-argument]"));
}

#[test]
fn test_rules_passes_on_sample_project_rules() {
    let output = run_in_sample_project(&["test-rules"]);
    assert_eq!(
        output.exit_code, 0,
        "stdout: {}\nstderr: {}",
        output.stdout, output.stderr
    );
    assert!(output.stdout.contains("prefer-is-empty"));
    assert!(output.stdout.contains("no-get-node-in-process"));
}

#[test]
fn sample_project_rules_match_the_example_rules() {
    // The sample project uses copies of `examples/rules/`; keep them in sync.
    let rules_folder = sample_project_path().join(".gdguard/rules");
    let examples_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/rules");
    let mut checked = 0;
    for entry in fs::read_dir(&rules_folder).expect("should list sample rules") {
        let path = entry.expect("should read entry").path();
        let file_name = path.file_name().expect("rule file has a name");
        let sample = fs::read_to_string(&path).expect("should read sample rule");
        let example = fs::read_to_string(examples_folder.join(file_name))
            .expect("every sample rule should come from examples/rules");
        assert_eq!(sample, example, "{} differs", path.display());
        checked += 1;
    }
    assert_eq!(checked, 3);
}

// W3: fix engine (C2), the rule pack, explain and list-rules.

/// Copies `source` into `destination` recursively.
fn copy_folder(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("should create folder");
    for entry in fs::read_dir(source).expect("should list folder") {
        let entry = entry.expect("should read entry");
        let target = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("should copy file");
        }
    }
}

/// A copy of the sample project in the temp folder, removed on drop.
struct SampleProjectCopy {
    root: PathBuf,
}

impl SampleProjectCopy {
    fn new(name: &str) -> SampleProjectCopy {
        let root =
            std::env::temp_dir().join(format!("gdguard-e2e-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        copy_folder(&sample_project_path(), &root);
        SampleProjectCopy { root }
    }

    fn run(&self, args: &[&str]) -> CommandOutput {
        let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
            .current_dir(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .expect("should run the binary");
        CommandOutput {
            exit_code: output
                .status
                .code()
                .expect("the binary should exit with a code"),
            stdout: String::from_utf8(output.stdout)
                .expect("stdout should be valid UTF-8")
                .replace('\\', "/"),
            stderr: String::from_utf8(output.stderr).expect("stderr should be valid UTF-8"),
        }
    }

    fn read(&self, relative_path: &str) -> String {
        fs::read_to_string(self.root.join(relative_path)).expect("should read file")
    }
}

impl Drop for SampleProjectCopy {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn diff_on_sample_project_shows_safe_fixes_and_writes_nothing() {
    let output = run_in_sample_project(&["lint", "--diff", "."]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert_snapshot("fix.diff", &output.stdout);
    let player = fs::read_to_string(sample_project_path().join("scripts/player.gd"))
        .expect("should read player.gd");
    assert!(player.contains("if len(inventory) == 0:"));
}

#[test]
fn fix_on_sample_project_applies_builtin_and_custom_fixes() {
    let project = SampleProjectCopy::new("fix");
    let output = project.run(&["lint", "--fix", "--format", "compact", "."]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    assert!(
        output.stdout.ends_with("Fixed 4 problems in 2 files\n"),
        "stdout: {}",
        output.stdout
    );

    // Custom rule fixes (`prefer-is-empty`, `prefer-not-is-empty`).
    let player = project.read("scripts/player.gd");
    assert!(player.contains("\tif inventory.is_empty():\n"));
    assert!(player.contains("\treturn not inventory.is_empty()\n"));
    // Built-in fixes (`no-else-return`, `unnecessary-pass`).
    let enemy = project.read("scripts/enemies/enemy.gd");
    assert!(!enemy.contains("else:"));
    assert!(enemy.ends_with("func idle() -> void:\n\tspeed = 0.0\n"));
    // The fixed issues are gone from the report; others stay.
    assert!(!output.stdout.contains("[prefer-is-empty]"));
    assert!(!output.stdout.contains("[unnecessary-pass]"));
    assert!(output.stdout.contains("[no-get-node-in-process]"));

    // The project stays formatted, and fixing again changes nothing.
    let check = project.run(&["--check", "."]);
    assert_eq!(check.exit_code, 0, "stderr: {}", check.stderr);
    let again = project.run(&["lint", "--fix", "--format", "compact", "."]);
    assert!(again.stdout.ends_with("Fixed 0 problems in 0 files\n"));
}

#[test]
fn fix_unsafe_on_sample_project_renames_unused_arguments() {
    let project = SampleProjectCopy::new("fix-unsafe");
    let output = project.run(&["lint", "--fix-unsafe", "scripts/player.gd"]);
    assert_eq!(output.exit_code, 1, "stderr: {}", output.stderr);
    let player = project.read("scripts/player.gd");
    assert!(player.contains("func _process(_delta: float) -> void:"));
    assert!(player.contains("func take_damage(amount: int, _source: Node) -> void:"));
}

#[test]
fn stdin_fix_on_sample_project_uses_its_config() {
    let source = fs::read_to_string(sample_project_path().join("scripts/player.gd"))
        .expect("should read player.gd");
    let mut child = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .current_dir(sample_project_path())
        .args(["lint", "--fix", "--stdin-filename", "scripts/player.gd"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("should run the binary");
    {
        use std::io::Write;
        let mut stdin = child.stdin.take().expect("should open stdin");
        stdin
            .write_all(source.as_bytes())
            .expect("should write stdin");
    }
    let output = child.wait_with_output().expect("should finish");
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout.contains("\tif inventory.is_empty():\n"));
    assert!(stderr.contains("scripts/player.gd:14:19: error [no-get-node-in-process]"));
    assert!(stderr.ends_with("Fixed 2 problems in 1 file\n"));
}

#[test]
fn test_rules_passes_on_the_example_rule_pack() {
    let examples_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/rules");
    let output = run_in_sample_project(&[
        "test-rules",
        examples_folder.to_str().expect("path is UTF-8"),
    ]);
    assert_eq!(
        output.exit_code, 0,
        "stdout: {}\nstderr: {}",
        output.stdout, output.stderr
    );
    assert!(output.stdout.contains(", 0 failed"));
}

#[test]
fn explain_and_list_rules_include_sample_custom_rules() {
    let builtin = run_in_sample_project(&["explain", "no-else-return"]);
    assert_eq!(builtin.exit_code, 0, "stderr: {}", builtin.stderr);
    assert!(builtin.stdout.starts_with("no-else-return\n"));
    assert!(builtin.stdout.contains("Fixable:  yes"));

    let custom = run_in_sample_project(&["explain", "prefer-is-empty"]);
    assert_eq!(custom.exit_code, 0, "stderr: {}", custom.stderr);
    assert!(custom.stdout.contains("custom ("));
    assert!(custom.stdout.contains(".gdguard/rules/prefer-is-empty.yml"));

    let unknown = run_in_sample_project(&["explain", "prefer-is-emty"]);
    assert_eq!(unknown.exit_code, 2);

    let list = run_in_sample_project_raw(&["lint", "--list-rules", "--format", "json"]);
    assert_eq!(list.exit_code, 0, "stderr: {}", list.stderr);
    let rules: serde_json::Value =
        serde_json::from_str(&list.stdout).expect("list-rules json should parse");
    let mut custom_ids = Vec::new();
    for rule in rules["rules"].as_array().expect("rules is an array") {
        if rule["origin"] == "custom" {
            custom_ids.push(rule["id"].as_str().expect("id is a string").to_string());
        }
    }
    custom_ids.sort();
    assert_eq!(
        custom_ids,
        vec![
            "no-get-node-in-process",
            "prefer-is-empty",
            "prefer-not-is-empty"
        ]
    );
}

// W4: CI reporters and baselines run against the whole sample project.
#[test]
fn lint_sample_project_github_output() {
    let output = run_in_sample_project_raw(&["lint", "--format", "github", "."]);
    assert_eq!(output.exit_code, 1, "{}", output.stderr);
    assert_eq!(output.stderr, "");
    assert_snapshot("lint_github.txt", &output.stdout);
}

#[test]
fn lint_sample_project_sarif_output() {
    let output = run_in_sample_project_raw(&["lint", "--format", "sarif", "."]);
    assert_eq!(output.exit_code, 1, "{}", output.stderr);
    let report: serde_json::Value =
        serde_json::from_str(&output.stdout).expect("valid e2e fixture and report");
    assert_eq!(report["version"], "2.1.0");
    assert_eq!(report["runs"][0]["columnKind"], "utf16CodeUnits");
    assert_eq!(
        report["runs"][0]["results"]
            .as_array()
            .expect("valid e2e fixture and report")
            .len(),
        19
    );
    assert!(
        report["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .expect("valid e2e fixture and report")
            .len()
            > 1
    );
    let stdout = output.stdout.replace(
        &format!("\"version\": \"{}\"", env!("CARGO_PKG_VERSION")),
        "\"version\": \"<version>\"",
    );
    assert_snapshot("lint_sarif.json", &stdout);
}

#[test]
fn baseline_round_trip_hides_old_issues_and_keeps_new_ones() {
    let project = SampleProjectCopy::new("baseline");
    let updated = project.run(&["lint", "--update-baseline", "."]);
    assert_eq!(updated.exit_code, 0, "{}", updated.stderr);
    let baseline = project.read(".gdguard/baseline.json");
    let repeated = project.run(&["lint", "--update-baseline", "."]);
    assert_eq!(repeated.exit_code, 0);
    assert_eq!(project.read(".gdguard/baseline.json"), baseline);

    let hidden = project.run(&[
        "lint",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "pretty",
        ".",
    ]);
    assert_eq!(hidden.exit_code, 0, "{}", hidden.stderr);
    assert!(
        hidden.stdout.contains("19 issues hidden by baseline."),
        "{}",
        hidden.stdout
    );
    fs::write(project.root.join("scripts/new.gd"), "var badName = 1\n")
        .expect("valid e2e fixture and report");
    let new_issue = project.run(&[
        "lint",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "json",
        ".",
    ]);
    assert_eq!(new_issue.exit_code, 1, "{}", new_issue.stderr);
    let report: serde_json::Value =
        serde_json::from_str(&new_issue.stdout).expect("valid e2e fixture and report");
    assert_eq!(report["summary"]["baseline_hidden"], 19);
    assert_eq!(
        report["issues"]
            .as_array()
            .expect("valid e2e fixture and report")
            .len(),
        1
    );
    assert_eq!(report["issues"][0]["file"], "scripts/new.gd");
}
