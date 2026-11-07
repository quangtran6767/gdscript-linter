//! Command-line entry point for the GDScript formatter and linter.
//!
//! Formatting settings come from three places. The formatter starts with its
//! built-in defaults, applies the `.editorconfig` properties that match each
//! input file, and then applies any CLI option flags explicitly provided on the
//! command line. Command line flags override editorconfig settings which
//! override built-in defaults.
//!
//! Stdin input uses the current directory to find `.editorconfig`. It uses a
//! synthetic `stdin.gd` path so filename sections such as `[*.gd]` also match.
//!
//! The formatter keeps both batch implementations below. The sequential path
//! is the normal path because it is easier to follow in stack traces. The
//! parallel path remains available for performance-sensitive callers and uses
//! the same per-file configuration logic.

mod cli;

use std::{
    env, fs,
    io::{self, IsTerminal, Read, Write},
    path::{Path, PathBuf},
    thread,
};

use gdscript_formatter::linter::baseline::{
    Baseline, BaselineTarget, apply_baseline, entries_for_report as baseline_entries_for_report,
    load_target, resolve_baseline, write_baseline,
};
use gdscript_formatter::linter::config::CliRuleOverrides;
use gdscript_formatter::linter::fix::unified_diff;
use gdscript_formatter::linter::output::{LintReport, OutputFormat, display_path, write_report};
use gdscript_formatter::linter::rule_config::parse_disabled_rules;
use gdscript_formatter::linter::{
    FixRunReport, LintRunOptions, fix_files, fix_stdin, lint_files, lint_stdin,
};
use gdscript_formatter::linter::{explain, init, rule_tester};
use gdscript_formatter::{
    FormatErrors, FormatterConfiguration, QuoteStyle, RenderElement, format_gdscript,
    format_gdscript_with_buffers,
};

use cli::{Command, parse_args};

#[repr(i32)]
enum FormatterExitCodes {
    NotFormatted = 1,
    ParseErrors = 2,
}

struct FormatterOutput {
    index: usize,
    file_path: PathBuf,
    formatted_content: String,
    is_formatted: bool,
}

enum FormatterFileProcessingResult {
    SkippedParseErrors { index: usize, file_path: PathBuf },
    Formatted(FormatterOutput),
}

#[derive(Clone, Copy)]
struct FormatterConfigOverrides {
    /// Explicitly requested tab or space indentation.
    use_spaces: Option<bool>,
    /// Explicitly requested indentation width.
    indent_size: Option<usize>,
    /// Explicitly requested maximum line length.
    max_line_length: Option<usize>,
    /// Explicitly requested blank lines between top-level definitions.
    blank_lines_around_definitions: Option<u16>,
    /// Explicitly requested continuation indentation depth.
    continuation_indent_level: Option<u16>,
    /// Explicitly requested string quote style.
    quote_style: Option<QuoteStyle>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let parsed_cli_args = parse_args();

    // The linter tool subcommands live in the library so their owners can
    // implement them without touching the CLI. Each returns an exit code.
    match &parsed_cli_args.command {
        Command::Explain(explain_args) => std::process::exit(explain::run(explain_args)),
        Command::TestRules(test_rules_args) => {
            std::process::exit(rule_tester::run(test_rules_args))
        }
        Command::Init(init_args) => std::process::exit(init::run(init_args)),
        Command::Lsp => exit_not_implemented("lsp"),
        Command::Format { .. } | Command::Lint { .. } => {}
    }

