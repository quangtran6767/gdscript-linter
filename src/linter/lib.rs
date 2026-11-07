use tree_sitter::Node;

pub fn get_node_text<'a>(node: &Node, source_code: &'a str) -> &'a str {
    &source_code[node.start_byte()..node.end_byte()]
}

/// Splits an identifier into lowercase words. Words are separated by
/// underscores and by case changes: `moveTowards` gives `move`, `towards`,
/// and `HTTPServer` gives `http`, `server`. Leading underscores are not part
/// of any word.
fn split_identifier_words(name: &str) -> Vec<String> {
    let characters: Vec<char> = name.chars().collect();
    let mut words: Vec<String> = Vec::new();
    let mut current_word = String::new();
    for current_index in 0..characters.len() {
        let character = characters[current_index];
        if character == '_' {
            if !current_word.is_empty() {
                words.push(current_word);
                current_word = String::new();
            }
            continue;
        }
        if character.is_uppercase() && !current_word.is_empty() {
            let previous = characters[current_index - 1];
            let next_is_lowercase = current_index + 1 < characters.len()
                && characters[current_index + 1].is_lowercase();
            // A new word starts at "aB" and at the last capital of an acronym
            // followed by a lowercase letter ("HTTPServer").
            if previous.is_lowercase()
                || previous.is_ascii_digit()
                || (previous.is_uppercase() && next_is_lowercase)
            {
                words.push(current_word);
                current_word = String::new();
            }
        }
        for lowercase_character in character.to_lowercase() {
            current_word.push(lowercase_character);
        }
    }
    if !current_word.is_empty() {
        words.push(current_word);
    }
    words
}

/// Returns the leading underscores of `name`, which mark private members.
fn leading_underscores(name: &str) -> &str {
    let trimmed = name.trim_start_matches('_');
    &name[..name.len() - trimmed.len()]
}

/// `moveTowards` -> `move_towards`, `_MyVar` -> `_my_var`.
pub fn to_snake_case(name: &str) -> String {
    let words = split_identifier_words(name);
    let mut result = String::with_capacity(name.len() + words.len());
    result.push_str(leading_underscores(name));
    for (word_index, word) in words.iter().enumerate() {
        if word_index > 0 {
            result.push('_');
        }
        result.push_str(word);
    }
    result
}

/// `bad_class_name` -> `BadClassName`. Leading underscores are dropped.
pub fn to_pascal_case(name: &str) -> String {
    let words = split_identifier_words(name);
    let mut result = String::with_capacity(name.len());
    for word in &words {
        let mut characters = word.chars();
        if let Some(first_character) = characters.next() {
            for uppercase_character in first_character.to_uppercase() {
                result.push(uppercase_character);
            }
            result.push_str(characters.as_str());
        }
    }
    result
}

/// `maxSpeed` -> `MAX_SPEED`, `_secret` -> `_SECRET`.
pub fn to_constant_case(name: &str) -> String {
    let snake_case = to_snake_case(name);
    snake_case.to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_identifier_cases() {
        assert_eq!(to_snake_case("moveTowards"), "move_towards");
        assert_eq!(to_snake_case("_MyVar"), "_my_var");
        assert_eq!(to_snake_case("HTTPServer"), "http_server");
        assert_eq!(to_snake_case("OnDefeated"), "on_defeated");
        assert_eq!(to_snake_case("level2Boss"), "level2_boss");
        assert_eq!(to_pascal_case("bad_class_name"), "BadClassName");
        assert_eq!(to_pascal_case("BAD_CLASS_NAME"), "BadClassName");
        assert_eq!(to_pascal_case("badClassName"), "BadClassName");
        assert_eq!(to_constant_case("maxSpeed"), "MAX_SPEED");
        assert_eq!(to_constant_case("chase_player"), "CHASE_PLAYER");
        assert_eq!(to_constant_case("_secret"), "_SECRET");
    }
}
