//! Tests for `gdguard.toml`: discovery, parsing, precedence, overrides and
//! validation errors (`src/linter/config.rs`).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use gdscript_formatter::linter::api::{self, Config, RuleLevel};
use gdscript_formatter::linter::config::{
    CliRuleOverrides, CompiledConfig, find_config_file, load_and_validate, load_config_file,
    parse_config,
};
use gdscript_formatter::linter::diagnostic::Severity;
use gdscript_formatter::linter::{LintRunOptions, lint_files};

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
            "gdguard-config-test-{}-{}",
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
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Parses `text` as a config file at `/project/gdguard.toml`.
fn parse(text: &str) -> Result<Config, String> {
    parse_config(Path::new("/project/gdguard.toml"), text).map_err(|error| error.to_string())
}

fn parse_error(text: &str) -> String {
    parse(text).unwrap_err()
}

// Discovery

#[test]
fn finds_the_nearest_config_in_nested_folders() {
    let project = TempProject::new();
    let root_config = project.write("gdguard.toml", "");
    let nested_config = project.write("game/levels/gdguard.toml", "");
    let deep_file = project.write("game/levels/forest/tree.gd", "");
    let sibling_file = project.write("game/player/player.gd", "");
    let root_file = project.write("main.gd", "");

    assert_eq!(find_config_file(&deep_file), Some(nested_config.clone()));
    assert_eq!(
        find_config_file(&project.root.join("game/levels")),
        Some(nested_config)
    );
    assert_eq!(find_config_file(&sibling_file), Some(root_config.clone()));
    assert_eq!(find_config_file(&root_file), Some(root_config));
}

#[test]
fn load_config_for_uses_defaults_without_a_config_file() {
    let project = TempProject::new();
    let file = project.write("a.gd", "");
    // The temp folder may be inside a folder with a gdguard.toml on some
    // machines; only check the defaults when there is none above it.
    if find_config_file(&file).is_none() {
        assert_eq!(api::load_config_for(&file).unwrap(), Config::default());
    }
}

#[test]
fn load_config_for_reads_and_validates_the_nearest_file() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[rules]\nunused-argument = \"error\"\n");
    let file = project.write("scripts/a.gd", "");
    let config = api::load_config_for(&file).unwrap();
    assert_eq!(config.config_path, Some(project.root.join("gdguard.toml")));
    assert_eq!(
        config.rules["unused-argument"].level,
        Some(RuleLevel::Error)
    );

    project.write("gdguard.toml", "[rules]\nunused-argumnet = \"error\"\n");
    let error = api::load_config_for(&file).unwrap_err();
    assert_eq!(error.key.as_deref(), Some("rules.unused-argumnet"));
}

// Parsing

#[test]
fn parses_every_section() {
    let config = parse(
        r#"
[rules]
unused-argument = "error"
max-line-length = { level = "warning", max = 120 }
prefer-is-empty = "info"

[custom_rules]
paths = [".gdguard/rules", "more_rules"]

[lint]
exclude = ["addons/**", ".godot/**"]
baseline = ".gdguard/baseline.json"

[[overrides]]
files = ["tests/**"]
rules = { private-access = "off", max-line-length = { max = 200 } }
"#,
    )
    .unwrap();

    assert_eq!(
        config.config_path,
        Some(PathBuf::from("/project/gdguard.toml"))
    );
    assert_eq!(
        config.rules["unused-argument"].level,
        Some(RuleLevel::Error)
    );
    let max_line_length = &config.rules["max-line-length"];
    assert_eq!(max_line_length.level, Some(RuleLevel::Warning));
    assert_eq!(max_line_length.options["max"].as_integer(), Some(120));
    assert!(!max_line_length.options.contains_key("level"));
    // Custom ids are accepted while parsing; `validate_rule_ids` checks them.
    assert_eq!(config.rules["prefer-is-empty"].level, Some(RuleLevel::Info));
    assert_eq!(
        config.custom_rule_paths,
        vec![
            PathBuf::from("/project/.gdguard/rules"),
            PathBuf::from("/project/more_rules")
        ]
    );
    assert_eq!(config.exclude, vec!["addons/**", ".godot/**"]);
    assert_eq!(
        config.baseline,
        Some(PathBuf::from("/project/.gdguard/baseline.json"))
    );
    assert_eq!(config.overrides.len(), 1);
    assert_eq!(config.overrides[0].files, vec!["tests/**"]);
    assert_eq!(
        config.overrides[0].rules["private-access"].level,
        Some(RuleLevel::Off)
    );
    assert_eq!(config.overrides[0].rules["max-line-length"].level, None);
}

