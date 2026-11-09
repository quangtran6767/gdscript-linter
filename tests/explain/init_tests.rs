//! W3/B7: `gdguard init`.

use std::path::{Path, PathBuf};
use std::process::Command;

use gdguard::linter::config::load_with_providers;
use gdguard::linter::custom::loader::load_rules;
use gdguard::linter::init::{InitArgs, find_project_root, init_project, starter_config};
use gdguard::linter::rule_config::get_all_rule_names;
use gdguard::linter::rule_tester::{exit_code, test_rules};

fn temp_dir(name: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("gdguard-init-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).unwrap();
    directory
}

fn display(path: &Path) -> String {
    path.display().to_string().replace('\\', "/")
}

#[test]
fn fresh_folder_gets_a_valid_config_and_example_rule() {
    let directory = temp_dir("fresh");
    let mut output = String::new();
    let code = init_project(&directory, &InitArgs::default(), &mut output);
    assert_eq!(code, 0, "{}", output);
    let config_path = directory.join("gdguard.toml");
    let rule_path = directory
        .join(".gdguard")
        .join("rules")
        .join("prefer-is-empty.yml");
    assert_eq!(
        output,
        format!(
            "Created {}\nCreated {}\nRun `gdguard lint` to check the project, and `gdguard test-rules` to test the rules in .gdguard/rules.\n",
            display(&config_path),
            display(&rule_path)
        )
    );

    // The config loads, its rule ids validate (including the custom one),
    // and the example rule's tests pass.
    let (config, providers) = load_with_providers(&config_path).unwrap();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0].rules()[0].id, "prefer-is-empty");
    for name in get_all_rule_names() {
        assert!(config.rules.contains_key(name), "`{}` isn't listed", name);
    }
    let reports = test_rules(load_rules(&config.custom_rule_paths).unwrap());
    assert_eq!(exit_code(&reports), 0, "{:?}", reports);
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn starter_config_lists_defaults_with_descriptions() {
    let config = starter_config();
    assert!(config.contains(
        "\nunused-code                     = \"warning\"                            # Variables, arguments, private functions and signals are used; redundant pass is removed.\n"
    ), "{}", config);
    assert!(
        config.contains(
            "\nmax-line-length                 = { level = \"warning\", max = 100 }     # "
        ),
        "{}",
        config
    );
    assert!(
        config.contains("\nnaming-convention               = \"error\""),
        "{}",
        config
    );
    assert!(!config.contains("load-variable-name ="), "{}", config);
}

#[test]
fn files_go_to_the_godot_project_root() {
    let directory = temp_dir("root");
    std::fs::write(directory.join("project.godot"), "config_version=5\n").unwrap();
    let nested = directory.join("scripts").join("ui");
    std::fs::create_dir_all(&nested).unwrap();
    assert_eq!(find_project_root(&nested), directory);

    let mut output = String::new();
    assert_eq!(init_project(&nested, &InitArgs::default(), &mut output), 0);
    assert!(directory.join("gdguard.toml").is_file());
    assert!(!nested.join("gdguard.toml").exists());
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn without_project_godot_the_current_folder_is_used() {
    let directory = temp_dir("no-project");
    assert_eq!(find_project_root(&directory), directory);
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn existing_files_are_not_overwritten() {
    let directory = temp_dir("existing");
    let config_path = directory.join("gdguard.toml");
    std::fs::write(&config_path, "# mine\n").unwrap();
    let mut output = String::new();
    let code = init_project(&directory, &InitArgs::default(), &mut output);
    assert_eq!(code, 1);
    assert_eq!(
        output,
        format!(
            "{} already exists\nNothing was written. Run `gdguard init --force` to overwrite.\n",
            display(&config_path)
        )
    );
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), "# mine\n");
    // Nothing else was written either.
    assert!(!directory.join(".gdguard").exists());
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn force_overwrites_existing_files() {
    let directory = temp_dir("force");
    let config_path = directory.join("gdguard.toml");
    std::fs::write(&config_path, "# mine\n").unwrap();
    let mut output = String::new();
    let code = init_project(&directory, &InitArgs { force: true }, &mut output);
    assert_eq!(code, 0, "{}", output);
    assert_eq!(
        std::fs::read_to_string(&config_path).unwrap(),
        starter_config()
    );
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn cli_init_then_lint_and_test_rules_work() {
    let directory = temp_dir("cli");
    std::fs::write(directory.join("project.godot"), "config_version=5\n").unwrap();
    std::fs::write(
        directory.join("main.gd"),
        "func f(items):\n\tif len(items) == 0:\n\t\tprint(items)\n",
    )
    .unwrap();
    let binary = env!("CARGO_BIN_EXE_gdscript-formatter");

    let init = Command::new(binary)
        .current_dir(&directory)
        .arg("init")
        .output()
        .unwrap();
    assert_eq!(init.status.code(), Some(0));

    let again = Command::new(binary)
        .current_dir(&directory)
        .arg("init")
        .output()
        .unwrap();
    assert_eq!(again.status.code(), Some(1));
    assert!(
        String::from_utf8(again.stderr)
            .unwrap()
            .contains("already exists")
    );

    let test_rules = Command::new(binary)
        .current_dir(&directory)
        .arg("test-rules")
        .output()
        .unwrap();
    assert_eq!(
        test_rules.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&test_rules.stdout)
    );

    let lint = Command::new(binary)
        .current_dir(&directory)
        .args(["lint", "--format", "compact", "main.gd"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(lint.stdout).unwrap();
    assert!(stdout.contains("[prefer-is-empty]"), "{}", stdout);
    std::fs::remove_dir_all(&directory).unwrap();
}
