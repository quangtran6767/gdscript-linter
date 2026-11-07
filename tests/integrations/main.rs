//! W4/B8: the Claude Code hook `integrations/claude-code/hooks/lint-gd.sh`.
//!
//! The hook is a POSIX sh script. These tests run it with `sh` and the
//! freshly built binary. When `sh` isn't available they print a note and
//! return early (a test can't mark itself ignored at run time).

#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn hook_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("integrations")
        .join("claude-code")
        .join("hooks")
        .join("lint-gd.sh")
}

fn has_command(name: &str, argument: &str) -> bool {
    match Command::new(name)
        .arg(argument)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

fn has_sh() -> bool {
    match Command::new("sh").args(["-c", "exit 0"]).status() {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}

/// Paths with `/` work in sh on every platform.
fn slash_path(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

fn project(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("gdguard-hook-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(directory.join("project.godot"), "config_version=5\n").unwrap();
    directory
}

struct HookOutput {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run_hook(hook_json: &str, use_jq: bool) -> HookOutput {
    let mut command = Command::new("sh");
    command
        .arg(slash_path(&hook_path()))
        .env(
            "GDGUARD",
            slash_path(Path::new(env!("CARGO_BIN_EXE_gdscript-formatter"))),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if !use_jq {
        command.env("GDGUARD_HOOK_NO_JQ", "1");
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(hook_json.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    HookOutput {
        code: output.status.code().unwrap(),
        stdout: String::from_utf8(output.stdout).unwrap(),
        stderr: String::from_utf8(output.stderr).unwrap(),
    }
}

fn edit_json(file: &Path) -> String {
    serde_json::json!({
        "session_id": "test",
        "hook_event_name": "PostToolUse",
        "tool_name": "Edit",
        "tool_input": {
            "file_path": slash_path(file),
            "old_string": "a",
            "new_string": "b"
        },
        "tool_response": { "success": true }
    })
    .to_string()
}

const DIRTY: &str = "func f():\n\tprint(1)\n\tpass\n\tvar targetName = 1\n\tprint(targetName)\n";
const DIRTY_AFTER_FIX: &str = "func f():\n\tprint(1)\n\tvar targetName = 1\n\tprint(targetName)\n";
const CLEAN: &str = "func f():\n\tprint(1)\n";

fn check_dirty_file(use_jq: bool) {
    let directory = project(if use_jq { "dirty-jq" } else { "dirty" });
    let file = directory.join("player.gd");
    std::fs::write(&file, DIRTY).unwrap();

    let output = run_hook(&edit_json(&file), use_jq);
    let fixed = std::fs::read_to_string(&file).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();

    assert_eq!(output.code, 2, "{}", output.stderr);
    assert_eq!(output.stdout, "");
    // The safe fix (unnecessary-pass) was applied in place.
    assert_eq!(fixed, DIRTY_AFTER_FIX);
    assert!(
        output.stderr.starts_with(&format!(
            "gdguard: {} still has issues after safe fixes (1 errors, 0 warnings, 0 infos).\n",
            slash_path(&file)
        )),
        "{}",
        output.stderr
    );
    assert!(
        output.stderr.contains("Fixed 1 problem"),
        "{}",
        output.stderr
    );
    // The JSON report follows the summary.
    let json_start = output.stderr.find("{\n").unwrap();
    let report: serde_json::Value = serde_json::from_str(&output.stderr[json_start..]).unwrap();
    assert_eq!(report["summary"]["errors"], 1);
    assert_eq!(report["issues"][0]["rule"], "variable-name");
    if use_jq {
        assert!(
            output.stderr.contains(":3:6: error [variable-name] "),
            "{}",
            output.stderr
        );
    }
}

#[test]
fn dirty_file_is_fixed_and_remaining_issues_block_without_jq() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    check_dirty_file(false);
}

#[test]
fn dirty_file_is_fixed_and_remaining_issues_block_with_jq() {
    if !has_sh() || !has_command("jq", "--version") {
        eprintln!("skipped: `sh` or `jq` is not available");
        return;
    }
    check_dirty_file(true);
}

#[test]
fn clean_file_is_silent() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    let directory = project("clean");
    let file = directory.join("clean.gd");
    std::fs::write(&file, CLEAN).unwrap();
    let output = run_hook(&edit_json(&file), false);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stdout, "");
    assert_eq!(output.stderr, "");
}

#[test]
fn file_that_becomes_clean_after_safe_fixes_is_silent() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    let directory = project("fixable");
    let file = directory.join("fixable.gd");
    std::fs::write(&file, "func f():\n\tprint(1)\n\tpass\n").unwrap();
    let output = run_hook(&edit_json(&file), false);
    let fixed = std::fs::read_to_string(&file).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stderr, "");
    assert_eq!(fixed, CLEAN);
}

#[test]
fn other_files_and_missing_files_are_ignored() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    let directory = project("other");
    let text_file = directory.join("notes.txt");
    std::fs::write(&text_file, "var targetName = 1\n").unwrap();
    let text_output = run_hook(&edit_json(&text_file), false);
    let missing_output = run_hook(&edit_json(&directory.join("missing.gd")), false);
    let no_file_output = run_hook(
        "{\"tool_name\":\"Bash\",\"tool_input\":{\"command\":\"ls\"}}",
        false,
    );
    std::fs::remove_dir_all(&directory).unwrap();
    for output in [text_output, missing_output, no_file_output] {
        assert_eq!(output.code, 0, "{}", output.stderr);
        assert_eq!(output.stderr, "");
    }
}

#[test]
fn rule_file_error_is_reported_and_blocks() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    let directory = project("config-error");
    std::fs::write(
        directory.join("gdguard.toml"),
        "[rules]\nunused-argumnet = \"error\"\n",
    )
    .unwrap();
    let file = directory.join("a.gd");
    std::fs::write(&file, CLEAN).unwrap();
    let output = run_hook(&edit_json(&file), false);
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(output.code, 2);
    assert!(
        output.stderr.starts_with(&format!(
            "gdguard couldn't lint {} (exit code 2):\n",
            slash_path(&file)
        )),
        "{}",
        output.stderr
    );
    assert!(
        output.stderr.contains("unused-argumnet"),
        "{}",
        output.stderr
    );
}

#[test]
fn missing_binary_does_not_block() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    let directory = project("no-binary");
    let file = directory.join("a.gd");
    std::fs::write(&file, DIRTY).unwrap();
    let mut child = Command::new("sh")
        .arg(slash_path(&hook_path()))
        .env("GDGUARD", "gdguard-binary-that-does-not-exist")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(edit_json(&file).as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "gdguard hook: 'gdguard-binary-that-does-not-exist' not found; set GDGUARD or add gdguard to PATH. Skipping lint.\n"
    );
}

#[test]
fn file_path_with_escaped_backslashes_is_read_without_jq() {
    if !has_sh() {
        eprintln!("skipped: `sh` is not available");
        return;
    }
    // Windows paths arrive as JSON with `\\`. The sed fallback must unescape
    // them; checking a non-.gd path keeps this test platform independent.
    let output = run_hook(
        "{\"tool_input\":{\"file_path\":\"C:\\\\project\\\\notes.txt\"}}",
        false,
    );
    assert_eq!(output.code, 0);
    assert_eq!(output.stderr, "");
}