#[test]
fn an_empty_config_is_valid() {
    let config = parse("").unwrap();
    assert!(config.rules.is_empty());
    assert!(config.overrides.is_empty());
}

// Errors: every message names the file and the key.

#[test]
fn error_invalid_toml() {
    let error = parse_error("[rules\n");
    assert!(
        error.starts_with("/project/gdguard.toml: invalid TOML: "),
        "{}",
        error
    );
}

#[test]
fn error_unknown_section() {
    assert_eq!(
        parse_error("[rule]\n"),
        "/project/gdguard.toml: `rule`: unknown key; expected one of: rules, custom_rules, lint, overrides"
    );
}

#[test]
fn error_unknown_keys_in_sections() {
    assert_eq!(
        parse_error("[lint]\nexclud = []\n"),
        "/project/gdguard.toml: `lint.exclud`: unknown key; expected one of: exclude, baseline"
    );
    assert_eq!(
        parse_error("[custom_rules]\npath = []\n"),
        "/project/gdguard.toml: `custom_rules.path`: unknown key; expected: paths"
    );
    assert_eq!(
        parse_error("[[overrides]]\nfiles = [\"a\"]\nrule = {}\n"),
        "/project/gdguard.toml: `overrides[0].rule`: unknown key; expected one of: files, rules"
    );
}

#[test]
fn error_bad_level() {
    assert_eq!(
        parse_error("[rules]\nunused-argument = \"warn\"\n"),
        "/project/gdguard.toml: `rules.unused-argument`: invalid level \"warn\"; expected \"off\", \"info\", \"warning\" or \"error\""
    );
    assert_eq!(
        parse_error("[rules]\nmax-line-length = { level = \"loud\" }\n"),
        "/project/gdguard.toml: `rules.max-line-length.level`: invalid level \"loud\"; expected \"off\", \"info\", \"warning\" or \"error\""
    );
    assert_eq!(
        parse_error("[rules]\nunused-argument = 1\n"),
        "/project/gdguard.toml: `rules.unused-argument`: expected a level string (\"off\", \"info\", \"warning\", \"error\") or a table"
    );
    assert_eq!(
        parse_error("[[overrides]]\nfiles = [\"a\"]\nrules = { class-name = \"nope\" }\n"),
        "/project/gdguard.toml: `overrides[0].rules.class-name`: invalid level \"nope\"; expected \"off\", \"info\", \"warning\" or \"error\""
    );
}

#[test]
fn error_bad_builtin_options() {
    assert_eq!(
        parse_error("[rules]\nmax-line-length = { maximum = 120 }\n"),
        "/project/gdguard.toml: `rules.max-line-length.maximum`: unknown option; `max-line-length` accepts: level, max"
    );
    assert_eq!(
        parse_error("[rules]\nunused-argument = { strict = true }\n"),
        "/project/gdguard.toml: `rules.unused-argument.strict`: unknown option; `unused-argument` has no options besides level"
    );
    assert_eq!(
        parse_error("[rules]\nmax-line-length = { max = 0 }\n"),
        "/project/gdguard.toml: `rules.max-line-length.max`: expected a positive integer"
    );
    assert_eq!(
        parse_error("[rules]\nmax-line-length = { max = \"120\" }\n"),
        "/project/gdguard.toml: `rules.max-line-length.max`: expected a positive integer"
    );
}

#[test]
fn error_wrong_value_types() {
    assert_eq!(
        parse_error("[lint]\nexclude = \"addons/**\"\n"),
        "/project/gdguard.toml: `lint.exclude`: expected an array of strings"
    );
    assert_eq!(
        parse_error("[lint]\nbaseline = 1\n"),
        "/project/gdguard.toml: `lint.baseline`: expected a string"
    );
    assert_eq!(
        parse_error("[custom_rules]\npaths = [1]\n"),
        "/project/gdguard.toml: `custom_rules.paths`: expected an array of strings"
    );
    assert_eq!(
        parse_error("rules = 1\n"),
        "/project/gdguard.toml: `rules`: expected a table"
    );
    assert_eq!(
        parse_error("[overrides]\nfiles = []\n"),
        "/project/gdguard.toml: `overrides`: expected an array of tables: write each one as [[overrides]]"
    );
}

#[test]
fn error_override_without_files() {
    assert_eq!(
        parse_error("[[overrides]]\nrules = { class-name = \"off\" }\n"),
        "/project/gdguard.toml: `overrides[0].files`: required: list the glob patterns this override applies to"
    );
}

#[test]
fn error_invalid_glob() {
    let error = parse_error("[lint]\nexclude = [\"addons/[\"]\n");
    assert!(
        error.starts_with(
            "/project/gdguard.toml: `lint.exclude`: invalid glob pattern \"addons/[\": "
        ),
        "{}",
        error
    );
}

