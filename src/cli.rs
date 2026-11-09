//! Command-line argument parser for the GDScript formatter.
//!
//! At the time of writing, the linter program that ships with the formatter and
//! runs as a sub-command. It may be moved to a dedicated program in the future.
//!
//! NB: do not replace with a dependency like clap: it brings too many
//! dependencies only to save a little straightforward code.
use gdguard::QuoteStyle;
use gdguard::linter::explain::{ExplainArgs, ListRulesFormat};
use gdguard::linter::init::InitArgs;
use gdguard::linter::output::OutputFormat;
use gdguard::linter::rule_tester::TestRulesArgs;
use std::path::PathBuf;

const HELP_FORMATTER: &str = "\
	GDScript linting, reviewed fixes and formatting for CLI and AI agents.

	Usage: gdguard [OPTIONS] [FILES]...
	       gdguard lint [OPTIONS] [FILES]...

	Arguments:
	  <FILES>...  GDScript files or directories to format. If empty, uses
	              the current directory. Reads from stdin when piped.

	Options:
	  -c, --check                                Check if files are formatted, exit 1 if not
	  -x, --exclude <PATH>                       Exclude one file or directory (you can repeat this option multiple times)
	      --verify-structure                     Verify formatted output has the same structure as the input
	      --stdout                               Write to stdout instead of overwriting files
	-v, --verbose                              Print one status line for each processed file
	-d, --debug                                Print effective settings for each processed file
	      --use-spaces                           Use spaces instead of tabs for indentation
	      --indent-size <NUM>                    Spaces per indent level (default: 4)
	      --reorder-code                         Reorder code to match the style guide
	      --max-line-length <NUM>                Maximum line length before wrapping (default: 100)
	      --blank-lines-around-definitions <NUM> Blank lines between top-level definitions (default: 2)
	      --continuation-indent-level <NUM>      Extra indent for line continuations (default: 2)
	      --quote-style <STYLE>                  String quotes: preserve, single, or double (default: preserve)
	  -h, --help                                 Print help
	  -V, --version                              Print version

	Subcommands:
	  lint                     Lint GDScript files and project issues
	  explain <RULE>           Explain a lint rule, with examples
	  test-rules [RULE_DIR]    Run the tests of custom lint rules
	  init                     Write a starter gdguard.toml and .gdguard/ folder
	  lsp                      Start the linter language server

	Run 'gdguard lint --help' for lint options.
";

const HELP_LINTER: &str = "\
Lint GDScript syntax, style, semantic and project issues.

Usage: gdguard lint [OPTIONS] [FILES]...

Arguments:
  <FILES>...                 GDScript files or directories to lint

Options:
  -x, --exclude <PATH>         Exclude a file or directory (may be repeated)
      --disable <RULES>        Disable specific rules (comma-separated)
      --max-line-length <NUM>  Maximum line length allowed (default: 100)
      --list-rules             List all available linting rules (--format text|json)
      --pretty                 Same as --format pretty
      --fix                    Apply safe fixes
      --fix-unsafe             Apply safe and unsafe fixes
      --diff                   Print the fixes as a unified diff, don't write files
      --format <FORMAT>        Output format: pretty, compact, json, github, sarif,
                               agent (default: pretty on a terminal, compact otherwise)
      --config <PATH>          Use this gdguard.toml instead of searching for one
      --baseline <PATH>        Hide the issues listed in this baseline file
      --update-baseline        Write all current issues to the baseline file
      --no-cache               Lint every file, don't read or write .gdguard/cache
      --max-warnings <NUM>     Fail when there are more than NUM warnings
      --stdin-filename <PATH>  Read the source from stdin, report it as PATH
      --godot <PATH>           Also load the files in headless Godot 4 and report
                               its errors as godot-error (default: $GDGUARD_GODOT)
  -h, --help                   Print help

Exit codes: 0 no problems, 1 lint problems found, 2 usage, config or rule-file error.
";

const HELP_EXPLAIN: &str = "\
Explain a lint rule: rationale, examples and whether it can be fixed.

Usage: gdguard explain <RULE>
";

const HELP_TEST_RULES: &str = "\
Run the tests written inside custom rule files.

Usage: gdguard test-rules [RULE_DIR]

Arguments:
  [RULE_DIR]  Folder of rule files (default: the custom rule paths from
              gdguard.toml, or .gdguard/rules)
