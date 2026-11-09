#![allow(clippy::unwrap_used)]

use gdguard::linter::api::{self, Config};
use gdguard::linter::config::parse_config;
use gdguard::linter::diagnostic::Diagnostic;
use std::path::Path;

fn typing(source: &str, options: &str) -> Vec<Diagnostic> {
    let config = parse_config(Path::new("/tmp/gdguard-typing/gdguard.toml"), options).unwrap();
    let result =
        api::lint_source(Path::new("/tmp/gdguard-typing/example.gd"), source, &config).unwrap();
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|issue| issue.rule == "parse-error"),
        "{:#?}",
        result.diagnostics
    );
    result
        .diagnostics
        .into_iter()
        .filter(|issue| issue.rule == "type-annotations")
        .collect()
}

#[test]
fn requires_return_types_including_void_and_parameters() {
    let source = "static func damage(amount, scale = 1.0):\n\tprint(amount, scale)\n";
    let issues = typing(source, "");
    assert_eq!(issues.len(), 3);
    assert!(issues[0].message.contains("return type"));
    assert_eq!(
        &source[issues[0].span.start_byte..issues[0].span.end_byte],
        "damage"
    );
    assert!(issues[1].message.contains("'amount'"));
    assert!(issues[2].message.contains("'scale'"));
    assert!(typing("static func damage(amount: int, scale: float = 1.0) -> void:\n\tprint(amount, scale)\n", "").is_empty());
}

#[test]
fn covers_constants_member_and_local_variables_signals_and_loops() {
    let source = "signal changed(value)\nconst LIMIT = 10\nvar health = 100\nfunc run() -> void:\n\tvar local = 1\n\tfor item in [local]:\n\t\tprint(item)\n";
    let issues = typing(source, "");
    assert_eq!(issues.len(), 5);
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.starts_with("Signal parameter"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.starts_with("Constant"))
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.message.starts_with("Loop variable"))
    );
    assert!(typing("signal changed(value: int)\nconst LIMIT: int = 10\nvar health: int = 100\nfunc run() -> void:\n\tvar local: int = 1\n\tfor item: int in [local]:\n\t\tprint(item)\n", "").is_empty());
}

#[test]
fn checks_lambdas_and_inner_classes() {
    let source = "class Inner:\n\tfunc compute(value):\n\t\treturn value\nvar callback: Callable = func(value): return value\n";
    assert_eq!(typing(source, "").len(), 4);
    assert!(typing("class Inner:\n\tfunc compute(value: int) -> int:\n\t\treturn value\nvar callback: Callable = func(value: int) -> int: return value\n", "").is_empty());
}

#[test]
fn property_accessors_use_the_property_type() {
    assert!(typing("var health: int = 100:\n\tget:\n\t\treturn health\n\tset(value):\n\t\thealth = value\n", "").is_empty());
}

#[test]
fn requires_container_arguments_at_all_annotation_positions() {
    let source = "signal changed(values: Array)\nconst DATA: Dictionary = {}\nvar values: Array = []\nfunc run(input: Dictionary) -> Array:\n\tfor item: Array in values:\n\t\tprint(item, input)\n\treturn []\n";
    assert_eq!(typing(source, "").len(), 6);
    assert!(typing("signal changed(values: Array[int])\nconst DATA: Dictionary[String, int] = {}\nvar values: Array[Array] = []\n", "").iter().any(|issue| issue.message.starts_with("Array needs")));
    assert!(typing("var values: Array[int] = []\nvar data: Dictionary[String, Variant] = {}\nvar packed: PackedByteArray = PackedByteArray()\nvar qualified: Types.Array\nfunc run(input: Array[Node]) -> Dictionary[String, int]:\n\treturn {}\n", "").is_empty());
}

#[test]
fn inference_is_an_explicit_option_and_does_not_waive_function_contracts() {
    let source =
        "const LIMIT := 10\nvar health := 100\nfunc run(value := 1) -> void:\n\tprint(value)\n";
    assert_eq!(typing(source, "").len(), 3);
    assert!(
        typing(
            source,
            "[rules]\ntype-annotations = { allow_inferred = true }\n"
        )
        .is_empty()
    );
    assert_eq!(
        typing(
            "func run(value):\n\tprint(value)\n",
            "[rules]\ntype-annotations = { allow_inferred = true }\n"
        )
        .len(),
        2
    );
    assert_eq!(
        typing(
            "var values = []\n",
            "[rules]\ntype-annotations = { allow_inferred = true }\n"
        )
        .len(),
        1
    );
}

#[test]
fn supports_levels_ignores_and_folder_overrides() {
    assert!(
        typing(
            "func run():\n\tpass\n",
            "[rules]\ntype-annotations = 'off'\n"
        )
        .is_empty()
    );
    let issues = typing(
        "# gdguard-ignore-next-line type-annotations -- external callback\nfunc run(value):\n\tprint(value)\n",
        "",
    );
    assert!(issues.is_empty());
    let issues = typing(
        "func run():\n\tpass\n",
        "[rules]\ntype-annotations = 'error'\n",
    );
    assert_eq!(issues[0].severity.as_str(), "error");
    assert!(typing("func run():\n\tpass\n", "[rules]\ntype-annotations = 'error'\n[[overrides]]\nfiles = ['example.gd']\nrules = { type-annotations = 'off' }\n").is_empty());
}

#[test]
fn rejects_invalid_options() {
    for options in [
        "allow_inferred = 1",
        "allow_inferred = 'yes'",
        "unknown = true",
    ] {
        assert!(
            parse_config(
                Path::new("/tmp/gdguard-typing/gdguard.toml"),
                &format!("[rules]\ntype-annotations = {{ {options} }}\n")
            )
            .is_err()
        );
    }
}

#[test]
fn neither_fix_mode_invents_types() {
    let source = "func run(value):\n\tprint(value)\n";
    for unsafe_fixes in [false, true] {
        let result = api::fix_source(
            Path::new("/tmp/gdguard-typing/example.gd"),
            source,
            &Config::default(),
            unsafe_fixes,
        )
        .unwrap();
        assert_eq!(result.output, source);
        assert_eq!(result.applied, 0);
        assert_eq!(
            result
                .remaining
                .iter()
                .filter(|issue| issue.rule == "type-annotations")
                .count(),
            2
        );
        assert!(
            result
                .remaining
                .iter()
                .filter(|issue| issue.rule == "type-annotations")
                .all(|issue| issue.fixes.is_empty())
        );
    }
}

#[test]
fn empty_enums_do_not_hide_missing_annotations_elsewhere() {
    assert_eq!(typing("enum State {}\nfunc run():\n\tpass\n", "").len(), 1);
}

#[test]
fn variadic_parameters_still_need_types() {
    assert_eq!(
        typing("func run(...args) -> void:\n\tprint(args)\n", "").len(),
        1
    );
    assert!(
        typing(
            "func run(...args: Array[Variant]) -> void:\n\tprint(args)\n",
            ""
        )
        .is_empty()
    );
}
