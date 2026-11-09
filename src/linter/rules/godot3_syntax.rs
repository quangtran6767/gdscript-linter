//! Godot 3 syntax that Godot 4 rejects, common in code written from older
//! examples. Parsed calls (`yield`, `instance()`, string-method `connect`)
//! come from the tree; keywords the GDScript 2 grammar can't parse (`tool`,
//! `export`, `onready`, `setget`, RPC keywords) come from the source lines,
//! so they are reported even when the file has a parse error.
use super::Rule;
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, FixSafety, Severity, Span};

pub struct Godot3SyntaxRule;

fn is_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

/// The text of a plain one-line string literal, without quotes.
fn string_value<'a>(node: &tree_sitter::Node, source: &'a str) -> Option<&'a str> {
    if node.kind() != "string" {
        return None;
    }
    let text = &source[node.byte_range()];
    if text.len() < 2 || text.starts_with("\"\"\"") || text.contains('\\') {
        return None;
    }
    Some(&text[1..text.len() - 1])
}

fn report(
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
    message: &str,
    help: &str,
    fix: Option<(String, FixSafety, Vec<Edit>)>,
) {
    let mut fixes = Vec::new();
    if let Some((description, safety, edits)) = fix {
        fixes.push(Fix {
            description,
            safety,
            edits,
        });
    }
    diagnostics.push(Diagnostic {
        check: None,
        rule: "godot3-syntax".to_string(),
        severity: Severity::Error,
        message: message.to_string(),
        help: Some(help.to_string()),
        span,
        fixes,
    });
}

impl Rule for Godot3SyntaxRule {
    fn check_context(&mut self, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
        let source = context.source;
        check_calls(context, diagnostics);
        let mut line_start = 0;
        for line in source.split_inclusive('\n') {
            let content = line.trim_end_matches(['\n', '\r']);
            let indent = content.len() - content.trim_start().len();
            let trimmed = content.trim_start();
            let start = line_start + indent;
            line_start += line.len();
            let first_word: &str = trimmed
                .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .next()
                .unwrap_or("");
            if first_word.is_empty() || in_string_or_comment(context, start) {
                continue;
            }
            let rest = trimmed[first_word.len()..].trim_start();
            let word_span = Span::from_byte_range(source, start, start + first_word.len());
            match first_word {
                "tool" if indent == 0 && rest.is_empty() => report(
                    diagnostics,
                    word_span,
                    "`tool` is Godot 3 syntax.",
                    "Use the `@tool` annotation.",
                    Some((
                        "Replace with `@tool`".to_string(),
                        FixSafety::Safe,
                        vec![Edit {
                            span: word_span,
                            replacement: "@tool".to_string(),
                        }],
                    )),
                ),
                "onready" | "export" if rest.starts_with("var ") => {
                    let safety = if first_word == "onready" {
                        FixSafety::Safe
                    } else {
                        // `@export` needs a type or an initial value.
                        FixSafety::Unsafe
                    };
                    report(
                        diagnostics,
                        word_span,
                        &format!("`{}` is Godot 3 syntax.", first_word),
                        &format!("Use the `@{}` annotation.", first_word),
                        Some((
                            format!("Replace with `@{}`", first_word),
                            safety,
                            vec![Edit {
                                span: word_span,
                                replacement: format!("@{}", first_word),
                            }],
                        )),
                    );
                }
                "export" if rest.starts_with('(') => report(
                    diagnostics,
                    word_span,
                    "`export(...)` is Godot 3 syntax.",
                    "Use `@export var name: Type`, or a specific annotation such as `@export_range`, `@export_enum` or `@export_file`.",
                    None,
                ),
                "var" if contains_word(trimmed, "setget") => {
                    let offset = word_offset(trimmed, "setget").unwrap_or(0);
                    report(
                        diagnostics,
                        Span::from_byte_range(source, start + offset, start + offset + 6),
                        "`setget` is Godot 3 syntax.",
                        "Declare the accessors inline: `var x: int: set = set_x, get = get_x`, or with `set(value):` and `get:` blocks.",
                        None,
                    );
                }
                "remote" | "master" | "puppet" | "remotesync" | "mastersync" | "puppetsync"
                | "sync"
                    if rest.starts_with("func ") || rest.starts_with("var ") =>
                {
                    report(
                        diagnostics,
                        word_span,
                        &format!("`{}` is Godot 3 syntax.", first_word),
                        "Use the `@rpc` annotation, for example `@rpc(\"any_peer\")`.",
                        None,
                    );
                }
                _ => {}
            }
        }
    }
}

