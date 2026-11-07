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

/// Runs the binary inside the sample project with `args`.
fn run_in_sample_project(args: &[&str]) -> CommandOutput {
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
        // Windows prints `.\scripts\a.gd`; snapshots use `/` so they work on
        // every platform.
        stdout: String::from_utf8(output.stdout)
            .expect("stdout should be valid UTF-8")
            .replace('\\', "/"),
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
