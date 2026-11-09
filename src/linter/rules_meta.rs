//! Owner: Track B (W3). Rule metadata (description, rationale, bad and good
//! examples) for the built-in rules, used by `explain` and `--list-rules`.
//!
//! Every id in `rule_config::get_all_rule_names()` has an entry, plus
//! `load-variable-name`, which the `variable-name` rule reports for variables
//! holding a loaded resource, and `fix-failed`, which the fix engine reports
//! when a fix would break the code, and `godot-error`, which the optional
//! Godot check reports. A test runs every bad example through the
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

const BUILTIN_RULES: [BuiltinInfo; 40] = [
    BuiltinInfo {
        id: "assigned-but-never-read",
        severity: Severity::Warning,
        fixable: false,
        description: "Assignments to local variables are read.",
        rationale: "A value repeatedly assigned and never read often means a missing use or assignment to the wrong variable.",
        bad_example: "func f():\n\tvar value = 0\n\tvalue = 1\n",
        good_example: "func f():\n\tvar value = 0\n\tvalue = 1\n\tprint(value)\n",
    },
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
        id: "declaration-order",
        severity: Severity::Warning,
        fixable: true,
        description: "Top-level declarations follow the formatter's style-guide order.",
        rationale: "A consistent declaration order makes scripts easier to scan. The formatter's reorder plan keeps region groups, comments and annotations together. Fixes preserve declaration bodies in formatted and unformatted files, except files with fmt: off regions. Reversing a member-variable initializer dependency requires an unsafe fix, including bare identifiers and self.member reads.",
        bad_example: "var health = 100\nsignal died\n",
        good_example: "signal died\nvar health = 100\n",
    },
    BuiltinInfo {
        id: "duplicate-class-name",
        severity: Severity::Error,
        fixable: false,
        description: "Each class_name is declared by one script in the project.",
        rationale: "Godot registers one global class per name. The other script fails to load with `Class \"X\" hides a global script class`, and the editor doesn't always say which file wins.",
        bad_example: "class_name ProjectData\nextends Node\n",
        good_example: "class_name ExampleData\nextends Node\n",
    },
    BuiltinInfo {
        id: "duplicate-dictionary-key",
        severity: Severity::Error,
        fixable: false,
        description: "Dictionary literals don't repeat a key.",
        rationale: "Only the last value of a repeated key is kept, so the earlier entry is dead and usually a typo. Godot refuses to load scripts with duplicate literal keys.",
        bad_example: "var stats = {\"hp\": 10, \"mp\": 5, \"hp\": 20}\n",
        good_example: "var stats = {\"hp\": 10, \"mp\": 5, \"sp\": 20}\n",
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
        id: "fix-failed",
        severity: Severity::Warning,
        fixable: false,
        description: "A fix was not applied because it would break the code.",
        rationale: "The fix engine re-parses the code after each pass. When a fix produces code that doesn't parse, it is rolled back and reported under this id instead, so `--fix` never writes broken code. Fix the original issue by hand; if the fix comes from a custom rule, check its `fix` template.",
        bad_example: "",
        good_example: "",
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
        id: "godot-error",
        severity: Severity::Error,
        fixable: false,
        description: "Godot loads the script without errors (only with `--godot` or `GDGUARD_GODOT`).",
        rationale: "gdguard doesn't check types. With a Godot 4 executable configured, `lint` loads the linted scripts in headless Godot once per project and reports its parse, type and warning-as-error messages under this id.",
        bad_example: "",
        good_example: "",
    },
    BuiltinInfo {
        id: "godot3-syntax",
        severity: Severity::Error,
        fixable: true,
        description: "No Godot 3 syntax that Godot 4 rejects.",
        rationale: "Code copied from Godot 3 examples uses `tool`, `onready`, `export`, `setget`, `yield`, `instance()` and string-method `connect`. Godot 4 rejects or renamed them. Mechanical replacements are offered as fixes (`tool` and `onready` safe, the others unsafe).",
        bad_example: "func _ready():\n\tyield(get_tree(), \"idle_frame\")\n",
        good_example: "func _ready():\n\tawait get_tree().process_frame\n",
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
        id: "missing-class-name",
        severity: Severity::Warning,
        fixable: true,
        description: "Every project script declares a class_name, except autoloads.",
        rationale: "A class_name lets other scripts use the script as a type, which enables typed code and the cross-file checks. Autoload scripts are exempt: they are reached through their singleton name. The unsafe fix adds a PascalCase name from the file name when it is free.",
        bad_example: "extends Node\n",
        good_example: "class_name Example\nextends Node\n",
    },
    BuiltinInfo {
        id: "missing-resource-path",
        severity: Severity::Error,
        fixable: false,
        description: "Paths given to load() and preload() exist in the project.",
        rationale: "A wrong `load()` path only fails at runtime, when the line runs. `res://` and relative paths must name an existing file, and `uid://` paths a UID known from `.uid` files, `.import` files or scene headers.",
        bad_example: "var icon = load(\"res://missing.png\")\n",
        good_example: "var data = load(\"res://data.gd\")\n",
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
        id: "parse-error",
        severity: Severity::Error,
        fixable: false,
        description: "The source parses as GDScript.",
        rationale: "A syntax error prevents reliable analysis. Repair the reported range and lint again; use Godot for additional parser and type checks.",
        bad_example: "func f(:\n",
        good_example: "func f():\n\tpass\n",
    },
    BuiltinInfo {
        id: "prefer-uid-path",
        severity: Severity::Warning,
        fixable: true,
        description: "load() and preload() use uid:// paths when the resource has a UID.",
        rationale: "UID paths keep working when files are moved or renamed. Only paths with a known UID (from `.uid` files, `.import` files or scene headers) are reported, and the safe fix replaces the path with the UID.",
        bad_example: "const Data = preload(\"res://data.gd\")\n",
        good_example: "const Data = preload(\"uid://cexampledata\")\n",
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
        id: "scene-connection-missing-method",
        severity: Severity::Error,
        fixable: false,
        description: "Scene connections target methods the script defines.",
        rationale: "A `[connection]` in a `.tscn` file names its method as a string. Godot only reports a missing method when the signal is emitted. Methods inherited from engine classes can't be checked, except that engine methods never start with `_` unless they are virtual callbacks.",
        bad_example: "extends Control\n",
        good_example: "extends Control\n\nfunc _on_button_pressed():\n\tpass\n",
    },
    BuiltinInfo {
        id: "script-error",
        severity: Severity::Error,
        fixable: false,
        description: "A JavaScript rule failed or exceeded its sandbox limits.",
        rationale: "A failing custom rule cannot reliably check this file. Fix the rule, invalid report, or resource usage before trusting the lint result.",
        bad_example: "",
        good_example: "",
    },
    BuiltinInfo {
        id: "self-assignment",
        severity: Severity::Warning,
        fixable: false,
        description: "Local variables and parameters aren't assigned to themselves.",
        rationale: "`x = x` on a local has no effect and is usually a typo for another value. Members are skipped: `property = property` is a Godot idiom to run a setter again.",
        bad_example: "func set_speed(speed):\n\tspeed = speed\n",
        good_example: "var speed = 0\nfunc set_speed(value):\n\tspeed = value\n",
    },
    BuiltinInfo {
        id: "shadowed-variable",
        severity: Severity::Warning,
        fixable: false,
        description: "Local names do not shadow outer declarations.",
        rationale: "Shadowing makes it easy to read or assign a different variable than intended.",
        bad_example: "var value = 1\nfunc f():\n\tvar value = 2\n\tprint(value)\n",
        good_example: "var value = 1\nfunc f():\n\tvar local_value = 2\n\tprint(local_value)\n",
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
        id: "unknown-member-on-project-class",
        severity: Severity::Warning,
        fixable: false,
        description: "Declared project types refer to existing project members.",
        rationale: "A missing member on a known project type is often a typo. Engine, dynamic and inferred types remain unknown.",
        bad_example: "func f(data: ProjectData):\n\treturn data.missing\n",
        good_example: "func f(data: ProjectData):\n\treturn data.value\n",
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
        id: "unreachable-code",
        severity: Severity::Warning,
        fixable: false,
        description: "No statements after `return`, `break` or `continue` in the same block.",
        rationale: "Code after these statements never runs. Godot only warns about it in the editor, so headless checks and agents miss it.",
        bad_example: "func f():\n\treturn 1\n\tprint(\"done\")\n",
        good_example: "func f():\n\tprint(\"done\")\n\treturn 1\n",
    },
    BuiltinInfo {
        id: "unused-argument",
        severity: Severity::Warning,
        fixable: true,
        description: "Function arguments are used, or start with `_`.",
        rationale: "An unused argument is often a bug or a leftover. When the signature must stay (callbacks, overrides), prefix the name with `_` to say so.",
        bad_example: "func _process(delta):\n\tprint(\"tick\")\n",
        good_example: "func _process(_delta):\n\tprint(\"tick\")\n",
    },
    BuiltinInfo {
        id: "unused-ignore",
        severity: Severity::Warning,
        fixable: true,
        description: "Ignore comments suppress an existing issue.",
        rationale: "An unused ignore can hide future issues. Remove the comment or the rule ids that no longer apply. Disabled rules are exempt, since they cannot produce an issue.",
        bad_example: "# gdguard-ignore-next-line unused-argument -- old callback\nfunc f():\n\tpass\n",
        good_example: "func f():\n\tpass\n",
    },
    BuiltinInfo {
        id: "unused-local-signal",
        severity: Severity::Warning,
        fixable: false,
        description: "Signals are emitted or connected in their declaring file.",
        rationale: "Unused signals can be leftovers; external or scene-only usage requires a documented exception.",
        bad_example: "signal changed\n",
        good_example: "signal changed\nfunc f():\n\tchanged.emit()\n",
    },
    BuiltinInfo {
        id: "unused-private-function",
        severity: Severity::Warning,
        fixable: false,
        description: "Private functions are referenced or are Godot virtual callbacks.",
        rationale: "A private helper with no local references may be dead code. Check external callbacks before removing it.",
        bad_example: "func _helper():\n\tpass\n",
        good_example: "func _helper():\n\tpass\nfunc f():\n\t_helper()\n",
    },
    BuiltinInfo {
        id: "unused-signal",
        severity: Severity::Warning,
        fixable: false,
        description: "Project signals are emitted or connected somewhere in the project.",
        rationale: "Unused signals can be removed after reviewing external and dynamic use. Script calls and scene connections count as use.",
        bad_example: "signal unused\n",
        good_example: "signal used\nfunc f():\n\tused.emit()\n",
    },
    BuiltinInfo {
        id: "unused-variable",
        severity: Severity::Warning,
        fixable: true,
        description: "Local variables are used or start with an underscore.",
        rationale: "Unused locals can hide mistakes or unnecessary work. Removing a declaration can affect behavior, so the offered fix is unsafe.",
        bad_example: "func f():\n\tvar value = 1\n",
        good_example: "func f():\n\tvar value = 1\n\tprint(value)\n",
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

    fn lint_example(
        info: &BuiltinInfo,
        source: &str,
    ) -> Vec<crate::linter::diagnostic::Diagnostic> {
        if matches!(
            info.id,
            "unused-signal"
                | "unknown-member-on-project-class"
                | "duplicate-class-name"
                | "missing-class-name"
                | "missing-resource-path"
                | "prefer-uid-path"
                | "scene-connection-missing-method"
        ) {
            let root = std::env::temp_dir().join(format!(
                "gdguard-meta-{}-{}-{:?}-{}",
                info.id,
                std::process::id(),
                std::thread::current().id(),
                if source == info.bad_example {
                    "bad"
                } else {
                    "good"
                }
            ));
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(root.join("project.godot"), "config_version=5\n").unwrap();
            std::fs::write(
                root.join("data.gd"),
                "class_name ProjectData\nvar value = 0\n",
            )
            .unwrap();
            std::fs::write(root.join("data.gd.uid"), "uid://cexampledata\n").unwrap();
            if info.id == "scene-connection-missing-method" {
                std::fs::write(
                    root.join("ui.tscn"),
                    concat!(
                        "[gd_scene format=3]\n\n",
                        "[ext_resource type=\"Script\" path=\"res://example.gd\" id=\"1\"]\n\n",
                        "[node name=\"UI\" type=\"Control\"]\n",
                        "script = ExtResource(\"1\")\n\n",
                        "[node name=\"Button\" type=\"Button\" parent=\".\"]\n\n",
                        "[connection signal=\"pressed\" from=\"Button\" to=\".\" method=\"_on_button_pressed\"]\n",
                    ),
                )
                .unwrap();
            }
            let path = root.join("example.gd");
            std::fs::write(&path, source).unwrap();
            let result = crate::linter::api::lint_source(
                &path,
                source,
                &crate::linter::api::Config::default(),
            )
            .unwrap();
            let _ = std::fs::remove_dir_all(root);
            result.diagnostics
        } else {
            lint_gdscript(source, "example.gd").unwrap()
        }
    }

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
        assert!(builtin_rule_meta("fix-failed").is_some());
        assert!(builtin_rule_meta("unused-ignore").is_some());
        assert!(builtin_rule_meta("godot-error").is_some());
        assert_eq!(BUILTIN_RULES.len(), get_all_rule_names().len() + 6);
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
            let diagnostics = lint_example(info, info.bad_example);
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
            // `fix-failed` comes from the fix engine, not from linting code.
            if info.bad_example.is_empty() {
                assert!(matches!(
                    info.id,
                    "fix-failed" | "script-error" | "godot-error"
                ));
                continue;
            }
            let bad = lint_example(info, info.bad_example);
            let mut reported = false;
            for diagnostic in &bad {
                if diagnostic.rule == info.id {
                    reported = true;
                }
            }
            assert!(reported, "bad example of `{}` isn't reported", info.id);

            let good = lint_example(info, info.good_example);
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
