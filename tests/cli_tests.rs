use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn test_directory() -> std::path::PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "gdscript-formatter-cli-test-{}-{}",
        std::process::id(),
        timestamp
    ));
    fs::create_dir(&path).expect("should create temporary test directory");
    path
}

fn formatter_command(directory: &std::path::Path, args: &[&str]) -> Command {
    let binary = std::env::current_exe()
        .expect("test executable path should be available")
        .parent()
        .expect("test executable should have a parent")
        .parent()
        .expect("test executable should be in target/debug/deps")
        .join("gdscript-formatter");
    let mut command = Command::new(binary);
    command.current_dir(directory).args(args);
    command
}

#[test]
fn stdin_and_file_modes_apply_editorconfig_and_cli_overrides() {
    let directory = test_directory();
    fs::write(
        directory.join(".editorconfig"),
        "root = true\n\n[*.gd]\nindent_style = space\nindent_size = 8\nmax_line_length = 120\ngdscript_formatter_blank_lines_around_definitions = 2\ngdscript_formatter_quote_style = double\n",
    )
    .expect("should write EditorConfig");

    let mut stdin_command = formatter_command(
        &directory,
        &[
            "--blank-lines-around-definitions",
            "1",
            "--use-spaces",
            "--indent-size",
            "2",
            "--quote-style",
            "single",
        ],
    );

    let input = "func first():\n\tvar value = \"first\"\n\n\nfunc second():\n\tpass\n";
    let expected = "func first():\n  var value = 'first'\n\nfunc second():\n  pass\n";

    stdin_command.stdin(Stdio::piped()).stdout(Stdio::piped());
    let mut child = stdin_command.spawn().expect("should start formatter");
    child
        .stdin
        .take()
        .expect("stdin should be piped")
        .write_all(input.as_bytes())
        .expect("should write formatter input");
    let stdin_output = child
        .wait_with_output()
        .expect("should collect formatter output");
    assert!(stdin_output.status.success());
    assert_eq!(
        String::from_utf8(stdin_output.stdout).expect("stdin output should be valid UTF-8"),
        expected,
    );

    let input_path = directory.join("input.gd");
    fs::write(&input_path, input).expect("should write input file");
    let file_output = formatter_command(
        &directory,
        &[
            "--stdout",
            "--blank-lines-around-definitions",
            "1",
            "--use-spaces",
            "--indent-size",
            "2",
            "--quote-style",
            "single",
            "input.gd",
        ],
    )
    .output()
    .expect("should format file");
    assert!(file_output.status.success());
    assert_eq!(
        String::from_utf8(file_output.stdout).expect("file output should be valid UTF-8"),
        expected,
    );

    let lint_path = directory.join("lint.gd");
    fs::write(&lint_path, format!("#{}\n", "1".repeat(119))).expect("should write lint input");
    let lint_output = formatter_command(&directory, &["lint", "lint.gd"])
        .output()
        .expect("should lint file");
    assert!(lint_output.status.success());
    assert!(lint_output.stdout.is_empty());

    let lint_override_output =
        formatter_command(&directory, &["lint", "--max-line-length", "100", "lint.gd"])
            .output()
            .expect("should lint file with an override");
    assert!(!lint_override_output.status.success());
    assert!(
        String::from_utf8(lint_override_output.stdout)
            .expect("lint output should be valid UTF-8")
            .contains("maximum allowed is 100")
    );

    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

/// Runs the binary and returns its exit code and stderr.
fn run_and_capture_stderr(directory: &std::path::Path, args: &[&str]) -> (i32, String) {
    let output = formatter_command(directory, args)
        .stdin(Stdio::null())
        .output()
        .expect("should run the binary");
    let exit_code = output
        .status
        .code()
        .expect("the binary should exit with a code");
    let stderr = String::from_utf8(output.stderr).expect("stderr should be valid UTF-8");
    (exit_code, stderr)
}

#[test]
fn github_and_sarif_formats_report_issues() {
    let directory = test_directory();
    fs::write(directory.join("a.gd"), SCRIPT_WITH_ONE_ERROR).expect("should write input file");

    let (exit_code, stdout, stderr) =
        run_and_capture(&directory, &["lint", "--format", "github", "a.gd"]);
    assert_eq!(exit_code, 1, "{}", stderr);
    assert!(
        stdout.starts_with("::error file=a.gd,line=1,col="),
        "{}",
        stdout
    );
    assert!(stdout.contains(",title=function-name::"));

    let (exit_code, stdout, stderr) =
        run_and_capture(&directory, &["lint", "--format=sarif", "a.gd"]);
    assert_eq!(exit_code, 1, "{}", stderr);
    let log: serde_json::Value = serde_json::from_str(&stdout).expect("sarif should parse");
    assert_eq!(log["runs"][0]["results"][0]["ruleId"], "function-name");
    assert_eq!(
        log["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "a.gd"
    );
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn unimplemented_subcommands_exit_with_code_2_and_name_the_command() {
    let directory = test_directory();
    let cases: [(&[&str], &str); 1] = [(&["lsp"], "lsp")];
    for (args, command) in cases {
        let (exit_code, stderr) = run_and_capture_stderr(&directory, args);
        assert_eq!(exit_code, 2, "{:?} should exit with code 2", args);
        assert_eq!(stderr, format!("`{}` is not implemented yet\n", command));
    }
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn list_rules_text_output_lists_rules_sorted_by_id() {
    let directory = test_directory();
    let output = formatter_command(&directory, &["lint", "--list-rules"])
        .output()
        .expect("should list rules");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be valid UTF-8");
    assert!(stdout.starts_with("Available linting rules:\n  assigned-but-never-read "));
    assert!(stdout.contains("\n  duplicated-load "));
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn bad_subcommand_and_format_arguments_are_usage_errors() {
    let directory = test_directory();
    let cases: [(&[&str], &str); 5] = [
        (
            &["lint", "--format", "xml", "a.gd"],
            "gdscript-formatter: error: --format expects pretty, compact, json, github or sarif, got 'xml'\n",
        ),
        (
            &["lint", "--list-rules", "--format", "sarif"],
            "gdscript-formatter: error: --format with --list-rules expects text or json, got 'sarif'\n",
        ),
        (
            &["explain"],
            "gdscript-formatter: error: explain requires a rule id: explain <RULE>\n",
        ),
        (
            &["test-rules", "a", "b"],
            "gdscript-formatter: error: unexpected argument 'b' for 'test-rules'\n",
        ),
        (
            &["init", "--oops"],
            "gdscript-formatter: error: unexpected argument '--oops' for 'init'\n",
        ),
    ];
    for (args, expected_stderr) in cases {
        let (exit_code, stderr) = run_and_capture_stderr(&directory, args);
        assert_eq!(exit_code, 2, "{:?} should exit with code 2", args);
        assert_eq!(stderr, expected_stderr);
    }
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

/// Writes `files` (name, content) into a new temporary directory.
fn directory_with_files(files: &[(&str, &str)]) -> std::path::PathBuf {
    let directory = test_directory();
    for (name, content) in files {
        fs::write(directory.join(name), content).expect("should write test file");
    }
    directory
}

const CLEAN_SCRIPT: &str = "var health = 1\n";
// One `unused-argument` warning.
const SCRIPT_WITH_ONE_WARNING: &str = "func f(unused):\n\tpass\n";
// Two `unused-argument` warnings.
const SCRIPT_WITH_TWO_WARNINGS: &str = "func f(first, second):\n\tpass\n";
// One `function-name` error.
const SCRIPT_WITH_ONE_ERROR: &str = "func badName():\n\tpass\n";

#[test]
fn lint_exit_code_is_0_when_clean() {
    let directory = directory_with_files(&[("clean.gd", CLEAN_SCRIPT)]);
    let (exit_code, stderr) = run_and_capture_stderr(&directory, &["lint", "clean.gd"]);
    assert_eq!(exit_code, 0, "stderr: {}", stderr);
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_exit_code_is_1_when_problems_are_found() {
    let directory = directory_with_files(&[
        ("warning.gd", SCRIPT_WITH_ONE_WARNING),
        ("error.gd", SCRIPT_WITH_ONE_ERROR),
    ]);
    let (warning_exit_code, _) = run_and_capture_stderr(&directory, &["lint", "warning.gd"]);
    assert_eq!(warning_exit_code, 1);
    let (error_exit_code, _) = run_and_capture_stderr(&directory, &["lint", "error.gd"]);
    assert_eq!(error_exit_code, 1);
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_exit_code_is_2_on_usage_errors() {
    let directory = directory_with_files(&[("clean.gd", CLEAN_SCRIPT)]);

    let (exit_code, stderr) =
        run_and_capture_stderr(&directory, &["lint", "--disable", "not-a-rule", "clean.gd"]);
    assert_eq!(exit_code, 2);
    assert_eq!(
        stderr,
        "Error: Invalid rule names: not-a-rule\nUse --list-rules to see all available rules\n"
    );

    let (exit_code, stderr) = run_and_capture_stderr(&directory, &["lint", "no_scripts_here"]);
    assert_eq!(exit_code, 2);
    assert!(
        stderr.starts_with("Error: No GDScript files found"),
        "{}",
        stderr
    );

    let (exit_code, stderr) = run_and_capture_stderr(&directory, &["lint", "missing.gd"]);
    assert_eq!(exit_code, 2);
    assert!(
        stderr.starts_with("Error: Failed to read file missing.gd"),
        "{}",
        stderr
    );

    let (exit_code, _) = run_and_capture_stderr(&directory, &["lint", "--max-warnings", "x"]);
    assert_eq!(exit_code, 2);
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_max_warnings_allows_up_to_n_warnings() {
    let directory = directory_with_files(&[
        ("two_warnings.gd", SCRIPT_WITH_TWO_WARNINGS),
        ("error.gd", SCRIPT_WITH_ONE_ERROR),
    ]);
    let cases: [(&[&str], i32); 5] = [
        (&["lint", "two_warnings.gd"], 1),
        (&["lint", "--max-warnings", "1", "two_warnings.gd"], 1),
        (&["lint", "--max-warnings", "2", "two_warnings.gd"], 0),
        (&["lint", "--max-warnings=5", "two_warnings.gd"], 0),
        // Errors fail no matter how many warnings are allowed.
        (&["lint", "--max-warnings", "100", "error.gd"], 1),
    ];
    for (args, expected_exit_code) in cases {
        let (exit_code, stderr) = run_and_capture_stderr(&directory, args);
        assert_eq!(exit_code, expected_exit_code, "{:?}: {}", args, stderr);
    }
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_stdin_uses_the_same_exit_codes() {
    let directory = test_directory();
    let cases: [(&str, &[&str], i32); 3] = [
        (CLEAN_SCRIPT, &["lint"], 0),
        (SCRIPT_WITH_ONE_WARNING, &["lint"], 1),
        (SCRIPT_WITH_ONE_WARNING, &["lint", "--max-warnings", "1"], 0),
    ];
    for (input, args, expected_exit_code) in cases {
        let mut child = formatter_command(&directory, args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("should start the linter");
        child
            .stdin
            .take()
            .expect("stdin should be piped")
            .write_all(input.as_bytes())
            .expect("should write linter input");
        let output = child.wait_with_output().expect("should collect output");
        assert_eq!(
            output.status.code(),
            Some(expected_exit_code),
            "{:?} on {:?}",
            args,
            input
        );
    }
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

fn run_and_capture(directory: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let output = formatter_command(directory, args)
        .stdin(Stdio::null())
        .output()
        .expect("should run the binary");
    (
        output.status.code().expect("should exit with a code"),
        String::from_utf8(output.stdout).expect("stdout should be valid UTF-8"),
        String::from_utf8(output.stderr).expect("stderr should be valid UTF-8"),
    )
}

#[test]
fn lint_default_output_is_compact_when_not_a_terminal() {
    let directory = directory_with_files(&[("error.gd", SCRIPT_WITH_ONE_ERROR)]);
    let (exit_code, stdout, _) = run_and_capture(&directory, &["lint", "error.gd"]);
    assert_eq!(exit_code, 1);
    assert_eq!(
        stdout,
        "error.gd:1:6: error [function-name] Function name 'badName' should be in snake_case, _private_snake_case format\n"
    );
    // `--pretty` is kept as an alias of `--format pretty`.
    let (_, pretty_stdout, _) = run_and_capture(&directory, &["lint", "--pretty", "error.gd"]);
    let (_, format_stdout, _) =
        run_and_capture(&directory, &["lint", "--format", "pretty", "error.gd"]);
    assert_eq!(pretty_stdout, format_stdout);
    assert!(
        pretty_stdout.contains("1 | func badName():\n"),
        "{}",
        pretty_stdout
    );
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_json_output_is_valid_json() {
    let directory = directory_with_files(&[("error.gd", SCRIPT_WITH_ONE_ERROR)]);
    let (exit_code, stdout, _) =
        run_and_capture(&directory, &["lint", "--format", "json", "error.gd"]);
    assert_eq!(exit_code, 1);
    assert!(stdout.starts_with("{\n  \"version\": 1,\n"), "{}", stdout);
    assert!(stdout.contains("\"rule\": \"function-name\""), "{}", stdout);
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_reads_the_nearest_gdguard_toml() {
    let directory = directory_with_files(&[
        (
            "gdguard.toml",
            "[rules]\nfunction-name = \"off\"\nunused-argument = \"error\"\n",
        ),
        ("error.gd", SCRIPT_WITH_ONE_ERROR),
        ("warning.gd", SCRIPT_WITH_ONE_WARNING),
    ]);
    let (exit_code, stdout, _) = run_and_capture(&directory, &["lint", "error.gd"]);
    assert_eq!(exit_code, 0, "{}", stdout);
    let (exit_code, stdout, _) = run_and_capture(&directory, &["lint", "warning.gd"]);
    assert_eq!(exit_code, 1);
    assert!(stdout.contains(": error [unused-argument]"), "{}", stdout);
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_config_errors_exit_with_code_2() {
    let directory = directory_with_files(&[
        ("gdguard.toml", "[rules]\nfunction-nam = \"off\"\n"),
        ("clean.gd", CLEAN_SCRIPT),
    ]);
    let (exit_code, _, stderr) = run_and_capture(&directory, &["lint", "clean.gd"]);
    assert_eq!(exit_code, 2);
    assert!(
        stderr.ends_with(
            "gdguard.toml: `rules.function-nam`: unknown rule id \"function-nam\"; did you mean \"function-name\"?\n"
        ),
        "{}",
        stderr
    );
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_config_flag_replaces_discovery() {
    let directory = directory_with_files(&[
        ("gdguard.toml", "[rules]\nfunction-nam = \"off\"\n"),
        ("other.toml", "[rules]\nfunction-name = \"off\"\n"),
        ("error.gd", SCRIPT_WITH_ONE_ERROR),
    ]);
    let (exit_code, stdout, stderr) =
        run_and_capture(&directory, &["lint", "--config", "other.toml", "error.gd"]);
    assert_eq!(exit_code, 0, "{}{}", stdout, stderr);

    let (exit_code, _, stderr) = run_and_capture(
        &directory,
        &["lint", "--config", "missing.toml", "error.gd"],
    );
    assert_eq!(exit_code, 2);
    assert_eq!(stderr, "Error: --config: missing.toml is not a file\n");
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}

#[test]
fn lint_skips_excluded_files() {
    let directory = directory_with_files(&[
        ("gdguard.toml", "[lint]\nexclude = [\"generated\"]\n"),
        ("clean.gd", CLEAN_SCRIPT),
    ]);
    fs::create_dir(directory.join("generated")).expect("should create folder");
    fs::write(directory.join("generated/error.gd"), SCRIPT_WITH_ONE_ERROR)
        .expect("should write file");
    let (exit_code, stdout, _) = run_and_capture(&directory, &["lint", "."]);
    assert_eq!(exit_code, 0, "{}", stdout);
    fs::remove_dir_all(directory).expect("should remove temporary test directory");
}
