#![allow(clippy::unwrap_used)]

use gdguard::linter::api::{self, Config};
use gdguard::linter::config::parse_config;
use gdguard::linter::diagnostic::Diagnostic;
use gdguard::linter::{GDScriptLinter, LinterConfig};
use std::path::Path;

fn config(text: &str) -> Config {
    parse_config(Path::new("/tmp/gdguard-groups/gdguard.toml"), text).unwrap()
}

fn lint(source: &str, settings: &str) -> Vec<Diagnostic> {
    api::lint_source(
        Path::new("/tmp/gdguard-groups/example.gd"),
        source,
        &config(settings),
    )
    .unwrap()
    .diagnostics
}

#[test]
fn emits_grouped_rules_with_granular_check_identities() {
    let source = "var badName: int = 1\nfunc run(value: int) -> bool:\n\tvar unused: int = 1\n\treturn value == value\n";
    let issues = lint(source, "");
    for (rule, check) in [
        ("naming-convention", "variable-name"),
        ("unused-code", "unused-variable"),
        ("redundant-expression", "comparison-with-itself"),
    ] {
        assert!(
            issues
                .iter()
                .any(|issue| issue.rule == rule && issue.check_id() == check),
            "{issues:#?}"
        );
    }
}

#[test]
fn one_group_setting_controls_all_related_checks() {
    let source = "signal BadSignal\nvar badName: int = 1\nfunc badFunction(BadArgument: int) -> void:\n\tvar unused: int = 1\n\tBadArgument = BadArgument\n";
    let issues = lint(
        source,
        "[rules]\nnaming-convention = 'off'\nunused-code = 'off'\nredundant-expression = 'off'\n",
    );
    assert!(issues.is_empty(), "{issues:#?}");
    let issues = lint(
        source,
        "[rules]\nnaming-convention = 'info'\nunused-code = 'error'\n",
    );
    assert!(
        issues
            .iter()
            .filter(|issue| issue.rule == "naming-convention")
            .all(|issue| issue.severity.as_str() == "info")
    );
    assert!(
        issues
            .iter()
            .filter(|issue| issue.rule == "unused-code")
            .all(|issue| issue.severity.as_str() == "error")
    );
}

#[test]
fn specific_legacy_setting_wins_within_a_table_and_later_overrides_win() {
    let source = "signal BadSignal\nvar badName: int = 1\n";
    let issues = lint(
        source,
        "[rules]\nnaming-convention = 'off'\nvariable-name = 'error'\n",
    );
    assert_eq!(
        issues
            .iter()
            .filter(|issue| issue.rule == "naming-convention")
            .count(),
        1
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.check_id() == "variable-name")
    );
    let issues = lint(
        source,
        "[rules]\nvariable-name = 'error'\n[[overrides]]\nfiles = ['example.gd']\nrules = { naming-convention = 'off' }\n",
    );
    assert!(!issues.iter().any(|issue| issue.rule == "naming-convention"));
}

#[test]
fn group_and_legacy_ignores_remain_precise() {
    let source = "# gdguard-ignore-next-line unused-argument -- callback\nfunc run(value: int) -> void:\n\tvar unused: int = 1\n\tprint(1)\n";
    let issues = lint(source, "");
    assert!(
        !issues
            .iter()
            .any(|issue| issue.check_id() == "unused-argument" || issue.rule == "unused-ignore")
    );
    assert!(
        issues
            .iter()
            .any(|issue| issue.check_id() == "unused-variable")
    );
    let source = "# gdguard-ignore-file unused-code -- exported callbacks\nfunc run(value: int) -> void:\n\tvar unused: int = 1\n\tprint(1)\n";
    assert!(lint(source, "").is_empty());
}

#[test]
fn unsafe_allowlists_can_select_a_group_or_one_legacy_check() {
    let source = "func run(value: int) -> void:\n\tvar unused: int = 1\n\tprint(1)\n";
    let path = Path::new("/tmp/gdguard-groups/example.gd");
    let result = api::fix_source(
        path,
        source,
        &config("[fix]\nunsafe = ['unused-argument']\n"),
        false,
    )
    .unwrap();
    assert!(result.output.contains("_value"));
    assert!(result.output.contains("var unused"));
    let result = api::fix_source(
        path,
        source,
        &config("[fix]\nunsafe = ['unused-code']\n"),
        false,
    )
    .unwrap();
    assert!(result.output.contains("_value"));
    assert!(!result.output.contains("var unused"));
    assert_eq!(result.applied, 2);
}

