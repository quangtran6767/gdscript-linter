//! Text helpers that built-in rules use to build their fixes.

use crate::linter::diagnostic::{Edit, Span};

/// Byte offset where the line containing `byte` starts.
pub fn line_start(source: &str, byte: usize) -> usize {
    match source[..byte].rfind('\n') {
        Some(newline) => newline + 1,
        None => 0,
    }
}

/// Byte offset of the `\n` ending the line containing `byte`, or the source
/// length on the last line.
pub fn line_end(source: &str, byte: usize) -> usize {
    match source[byte..].find('\n') {
        Some(offset) => byte + offset,
        None => source.len(),
    }
}

/// The whitespace at the start of the line containing `byte`.
pub fn line_indentation(source: &str, byte: usize) -> &str {
    let start = line_start(source, byte);
    let line = &source[start..line_end(source, byte)];
    let trimmed = line.trim_start_matches([' ', '\t']);
    &line[..line.len() - trimmed.len()]
}

/// An edit that deletes the lines holding `start_byte..end_byte`, when
/// nothing but whitespace shares those lines with the range. Returns None
/// otherwise, for example when a comment follows on the same line.
pub fn remove_lines_edit(source: &str, start_byte: usize, end_byte: usize) -> Option<Edit> {
    let first_line_start = line_start(source, start_byte);
    let last_line_end = line_end(source, end_byte);
    if !source[first_line_start..start_byte].trim().is_empty()
        || !source[end_byte..last_line_end].trim().is_empty()
    {
        return None;
    }
    // Delete the line and its newline. On a last line without a newline,
    // delete the newline before it instead.
    let (remove_start, remove_end) = if last_line_end < source.len() {
        (first_line_start, last_line_end + 1)
    } else if first_line_start > 0 {
        (first_line_start - 1, last_line_end)
    } else {
        (first_line_start, last_line_end)
    };
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

    fn remove(source: &str, needle: &str) -> Option<String> {
        let start = source.find(needle).unwrap();
        let edit = remove_lines_edit(source, start, start + needle.len())?;
        Some(apply_edits(source, &[edit]).unwrap())
    }

    #[test]
    fn removes_a_line_with_its_newline() {
        assert_eq!(remove("a\n\tpass\nb\n", "pass").as_deref(), Some("a\nb\n"));
    }

    #[test]
    fn removes_the_last_line_without_a_newline() {
        assert_eq!(remove("a\n\tpass", "pass").as_deref(), Some("a"));
        assert_eq!(remove("pass", "pass").as_deref(), Some(""));
    }

    #[test]
    fn keeps_lines_shared_with_other_code_or_comments() {
        assert_eq!(remove("a\n\tpass # why\n", "pass"), None);
        assert_eq!(remove("if x: pass\n", "pass"), None);
    }

    #[test]
    fn handles_crlf_line_endings() {
        assert_eq!(
            remove("a\r\n\tpass\r\nb\r\n", "pass").as_deref(),
            Some("a\r\nb\r\n")
        );
    }

    #[test]
    fn finds_line_indentation() {
        let source = "func f():\n\t\tvar a = 1\n";
        assert_eq!(
            line_indentation(source, source.find("var").unwrap()),
            "\t\t"
        );
        assert_eq!(line_indentation(source, 0), "");
    }
}
