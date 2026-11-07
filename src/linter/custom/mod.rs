//! Owner: Track B (W1 to W3). Custom YAML rules: loader, matching engine and
//! the `RuleProvider` that turns matches into `Diagnostic`s.

pub mod doc;
pub mod language;
pub mod loader;
pub mod provider;

#[cfg(test)]
mod spike_tests;

use crate::linter::api::Config;
use crate::linter::diagnostic::RuleProvider;

pub use provider::YamlRuleProvider;

/// Loads every custom rule source for `config` (the folders in
/// `config.custom_rule_paths`). The pipeline calls this once per config and
/// runs each provider on every file.
///
/// Returns one `YamlRuleProvider` holding every YAML rule, or no provider
/// when `custom_rule_paths` is empty. Errors are rule-file errors, formatted
/// as `<path>: `<key>`: <message>`; the CLI prints them and exits with 2.
pub fn load_providers(config: &Config) -> Result<Vec<Box<dyn RuleProvider>>, String> {
    if config.custom_rule_paths.is_empty() {
        return Ok(Vec::new());
    }
    let rules = match loader::load_rules(&config.custom_rule_paths) {
        Ok(rules) => rules,
        Err(error) => return Err(error.to_string()),
    };
    let provider: Box<dyn RuleProvider> = Box::new(YamlRuleProvider::new(rules));
    Ok(vec![provider])
}