#[test]
fn low_level_api_can_disable_groups() {
    let mut config = LinterConfig::default();
    config
        .disabled_rules
        .insert("naming-convention".to_string());
    config.disabled_rules.insert("unused-code".to_string());
    let mut linter = GDScriptLinter::new(config).unwrap();
    let issues = linter
        .lint(
            "# gdguard-ignore-next-line signal-name -- callback\nsignal BadSignal\n",
            "example.gd",
        )
        .unwrap();
    assert!(issues.is_empty(), "{issues:#?}");
}

#[test]
fn public_catalog_has_fewer_rules_and_legacy_names_are_still_explainable() {
    let metas = gdguard::linter::rules_meta::builtin_rule_metas();
    assert_eq!(metas.len(), 29);
    for group in gdguard::linter::rule_groups::RULE_GROUPS {
        assert!(metas.iter().any(|meta| meta.id == group.id));
        for check in group.checks {
            assert!(!metas.iter().any(|meta| meta.id == *check));
            assert!(gdguard::linter::explain::explain(check, &metas).is_ok());
        }
    }
}

#[test]
fn fully_disabled_group_ignores_are_not_reported_as_stale() {
    let source =
        "# gdguard-ignore-file unused-code -- callbacks\nfunc run(value: int) -> void:\n\tpass\n";
    assert!(lint(source, "[rules]\nunused-code = 'off'\n").is_empty());
}

#[test]
fn loaded_resource_naming_can_be_enabled_separately() {
    let source = "var badName: int = 1\nvar bad_LOAD: Resource = load(\"data.tres\")\n";
    let issues = lint(
        source,
        "[rules]\nnaming-convention = 'off'\nload-variable-name = 'error'\n",
    );
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].check_id(), "load-variable-name");
    let issues = lint(
        source,
        "[rules]\nvariable-name = 'off'\nload-variable-name = 'error'\n",
    );
    assert_eq!(issues.len(), 1, "{issues:#?}");
    assert_eq!(issues[0].check_id(), "load-variable-name");
}

#[test]
fn cli_group_disables_win_over_specific_config_settings() {
    let compiled = gdguard::linter::config::CompiledConfig::new(config(
        "[rules]\nnaming-convention = 'off'\nvariable-name = 'error'\n",
    ))
    .unwrap();
    let mut cli = gdguard::linter::config::CliRuleOverrides::default();
    cli.disabled_rules.insert("naming-convention".to_string());
    let settings = compiled.settings_for_file(Path::new("/tmp/gdguard-groups/example.gd"), &cli);
    assert!(
        settings
            .linter_config
            .disabled_rules
            .contains("variable-name")
    );
    assert!(
        settings
            .linter_config
            .disabled_rules
            .contains("load-variable-name")
    );
}

#[test]
fn legacy_baseline_fingerprints_survive_grouping() {
    use gdguard::linter::baseline::{Baseline, apply_baseline, entries_for_report};
    use gdguard::linter::output::{FileDiagnostics, LintReport};
    let source = "var badName: int = 1\n";
    let issues = lint(source, "");
    let report = LintReport {
        files: vec![FileDiagnostics {
            path: "example.gd".to_string(),
            source: source.to_string(),
            diagnostics: issues,
        }],
        files_checked: 1,
        ..Default::default()
    };
    let mut old_report = report.clone();
    for issue in &mut old_report.files[0].diagnostics {
        issue.rule = issue.check_id().to_string();
        issue.check = None;
    }
    let root = Path::new("/tmp/gdguard-groups");
    let old_entries = entries_for_report(&old_report, root, root);
    assert_eq!(entries_for_report(&report, root, root), old_entries);
    let baseline = Baseline::from_entries(&old_entries);
    let mut current = report;
    assert_eq!(apply_baseline(&baseline, &mut current, root, root), 1);
    assert!(current.files.is_empty());
}

#[test]
fn distance_optimization_is_informational_and_still_unsafe() {
    let issues = lint(
        "func near(a: Vector2, b: Vector2) -> bool:\n\treturn a.distance_to(b) < 40.0\n",
        "",
    );
    let issue = issues
        .iter()
        .find(|issue| issue.rule == "prefer-squared-distance")
        .unwrap();
    assert_eq!(issue.severity.as_str(), "info");
    assert_eq!(issue.fixes[0].safety.as_str(), "unsafe");
}
