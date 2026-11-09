//! Public rule names and their granular compatibility checks.
//! Keep check identities for existing config, ignores, fixes and baselines.

use super::diagnostic::Diagnostic;

pub struct RuleGroup {
    pub id: &'static str,
    pub checks: &'static [&'static str],
}

pub const RULE_GROUPS: &[RuleGroup] = &[
    RuleGroup {
        id: "naming-convention",
        checks: &[
            "class-name",
            "signal-name",
            "enum-name",
            "enum-member-name",
            "constant-name",
            "variable-name",
            "load-variable-name",
            "function-name",
            "function-argument-name",
            "loop-variable-name",
        ],
    },
    RuleGroup {
        id: "unused-code",
        checks: &[
            "unused-variable",
            "unused-argument",
            "assigned-but-never-read",
            "unused-private-function",
            "unused-local-signal",
            "unused-signal",
            "unnecessary-pass",
        ],
    },
    RuleGroup {
        id: "redundant-expression",
        checks: &[
            "comparison-with-itself",
            "self-assignment",
            "standalone-expression",
        ],
    },
];

pub fn rule_id(check: &str) -> &str {
    for group in RULE_GROUPS {
        if group.checks.contains(&check) {
            return group.id;
        }
    }
    check
}

pub fn checks(id: &str) -> &[&str] {
    for group in RULE_GROUPS {
        if group.id == id {
            return group.checks;
        }
    }
    &[]
}

pub fn is_disabled(disabled: &std::collections::HashSet<String>, id: &str) -> bool {
    if disabled.contains(id) || disabled.contains(rule_id(id)) {
        return true;
    }
    let members = checks(id);
    !members.is_empty() && members.iter().all(|member| disabled.contains(*member))
}

pub fn consolidate(diagnostics: &mut [Diagnostic]) {
    for diagnostic in diagnostics {
        let group = rule_id(&diagnostic.rule);
        if group != diagnostic.rule {
            diagnostic.check = Some(diagnostic.rule.clone());
            diagnostic.rule = group.to_string();
        }
    }
}
