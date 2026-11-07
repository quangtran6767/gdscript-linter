//! W1/B2: the YAML rule loader. Every validation error is checked with its
//! exact message so changes to the wording are deliberate.

use std::path::{Path, PathBuf};

use gdscript_formatter::linter::custom::loader::{
    InvalidCase, LoadedRule, load_rule_str, load_rules,
};
use gdscript_formatter::linter::diagnostic::{FixSafety, RuleOrigin, Severity};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("custom_rules")
        .join("loader")
        .join(name)
}

fn load(text: &str) -> LoadedRule {
    load_rule_str(Path::new("rules/test.yml"), text).unwrap()
}

/// Loads `text` as `rules/test.yml` and returns the error as the CLI prints
/// it.
fn load_error(text: &str) -> String {
    load_rule_str(Path::new("rules/test.yml"), text)
        .unwrap_err()
        .to_string()
}

const MINIMAL: &str = "id: my-rule\nmessage: Found it\nrule:\n  pattern: print($A)\n";

// Valid rules

#[test]
fn loads_a_folder_of_valid_rules_in_path_order() {
    let rules = load_rules(&[fixture_dir("valid")]).unwrap();
    assert_eq!(rules.len(), 2);
    // `.yaml` and `.yml` are both loaded; README.txt is ignored.
    assert_eq!(rules[0].meta.id, "no-get-node-in-process");
    assert_eq!(rules[1].meta.id, "prefer-is-empty");
}

#[test]
fn valid_rule_fills_meta_compiled_and_tests() {
    let rules = load_rules(&[fixture_dir("valid")]).unwrap();
    let rule = &rules[1];
    assert_eq!(rule.meta.id, "prefer-is-empty");
    assert_eq!(
        rule.meta.description,
        "Use `$X.is_empty()` instead of `len($X) == 0`"
    );
    assert_eq!(
        rule.meta.rationale,
        "Comparing len() to zero hides the intent of the check.\n"
    );
    assert_eq!(rule.meta.default_severity, Severity::Warning);
    assert!(rule.meta.fixable);
    assert_eq!(rule.meta.bad_example, "if len(items) == 0:\n\tpass");
    assert_eq!(rule.meta.good_example, "if items.is_empty():\n\tpass");
    assert_eq!(
        rule.meta.origin,
        RuleOrigin::Custom {
            path: fixture_dir("valid").join("prefer-is-empty.yml")
        }
    );
    assert_eq!(rule.compiled.fix_safety, FixSafety::Safe);
    assert!(rule.compiled.fix.is_some());
    assert_eq!(
        rule.compiled.help.as_deref(),
        Some("is_empty() states the intent and works on Array, Dictionary and String.")
    );
    assert_eq!(
        rule.tests.invalid,
        vec![InvalidCase {
            code: "if len(items) == 0:\n\tpass".to_string(),
            output: Some("if items.is_empty():\n\tpass".to_string()),
            errors: None,
        }]
    );
    assert_eq!(rule.tests.valid, vec!["if items.is_empty():\n\tpass"]);
}

#[test]
fn defaults_for_optional_keys() {
    let rule = load(MINIMAL);
    assert_eq!(rule.meta.default_severity, Severity::Warning);
    assert_eq!(rule.meta.rationale, "");
    assert!(!rule.meta.fixable);
    assert!(rule.compiled.fix.is_none());
    assert_eq!(rule.compiled.fix_safety, FixSafety::Suggestion);
    assert!(rule.tests.valid.is_empty());
    assert!(rule.tests.invalid.is_empty());
}

#[test]
fn fix_without_safety_is_a_suggestion_and_not_fixable() {
    let rule =
        load("id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\nfix: push_warning($A)\n");
    assert!(rule.compiled.fix.is_some());
    assert_eq!(rule.compiled.fix_safety, FixSafety::Suggestion);
    assert!(!rule.meta.fixable);
}

