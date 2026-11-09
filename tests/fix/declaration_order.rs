use super::*;
use gdguard::linter::diagnostic::{Diagnostic, FixSafety, apply_edits};
use gdguard::{FormatterConfiguration, format_gdscript};

fn formatted(source: &str) -> String {
    format_gdscript(source, &FormatterConfiguration::default()).unwrap()
}

fn order_diagnostics(project: &TempProject, source: &str) -> Vec<Diagnostic> {
    api::lint_source(&project.root.join("a.gd"), source, &project.config())
        .unwrap()
        .diagnostics
        .into_iter()
        .filter(|diagnostic| diagnostic.rule == "declaration-order")
        .collect()
}

#[test]
fn declaration_order_reports_once_with_stable_message_and_span() {
    let project = TempProject::new();
    let source = "var health = 100\nsignal died\nconst MAX_HEALTH = 100\n";
    let diagnostics = order_diagnostics(&project, source);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, Severity::Warning);
    assert_eq!(
        diagnostics[0].message,
        "signal `died` should come before variables"
    );
    assert_eq!(
        &source[diagnostics[0].span.start_byte..diagnostics[0].span.end_byte],
        "signal died"
    );
    let result = fix(&project, source, false);
    assert_eq!(result.applied, 1);
    assert_eq!(
        result.output,
        "signal died\n\nconst MAX_HEALTH = 100\n\nvar health = 100\n"
    );
    assert!(order_diagnostics(&project, &result.output).is_empty());
    assert_eq!(fix(&project, &result.output, false).applied, 0);
}

#[test]
fn declaration_order_unsafe_initializer_dependency_requires_opt_in() {
    let project = TempProject::new();
    for initializer in [
        "a",
        "self.a",
        "a + 1",
        "self.a + 1",
        "max(a, 1)",
        "[a]",
        "other.get(a)",
        "other.values[a]",
    ] {
        let source = formatted(&format!("var a = 1\n@export var b = {initializer}\n"));
        let diagnostics = order_diagnostics(&project, &source);
        assert_eq!(
            diagnostics[0].fixes[0].safety,
            FixSafety::Unsafe,
            "{initializer}"
        );
        let safe = fix(&project, &source, false);
        assert_eq!(safe.output, source);
        assert_eq!(safe.applied, 0);
        let unsafe_result = fix(&project, &source, true);
        assert_eq!(unsafe_result.applied, 1);
        assert!(order_diagnostics(&project, &unsafe_result.output).is_empty());
    }
}

#[test]
fn declaration_order_safe_initializers_and_other_declarations() {
    let project = TempProject::new();
    for source in [
        "var a = 1\n@export var b = 2\n",
        "var a = 1\nvar b = a\nsignal changed\n",
        "var a = 1\nvar b = self.a\nsignal changed\n",
        "var a = 1\nconst VALUE = 2\n",
        "var a = 1\n@export var b = VALUE\nconst VALUE = 2\n",
        "var a = 1\n@export var b = \"a\"\n",
        "var a = 1\n@export var b = other.a\n",
        "func create():\n\treturn 1\nvar a = create()\n",
        "func _process(_delta):\n\tprint(1)\nfunc _ready():\n\tprint(2)\n",
        "class Inner:\n\tvar a = 1\nfunc f():\n\tprint(1)\n",
    ] {
        let source = formatted(source);
        let diagnostics = order_diagnostics(&project, &source);
        assert_eq!(diagnostics[0].fixes[0].safety, FixSafety::Safe, "{source}");
        let result = fix(&project, &source, false);
        assert!(result.applied > 0, "{source}");
        assert!(order_diagnostics(&project, &result.output).is_empty());
    }
}

#[test]
fn declaration_order_does_not_introduce_a_dependency_crossing_region_boundaries() {
    let project = TempProject::new();
    let source =
        formatted("var c = 1\n@export var b = a + 1\n#region Later\nvar a = 1\n#endregion\n");
    assert_eq!(
        order_diagnostics(&project, &source)[0].fixes[0].safety,
        FixSafety::Safe
    );
}

