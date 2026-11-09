//! Owner: Track A (W4). Inline ignore comments and the `unused-ignore` rule.
//!
//! A directive starts a comment (after `#` and optional spaces):
//!
//! ```gdscript
//! # gdguard-ignore-file prefer-is-empty
//! var x = 1  # gdguard-ignore no-magic-number -- reason text is allowed
//! # gdguard-ignore-next-line private-access
//! ```
//!
//! - `-file` applies to the whole file, `-next-line` to the line after the
//!   comment, and `-line` or no suffix to the comment's own line.
//! - Rule ids are separated by commas and/or spaces. Without ids, the
//!   comment ignores every rule.
//! - Text after `--` is a reason and isn't parsed.
//! - The legacy `gdlint-ignore*` prefixes work the same way.
//!
//! A comment, or a rule id in it, that suppresses no diagnostic gets an
//! `unused-ignore` warning with a safe fix that removes it.

use std::collections::HashSet;

use tree_sitter::{Node, Tree};

use crate::linter::diagnostic::{Diagnostic, Edit, Fix, FixSafety, Severity, Span};
use crate::linter::rules::fix_helpers::{line_end, line_start, remove_lines_edit};

/// The rule id of the warning for ignore comments that suppress nothing.
pub const UNUSED_IGNORE_RULE_ID: &str = "unused-ignore";

/// Which lines an ignore comment covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoreScope {
    /// The whole file.
    File,
    /// One line, 1-based.
    Line(usize),
}

/// One parsed ignore comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnoreComment {
    pub scope: IgnoreScope,
    /// The directive as written, for example `gdguard-ignore-next-line`.
    pub directive: String,
    /// Rule ids, in written order without duplicates. Empty means every rule.
    pub rules: Vec<String>,
    /// The text after `--`, trimmed, if any.
    pub reason: Option<String>,
    /// The span of the whole comment, `#` included.
    pub span: Span,
}

/// Directives, longest first so a prefix never shadows a longer one.
const DIRECTIVES: [&str; 8] = [
    "gdguard-ignore-next-line",
    "gdguard-ignore-file",
    "gdguard-ignore-line",
    "gdguard-ignore",
    "gdlint-ignore-next-line",
    "gdlint-ignore-file",
    "gdlint-ignore-line",
    "gdlint-ignore",
];

/// A directive parsed from the text of one comment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDirective {
    pub directive: &'static str,
    pub rules: Vec<String>,
    pub reason: Option<String>,
}

/// Parses the text of one comment, `#` included. Returns None when the
/// comment isn't an ignore directive.
pub fn parse_directive(comment_text: &str) -> Option<ParsedDirective> {
    let text = comment_text.strip_prefix('#')?.trim_start();
    let mut found: Option<&'static str> = None;
    for directive in DIRECTIVES {
        if let Some(rest) = text.strip_prefix(directive) {
            // `gdguard-ignored` or `gdguard-ignore:x` isn't a directive.
            let is_word_end = match rest.chars().next() {
                Some(character) => character.is_whitespace() || character == ',',
                None => true,
            };
            if is_word_end {
                found = Some(directive);
                break;
            }
        }
    }
    let directive = found?;
    let rest = &text[directive.len()..];

    let (rules_text, reason) = match find_reason_separator(rest) {
        Some(index) => {
            let reason = rest[index + 2..].trim();
            let reason = if reason.is_empty() {
                None
            } else {
                Some(reason.to_string())
            };
            (&rest[..index], reason)
        }
        None => (rest, None),
    };
    let mut rules: Vec<String> = Vec::new();
    for part in rules_text.split([',', ' ', '\t']) {
        let rule = part.trim();
        if !rule.is_empty() && !contains_rule(&rules, rule) {
            rules.push(rule.to_string());
        }
    }
    Some(ParsedDirective {
        directive,
        rules,
        reason,
    })
}

fn contains_rule(rules: &[String], rule: &str) -> bool {
    for existing in rules {
        if existing == rule {
            return true;
        }
    }
    false
}

/// The byte index of a `--` that starts a reason: at the start of `text` or
/// after whitespace.
fn find_reason_separator(text: &str) -> Option<usize> {
    let mut search_start = 0;
    while let Some(offset) = text[search_start..].find("--") {
        let index = search_start + offset;
        let is_after_space = index == 0
            || text[..index]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        if is_after_space {
            return Some(index);
        }
        search_start = index + 2;
    }
    None
}

/// Collects every ignore comment in a parsed file.
pub fn parse_ignore_comments(source: &str, tree: &Tree) -> Vec<IgnoreComment> {
    let mut comments = Vec::new();
    collect_ignore_comments(&tree.root_node(), source, &mut comments);
    comments
}

