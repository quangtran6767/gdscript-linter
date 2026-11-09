//! `json`: schema version 1, for tools and AI agents. Field names are stable;
//! new optional fields may be added without bumping `version`. See
//! "JSON output" in `docs/specification_linter.md`.

use serde::Serialize;

use crate::linter::config::builtin_rule_ids;
use crate::linter::diagnostic::Position;
use crate::linter::output::{LintReport, line_text_at};

pub const JSON_SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
struct JsonReport<'a> {
    version: u32,
    tool: JsonTool,
    summary: JsonSummary,
    issues: Vec<JsonIssue<'a>>,
}

#[derive(Serialize)]
struct JsonTool {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct JsonSummary {
    files: usize,
    errors: usize,
    warnings: usize,
    infos: usize,
    fixable: usize,
    /// Added in W4; optional for readers of version 1.
    baseline_hidden: usize,
}

#[derive(Serialize)]
struct JsonIssue<'a> {
    file: &'a str,
    rule: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    check: Option<&'a str>,
    origin: &'static str,
    severity: &'static str,
    message: &'a str,
    help: Option<&'a str>,
    range: JsonRange,
    snippet: &'a str,
    explain: String,
    fixes: Vec<JsonFix<'a>>,
}

#[derive(Serialize)]
struct JsonRange {
    start: JsonPosition,
    end: JsonPosition,
}

#[derive(Serialize)]
struct JsonPosition {
    line: usize,
    column: usize,
    byte: usize,
}

#[derive(Serialize)]
struct JsonFix<'a> {
    description: &'a str,
    safety: &'static str,
    edits: Vec<JsonEdit<'a>>,
}

#[derive(Serialize)]
struct JsonEdit<'a> {
    range: JsonByteRange,
    replacement: &'a str,
}

#[derive(Serialize)]
struct JsonByteRange {
    start: JsonByte,
    end: JsonByte,
}

#[derive(Serialize)]
struct JsonByte {
    byte: usize,
}

fn json_position(position: Position, byte: usize) -> JsonPosition {
    JsonPosition {
        line: position.line,
        column: position.column,
        byte,
    }
}

pub fn write_json(report: &LintReport, output: &mut String) -> Result<(), String> {
    let builtin_ids = builtin_rule_ids();
    let summary = report.summary();

    let mut issue_count = 0;
    for file in &report.files {
        issue_count += file.diagnostics.len();
    }
    let mut issues = Vec::with_capacity(issue_count);
    for file in &report.files {
        for diagnostic in &file.diagnostics {
            let span = diagnostic.span;
            let mut fixes = Vec::with_capacity(diagnostic.fixes.len());
            for fix in &diagnostic.fixes {
                let mut edits = Vec::with_capacity(fix.edits.len());
                for edit in &fix.edits {
                    edits.push(JsonEdit {
                        range: JsonByteRange {
                            start: JsonByte {
                                byte: edit.span.start_byte,
                            },
                            end: JsonByte {
                                byte: edit.span.end_byte,
                            },
                        },
                        replacement: &edit.replacement,
                    });
                }
                fixes.push(JsonFix {
                    description: &fix.description,
                    safety: fix.safety.as_str(),
                    edits,
                });
            }
            let origin = if builtin_ids.contains(&diagnostic.rule.as_str()) {
                "builtin"
            } else {
                "custom"
            };
            issues.push(JsonIssue {
                file: &file.path,
                rule: &diagnostic.rule,
                check: diagnostic.check.as_deref(),
                origin,
                severity: diagnostic.severity.as_str(),
                message: &diagnostic.message,
                help: diagnostic.help.as_deref(),
                range: JsonRange {
                    start: json_position(span.start, span.start_byte),
                    end: json_position(span.end, span.end_byte),
                },
                snippet: line_text_at(&file.source, span.start_byte),
                explain: format!("gdguard explain {}", diagnostic.rule),
                fixes,
            });
        }
    }

    let json_report = JsonReport {
        version: JSON_SCHEMA_VERSION,
        tool: JsonTool {
            name: "gdguard",
            version: env!("CARGO_PKG_VERSION"),
        },
        summary: JsonSummary {
            files: report.files_checked,
            errors: summary.errors,
            warnings: summary.warnings,
            infos: summary.infos,
            fixable: summary.fixable,
            baseline_hidden: report.baseline_hidden,
        },
        issues,
    };
    let text = serde_json::to_string_pretty(&json_report)
        .map_err(|error| format!("failed to write JSON output: {}", error))?;
    output.push_str(&text);
    output.push('\n');
    Ok(())
}