#[test]
fn declaration_order_keeps_regions_comments_annotations_and_bodies() {
    let project = TempProject::new();
    let source = formatted(
        "class_name Ordered extends Node\nvar outside = 0\n#region First\nvar health = 1 # health\n## Died documentation\nsignal died # emitted on death\n## Export documentation\n@export_range(0, 10)\nvar amount = 2\n#endregion\n#region Second\nfunc f():\n\tprint(\"body\")\nvar inside = 3\n#endregion\n",
    );
    let diagnostics = order_diagnostics(&project, &source);
    let reordered = apply_edits(&source, &diagnostics[0].fixes[0].edits).unwrap();
    let expected = format_gdscript(
        &source,
        &FormatterConfiguration {
            reorder_code: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(reordered, expected);
    assert!(reordered.contains("class_name Ordered\nextends Node"));
    assert!(reordered.contains("## Died documentation\nsignal died # emitted on death"));
    assert!(reordered.contains("## Export documentation\n@export_range(0, 10) var amount = 2"));
    assert!(reordered.contains("var health = 1 # health"));
    assert!(reordered.find("var outside").unwrap() < reordered.find("#region First").unwrap());
    assert!(reordered.find("var inside").unwrap() > reordered.find("#region Second").unwrap());
    assert!(reordered.contains("func f():\n\tprint(\"body\")\n"));
    assert!(order_diagnostics(&project, &reordered).is_empty());
}

#[test]
fn declaration_order_fmt_off_reports_without_fix_including_nested_marker() {
    let project = TempProject::new();
    for source in [
        "# fmt: off\nvar a = 1\nsignal died\n# fmt: on\n",
        "var a = 1\nsignal died\nfunc f():\n\t# fmt: off\n\tprint(  1 )\n",
        "#fmt:off\nvar a = 1\nsignal died\n",
    ] {
        let diagnostics = order_diagnostics(&project, source);
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].fixes.is_empty());
        assert_eq!(fix(&project, source, true).output, source);
    }
    let source = "var a = \"# fmt: off\"\nsignal died\n";
    assert!(!order_diagnostics(&project, source)[0].fixes.is_empty());
}

#[test]
fn declaration_order_unformatted_bodies_are_fixed_without_reformatting() {
    let project = TempProject::new();
    let source = "func f():\n    print(  1 )\nsignal died\n";
    let diagnostics = order_diagnostics(&project, source);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].fixes.len(), 1);
    let result = fix(&project, source, false);
    assert_eq!(
        result.output,
        "signal died\n\n\nfunc f():\n    print(  1 )\n"
    );
    assert!(order_diagnostics(&project, &result.output).is_empty());
}

#[test]
fn declaration_order_skips_parse_errors_inner_classes_and_ordered_regions() {
    let project = TempProject::new();
    for source in [
        "var a = 1\nsignal died\nfunc broken(:\n",
        "class Inner:\n\tvar a = 1\n\tsignal died\n",
        "#region Variables\nvar a = 1\n#endregion\n#region Signals\nsignal died\n#endregion\n",
        "class_name Inline extends Node\n",
        "# just a comment\n",
        "",
    ] {
        assert!(order_diagnostics(&project, source).is_empty(), "{source}");
    }
}

#[test]
fn declaration_order_uses_editorconfig_for_fix_availability() {
    let project = TempProject::new();
    project.write(
        ".editorconfig",
        "root = true\n[*.gd]\nindent_style = space\nindent_size = 4\n",
    );
    let source = "var a = 1\nsignal died\n\n\nfunc f():\n    print(1)\n";
    assert!(!order_diagnostics(&project, source)[0].fixes.is_empty());
    let result = fix(&project, source, false);
    assert!(result.output.contains("    print(1)"));
    assert!(order_diagnostics(&project, &result.output).is_empty());
}

#[test]
fn declaration_order_can_be_disabled_in_config() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[rules]\ndeclaration-order = \"off\"\n");
    assert!(order_diagnostics(&project, "var a = 1\nsignal died\n").is_empty());
}

#[test]
fn declaration_order_runs_without_semantic_rules() {
    let mut config = gdguard::linter::LinterConfig::default();
    for rule in gdguard::linter::rules::ALL_RULES {
        if rule.name != "declaration-order" {
            config.disabled_rules.insert(rule.name.to_string());
        }
    }
    let mut linter = gdguard::linter::GDScriptLinter::new(config).unwrap();
    let diagnostics = linter.lint("var a = 1\nsignal died\n", "a.gd").unwrap();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].rule, "declaration-order");
    assert_eq!(diagnostics[0].fixes[0].safety, FixSafety::Safe);
}

