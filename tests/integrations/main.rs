//! The hook runs the real CLI, including its fix and error paths.

#![allow(clippy::unwrap_used)]

mod docs;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

struct Project {
    root: PathBuf,
}

impl Project {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("gdguard-hook-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn hook(&self, path: &Path, search_path: Option<&Path>) -> Output {
        self.hook_with_env(path, search_path, &[])
    }

    fn hook_with_env(
        &self,
        path: &Path,
        search_path: Option<&Path>,
        envs: &[(&str, &str)],
    ) -> Output {
        let script =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("integrations/claude-code/hooks/lint-gd.sh");
        let mut command = Command::new("sh");
        command
            .arg(script)
            .current_dir(&self.root)
            .env("GDGUARD_BIN", env!("CARGO_BIN_EXE_gdscript-formatter"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(search_path) = search_path {
            command.env("PATH", search_path);
        }
        for (name, value) in envs {
            command.env(name, value);
        }
        let mut child = command.spawn().expect("hook needs POSIX sh");
        let payload = serde_json::json!({
            "tool_name": "MultiEdit",
            "tool_input": { "file_path": path, "edits": [] }
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(payload.to_string().as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn clean_file_returns_nothing() {
    let project = Project::new("clean");
    let path = project.root.join("clean.gd");
    fs::write(&path, "var value: int = 1\n").unwrap();
    let output = project.hook(&path, None);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

const DIRTY_SOURCE: &str = "var badName: int = 1\n# gdguard-ignore-next-line unused-argument -- stale\nfunc f() -> void:\n\tpass\n";

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn dirty_file_returns_short_text_and_applies_safe_fixes() {
    let project = Project::new("dirty");
    let path = project.root.join("dirty.gd");
    fs::write(&path, DIRTY_SOURCE).unwrap();
    let output = project.hook(&path, None);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let mut lines = stderr.lines();
    assert_eq!(
        lines.next(),
        Some("gdguard: safe fixes are applied; these issues remain. Fix them and lint again.")
    );
    assert!(
        stderr.contains("dirty.gd:1:5: error [naming-convention]"),
        "{}",
        stderr
    );
    assert!(stderr.contains("  > var badName: int = 1\n"), "{}", stderr);
    assert!(stderr.contains("1 issue (1 error)"), "{}", stderr);
    assert!(stderr.contains("Fixed 1 problem in 1 file"), "{}", stderr);
    assert!(!stderr.contains('{'), "no JSON by default: {}", stderr);
    assert!(!fs::read_to_string(path).unwrap().contains("gdguard-ignore"));
}

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn json_format_returns_remaining_json() {
    let project = Project::new("dirty-json");
    let path = project.root.join("dirty.gd");
    fs::write(&path, DIRTY_SOURCE).unwrap();
    let output = project.hook_with_env(&path, None, &[("GDGUARD_HOOK_FORMAT", "json")]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    let (_, json) = stderr.split_once('\n').unwrap();
    let report: serde_json::Value = serde_json::from_str(json).unwrap();
    assert_eq!(report["issues"][0]["rule"], "naming-convention");
    assert!(!fs::read_to_string(path).unwrap().contains("gdguard-ignore"));
}

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn unknown_hook_format_is_a_tool_error() {
    let project = Project::new("bad-format");
    let path = project.root.join("clean.gd");
    fs::write(&path, "var value: int = 1\n").unwrap();
    let output = project.hook_with_env(&path, None, &[("GDGUARD_HOOK_FORMAT", "xml")]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("GDGUARD_HOOK_FORMAT must be agent or json, got xml")
    );
}

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn fix_format_config_formats_the_edited_file() {
    let project = Project::new("fix-format");
    fs::write(project.root.join("gdguard.toml"), "[fix]\nformat = true\n").unwrap();
    let path = project.root.join("messy.gd");
    fs::write(
        &path,
        "var value: int = 1\nfunc f() -> void:\n\tprint(  value )\n",
    )
    .unwrap();
    let output = project.hook(&path, None);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read_to_string(path).unwrap(),
        "var value: int = 1\n\n\nfunc f() -> void:\n\tprint(value)\n"
    );
}

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn safe_fixes_can_make_the_file_clean() {
    let project = Project::new("fix");
    let path = project.root.join("fixed.gd");
    fs::write(
        &path,
        "# gdguard-ignore-file unused-argument -- stale\nvar value: int = 1\n",
    )
    .unwrap();
    let output = project.hook(&path, None);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(fs::read_to_string(path).unwrap(), "var value: int = 1\n");
}

#[test]
#[cfg_attr(windows, ignore = "requires POSIX sh")]
fn other_files_are_skipped_and_tool_errors_are_returned() {
    let project = Project::new("errors");
    let skipped = project.hook(&project.root.join("not present.txt"), None);
    assert_eq!(skipped.status.code(), Some(0));
    assert!(skipped.stderr.is_empty());
    let missing = project.hook(&project.root.join("not present.gd"), None);
    assert_eq!(missing.status.code(), Some(2));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("lint failed")
    );
}

#[test]
#[cfg(unix)]
fn python_fallback_decodes_escaped_paths_without_jq() {
    let project = Project::new("no-jq");
    let tools_dir = project.root.join("tools");
    fs::create_dir(&tools_dir).unwrap();
    // An isolated PATH forces the fallback even on machines with jq.
    for program in ["sh", "python3", "cat", "mktemp", "rm"] {
        let output = Command::new("sh")
            .args(["-c", &format!("command -v {}", program)])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{} required for fallback test",
            program
        );
        let executable = String::from_utf8(output.stdout).unwrap();
        std::os::unix::fs::symlink(executable.trim(), tools_dir.join(program)).unwrap();
    }
    let path = project.root.join("space \"quoted\" \\ unicode-đ.gd");
    fs::write(&path, "var badName: int = 1\n").unwrap();
    let output = project.hook(&path, Some(&tools_dir));
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("space \"quoted\" / unicode-đ.gd:1:5: error [naming-convention]"),
        "{}",
        stderr
    );
}