fn check_calls(context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
    let mut cursor = context.tree.walk();
    loop {
        let node = cursor.node();
        if matches!(node.kind(), "call" | "attribute_call") {
            check_call(node, context, diagnostics);
        }
        if cursor.goto_first_child() {
            continue;
        }
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

fn check_call(node: tree_sitter::Node, context: &FileContext, diagnostics: &mut Vec<Diagnostic>) {
    let source = context.source;
    let Some(callee) = node.named_child(0) else {
        return;
    };
    let name = &source[callee.byte_range()];
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return;
    };
    let mut values = Vec::new();
    for index in 0..arguments.named_child_count() {
        if let Some(argument) = arguments.named_child(index as u32) {
            if argument.kind() != "comment" {
                values.push(argument);
            }
        }
    }
    if node.kind() == "call" && name == "yield" {
        let mut fix = None;
        if let [object, signal] = values.as_slice() {
            if let Some(signal_name) =
                string_value(signal, source).filter(|name| is_identifier(name))
            {
                // SceneTree.idle_frame is process_frame in Godot 4.
                let signal_name = if signal_name == "idle_frame" {
                    "process_frame"
                } else {
                    signal_name
                };
                let object_text = &source[object.byte_range()];
                let replacement = if object_text == "self" {
                    format!("await {}", signal_name)
                } else {
                    format!("await {}.{}", object_text, signal_name)
                };
                fix = Some((
                    format!("Replace with `{}`", replacement),
                    FixSafety::Unsafe,
                    vec![Edit {
                        span: Span::from_node(&node),
                        replacement,
                    }],
                ));
            }
        }
        report(
            diagnostics,
            Span::from_node(&callee),
            "`yield` was removed in Godot 4.",
            "Use `await object.signal_name`, or `await function_call()` for coroutines.",
            fix,
        );
    } else if node.kind() == "attribute_call" && name == "instance" && values.is_empty() {
        report(
            diagnostics,
            Span::from_node(&callee),
            "`PackedScene.instance()` was renamed in Godot 4.",
            "Use `instantiate()`.",
            Some((
                "Replace with `instantiate`".to_string(),
                FixSafety::Unsafe,
                vec![Edit {
                    span: Span::from_node(&callee),
                    replacement: "instantiate".to_string(),
                }],
            )),
        );
    } else if name == "connect"
        && values.len() >= 3
        && string_value(&values[0], source).is_some()
        && string_value(&values[2], source).is_some()
    {
        let mut fix = None;
        if let (3, Some(method)) = (
            values.len(),
            string_value(&values[2], source).filter(|method| is_identifier(method)),
        ) {
            let target = &source[values[1].byte_range()];
            let callable = if target == "self" {
                method.to_string()
            } else if values[1].kind() == "identifier" {
                format!("{}.{}", target, method)
            } else {
                String::new()
            };
            if !callable.is_empty() {
                fix = Some((
                    format!("Pass the callable `{}`", callable),
                    FixSafety::Unsafe,
                    vec![Edit {
                        span: Span::from_byte_range(
                            source,
                            values[1].start_byte(),
                            values[2].end_byte(),
                        ),
                        replacement: callable,
                    }],
                ));
            }
        }
        report(
            diagnostics,
            Span::from_node(&callee),
            "`connect(signal, target, \"method\")` is Godot 3 syntax.",
            "Pass a Callable: `signal_name.connect(method)` or `connect(\"signal_name\", method)`. Use `method.bind(...)` for extra arguments.",
            fix,
        );
    }
}

fn in_string_or_comment(context: &FileContext, byte: usize) -> bool {
    let mut node = context
        .tree
        .root_node()
        .descendant_for_byte_range(byte, byte + 1);
    while let Some(current) = node {
        if matches!(current.kind(), "string" | "comment") {
            return true;
        }
        node = current.parent();
    }
    false
}

fn word_offset(text: &str, word: &str) -> Option<usize> {
    let mut from = 0;
    while let Some(offset) = text[from..].find(word) {
        let start = from + offset;
        let end = start + word.len();
        let before = text[..start].chars().last();
        let after = text[end..].chars().next();
        let boundary = |character: Option<char>| {
            character
                .is_none_or(|character| !(character.is_ascii_alphanumeric() || character == '_'))
        };
        if boundary(before) && boundary(after) {
            return Some(start);
        }
        from = end;
    }
    None
}

fn contains_word(text: &str, word: &str) -> bool {
    word_offset(text, word).is_some()
}
