//! Owner: Track B (W3). `gdguard init`: writes a starter `gdguard.toml` and
//! `.gdguard/rules/`.

/// Arguments of `gdguard init`, parsed by the CLI.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InitArgs {
    /// Overwrite existing files.
    pub force: bool,
}

/// Runs `gdguard init` and returns the process exit code.
pub fn run(_args: &InitArgs) -> i32 {
    eprintln!("`init` is not implemented yet");
    2
}
