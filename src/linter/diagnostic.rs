//! Shared contracts for every rule source (built-in rules, custom YAML rules
//! and, later, JS rules) and everything downstream of them (ignore filter, fix
//! engine, reporters, LSP and the Godot editor integration).
//!
//! FROZEN after W0: changes must be additive (new optional fields or new
//! helper functions) and approved by both tracks. See
//! `docs/specification_linter.md`, section "Shared contracts".

use std::path::{Path, PathBuf};

/// A position in a file. `line` is 1-based. `column` is 1-based and counted
/// in UTF-8 bytes, like tree-sitter. Reporters that need UTF-16 columns (SARIF,
/// LSP) convert at the edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    /// Computes the line and column of `byte` in `source`. `byte` may be equal
    /// to `source.len()` (the end of the file).
    pub fn from_byte(source: &str, byte: usize) -> Position {
        let bytes = source.as_bytes();
        let mut line = 1;
        let mut line_start = 0;
        let end = byte.min(bytes.len());
        for current_index in 0..end {
            if bytes[current_index] == b'\n' {
                line += 1;
                line_start = current_index + 1;
            }
        }
        Position {
            line,
            column: byte - line_start + 1,
        }
    }
}

/// A range in a file. The byte offsets are authoritative and the end is
/// exclusive. `start` and `end` are the matching line and column positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub start: Position,
    pub end: Position,
}

impl Span {
    /// The span covering a tree-sitter node.
    pub fn from_node(node: &tree_sitter::Node) -> Span {
        let start_point = node.start_position();
        let end_point = node.end_position();
        Span {
            start_byte: node.start_byte(),
            end_byte: node.end_byte(),
            start: Position {
                line: start_point.row + 1,
                column: start_point.column + 1,
            },
            end: Position {
                line: end_point.row + 1,
                column: end_point.column + 1,
            },
        }
    }

