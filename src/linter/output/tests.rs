//! Snapshot tests for the reporters. The expected outputs live in
//! `tests/output/expected/`. Run with `UPDATE_SNAPSHOTS=1` to rewrite them
//! after an intended change, then review the diff.

#![allow(clippy::unwrap_used)]

use super::*;
use crate::linter::diagnostic::{Edit, Fix, FixSafety, Span};

const PLAYER_SOURCE: &str = "extends Node\n\nfunc café(niño: int) -> void:\n\tvar label = \"🎮 joué\"\n\tif len(items) == 0:\n\t\tpass\n";
const ENEMY_SOURCE: &str = "signal OnDefeated\n";

/// A diagnostic covering the first occurrence of `needle` in `source`.
fn diagnostic_on(
    source: &str,
    needle: &str,
    rule: &str,
    severity: Severity,
    message: &str,
    help: Option<&str>,
    fixes: Vec<Fix>,
) -> Diagnostic {
    let start_byte = source.find(needle).unwrap();
    Diagnostic {
        rule: rule.to_string(),
        severity,
        message: message.to_string(),
        help: help.map(str::to_string),
        span: Span::from_byte_range(source, start_byte, start_byte + needle.len()),
        fixes,
    }
}

fn sample_report() -> LintReport {
    let comparison = "len(items) == 0";
    let comparison_start = PLAYER_SOURCE.find(comparison).unwrap();
    let is_empty_fix = Fix {
        description: "Replace with items.is_empty()".to_string(),
        safety: FixSafety::Safe,
        edits: vec![Edit {
            span: Span::from_byte_range(
                PLAYER_SOURCE,
                comparison_start,
                comparison_start + comparison.len(),
            ),
            replacement: "items.is_empty()".to_string(),
        }],
    };
    let player_diagnostics = vec![
        diagnostic_on(
            PLAYER_SOURCE,
            "niño",
            "unused-argument",
            Severity::Warning,
            "Function argument 'niño' is unused. Consider removing it or prefixing with '_'",
            Some(
                "Remove 'niño', or rename it to '_niño' if the function signature must stay the same.",
            ),
            Vec::new(),
        ),
        diagnostic_on(
            PLAYER_SOURCE,
            "\"🎮 joué\"",
            "no-emoji-strings",
            Severity::Info,
            "String contains an emoji",
            None,
            Vec::new(),
        ),
        diagnostic_on(
            PLAYER_SOURCE,
            comparison,
            "prefer-is-empty",
            Severity::Warning,
            "Use `items.is_empty()` instead of `len(items) == 0`",
            Some("is_empty() states the intent."),
            vec![is_empty_fix],
        ),
        // A span over two lines: carets go to the end of the first line.
        diagnostic_on(
            PLAYER_SOURCE,
            "if len(items) == 0:\n\t\tpass",
            "no-empty-if",
            Severity::Error,
            "This if block does nothing",
            None,
            vec![Fix {
                description: "Remove the if block".to_string(),
                safety: FixSafety::Suggestion,
                edits: Vec::new(),
            }],
        ),
    ];
    let enemy_diagnostics = vec![diagnostic_on(
        ENEMY_SOURCE,
        "OnDefeated",
        "signal-name",
        Severity::Error,
        "Signal name 'OnDefeated' should be in snake_case format",
        Some("Rename it to 'on_defeated'."),
        Vec::new(),
    )];
    LintReport {
        files: vec![
            FileDiagnostics {
                path: "scripts/player.gd".to_string(),
                source: PLAYER_SOURCE.to_string(),
                diagnostics: player_diagnostics,
            },
            FileDiagnostics {
                path: "scripts/enemies/enemy.gd".to_string(),
                source: ENEMY_SOURCE.to_string(),
                diagnostics: enemy_diagnostics,
            },
        ],
        files_checked: 3,
        baseline_hidden: 0,
    }
}

