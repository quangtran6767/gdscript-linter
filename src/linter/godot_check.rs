//! Optional Godot check (`lint --godot <PATH>` or `GDGUARD_GODOT`).
//!
//! gdguard doesn't check types, so this loads the linted scripts in headless
//! Godot 4, once per project, and turns its `SCRIPT ERROR` messages into
//! `godot-error` diagnostics. Godot only prints its warnings as errors when
//! the project sets them to error (`debug/gdscript/warnings/*=2`).
//!
//! One run per project root:
//!
//! 1. When Godot's global class cache (`.godot/global_script_class_cache.cfg`)
//!    doesn't list the same `class_name`s as the project, run `--import` so
//!    new classes resolve. This writes `.godot/` and `.uid` files, as opening
//!    the editor does.
//! 2. Run a small loader script with `--script` that loads each file with
//!    `ResourceLoader.load(path, "", CACHE_MODE_IGNORE)`. Scripts are
//!    compiled, not instantiated, but static initializers may run.
//! 3. Map each `at: GDScript::reload (res://file.gd:LINE)` back to a file.
//!    Errors in other scripts are reported on line 1 of the file that loaded
//!    them.
use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::project::{ProjectIndex, find_root};
use std::collections::{BTreeSet, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const GODOT_ERROR_RULE_ID: &str = "godot-error";

/// How long one Godot process may run before it is stopped.
const GODOT_TIMEOUT: Duration = Duration::from_secs(300);

const LOADER_SCRIPT: &str = "extends SceneTree\n\
\n\
func _init() -> void:\n\
\tfor path in OS.get_cmdline_user_args():\n\
\t\tprinterr(\"GDGUARD-BEGIN \", path)\n\
\t\tResourceLoader.load(path, \"\", ResourceLoader.CACHE_MODE_IGNORE)\n\
\t\tprinterr(\"GDGUARD-END \", path)\n\
\tquit()\n";

/// Loads `files` in Godot and returns the diagnostics of each file, keyed by
/// the path as given. Files outside a Godot project are skipped.
pub fn check_files(
    executable: &Path,
    files: &[PathBuf],
) -> Result<HashMap<PathBuf, Vec<Diagnostic>>, String> {
    let mut by_root: Vec<(PathBuf, Vec<(PathBuf, String)>)> = Vec::new();
    for file in files {
        let Some(root) = find_root(file) else {
            continue;
        };
        let Some(res) = res_path(&root, file) else {
            continue;
        };
        match by_root.iter_mut().find(|(existing, _)| *existing == root) {
            Some((_, entries)) => entries.push((file.clone(), res)),
            None => by_root.push((root, vec![(file.clone(), res)])),
        }
    }
    let mut results = HashMap::new();
    for (root, entries) in by_root {
        let index = ProjectIndex::build(&root)?;
        if !class_cache_is_current(&root, &index) {
            run_godot(
                executable,
                &[
                    "--headless".into(),
                    "--path".into(),
                    root.clone().into_os_string(),
                    "--import".into(),
                ],
            )?;
        }
        let loader = std::env::temp_dir().join(format!(
            "gdguard-godot-check-{}-{}.gd",
            std::process::id(),
            results.len()
        ));
        std::fs::write(&loader, LOADER_SCRIPT)
            .map_err(|error| format!("{}: {}", loader.display(), error))?;
        let mut arguments: Vec<std::ffi::OsString> = vec![
            "--headless".into(),
            "--path".into(),
            root.clone().into_os_string(),
            "--script".into(),
            loader.clone().into_os_string(),
            "--".into(),
        ];
        for (_, res) in &entries {
            arguments.push(res.into());
        }
        let output = run_godot(executable, &arguments);
        let _ = std::fs::remove_file(&loader);
        let output = output?;
        let mut sources = HashMap::new();
        for (file, res) in &entries {
            sources.insert(
                res.clone(),
                (file.clone(), std::fs::read_to_string(file).ok()),
            );
        }
        for (res, line, message) in parse_output(&output) {
            let Some((file, source)) = sources.get(&res) else {
                continue;
            };
            let source = source.as_deref().unwrap_or("");
            let diagnostics: &mut Vec<Diagnostic> = results.entry(file.clone()).or_default();
            diagnostics.push(Diagnostic {
                check: None,
                rule: GODOT_ERROR_RULE_ID.to_string(),
                severity: Severity::Error,
                message,
                help: Some(
                    "Reported by Godot when loading the script. Fix it, then lint again."
                        .to_string(),
                ),
                span: line_span(source, line),
                fixes: Vec::new(),
            });
        }
    }
    Ok(results)
}

/// `res://` path of a file inside `root`.
fn res_path(root: &Path, file: &Path) -> Option<String> {
    let absolute = std::fs::canonicalize(file).ok()?;
    let relative = absolute.strip_prefix(root).ok()?;
    Some(format!(
        "res://{}",
        relative.to_string_lossy().replace('\\', "/")
    ))
}

/// True when Godot's global class cache lists exactly the project's valid
/// `class_name` scripts.
fn class_cache_is_current(root: &Path, index: &ProjectIndex) -> bool {
    let Ok(text) = std::fs::read_to_string(root.join(".godot/global_script_class_cache.cfg"))
    else {
        return false;
    };
    let mut cached = BTreeSet::new();
    let mut class = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("\"class\": &\"") {
            class = Some(value.trim_end_matches(['"', ',']).to_string());
        } else if let Some(value) = line.strip_prefix("\"path\": \"") {
            if let Some(name) = class.take() {
                cached.insert((name, value.trim_end_matches(['"', ',']).to_string()));
            }
        }
    }
    let mut expected = BTreeSet::new();
    for script in &index.scripts {
        if let Some(name) = &script.class_name {
            expected.insert((
                name.clone(),
                format!("res://{}", script.path.to_string_lossy().replace('\\', "/")),
            ));
        }
    }
    cached == expected
}

