//! A dictionary literal using the same key twice: the later value silently
//! replaces the earlier one. Godot rejects duplicate literal keys at load.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, Severity, Span};
use crate::node_kind::GDScriptNodeKind;
use tree_sitter::Node;

pub struct DuplicateDictionaryKeyRule;
impl Rule for DuplicateDictionaryKeyRule {
    fn get_target_ast_nodes(&self) -> &[GDScriptNodeKind] {
        &[GDScriptNodeKind::Dictionary]
    }

    fn check_node(&mut self, node: &Node, source_code: &str, diagnostics: &mut Vec<Diagnostic>) {
        let mut seen: Vec<(String, usize)> = Vec::new();
        for index in 0..node.named_child_count() {
            let Some(pair) = node.named_child(index as u32) else {
                continue;
            };
            if pair.kind() != "pair" {
                continue;
            }
            let Some(key) = pair.child_by_field_name("left") else {
                continue;
            };
            let lua_style = pair
                .child(1)
                .is_some_and(|separator| separator.kind() == "=");
            let text = &source_code[key.byte_range()];
            // Normalize what has one meaning; skip what may differ per call.
            let normalized = match key.kind() {
                "string" if !text.contains('\\') && text.len() >= 2 => {
                    format!("string:{}", &text[1..text.len() - 1])
                }
                "identifier" if lua_style => format!("lua:{}", text),
                "identifier" | "attribute" if !text.contains('(') => format!("name:{}", text),
                "integer" | "float" | "true" | "false" | "null" => {
                    format!("{}:{}", key.kind(), text)
                }
                _ => continue,
            };
            if let Some((_, line)) = seen.iter().find(|(previous, _)| *previous == normalized) {
                diagnostics.push(Diagnostic {
                    check: None,
                    rule: "duplicate-dictionary-key".to_string(),
                    severity: Severity::Error,
                    message: format!(
                        "Key {} is already used in this dictionary (line {}).",
                        text, line
                    ),
                    help: Some(
                        "Remove one entry or use the key you meant: only the last value is kept. Godot refuses to load scripts with duplicate literal keys."
                            .to_string(),
                    ),
                    span: Span::from_node(&key),
                    fixes: Vec::new(),
                });
            } else {
                seen.push((normalized, key.start_position().row + 1));
            }
        }
    }
}