    if let Command::Lint {
        disabled_linter_rules,
        max_line_length,
        do_list_rules,
        do_pretty_print,
        list_rules_format,
        do_fix,
        do_fix_unsafe,
        do_diff,
        output_format,
        config_path,
        baseline_path,
        do_update_baseline,
        max_warnings,
        stdin_filename,
    } = parsed_cli_args.command
    {
        if do_list_rules {
            std::process::exit(explain::list_rules(list_rules_format));
        }

        if do_update_baseline && (do_fix || do_fix_unsafe || do_diff) {
            eprintln!(
                "Error: --update-baseline can't be combined with --fix, --fix-unsafe or --diff"
            );
            std::process::exit(2);
        }
        let current_directory = match env::current_dir() {
            Ok(directory) => directory,
            Err(error) => {
                eprintln!("Error: Failed to get current directory: {}", error);
                std::process::exit(2);
            }
        };
        let baseline_target = match resolve_baseline(
            baseline_path.as_deref(),
            config_path.as_deref(),
            &current_directory,
            do_update_baseline,
        ) {
            Ok(target) => target,
            Err(error) => {
                eprintln!("Error: {}", error);
                std::process::exit(2);
            }
        };

        // `--format` wins; the old `--pretty` flag means `--format pretty`.
        // Otherwise: pretty for people at a terminal, compact for tools.
        let is_terminal = io::stdout().is_terminal();
        let format = match output_format {
            Some(format) => format,
            None if do_pretty_print || is_terminal => OutputFormat::Pretty,
            None => OutputFormat::Compact,
        };
        let lint_command = LintCommand {
            input_file_paths: parsed_cli_args.input_file_paths,
            excluded_paths: parsed_cli_args.excluded_paths,
            disabled_linter_rules,
            max_line_length,
            config_path,
            stdin_filename,
        };
        if do_update_baseline {
            std::process::exit(run_update_baseline(
                lint_command,
                baseline_target,
                &current_directory,
            ));
        }
        let baseline = match load_run_baseline(baseline_target) {
            Ok(baseline) => baseline,
            Err(error) => {
                eprintln!("Error: {}", error);
                std::process::exit(2);
            }
        };
        if do_fix || do_fix_unsafe || do_diff {
            let fix_command = FixCommand {
                allow_unsafe: do_fix_unsafe,
                diff_only: do_diff,
                format,
                is_terminal,
                max_warnings,
                baseline,
                current_directory,
            };
            std::process::exit(run_fix_command(lint_command, &fix_command));
        }
        // Exit codes: 0 clean, 1 lint problems, 2 usage or config errors.
        let mut report = match run_linter(lint_command) {
            Ok(report) => report,
            Err(error) => {
                eprintln!("Error: {}", error);
                std::process::exit(2);
            }
        };
        if let Some((baseline, root)) = &baseline {
            report.baseline_hidden =
                apply_baseline(baseline, &mut report, &current_directory, root);
        }
        let mut output = String::new();
        if let Err(error) = write_report(format, &report, is_terminal, &mut output) {
            eprintln!("Error: {}", error);
            std::process::exit(2);
        }
        print!("{}", output);
        std::process::exit(report.summary().exit_code(max_warnings));
    }

    let Command::Format {
        do_print_to_stdout,
        use_verbose_output,
        use_debug_output,
        do_check_formatted_only,
        use_spaces,
        indent_size,
        use_verify_structure,
        do_reorder_code,
        max_line_length,
        blank_lines_around_definitions,
        continuation_indent_level,
        quote_style,
    } = parsed_cli_args.command
    else {
        unreachable!();
    };

    let mut config = FormatterConfiguration {
        safe: use_verify_structure,
        reorder_code: do_reorder_code,
        ..Default::default()
    };

    if let Some(quote_style) = quote_style {
        config.quote_style = quote_style;
    }

    let config_overrides = FormatterConfigOverrides {
        use_spaces,
        indent_size,
        max_line_length,
        blank_lines_around_definitions,
        continuation_indent_level,
        quote_style,
    };

    if parsed_cli_args.input_file_paths.is_empty() && !io::stdin().is_terminal() {
        let mut input_content = String::new();
        io::stdin()
            .read_to_string(&mut input_content)
            .map_err(|error| format!("Failed to read from stdin: {}", error))?;

        let current_directory = env::current_dir().expect("Failed to get current directory");
        // When running from stdin users would still like to apply editorconfig
        // settings. For the most part, you'd use a section like `[*.gd]`
        // matching GDScript files. we fake running the formatter on a `.gd`
        // file in the current directory to get user settings to apply.
        let stdin_path = current_directory.join("stdin.gd");
        let stdin_config =
            get_final_config_applied_to_script(&config, &stdin_path, config_overrides);
        if use_debug_output {
            eprintln!(
                "{}",
                get_formatted_configuration_as_string(&stdin_path, &stdin_config)
            );
        }
        let formatted_content = match format_gdscript(&input_content, &stdin_config) {
            Ok(formatted_content) => formatted_content,
            Err(FormatErrors::ParseErrors) => {
                eprintln!(
                    "Skipping formatting stdin: the input GDScript code contains parse errors"
                );
                std::process::exit(FormatterExitCodes::ParseErrors as i32);
            }
            Err(error) => return Err(error.into()),
        };

        if do_check_formatted_only {
            if input_content != formatted_content {
                eprintln!("The input passed via stdin is not formatted");
                std::process::exit(1);
            } else {
                eprintln!("The input passed via stdin is already formatted");
            }
        } else {
            print!("{}", formatted_content);
        }

        return Ok(());
    }

