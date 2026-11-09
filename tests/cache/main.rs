//! Tests for the lint cache (`src/linter/cache.rs`) and parallel linting
//! (`lint_files` in `src/linter.rs`).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

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
            "gdguard-cache-test-{}-{}",
            std::process::id(),
            timestamp
        ));
        fs::create_dir_all(&root).unwrap();
        TempProject { root }
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.root.join(relative_path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, content).unwrap();
    }

    fn read(&self, relative_path: &str) -> String {
        fs::read_to_string(self.root.join(relative_path)).unwrap()
    }

    fn exists(&self, relative_path: &str) -> bool {
        self.root.join(relative_path).exists()
    }

    /// Runs `lint --format compact` with `args`, from the project folder.
    fn lint(&self, args: &[&str]) -> String {
        let mut full_args = vec!["lint", "--format", "compact"];
        full_args.extend_from_slice(args);
        let output = Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
            .current_dir(&self.root)
            .args(&full_args)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(output.status.code().unwrap() < 2, "lint failed: {}", stderr);
        String::from_utf8(output.stdout).unwrap()
    }

    /// Replaces `from` with `to` in the cache file, so a later run that
    /// prints `to` must have read its result from the cache.
    fn tamper_cache(&self, from: &str, to: &str) {
        let cache = self.read(CACHE);
        assert!(cache.contains(from), "cache: {}", cache);
        self.write(CACHE, &cache.replace(from, to));
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

const CACHE: &str = ".gdguard/cache/lint.json";
const MESSAGE: &str = "Debug print left in the code";
const CACHED: &str = "Message read from the cache";

const RULE: &str = "id: no-print-left
severity: warning
message: \"Debug print left in the code\"
rule:
  pattern:
    context: print($$$ARGS)
    selector: expression_statement
";

/// A project with a config, one custom rule and one script that breaks it.
fn project_with_config() -> TempProject {
    let project = TempProject::new();
    project.write(
        "gdguard.toml",
        "[custom_rules]\npaths = [\".gdguard/rules\"]\n",
    );
    project.write(".gdguard/rules/no-print-left.yml", RULE);
    project.write("scripts/player.gd", "func f(x):\n\tprint(x)\n");
    project
}

#[test]
fn second_run_reads_the_cache() {
    let project = project_with_config();
    let first = project.lint(&["scripts"]);
    assert!(first.contains(MESSAGE), "{}", first);
    assert!(project.exists(CACHE));
    assert_eq!(
        project.read(".gdguard/cache/.gitignore"),
        "# Created by gdguard.\n*\n"
    );

    project.tamper_cache(MESSAGE, CACHED);
    let second = project.lint(&["scripts"]);
    assert_eq!(second, first.replace(MESSAGE, CACHED));
}

#[test]
fn w9_cache_does_not_hide_the_new_default_declaration_order_rule() {
    let project = TempProject::new();
    project.write("gdguard.toml", "");
    project.write("a.gd", "var a = 1\nsignal changed\n");
    assert!(project.lint(&["a.gd"]).contains("[declaration-order]"));
    // A W9 cache has no declaration-order diagnostics, despite the same
    // source, configuration and package version.
    let mut cached: serde_json::Value = serde_json::from_str(&project.read(CACHE)).unwrap();
    cached["version"] = 4.into();
    for entry in cached["files"].as_object_mut().unwrap().values_mut() {
        entry["diagnostics"].as_array_mut().unwrap().clear();
    }
    project.write(CACHE, &serde_json::to_string(&cached).unwrap());
    assert!(project.lint(&["a.gd"]).contains("[declaration-order]"));
}

#[test]
fn old_cache_cannot_restore_explicit_loop_warnings_when_inference_is_allowed() {
    let project = TempProject::new();
    project.write(
        "a.gd",
        "func run(values: Array[int]) -> void:\n\tfor value in values:\n\t\tprint(value)\n",
    );
    project.write("gdguard.toml", "");
    assert!(project.lint(&["a.gd"]).contains("Loop variable"));
    let strict: serde_json::Value = serde_json::from_str(&project.read(CACHE)).unwrap();
    let old_diagnostics = strict["files"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap()["diagnostics"]
        .clone();

    project.write(
        "gdguard.toml",
        "[rules]\ntype-annotations = { allow_inferred = true }\n",
    );
    assert!(project.lint(&["a.gd"]).is_empty());
    let mut cached: serde_json::Value = serde_json::from_str(&project.read(CACHE)).unwrap();
    cached["version"] = 8.into();
    // Preserve the current file/settings hashes so only the cache version
    // stops a previous implementation's now-obsolete loop diagnostic.
    for entry in cached["files"].as_object_mut().unwrap().values_mut() {
        entry["diagnostics"] = old_diagnostics.clone();
    }
    project.write(CACHE, &serde_json::to_string(&cached).unwrap());
    assert!(project.lint(&["a.gd"]).is_empty());
}

#[test]
fn original_w10_cache_cannot_reuse_an_incorrectly_safe_reorder_fix() {
    let project = TempProject::new();
    project.write("gdguard.toml", "");
    project.write("a.gd", "var a = 1\n@export var b = a\n");
    project.lint(&["a.gd"]);
    let mut cached: serde_json::Value = serde_json::from_str(&project.read(CACHE)).unwrap();
    cached["version"] = 5.into();
    for entry in cached["files"].as_object_mut().unwrap().values_mut() {
        for diagnostic in entry["diagnostics"].as_array_mut().unwrap() {
            if diagnostic["rule"] == "declaration-order" {
                diagnostic["fixes"][0]["safety"] = "safe".into();
            }
        }
    }
    project.write(CACHE, &serde_json::to_string(&cached).unwrap());
    project.lint(&["a.gd"]);
    let refreshed: serde_json::Value = serde_json::from_str(&project.read(CACHE)).unwrap();
    let entry = refreshed["files"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    let diagnostic = entry["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["rule"] == "declaration-order")
        .unwrap();
    assert_eq!(diagnostic["fixes"][0]["safety"], "unsafe");
}

#[test]
fn reorder_fix_cache_tracks_formatter_only_editorconfig_settings() {
    let project = TempProject::new();
    project.write("gdguard.toml", "");
    project.write(
        "a.gd",
        "var a = 1\nvar b = 2\nsignal changed\n\n\nfunc f():\n\tprint(\"body\")\n",
    );
    let replacement = || {
        project.lint(&["a.gd"]);
        let cached: serde_json::Value = serde_json::from_str(&project.read(CACHE)).unwrap();
        let entry = cached["files"]
            .as_object()
            .unwrap()
            .values()
            .next()
            .unwrap();
        let order = entry["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|diagnostic| diagnostic["rule"] == "declaration-order")
            .unwrap();
        order["fixes"][0]["edits"][0]["replacement"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let formatted_fix = replacement();
    assert!(formatted_fix.contains("var a = 1\nvar b = 2"));
    project.write(
        ".editorconfig",
        "root = true\n[*.gd]\nindent_style = space\n",
    );
    let raw_fix = replacement();
    assert!(raw_fix.contains("var a = 1\n\nvar b = 2"));
    assert!(raw_fix.contains("\tprint(\"body\")"));
    project.write(
        ".editorconfig",
        "root = true\n[*.gd]\nindent_style = tab\ngdscript_formatter_quote_style = single\n",
    );
    assert_eq!(replacement(), raw_fix);
    project.write(".editorconfig", "root = true\n[*.gd]\nindent_style = tab\n");
    assert_eq!(replacement(), formatted_fix);
}

#[test]
fn editing_the_file_misses_the_cache() {
    let project = project_with_config();
    project.lint(&["scripts"]);
    project.tamper_cache(MESSAGE, CACHED);

    project.write("scripts/player.gd", "func f(x):\n\tprint(x + 1)\n");
    let output = project.lint(&["scripts"]);
    assert!(output.contains(MESSAGE), "{}", output);
    assert!(!output.contains(CACHED), "{}", output);
}

#[test]
fn editing_the_config_misses_the_cache() {
    let project = project_with_config();
    project.lint(&["scripts"]);
    project.tamper_cache(MESSAGE, CACHED);

    project.write(
        "gdguard.toml",
        "[custom_rules]\npaths = [\".gdguard/rules\"]\n\n[rules]\nno-print-left = \"error\"\n",
    );
    let output = project.lint(&["scripts"]);
    assert!(output.contains(MESSAGE), "{}", output);
    assert!(output.contains("error"), "{}", output);
    assert!(!output.contains(CACHED), "{}", output);
}

#[test]
fn editing_a_rule_file_misses_the_cache() {
    let project = project_with_config();
    project.lint(&["scripts"]);
    project.tamper_cache(MESSAGE, CACHED);

    project.write(
        ".gdguard/rules/no-print-left.yml",
        &RULE.replace(MESSAGE, "Print call found"),
    );
    let output = project.lint(&["scripts"]);
    assert!(output.contains("Print call found"), "{}", output);
    assert!(!output.contains(CACHED), "{}", output);
}

#[test]
fn no_cache_flag_ignores_and_keeps_the_cache() {
    let project = project_with_config();
    project.lint(&["scripts"]);
    project.tamper_cache(MESSAGE, CACHED);
    let tampered = project.read(CACHE);

    let output = project.lint(&["--no-cache", "scripts"]);
    assert!(output.contains(MESSAGE), "{}", output);
    assert_eq!(project.read(CACHE), tampered);
}

#[test]
fn no_config_means_no_cache_folder() {
    let project = TempProject::new();
    project.write(
        "scripts/player.gd",
        "func f(enemy: Variant) -> void:\n\tenemy._hidden()\n",
    );
    let output = project.lint(&["scripts"]);
    assert!(output.contains("private-access"), "{}", output);
    assert!(!project.exists(".gdguard"));
}

#[test]
fn parallel_output_follows_the_input_order() {
    let project = TempProject::new();
    let mut names = Vec::new();
    for index in 0..40 {
        let name = format!("scripts/file_{:02}.gd", index);
        project.write(
            &name,
            "func f(enemy: Variant) -> void:\n\tenemy._hidden()\n\tenemy._other()\n",
        );
        names.push(name);
    }
    // File discovery sorts the paths; pass them shuffled to show it, and
    // check that the parallel workers keep that sorted order.
    let mut args: Vec<&str> = Vec::with_capacity(names.len());
    for index in 0..names.len() {
        args.push(&names[(index * 17) % names.len()]);
    }

    let first = project.lint(&args);
    let mut expected_order = Vec::with_capacity(names.len() * 2);
    for name in &names {
        expected_order.push(format!("{}:2:", name));
        expected_order.push(format!("{}:3:", name));
    }
    let lines: Vec<&str> = first.lines().collect();
    assert_eq!(lines.len(), expected_order.len(), "{}", first);
    for (line, prefix) in lines.iter().zip(&expected_order) {
        assert!(line.starts_with(prefix.as_str()), "{} vs {}", line, prefix);
    }
    for _ in 0..5 {
        assert_eq!(project.lint(&args), first);
    }
}