#[test]
fn every_supported_rule_key_loads() {
    let text = "\
id: all-keys
message: m
rule:
  all:
    - kind: call
    - regex: ^print
    - any:
        - pattern: print($A)
        - pattern:
            context: print($A, $B)
            selector: call
    - not:
        inside:
          kind: function_definition
          has:
            field: name
            regex: ^_ready$
          stopBy: end
    - not:
        has:
          kind: string
    - not:
        follows:
          kind: expression_statement
          stopBy: neighbor
    - not:
        precedes:
          kind: expression_statement
constraints:
  A:
    kind: identifier
";
    let rule = load(text);
    assert_eq!(rule.meta.id, "all-keys");
}

#[test]
fn literal_node_paths_are_fine_in_fix_and_message() {
    let rule = load(
        "id: my-rule\nmessage: Use $Player\nrule:\n  pattern: get_node(\"Player\")\nfix: $Player\nfixSafety: safe\n",
    );
    assert!(rule.meta.fixable);
}

#[test]
fn invalid_test_case_with_an_exact_error_count() {
    let rule = load(
        "id: my-rule
message: m
rule:
  pattern: print($A)
tests:
  invalid:
    - code: \"f(print(1), print(2))\"
      errors: 2
",
    );
    assert_eq!(rule.tests.invalid[0].errors, Some(2));
    assert_eq!(
        load_error(
            "id: my-rule
message: m
rule:
  pattern: print($A)
tests:
  invalid:
    - code: print(1)
      errors: 0
"
        ),
        "rules/test.yml: `tests.invalid[0].errors`: expected a number greater than 0"
    );
}

#[test]
fn invalid_test_case_may_be_a_bare_string() {
    let rule = load(
        "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\ntests:\n  invalid:\n    - print(1)\n",
    );
    assert_eq!(
        rule.tests.invalid,
        vec![InvalidCase {
            code: "print(1)".to_string(),
            output: None,
            errors: None,
        }]
    );
}

// Ids

#[test]
fn duplicate_id_across_two_files() {
    let folder = fixture_dir("duplicate");
    let error = load_rules(std::slice::from_ref(&folder)).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "{}: `id`: duplicate rule id `same-id`, already defined in {}",
            folder.join("b.yml").display(),
            folder.join("a.yml").display()
        )
    );
}

#[test]
fn duplicate_id_across_two_folders() {
    let valid = fixture_dir("valid");
    let error = load_rules(&[valid.clone(), valid.clone()]).unwrap_err();
    assert_eq!(
        error.to_string(),
        format!(
            "{}: `id`: duplicate rule id `no-get-node-in-process`, already defined in {}",
            valid.join("no-get-node-in-process.yaml").display(),
            valid.join("no-get-node-in-process.yaml").display()
        )
    );
}

#[test]
fn id_that_clashes_with_a_built_in_rule() {
    assert_eq!(
        load_error("id: unused-argument\nmessage: m\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `id`: rule id `unused-argument` is already used by a built-in rule"
    );
}

#[test]
fn id_that_is_not_kebab_case() {
    let expected_suffix =
        "must be kebab-case (lowercase letters, digits and single dashes, starting with a letter)";
    for id in [
        "MyRule", "my_rule", "my--rule", "-my-rule", "my-rule-", "1-rule", "\"\"",
    ] {
        let text = format!("id: {}\nmessage: m\nrule:\n  pattern: print($A)\n", id);
        let shown = if id == "\"\"" { "" } else { id };
        assert_eq!(
            load_error(&text),
            format!(
                "rules/test.yml: `id`: rule id `{}` {}",
                shown, expected_suffix
            ),
            "id {}",
            id
        );
    }
}

// Required keys and types

#[test]
fn missing_required_keys() {
    assert_eq!(
        load_error("message: m\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `id`: missing required key"
    );
    assert_eq!(
        load_error("id: my-rule\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `message`: missing required key"
    );
    assert_eq!(
        load_error("id: my-rule\nmessage: m\n"),
        "rules/test.yml: `rule`: missing required key"
    );
}

#[test]
fn key_with_the_wrong_type() {
    assert_eq!(
        load_error("id: my-rule\nmessage: [a, b]\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `message`: expected a string"
    );
}

#[test]
fn file_that_is_not_a_mapping() {
    assert_eq!(
        load_error("- id: my-rule\n"),
        "rules/test.yml: a rule file must be a YAML mapping with `id`, `message` and `rule`"
    );
}

#[test]
fn invalid_yaml_names_the_file_and_line() {
    let error = load_error("id: my-rule\nmessage: \"unclosed\nrule: x\n");
    assert!(
        error.starts_with("rules/test.yml: invalid YAML: "),
        "{}",
        error
    );
    assert!(error.contains("line"), "{}", error);
}

// Unknown keys

#[test]
fn unknown_top_level_key() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nmesage: typo\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `mesage`: unknown key, expected one of `id`, `severity`, `message`, `help`, `rationale`, `rule`, `constraints`, `fix`, `fixSafety`, `tests`"
    );
}

#[test]
fn unknown_key_inside_rule() {
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\n  insde:\n    kind: call\n"
        ),
        "rules/test.yml: `rule.insde`: unknown key, expected one of `pattern`, `kind`, `regex`, `inside`, `has`, `follows`, `precedes`, `all`, `any`, `not`"
    );
}

