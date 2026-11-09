//! The `RuleProvider` for custom YAML rules: runs every loaded rule on a file
//! and turns ast-grep matches into `Diagnostic`s with optional `Fix`es.

use ast_grep_core::meta_var::MetaVarEnv;
use ast_grep_core::{AstGrep, NodeMatch};

use super::doc::GdDoc;
use super::language::EXPANDO_CHAR;
use super::loader::{CompiledRule, LoadedRule};
use crate::linter::diagnostic::{Diagnostic, Edit, FileContext, Fix, RuleMeta, RuleProvider, Span};

/// Fix descriptions show the replacement when it is one short line.
const MAX_INLINE_REPLACEMENT_LENGTH: usize = 60;

/// Holds every custom YAML rule of one config. `rules()[i]` describes the
/// rule compiled in `compiled[i]`.
pub struct YamlRuleProvider {
    metas: Vec<RuleMeta>,
    compiled: Vec<CompiledRule>,
}

impl YamlRuleProvider {
    pub fn new(loaded_rules: Vec<LoadedRule>) -> YamlRuleProvider {
        let mut metas = Vec::with_capacity(loaded_rules.len());
        let mut compiled = Vec::with_capacity(loaded_rules.len());
        for rule in loaded_rules {
            metas.push(rule.meta);
            compiled.push(rule.compiled);
        }
        YamlRuleProvider { metas, compiled }
    }

    /// Runs only the rule with `rule_id`. Used by `test-rules`, so each test
    /// case only sees its own rule's diagnostics.
    pub fn check_rule(&self, rule_id: &str, context: &FileContext) -> Vec<Diagnostic> {
        let grep = make_grep(context);
        let mut diagnostics = Vec::new();
        for index in 0..self.metas.len() {
            if self.metas[index].id == rule_id {
                run_rule(
                    &self.metas[index],
                    &self.compiled[index],
                    &grep,
                    context.source,
                    &mut diagnostics,
                );
            }
        }
        diagnostics
    }
}

impl RuleProvider for YamlRuleProvider {
    fn rules(&self) -> &[RuleMeta] {
        &self.metas
    }

    fn check_file(&mut self, context: &FileContext) -> Vec<Diagnostic> {
        if self.compiled.is_empty() {
            return Vec::new();
        }
        let grep = make_grep(context);
        let mut diagnostics = Vec::new();
        for index in 0..self.compiled.len() {
            run_rule(
                &self.metas[index],
                &self.compiled[index],
                &grep,
                context.source,
                &mut diagnostics,
            );
        }
        diagnostics
    }
}

/// Wraps the pipeline's tree for ast-grep. Cloning a tree-sitter `Tree` is a
/// reference-count increment, so the file is never parsed again.
fn make_grep(context: &FileContext) -> AstGrep<GdDoc> {
    AstGrep::doc(GdDoc::from_tree(context.source, context.tree))
}

fn run_rule(
    meta: &RuleMeta,
    rule: &CompiledRule,
    grep: &AstGrep<GdDoc>,
    source: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for node_match in grep.root().find_all(&rule.matcher) {
        diagnostics.push(build_diagnostic(meta, rule, &node_match, source));
    }
}

fn build_diagnostic(
    meta: &RuleMeta,
    rule: &CompiledRule,
    node_match: &NodeMatch<'_, GdDoc>,
    source: &str,
) -> Diagnostic {
    let span = Span::from_node(&node_match.get_inner_node().0);
    let environment = node_match.get_env();
    let message = render_template(&rule.message, environment, source);
    let mut help = None;
    if let Some(help_template) = &rule.help {
        help = Some(render_template(help_template, environment, source));
    }

    let mut fixes = Vec::new();
    if let Some(fix_template) = &rule.fix {
        let replacement = render_fix(fix_template, environment, source, span.start_byte);
        fixes.push(Fix {
            description: fix_description(&meta.id, &replacement),
            safety: rule.fix_safety,
            edits: vec![Edit { span, replacement }],
        });
    }

    Diagnostic {
        check: None,
        rule: meta.id.clone(),
        // The rule's default; the pipeline applies `[rules]` levels later.
        severity: meta.default_severity,
        message,
        help,
        span,
        fixes,
    }
}

fn fix_description(rule_id: &str, replacement: &str) -> String {
    if !replacement.contains('\n') && replacement.len() <= MAX_INLINE_REPLACEMENT_LENGTH {
        return format!("Replace with `{}`", replacement);
    }
    format!("Apply the `{}` fix", rule_id)
}

/// Renders a fix template (internal form) for a match starting at
/// `match_start`. Every template line after the first gets the leading
/// whitespace of the line the match starts on, so a multi-line fix lines up
/// with the code it replaces, with tabs or spaces. Captured text is inserted
/// as is.
pub fn render_fix(
    template: &str,
    environment: &MetaVarEnv<'_, GdDoc>,
    source: &str,
    match_start: usize,
) -> String {
    if !template.contains('\n') {
        return render_template(template, environment, source);
    }
    let line_start = match source[..match_start].rfind('\n') {
        Some(newline) => newline + 1,
        None => 0,
    };
    let mut indent_end = line_start;
    let bytes = source.as_bytes();
    while indent_end < match_start && (bytes[indent_end] == b'\t' || bytes[indent_end] == b' ') {
        indent_end += 1;
    }
    let indent = &source[line_start..indent_end];
    let mut indented = String::with_capacity(template.len() + indent.len() * 4);
    for (index, line) in template.split('\n').enumerate() {
        if index > 0 {
            indented.push('\n');
            if !line.is_empty() {
                indented.push_str(indent);
            }
        }
        indented.push_str(line);
    }
    render_template(&indented, environment, source)
}

/// Replaces `µNAME` and `µµµNAME` (internal metavariables) in `template` with
/// the captured text. A multi-match (`$$$ARGS`) renders as the source text
/// from its first to its last node, so separators like `, ` are kept.
/// Metavariables that captured nothing render as an empty string.
pub fn render_template(
    template: &str,
    environment: &MetaVarEnv<'_, GdDoc>,
    source: &str,
) -> String {
    if !template.contains(EXPANDO_CHAR) {
        return template.to_string();
    }
    let characters: Vec<char> = template.chars().collect();
    let mut output = String::with_capacity(template.len() + 16);
    let mut index = 0;
    while index < characters.len() {
        if characters[index] != EXPANDO_CHAR {
            output.push(characters[index]);
            index += 1;
            continue;
        }
        let mut expando_count = 0;
        while index < characters.len() && characters[index] == EXPANDO_CHAR {
            expando_count += 1;
            index += 1;
        }
        let name_start = index;
        while index < characters.len()
            && (characters[index].is_ascii_uppercase()
                || characters[index].is_ascii_digit()
                || characters[index] == '_')
        {
            index += 1;
        }
        let name: String = characters[name_start..index].iter().collect();
        if name.is_empty() {
            // Not a metavariable (the loader rejects these); keep the text.
            for _ in 0..expando_count {
                output.push('$');
            }
            continue;
        }
        if expando_count >= 3 {
            let nodes = environment.get_multiple_matches(&name);
            if let (Some(first), Some(last)) = (nodes.first(), nodes.last()) {
                output.push_str(&source[first.range().start..last.range().end]);
            }
        } else if let Some(node) = environment.get_match(&name) {
            output.push_str(&node.text());
        }
    }
    output
}
