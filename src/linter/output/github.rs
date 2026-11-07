//! `github`: GitHub Actions workflow commands, one per issue, so issues show
//! up as annotations on pull requests:
//!
//! `::error file=<path>,line=<l>,col=<c>,endLine=<l>,endColumn=<c>,title=<rule>::<message>`
//!
//! Errors use `::error`, warnings `::warning`, infos `::notice`. Columns are
//! 1-based UTF-16 code units, like the SARIF output.

use crate::linter::diagnostic::Severity;
use crate::linter::output::{LintReport, utf16_column};

pub fn write_github(report: &LintReport, output: &mut String) {
    for file in &report.files {
        for diagnostic in &file.diagnostics {
            let command = match diagnostic.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
                Severity::Info => "notice",
            };
            let span = &diagnostic.span;
            output.push_str(&format!(
                "::{} file={},line={},col={},endLine={},endColumn={},title={}::{}\n",
                command,
                escape_property(&file.path),
                span.start.line,
                utf16_column(&file.source, span.start_byte),
                span.end.line,
                utf16_column(&file.source, span.end_byte),
                escape_property(&diagnostic.rule),
                escape_data(&diagnostic.message),
            ));
        }
    }
}

/// Escapes a workflow command message.
fn escape_data(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '%' => escaped.push_str("%25"),
            '\r' => escaped.push_str("%0D"),
            '\n' => escaped.push_str("%0A"),
            _ => escaped.push(character),
        }
    }
    escaped
}

/// Escapes a workflow command property value: like data, plus `:` and `,`.
fn escape_property(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in escape_data(text).chars() {
        match character {
            ':' => escaped.push_str("%3A"),
            ',' => escaped.push_str("%2C"),
            _ => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_data_and_properties() {
        assert_eq!(escape_data("50% done\r\nnext"), "50%25 done%0D%0Anext");
        assert_eq!(escape_property("C:/a,b.gd"), "C%3A/a%2Cb.gd");
    }
}