#[test]
fn unknown_key_inside_a_relational_rule_lists_stop_by_and_field() {
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\n  inside:\n    kind: call\n    stop_by: end\n"
        ),
        "rules/test.yml: `rule.inside.stop_by`: unknown key, expected one of `pattern`, `kind`, `regex`, `inside`, `has`, `follows`, `precedes`, `all`, `any`, `not`, `stopBy`, `field`"
    );
}

#[test]
fn unknown_key_in_a_nested_list_item() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  any:\n    - kind: call\n    - kinds: call\n"),
        "rules/test.yml: `rule.any[1].kinds`: unknown key, expected one of `pattern`, `kind`, `regex`, `inside`, `has`, `follows`, `precedes`, `all`, `any`, `not`"
    );
}

#[test]
fn stop_by_is_not_allowed_outside_relational_rules() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\n  stopBy: end\n"),
        "rules/test.yml: `rule.stopBy`: unknown key, expected one of `pattern`, `kind`, `regex`, `inside`, `has`, `follows`, `precedes`, `all`, `any`, `not`"
    );
}

#[test]
fn unsupported_ast_grep_keys() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  kind: call\n  nthChild: 1\n"),
        "rules/test.yml: `rule.nthChild`: `nthChild` is not supported in gdguard rules"
    );
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  kind: call\nutils:\n  a:\n    kind: call\n"),
        "rules/test.yml: `utils`: `utils` is not supported in gdguard rules"
    );
}

#[test]
fn unknown_key_in_pattern_object() {
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern:\n    context: print($A)\n    selecter: call\n"
        ),
        "rules/test.yml: `rule.pattern.selecter`: unknown key, expected one of `context`, `selector`, `strictness`"
    );
}

#[test]
fn unknown_key_in_tests() {
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\ntests:\n  invalid:\n    - code: print(1)\n      outptu: x\n"
        ),
        "rules/test.yml: `tests.invalid[0].outptu`: unknown key, expected one of `code`, `output`, `errors`"
    );
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\ntests:\n  vaild: []\n"),
        "rules/test.yml: `tests.vaild`: unknown key, expected one of `valid`, `invalid`"
    );
}

// Severity and fixSafety

#[test]
fn unknown_severity() {
    assert_eq!(
        load_error("id: my-rule\nseverity: warn\nmessage: m\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `severity`: unknown severity `warn`, expected `error`, `warning` or `info`"
    );
}

#[test]
fn unknown_fix_safety() {
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\nfix: x\nfixSafety: maybe\n"
        ),
        "rules/test.yml: `fixSafety`: unknown fix safety `maybe`, expected `safe`, `unsafe` or `suggestion`"
    );
}