fn collect_ignore_comments(node: &Node, source: &str, comments: &mut Vec<IgnoreComment>) {
    if node.kind() == "comment" {
        let text = &source[node.start_byte()..node.end_byte()];
        if let Some(parsed) = parse_directive(text) {
            let comment_line = node.start_position().row + 1;
            let scope = if parsed.directive.ends_with("-file") {
                IgnoreScope::File
            } else if parsed.directive.ends_with("-next-line") {
                IgnoreScope::Line(comment_line + 1)
            } else {
                IgnoreScope::Line(comment_line)
            };
            comments.push(IgnoreComment {
                scope,
                directive: parsed.directive.to_string(),
                rules: parsed.rules,
                reason: parsed.reason,
                span: Span::from_node(node),
            });
        }
        return;
    }
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            collect_ignore_comments(&cursor.node(), source, comments);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
}

/// True when `comment` covers `diagnostic` (scope and rule id).
fn covers(comment: &IgnoreComment, diagnostic: &Diagnostic) -> bool {
    let is_in_scope = match comment.scope {
        IgnoreScope::File => true,
        IgnoreScope::Line(line) => line == diagnostic.span.start.line,
    };
    if !is_in_scope {
        return false;
    }
    comment.rules.is_empty()
        || comment
            .rules
            .iter()
            .any(|rule| diagnostic.matches_rule(rule))
}

/// Removes the diagnostics that ignore comments cover. When `report_unused`
/// is true, appends an `unused-ignore` warning to `unused` for each comment
/// with a rule id (or, for comments without ids, the comment itself) that
/// suppressed nothing.
///
/// Rule ids in `disabled_rules` (level `off`, `--disable`) are never
/// reported: the comment may matter again when the rule is turned back on.
/// `unused-ignore` warnings are only hidden by a file-level comment that
/// names `unused-ignore`; comments naming it are never reported.
pub fn apply_ignores(
    source: &str,
    comments: &[IgnoreComment],
    report_unused: bool,
    disabled_rules: &HashSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
    unused: &mut Vec<Diagnostic>,
) {
    if comments.is_empty() {
        return;
    }
    // used[comment][rule]; comments without ids use one slot.
    let mut used: Vec<Vec<bool>> = Vec::with_capacity(comments.len());
    for comment in comments {
        used.push(vec![false; comment.rules.len().max(1)]);
    }
    let mut kept = Vec::with_capacity(diagnostics.len());
    for diagnostic in diagnostics.drain(..) {
        let mut is_ignored = false;
        for (comment_index, comment) in comments.iter().enumerate() {
            if !covers(comment, &diagnostic) {
                continue;
            }
            is_ignored = true;
            if comment.rules.is_empty() {
                used[comment_index][0] = true;
            } else {
                for (rule_index, rule) in comment.rules.iter().enumerate() {
                    if diagnostic.matches_rule(rule) {
                        used[comment_index][rule_index] = true;
                    }
                }
            }
        }
        if !is_ignored {
            kept.push(diagnostic);
        }
    }
    *diagnostics = kept;

    if !report_unused {
        return;
    }
    let mut is_unused_ignore_hidden = false;
    for comment in comments {
        if comment.scope == IgnoreScope::File
            && contains_rule(&comment.rules, UNUSED_IGNORE_RULE_ID)
        {
            is_unused_ignore_hidden = true;
        }
    }
    if is_unused_ignore_hidden {
        return;
    }
    for (comment_index, comment) in comments.iter().enumerate() {
        if contains_rule(&comment.rules, UNUSED_IGNORE_RULE_ID) {
            continue;
        }
        let comment_used = &used[comment_index];
        let mut unused_rules: Vec<&str> = Vec::new();
        let mut used_rules: Vec<&str> = Vec::new();
        for (rule_index, rule) in comment.rules.iter().enumerate() {
            if comment_used[rule_index] || super::rule_groups::is_disabled(disabled_rules, rule) {
                used_rules.push(rule);
            } else {
                unused_rules.push(rule);
            }
        }
        if comment.rules.is_empty() {
            if !comment_used[0] {
                unused.push(unused_ignore_diagnostic(source, comment, &[], &[]));
            }
        } else if !unused_rules.is_empty() {
            unused.push(unused_ignore_diagnostic(
                source,
                comment,
                &unused_rules,
                &used_rules,
            ));
        }
    }
}

