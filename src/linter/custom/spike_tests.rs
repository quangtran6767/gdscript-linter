//! W1/B1 spike: proves that ast-grep 0.51 can match GDScript through the
//! `GdScript` language in `language.rs`. Each test compiles a rule written in
//! the same YAML form users write in `.gdguard/rules/*.yml`.

#![allow(clippy::unwrap_used)]

use ast_grep_config::{DeserializeEnv, Fixer, RuleCore, SerializableRuleCore};
use ast_grep_core::tree_sitter::LanguageExt;

use super::language::{GdScript, to_internal};

fn compile(rule_yaml: &str) -> RuleCore {
    let core: SerializableRuleCore = ast_grep_config::from_str(rule_yaml).unwrap();
    core.get_matcher(DeserializeEnv::new(GdScript)).unwrap()
}

/// Returns the text of every match, in source order.
fn matched_texts(rule_yaml: &str, source: &str) -> Vec<String> {
    let rule = compile(rule_yaml);
    let grep = GdScript.ast_grep(source);
    let mut texts = Vec::new();
    for node_match in grep.root().find_all(&rule) {
        texts.push(node_match.text().to_string());
    }
    texts
}

/// Applies `fix_template` to every match of `rule_yaml` and returns the new
/// source.
fn apply_fix(rule_yaml: &str, fix_template: &str, source: &str) -> String {
    let rule = compile(rule_yaml);
    let fixer = Fixer::from_str(&to_internal(fix_template), &GdScript).unwrap();
    let grep = GdScript.ast_grep(source);
    let mut edits = Vec::new();
    for node_match in grep.root().find_all(&rule) {
        edits.push(node_match.replace_by(&fixer));
    }
    let mut output = source.to_string();
    // Apply from the end so earlier offsets stay valid.
    for edit in edits.iter().rev() {
        let inserted = String::from_utf8(edit.inserted_text.clone()).unwrap();
        output.replace_range(
            edit.position..edit.position + edit.deleted_length,
            &inserted,
        );
    }
    output
}

#[test]
fn pattern_with_metavariable_matches_and_captures() {
    let rule = compile("rule:\n  pattern: len($X) == 0\n");
    let source =
        "func f(items):\n\tif len(items) == 0:\n\t\tpass\n\tif len(items) == 1:\n\t\tpass\n";
    let grep = GdScript.ast_grep(source);
    let matches: Vec<_> = grep.root().find_all(&rule).collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text(), "len(items) == 0");
    let captured = matches[0].get_env().get_match("X").unwrap();
    assert_eq!(captured.text(), "items");
}

#[test]
fn metavariable_captures_any_expression() {
    let texts = matched_texts(
        "rule:\n  pattern: len($X) == 0\n",
        "var a = len(get_children()) == 0\nvar b = len(self.items) == 0\n",
    );
    assert_eq!(
        texts,
        vec!["len(get_children()) == 0", "len(self.items) == 0"]
    );
}

#[test]
fn multi_metavariable_matches_any_number_of_arguments() {
    let rule = compile("rule:\n  pattern: get_node($$$A)\n");
    let source =
        "func f():\n\tget_node()\n\tget_node(\"A\")\n\tget_node(\"A\", 1)\n\tfind_node(\"A\")\n";
    let grep = GdScript.ast_grep(source);
    let matches: Vec<_> = grep.root().find_all(&rule).collect();
    let mut texts = Vec::new();
    for node_match in &matches {
        texts.push(node_match.text().to_string());
    }
    assert_eq!(
        texts,
        vec!["get_node()", "get_node(\"A\")", "get_node(\"A\", 1)"]
    );
    let arguments = matches[2].get_env().get_multiple_matches("A");
    let mut argument_texts = Vec::new();
    for argument in &arguments {
        if argument.is_named() {
            argument_texts.push(argument.text().to_string());
        }
    }
    assert_eq!(argument_texts, vec!["\"A\"", "1"]);
}