";

const HELP_INIT: &str = "\
Write a starter gdguard.toml and .gdguard/rules/ folder.

Usage: gdguard init [OPTIONS]

Options:
      --force  Overwrite existing files
  -h, --help   Print help
";

const HELP_LSP: &str = "\
Start the linter language server on stdin and stdout.

Usage: gdguard lsp
";

/// Represents the parsed command-line arguments for the GDScript formatter. You
/// can use exactly one command: currently, Format (the default) or Lint.
pub struct CliArguments {
    /// List of input file paths or directories to process.
    pub input_file_paths: Vec<PathBuf>,
    /// Files or directories to skip during discovery.
    pub excluded_paths: Vec<PathBuf>,
    /// Which command to run.
    pub command: Command,
}

/// The command selected by the user on the command line, and its associated
/// arguments.
pub enum Command {
    /// Format GDScript files according to the official style guide.
    Format {
        /// If true, prints the formatted output to stdout instead of writing to
        /// files.
        do_print_to_stdout: bool,
        /// If true, prints one status line for each processed file.
        use_verbose_output: bool,
        /// If true, prints the effective formatter settings for each processed file.
        use_debug_output: bool,
        /// If true, only checks if the files are formatted, without modifying
        /// them. Returns error code ERROR_CODE_NOT_FORMATTED if any of the input
        /// files are not formatted.
        do_check_formatted_only: bool,
        /// If set, uses spaces for indentation instead of tabs.
        use_spaces: Option<bool>,
        /// Number of spaces to use for indentation.
        indent_size: Option<usize>,
        /// If true, the formatter will re-parse the formatted code and verify it
        /// has the same structure as the original before writing it to files.
        use_verify_structure: bool,
        /// If true, the formatter will reorder code to follow the official
        /// GDScript style guide's recommended order of code elements.
        do_reorder_code: bool,
        /// Maximum line length before wrapping. If not specified, uses an
        /// editorconfig value, or falls back to a default of 100 characters.
        max_line_length: Option<usize>,
        /// Number of blank lines between top-level definitions (functions,
        /// inner classes). 2 by default.
        blank_lines_around_definitions: Option<u16>,
        /// Extra indent level for continuation lines.
        continuation_indent_level: Option<u16>,
        /// If set to `single` or `double`, the formatter will try to use that
        /// quote style for strings.
        quote_style: Option<QuoteStyle>,
    },
    /// Lint GDScript syntax, style, semantic and project issues.
    Lint {
        /// Optional list of comma-separated linter rule names to disable.
        disabled_linter_rules: Option<String>,
        /// Maximum line length for the linter.
        max_line_length: Option<usize>,
        /// If true, the linter program will list all available linting rules and
        /// exit.
        do_list_rules: bool,
        /// If true, the linter program will print the list of available linting
        /// rules in a human-readable format.
        do_pretty_print: bool,
        /// `--format` value when used with `--list-rules`.
        list_rules_format: ListRulesFormat,
        /// `--fix`: apply safe fixes.
        do_fix: bool,
        /// `--fix-unsafe`: apply safe and unsafe fixes.
        do_fix_unsafe: bool,
        /// `--diff`: print fixes as a unified diff instead of writing files.
        do_diff: bool,
        /// `--format`. `None` means: pretty on a TTY, compact otherwise.
        output_format: Option<OutputFormat>,
        /// `--config`: use this config file instead of searching for one.
        config_path: Option<PathBuf>,
        /// `--baseline`: hide the issues in this baseline file.
        baseline_path: Option<PathBuf>,
        /// `--update-baseline`: write all current issues to the baseline file.
        do_update_baseline: bool,
        /// `--no-cache`: don't read or write the lint cache.
        no_cache: bool,
        /// `--max-warnings`: warnings only fail when there are more than this.
        max_warnings: Option<usize>,
        /// `--stdin-filename`: read the source from stdin and use this path
        /// for config lookup and output.
        stdin_filename: Option<PathBuf>,
        /// `--godot`: the Godot 4 executable for the optional Godot check.
        godot_path: Option<PathBuf>,
    },
    /// Explain one lint rule. Implemented by `linter::explain::run`.
    Explain(ExplainArgs),
    /// Run the tests of custom rules. Implemented by `linter::rule_tester::run`.
    TestRules(TestRulesArgs),
    /// Write a starter config. Implemented by `linter::init::run`.
    Init(InitArgs),
    /// Start the language server.
    Lsp,
}