#[test]
fn declaration_order_matches_formatter_on_every_reorder_fixture() {
    let project = TempProject::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reorder_code/input");
    let config = FormatterConfiguration {
        reorder_code: true,
        ..Default::default()
    };
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let input = fs::read_to_string(&path).unwrap();
        let source = formatted(&input);
        let expected = format_gdscript(&source, &config).unwrap();
        let diagnostics = order_diagnostics(&project, &source);
        if let Some(diagnostic) = diagnostics.first() {
            if path.file_name().unwrap() == "reorder_disable.gd" {
                assert!(diagnostic.fixes.is_empty());
                continue;
            }
            assert_eq!(diagnostic.fixes.len(), 1, "{}", path.display());
            let fixed = apply_edits(&source, &diagnostic.fixes[0].edits).unwrap();
            assert_eq!(fixed, expected, "{}", path.display());
            assert!(
                order_diagnostics(&project, &fixed).is_empty(),
                "{}",
                path.display()
            );
        } else {
            // Reorder mode can adjust separators or split an inline header
            // even when the declaration sequence was already correct.
            assert!(
                order_diagnostics(&project, &expected).is_empty(),
                "{}",
                path.display()
            );
        }
    }
}

#[test]
fn cli_lint_fix_reorders_file_and_second_lint_is_clean() {
    let project = TempProject::new();
    project.write("gdguard.toml", "[rules]\nunused-local-signal = \"off\"\n");
    let original = "class_name Ordered\nextends Node\nvar health = 100\nsignal died\n";
    project.write("a.gd", original);
    let before = run(
        &project.root,
        &["lint", "--no-cache", "--format", "compact", "a.gd"],
        None,
    );
    assert_eq!(before.exit_code, 1);
    assert!(before.stdout.contains("[declaration-order]"));
    let fixed = run(
        &project.root,
        &["lint", "--fix", "--no-cache", "a.gd"],
        None,
    );
    assert_eq!(fixed.exit_code, 0, "{} {}", fixed.stdout, fixed.stderr);
    assert_eq!(
        project.read("a.gd"),
        format_gdscript(
            original,
            &FormatterConfiguration {
                reorder_code: true,
                ..Default::default()
            }
        )
        .unwrap()
    );
    let after = run(&project.root, &["lint", "--no-cache", "a.gd"], None);
    assert_eq!(after.exit_code, 0, "{} {}", after.stdout, after.stderr);
}

#[test]
fn declaration_order_dependency_can_cross_a_variable_at_the_same_index() {
    let project = TempProject::new();
    for source in [
        "extends Node\n\nvar d = 1\n@export var v = d + 1\nsignal e\n",
        "extends Node\n\nvar d=1\n@export var v=d+1\nsignal e\n",
    ] {
        let diagnostics = order_diagnostics(&project, source);
        assert_eq!(diagnostics[0].fixes[0].safety, FixSafety::Unsafe);
        assert_eq!(fix(&project, source, false).output, source);
        let result = fix(&project, source, true);
        assert!(result.applied > 0);
        assert!(order_diagnostics(&project, &result.output).is_empty());
    }
}

#[test]
fn declaration_order_checks_dependencies_for_every_variable_kind() {
    let project = TempProject::new();
    for declarations in [
        "var _a = 1\nvar b = _a",
        "static var _a = 1\nstatic var b = self._a",
        "var a = 1\n@export var b = a",
        "@onready var _a = 1\n@onready var b = self._a",
    ] {
        let source = formatted(&format!("extends Node\n{declarations}\nsignal changed\n"));
        let diagnostics = order_diagnostics(&project, &source);
        assert_eq!(
            diagnostics[0].fixes[0].safety,
            FixSafety::Unsafe,
            "{source}"
        );
        assert_eq!(fix(&project, &source, false).output, source);
        let result = fix(&project, &source, true);
        assert!(order_diagnostics(&project, &result.output).is_empty());
    }
}