    let input_paths = if parsed_cli_args.input_file_paths.is_empty() {
        vec![
            env::current_dir()
                .map_err(|error| format!("Failed to get current directory: {}", error))?,
        ]
    } else {
        parsed_cli_args.input_file_paths
    };
    let input_gdscript_files = find_gdscript_files(&input_paths, &parsed_cli_args.excluded_paths)?;
    if input_gdscript_files.is_empty() {
        eprintln!("Error: {}", NO_GDSCRIPT_FILES_MESSAGE);
        std::process::exit(1);
    }

    let total_files = input_gdscript_files.len();

    if !use_verbose_output {
        eprint!(
            "Formatting {} file{}...",
            total_files,
            if total_files == 1 { "" } else { "s" }
        );
        let _ = io::stdout().flush();
    }

    if use_debug_output {
        for file_path in &input_gdscript_files {
            let file_config =
                get_final_config_applied_to_script(&config, file_path, config_overrides);
            eprintln!(
                "{}",
                get_formatted_configuration_as_string(file_path, &file_config)
            );
        }
    }

    let mut sorted_outputs: Vec<Result<FormatterFileProcessingResult, String>> =
        format_files_parallel(&input_gdscript_files, &config, config_overrides);

    sorted_outputs.sort_by(compare_output_index);

    let mut all_formatted = true;
    let mut modified_file_count = 0;
    let mut unformatted_files = Vec::new();
    let mut skipped_parse_error_files = Vec::new();
    for output in sorted_outputs {
        match output {
            Ok(FormatterFileProcessingResult::SkippedParseErrors { file_path, .. }) => {
                skipped_parse_error_files.push(file_path);
            }
            Ok(FormatterFileProcessingResult::Formatted(output)) => {
                if do_check_formatted_only {
                    if use_verbose_output {
                        eprintln!(
                            "Checking {}... {}",
                            output.file_path.display(),
                            if output.is_formatted {
                                "OK"
                            } else {
                                "needs formatting"
                            }
                        );
                    }
                    if !output.is_formatted {
                        all_formatted = false;
                        unformatted_files.push(output.file_path);
                    }
                } else if do_print_to_stdout {
                    terminal_clear_line();
                    eprint!("\r");
                    if total_files > 1 {
                        println!("#--file:{}", output.file_path.display());
                    }
                    print!("{}", output.formatted_content);
                    if use_verbose_output {
                        eprintln!(
                            "Formatting {}... {}",
                            output.file_path.display(),
                            if output.is_formatted {
                                "already formatted"
                            } else {
                                "done"
                            }
                        );
                    }
                } else if !output.is_formatted {
                    fs::write(&output.file_path, output.formatted_content).map_err(|error| {
                        format!(
                            "Failed to write to file {}: {}",
                            output.file_path.display(),
                            error
                        )
                    })?;
                    modified_file_count += 1;
                    if use_verbose_output {
                        eprintln!("Formatting {}... done", output.file_path.display());
                    }
                } else if use_verbose_output {
                    eprintln!(
                        "Formatting {}... already formatted",
                        output.file_path.display()
                    );
                }
            }
            Err(error_msg) => {
                return Err(error_msg.into());
            }
        }
    }

    if !skipped_parse_error_files.is_empty() {
        for file_path in skipped_parse_error_files {
            eprintln!(
                "Skipped formatting file {}: the input GDScript code contains parse errors",
                file_path.display()
            );
        }
        std::process::exit(FormatterExitCodes::ParseErrors as i32);
    }