/// Internal discriminator used during parsing to track which command's flags
/// we are currently parsing.
enum ActiveCommand {
    Format,
    Lint,
}

pub fn parse_args() -> CliArguments {
    let argument_list: Vec<String> = std::env::args().collect();
    let mut active_command = ActiveCommand::Format;

    let mut input_file_paths: Vec<PathBuf> = Vec::new();
    let mut excluded_paths: Vec<PathBuf> = Vec::new();
    let mut format_do_print_to_stdout = false;
    let mut format_use_verbose_output = false;
    let mut format_use_debug_output = false;
    let mut format_do_check_formatted_only = false;
    let mut format_use_spaces: Option<bool> = None;
    let mut format_indent_size: Option<usize> = None;
    let mut format_use_verify_structure = false;
    let mut format_do_reorder_code = false;
    let mut format_max_line_length: Option<usize> = None;
    let mut format_blank_lines_around_definitions: Option<u16> = None;
    let mut format_continuation_indent_level: Option<u16> = None;
    let mut format_quote_style: Option<QuoteStyle> = None;

    let mut lint_disabled_rules: Option<String> = None;
    let mut lint_max_line_length: Option<usize> = None;
    let mut lint_list_rules = false;
    let mut lint_pretty_print = false;
    let mut lint_do_fix = false;
    let mut lint_do_fix_unsafe = false;
    let mut lint_do_diff = false;
    let mut lint_format_name: Option<String> = None;
    let mut lint_config_path: Option<PathBuf> = None;
    let mut lint_baseline_path: Option<PathBuf> = None;
    let mut lint_do_update_baseline = false;
    let mut lint_no_cache = false;
    let mut lint_max_warnings: Option<usize> = None;
    let mut lint_stdin_filename: Option<PathBuf> = None;
    let mut lint_godot_path: Option<PathBuf> = None;

    // The first positional argument optionally selects a command. If it is
    // "lint", we run the linter program. Defaults to the formatter.
    let mut current_argument_index = 1;
    if argument_list.len() > 1 {
        match argument_list[1].as_str() {
            "lint" => {
                active_command = ActiveCommand::Lint;
                current_argument_index = 2;
            }
            "explain" | "test-rules" | "init" | "lsp" => {
                return parse_tool_subcommand(&argument_list[1], &argument_list[2..]);
            }
            _ => {}
        }
    }

    while current_argument_index < argument_list.len() {
        let current_argument = argument_list[current_argument_index].as_str();

        // -- stops option parsing. Everything after it is a file path.
        if current_argument == "--" {
            current_argument_index += 1;
            while current_argument_index < argument_list.len() {
                let file_path = argument_list[current_argument_index].as_str();
                input_file_paths.push(PathBuf::from(file_path));
                current_argument_index += 1;
            }
            break;
        }

        if current_argument == "--help" || current_argument == "-h" {
            match active_command {
                ActiveCommand::Format => print!("{}", HELP_FORMATTER),
                ActiveCommand::Lint => print!("{}", HELP_LINTER),
            }
            std::process::exit(0);
        }

        if current_argument == "--version" || current_argument == "-V" {
            println!("gdguard {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }

        if let Some(flag_without_prefix) = current_argument.strip_prefix("--") {
            let (flag_name, assigned_value) = split_flag_and_value(flag_without_prefix);
            match active_command {
                ActiveCommand::Format => match flag_name {
                    "exclude" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--exclude",
                        );
                        excluded_paths.push(PathBuf::from(value));
                    }
                    "stdout" => {
                        require_no_value(assigned_value, "--stdout");
                        format_do_print_to_stdout = true;
                    }
                    "verbose" => {
                        require_no_value(assigned_value, "--verbose");
                        format_use_verbose_output = true;
                    }
                    "debug" => {
                        require_no_value(assigned_value, "--debug");
                        format_use_debug_output = true;
                    }
                    "check" => {
                        require_no_value(assigned_value, "--check");
                        format_do_check_formatted_only = true;
                    }
                    "use-spaces" => {
                        require_no_value(assigned_value, "--use-spaces");
                        format_use_spaces = Some(true);
                    }
                    "verify-structure" => {
                        require_no_value(assigned_value, "--verify-structure");
                        format_use_verify_structure = true;
                    }
                    "safe" => {
                        // DEPRECATED: We keep this flag for anyone who already
                        // used it as part of a CI or of their workflow. But
                        // it's now replaced by --verify-structure, which is a
                        // more accurate name for this mode.
                        require_no_value(assigned_value, "--safe");
                        format_use_verify_structure = true;
                    }
                    "reorder-code" => {
                        require_no_value(assigned_value, "--reorder-code");
                        format_do_reorder_code = true;
                    }
                    "indent-size" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--indent-size",
                        );
                        format_indent_size = match value.parse::<usize>() {
                            Ok(n) => Some(n),
                            Err(_) => print_error_invalid_argument(&format!(
                                "--indent-size expects a number, got '{}'",
                                value
                            )),
                        };
                    }
                    "max-line-length" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--max-line-length",
                        );
                        format_max_line_length = match value.parse::<usize>() {
                            Ok(n) => Some(n),
                            Err(_) => print_error_invalid_argument(&format!(
                                "--max-line-length expects a number, got '{}'",
                                value
                            )),
                        };
                    }
                    "blank-lines-around-definitions" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--blank-lines-around-definitions",
                        );
                        format_blank_lines_around_definitions = match value.parse::<u16>() {
                            Ok(n) => Some(n),
                            Err(_) => print_error_invalid_argument(&format!(
                                "--blank-lines-around-definitions expects a number, got '{}'",
                                value
                            )),
                        };
                    }
                    "continuation-indent-level" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--continuation-indent-level",
                        );
                        format_continuation_indent_level = match value.parse::<u16>() {
                            Ok(n) => Some(n),
                            Err(_) => print_error_invalid_argument(&format!(
                                "--continuation-indent-level expects a number, got '{}'",
                                value
                            )),
                        };
                    }
                    "quote-style" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--quote-style",
                        );
                        format_quote_style = match QuoteStyle::from_name(&value) {
                            Some(quote_style) => Some(quote_style),
                            None => print_error_invalid_argument(&format!(
                                "--quote-style expects preserve, single, or double, got '{}'",
                                value
                            )),
                        };
                    }
                    _ => print_error_invalid_argument(&format!(
                        "unexpected argument '--{}'",
                        flag_name
                    )),
                },
                ActiveCommand::Lint => match flag_name {
                    "exclude" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--exclude",
                        );
                        excluded_paths.push(PathBuf::from(value));
                    }
                    "disable" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--disable",
                        );
                        lint_disabled_rules = Some(value);
                    }
                    "max-line-length" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--max-line-length",
                        );
                        lint_max_line_length = match value.parse::<usize>() {
                            Ok(n) => Some(n),
                            Err(_) => print_error_invalid_argument(&format!(
                                "--max-line-length expects a number, got '{}'",
                                value
                            )),
                        };
                    }
                    "list-rules" => {
                        require_no_value(assigned_value, "--list-rules");
                        lint_list_rules = true;
                    }
                    "pretty" => {
                        require_no_value(assigned_value, "--pretty");
                        lint_pretty_print = true;
                    }
                    "fix" => {
                        require_no_value(assigned_value, "--fix");
                        lint_do_fix = true;
                    }
                    "fix-unsafe" => {
                        require_no_value(assigned_value, "--fix-unsafe");
                        lint_do_fix_unsafe = true;
                    }
                    "diff" => {
                        require_no_value(assigned_value, "--diff");
                        lint_do_diff = true;
                    }
                    "format" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--format",
                        );
                        lint_format_name = Some(value);
                    }
                    "config" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--config",
                        );
                        lint_config_path = Some(PathBuf::from(value));
                    }
                    "baseline" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--baseline",
                        );
                        lint_baseline_path = Some(PathBuf::from(value));
                    }
                    "update-baseline" => {
                        require_no_value(assigned_value, "--update-baseline");
                        lint_do_update_baseline = true;
                    }
                    "no-cache" => {
                        require_no_value(assigned_value, "--no-cache");
                        lint_no_cache = true;
                    }
                    "max-warnings" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--max-warnings",
                        );
                        lint_max_warnings = match value.parse::<usize>() {
                            Ok(n) => Some(n),
                            Err(_) => print_error_invalid_argument(&format!(
                                "--max-warnings expects a number, got '{}'",
                                value
                            )),
                        };
                    }
                    "stdin-filename" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--stdin-filename",
                        );
                        lint_stdin_filename = Some(PathBuf::from(value));
                    }
                    "godot" => {
                        let value = consume_flag_value(
                            assigned_value,
                            &argument_list,
                            &mut current_argument_index,
                            "--godot",
                        );
                        lint_godot_path = Some(PathBuf::from(value));
                    }
                    _ => print_error_invalid_argument(&format!(
                        "unexpected argument '--{}'",
                        flag_name
                    )),
                },
            }
        } else if current_argument.starts_with('-') && current_argument.len() > 1 {
            let short_flags = &current_argument[1..];
            if short_flags == "x" {
                let value = consume_flag_value(
                    None,
                    &argument_list,
                    &mut current_argument_index,
                    "-x/--exclude",
                );
                excluded_paths.push(PathBuf::from(value));
                current_argument_index += 1;
                continue;
            }
            for flag_char in short_flags.chars() {
                match flag_char {
                    'c' => {
                        if matches!(active_command, ActiveCommand::Lint) {
                            print_error_invalid_argument("unexpected argument '-c'");
                        }
                        format_do_check_formatted_only = true;
                    }
                    's' => {
                        if matches!(active_command, ActiveCommand::Lint) {
                            print_error_invalid_argument("unexpected argument '-s'");
                        }
                        format_use_verify_structure = true;
                    }
                    'v' => {
                        if matches!(active_command, ActiveCommand::Lint) {
                            print_error_invalid_argument("unexpected argument '-v'");
                        }
                        format_use_verbose_output = true;
                    }
                    'd' => {
                        if matches!(active_command, ActiveCommand::Lint) {
                            print_error_invalid_argument("unexpected argument '-d'");
                        }
                        format_use_debug_output = true;
                    }
                    _ => print_error_invalid_argument(&format!(
                        "unexpected argument '-{}'",
                        flag_char
                    )),
                }
            }
        } else {
            input_file_paths.push(PathBuf::from(current_argument));
        }
        current_argument_index += 1;
    }

    match active_command {
        ActiveCommand::Format => CliArguments {
            input_file_paths,
            excluded_paths,
            command: Command::Format {
                do_print_to_stdout: format_do_print_to_stdout,
                use_verbose_output: format_use_verbose_output,
                use_debug_output: format_use_debug_output,
                do_check_formatted_only: format_do_check_formatted_only,
                use_spaces: format_use_spaces,
                indent_size: format_indent_size,
                use_verify_structure: format_use_verify_structure,
                do_reorder_code: format_do_reorder_code,
                max_line_length: format_max_line_length,
                blank_lines_around_definitions: format_blank_lines_around_definitions,
                continuation_indent_level: format_continuation_indent_level,
                quote_style: format_quote_style,
            },
        },
        ActiveCommand::Lint => {
            // `--list-rules` accepts `text` or `json`; linting accepts the
            // reporter names. We check after parsing because the flags can
            // come in any order.
            let mut list_rules_format = ListRulesFormat::Text;
            let mut output_format: Option<OutputFormat> = None;
            if let Some(format_name) = lint_format_name {
                if lint_list_rules {
                    list_rules_format = match format_name.as_str() {
                        "text" => ListRulesFormat::Text,
                        "json" => ListRulesFormat::Json,
                        _ => print_error_invalid_argument(&format!(
                            "--format with --list-rules expects text or json, got '{}'",
                            format_name
                        )),
                    };
                } else {
                    output_format = match OutputFormat::from_name(&format_name) {
                        Some(format) => Some(format),
                        None => print_error_invalid_argument(&format!(
                            "--format expects pretty, compact, json, github, sarif or agent, got '{}'",
                            format_name
                        )),
                    };
                }
            }
            CliArguments {
                input_file_paths,
                excluded_paths,
                command: Command::Lint {
                    disabled_linter_rules: lint_disabled_rules,
                    max_line_length: lint_max_line_length,
                    do_list_rules: lint_list_rules,
                    do_pretty_print: lint_pretty_print,
                    list_rules_format,
                    do_fix: lint_do_fix,
                    do_fix_unsafe: lint_do_fix_unsafe,
                    do_diff: lint_do_diff,
                    output_format,
                    config_path: lint_config_path,
                    baseline_path: lint_baseline_path,
                    do_update_baseline: lint_do_update_baseline,
                    no_cache: lint_no_cache,
                    max_warnings: lint_max_warnings,
                    stdin_filename: lint_stdin_filename,
                    godot_path: lint_godot_path,
                },
            }
        }
    }
}

