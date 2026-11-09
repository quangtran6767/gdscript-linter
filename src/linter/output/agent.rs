//! `agent`: short plain text for coding agents. Each issue is one
//! `file:line:col: severity [rule] message` line (like compact), followed by
//! the source line, the help and the fixes that `--fix` doesn't apply, so an
//! agent can act without reading the full JSON or reopening the file. One
//! summary line ends the report. A clean run prints nothing.

use crate::linter::diagnostic::FixSafety;
use crate::linter::output::{LintReport, line_text_at};

/// Source lines longer than this are cut, with `...` appended.
const MAX_SNIPPET_CHARS: usize = 120;

pub fn write_agent(report: &LintReport, output: &mut String) {
    let summary = report.summary();
    let issue_count = summary.errors + summary.warnings + summary.infos;
    if issue_count == 0 {
        return;
    }
    let mut has_unsafe_fix = false;
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
            let line_text = line_text_at(&file.source, diagnostic.span.start_byte).trim();
            if !line_text.is_empty() {
                output.push_str(&format!("  > {}\n", shorten(line_text)));
            }
            if let Some(help) = &diagnostic.help {
                output.push_str(&format!("  help: {}\n", help));
            }
            for fix in &diagnostic.fixes {
                let label = match fix.safety {
                    FixSafety::Safe => "safe fix",
                    FixSafety::Unsafe => {
                        has_unsafe_fix = true;
                        "unsafe fix"
                    }
                    FixSafety::Suggestion => "suggestion",
                };
                output.push_str(&format!("  {}: {}\n", label, fix.description));
            }
        }
    }

    let mut counts = Vec::new();
    push_count(&mut counts, summary.errors, "error", "errors");
    push_count(&mut counts, summary.warnings, "warning", "warnings");
    push_count(&mut counts, summary.infos, "info", "infos");
    let issue_word = if issue_count == 1 { "issue" } else { "issues" };
    output.push_str(&format!(
        "{} {} ({})",
        issue_count,
        issue_word,
        counts.join(", ")
    ));
    if report.baseline_hidden > 0 {
        output.push_str(&format!(", {} hidden by baseline", report.baseline_hidden));
    }
    output.push_str(". Rule details: gdguard explain <rule>\n");
    if summary.fixable > 0 {
        output.push_str("Safe fixes: run gdguard lint --fix.\n");
    }
    if has_unsafe_fix {
        output.push_str(
            "Unsafe fixes may change behavior: review each, then edit the code or run gdguard lint --fix-unsafe.\n",
        );
    }
}

fn push_count(counts: &mut Vec<String>, count: usize, singular: &str, plural: &str) {
    if count == 0 {
        return;
    }
    let word = if count == 1 { singular } else { plural };
    counts.push(format!("{} {}", count, word));
}

/// Cuts `text` to `MAX_SNIPPET_CHARS` characters, on a character boundary.
fn shorten(text: &str) -> String {
    let mut shortened = String::new();
    for (index, character) in text.chars().enumerate() {
        if index == MAX_SNIPPET_CHARS {
            shortened.push_str("...");
            return shortened;
        }
        shortened.push(character);
    }
    shortened
}