#[test]
fn declaration_order_messages_name_the_variable_category() {
    let project = TempProject::new();
    for (source, expected) in [
        (
            "var a=1\n@export var b=2\n",
            "exported variable `b` should come before variables",
        ),
        (
            "var a=1\nstatic var b=2\n",
            "static variable `b` should come before variables",
        ),
        (
            "func f():\n    print(1)\n@onready var b=2\n",
            "onready variable `b` should come before methods",
        ),
        (
            "@onready var a=1\nvar b=2\n",
            "variable `b` should come before onready variables",
        ),
    ] {
        assert_eq!(order_diagnostics(&project, source)[0].message, expected);
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct DeclarationSnapshot {
    kind: String,
    name: String,
    body: Option<String>,
}

fn declaration_snapshots(source: &str) -> Vec<DeclarationSnapshot> {
    let parsed =
        gdguard::parser::ParseInput::new(source, &FormatterConfiguration::default()).unwrap();
    assert!(!parsed.has_parse_errors, "{source}");
    let root = parsed.tree.root_node();
    let mut declarations = Vec::new();
    for index in 0..root.named_child_count() {
        let node = root.named_child(index as u32).unwrap();
        if let Some(name) = node.child_by_field_name("name") {
            declarations.push(DeclarationSnapshot {
                kind: node.kind().to_string(),
                name: source[name.byte_range()].to_string(),
                body: node
                    .child_by_field_name("body")
                    .map(|_| source[node.byte_range()].to_string()),
            });
        }
        if node.kind() == "extends_statement" {
            declarations.push(DeclarationSnapshot {
                kind: node.kind().to_string(),
                name: source[node.byte_range()]
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                body: None,
            });
        }
        if node.kind() == "class_name_statement" {
            for index in 0..node.named_child_count() {
                let child = node.named_child(index as u32).unwrap();
                if child.kind() == "extends_statement" {
                    declarations.push(DeclarationSnapshot {
                        kind: child.kind().to_string(),
                        name: source[child.byte_range()]
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" "),
                        body: None,
                    });
                }
            }
        }
    }
    declarations.sort();
    declarations
}

fn raw_reorder(project: &TempProject, source: &str) -> String {
    assert!(!gdguard::linter::fix::is_formatted(
        source,
        &FormatterConfiguration::default()
    ));
    let diagnostics = order_diagnostics(project, source);
    assert_eq!(diagnostics.len(), 1);
    assert!(
        !diagnostics[0]
            .help
            .as_ref()
            .unwrap()
            .contains("Format the file first")
    );
    let fixed = apply_edits(source, &diagnostics[0].fixes[0].edits).unwrap();
    assert_eq!(declaration_snapshots(source), declaration_snapshots(&fixed));
    assert!(order_diagnostics(project, &fixed).is_empty(), "{fixed}");
    assert_eq!(fix(project, source, true).output, fixed);
    assert_eq!(fix(project, &fixed, true).applied, 0);
    fixed
}

#[test]
fn declaration_order_raw_fix_preserves_mixed_indentation_and_file_edges() {
    let project = TempProject::new();
    let source = "\n\nfunc f( ):\n    if true:\n    \tprint(  1 )\n    print( 2 )\n\nsignal done\n\nfunc _ready():\n\tif true:\n\t\tprint(  3 )\n\tprint( 4 )\n\n# final comment stays at the end\n\n";
    let fixed = raw_reorder(&project, source);
    assert!(fixed.starts_with("\n\nsignal done\n\n\nfunc _ready():"));
    assert!(fixed.ends_with("\n\n# final comment stays at the end\n\n"));
    assert!(fixed.contains("func f( ):\n    if true:\n    \tprint(  1 )\n    print( 2 )"));
    assert!(fixed.contains("func _ready():\n\tif true:\n\t\tprint(  3 )\n\tprint( 4 )"));
}

#[test]
fn declaration_order_raw_fix_moves_comments_and_annotations_in_regions() {
    let project = TempProject::new();
    let source = "extends  Node\n#region First\nvar health=1 # health\n## Signal docs\nsignal done # inline signal comment\n## Export docs\n@export_range(0,10)\nvar amount=2\n#endregion\n#region Second\nclass Inner:\n    var inner=1\n## Method docs\nfunc f( ):\n    print( 1 )\n#endregion\n# final comment\n";
    let fixed = raw_reorder(&project, source);
    assert!(fixed.contains("## Signal docs\nsignal done # inline signal comment"));
    assert!(fixed.contains("## Export docs\n@export_range(0,10)\nvar amount=2"));
    assert!(fixed.contains("var health=1 # health"));
    let first = fixed
        .split("#region First")
        .nth(1)
        .unwrap()
        .split("#endregion")
        .next()
        .unwrap();
    assert!(first.find("signal done").unwrap() < first.find("var amount").unwrap());
    assert!(first.find("var amount").unwrap() < first.find("var health").unwrap());
    let second = fixed
        .split("#region Second")
        .nth(1)
        .unwrap()
        .split("#endregion")
        .next()
        .unwrap();
    assert!(
        second.find("## Method docs\nfunc f( ):").unwrap() < second.find("class Inner:").unwrap()
    );
    assert!(fixed.ends_with("#endregion\n# final comment\n"));
}

#[test]
fn declaration_order_raw_fix_splits_inline_extends_without_losing_text() {
    let project = TempProject::new();
    let source = "var health=1\n# Header comment\nclass_name  Example extends  Node # base comment\n## Class docs\n## More docs\nsignal done\n# final comment\n";
    let fixed = raw_reorder(&project, source);
    assert!(
        fixed.starts_with("# Header comment\nclass_name  Example\n\nextends  Node # base comment"),
        "{fixed}"
    );
    assert_eq!(fixed.matches("class_name").count(), 1);
    assert_eq!(fixed.matches("extends").count(), 1);
    assert_eq!(fixed.matches("## Class docs").count(), 1);
    assert!(fixed.ends_with("\n# final comment\n"));
}

#[test]
fn declaration_order_raw_fix_preserves_semicolon_declarations_and_crlf() {
    let project = TempProject::new();
    let source = "\r\nvar a=1; signal done # inline\r\n# tail\r\n";
    let fixed = raw_reorder(&project, source);
    assert_eq!(
        fixed,
        "\r\nsignal done # inline\r\n\r\nvar a=1\r\n# tail\r\n"
    );
}

#[test]
fn declaration_order_raw_fix_preserves_declarations_across_reorder_fixtures() {
    let project = TempProject::new();
    let mut config = String::from("[rules]\nunused-ignore = \"off\"\n");
    for rule in gdguard::linter::rules::ALL_RULES {
        if rule.name != "declaration-order" {
            config.push_str(&format!("{} = \"off\"\n", rule.name));
        }
    }
    project.write("gdguard.toml", &config);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/reorder_code/input");
    let mut checked_fixes = 0;
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        let source = format!(
            "\n{}\nsignal w10_probe\n",
            fs::read_to_string(&path).unwrap()
        );
        let diagnostics = order_diagnostics(&project, &source);
        if let Some(diagnostic) = diagnostics.first() {
            if path.file_name().unwrap() == "reorder_disable.gd" {
                assert!(diagnostic.fixes.is_empty());
                continue;
            }
            assert!(!diagnostic.fixes.is_empty(), "{}", path.display());
            let fixed = raw_reorder(&project, &source);
            assert!(fixed.starts_with('\n'));
            checked_fixes += 1;
        }
    }
    assert!(
        checked_fixes >= 10,
        "only {checked_fixes} fixtures exercised raw fixes"
    );
}

#[test]
fn cli_lint_fix_reorders_unformatted_file_without_touching_function_body() {
    let project = TempProject::new();
    let source = "class_name Ordered\nextends Node\nfunc f( ):\n    print( 1 )\nvar value=1\n";
    project.write("a.gd", source);
    let output = run(
        &project.root,
        &["lint", "--fix", "--no-cache", "a.gd"],
        None,
    );
    assert_eq!(output.exit_code, 0, "{} {}", output.stdout, output.stderr);
    let fixed = project.read("a.gd");
    assert!(fixed.contains("func f( ):\n    print( 1 )\n"));
    assert_eq!(declaration_snapshots(source), declaration_snapshots(&fixed));
    assert!(fixed.find("var value=1").unwrap() < fixed.find("func f( )").unwrap());
    let after = run(&project.root, &["lint", "--no-cache", "a.gd"], None);
    assert_eq!(after.exit_code, 0, "{} {}", after.stdout, after.stderr);
}