/// Parses the arguments of the linter tool subcommands (`explain`,
/// `test-rules`, `init`, `lsp`). They take few arguments, so they have their
/// own small loop instead of sharing the formatter and lint flag parser.
fn parse_tool_subcommand(subcommand_name: &str, arguments: &[String]) -> CliArguments {
    let help_text = match subcommand_name {
        "explain" => HELP_EXPLAIN,
        "test-rules" => HELP_TEST_RULES,
        "init" => HELP_INIT,
        _ => HELP_LSP,
    };

    let mut positional_arguments: Vec<&str> = Vec::with_capacity(arguments.len());
    let mut do_force = false;
    let mut current_argument_index = 0;
    while current_argument_index < arguments.len() {
        let current_argument = arguments[current_argument_index].as_str();
        current_argument_index += 1;
        if current_argument == "--help" || current_argument == "-h" {
            print!("{}", help_text);
            std::process::exit(0);
        }
        if current_argument == "--" {
            while current_argument_index < arguments.len() {
                positional_arguments.push(arguments[current_argument_index].as_str());
                current_argument_index += 1;
            }
            break;
        }
        if current_argument == "--force" && subcommand_name == "init" {
            do_force = true;
            continue;
        }
        if current_argument.starts_with('-') && current_argument.len() > 1 {
            print_error_invalid_argument(&format!(
                "unexpected argument '{}' for '{}'",
                current_argument, subcommand_name
            ));
        }
        positional_arguments.push(current_argument);
    }

    let maximum_positional_arguments = match subcommand_name {
        "explain" | "test-rules" => 1,
        _ => 0,
    };
    if positional_arguments.len() > maximum_positional_arguments {
        print_error_invalid_argument(&format!(
            "unexpected argument '{}' for '{}'",
            positional_arguments[maximum_positional_arguments], subcommand_name
        ));
    }

    let command = match subcommand_name {
        "explain" => {
            if positional_arguments.is_empty() {
                print_error_invalid_argument("explain requires a rule id: explain <RULE>");
            }
            Command::Explain(ExplainArgs {
                rule: positional_arguments[0].to_string(),
            })
        }
        "test-rules" => {
            let mut rule_dir = None;
            if !positional_arguments.is_empty() {
                rule_dir = Some(PathBuf::from(positional_arguments[0]));
            }
            Command::TestRules(TestRulesArgs { rule_dir })
        }
        "init" => Command::Init(InitArgs { force: do_force }),
        _ => Command::Lsp,
    };
    CliArguments {
        input_file_paths: Vec::new(),
        excluded_paths: Vec::new(),
        command,
    }
}