#[test]
fn error_unknown_rule_id_suggests_the_closest_one() {
    let project = TempProject::new();
    let config_path = project.write(
        "gdguard.toml",
        "[rules]\nunused-argumnet = \"error\"\n\n[[overrides]]\nfiles = [\"a\"]\nrules = { totally-unknown = \"off\" }\n",
    );
    let error = load_and_validate(&config_path).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "{}: `rules.unused-argumnet`: unknown rule id \"unused-argumnet\"; did you mean \"unused-argument\"?",
            config_path.display()
        )
    );

    project.write(
        "gdguard.toml",
        "[[overrides]]\nfiles = [\"a\"]\nrules = { totally-unknown = \"off\" }\n",
    );
    let error = load_and_validate(&config_path).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "{}: `overrides[0].rules.totally-unknown`: unknown rule id \"totally-unknown\"",
            config_path.display()
        )
    );
}

#[test]
fn load_variable_name_is_a_known_rule_id() {
    let project = TempProject::new();
    let config_path = project.write("gdguard.toml", "[rules]\nload-variable-name = \"off\"\n");
    assert!(load_and_validate(&config_path).is_ok());
}

#[test]
fn error_unreadable_config_file() {
    let project = TempProject::new();
    let missing = project.root.join("missing.toml");
    let error = load_config_file(&missing).unwrap_err();
    assert_eq!(error.path, Some(missing));
    assert!(error.message.starts_with("can't read the config file: "));
}

// Precedence: CLI, then overrides, then [rules], then .editorconfig, then
// defaults.

fn compiled(project: &TempProject, config_text: &str) -> CompiledConfig {
    let config_path = project.write("gdguard.toml", config_text);
    CompiledConfig::new(load_and_validate(&config_path).unwrap()).unwrap()
}

#[test]
fn max_line_length_follows_each_precedence_level() {
    let project = TempProject::new();
    let source_file = project.write("src/a.gd", "");
    let test_file = project.write("tests/a.gd", "");
    let no_cli = CliRuleOverrides::default();

    // 1. Defaults.
    let defaults = compiled(&project, "");
    assert_eq!(
        defaults
            .settings_for_file(&source_file, &no_cli)
            .linter_config
            .max_line_length,
        100
    );

    // 2. `.editorconfig` beats the defaults.
    project.write(
        ".editorconfig",
        "root = true\n\n[*.gd]\nmax_line_length = 80\nindent_size = 2\n",
    );
    let settings = defaults.settings_for_file(&source_file, &no_cli);
    assert_eq!(settings.linter_config.max_line_length, 80);
    assert_eq!(settings.linter_config.indent_size, 2);

    // 3. `[rules]` beats `.editorconfig`.
    let with_rules = compiled(&project, "[rules]\nmax-line-length = { max = 90 }\n");
    assert_eq!(
        with_rules
            .settings_for_file(&source_file, &no_cli)
            .linter_config
            .max_line_length,
        90
    );

    // 4. A matching override beats `[rules]`; other files keep `[rules]`.
    let with_override = compiled(
        &project,
        "[rules]\nmax-line-length = { max = 90 }\n\n[[overrides]]\nfiles = [\"tests/**\"]\nrules = { max-line-length = { max = 120 } }\n",
    );
    assert_eq!(
        with_override
            .settings_for_file(&test_file, &no_cli)
            .linter_config
            .max_line_length,
        120
    );
    assert_eq!(
        with_override
            .settings_for_file(&source_file, &no_cli)
            .linter_config
            .max_line_length,
        90
    );

    // 5. The CLI beats everything.
    let cli = CliRuleOverrides {
        max_line_length: Some(50),
        ..CliRuleOverrides::default()
    };
    assert_eq!(
        with_override
            .settings_for_file(&test_file, &cli)
            .linter_config
            .max_line_length,
        50
    );
}

