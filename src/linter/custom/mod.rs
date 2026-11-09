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
    let mut metas = Vec::with_capacity(rules.len());
    for rule in &rules {
        metas.push(rule.meta.clone());
    }
    #[allow(unused_mut)]
    let mut providers: Vec<Box<dyn RuleProvider>> = vec![Box::new(YamlRuleProvider::new(rules))];
    let files = find_js_files(&config.custom_rule_paths)?;
    if !files.is_empty() {
        #[cfg(feature = "js-rules")]
        providers.push(Box::new(crate::linter::script::JsRuleProvider::load(
            config, &files, &metas,
        )?));
        #[cfg(not(feature = "js-rules"))]
        return Err(format!(
            "{}: JavaScript rules require a build with the js-rules feature",
            files[0].display()
        ));
    }
    Ok(providers)
}

/// Sorted, non-recursive JS discovery, shared with the rule tester/cache.
pub fn find_js_files(folders: &[std::path::PathBuf]) -> Result<Vec<std::path::PathBuf>, String> {
    let mut files = Vec::new();
    for folder in folders {
        for entry in
            std::fs::read_dir(folder).map_err(|error| format!("{}: {}", folder.display(), error))?
        {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_file() && path.extension().is_some_and(|extension| extension == "js") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}