/// Runs Godot and returns its stderr. Stdout is discarded.
fn run_godot(executable: &Path, arguments: &[std::ffi::OsString]) -> Result<String, String> {
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("--godot: can't run {}: {}", executable.display(), error))?;
    let Some(mut stderr) = child.stderr.take() else {
        return Err("--godot: no stderr pipe".to_string());
    };
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stderr.read_to_end(&mut bytes);
        bytes
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > GODOT_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!(
                    "--godot: {} didn't finish within {} seconds",
                    executable.display(),
                    GODOT_TIMEOUT.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(error) => return Err(format!("--godot: {}", error)),
        }
    }
    let bytes = reader
        .join()
        .map_err(|_| "--godot: the output reader panicked".to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn strip_ansi(line: &str) -> String {
    let mut text = String::with_capacity(line.len());
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' && characters.peek() == Some(&'[') {
            for next in characters.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            text.push(character);
        }
    }
    text
}

/// `(res path, line, message)` for each Godot script error, attributed to
/// the file it is in, or to line 1 of the file being loaded when the error
/// is in another script. Duplicates are dropped.
fn parse_output(output: &str) -> Vec<(String, usize, String)> {
    let mut errors: Vec<(String, usize, String)> = Vec::new();
    let mut current: Option<String> = None;
    let mut pending: Option<String> = None;
    for raw_line in output.lines() {
        let line = strip_ansi(raw_line);
        let trimmed = line.trim();
        if let Some(path) = trimmed.strip_prefix("GDGUARD-BEGIN ") {
            current = Some(path.to_string());
            pending = None;
        } else if trimmed.starts_with("GDGUARD-END ") {
            current = None;
            pending = None;
        } else if let Some(message) = trimmed.strip_prefix("SCRIPT ERROR: ") {
            let message = message.strip_prefix("Parse Error: ").unwrap_or(message);
            pending = Some(message.to_string());
        } else if let (Some(location), Some(message)) = (trimmed.strip_prefix("at: "), &pending) {
            let Some(current) = &current else {
                pending = None;
                continue;
            };
            let mut error = (current.clone(), 1, message.clone());
            if let Some(open) = location.rfind('(') {
                let inside = location[open + 1..].trim_end_matches(')');
                if let Some((path, line_text)) = inside.rsplit_once(':') {
                    let line_number = line_text.parse::<usize>().unwrap_or(1).max(1);
                    if path == current {
                        error = (current.clone(), line_number, message.clone());
                    } else if path.starts_with("res://") {
                        error = (
                            current.clone(),
                            1,
                            format!("In {}:{}: {}", path, line_number, message),
                        );
                    }
                }
            }
            if !errors.contains(&error) {
                errors.push(error);
            }
            pending = None;
        }
    }
    errors
}

