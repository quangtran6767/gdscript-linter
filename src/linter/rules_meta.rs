//! Owner: Track B (W3). Rule metadata (description, rationale, bad and good
//! examples) for the built-in rules, used by `explain` and `--list-rules`.
//!
//! Every id in `rule_config::get_all_rule_names()` has an entry, plus
//! `load-variable-name`, which the `variable-name` rule reports for variables
//! holding a loaded resource. A test runs every bad example through the
//! linter and checks it is reported, and that every good example is not.

use crate::linter::diagnostic::{RuleMeta, RuleOrigin, Severity};

struct BuiltinInfo {
    id: &'static str,
    severity: Severity,
    fixable: bool,
    description: &'static str,
    rationale: &'static str,
    bad_example: &'static str,
    good_example: &'static str,
}

const BUILTIN_RULES: [BuiltinInfo; 18] = [
    BuiltinInfo {
        id: "class-name",
        severity: Severity::Error,
        fixable: false,
        description: "Class names use PascalCase.",
        rationale: "The GDScript style guide names classes in PascalCase, like Godot's own classes. A consistent case tells types apart from variables and functions at a glance.",
        bad_example: "class_name player_controller\nextends Node\n",
        good_example: "class_name PlayerController\nextends Node\n",
    },
    BuiltinInfo {
        id: "comparison-with-itself",
        severity: Severity::Warning,
        fixable: false,
        description: "Comparing an expression with itself is always true or always false.",
        rationale: "`x == x` or `x != x` has a fixed result, so it is almost always a typo for a comparison with another value.",
        bad_example: "func is_target(target, other):\n\treturn target == target\n",
        good_example: "func is_target(target, other):\n\treturn target == other\n",
    },
    BuiltinInfo {
        id: "constant-name",
        severity: Severity::Error,
        fixable: false,
        description: "Constants use CONSTANT_CASE (preloaded constants may use PascalCase).",
        rationale: "The GDScript style guide names constants in CONSTANT_CASE so they stand out from variables. Constants holding a preloaded resource may also use PascalCase, like the class they stand for.",
        bad_example: "const maxSpeed = 10\n",
        good_example: "const MAX_SPEED = 10\nconst Bullet = preload(\"res://bullet.tscn\")\n",
    },
    BuiltinInfo {
        id: "duplicated-load",
        severity: Severity::Warning,
        fixable: false,
        description: "The same resource path is loaded more than once in a file.",
        rationale: "Loading the same path in several places repeats the string and makes renames error-prone. Load it once into a constant and reuse it.",
        bad_example: "var a = load(\"res://icon.svg\")\nvar b = load(\"res://icon.svg\")\n",
        good_example: "const ICON = preload(\"res://icon.svg\")\nvar a = ICON\nvar b = ICON\n",
    },
    BuiltinInfo {
        id: "enum-member-name",
        severity: Severity::Error,
        fixable: false,
        description: "Enum members use CONSTANT_CASE.",
        rationale: "Enum members are constants, so the GDScript style guide names them in CONSTANT_CASE.",
        bad_example: "enum State { idle, chase_player }\n",
        good_example: "enum State { IDLE, CHASE_PLAYER }\n",
    },
    BuiltinInfo {
        id: "enum-name",
        severity: Severity::Error,
        fixable: false,
        description: "Enum names use PascalCase.",
        rationale: "Enums are types, so the GDScript style guide names them in PascalCase like classes.",
        bad_example: "enum state { IDLE, RUN }\n",
        good_example: "enum State { IDLE, RUN }\n",
    },
    BuiltinInfo {
        id: "function-argument-name",
        severity: Severity::Error,
        fixable: false,
        description: "Function arguments use snake_case.",
        rationale: "The GDScript style guide names arguments in snake_case, like variables, so code reads the same everywhere.",
        bad_example: "func move(TargetPosition):\n\tprint(TargetPosition)\n",
        good_example: "func move(target_position):\n\tprint(target_position)\n",
    },
    BuiltinInfo {
        id: "function-name",
        severity: Severity::Error,
        fixable: false,
        description: "Functions use snake_case (or _private_snake_case).",
        rationale: "The GDScript style guide names functions in snake_case, like Godot's own API. A leading underscore marks private functions and virtual callbacks.",
        bad_example: "func moveTowards(target):\n\tprint(target)\n",
        good_example: "func move_towards(target):\n\tprint(target)\n",
    },
    BuiltinInfo {
        id: "load-variable-name",
        severity: Severity::Error,
        fixable: false,
        description: "Variables holding a loaded resource use PascalCase, snake_case or _private_snake_case.",
        rationale: "A variable that holds a loaded scene or script stands for a type, so PascalCase is allowed in addition to the usual variable cases. This id is reported by the `variable-name` rule and is turned on and off with it.",
        bad_example: "var player_SCENE = load(\"res://player.tscn\")\n",
        good_example: "var PlayerScene = load(\"res://player.tscn\")\n",
    },
    BuiltinInfo {
        id: "loop-variable-name",
        severity: Severity::Error,
        fixable: false,
        description: "Loop variables use snake_case.",
        rationale: "Loop variables are variables, so the GDScript style guide names them in snake_case.",
        bad_example: "func f(items):\n\tfor Item in items:\n\t\tprint(Item)\n",
        good_example: "func f(items):\n\tfor item in items:\n\t\tprint(item)\n",
    },
    BuiltinInfo {
        id: "max-line-length",
        severity: Severity::Warning,
        fixable: false,
        description: "Lines are no longer than the configured maximum (100 by default).",
        rationale: "Long lines are hard to read side by side and in diffs. The limit comes from `max-line-length.max` in gdguard.toml, then `max_line_length` in .editorconfig, then 100.",
        bad_example: "var message = \"This line is far too long because it keeps going and going well past the one hundred character limit\"\n",
        good_example: "var message = (\n\t\"This line is split so that every part \"\n\t+ \"stays under the limit\"\n)\n",
    },
    BuiltinInfo {
        id: "no-else-return",
        severity: Severity::Warning,
        fixable: true,
        description: "No `else`/`elif` after an `if` block that ends with `return`.",
        rationale: "When the `if` branch returns, the code after it only runs in the other case anyway. Removing the `else` reduces nesting.",
        bad_example: "func sign_of(x):\n\tif x < 0:\n\t\treturn -1\n\telse:\n\t\treturn 1\n",
        good_example: "func sign_of(x):\n\tif x < 0:\n\t\treturn -1\n\treturn 1\n",
    },
    BuiltinInfo {
        id: "private-access",
        severity: Severity::Error,
        fixable: false,
        description: "Members starting with `_` are not used from outside their class.",
        rationale: "A leading underscore marks a member as private. Using it from another object couples the two classes to internals that may change.",
        bad_example: "func damage(enemy):\n\tenemy._health -= 1\n",
        good_example: "func damage(enemy):\n\tenemy.take_damage(1)\n",
    },
    BuiltinInfo {
        id: "signal-name",
        severity: Severity::Error,
        fixable: false,
        description: "Signals use snake_case.",
        rationale: "The GDScript style guide names signals in snake_case, usually in the past tense, like Godot's built-in signals.",
        bad_example: "signal OnDefeated\n",
        good_example: "signal defeated\n",
    },
    BuiltinInfo {
        id: "standalone-expression",
        severity: Severity::Warning,
        fixable: false,
        description: "Expressions on their own line have an effect.",
        rationale: "A line like `score + 1` computes a value and throws it away. It is usually a mistake for `score += 1` or a leftover.",
        bad_example: "func add_point(score):\n\tscore + 1\n\treturn score\n",
        good_example: "func add_point(score):\n\tscore += 1\n\treturn score\n",
    },
    BuiltinInfo {
        id: "unnecessary-pass",
        severity: Severity::Warning,
        fixable: true,
        description: "No `pass` in a block that has other statements.",
        rationale: "`pass` only exists to fill an empty block. Next to other statements it does nothing.",
        bad_example: "func f():\n\tprint(1)\n\tpass\n",
        good_example: "func f():\n\tprint(1)\n",
    },
    BuiltinInfo {
        id: "unused-argument",
        severity: Severity::Warning,
        fixable: false,
        description: "Function arguments are used, or start with `_`.",
        rationale: "An unused argument is often a bug or a leftover. When the signature must stay (callbacks, overrides), prefix the name with `_` to say so.",
        bad_example: "func _process(delta):\n\tprint(\"tick\")\n",
        good_example: "func _process(_delta):\n\tprint(\"tick\")\n",
    },
    BuiltinInfo {
        id: "variable-name",
        severity: Severity::Error,
        fixable: false,
        description: "Variables use snake_case (or _private_snake_case).",
        rationale: "The GDScript style guide names variables in snake_case. A leading underscore marks private variables.",
        bad_example: "var targetName = \"\"\n",
        good_example: "var target_name = \"\"\n",
    },
];

