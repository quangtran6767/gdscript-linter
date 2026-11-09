//! Tests for `lint --baseline` and `--update-baseline`
//! (`src/linter/baseline.rs`).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use gdguard::linter::baseline::{BaselineTarget, resolve_baseline};

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
            "gdguard-baseline-test-{}-{}",
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

    fn run(&self, args: &[&str]) -> CommandOutput {
        run_in(&self.root, args)
    }
}

fn run_in(directory: &Path, args: &[&str]) -> CommandOutput {
    let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .current_dir(directory)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    CommandOutput {
        exit_code: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct CommandOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Two private-access issues and one constant-name issue.
const PLAYER: &str = "class_name Player\nconst bad_name = 1\n\n\nfunc f(enemy):\n\tenemy._hidden()\n\tenemy._other()\n";

#[test]
fn update_writes_a_sorted_stable_file_and_hides_every_issue() {
    let project = TempProject::new();
    project.write("project.godot", "");
    project.write("scripts/player.gd", PLAYER);

    let update = project.run(&["lint", "--update-baseline", "scripts"]);
    assert_eq!(update.exit_code, 0, "stderr: {}", update.stderr);
    assert_eq!(update.stdout, "Wrote 3 issues to .gdguard/baseline.json\n");
    let first = project.read(".gdguard/baseline.json");
    assert!(first.starts_with("{\n  \"version\": 1,\n  \"issues\": [\n"));
    assert!(first.contains("\"file\": \"scripts/player.gd\""));
    let constant_index = first.find("\"rule\": \"constant-name\"").unwrap();
    let private_index = first.find("\"rule\": \"private-access\"").unwrap();
    assert!(constant_index < private_index, "entries are sorted by rule");

    // Updating again gives the same bytes.
    project.run(&["lint", "--update-baseline", "scripts"]);
    assert_eq!(project.read(".gdguard/baseline.json"), first);

    // With the baseline, nothing is left.
    let lint = project.run(&[
        "lint",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "pretty",
        "scripts",
    ]);
    assert_eq!(lint.exit_code, 0, "stdout: {}", lint.stdout);
    assert_eq!(
        lint.stdout,
        "No problems found. 3 issues hidden by baseline.\n"
    );
}

#[test]
fn baseline_survives_lines_inserted_above_the_issue() {
    let project = TempProject::new();
    project.write("scripts/player.gd", PLAYER);
    project.run(&["lint", "--update-baseline", "scripts"]);

    project.write(
        "scripts/player.gd",
        &format!("# A new comment\n# Another inserted line\n\n{}", PLAYER),
    );
    let lint = project.run(&[
        "lint",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "compact",
        "scripts",
    ]);
    assert_eq!(lint.exit_code, 0, "stdout: {}", lint.stdout);
    assert_eq!(lint.stdout, "");
}

#[test]
fn new_duplicate_of_a_baselined_line_is_reported() {
    let project = TempProject::new();
    project.write("scripts/player.gd", PLAYER);
    project.run(&["lint", "--update-baseline", "scripts"]);

    // A second `enemy._hidden()` line: same rule and text, new occurrence.
    project.write(
        "scripts/player.gd",
        &format!("{}\tenemy._hidden()\n", PLAYER),
    );
    let lint = project.run(&[
        "lint",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "json",
        "scripts",
    ]);
    assert_eq!(lint.exit_code, 1);
    let report: serde_json::Value = serde_json::from_str(&lint.stdout).unwrap();
    assert_eq!(report["summary"]["baseline_hidden"], 3);
    let issues = report["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["rule"], "private-access");
    assert_eq!(issues[0]["range"]["start"]["line"], 8);
}

#[test]
fn edited_issue_line_is_reported_again() {
    let project = TempProject::new();
    project.write("scripts/player.gd", PLAYER);
    project.run(&["lint", "--update-baseline", "scripts"]);

    project.write(
        "scripts/player.gd",
        &PLAYER.replace("enemy._other()", "enemy._renamed()"),
    );
    let lint = project.run(&[
        "lint",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "compact",
        "scripts",
    ]);
    assert_eq!(lint.exit_code, 1);
    assert!(
        lint.stdout.contains("scripts/player.gd:7:"),
        "{}",
        lint.stdout
    );
    assert_eq!(lint.stdout.lines().count(), 1);
}

#[test]
fn config_baseline_is_used_automatically_and_paths_are_relative_to_the_config() {
    let project = TempProject::new();
    project.write(
        "gdguard.toml",
        "[lint]\nbaseline = \"lint/baseline.json\"\n",
    );
    project.write("scripts/player.gd", PLAYER);

    // Without the file yet, nothing is hidden and nothing fails.
    let before = project.run(&["lint", "--format", "compact", "scripts"]);
    assert_eq!(before.exit_code, 1, "stderr: {}", before.stderr);
    assert_eq!(before.stdout.lines().count(), 3);

    // Update from a sub-folder: the file goes where the config says and
    // paths stay relative to the config folder.
    let scripts = project.root.join("scripts");
    let update = run_in(&scripts, &["lint", "--update-baseline", "player.gd"]);
    assert_eq!(update.exit_code, 0, "stderr: {}", update.stderr);
    let baseline = project.read("lint/baseline.json");
    assert!(baseline.contains("\"file\": \"scripts/player.gd\""));

    let after = project.run(&["lint", "--format", "compact", "scripts"]);
    assert_eq!(after.exit_code, 0, "stdout: {}", after.stdout);
}

#[test]
fn baseline_errors_exit_with_code_2() {
    let project = TempProject::new();
    project.write("scripts/player.gd", PLAYER);

    let missing = project.run(&["lint", "--baseline", "missing.json", "scripts"]);
    assert_eq!(missing.exit_code, 2);
    assert!(
        missing
            .stderr
            .starts_with("Error: Failed to read baseline ")
    );

    project.write("bad.json", "{\"version\": 9, \"issues\": []}");
    let bad_version = project.run(&["lint", "--baseline", "bad.json", "scripts"]);
    assert_eq!(bad_version.exit_code, 2);
    assert!(bad_version.stderr.contains("unsupported version 9"));

    let combined = project.run(&["lint", "--update-baseline", "--fix", "scripts"]);
    assert_eq!(combined.exit_code, 2);
    assert_eq!(
        combined.stderr,
        "Error: --update-baseline can't be combined with --fix, --fix-unsafe or --diff\n"
    );
}

#[test]
fn fix_reports_only_issues_outside_the_baseline() {
    let project = TempProject::new();
    project.write("scripts/player.gd", PLAYER);
    project.run(&["lint", "--update-baseline", "scripts"]);
    project.write(
        "scripts/player.gd",
        &format!("{}\tprint(1)\n\tpass\n", PLAYER),
    );
    let fix = project.run(&[
        "lint",
        "--fix",
        "--baseline",
        ".gdguard/baseline.json",
        "--format",
        "compact",
        "scripts",
    ]);
    assert_eq!(fix.exit_code, 0, "stdout: {}", fix.stdout);
    assert_eq!(fix.stdout, "Fixed 1 problem in 1 file\n");
}

#[test]
fn resolve_baseline_prefers_the_cli_then_the_config() {
    let project = TempProject::new();
    let none = resolve_baseline(None, None, &project.root, false).unwrap();
    assert_eq!(none, None);
    let default = resolve_baseline(None, None, &project.root, true).unwrap();
    assert_eq!(
        default,
        Some(BaselineTarget {
            path: project.root.join(".gdguard/baseline.json"),
            root: project.root.clone(),
            is_explicit: false,
        })
    );

    project.write("gdguard.toml", "[lint]\nbaseline = \"b.json\"\n");
    let nested = project.root.join("nested");
    fs::create_dir_all(&nested).unwrap();
    let from_config = resolve_baseline(None, None, &nested, false)
        .unwrap()
        .unwrap();
    assert_eq!(from_config.path, project.root.join("b.json"));
    assert_eq!(from_config.root, project.root);

    let from_cli = resolve_baseline(Some(Path::new("x.json")), None, &nested, false)
        .unwrap()
        .unwrap();
    assert_eq!(from_cli.path, nested.join("x.json"));
    assert_eq!(from_cli.root, project.root);
    assert!(from_cli.is_explicit);
}