fn assert_snapshot(snapshot_name: &str, actual: &str) {
    let snapshot_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/output/expected")
        .join(snapshot_name);
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() {
        std::fs::write(&snapshot_path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&snapshot_path).unwrap();
    assert_eq!(
        actual, expected,
        "snapshot {} doesn't match (UPDATE_SNAPSHOTS=1 rewrites it)",
        snapshot_name
    );
}

fn render(format: OutputFormat, report: &LintReport, with_colors: bool) -> String {
    let mut output = String::new();
    write_report(format, report, with_colors, &mut output).unwrap();
    output
}

#[test]
fn pretty_snapshot() {
    assert_snapshot(
        "pretty.txt",
        &render(OutputFormat::Pretty, &sample_report(), false),
    );
}

#[test]
fn pretty_colored_snapshot() {
    assert_snapshot(
        "pretty_colored.txt",
        &render(OutputFormat::Pretty, &sample_report(), true),
    );
}

#[test]
fn compact_snapshot() {
    assert_snapshot(
        "compact.txt",
        &render(OutputFormat::Compact, &sample_report(), false),
    );
}

#[test]
fn json_snapshot() {
    assert_snapshot(
        "json.txt",
        &render(OutputFormat::Json, &sample_report(), false),
    );
}

#[test]
fn empty_report_prints_nothing_except_json() {
    let report = LintReport {
        files: Vec::new(),
        files_checked: 2,
        baseline_hidden: 0,
    };
    assert_eq!(render(OutputFormat::Pretty, &report, false), "");
    assert_eq!(render(OutputFormat::Compact, &report, false), "");
    let json: serde_json::Value =
        serde_json::from_str(&render(OutputFormat::Json, &report, false)).unwrap();
    assert_eq!(json["summary"]["files"], 2);
    assert_eq!(json["issues"].as_array().unwrap().len(), 0);
}

#[test]
fn pretty_carets_line_up_with_tabs_and_multi_byte_characters() {
    let output = render(OutputFormat::Pretty, &sample_report(), false);
    // The comparison is after one tab (4 columns) and "if " (3 columns).
    assert!(output.contains("5 |     if len(items) == 0:\n  |        ^^^^^^^^^^^^^^^\n"));
    // "niño" is 5 bytes but 4 characters wide.
    assert!(output.contains("3 | func café(niño: int) -> void:\n  |           ^^^^\n"));
}

/// Checks every field the spec's "JSON output" section names, with types.
#[test]
fn json_has_every_schema_field() {
    let json: serde_json::Value =
        serde_json::from_str(&render(OutputFormat::Json, &sample_report(), false)).unwrap();

    assert_eq!(json["version"], 1);
    assert_eq!(json["tool"]["name"], "gdguard");
    assert_eq!(json["tool"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(json["summary"]["files"], 3);
    assert_eq!(json["summary"]["errors"], 2);
    assert_eq!(json["summary"]["warnings"], 2);
    assert_eq!(json["summary"]["infos"], 1);
    assert_eq!(json["summary"]["fixable"], 1);

    let issues = json["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 5);
    for issue in issues {
        for field in [
            "file", "rule", "origin", "severity", "message", "snippet", "explain",
        ] {
            assert!(issue[field].is_string(), "{} should be a string", field);
        }
        assert!(issue["help"].is_string() || issue["help"].is_null());
        assert!(issue["fixes"].is_array());
        for end in ["start", "end"] {
            for key in ["line", "column", "byte"] {
                assert!(issue["range"][end][key].is_u64(), "range.{}.{}", end, key);
            }
        }
    }

    let unused_argument = &issues[0];
    assert_eq!(unused_argument["file"], "scripts/player.gd");
    assert_eq!(unused_argument["rule"], "unused-argument");
    assert_eq!(unused_argument["origin"], "builtin");
    assert_eq!(unused_argument["severity"], "warning");
    assert_eq!(unused_argument["snippet"], "func café(niño: int) -> void:");
    assert_eq!(
        unused_argument["explain"],
        "gdguard explain unused-argument"
    );
    let niño_byte = PLAYER_SOURCE.find("niño").unwrap() as u64;
    assert_eq!(unused_argument["range"]["start"]["byte"], niño_byte);
    assert_eq!(unused_argument["range"]["start"]["line"], 3);
    assert_eq!(unused_argument["range"]["start"]["column"], niño_byte - 13);
    assert_eq!(unused_argument["range"]["end"]["byte"], niño_byte + 5);

    let prefer_is_empty = &issues[2];
    assert_eq!(prefer_is_empty["origin"], "custom");
    assert_eq!(prefer_is_empty["help"], "is_empty() states the intent.");
    let fix = &prefer_is_empty["fixes"][0];
    assert_eq!(fix["description"], "Replace with items.is_empty()");
    assert_eq!(fix["safety"], "safe");
    let edit = &fix["edits"][0];
    assert_eq!(edit["replacement"], "items.is_empty()");
    let comparison_byte = PLAYER_SOURCE.find("len(items)").unwrap() as u64;
    assert_eq!(edit["range"]["start"]["byte"], comparison_byte);
    assert_eq!(edit["range"]["end"]["byte"], comparison_byte + 15);

    assert!(issues[1]["help"].is_null());
    assert_eq!(issues[4]["file"], "scripts/enemies/enemy.gd");
}

#[test]
fn display_path_strips_dot_slash_and_uses_forward_slashes() {
    assert_eq!(
        display_path(std::path::Path::new("./scripts/a.gd")),
        "scripts/a.gd"
    );
    assert_eq!(
        display_path(std::path::Path::new("scripts\\ui\\b.gd")),
        "scripts/ui/b.gd"
    );
    assert_eq!(display_path(std::path::Path::new("/abs/c.gd")), "/abs/c.gd");
}

#[test]
fn exit_code_follows_errors_and_max_warnings() {
    let summary = ReportSummary {
        errors: 0,
        warnings: 2,
        infos: 5,
        fixable: 0,
    };
    assert_eq!(summary.exit_code(None), 1);
    assert_eq!(summary.exit_code(Some(1)), 1);
    assert_eq!(summary.exit_code(Some(2)), 0);
    let with_error = ReportSummary {
        errors: 1,
        ..summary
    };
    assert_eq!(with_error.exit_code(Some(100)), 1);
    assert_eq!(ReportSummary::default().exit_code(None), 0);
}