#[test]
fn inside_limits_matches_to_a_function() {
    let rule_yaml = "\
rule:
  pattern: get_node($$$A)
  inside:
    kind: function_definition
    has:
      field: name
      regex: ^_process$
    stopBy: end
";
    let source = "\
func _ready():
\tget_node(\"Ready\")

func _process(delta):
\tif delta > 0:
\t\tget_node(\"Process\")
";
    assert_eq!(
        matched_texts(rule_yaml, source),
        vec!["get_node(\"Process\")"]
    );
}

#[test]
fn inside_with_a_function_pattern() {
    let rule_yaml = "\
rule:
  pattern: get_node($$$A)
  inside:
    pattern: |
      func _process($$$P):
        $$$BODY
    stopBy: end
";
    let source = "\
func _ready():
\tget_node(\"Ready\")

func _process(delta):
\tget_node(\"Process\")
";
    assert_eq!(
        matched_texts(rule_yaml, source),
        vec!["get_node(\"Process\")"]
    );
}

#[test]
fn has_and_not_filter_functions() {
    // Functions that call `print` but never `push_warning`.
    let rule_yaml = "\
rule:
  kind: function_definition
  has:
    pattern: print($$$A)
    stopBy: end
  not:
    has:
      pattern: push_warning($$$B)
      stopBy: end
";
    let source = "\
func a():
\tprint(1)

func b():
\tprint(1)
\tpush_warning(\"x\")

func c():
\tpass
";
    let texts = matched_texts(rule_yaml, source);
    assert_eq!(texts.len(), 1);
    assert!(texts[0].starts_with("func a():"));
}

#[test]
fn constraints_filter_metavariables() {
    let rule_yaml = "\
rule:
  pattern: len($X) == 0
constraints:
  X:
    regex: ^items$
";
    let texts = matched_texts(
        rule_yaml,
        "var a = len(items) == 0\nvar b = len(other) == 0\n",
    );
    assert_eq!(texts, vec!["len(items) == 0"]);
}

#[test]
fn any_combines_rules() {
    let rule_yaml = "\
rule:
  any:
    - pattern: len($X) == 0
    - pattern: len($X) < 1
";
    let texts = matched_texts(
        rule_yaml,
        "var a = len(x) == 0\nvar b = len(y) < 1\nvar c = len(z) > 1\n",
    );
    assert_eq!(texts, vec!["len(x) == 0", "len(y) < 1"]);
}

#[test]
fn follows_and_precedes_work_on_statements() {
    // The pattern root is the call, which has no siblings. `follows` and
    // `precedes` compare statements, so select the statement with
    // `context` + `selector`.
    let rule_yaml = "\
rule:
  pattern:
    context: queue_free()
    selector: expression_statement
  follows:
    pattern:
      context: emit_signal($$$A)
      selector: expression_statement
";
    let source =
        "func f():\n\temit_signal(\"done\")\n\tqueue_free()\n\nfunc g():\n\tqueue_free()\n";
    assert_eq!(matched_texts(rule_yaml, source), vec!["queue_free()"]);

    let precedes_yaml = "\
rule:
  pattern:
    context: emit_signal($$$A)
    selector: expression_statement
  precedes:
    pattern:
      context: queue_free()
      selector: expression_statement
";
    assert_eq!(
        matched_texts(precedes_yaml, source),
        vec!["emit_signal(\"done\")"]
    );
}

#[test]
fn follows_on_the_call_itself_never_matches() {
    // Documents the pitfall above: without a selector, nothing matches.
    let rule_yaml = "\
rule:
  pattern: queue_free()
  follows:
    pattern: emit_signal($$$A)
";
    let source = "func f():\n\temit_signal(\"done\")\n\tqueue_free()\n";
    assert!(matched_texts(rule_yaml, source).is_empty());
}

#[test]
fn fix_template_rewrites_with_captures() {
    let source =
        "func f(items):\n\tif len(items) == 0:\n\t\tpass\n\tvar n = len(get_children()) == 0\n";
    let output = apply_fix("rule:\n  pattern: len($X) == 0\n", "$X.is_empty()", source);
    assert_eq!(
        output,
        "func f(items):\n\tif items.is_empty():\n\t\tpass\n\tvar n = get_children().is_empty()\n"
    );
}

#[test]
fn fix_template_keeps_literal_node_paths() {
    // `$Player` in a template is a node path, not a metavariable.
    let output = apply_fix(
        "rule:\n  pattern: get_node(\"Player\")\n",
        "$Player",
        "var p = get_node(\"Player\")\n",
    );
    assert_eq!(output, "var p = $Player\n");
}

#[test]
fn fix_template_with_multi_metavariable() {
    let output = apply_fix(
        "rule:\n  pattern: get_node($$$A)\n",
        "get_node_or_null($$$A)",
        "var p = get_node(\"A\", 1)\n",
    );
    assert_eq!(output, "var p = get_node_or_null(\"A\", 1)\n");
}

#[test]
fn escaped_all_caps_node_path_in_fix_template() {
    // `\$HUD` is the node path `$HUD`, not the metavariable `HUD`.
    let output = apply_fix(
        "rule:\n  pattern: get_node(\"HUD\")\n",
        "\\$HUD",
        "var hud = get_node(\"HUD\")\n",
    );
    assert_eq!(output, "var hud = $HUD\n");
}

#[test]
fn literal_node_path_in_pattern_matches_every_node_path() {
    // In tree-sitter-gdscript, `$Player` is one `get_node` node whose only
    // child is the anonymous `$`. The path text is not a node, so ast-grep
    // can't tell `$Player` from `$Enemy`. This is why the loader rejects
    // literal node paths in patterns (see `loader.rs`).
    let texts = matched_texts(
        "rule:\n  pattern: $Player.hide()\n",
        "func f():\n\t$Player.hide()\n\t$Enemy.hide()\n",
    );
    assert_eq!(texts, vec!["$Player.hide()", "$Enemy.hide()"]);
}

#[test]
fn node_path_through_metavariable_and_constraint() {
    // The supported way to match one node path inside a pattern.
    let rule_yaml = "\
rule:
  pattern: $P.$METHOD()
constraints:
  P:
    kind: get_node
    regex: ^\\$Player$
";
    let rule = compile(rule_yaml);
    let source = "func f():\n\t$Player.hide()\n\t$Enemy.hide()\n\tplayer.hide()\n";
    let grep = GdScript.ast_grep(source);
    let matches: Vec<_> = grep.root().find_all(&rule).collect();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].text(), "$Player.hide()");
    assert_eq!(
        matches[0].get_env().get_match("METHOD").unwrap().text(),
        "hide"
    );
}

#[test]
fn quoted_and_unique_node_paths_through_constraints() {
    let rule_yaml = "\
rule:
  pattern: $P.text
constraints:
  P:
    regex: ^\\$\"UI/Label\"$
";
    let texts = matched_texts(
        rule_yaml,
        "func f():\n\tprint($\"UI/Label\".text)\n\tprint($\"UI/Other\".text)\n",
    );
    assert_eq!(texts, vec!["$\"UI/Label\".text"]);

    let unique_yaml = "\
rule:
  pattern: $P.value
constraints:
  P:
    regex: ^%Health$
";
    let unique = matched_texts(
        unique_yaml,
        "func f():\n\tprint(%Health.value)\n\tprint(%Mana.value)\n",
    );
    assert_eq!(unique, vec!["%Health.value"]);
}

#[test]
fn unescaped_all_caps_name_is_a_metavariable() {
    let texts = matched_texts(
        "rule:\n  pattern: $HUD.show()\n",
        "func f():\n\t$HUD.show()\n\t$Menu.show()\n\tpanel.show()\n",
    );
    assert_eq!(texts, vec!["$HUD.show()", "$Menu.show()", "panel.show()"]);
}

#[test]
fn kind_and_regex_match_node_paths() {
    let rule_yaml = "\
rule:
  kind: get_node
  regex: ^\\$HUD$
";
    let texts = matched_texts(
        rule_yaml,
        "func f():\n\t$HUD.show()\n\t$HUDLabel.show()\n\tget_node(\"HUD\")\n",
    );
    assert_eq!(texts, vec!["$HUD"]);
}

#[test]
fn any_get_node_path_by_kind() {
    let texts = matched_texts(
        "rule:\n  kind: get_node\n",
        "func f():\n\t$A.show()\n\t$\"B/C\".show()\n\t%D.show()\n",
    );
    // `%D` (unique name) is a `get_node` too.
    assert_eq!(texts, vec!["$A", "$\"B/C\"", "%D"]);
}

#[test]
fn ast_grep_accepts_patterns_with_syntax_errors() {
    // ast-grep builds a pattern even from broken code, so the loader parses
    // every pattern itself and rejects `ERROR` and `MISSING` nodes.
    let core: SerializableRuleCore =
        ast_grep_config::from_str("rule:\n  pattern: \"func (\"\n").unwrap();
    assert!(core.get_matcher(DeserializeEnv::new(GdScript)).is_ok());
}
