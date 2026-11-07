use crate::node_kind::GDScriptNodeKind;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io::IsTerminal};
use tree_sitter::{Node, Parser, Tree};

// Shared contracts (frozen after W0).
pub mod api;
pub mod diagnostic;

pub mod ignore_patterns;
pub mod lib;
pub mod regex_patterns;
pub mod rule_config;
pub mod rules;

// Track A modules.
pub mod baseline;
pub mod cache;
pub mod config;
pub mod fix;
pub mod output;

// Track B modules.
pub mod custom;
pub mod explain;
pub mod init;
pub mod rule_tester;
pub mod rules_meta;

#[cfg(test)]
mod tests;

use diagnostic::{Diagnostic, Severity};
use ignore_patterns::{parse_ignore_patterns, should_ignore_rule};
use rules::{ALL_RULES, Rule};

/// Formats one diagnostic in the default text output:
/// `path:line:rule:severity: message`.
pub fn format_diagnostic_line(file_path: &str, diagnostic: &Diagnostic) -> String {
    format!(
        "{}:{}:{}:{}: {}",
        file_path,
        diagnostic.span.start.line,
        diagnostic.rule,
        diagnostic.severity.as_str(),
        diagnostic.message
    )
}

#[derive(Debug, Clone)]
pub struct LinterConfig {
    pub disabled_rules: HashSet<String>,
    pub max_line_length: usize,
    pub indent_size: usize,
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            disabled_rules: HashSet::new(),
            max_line_length: 100,
            indent_size: 4,
        }
    }
}

/// Counts of the diagnostics reported over a lint run, used to pick the exit
/// code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LintSummary {
    pub error_count: usize,
    pub warning_count: usize,
    pub info_count: usize,
}

impl LintSummary {
    pub fn add(&mut self, diagnostics: &[Diagnostic]) {
        for diagnostic in diagnostics {
            match diagnostic.severity {
                Severity::Error => self.error_count += 1,
                Severity::Warning => self.warning_count += 1,
                Severity::Info => self.info_count += 1,
            }
        }
    }

    /// Exit code of the `lint` command: 1 when there are errors, or more
    /// warnings than `max_warnings` allows; 0 otherwise. Without
    /// `--max-warnings`, any warning fails, like before exit codes existed.
    /// Info diagnostics never fail.
    pub fn exit_code(&self, max_warnings: Option<usize>) -> i32 {
        if self.error_count > 0 {
            return 1;
        }
        let allowed_warnings = max_warnings.unwrap_or(0);
        if self.warning_count > allowed_warnings {
            return 1;
        }
        0
    }
}

pub struct GDScriptLinter {
    config: LinterConfig,
    parser: Parser,
}

impl GDScriptLinter {
    pub fn new(config: LinterConfig) -> Result<Self, String> {
        let mut parser = Parser::new();
        let language = tree_sitter_gdscript::LANGUAGE.into();
        parser
            .set_language(&language)
            .map_err(|e| format!("Failed to set language: {}", e))?;
        GDScriptNodeKind::populate_lookup_table();
        Ok(Self { config, parser })
    }

    /// Parses and lints one file. Returns the diagnostics left after inline
    /// ignore comments, sorted by position.
    pub fn lint(&mut self, source_code: &str, _file_path: &str) -> Result<Vec<Diagnostic>, String> {
        let tree = self
            .parser
            .parse(source_code, None)
            .ok_or("Failed to parse GDScript code")?;

        let mut diagnostics = Vec::new();
        run_builtin_rules(&self.config, &tree, source_code, &mut diagnostics);

        // Inline ignore comments apply to the line where a diagnostic starts.
        let ignore_map = parse_ignore_patterns(source_code);
        if !ignore_map.is_empty() {
            diagnostics.retain(|diagnostic| {
                !should_ignore_rule(&ignore_map, diagnostic.span.start.line, &diagnostic.rule)
            });
        }

        // Sort by position. Rules that run on the source code like the line
        // length check would otherwise appear at the end.
        diagnostics.sort_by(|a, b| {
            a.span
                .start
                .line
                .cmp(&b.span.start.line)
                .then(a.span.start.column.cmp(&b.span.start.column))
        });

        Ok(diagnostics)
    }

    pub fn lint_files(
        &mut self,
        gdscript_files: &[PathBuf],
        do_pretty_print: bool,
    ) -> Result<LintSummary, String> {
        self.lint_files_with_editorconfig(gdscript_files, do_pretty_print, None)
    }

    /// Lints each file with its own `.editorconfig` settings, prints the
    /// diagnostics and returns their counts.
    pub fn lint_files_with_editorconfig(
        &mut self,
        gdscript_files: &[PathBuf],
        do_pretty_print: bool,
        max_line_length_override: Option<usize>,
    ) -> Result<LintSummary, String> {
        let with_colors = std::io::stdout().is_terminal();
        let base_config = self.config.clone();

        if do_pretty_print {
            self.lint_files_pretty(
                gdscript_files,
                with_colors,
                &base_config,
                max_line_length_override,
            )
        } else {
            self.lint_files_standard(gdscript_files, &base_config, max_line_length_override)
        }
    }

    fn apply_file_config(
        &mut self,
        base_config: &LinterConfig,
        file_path: &Path,
        max_line_length_override: Option<usize>,
    ) {
        self.config = base_config.clone();
        crate::editorconfig::apply_editorconfig_to_linter_config(&mut self.config, file_path);
        if let Some(max_line_length) = max_line_length_override {
            self.config.max_line_length = max_line_length;
        }
    }

