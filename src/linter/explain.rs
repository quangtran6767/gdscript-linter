//! Owner: Track B (W3). `gdguard explain <RULE>` and `lint --list-rules`.

use crate::linter::rule_config::get_all_rule_names;

/// Arguments of `gdguard explain <RULE>`, parsed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplainArgs {
    pub rule: String,
}

/// The `--format` value of `lint --list-rules`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListRulesFormat {
    Text,
    Json,
}

/// Runs `gdguard explain` and returns the process exit code.
pub fn run(_args: &ExplainArgs) -> i32 {
    eprintln!("`explain` is not implemented yet");
    2
}

/// Runs `lint --list-rules` and returns the process exit code.
///
/// The text format keeps the pre-W0 output until W3 replaces it with
/// descriptions from the rule metadata.
pub fn list_rules(format: ListRulesFormat) -> i32 {
    match format {
        ListRulesFormat::Text => {
            println!("Available linting rules:");
            for rule in get_all_rule_names() {
                println!("  {}", rule);
            }
            0
        }
        ListRulesFormat::Json => {
            eprintln!("`--list-rules --format json` is not implemented yet");
            2
        }
    }
}
