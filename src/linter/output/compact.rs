//! `compact`: one line per diagnostic, `file:line:col: severity [rule] message`.
//! The default when stdout isn't a terminal, and easy to parse.

use crate::linter::output::LintReport;

pub fn write_compact(report: &LintReport, output: &mut String) {
    for file in &report.files {
        for diagnostic in &file.diagnostics {
            output.push_str(&format!(
                "{}:{}:{}: {} [{}] {}\n",
                file.path,
                diagnostic.span.start.line,
                diagnostic.span.start.column,
                diagnostic.severity.as_str(),
                diagnostic.rule,
                diagnostic.message
            ));
        }
    }
}
