#![cfg(feature = "js-rules")]
#![allow(clippy::unwrap_used)]
use gdguard::linter::api::{Config, ConfigOverride, RuleSetting, lint_source};
use gdguard::linter::diagnostic::{FixSafety, RuleProvider, apply_edits};
use gdguard::linter::script::JsRuleProvider;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Project {
    root: PathBuf,
    config: Config,
}
impl Project {
    fn new(source: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gdguard-js-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("rules")).unwrap();
        std::fs::write(root.join("rules/rule.js"), source).unwrap();
        Self {
            config: Config {
                config_path: Some(root.join("gdguard.toml")),
                custom_rule_paths: vec![root.join("rules")],
                ..Default::default()
            },
            root,
        }
    }
    fn lint(&self, source: &str) -> Result<Vec<gdguard::linter::diagnostic::Diagnostic>, String> {
        Ok(lint_source(&self.root.join("main.gd"), source, &self.config)?.diagnostics)
    }
}
impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn rule(body: &str) -> String {
    format!(
        "export default {{meta:{{id:'test-rule',description:'test',fixable:true,fixSafety:'safe'}},create(context) {{return {{identifier(node) {{if(node.text === 'old') {{ {} }} }} }};}}}}",
        body
    )
}

#[test]
fn reports_precise_utf8_spans_and_each_fixer_method() {
    for (fix, expected) in [
        (
            "context.fix.replace(node, 'value')",
            "var value = 1\nprint(value)\n",
        ),
        (
            "context.fix.insertBefore(node, 'new_')",
            "var value = 1\nprint(new_old)\n",
        ),
        (
            "context.fix.insertAfter(node, '_new')",
            "var value = 1\nprint(old_new)\n",
        ),
        ("context.fix.remove(node)", "var value = 1\nprint()\n"),
    ] {
        let project = Project::new(&rule(&format!(
            "context.report({{node,message:'replace',help:'help text',fix:{}}});",
            fix
        )));
        let source = "var value = 1\nprint(old)\n";
        let diagnostics = project.lint(source).unwrap();
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule == "test-rule")
            .unwrap();
        assert_eq!(
            &source[diagnostic.span.start_byte..diagnostic.span.end_byte],
            "old"
        );
        assert_eq!(diagnostic.help.as_deref(), Some("help text"));
        assert_eq!(diagnostic.fixes[0].safety, FixSafety::Safe);
        assert_eq!(
            apply_edits(source, &diagnostic.fixes[0].edits).unwrap(),
            expected
        );
    }
}

#[test]
fn options_follow_config_and_overrides_and_semantics_are_real() {
    let mut project = Project::new(&rule(
        "if(context.options.enabled && context.semantic.lookup(context.semantic.scopeAt(node.range.start.byte),'value') !== null) context.report({range:node.range,message:context.filename,fix:fixer=>fixer.replace(node,'value')});",
    ));
    project.config.rules.insert(
        "test-rule".to_string(),
        RuleSetting {
            options: toml::Table::from_iter([("enabled".to_string(), toml::Value::Boolean(false))]),
            ..Default::default()
        },
    );
    project.config.overrides.push(ConfigOverride {
        files: vec!["main.gd".to_string()],
        rules: std::collections::BTreeMap::from([(
            "test-rule".to_string(),
            RuleSetting {
                options: toml::Table::from_iter([(
                    "enabled".to_string(),
                    toml::Value::Boolean(true),
                )]),
                ..Default::default()
            },
        )]),
    });
    let diagnostics = project
        .lint("var value = 1\nfunc f():\n\treturn old\n")
        .unwrap();
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule == "test-rule")
        .unwrap();
    assert!(diagnostic.message.ends_with("main.gd"));
}

#[test]
fn invalid_rule_files_report_path_and_reason() {
    for source in [
        "export default {",
        "export default {create(){return {}}}",
        "export default {meta:{id:'test-rule',description:'x'},create:3}",
        "import fs from 'fs'; export default {}",
        "export default {meta:{id:'unused-ignore',description:'x'},create(){return {}}}",
    ] {
        let project = Project::new(source);
        let error = project.lint("").unwrap_err();
        assert!(error.contains("rule.js"), "{}", error);
    }
}

#[test]
fn duplicate_ids_across_js_and_yaml_are_rejected() {
    let project = Project::new(&rule(""));
    std::fs::write(
        project.root.join("rules/other.yml"),
        "id: test-rule\nmessage: test\nrule:\n  pattern: print($X)\n",
    )
    .unwrap();
    assert!(project.lint("").unwrap_err().contains("duplicate rule id"));
    std::fs::remove_file(project.root.join("rules/other.yml")).unwrap();
    std::fs::write(project.root.join("rules/other.js"), rule("")).unwrap();
    assert!(project.lint("").unwrap_err().contains("duplicate rule id"));
}

#[test]
fn resource_limits_return_clear_errors() {
    for body in [
        "while(true) {}",
        "new Array(20000000).fill(1);",
        "context.report({range:{start:{byte:13},end:{byte:14}},message:'bad UTF-8 range'});",
    ] {
        let project = Project::new(&rule(body));
        let mut provider =
            JsRuleProvider::load(&project.config, &[project.root.join("rules/rule.js")], &[])
                .unwrap();
        provider.sandbox.time_limit = std::time::Duration::from_millis(50);
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_gdscript::LANGUAGE.into())
            .unwrap();
        let source = "var text = 'đ'\nprint(old)\n";
        let tree = parser.parse(source, None).unwrap();
        let context = gdguard::linter::diagnostic::FileContext {
            path: &project.root,
            source,
            tree: &tree,
            semantic: None,
            project: None,
        };
        let diagnostics = provider.check_file(&context);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "script-error"),
            "{:?}",
            diagnostics
        );
    }
}