/// Builds the `unused-ignore` warning. With `used_rules` empty, the fix
/// removes the whole comment; otherwise it rewrites the comment with only
/// `used_rules`.
fn unused_ignore_diagnostic(
    source: &str,
    comment: &IgnoreComment,
    unused_rules: &[&str],
    used_rules: &[&str],
) -> Diagnostic {
    let message = if unused_rules.is_empty() {
        format!("`{}` doesn't suppress any issue", comment.directive)
    } else {
        let mut names = String::new();
        for (index, rule) in unused_rules.iter().enumerate() {
            if index > 0 {
                names.push_str(", ");
            }
            names.push('`');
            names.push_str(rule);
            names.push('`');
        }
        format!(
            "`{}` for {} doesn't suppress any issue",
            comment.directive, names
        )
    };

    let mut fixes = Vec::new();
    if used_rules.is_empty() {
        if let Some(edit) = remove_comment_edit(source, &comment.span) {
            fixes.push(Fix {
                description: "Remove the ignore comment".to_string(),
                safety: FixSafety::Safe,
                edits: vec![edit],
            });
        }
    } else {
        let mut replacement = format!("# {} {}", comment.directive, used_rules.join(", "));
        if let Some(reason) = &comment.reason {
            replacement.push_str(" -- ");
            replacement.push_str(reason);
        }
        fixes.push(Fix {
            description: "Remove the unused rule ids from the comment".to_string(),
            safety: FixSafety::Safe,
            edits: vec![Edit {
                span: comment.span,
                replacement,
            }],
        });
    }

    Diagnostic {
        check: None,
        rule: UNUSED_IGNORE_RULE_ID.to_string(),
        severity: Severity::Warning,
        message,
        help: Some(
            "Remove the comment or the rule ids that no longer apply, so ignores don't hide future issues."
                .to_string(),
        ),
        span: comment.span,
        fixes,
    }
}