#[test]
fn rule_levels_follow_each_precedence_level() {
    let project = TempProject::new();
    let source_file = project.write("src/a.gd", "");
    let test_file = project.write("tests/unit/a.gd", "");
    let config = compiled(
        &project,
        "[rules]\nunused-argument = \"error\"\nclass-name = \"info\"\n\n[[overrides]]\nfiles = [\"tests/**\"]\nrules = { unused-argument = \"off\" }\n\n[[overrides]]\nfiles = [\"tests/unit/*.gd\"]\nrules = { class-name = \"warning\" }\n",
    );
    let no_cli = CliRuleOverrides::default();

    let source_settings = config.settings_for_file(&source_file, &no_cli);
    assert_eq!(source_settings.levels["unused-argument"], RuleLevel::Error);
    assert_eq!(source_settings.levels["class-name"], RuleLevel::Info);
    assert!(source_settings.linter_config.disabled_rules.is_empty());

    // Both overrides match; each one changes its own rule.
    let test_settings = config.settings_for_file(&test_file, &no_cli);
    assert_eq!(test_settings.levels["unused-argument"], RuleLevel::Off);
    assert_eq!(test_settings.levels["class-name"], RuleLevel::Warning);
    assert!(
        test_settings
            .linter_config
            .disabled_rules
            .contains("unused-argument")
    );

    let mut cli = CliRuleOverrides::default();
    cli.disabled_rules.insert("class-name".to_string());
    let cli_settings = config.settings_for_file(&source_file, &cli);
    assert_eq!(cli_settings.levels["class-name"], RuleLevel::Off);
    assert!(
        cli_settings
            .linter_config
            .disabled_rules
            .contains("class-name")
    );
}

#[test]
fn later_overrides_win() {
    let project = TempProject::new();
    let file = project.write("tests/a.gd", "");
    let config = compiled(
        &project,
        "[[overrides]]\nfiles = [\"tests/**\"]\nrules = { class-name = \"off\" }\n\n[[overrides]]\nfiles = [\"**/*.gd\"]\nrules = { class-name = \"error\" }\n",
    );
    let settings = config.settings_for_file(&file, &CliRuleOverrides::default());
    assert_eq!(settings.levels["class-name"], RuleLevel::Error);
}

#[test]
fn override_globs_match_relative_to_the_config_folder() {
    let project = TempProject::new();
    let config = compiled(
        &project,
        "[[overrides]]\nfiles = [\"*.gd\"]\nrules = { class-name = \"off\" }\n",
    );
    let no_cli = CliRuleOverrides::default();
    // `*` doesn't cross folders.
    let top_level = project.write("main.gd", "");
    let nested = project.write("scripts/main.gd", "");
    assert!(
        config
            .settings_for_file(&top_level, &no_cli)
            .levels
            .contains_key("class-name")
    );
    assert!(
        !config
            .settings_for_file(&nested, &no_cli)
            .levels
            .contains_key("class-name")
    );
}

#[test]
fn exclude_matches_files_and_their_folders() {
    let project = TempProject::new();
    let config = compiled(
        &project,
        "[lint]\nexclude = [\"addons\", \".godot/**\", \"*_generated.gd\"]\n",
    );
    assert!(config.is_excluded(&project.root.join("addons/plugin/a.gd")));
    assert!(config.is_excluded(&project.root.join(".godot/imported/b.gd")));
    assert!(config.is_excluded(&project.root.join("level_generated.gd")));
    assert!(!config.is_excluded(&project.root.join("scripts/level_generated.gd")));
    assert!(!config.is_excluded(&project.root.join("scripts/addons_helper.gd")));
    // Files outside the config folder never match.
    assert!(!config.is_excluded(Path::new("/elsewhere/addons/a.gd")));
}

// lint_source applies the config like the CLI.

#[test]
fn lint_source_applies_overrides_and_options() {
    let project = TempProject::new();
    let config_path = project.write(
        "gdguard.toml",
        "[rules]\nmax-line-length = { level = \"error\", max = 20 }\n\n[[overrides]]\nfiles = [\"tests/**\"]\nrules = { max-line-length = \"off\" }\n",
    );
    let config = load_and_validate(&config_path).unwrap();
    let source = "var text = \"0123456789\"\n";

    let result = api::lint_source(&project.root.join("src/a.gd"), source, &config).unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].rule, "max-line-length");
    assert_eq!(result.diagnostics[0].severity, Severity::Error);

    let test_result = api::lint_source(&project.root.join("tests/a.gd"), source, &config).unwrap();
    assert!(test_result.diagnostics.is_empty());
}

// Custom rules in the lint pipeline (C1, W2 merge)

const PREFER_IS_EMPTY_RULE: &str = "id: prefer-is-empty\nseverity: warning\nmessage: \"Use `$X.is_empty()`\"\nrule:\n  pattern: len($X) == 0\nfix: $X.is_empty()\nfixSafety: safe\n";

/// Lints every listed file with the nearest config, like `gdguard lint`.
/// Returns `(rule, severity, line)` per diagnostic, in output order.
fn lint_project_files(files: &[PathBuf]) -> Vec<(String, Severity, usize)> {
    let report = lint_files(files, &LintRunOptions::default()).unwrap();
    let mut results = Vec::new();
    for file in &report.files {
        for diagnostic in &file.diagnostics {
            results.push((
                diagnostic.rule.clone(),
                diagnostic.severity,
                diagnostic.span.start.line,
            ));
        }
    }
    results
}