    fn lint_files_pretty(
        &mut self,
        gdscript_files: &[PathBuf],
        with_colors: bool,
        base_config: &LinterConfig,
        max_line_length_override: Option<usize>,
    ) -> Result<LintSummary, String> {
        let mut summary = LintSummary::default();
        // Files keep the order they were given in (sorted by the CLI).
        let mut files_with_diagnostics: Vec<(String, Vec<Diagnostic>)> = Vec::new();

        for file_path in gdscript_files {
            self.apply_file_config(base_config, file_path, max_line_length_override);
            let source_code = fs::read_to_string(file_path).map_err(|error| {
                format!("Failed to read file {}: {}", file_path.display(), error)
            })?;

            let diagnostics = self.lint(&source_code, &file_path.to_string_lossy())?;
            summary.add(&diagnostics);
            if !diagnostics.is_empty() {
                files_with_diagnostics.push((file_path.to_string_lossy().to_string(), diagnostics));
            }
        }

        let bold = if with_colors { "\x1b[1m" } else { "" };
        let reset = if with_colors { "\x1b[0m" } else { "" };
        for (file_index, (file_path, diagnostics)) in files_with_diagnostics.iter().enumerate() {
            println!("{}{}{}", bold, file_path, reset);

            // Diagnostics are sorted by line, so each line's group is a run of
            // consecutive diagnostics.
            let mut current_line = 0;
            for diagnostic in diagnostics {
                let line = diagnostic.span.start.line;
                if line != current_line {
                    if current_line != 0 {
                        println!();
                    }
                    println!("    {}:{}", file_path, line);
                    current_line = line;
                }
                let (severity_label, severity_color) = match diagnostic.severity {
                    Severity::Error => ("ERROR", "\x1b[31m"),  // Red
                    Severity::Warning => ("WARN", "\x1b[33m"), // Yellow
                    Severity::Info => ("INFO", "\x1b[34m"),    // Blue
                };
                if with_colors {
                    println!(
                        "        {}{}\x1b[0m: `{}`",
                        severity_color, severity_label, diagnostic.rule
                    );
                } else {
                    println!("        {}: `{}`", severity_label, diagnostic.rule);
                }
                println!("        {}", diagnostic.message);
            }

            // Add separator between files (except for the last file)
            if file_index + 1 < files_with_diagnostics.len() {
                println!("\n{}", "-".repeat(60));
                println!();
            }
        }

        Ok(summary)
    }

    fn lint_files_standard(
        &mut self,
        gdscript_files: &[PathBuf],
        base_config: &LinterConfig,
        max_line_length_override: Option<usize>,
    ) -> Result<LintSummary, String> {
        let mut summary = LintSummary::default();

        for file_path in gdscript_files {
            self.apply_file_config(base_config, file_path, max_line_length_override);
            let source_code = fs::read_to_string(file_path).map_err(|error| {
                format!("Failed to read file {}: {}", file_path.display(), error)
            })?;

            let diagnostics = self.lint(&source_code, &file_path.to_string_lossy())?;
            summary.add(&diagnostics);
            let display_path = file_path.to_string_lossy();
            for diagnostic in &diagnostics {
                println!("{}", format_diagnostic_line(&display_path, diagnostic));
            }
        }

        Ok(summary)
    }
}

/// Runs every enabled built-in rule on a parsed file and appends their
/// diagnostics, unfiltered and unsorted.
///
/// We build a map from node kinds to the rules that care about them, so we
/// walk the tree only once and call each rule only on the nodes it targets.
/// Source-only rules run first, then the tree walk, then each rule's
/// `finalize` for rules that report after collecting state.
pub fn run_builtin_rules(
    config: &LinterConfig,
    tree: &Tree,
    source_code: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut checkers: Vec<Box<dyn Rule>> = Vec::with_capacity(ALL_RULES.len());
    for current_rule in ALL_RULES {
        if !config.disabled_rules.contains(current_rule.name) {
            checkers.push((current_rule.create)(config));
        }
    }

    let mut node_kind_map: HashMap<GDScriptNodeKind, Vec<usize>> = HashMap::new();
    let mut source_only_rules = Vec::new();
    for (current_index, checker) in checkers.iter().enumerate() {
        let kinds = checker.get_target_ast_nodes();
        if kinds.is_empty() {
            source_only_rules.push(current_index);
        } else {
            for &kind in kinds {
                node_kind_map.entry(kind).or_default().push(current_index);
            }
        }
    }

    for &current_index in &source_only_rules {
        checkers[current_index].check_source(source_code, diagnostics);
    }
    visit_each_node(
        &tree.root_node(),
        source_code,
        &mut checkers,
        &node_kind_map,
        diagnostics,
    );
    for checker in checkers.iter_mut() {
        checker.finalize(source_code, diagnostics);
    }
}

/// This uses the visitor pattern to walk the parsed tree sitter AST only once.
/// We call each rule only when we encounter an AST node it cares about.
fn visit_each_node(
    node: &Node,
    source_code: &str,
    checkers: &mut [Box<dyn Rule>],
    node_kind_map: &HashMap<GDScriptNodeKind, Vec<usize>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(matching_rules) =
        node_kind_map.get(&GDScriptNodeKind::get_kind_from_ast_node(*node))
    {
        for &rule_index in matching_rules {
            checkers[rule_index].check_node(node, source_code, diagnostics);
        }
    }

    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            visit_each_node(
                &cursor.node(),
                source_code,
                checkers,
                node_kind_map,
                diagnostics,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

pub fn lint_gdscript_with_config(
    source_code: &str,
    file_path: &str,
    config: &LinterConfig,
) -> Result<Vec<Diagnostic>, String> {
    let mut linter = GDScriptLinter::new(config.clone())?;
    linter.lint(source_code, file_path)
}

pub fn lint_gdscript(source_code: &str, file_path: &str) -> Result<Vec<Diagnostic>, String> {
    let config = LinterConfig::default();
    lint_gdscript_with_config(source_code, file_path, &config)
}
