//! Owner: Track A. Reporters: pretty, compact and json (W2), github and sarif
//! (W4). Each reporter writes into a `String` so it can be tested without a
//! terminal; the CLI prints the result.

pub mod compact;
pub mod json;
pub mod pretty;

use crate::linter::diagnostic::{Diagnostic, FixSafety, Severity};

/// The value of `lint --format`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Pretty,
    Compact,
    Json,
    Github,
    Sarif,
}

impl OutputFormat {
    pub fn from_name(name: &str) -> Option<OutputFormat> {
        match name {
            "pretty" => Some(OutputFormat::Pretty),
            "compact" => Some(OutputFormat::Compact),
            "json" => Some(OutputFormat::Json),
            "github" => Some(OutputFormat::Github),
            "sarif" => Some(OutputFormat::Sarif),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OutputFormat::Pretty => "pretty",
            OutputFormat::Compact => "compact",
            OutputFormat::Json => "json",
            OutputFormat::Github => "github",
            OutputFormat::Sarif => "sarif",
        }
    }
}

/// The diagnostics of one file, with its source for snippets.
#[derive(Debug, Clone, PartialEq)]
pub struct FileDiagnostics {
    /// The path as shown to the user: relative paths without a leading `./`,
    /// with `/` separators.
    pub path: String,
    pub source: String,
    /// Sorted by position.
    pub diagnostics: Vec<Diagnostic>,
}

/// Everything a reporter needs about one lint run.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LintReport {
    /// Only files with at least one diagnostic, in the order they were linted.
    pub files: Vec<FileDiagnostics>,
    /// How many files were linted, including clean ones.
    pub files_checked: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReportSummary {
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    /// Diagnostics with at least one `Safe` fix, which `--fix` applies.
    pub fixable: usize,
}

impl ReportSummary {
    /// Exit code of the `lint` command: 1 when there are errors, or more
    /// warnings than `max_warnings` allows; 0 otherwise. Without
    /// `--max-warnings`, any warning fails. Info diagnostics never fail.
    pub fn exit_code(&self, max_warnings: Option<usize>) -> i32 {
        if self.errors > 0 {
            return 1;
        }
        if self.warnings > max_warnings.unwrap_or(0) {
            return 1;
        }
        0
    }
}

impl LintReport {
    pub fn summary(&self) -> ReportSummary {
        let mut summary = ReportSummary::default();
        for file in &self.files {
            for diagnostic in &file.diagnostics {
                match diagnostic.severity {
                    Severity::Error => summary.errors += 1,
                    Severity::Warning => summary.warnings += 1,
                    Severity::Info => summary.infos += 1,
                }
                if has_safe_fix(diagnostic) {
                    summary.fixable += 1;
                }
            }
        }
        summary
    }
}

fn has_safe_fix(diagnostic: &Diagnostic) -> bool {
    for fix in &diagnostic.fixes {
        if fix.safety == FixSafety::Safe {
            return true;
        }
    }
    false
}

/// Writes the report in `format` into `output`. `with_colors` only affects
/// the pretty format. Returns an error for formats that aren't implemented
/// yet.
pub fn write_report(
    format: OutputFormat,
    report: &LintReport,
    with_colors: bool,
    output: &mut String,
) -> Result<(), String> {
    match format {
        OutputFormat::Pretty => pretty::write_pretty(report, with_colors, output),
        OutputFormat::Compact => compact::write_compact(report, output),
        OutputFormat::Json => json::write_json(report, output)?,
        OutputFormat::Github | OutputFormat::Sarif => {
            return Err(format!(
                "`--format {}` is not implemented yet",
                format.as_str()
            ));
        }
    }
    Ok(())
}

/// Turns a path into the form reporters show: `/` separators and no leading
/// `./`.
pub fn display_path(path: &std::path::Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    match text.strip_prefix("./") {
        Some(stripped) => stripped.to_string(),
        None => text,
    }
}

/// The full text of the line that contains `byte`, without its line ending.
pub fn line_text_at(source: &str, byte: usize) -> &str {
    let byte = byte.min(source.len());
    let line_start = match source[..byte].rfind('\n') {
        Some(newline_index) => newline_index + 1,
        None => 0,
    };
    let line_end = match source[line_start..].find('\n') {
        Some(newline_offset) => line_start + newline_offset,
        None => source.len(),
    };
    source[line_start..line_end].trim_end_matches('\r')
}

#[cfg(test)]
mod tests;