fn to_meta(info: &BuiltinInfo) -> RuleMeta {
    RuleMeta {
        id: info.id.to_string(),
        description: info.description.to_string(),
        rationale: info.rationale.to_string(),
        bad_example: info.bad_example.to_string(),
        good_example: info.good_example.to_string(),
        default_severity: info.severity,
        fixable: info.fixable,
        origin: RuleOrigin::BuiltIn,
    }
}

/// The metadata of every built-in rule id, sorted by id.
pub fn builtin_rule_metas() -> Vec<RuleMeta> {
    let mut metas = Vec::with_capacity(BUILTIN_RULES.len());
    for info in &BUILTIN_RULES {
        metas.push(to_meta(info));
    }
    metas
}

/// The metadata of one built-in rule id.
pub fn builtin_rule_meta(id: &str) -> Option<RuleMeta> {
    for info in &BUILTIN_RULES {
        if info.id == id {
            return Some(to_meta(info));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::linter::lint_gdscript;
    use crate::linter::rule_config::get_all_rule_names;

    #[test]
    fn every_built_in_rule_has_metadata() {
        for name in get_all_rule_names() {
            assert!(
                builtin_rule_meta(name).is_some(),
                "missing metadata for built-in rule `{}`",
                name
            );
        }
        assert!(builtin_rule_meta("load-variable-name").is_some());
        assert_eq!(BUILTIN_RULES.len(), get_all_rule_names().len() + 1);
    }

    #[test]
    fn ids_are_sorted_and_unique() {
        for index in 1..BUILTIN_RULES.len() {
            assert!(
                BUILTIN_RULES[index - 1].id < BUILTIN_RULES[index].id,
                "`{}` should come before `{}`",
                BUILTIN_RULES[index - 1].id,
                BUILTIN_RULES[index].id
            );
        }
    }

    #[test]
    fn severities_match_what_the_rules_report() {
        for info in &BUILTIN_RULES {
            let diagnostics = lint_gdscript(info.bad_example, "example.gd").unwrap();
            for diagnostic in &diagnostics {
                if diagnostic.rule == info.id {
                    assert_eq!(diagnostic.severity, info.severity, "rule `{}`", info.id);
                }
            }
        }
    }

    #[test]
    fn bad_examples_are_reported_and_good_examples_are_not() {
        for info in &BUILTIN_RULES {
            let bad = lint_gdscript(info.bad_example, "example.gd").unwrap();
            let mut reported = false;
            for diagnostic in &bad {
                if diagnostic.rule == info.id {
                    reported = true;
                }
            }
            assert!(reported, "bad example of `{}` isn't reported", info.id);

            let good = lint_gdscript(info.good_example, "example.gd").unwrap();
            for diagnostic in &good {
                assert_ne!(
                    diagnostic.rule, info.id,
                    "good example of `{}` is reported: {}",
                    info.id, diagnostic.message
                );
            }
        }
    }

    #[test]
    fn descriptions_are_one_line() {
        for info in &BUILTIN_RULES {
            assert!(!info.description.contains('\n'), "rule `{}`", info.id);
            assert!(!info.description.is_empty(), "rule `{}`", info.id);
        }
    }
}
