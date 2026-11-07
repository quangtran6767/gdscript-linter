//! `sarif`: SARIF 2.1.0, for GitHub code scanning and other tools. Rules
//! that appear in the results are listed in `tool.driver.rules` with their
//! metadata; columns are 1-based UTF-16 code units (`columnKind`).

use serde::Serialize;

use crate::linter::diagnostic::{RuleMeta, RuleOrigin, Severity};
use crate::linter::output::{LintReport, utf16_column};
use crate::linter::rules_meta::builtin_rule_meta;

pub const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";
pub const SARIF_VERSION: &str = "2.1.0";
const INFORMATION_URI: &str = "https://github.com/GDQuest/GDScript-formatter";

#[derive(Serialize)]
struct SarifLog {
    #[serde(rename = "$schema")]
    schema: &'static str,
    version: &'static str,
    runs: Vec<SarifRun>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRun {
    tool: SarifTool,
    column_kind: &'static str,
    results: Vec<SarifResult>,
}

#[derive(Serialize)]
struct SarifTool {
    driver: SarifDriver,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifDriver {
    name: &'static str,
    version: &'static str,
    information_uri: &'static str,
    rules: Vec<SarifRule>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRule {
    id: String,
    short_description: SarifText,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_description: Option<SarifText>,
    default_configuration: SarifConfiguration,
    properties: SarifRuleProperties,
}

#[derive(Serialize)]
struct SarifText {
    text: String,
}

#[derive(Serialize)]
struct SarifConfiguration {
    level: &'static str,
}

#[derive(Serialize)]
struct SarifRuleProperties {
    fixable: bool,
    origin: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifResult {
    rule_id: String,
    rule_index: usize,
    level: &'static str,
    message: SarifText,
    locations: Vec<SarifLocation>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifLocation {
    physical_location: SarifPhysicalLocation,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifPhysicalLocation {
    artifact_location: SarifArtifactLocation,
    region: SarifRegion,
}

#[derive(Serialize)]
struct SarifArtifactLocation {
    uri: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SarifRegion {
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "note",
    }
}

/// The metadata of `rule_id`: built-in, then the report's custom rules.
fn find_meta(rule_id: &str, custom_metas: &[RuleMeta]) -> Option<RuleMeta> {
    if let Some(meta) = builtin_rule_meta(rule_id) {
        return Some(meta);
    }
    for meta in custom_metas {
        if meta.id == rule_id {
            return Some(meta.clone());
        }
    }
    None
}

fn sarif_rule(rule_id: &str, severity: Severity, custom_metas: &[RuleMeta]) -> SarifRule {
    match find_meta(rule_id, custom_metas) {
        Some(meta) => {
            let full_description = if meta.rationale.trim().is_empty() {
                None
            } else {
                Some(SarifText {
                    text: meta.rationale.trim().to_string(),
                })
            };
            let origin = match meta.origin {
                RuleOrigin::BuiltIn => "builtin",
                RuleOrigin::Custom { .. } => "custom",
            };
            SarifRule {
                id: rule_id.to_string(),
                short_description: SarifText {
                    text: meta.description,
                },
                full_description,
                default_configuration: SarifConfiguration {
                    level: level(meta.default_severity),
                },
                properties: SarifRuleProperties {
                    fixable: meta.fixable,
                    origin,
                },
            }
        }
        // A rule without metadata still gets an entry, so every result has a
        // valid `ruleIndex`.
        None => SarifRule {
            id: rule_id.to_string(),
            short_description: SarifText {
                text: rule_id.to_string(),
            },
            full_description: None,
            default_configuration: SarifConfiguration {
                level: level(severity),
            },
            properties: SarifRuleProperties {
                fixable: false,
                origin: "custom",
            },
        },
    }
}

/// Percent-encodes characters that aren't allowed in a URI path.
fn uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        let is_allowed =
            byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.' | b'~' | b':');
        if is_allowed {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{:02X}", byte));
        }
    }
    encoded
}

pub fn write_sarif(report: &LintReport, output: &mut String) -> Result<(), String> {
    // Rules sorted by id; results point at them by index.
    let mut rule_ids: Vec<(String, Severity)> = Vec::new();
    for file in &report.files {
        for diagnostic in &file.diagnostics {
            let mut is_known = false;
            for (rule_id, _) in &rule_ids {
                if *rule_id == diagnostic.rule {
                    is_known = true;
                    break;
                }
            }
            if !is_known {
                rule_ids.push((diagnostic.rule.clone(), diagnostic.severity));
            }
        }
    }
    rule_ids.sort_by(|left, right| left.0.cmp(&right.0));

    let mut rules = Vec::with_capacity(rule_ids.len());
    for (rule_id, severity) in &rule_ids {
        rules.push(sarif_rule(rule_id, *severity, &report.rule_metas));
    }

    let mut results = Vec::new();
    for file in &report.files {
        for diagnostic in &file.diagnostics {
            let mut rule_index = 0;
            for (index, (rule_id, _)) in rule_ids.iter().enumerate() {
                if *rule_id == diagnostic.rule {
                    rule_index = index;
                }
            }
            let span = &diagnostic.span;
            results.push(SarifResult {
                rule_id: diagnostic.rule.clone(),
                rule_index,
                level: level(diagnostic.severity),
                message: SarifText {
                    text: diagnostic.message.clone(),
                },
                locations: vec![SarifLocation {
                    physical_location: SarifPhysicalLocation {
                        artifact_location: SarifArtifactLocation {
                            uri: uri_path(&file.path),
                        },
                        region: SarifRegion {
                            start_line: span.start.line,
                            start_column: utf16_column(&file.source, span.start_byte),
                            end_line: span.end.line,
                            end_column: utf16_column(&file.source, span.end_byte),
                        },
                    },
                }],
            });
        }
    }

    let log = SarifLog {
        schema: SARIF_SCHEMA,
        version: SARIF_VERSION,
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "gdguard",
                    version: env!("CARGO_PKG_VERSION"),
                    information_uri: INFORMATION_URI,
                    rules,
                },
            },
            column_kind: "utf16CodeUnits",
            results,
        }],
    };
    let text = serde_json::to_string_pretty(&log)
        .map_err(|error| format!("Failed to write SARIF: {}", error))?;
    output.push_str(&text);
    output.push('\n');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_uri_paths() {
        assert_eq!(uri_path("scripts/my file.gd"), "scripts/my%20file.gd");
        assert_eq!(uri_path("é.gd"), "%C3%A9.gd");
    }
}