    if do_check_formatted_only {
        if !use_verbose_output {
            terminal_clear_line();
        }
        let summary_prefix = if use_verbose_output { "" } else { "\r" };
        if all_formatted {
            eprintln!(
                "{}All {} file(s) are formatted",
                summary_prefix, total_files
            );
        } else {
            eprintln!("{}Some files are not formatted", summary_prefix);
            for file_path in unformatted_files {
                eprintln!("{}", file_path.display());
            }
            std::process::exit(FormatterExitCodes::NotFormatted as i32);
        }
    } else if !do_print_to_stdout {
        if !use_verbose_output {
            terminal_clear_line();
        }
        let summary_prefix = if use_verbose_output { "" } else { "\r" };
        if total_files == 1 {
            if modified_file_count > 0 {
                eprintln!(
                    "{}Formatted {}",
                    summary_prefix,
                    input_gdscript_files[0].display()
                );
            } else {
                eprintln!(
                    "{}Already formatted: {}",
                    summary_prefix,
                    input_gdscript_files[0].display()
                );
            }
        } else {
            let already_formatted_count = total_files - modified_file_count;
            if modified_file_count > 0 && already_formatted_count > 0 {
                eprintln!(
                    "{}Formatted {} files, {} already formatted",
                    summary_prefix, modified_file_count, already_formatted_count
                );
            } else if modified_file_count > 0 {
                eprintln!("{}Formatted {} files", summary_prefix, modified_file_count);
            } else {
                eprintln!(
                    "{}All {} files already formatted",
                    summary_prefix, total_files
                );
            }
        }
    }

    Ok(())
}

/// Prints that a command or flag from the linter spec isn't implemented yet and
/// exits with the usage error code.
fn exit_not_implemented(name: &str) -> ! {
    eprintln!("`{}` is not implemented yet", name);
    std::process::exit(2);
}

/// What the `lint` command needs to lint files or stdin.
struct LintCommand {
    input_file_paths: Vec<PathBuf>,
    excluded_paths: Vec<PathBuf>,
    /// Comma-separated rule names from `--disable`.
    disabled_linter_rules: Option<String>,
    /// `--max-line-length`, which overrides every config.
    max_line_length: Option<usize>,
    /// `--config`, which replaces the upward search for `gdguard.toml`.
    config_path: Option<PathBuf>,
    /// `--stdin-filename`: read the source from stdin and report it as this
    /// path, which also finds the config.
    stdin_filename: Option<PathBuf>,
}

/// Where `lint` reads its source from.
enum LintInput {
    Files(Vec<PathBuf>),
    Stdin {
        source: String,
        /// The path used to find `gdguard.toml` and `.editorconfig`.
        lookup_path: PathBuf,
        /// The name shown in output.
        display_name: String,
    },
}

/// Builds the run options and reads the input (files or stdin) for `lint`.
/// Returns an error message for usage, config and I/O errors (exit code 2).
fn prepare_lint_input(command: LintCommand) -> Result<(LintInput, LintRunOptions), String> {
    let mut cli = CliRuleOverrides {
        max_line_length: command.max_line_length,
        ..CliRuleOverrides::default()
    };
    if let Some(disable_str) = command.disabled_linter_rules {
        // Names are checked against the built-in and custom rules when each
        // config loads (`lint_files`), since custom rules come from the config.
        cli.disabled_rules = parse_disabled_rules(&disable_str);
    }
    if let Some(config_path) = &command.config_path
        && !config_path.is_file()
    {
        return Err(format!("--config: {} is not a file", config_path.display()));
    }
    let options = LintRunOptions {
        config_path: command.config_path,
        cli,
    };

    let is_dash_only =
        command.input_file_paths.len() == 1 && command.input_file_paths[0] == Path::new("-");
    if let Some(stdin_filename) = command.stdin_filename {
        if !command.input_file_paths.is_empty() && !is_dash_only {
            return Err(
                "--stdin-filename reads from stdin and can't be used with file paths".to_string(),
            );
        }
        let source = read_stdin()?;
        let current_directory = env::current_dir()
            .map_err(|error| format!("Failed to get current directory: {}", error))?;
        let lookup_path = current_directory.join(&stdin_filename);
        let display_name = gdscript_formatter::linter::output::display_path(&stdin_filename);
        let input = LintInput::Stdin {
            source,
            lookup_path,
            display_name,
        };
        return Ok((input, options));
    }

    let is_stdin_requested = command.input_file_paths.is_empty() || is_dash_only;
    if is_stdin_requested && !io::stdin().is_terminal() {
        let source = read_stdin()?;
        let current_directory = env::current_dir()
            .map_err(|error| format!("Failed to get current directory: {}", error))?;
        // Like the formatter, stdin uses a `stdin.gd` file in the current
        // directory to find `gdguard.toml` and `.editorconfig`.
        let input = LintInput::Stdin {
            source,
            lookup_path: current_directory.join("stdin.gd"),
            display_name: "stdin".to_string(),
        };
        return Ok((input, options));
    }

    let input_gdscript_files =
        find_gdscript_files(&command.input_file_paths, &command.excluded_paths)
            .map_err(|error| error.to_string())?;
    if input_gdscript_files.is_empty() {
        return Err(NO_GDSCRIPT_FILES_MESSAGE.to_string());
    }
    Ok((LintInput::Files(input_gdscript_files), options))
}

