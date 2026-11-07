//! ast-grep `Language` implementation for GDScript.
//!
//! GDScript uses `$` for node paths (`$Player`, `$"UI/Label"`), which is also
//! ast-grep's metavariable character. To keep both working, rule text is
//! rewritten before ast-grep sees it (see [`to_internal`]):
//!
//! - `$NAME`, `$$NAME`, `$$$NAME` and `$$$`, where `NAME` is made of uppercase
//!   letters, digits and `_`, are metavariables. Their `$` signs become
//!   [`EXPANDO_CHAR`], which GDScript accepts in identifiers, so the pattern
//!   still parses.
//! - Any other `$` is kept, so `$Player`, `$"Path"` and `$%Unique` are literal
//!   GDScript node paths.
//! - `\$` is always a literal `$`. Use it for all-caps node paths: `\$HUD`.
//!
//! The same rewrite applies to patterns, fix templates and messages, so the
//! rules for `$` are identical everywhere in a rule file.

use std::borrow::Cow;

use ast_grep_core::Language;
use ast_grep_core::matcher::{Pattern, PatternBuilder, PatternError};
use ast_grep_core::tree_sitter::{LanguageExt, StrDoc, TSLanguage};

/// The character metavariables use internally. `µ` (U+00B5) is a Unicode
/// letter (XID_Start), so `µX` parses as a GDScript identifier. Rule authors
/// never type it.
pub const EXPANDO_CHAR: char = 'µ';

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GdScript;

impl Language for GdScript {
    fn pre_process_pattern<'q>(&self, query: &'q str) -> Cow<'q, str> {
        to_internal(query)
    }

    fn meta_var_char(&self) -> char {
        EXPANDO_CHAR
    }

    fn expando_char(&self) -> char {
        EXPANDO_CHAR
    }

    fn kind_to_id(&self, kind: &str) -> u16 {
        self.get_ts_language().id_for_node_kind(kind, true)
    }

    fn field_to_id(&self, field: &str) -> Option<u16> {
        let id = self.get_ts_language().field_id_for_name(field)?;
        Some(id.get())
    }

    fn build_pattern(&self, builder: &PatternBuilder) -> Result<Pattern, PatternError> {
        builder.build(|source| StrDoc::try_new(source, *self))
    }
}

impl LanguageExt for GdScript {
    fn get_ts_language(&self) -> TSLanguage {
        tree_sitter_gdscript::LANGUAGE.into()
    }
}

fn is_metavariable_first_char(character: char) -> bool {
    character.is_ascii_uppercase() || character == '_'
}

fn is_metavariable_char(character: char) -> bool {
    is_metavariable_first_char(character) || character.is_ascii_digit()
}

fn is_identifier_char(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

/// Rewrites user-facing rule text (pattern, fix template or message) into the
/// form ast-grep sees: metavariable `$` signs become [`EXPANDO_CHAR`], `\$`
/// becomes a literal `$`, and every other `$` stays as is. Returns the input
/// unchanged when it has no `$`.
pub fn to_internal(text: &str) -> Cow<'_, str> {
    if !text.contains('$') {
        return Cow::Borrowed(text);
    }
    let characters: Vec<char> = text.chars().collect();
    let mut output = String::with_capacity(text.len() + 8);
    let mut index = 0;
    while index < characters.len() {
        let character = characters[index];
        if character == '\\' && index + 1 < characters.len() && characters[index + 1] == '$' {
            output.push('$');
            index += 2;
            continue;
        }
        if character != '$' {
            output.push(character);
            index += 1;
            continue;
        }

        let mut dollar_count = 0;
        while dollar_count < 3
            && index + dollar_count < characters.len()
            && characters[index + dollar_count] == '$'
        {
            dollar_count += 1;
        }
        let name_start = index + dollar_count;
        let mut name_end = name_start;
        while name_end < characters.len() && is_metavariable_char(characters[name_end]) {
            name_end += 1;
        }
        let has_name = name_end > name_start;
        let name_is_valid = has_name && is_metavariable_first_char(characters[name_start]);
        let followed_by_identifier =
            name_end < characters.len() && is_identifier_char(characters[name_end]);
        // `$$$` alone is ast-grep's anonymous multi-match.
        let is_metavariable = if dollar_count == 3 && !has_name {
            !followed_by_identifier
        } else {
            name_is_valid && !followed_by_identifier
        };

        if is_metavariable {
            for _ in 0..dollar_count {
                output.push(EXPANDO_CHAR);
            }
            for name_index in name_start..name_end {
                output.push(characters[name_index]);
            }
            index = name_end;
        } else {
            // A literal node path: keep one `$` and look at the rest again.
            output.push('$');
            index += 1;
        }
    }
    Cow::Owned(output)
}

/// Rewrites text produced by ast-grep (for example a rendered fix) back for
/// the user. Only needed for text that can still contain [`EXPANDO_CHAR`],
/// which happens when a template names a metavariable the match didn't bind.
pub fn from_internal(text: &str) -> Cow<'_, str> {
    if !text.contains(EXPANDO_CHAR) {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace(EXPANDO_CHAR, "$"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metavariables_use_the_expando_char() {
        assert_eq!(to_internal("len($X) == 0"), "len(µX) == 0");
        assert_eq!(to_internal("f($$$ARGS)"), "f(µµµARGS)");
        assert_eq!(to_internal("f($$$)"), "f(µµµ)");
        assert_eq!(to_internal("f($_)"), "f(µ_)");
        assert_eq!(to_internal("$$A"), "µµA");
        assert_eq!(to_internal("$A1 + $B_2"), "µA1 + µB_2");
    }

    #[test]
    fn node_paths_stay_literal() {
        assert_eq!(to_internal("$Player.hide()"), "$Player.hide()");
        assert_eq!(to_internal("$\"UI/Label\""), "$\"UI/Label\"");
        assert_eq!(to_internal("$%Unique"), "$%Unique");
        assert_eq!(to_internal("$player"), "$player");
        assert_eq!(to_internal("$HUd"), "$HUd");
        assert_eq!(to_internal("$$Player"), "$$Player");
    }

    #[test]
    fn backslash_escapes_an_all_caps_node_path() {
        assert_eq!(to_internal("\\$HUD.show()"), "$HUD.show()");
        assert_eq!(to_internal("$X == \\$UI"), "µX == $UI");
    }

    #[test]
    fn text_without_dollar_is_borrowed() {
        assert!(matches!(to_internal("len(x)"), Cow::Borrowed(_)));
    }

    #[test]
    fn from_internal_restores_dollar_signs() {
        assert_eq!(from_internal("µX.is_empty()"), "$X.is_empty()");
        assert_eq!(from_internal("plain"), "plain");
    }
}