/// The span of a 1-based line, from its first non-blank character to its
/// end. Lines past the end use the last line.
fn line_span(source: &str, line: usize) -> Span {
    let mut start = 0;
    for _ in 1..line {
        match source[start..].find('\n') {
            Some(offset) if start + offset + 1 < source.len() => start += offset + 1,
            _ => break,
        }
    }
    let end = match source[start..].find('\n') {
        Some(offset) => start + offset,
        None => source.len(),
    };
    let text = &source[start..end];
    let content_start = start + (text.len() - text.trim_start().len());
    let end = start + text.trim_end().len();
    if content_start < end {
        Span::from_byte_range(source, content_start, end)
    } else {
        Span::from_byte_range(source, start, end)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    #[test]
    fn parses_errors_per_loaded_file() {
        let output = concat!(
            "GDGUARD-BEGIN res://a.gd\n",
            "\u{1b}[1;31mSCRIPT ERROR:\u{1b}[0m Parse Error: Key \"a\" was already used.\n",
            "          at: GDScript::reload (res://a.gd:4)\n",
            "          GDScript backtrace (most recent call first):\n",
            "ERROR: Failed to load script \"res://a.gd\" with error \"Parse error\".\n",
            "   at: load (modules/gdscript/gdscript_resource_format.cpp:46)\n",
            "GDGUARD-END res://a.gd\n",
            "GDGUARD-BEGIN res://b.gd\n",
            "SCRIPT ERROR: Parse Error: Could not parse global class \"Base\".\n",
            "          at: GDScript::reload (res://base.gd:2)\n",
            "SCRIPT ERROR: Parse Error: Could not parse global class \"Base\".\n",
            "          at: GDScript::reload (res://base.gd:2)\n",
            "GDGUARD-END res://b.gd\n",
        );
        assert_eq!(
            parse_output(output),
            vec![
                (
                    "res://a.gd".to_string(),
                    4,
                    "Key \"a\" was already used.".to_string()
                ),
                (
                    "res://b.gd".to_string(),
                    1,
                    "In res://base.gd:2: Could not parse global class \"Base\".".to_string()
                ),
            ]
        );
    }

    #[test]
    fn spans_cover_the_line_content() {
        let source = "extends Node\n\n\tvar x = 1  \n";
        let span = line_span(source, 3);
        assert_eq!(&source[span.start_byte..span.end_byte], "var x = 1");
        let span = line_span(source, 99);
        assert_eq!(span.start.line, 3);
        let span = line_span(source, 1);
        assert_eq!(&source[span.start_byte..span.end_byte], "extends Node");
    }

    #[test]
    fn compares_the_class_cache_with_the_project() {
        let root = std::env::temp_dir().join(format!("gdguard-godot-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".godot")).unwrap();
        std::fs::write(root.join("project.godot"), "config_version=5\n").unwrap();
        std::fs::write(root.join("a.gd"), "class_name Alpha\n").unwrap();
        let index = ProjectIndex::build_uncached(&root).unwrap();
        assert!(!class_cache_is_current(&root, &index));
        std::fs::write(
            root.join(".godot/global_script_class_cache.cfg"),
            "list=[{\n\"base\": &\"RefCounted\",\n\"class\": &\"Alpha\",\n\"icon\": \"\",\n\"path\": \"res://a.gd\"\n}]\n",
        )
        .unwrap();
        assert!(class_cache_is_current(&root, &index));
        std::fs::write(root.join("b.gd"), "class_name Beta\n").unwrap();
        let index = ProjectIndex::build_uncached(&root).unwrap();
        assert!(!class_cache_is_current(&root, &index));
        let _ = std::fs::remove_dir_all(&root);
    }
}