fn read_stdin() -> Result<String, String> {
    let mut input_content = String::new();
    io::stdin()
        .read_to_string(&mut input_content)
        .map_err(|error| format!("Failed to read from stdin: {}", error))?;
    Ok(input_content)
}

/// Options of `lint --fix`, `--fix-unsafe` and `--diff`.
struct FixCommand {
    /// `--fix-unsafe`: apply `Unsafe` fixes too.
    allow_unsafe: bool,
    /// `--diff`: print a unified diff and write nothing.
    diff_only: bool,
    format: OutputFormat,
    is_terminal: bool,
    max_warnings: Option<usize>,
    /// The baseline to hide issues with, and its root folder.
    baseline: Option<(Baseline, PathBuf)>,
    current_directory: PathBuf,
}

/// Loads the baseline for a lint or fix run, with the folder its paths are
/// relative to.
fn load_run_baseline(
    target: Option<BaselineTarget>,
) -> Result<Option<(Baseline, PathBuf)>, String> {
    let Some(target) = target else {
        return Ok(None);
    };
    let baseline = load_target(&target)?;
    Ok(Some((baseline, target.root)))
}

/// `lint --update-baseline`: lints, writes every issue to the baseline file
/// and returns the exit code (0, or 2 on errors).
fn run_update_baseline(
    lint_command: LintCommand,
    target: Option<BaselineTarget>,
    current_directory: &Path,
) -> i32 {
    let Some(target) = target else {
        eprintln!("Error: no baseline path");
        return 2;
    };
    let report = match run_linter(lint_command) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 2;
        }
    };
    let entries = baseline_entries_for_report(&report, current_directory, &target.root);
    let entry_count = entries.len();
    if let Err(error) = write_baseline(&target.path, entries) {
        eprintln!("Error: {}", error);
        return 2;
    }
    let shown_path = match target.path.strip_prefix(current_directory) {
        Ok(relative) => display_path(relative),
        Err(_) => display_path(&target.path),
    };
    let issue_word = if entry_count == 1 { "issue" } else { "issues" };
    println!("Wrote {} {} to {}", entry_count, issue_word, shown_path);
    0
}