/// An edit that removes a comment: the whole line when the comment is alone
/// on it, else the comment and the spaces before it.
fn remove_comment_edit(source: &str, span: &Span) -> Option<Edit> {
    if let Some(edit) = remove_lines_edit(source, span.start_byte, span.end_byte) {
        return Some(edit);
    }
    let start_of_line = line_start(source, span.start_byte);
    let before = &source[start_of_line..span.start_byte];
    let trimmed = before.trim_end_matches([' ', '\t']);
    let remove_start = start_of_line + trimmed.len();
    let remove_end = span.end_byte.min(line_end(source, span.start_byte));
    Some(Edit {
        span: Span::from_byte_range(source, remove_start, remove_end),
        replacement: String::new(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use crate::linter::diagnostic::apply_edits;

    fn parse(source: &str) -> Vec<IgnoreComment> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_gdscript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        parse_ignore_comments(source, &tree)
    }

    fn rules(comment: &IgnoreComment) -> Vec<&str> {
        let mut names = Vec::new();
        for rule in &comment.rules {
            names.push(rule.as_str());
        }
        names
    }

    fn diagnostic(source: &str, rule: &str, needle: &str) -> Diagnostic {
        let start = source.find(needle).unwrap();
        Diagnostic {
            check: None,
            rule: rule.to_string(),
            severity: Severity::Warning,
            message: String::new(),
            help: None,
            span: Span::from_byte_range(source, start, start + needle.len()),
            fixes: Vec::new(),
        }
    }

    #[test]
    fn parses_next_line_with_one_rule() {
        let comments = parse("# gdlint-ignore-next-line private-access\nobj._private_method()\n");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].scope, IgnoreScope::Line(2));
        assert_eq!(rules(&comments[0]), vec!["private-access"]);
    }

    #[test]
    fn parses_next_line_with_several_rules() {
        let comments = parse(
            "# gdlint-ignore-next-line private-access,constant-name\nobj._private_method()\n",
        );
        assert_eq!(rules(&comments[0]), vec!["private-access", "constant-name"]);
    }

    #[test]
    fn parses_current_line() {
        let comments = parse("obj._private_method() # gdlint-ignore private-access\n");
        assert_eq!(comments[0].scope, IgnoreScope::Line(1));
        assert_eq!(rules(&comments[0]), vec!["private-access"]);
    }

    #[test]
    fn no_rule_ids_means_every_rule() {
        let comments = parse("# gdlint-ignore-next-line\nsome_problematic_code()\n");
        assert!(comments[0].rules.is_empty());
    }

    #[test]
    fn parses_rules_separated_by_spaces_and_commas() {
        let comments = parse(
            "# gdlint-ignore-next-line private-access , constant-name  ,  other-rule\nsome_code()\n",
        );
        assert_eq!(
            rules(&comments[0]),
            vec!["private-access", "constant-name", "other-rule"]
        );
    }

    #[test]
    fn parses_every_gdguard_form_and_reasons() {
        let source = "# gdguard-ignore-file prefer-is-empty\nvar x = 1  # gdguard-ignore no-magic-number -- reason -- with dashes\n# gdguard-ignore-next-line private-access\nvar y = 2 # gdguard-ignore-line a -- \n# gdguard-ignore -- only a reason\n";
        let comments = parse(source);
        assert_eq!(comments.len(), 5);
        assert_eq!(comments[0].scope, IgnoreScope::File);
        assert_eq!(rules(&comments[0]), vec!["prefer-is-empty"]);
        assert_eq!(comments[1].scope, IgnoreScope::Line(2));
        assert_eq!(rules(&comments[1]), vec!["no-magic-number"]);
        assert_eq!(comments[1].reason.as_deref(), Some("reason -- with dashes"));
        assert_eq!(comments[2].scope, IgnoreScope::Line(4));
        assert_eq!(comments[3].scope, IgnoreScope::Line(4));
        assert_eq!(comments[3].reason, None);
        assert!(comments[4].rules.is_empty());
        assert_eq!(comments[4].reason.as_deref(), Some("only a reason"));
    }

    #[test]
    fn ignores_non_directives_and_strings() {
        let source = "var s = \"# gdguard-ignore x\"\n# gdguard-ignored x\n# note: gdguard-ignore x\n#gdguard-ignore-file\n";
        let comments = parse(source);
        // Only `#gdguard-ignore-file` (no space after `#`) is a directive.
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].scope, IgnoreScope::File);
    }

    #[test]
    fn file_ignores_cover_every_line() {
        let source = "# gdguard-ignore-file a\nvar x = 1\nvar y = 2\n";
        let comments = parse(source);
        let mut diagnostics = vec![
            diagnostic(source, "a", "var x"),
            diagnostic(source, "a", "var y"),
            diagnostic(source, "b", "var y"),
        ];
        let mut unused = Vec::new();
        apply_ignores(
            source,
            &comments,
            true,
            &HashSet::new(),
            &mut diagnostics,
            &mut unused,
        );
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule, "b");
        assert!(unused.is_empty());
    }

    #[test]
    fn reports_unused_comments_and_unused_rule_ids() {
        let source = "var x = 1 # gdguard-ignore a, b -- why\n# gdguard-ignore-next-line\nvar y = 2\nvar z = 3  # gdlint-ignore c\n";
        let comments = parse(source);
        let mut diagnostics = vec![diagnostic(source, "a", "var x")];
        let mut unused = Vec::new();
        apply_ignores(
            source,
            &comments,
            true,
            &HashSet::new(),
            &mut diagnostics,
            &mut unused,
        );
        assert!(diagnostics.is_empty());
        assert_eq!(unused.len(), 3);
        assert_eq!(
            unused[0].message,
            "`gdguard-ignore` for `b` doesn't suppress any issue"
        );
        assert_eq!(
            unused[1].message,
            "`gdguard-ignore-next-line` doesn't suppress any issue"
        );
        assert_eq!(
            unused[2].message,
            "`gdlint-ignore` for `c` doesn't suppress any issue"
        );

        let mut edits = Vec::new();
        for warning in &unused {
            assert_eq!(warning.rule, UNUSED_IGNORE_RULE_ID);
            assert_eq!(warning.fixes[0].safety, FixSafety::Safe);
            edits.extend(warning.fixes[0].edits.iter().cloned());
        }
        assert_eq!(
            apply_edits(source, &edits).unwrap(),
            "var x = 1 # gdguard-ignore a -- why\nvar y = 2\nvar z = 3\n"
        );
    }

    #[test]
    fn unused_reports_can_be_turned_off_or_hidden_per_file() {
        let source = "var x = 1 # gdguard-ignore a\n";
        let comments = parse(source);
        let mut unused = Vec::new();
        apply_ignores(
            source,
            &comments,
            false,
            &HashSet::new(),
            &mut Vec::new(),
            &mut unused,
        );
        assert!(unused.is_empty());

        let hidden = "# gdguard-ignore-file unused-ignore\nvar x = 1 # gdguard-ignore a\n";
        let comments = parse(hidden);
        apply_ignores(
            hidden,
            &comments,
            true,
            &HashSet::new(),
            &mut Vec::new(),
            &mut unused,
        );
        assert!(unused.is_empty());

        // A comment for a disabled rule isn't reported.
        let mut disabled = HashSet::new();
        disabled.insert("a".to_string());
        let comments = parse(source);
        apply_ignores(
            source,
            &comments,
            true,
            &disabled,
            &mut Vec::new(),
            &mut unused,
        );
        assert!(unused.is_empty());
    }
}
