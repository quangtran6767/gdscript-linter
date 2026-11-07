//! `pretty`: for people at a terminal. Each diagnostic shows its position,
//! severity and rule, the source line with carets under the span, the help
//! text and the available fixes, then a summary line closes the report.
//!
//! ```text
//! scripts/enemy.gd
//!  3:8 error [signal-name] Signal name 'OnDefeated' should be in snake_case format
//!   |
//! 3 | signal OnDefeated
//!   |        ^^^^^^^^^^
//!   = help: Rename it to 'on_defeated'.
//! ```

use crate::linter::diagnostic::{Diagnostic, Severity};
use crate::linter::output::{LintReport, line_text_at};

/// Tabs are shown as this many spaces so the carets line up.
const TAB_WIDTH: usize = 4;

struct Style {
    bold: &'static str,
    dim: &'static str,
    reset: &'static str,
    gutter: &'static str,
    help: &'static str,
}

const COLORED: Style = Style {
    bold: "\x1b[1m",
    dim: "\x1b[2m",
    reset: "\x1b[0m",
    gutter: "\x1b[34m",
    help: "\x1b[36m",
};

const PLAIN: Style = Style {
    bold: "",
    dim: "",
    reset: "",
    gutter: "",
    help: "",
};

fn severity_color(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "\x1b[1;31m",
        Severity::Warning => "\x1b[1;33m",
        Severity::Info => "\x1b[1;34m",
    }
}

pub fn write_pretty(report: &LintReport, with_colors: bool, output: &mut String) {
    let style = if with_colors { &COLORED } else { &PLAIN };

    for file in &report.files {
        output.push_str(&format!("{}{}{}\n", style.bold, file.path, style.reset));
        for diagnostic in &file.diagnostics {
            write_diagnostic(&file.source, diagnostic, with_colors, style, output);
            output.push('\n');
        }
    }

    let summary = report.summary();
    let problem_count = summary.errors + summary.warnings + summary.infos;
    if problem_count == 0 {
        return;
    }
    let mut counts: Vec<String> = Vec::with_capacity(3);
    counts.push(plural(summary.errors, "error"));
    counts.push(plural(summary.warnings, "warning"));
    if summary.infos > 0 {
        counts.push(plural(summary.infos, "info"));
    }
    output.push_str(&format!(
        "{}Found {} ({}) in {}.{}",
        style.bold,
        plural(problem_count, "problem"),
        counts.join(", "),
        plural(report.files.len(), "file"),
        style.reset
    ));
    if summary.fixable > 0 {
        output.push_str(&format!(" {} fixable with --fix.", summary.fixable));
    }
    output.push('\n');
}

fn plural(count: usize, word: &str) -> String {
    if count == 1 {
        format!("{} {}", count, word)
    } else {
        format!("{} {}s", count, word)
    }
}

fn write_diagnostic(
    source: &str,
    diagnostic: &Diagnostic,
    with_colors: bool,
    style: &Style,
    output: &mut String,
) {
    let span = diagnostic.span;
    let (color, color_reset) = if with_colors {
        (severity_color(diagnostic.severity), style.reset)
    } else {
        ("", "")
    };
    output.push_str(&format!(
        " {}:{} {}{}{} {}[{}]{} {}\n",
        span.start.line,
        span.start.column,
        color,
        diagnostic.severity.as_str(),
        color_reset,
        style.dim,
        diagnostic.rule,
        style.reset,
        diagnostic.message
    ));

    // Snippet of the first line of the span, with carets under the spanned
    // part. A span over several lines is underlined to the end of its first
    // line.
    let line_text = line_text_at(source, span.start_byte);
    let line_start_byte = span.start_byte - (span.start.column - 1);
    let span_start_in_line = span.start_byte - line_start_byte;
    let mut span_end_in_line = span.end_byte.saturating_sub(line_start_byte);
    if span_end_in_line > line_text.len() || span.end.line != span.start.line {
        span_end_in_line = line_text.len();
    }
    let caret_offset = display_width(&line_text[..span_start_in_line]);
    let caret_count = display_width(&line_text[span_start_in_line..span_end_in_line]).max(1);

    let line_number = span.start.line.to_string();
    let gutter_padding = " ".repeat(line_number.len());
    output.push_str(&format!(
        "{}{} |{}\n",
        style.gutter, gutter_padding, style.reset
    ));
    output.push_str(&format!(
        "{}{} |{} {}\n",
        style.gutter,
        line_number,
        style.reset,
        expand_tabs(line_text)
    ));
    output.push_str(&format!(
        "{}{} |{} {}{}{}{}\n",
        style.gutter,
        gutter_padding,
        style.reset,
        " ".repeat(caret_offset),
        color,
        "^".repeat(caret_count),
        color_reset
    ));

    if let Some(help) = &diagnostic.help {
        output.push_str(&format!(
            "{}{} ={} {}help{}: {}\n",
            style.gutter, gutter_padding, style.reset, style.help, style.reset, help
        ));
    }
    for fix in &diagnostic.fixes {
        output.push_str(&format!(
            "{}{} ={} {}fix ({}){}: {}\n",
            style.gutter,
            gutter_padding,
            style.reset,
            style.help,
            fix.safety.as_str(),
            style.reset,
            fix.description
        ));
    }
}

/// Width of `text` on screen, counting a tab as `TAB_WIDTH` columns and
/// every other character as one.
fn display_width(text: &str) -> usize {
    let mut width = 0;
    for character in text.chars() {
        if character == '\t' {
            width += TAB_WIDTH;
        } else {
            width += 1;
        }
    }
    width
}

fn expand_tabs(text: &str) -> String {
    text.replace('\t', &" ".repeat(TAB_WIDTH))
}