/// Runs the fix engine and returns the exit code.
///
/// - `--diff` prints a unified diff per changed file and writes nothing.
///   Exit code 1 when a file would change, else 0.
/// - `--fix`/`--fix-unsafe` on files write them, report what's left, and
///   print "Fixed N problems in M files".
/// - With stdin, the fixed source goes to stdout and the report to stderr.
///
/// The exit code of a fix run follows the issues left after fixing.
fn run_fix_command(lint_command: LintCommand, fix_command: &FixCommand) -> i32 {
    let mut fix_report = match run_fixer(lint_command, fix_command.allow_unsafe) {
        Ok(fix_report) => fix_report,
        Err(error) => {
            eprintln!("Error: {}", error);
            return 2;
        }
    };
    if let Some((baseline, root)) = &fix_command.baseline {
        fix_report.report.baseline_hidden = apply_baseline(
            baseline,
            &mut fix_report.report,
            &fix_command.current_directory,
            root,
        );
    }

    if fix_command.diff_only {
        let mut diff_output = String::new();
        for changed_file in &fix_report.changed_files {
            diff_output.push_str(&unified_diff(
                &changed_file.display_path,
                &changed_file.original,
                &changed_file.fixed,
            ));
        }
        print!("{}", diff_output);
        if fix_report.changed_files.is_empty() {
            return 0;
        }
        return 1;
    }

    let is_stdin = fix_report.stdin_output.is_some();
    if let Some(stdin_output) = &fix_report.stdin_output {
        print!("{}", stdin_output);
    } else {
        for changed_file in &fix_report.changed_files {
            if let Err(error) = fs::write(&changed_file.path, &changed_file.fixed) {
                eprintln!(
                    "Error: Failed to write file {}: {}",
                    changed_file.path.display(),
                    error
                );
                return 2;
            }
        }
    }

    // Stdout carries the fixed source for stdin, so the report goes to
    // stderr there.
    let with_colors = if is_stdin {
        false
    } else {
        fix_command.is_terminal
    };
    let mut output = String::new();
    if let Err(error) = write_report(
        fix_command.format,
        &fix_report.report,
        with_colors,
        &mut output,
    ) {
        eprintln!("Error: {}", error);
        return 2;
    }
    let fixed_line = fixed_summary_line(fix_report.applied(), fix_report.changed_files.len());
    // Machine-readable formats keep stdout parseable.
    let is_machine_format = matches!(
        fix_command.format,
        OutputFormat::Json | OutputFormat::Github | OutputFormat::Sarif
    );
    if is_stdin {
        eprint!("{}", output);
        eprintln!("{}", fixed_line);
    } else if is_machine_format {
        print!("{}", output);
        eprintln!("{}", fixed_line);
    } else {
        print!("{}", output);
        println!("{}", fixed_line);
    }
    fix_report
        .report
        .summary()
        .exit_code(fix_command.max_warnings)
}

/// "Fixed 3 problems in 2 files", with singular forms for 1.
fn fixed_summary_line(applied: usize, file_count: usize) -> String {
    let problem_word = if applied == 1 { "problem" } else { "problems" };
    let file_word = if file_count == 1 { "file" } else { "files" };
    format!(
        "Fixed {} {} in {} {}",
        applied, problem_word, file_count, file_word
    )
}

/// Runs the fix engine on the files (or stdin) without writing anything.
fn run_fixer(command: LintCommand, allow_unsafe: bool) -> Result<FixRunReport, String> {
    let (input, options) = prepare_lint_input(command)?;
    match input {
        LintInput::Files(files) => fix_files(&files, &options, allow_unsafe),
        LintInput::Stdin {
            source,
            lookup_path,
            display_name,
        } => fix_stdin(source, &lookup_path, &display_name, &options, allow_unsafe),
    }
}

/// Lints the files (or stdin) and returns the report. Returns an error
/// message for usage, config and I/O errors (exit code 2).
fn run_linter(command: LintCommand) -> Result<LintReport, String> {
    let (input, options) = prepare_lint_input(command)?;
    match input {
        LintInput::Files(files) => lint_files(&files, &options),
        LintInput::Stdin {
            source,
            lookup_path,
            display_name,
        } => lint_stdin(source, &lookup_path, &display_name, &options),
    }
}

/// Formats one file after applying its matching editorconfig settings.
///
/// Different files can fall under different editorconfig settings so we clone
/// the base configuration and apply file-specific settings individually.
/// CLI options override editorconfig are applied last by [`apply_file_config`].
fn format_one_file(
    index: usize,
    file_path: &PathBuf,
    config: &FormatterConfiguration,
    config_overrides: FormatterConfigOverrides,
    render_elements: &mut Vec<RenderElement>,
    output: &mut String,
) -> Result<FormatterFileProcessingResult, String> {
    let input_content = fs::read_to_string(file_path)
        .map_err(|error| format!("Failed to read file {}: {}", file_path.display(), error))?;

    // We need to clone that config because files in nested directories can
    // match different EditorConfig files and rules.
    let file_config = get_final_config_applied_to_script(config, file_path, config_overrides);
    match format_gdscript_with_buffers(&input_content, &file_config, render_elements, output) {
        Ok(()) => {}
        Err(FormatErrors::ParseErrors) => {
            return Ok(FormatterFileProcessingResult::SkippedParseErrors {
                index,
                file_path: file_path.clone(),
            });
        }
        Err(error) => {
            return Err(format!(
                "Failed to format file {}: {}",
                file_path.display(),
                error
            ));
        }
    }

    let is_formatted = input_content == *output;

    Ok(FormatterFileProcessingResult::Formatted(FormatterOutput {
        index,
        file_path: file_path.clone(),
        formatted_content: output.clone(),
        is_formatted,
    }))
}