    /// The span covering `start_byte..end_byte` in `source`, for rules and
    /// fixes that work on text instead of nodes (line length, inserted text).
    pub fn from_byte_range(source: &str, start_byte: usize, end_byte: usize) -> Span {
        Span {
            start_byte,
            end_byte,
            start: Position::from_byte(source, start_byte),
            end: Position::from_byte(source, end_byte),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    /// The lowercase name used in config files, YAML rules and JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }

    /// Parses `error`, `warning` or `info`. Returns `None` for anything else,
    /// including `off`, which is a config level and not a severity.
    pub fn from_name(name: &str) -> Option<Severity> {
        match name {
            "error" => Some(Severity::Error),
            "warning" => Some(Severity::Warning),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FixSafety {
    /// Never changes behavior. Applied by `--fix`.
    Safe,
    /// Probably keeps behavior. Applied only with `--fix-unsafe`.
    Unsafe,
    /// Never applied automatically. Shown to people and agents.
    Suggestion,
}

impl FixSafety {
    /// The lowercase name used in YAML rules and JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            FixSafety::Safe => "safe",
            FixSafety::Unsafe => "unsafe",
            FixSafety::Suggestion => "suggestion",
        }
    }

    /// Parses `safe`, `unsafe` or `suggestion`.
    pub fn from_name(name: &str) -> Option<FixSafety> {
        match name {
            "safe" => Some(FixSafety::Safe),
            "unsafe" => Some(FixSafety::Unsafe),
            "suggestion" => Some(FixSafety::Suggestion),
            _ => None,
        }
    }
}

/// Replaces the text covered by `span` with `replacement`. An empty span is an
/// insertion; an empty replacement is a deletion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub span: Span,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fix {
    pub description: String,
    pub safety: FixSafety,
    /// Edits must not overlap each other.
    pub edits: Vec<Edit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Rule id, for example "prefer-is-empty". Built-in or custom.
    pub rule: String,
    pub severity: Severity,
    /// What is wrong, in one sentence.
    pub message: String,
    /// How to fix it, in one or two sentences.
    pub help: Option<String>,
    pub span: Span,
    /// Usually zero or one.
    pub fixes: Vec<Fix>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleOrigin {
    BuiltIn,
    /// A rule loaded from a file: YAML now, JS later.
    Custom {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleMeta {
    pub id: String,
    /// One line, used by `--list-rules`.
    pub description: String,
    /// Why the rule exists, used by `explain`.
    pub rationale: String,
    pub bad_example: String,
    pub good_example: String,
    pub default_severity: Severity,
    pub fixable: bool,
    pub origin: RuleOrigin,
}

/// Everything a rule source needs to check one file.
pub struct FileContext<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub tree: &'a tree_sitter::Tree,
}

/// Implemented by the custom rule engine (and later the JS engine). The
/// built-in rules keep their existing `Rule` trait and visitor.
pub trait RuleProvider {
    fn rules(&self) -> &[RuleMeta];
    fn check_file(&mut self, context: &FileContext) -> Vec<Diagnostic>;
}

/// Single-pass edit applier shared by the fix engine and the rule tester.
///
/// Edits are sorted by `start_byte` (then `end_byte`, so an insertion at the
/// start of a replaced range goes first). Edits with the same start keep their
/// input order. Two edits overlap when one starts before the previous one
/// ends; overlapping edits return an error, as do ranges outside the source or
/// not on UTF-8 character boundaries.
pub fn apply_edits(source: &str, edits: &[Edit]) -> Result<String, String> {
    let mut sorted_edits: Vec<&Edit> = Vec::with_capacity(edits.len());
    let mut added_length = 0;
    for edit in edits {
        let start = edit.span.start_byte;
        let end = edit.span.end_byte;
        if start > end {
            return Err(format!(
                "edit range {}..{} starts after it ends",
                start, end
            ));
        }
        if end > source.len() {
            return Err(format!(
                "edit range {}..{} is outside the source ({} bytes)",
                start,
                end,
                source.len()
            ));
        }
        if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
            return Err(format!(
                "edit range {}..{} is not on UTF-8 character boundaries",
                start, end
            ));
        }
        added_length += edit.replacement.len();
        sorted_edits.push(edit);
    }
    sorted_edits.sort_by(|left, right| {
        left.span
            .start_byte
            .cmp(&right.span.start_byte)
            .then(left.span.end_byte.cmp(&right.span.end_byte))
    });

    let mut output = String::with_capacity(source.len() + added_length);
    let mut copied_until = 0;
    for edit in sorted_edits {
        if edit.span.start_byte < copied_until {
            return Err(format!(
                "edit range {}..{} overlaps a previous edit ending at byte {}",
                edit.span.start_byte, edit.span.end_byte, copied_until
            ));
        }
        output.push_str(&source[copied_until..edit.span.start_byte]);
        output.push_str(&edit.replacement);
        copied_until = edit.span.end_byte;
    }
    output.push_str(&source[copied_until..]);
    Ok(output)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    fn edit(source: &str, start_byte: usize, end_byte: usize, replacement: &str) -> Edit {
        Edit {
            span: Span::from_byte_range(source, start_byte, end_byte),
            replacement: replacement.to_string(),
        }
    }

    #[test]
    fn apply_edits_with_no_edits_returns_source() {
        let source = "var a = 1\n";
        assert_eq!(apply_edits(source, &[]).unwrap(), source);
    }

    #[test]
    fn apply_edits_inserts_text() {
        let source = "var a = 1\n";
        let edits = [edit(source, 4, 4, "my_")];
        assert_eq!(apply_edits(source, &edits).unwrap(), "var my_a = 1\n");
    }

    #[test]
    fn apply_edits_deletes_text() {
        let source = "func f():\n\tpass\n\treturn\n";
        let edits = [edit(source, 10, 16, "")];
        assert_eq!(
            apply_edits(source, &edits).unwrap(),
            "func f():\n\treturn\n"
        );
    }

    #[test]
    fn apply_edits_replaces_text() {
        let source = "if len(items) == 0:\n";
        let edits = [edit(source, 3, 18, "items.is_empty()")];
        assert_eq!(
            apply_edits(source, &edits).unwrap(),
            "if items.is_empty():\n"
        );
    }

    #[test]
    fn apply_edits_sorts_edits_given_out_of_order() {
        let source = "a b c";
        let edits = [
            edit(source, 4, 5, "C"),
            edit(source, 0, 1, "A"),
            edit(source, 2, 3, "B"),
        ];
        assert_eq!(apply_edits(source, &edits).unwrap(), "A B C");
    }

    #[test]
    fn apply_edits_rejects_overlapping_edits() {
        let source = "abcdef";
        let edits = [edit(source, 1, 4, "x"), edit(source, 3, 5, "y")];
        let error = apply_edits(source, &edits).unwrap_err();
        assert_eq!(
            error,
            "edit range 3..5 overlaps a previous edit ending at byte 4"
        );
    }

    #[test]
    fn apply_edits_allows_adjacent_edits() {
        let source = "abcdef";
        let edits = [edit(source, 3, 6, "Y"), edit(source, 0, 3, "X")];
        assert_eq!(apply_edits(source, &edits).unwrap(), "XY");
    }

    #[test]
    fn apply_edits_puts_insertion_before_replacement_at_same_start() {
        let source = "abc";
        let edits = [edit(source, 0, 3, "X"), edit(source, 0, 0, "<")];
        assert_eq!(apply_edits(source, &edits).unwrap(), "<X");
    }

    #[test]
    fn apply_edits_keeps_input_order_for_insertions_at_same_position() {
        let source = "ab";
        let edits = [edit(source, 1, 1, "1"), edit(source, 1, 1, "2")];
        assert_eq!(apply_edits(source, &edits).unwrap(), "a12b");
    }

    #[test]
    fn apply_edits_at_end_of_file() {
        let source = "var a = 1";
        let edits = [edit(source, source.len(), source.len(), "\n")];
        assert_eq!(apply_edits(source, &edits).unwrap(), "var a = 1\n");
    }

    #[test]
    fn apply_edits_rejects_range_past_end_of_file() {
        let source = "abc";
        let edits = [edit(source, 2, 3, "")];
        let mut out_of_range = edits[0].clone();
        out_of_range.span.end_byte = 4;
        let error = apply_edits(source, &[out_of_range]).unwrap_err();
        assert_eq!(error, "edit range 2..4 is outside the source (3 bytes)");
    }

    #[test]
    fn apply_edits_handles_multi_byte_text() {
        // "é" and "ñ" are two bytes each, "🎮" is four.
        let source = "var café = \"🎮\"\nvar niño = 1\n";
        let cafe_start = source.find("café").unwrap();
        let emoji_start = source.find('🎮').unwrap();
        let nino_start = source.find("niño").unwrap();
        let edits = [
            edit(source, nino_start, nino_start + "niño".len(), "kid"),
            edit(source, emoji_start, emoji_start + '🎮'.len_utf8(), "pad"),
            edit(source, cafe_start, cafe_start + "café".len(), "coffee"),
        ];
        assert_eq!(
            apply_edits(source, &edits).unwrap(),
            "var coffee = \"pad\"\nvar kid = 1\n"
        );
    }

    #[test]
    fn apply_edits_rejects_range_inside_a_character() {
        let source = "é";
        let mut inside = edit(source, 0, 0, "x");
        inside.span.start_byte = 1;
        inside.span.end_byte = 1;
        let error = apply_edits(source, &[inside]).unwrap_err();
        assert_eq!(
            error,
            "edit range 1..1 is not on UTF-8 character boundaries"
        );
    }

    #[test]
    fn position_from_byte_counts_lines_and_utf8_columns() {
        let source = "ab\ncé\nx";
        assert_eq!(
            Position::from_byte(source, 0),
            Position { line: 1, column: 1 }
        );
        assert_eq!(
            Position::from_byte(source, 3),
            Position { line: 2, column: 1 }
        );
        // "é" is two bytes, so "x" after the newline is on line 3.
        let x_byte = source.find('x').unwrap();
        assert_eq!(
            Position::from_byte(source, x_byte),
            Position { line: 3, column: 1 }
        );
        // Column after "cé" counts bytes: c (1) + é (2) = 3 bytes, so column 4.
        assert_eq!(
            Position::from_byte(source, 6),
            Position { line: 2, column: 4 }
        );
        assert_eq!(
            Position::from_byte(source, source.len()),
            Position { line: 3, column: 2 }
        );
    }

    #[test]
    fn span_from_node_matches_byte_range() {
        let source = "var café = 1\nfunc f():\n\tpass\n";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_gdscript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();
        let function = root.named_child(1).unwrap();
        assert_eq!(function.kind(), "function_definition");
        let span = Span::from_node(&function);
        assert_eq!(span.start_byte, source.find("func").unwrap());
        assert_eq!(span.start, Position { line: 2, column: 1 });
        assert_eq!(
            span,
            Span::from_byte_range(source, span.start_byte, span.end_byte)
        );
    }

    #[test]
    fn severity_and_fix_safety_names_round_trip() {
        for severity in [Severity::Error, Severity::Warning, Severity::Info] {
            assert_eq!(Severity::from_name(severity.as_str()), Some(severity));
        }
        assert_eq!(Severity::from_name("off"), None);
        for safety in [FixSafety::Safe, FixSafety::Unsafe, FixSafety::Suggestion] {
            assert_eq!(FixSafety::from_name(safety.as_str()), Some(safety));
        }
        assert_eq!(FixSafety::from_name("maybe"), None);
    }
}
