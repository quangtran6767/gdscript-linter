//! Owner: Track B (W2). `gdguard test-rules [RULE_DIR]`: runs the `tests` of
//! custom rules and checks fixes with `apply_edits`.

use std::path::PathBuf;

/// Arguments of `gdguard test-rules [RULE_DIR]`, parsed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TestRulesArgs {
    /// `None` means: the custom rule paths from the config, or
    /// `.gdguard/rules`.
    pub rule_dir: Option<PathBuf>,
}

/// Runs `gdguard test-rules` and returns the process exit code.
pub fn run(_args: &TestRulesArgs) -> i32 {
    eprintln!("`test-rules` is not implemented yet");
    2
}