/// Builds the final configuration applied to one file by applying the
/// editorconfig settings first and CLI settings second.
///
/// The override fields are `Option`s because `None` means that the user did
/// not pass that flag. Without this distinction, a CLI default such as an
/// indentation size of four would look like an explicit request and would
/// incorrectly override `.editorconfig`.
fn get_final_config_applied_to_script(
    config: &FormatterConfiguration,
    file_path: &Path,
    config_overrides: FormatterConfigOverrides,
) -> FormatterConfiguration {
    let mut file_config = config.clone();
    gdscript_formatter::editorconfig::apply_editorconfig_to_formatter_config(
        &mut file_config,
        file_path,
    );
    if let Some(use_spaces) = config_overrides.use_spaces {
        file_config.printer.use_spaces = use_spaces;
    }
    if let Some(indent_size) = config_overrides.indent_size {
        file_config.printer.indent_size = indent_size;
    }
    if let Some(max_line_length) = config_overrides.max_line_length {
        file_config.printer.max_line_length = max_line_length;
    }
    if let Some(blank_lines_around_definitions) = config_overrides.blank_lines_around_definitions {
        file_config.blank_lines_around_definitions = blank_lines_around_definitions;
    }
    if let Some(continuation_indent_level) = config_overrides.continuation_indent_level {
        file_config.printer.continuation_indent_level = continuation_indent_level;
    }
    if let Some(quote_style) = config_overrides.quote_style {
        file_config.quote_style = quote_style;
    }
    file_config
}

fn get_formatted_configuration_as_string(
    file_path: &Path,
    config: &FormatterConfiguration,
) -> String {
    let indentation_style = if config.printer.use_spaces {
        format!("spaces (size {})", config.printer.indent_size)
    } else {
        "tabs".to_string()
    };
    let quote_style = match config.quote_style {
        QuoteStyle::Preserve => "preserve",
        QuoteStyle::Single => "single",
        QuoteStyle::Double => "double",
    };

    format!(
        "Debug: {}\n  max_line_length: {}\n  indentation: {}\n  blank_lines_around_definitions: {}\n  continuation_indent_level: {}\n  quote_style: {}\n  insert_final_newline: {}\n  trim_trailing_whitespace: {}\n  indent_blank_lines: {}\n  reorder_code: {}\n  verify_structure: {}",
        file_path.display(),
        config.printer.max_line_length,
        indentation_style,
        config.blank_lines_around_definitions,
        config.printer.continuation_indent_level,
        quote_style,
        config.printer.insert_final_newline,
        config.printer.trim_trailing_whitespace,
        config.printer.indent_blank_lines,
        config.reorder_code,
        config.safe,
    )
}

/// Formats files concurrently but preserves their original order when
/// outputting the results.
fn format_files_parallel(
    files: &[PathBuf],
    config: &FormatterConfiguration,
    config_overrides: FormatterConfigOverrides,
) -> Vec<Result<FormatterFileProcessingResult, String>> {
    if files.is_empty() {
        return Vec::new();
    }

    let hardware_threads = match thread::available_parallelism() {
        Ok(n) => n.get(),
        Err(_) => 1,
    };
    let thread_count = hardware_threads.min(files.len());
    let chunk_size = files.len().div_ceil(thread_count);

    thread::scope(|scope| {
        let mut handles = Vec::with_capacity(thread_count);
        for (chunk_index, chunk) in files.chunks(chunk_size).enumerate() {
            let handle = scope.spawn(move || {
                format_chunk(chunk, chunk_index, chunk_size, config, config_overrides)
            });
            handles.push(handle);
        }

        let mut all = Vec::with_capacity(files.len());
        for handle in handles {
            all.extend(handle.join().expect("worker thread panicked"));
        }
        all
    })
}