#[test]
fn lint_files_runs_custom_rules_with_builtin_rules_sorted_by_position() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[custom_rules]\npaths = [\"rules\"]\n");
    project.write("rules/prefer-is-empty.yml", PREFER_IS_EMPTY_RULE);
    let file = project.write(
        "a.gd",
        "func f(items):\n\tif len(items) == 0:\n\t\tpass\n\nfunc badName():\n\tpass\n",
    );

    let results = lint_project_files(&[file]);
    assert_eq!(
        results,
        vec![
            ("prefer-is-empty".to_string(), Severity::Warning, 2),
            ("function-name".to_string(), Severity::Error, 5),
        ]
    );
}

#[test]
fn custom_rule_levels_come_from_rules_and_overrides() {
    let project = TempProject::new();
    project.write(
        "gdguard.toml",
        "[rules]\nprefer-is-empty = \"error\"\n\n[custom_rules]\npaths = [\"rules\"]\n\n[[overrides]]\nfiles = [\"tests/**\"]\nrules = { prefer-is-empty = \"off\" }\n",
    );
    project.write("rules/prefer-is-empty.yml", PREFER_IS_EMPTY_RULE);
    let source = "var empty = len([]) == 0\n";
    let game_file = project.write("game/a.gd", source);
    let test_file = project.write("tests/a.gd", source);

    let results = lint_project_files(&[game_file, test_file]);
    assert_eq!(
        results,
        vec![("prefer-is-empty".to_string(), Severity::Error, 1)]
    );
}

#[test]
fn cli_disable_and_inline_ignores_apply_to_custom_rules() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[custom_rules]\npaths = [\"rules\"]\n");
    project.write("rules/prefer-is-empty.yml", PREFER_IS_EMPTY_RULE);
    let file = project.write(
        "a.gd",
        "var a = len([]) == 0  # gdlint-ignore prefer-is-empty\nvar b = len([]) == 0\n",
    );

    let results = lint_project_files(std::slice::from_ref(&file));
    assert_eq!(
        results,
        vec![("prefer-is-empty".to_string(), Severity::Warning, 2)]
    );

    let mut options = LintRunOptions::default();
    options
        .cli
        .disabled_rules
        .insert("prefer-is-empty".to_string());
    let report = lint_files(&[file], &options).unwrap();
    assert!(report.files.is_empty());
}

#[test]
fn invalid_custom_rule_file_is_a_lint_error() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[custom_rules]\npaths = [\"rules\"]\n");
    project.write("rules/broken.yml", "id: broken\nmessage: x\n");
    let file = project.write("a.gd", "var a = 1\n");

    let error = lint_files(&[file], &LintRunOptions::default()).unwrap_err();
    assert!(error.contains("broken.yml"), "error: {}", error);
}

#[test]
fn lint_source_runs_custom_rules_from_the_config() {
    let project = TempProject::new();
    let config_path = project.write(
        "gdguard.toml",
        "[rules]\nprefer-is-empty = \"info\"\n\n[custom_rules]\npaths = [\"rules\"]\n",
    );
    project.write("rules/prefer-is-empty.yml", PREFER_IS_EMPTY_RULE);
    let config = load_and_validate(&config_path).unwrap();

    let result = api::lint_source(
        &project.root.join("a.gd"),
        "var empty = len([]) == 0\n",
        &config,
    )
    .unwrap();
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].rule, "prefer-is-empty");
    assert_eq!(result.diagnostics[0].severity, Severity::Info);
    assert_eq!(result.diagnostics[0].fixes.len(), 1);
}

#[test]
fn cli_disable_rejects_ids_that_are_neither_builtin_nor_custom() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[custom_rules]\npaths = [\"rules\"]\n");
    project.write("rules/prefer-is-empty.yml", PREFER_IS_EMPTY_RULE);
    let file = project.write("a.gd", "var a = 1\n");

    let mut options = LintRunOptions::default();
    options
        .cli
        .disabled_rules
        .insert("prefer-is-empty".to_string());
    options
        .cli
        .disabled_rules
        .insert("unused-argument".to_string());
    assert!(lint_files(std::slice::from_ref(&file), &options).is_ok());

    options.cli.disabled_rules.insert("not-a-rule".to_string());
    let error = lint_files(&[file], &options).unwrap_err();
    assert_eq!(
        error,
        "Invalid rule names: not-a-rule\nUse --list-rules to see all available rules"
    );
}