fn split_flag_and_value(flag: &str) -> (&str, Option<&str>) {
    let separator_position = flag.find('=');
    match separator_position {
        Some(position) => (&flag[..position], Some(&flag[position + 1..])),
        None => (flag, None),
    }
}

/// Resolves the value for a flag that takes an argument.
///
/// If the value was already assigned using an equal sign (e.g. `--flag=value`),
/// returns it directly. Otherwise, advances `current_scan_position` and
/// consumes the next command-line argument as the value.
///
/// Errors out if the flag is the last argument on the command line and no value
/// could be found.
fn consume_flag_value(
    assigned_value: Option<&str>,
    argument_list: &[String],
    current_argument_index: &mut usize,
    flag_name: &str,
) -> String {
    if let Some(value) = assigned_value {
        return value.to_string();
    }
    *current_argument_index += 1;
    if *current_argument_index >= argument_list.len() {
        print_error_invalid_argument(&format!("{} requires a value", flag_name));
    }
    argument_list[*current_argument_index].clone()
}

/// Ensures that a flag was not given a value. Exits with an error if a value
/// was assigned. This is used for flags that do not take an argument.
fn require_no_value(assigned_value: Option<&str>, flag_name: &str) {
    if assigned_value.is_some() {
        print_error_invalid_argument(&format!("{} does not take a value", flag_name));
    }
}

fn print_error_invalid_argument(message: &str) -> ! {
    eprintln!("gdguard: error: {}", message);
    std::process::exit(2);
}
