//! Owner: Track B (W3). `gdguard init`: writes a starter `gdguard.toml` and
//! `.gdguard/rules/`.
//!
//! The files go in the Godot project root: the nearest folder at or above
//! the current folder that has a `project.godot`, or the current folder when
//! there is none. Existing files are never overwritten without `--force`.

use std::path::{Path, PathBuf};

use crate::linter::rules_meta::builtin_rule_metas;

/// The example rule written to `.gdguard/rules/`. It is the same file as
/// `examples/rules/prefer-is-empty.yml`, so its tests run in CI.
const EXAMPLE_RULE: &str = include_str!("../../examples/rules/prefer-is-empty.yml");
const EXAMPLE_RULE_FILE_NAME: &str = "prefer-is-empty.yml";

/// Arguments of `gdguard init`, parsed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InitArgs {
    /// Overwrite existing files.
    pub force: bool,
}

/// Runs `gdguard init` in the current folder and returns the exit code: 0
/// when the files were written, 1 when one already exists (without
/// `--force`), 2 on an I/O error.
pub fn run(args: &InitArgs) -> i32 {
    let current_folder = match std::env::current_dir() {
        Ok(folder) => folder,
        Err(error) => {
            eprintln!("Error: can't read the current folder: {}", error);
            return 2;
        }
    };
    let mut output = String::new();
    let code = init_project(&current_folder, args, &mut output);
    if code == 0 {
        print!("{}", output);
    } else {
        eprint!("{}", output);
    }
    code
}

/// The folder `init` writes to: the nearest folder at or above `start` with
/// a `project.godot`, else `start`.
pub fn find_project_root(start: &Path) -> PathBuf {
    let mut folder = Some(start);
    while let Some(current) = folder {
        if current.join("project.godot").is_file() {
            return current.to_path_buf();
        }
        folder = current.parent();
    }
    start.to_path_buf()
}

/// Writes the starter files for the project around `start`. Messages go to
/// `output`, one per line. Returns the exit code (see [`run`]).
pub fn init_project(start: &Path, args: &InitArgs, output: &mut String) -> i32 {
    let root = find_project_root(start);
    let config_path = root.join("gdguard.toml");
    let rules_folder = root.join(".gdguard").join("rules");
    let rule_path = rules_folder.join(EXAMPLE_RULE_FILE_NAME);
    let files = [
        (config_path, starter_config()),
        (rule_path, EXAMPLE_RULE.replace("\r\n", "\n")),
    ];

    if !args.force {
        let mut existing = 0;
        for (path, _) in &files {
            if path.exists() {
                output.push_str(&format!("{} already exists\n", display(path)));
                existing += 1;
            }
        }
        if existing > 0 {
            output.push_str("Nothing was written. Run `gdguard init --force` to overwrite.\n");
            return 1;
        }
    }

    if let Err(error) = std::fs::create_dir_all(&rules_folder) {
        output.push_str(&format!(
            "Error: can't create {}: {}\n",
            display(&rules_folder),
            error
        ));
        return 2;
    }
    for (path, contents) in &files {
        if let Err(error) = std::fs::write(path, contents) {
            output.push_str(&format!(
                "Error: can't write {}: {}\n",
                display(path),
                error
            ));
            return 2;
        }
        output.push_str(&format!("Created {}\n", display(path)));
    }
    output.push_str("Run `gdguard lint` to check the project, and `gdguard test-rules` to test the rules in .gdguard/rules.\n");
    0
}

fn display(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

/// The commented `gdguard.toml` that `init` writes: every built-in rule with
/// its default level, the example custom rule, and the other sections.
pub fn starter_config() -> String {
    let metas = builtin_rule_metas();
    let mut id_width = 0;
    for meta in &metas {
        id_width = id_width.max(meta.id.len());
    }
    let mut text = String::with_capacity(4096);
    text.push_str(
        "\
# gdguard configuration. Docs: `gdguard explain <rule>`, `gdguard lint --list-rules`.
#
# Rule levels: \"off\" | \"info\" | \"warning\" | \"error\".
# Every built-in rule is listed with its default level; change or remove lines
# as you like. Rules not listed keep their default.

[rules]
",
    );
    for meta in &metas {
        // `load-variable-name` follows the level of `variable-name`, and
        // `fix-failed` comes from the fix engine, so neither is listed.
        if meta.id == "load-variable-name" || meta.id == "fix-failed" {
            continue;
        }
        let value = if meta.id == "max-line-length" {
            format!(
                "{{ level = \"{}\", max = 100 }}",
                meta.default_severity.as_str()
            )
        } else {
            format!("\"{}\"", meta.default_severity.as_str())
        };
        text.push_str(&format!(
            "{:<id_width$} = {:<36} # {}\n",
            meta.id,
            value,
            meta.description,
            id_width = id_width
        ));
    }
    text.push_str(
        "
# Custom rules use the same table.
prefer-is-empty = \"warning\"

[custom_rules]
# Folders of YAML rule files (*.yml). Test them with `gdguard test-rules`.
paths = [\".gdguard/rules\"]

[lint]
# Files to skip, as globs relative to this file.
exclude = [\"addons/**\", \".godot/**\"]
# baseline = \".gdguard/baseline.json\"

# Rule settings for some files only. Later entries win.
# [[overrides]]
# files = [\"tests/**\"]
# rules = { private-access = \"off\" }
",
    );
    text
}