fn format_chunk(
    chunk: &[PathBuf],
    chunk_index: usize,
    chunk_size: usize,
    config: &FormatterConfiguration,
    config_overrides: FormatterConfigOverrides,
) -> Vec<Result<FormatterFileProcessingResult, String>> {
    let mut results = Vec::with_capacity(chunk.len());
    let mut render_elements: Vec<RenderElement> = Vec::new();
    let mut output = String::new();
    for (local_index, file_path) in chunk.iter().enumerate() {
        let global_index = chunk_index * chunk_size + local_index;
        results.push(format_one_file(
            global_index,
            file_path,
            config,
            config_overrides,
            &mut render_elements,
            &mut output,
        ));
    }
    results
}

const NO_GDSCRIPT_FILES_MESSAGE: &str = "No GDScript files found in the arguments provided. Please provide at least one .gd file or directory containing .gd files.";

/// Finds the `.gd` files in `input_paths`, sorted. Returns an empty list when
/// there are none; each command decides how to report that.
fn find_gdscript_files(
    input_paths: &[PathBuf],
    excluded_paths: &[PathBuf],
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut gdscript_file_paths = Vec::new();
    let mut paths_to_check: Vec<PathBuf> = Vec::with_capacity(input_paths.len());
    for path in input_paths {
        paths_to_check.push(path.to_path_buf());
    }

    while let Some(current_path) = paths_to_check.pop() {
        if is_path_excluded(&current_path, excluded_paths) {
            continue;
        }
        if current_path.is_dir() {
            let entries = fs::read_dir(&current_path).map_err(|error| {
                format!(
                    "Failed to read directory {}: {}",
                    current_path.display(),
                    error
                )
            })?;
            for entry in entries {
                let entry = entry.map_err(|error| {
                    format!(
                        "Failed to read entry in {}: {}",
                        current_path.display(),
                        error
                    )
                })?;
                if entry.path().is_dir() {
                    paths_to_check.push(entry.path());
                } else if let Some(extension) = entry.path().extension() {
                    if extension == "gd" {
                        let file_path = entry.path();
                        if !is_path_excluded(&file_path, excluded_paths)
                            && !gdscript_formatter::editorconfig::is_excluded_by_editorconfig(
                                &file_path,
                            )
                        {
                            gdscript_file_paths.push(file_path);
                        }
                    }
                }
            }
        } else if let Some(extension) = current_path.extension() {
            if extension == "gd"
                && !gdscript_formatter::editorconfig::is_excluded_by_editorconfig(&current_path)
            {
                gdscript_file_paths.push(current_path);
            }
        }
    }

    gdscript_file_paths.sort();
    gdscript_file_paths.dedup();
    Ok(gdscript_file_paths)
}

fn is_path_excluded(path: &Path, excluded_paths: &[PathBuf]) -> bool {
    let current_directory = env::current_dir().expect("Failed to get current directory");
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        current_directory.join(path)
    };
    excluded_paths.iter().any(|exclude_path| {
        let absolute_exclude_path = if exclude_path.is_absolute() {
            exclude_path.clone()
        } else {
            current_directory.join(exclude_path)
        };
        absolute_path.starts_with(absolute_exclude_path)
    })
}

fn compare_output_index(
    left: &Result<FormatterFileProcessingResult, String>,
    right: &Result<FormatterFileProcessingResult, String>,
) -> std::cmp::Ordering {
    let left_index = match left {
        Ok(FormatterFileProcessingResult::Formatted(formatter_output)) => formatter_output.index,
        Ok(FormatterFileProcessingResult::SkippedParseErrors { index, .. }) => *index,
        Err(_) => usize::MAX,
    };
    let right_index = match right {
        Ok(FormatterFileProcessingResult::Formatted(formatter_output)) => formatter_output.index,
        Ok(FormatterFileProcessingResult::SkippedParseErrors { index, .. }) => *index,
        Err(_) => usize::MAX,
    };
    left_index.cmp(&right_index)
}

fn terminal_clear_line() {
    eprint!("\r{}", " ".repeat(80));
}