#[test]
fn example_rules_and_their_output_tests_pass() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .args(["test-rules", "examples/rules"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("max-function-params"));
}

#[test]
fn parallel_files_have_independent_worker_runtimes_and_deterministic_results() {
    let project = Project::new(&rule("context.report({node,message:'found'});"));
    std::fs::write(
        project.root.join("gdguard.toml"),
        "[custom_rules]\npaths=['rules']\n",
    )
    .unwrap();
    for index in 0..12 {
        std::fs::write(
            project.root.join(format!("{}.gd", index)),
            "func f():\n\treturn old\n",
        )
        .unwrap();
    }
    let run = || {
        std::process::Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
            .current_dir(&project.root)
            .args(["lint", "--no-cache", "--format", "json", "."])
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first.status.code(), Some(1));
    assert_eq!(first.stdout, second.stdout);
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(
        report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|issue| issue["rule"] == "test-rule")
            .count(),
        12
    );
}

#[test]
fn test_rules_checks_fix_output_and_idempotency() {
    for (replacement, expected, exit) in [
        ("value", "var value = 1\nprint(value)\n", 0),
        ("old + 1", "var value = 1\nprint(old + 1)\n", 1),
    ] {
        let mut source = rule(&format!(
            "context.report({{node,message:'replace',fix:context.fix.replace(node,{})}});",
            serde_json::to_string(replacement).unwrap()
        ));
        source.pop();
        source.push_str(&format!(",tests:{{valid:['var value = 1\\nprint(value)\\n'],invalid:[{{code:'var value = 1\\nprint(old)\\n',output:{}}}]}}}}",serde_json::to_string(expected).unwrap()));
        let project = Project::new(&source);
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
            .arg("test-rules")
            .arg(project.root.join("rules"))
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(exit),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if exit == 1 {
            assert!(String::from_utf8_lossy(&output.stdout).contains("not idempotent"));
        }
    }
}

#[test]
fn project_api_has_autoloads_members_and_null_outside_projects() {
    let project = Project::new(&rule(
        "if(context.project && context.project.classNames.includes('Data') && context.project.autoloads.Game==='game.gd' && context.project.hasMember('Data','score')==='yes' && context.project.hasMember('Data','missing')==='no') context.report({node,message:'project data'});",
    ));
    std::fs::write(
        project.root.join("project.godot"),
        "[autoload]\nGame=\"*res://game.gd\"\n",
    )
    .unwrap();
    std::fs::write(
        project.root.join("game.gd"),
        "class_name Data\nvar score = 0\n",
    )
    .unwrap();
    assert!(
        project
            .lint("func f():\n\treturn old\n")
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic.rule == "test-rule")
    );
    let outside = Project::new(&rule(
        "if(context.project===null)context.report({node,message:'no project'});",
    ));
    assert!(
        outside
            .lint("func f():\n\treturn old\n")
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic.message == "no project")
    );
}

#[test]
fn autoload_example_flags_external_mutation_but_allows_its_owner() {
    let source = include_str!("../../examples/rules/no-autoload-mutation-outside-owner.js");
    let project = Project::new(source);
    std::fs::write(
        project.root.join("project.godot"),
        "[autoload]\nGame=\"*res://main.gd\"\n",
    )
    .unwrap();
    assert!(
        !project
            .lint("func f():\n\tGame.score = 1\n")
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic.rule == "no-autoload-mutation-outside-owner")
    );
    let diagnostics = lint_source(
        &project.root.join("other.gd"),
        "func f():\n\tGame.score += 1\n",
        &project.config,
    )
    .unwrap()
    .diagnostics;
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "no-autoload-mutation-outside-owner")
    );
    std::fs::create_dir_all(project.root.join("nested")).unwrap();
    let nested = lint_source(
        &project.root.join("nested/main.gd"),
        "func f():\n\tGame.score = 1\n",
        &project.config,
    )
    .unwrap();
    assert!(
        nested
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule == "no-autoload-mutation-outside-owner")
    );
}

#[test]
fn project_edits_invalidate_cached_lint_results_in_other_files() {
    let project = Project::new(&rule(
        "context.report({node,message:context.project.hasMember('Data','added')});",
    ));
    std::fs::write(project.root.join("project.godot"), "config_version=5\n").unwrap();
    std::fs::write(
        project.root.join("gdguard.toml"),
        "[custom_rules]\npaths=['rules']\n",
    )
    .unwrap();
    std::fs::write(
        project.root.join("main.gd"),
        "class_name Main\nfunc f() -> Variant:\n\treturn old\n",
    )
    .unwrap();
    std::fs::write(
        project.root.join("data.gd"),
        "class_name Data\nvar value = 1\n",
    )
    .unwrap();
    let run = || {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
            .current_dir(&project.root)
            .args(["lint", "--format", "json", "main.gd"])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let before = run();
    assert_eq!(before["issues"][0]["message"], "no");
    std::fs::write(
        project.root.join("data.gd"),
        "class_name Data\nvar added = 1\n",
    )
    .unwrap();
    let after = run();
    assert_eq!(after["issues"][0]["message"], "yes");
}

#[test]
fn complete_js_documentation_rule_passes_embedded_tests() {
    let document = include_str!("../../docs/js_rules.md");
    let source = document
        .split("```js\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let project = Project::new(source);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_gdscript-formatter"))
        .arg("test-rules")
        .arg(project.root.join("rules"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