#[test]
fn fix_safety_without_fix() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\nfixSafety: safe\n"),
        "rules/test.yml: `fixSafety`: `fixSafety` needs a `fix`"
    );
}

// Patterns and compilation

#[test]
fn pattern_that_does_not_parse_as_gdscript() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  pattern: \"func (\"\n"),
        "rules/test.yml: `rule.pattern`: pattern `func (` doesn't parse as GDScript"
    );
}

#[test]
fn nested_pattern_that_does_not_parse() {
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\n  inside:\n    pattern: \"if :\"\n"
        ),
        "rules/test.yml: `rule.inside.pattern`: pattern `if :` doesn't parse as GDScript"
    );
    assert_eq!(
        load_error(
            "id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\nconstraints:\n  A:\n    pattern: \"(\"\n"
        ),
        "rules/test.yml: `constraints.A.pattern`: pattern `(` doesn't parse as GDScript"
    );
}

#[test]
fn literal_node_paths_are_allowed_in_patterns() {
    let rule = load("id: my-rule\nmessage: m\nrule:\n  pattern: $Player.hide()\n");
    assert_eq!(rule.meta.id, "my-rule");
}

#[test]
fn unknown_kind() {
    let error = load_error("id: my-rule\nmessage: m\nrule:\n  kind: not_a_kind\n");
    assert!(
        error.starts_with("rules/test.yml: `rule`: `rule` is not configured correctly."),
        "{}",
        error
    );
    assert!(error.contains("not_a_kind"), "{}", error);
}

#[test]
fn invalid_regex() {
    let error = load_error("id: my-rule\nmessage: m\nrule:\n  kind: call\n  regex: \"(\"\n");
    assert!(
        error.starts_with("rules/test.yml: `rule`: `rule` is not configured correctly."),
        "{}",
        error
    );
}

#[test]
fn rule_without_a_positive_matcher() {
    let expected = "rules/test.yml: `rule`: needs at least one of `pattern`, `kind`, `regex`, `all` or `any`; a rule with only `not` or relational keys matches almost every node";
    assert_eq!(
        load_error(
            "id: my-rule
message: m
rule:
  not:
    kind: call
"
        ),
        expected
    );
    assert_eq!(
        load_error(
            "id: my-rule
message: m
rule:
  inside:
    kind: call
"
        ),
        expected
    );
}

#[test]
fn rule_that_is_not_a_mapping() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule: print($A)\n"),
        "rules/test.yml: `rule`: expected a rule object (a mapping)"
    );
}

// Metavariables in templates

#[test]
fn fix_with_undefined_metavariable() {
    assert_eq!(
        load_error("id: my-rule\nmessage: m\nrule:\n  pattern: print($A)\nfix: print($B)\n"),
        "rules/test.yml: `fix`: uses the metavariable `$B`, which the rule's patterns don't define"
    );
}

#[test]
fn message_with_undefined_metavariable() {
    assert_eq!(
        load_error("id: my-rule\nmessage: Found $B\nrule:\n  pattern: print($A)\n"),
        "rules/test.yml: `message`: uses the metavariable `$B`, which the rule's patterns don't define"
    );
}

#[test]
fn templates_may_use_multi_metavariables_and_escapes() {
    let rule = load(
        "id: my-rule\nmessage: \"Call with $$$ARGS on \\\\$HUD\"\nrule:\n  pattern: print($$$ARGS)\nfix: push_warning($$$ARGS)\n",
    );
    assert!(rule.compiled.fix.is_some());
}

// Folders

#[test]
fn missing_rule_folder() {
    let folder = fixture_dir("does-not-exist");
    let error = load_rules(std::slice::from_ref(&folder)).unwrap_err();
    assert_eq!(error.path.as_deref(), Some(folder.as_path()));
    assert_eq!(error.key.as_deref(), Some("custom_rules.paths"));
    assert!(
        error.message.starts_with("can't read the rule folder: "),
        "{}",
        error.message
    );
}

#[test]
fn no_rule_paths_loads_nothing() {
    assert!(load_rules(&[]).unwrap().is_empty());
}
