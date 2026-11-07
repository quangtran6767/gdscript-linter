//! Owner: Track B (W1 to W3). Custom YAML rules: loader, matching engine and
//! the `RuleProvider` that turns matches into `Diagnostic`s.

pub mod language;

#[cfg(test)]
mod spike_tests;

use crate::linter::api::Config;
use crate::linter::diagnostic::RuleProvider;

/// Loads every custom rule source for `config` (the folders in
/// `config.custom_rule_paths`). The pipeline calls this once per config and
/// runs each provider on every file.
///
/// Contract agreed in W0. Returns no providers until W2 connects the engine.
pub fn load_providers(_config: &Config) -> Result<Vec<Box<dyn RuleProvider>>, String> {
    Ok(Vec::new())
}
